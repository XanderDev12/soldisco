use std::{collections::BTreeMap, time::Duration};

use soldisco_discovery_engine::{
    ObservationTarget, ObservationWindowRegistry, ObservationWindowToken,
};
use soldisco_domain::ChainCoordinate;
use soldisco_solana_rpc::ProgramLogNotification;
use soldisco_source_pump::{DecodeError, PumpEvent, PumpProgram, decode_program_data_log};
use thiserror::Error;

const MAX_SOURCE_CLOCK_SKEW: Duration = Duration::from_secs(10);

pub const PUMP_PROGRAMS: [PumpProgram; 2] = [PumpProgram::Pump, PumpProgram::PumpSwap];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BatchPurpose {
    Discovery,
    TrackedActivity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntakeDropReason {
    FailedTransaction,
    Irrelevant,
    MalformedLogs,
    StaleDiscovery,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NotificationDisposition {
    FetchDiscovery {
        seeds: Vec<DiscoverySeed>,
        tracked_windows: Vec<ObservationWindowToken>,
    },
    CollectTrackedActivity(Vec<ObservationWindowToken>),
    Drop(IntakeDropReason),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoverySeed {
    pub target: ObservationTarget,
    pub mint: String,
    pub source_event_time_unix_ms: i64,
}

/// One Anchor `Program data:` record attributed to an exact top-level
/// transaction instruction. Event indexes are scoped to that instruction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScopedProgramData {
    pub coordinate: ChainCoordinate,
    pub log: String,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum LogScopeError {
    #[error("transaction signature cannot be empty")]
    EmptySignature,
    #[error("program invocation depth {depth} is inconsistent with the log stack")]
    InvalidInvocationDepth { depth: usize },
    #[error("top-level transaction contains more than {0} instructions")]
    TooManyInstructions(u16),
    #[error("one transaction instruction contains more than {0} program-data events")]
    TooManyEvents(u16),
}

/// Classifies direct Anchor events already present in one successful PubSub
/// notification. CPI-only events are intentionally invisible in this
/// live-first mode; the collector never spends HTTP capacity on an
/// unclassified transaction.
#[must_use]
pub fn classify_notification(
    notification: &ProgramLogNotification,
    windows: &ObservationWindowRegistry,
    maximum_discovery_age: Duration,
    observed_at_unix_ms: i64,
    now_unix_ms: i64,
) -> NotificationDisposition {
    if !notification.succeeded() {
        return NotificationDisposition::Drop(IntakeDropReason::FailedTransaction);
    }

    let mut discoveries = Vec::new();
    let mut tracked_windows = Vec::new();
    let mut stale_discovery = false;
    let mut malformed = false;
    for program in PUMP_PROGRAMS {
        let scoped = match scope_program_data_logs(
            program.program_id(),
            notification.slot,
            None,
            &notification.signature,
            &notification.log_messages,
        ) {
            Ok(scoped) => scoped,
            Err(_) => {
                malformed = true;
                continue;
            }
        };

        for record in scoped {
            let decoded =
                match decode_program_data_log(program.program_id(), record.coordinate, &record.log)
                {
                    Ok(decoded) => decoded,
                    Err(DecodeError::UnknownDiscriminator { .. }) => continue,
                    Err(_) => {
                        malformed = true;
                        continue;
                    }
                };

            if let Some(seed) = discovery_seed(&decoded.event) {
                if discovery_event_is_fresh(&decoded.event, now_unix_ms, maximum_discovery_age) {
                    if !discoveries
                        .iter()
                        .any(|existing: &DiscoverySeed| existing.target == seed.target)
                    {
                        discoveries.push(seed);
                    }
                } else {
                    stale_discovery = true;
                }
                continue;
            }

            if let Some(token) = event_tracking_target(&decoded.event)
                .and_then(|target| windows.matching_token(&target, observed_at_unix_ms))
                && !tracked_windows.iter().any(|existing| existing == &token)
            {
                tracked_windows.push(token);
            }
        }
    }

    if !discoveries.is_empty() {
        NotificationDisposition::FetchDiscovery {
            seeds: discoveries,
            tracked_windows,
        }
    } else if !tracked_windows.is_empty() {
        NotificationDisposition::CollectTrackedActivity(tracked_windows)
    } else if stale_discovery {
        NotificationDisposition::Drop(IntakeDropReason::StaleDiscovery)
    } else if malformed {
        NotificationDisposition::Drop(IntakeDropReason::MalformedLogs)
    } else {
        NotificationDisposition::Drop(IntakeDropReason::Irrelevant)
    }
}

#[must_use]
pub const fn is_discovery_event(event: &PumpEvent) -> bool {
    matches!(
        event,
        PumpEvent::Create(_) | PumpEvent::PumpSwapCreatePool(_)
    )
}

#[must_use]
pub fn discovery_seed(event: &PumpEvent) -> Option<DiscoverySeed> {
    match event {
        PumpEvent::Create(event) => Some(DiscoverySeed {
            target: ObservationTarget::PumpMint(event.mint.clone()),
            mint: event.mint.clone(),
            source_event_time_unix_ms: event.timestamp.saturating_mul(1_000),
        }),
        PumpEvent::PumpSwapCreatePool(event) => Some(DiscoverySeed {
            target: ObservationTarget::PumpSwapPool(event.pool.clone()),
            mint: event.base_mint.clone(),
            source_event_time_unix_ms: event.timestamp.saturating_mul(1_000),
        }),
        PumpEvent::Trade(_)
        | PumpEvent::Complete(_)
        | PumpEvent::CompletePumpAmmMigration(_)
        | PumpEvent::PumpSwapBuy(_)
        | PumpEvent::PumpSwapSell(_) => None,
    }
}

#[must_use]
pub fn discovery_event_is_fresh(
    event: &PumpEvent,
    now_unix_ms: i64,
    maximum_age: Duration,
) -> bool {
    if !is_discovery_event(event) {
        return false;
    }
    let Some(event_unix_ms) = event.timestamp().checked_mul(1_000) else {
        return false;
    };
    let maximum_age_ms = i64::try_from(maximum_age.as_millis()).unwrap_or(i64::MAX);
    let maximum_future_ms = i64::try_from(MAX_SOURCE_CLOCK_SKEW.as_millis()).unwrap_or(i64::MAX);
    event_unix_ms >= now_unix_ms.saturating_sub(maximum_age_ms)
        && event_unix_ms <= now_unix_ms.saturating_add(maximum_future_ms)
}

#[must_use]
pub fn event_is_in_active_window(
    event: &PumpEvent,
    windows: &ObservationWindowRegistry,
    observed_at_unix_ms: i64,
) -> bool {
    event_tracking_target(event)
        .and_then(|target| windows.matching_token(&target, observed_at_unix_ms))
        .is_some()
}

#[must_use]
pub fn event_matches_confirmed_window(
    event: &PumpEvent,
    tokens: &[ObservationWindowToken],
    observed_at_unix_ms: i64,
) -> bool {
    event_tracking_target(event).is_some_and(|target| {
        tokens
            .iter()
            .any(|token| token.target() == &target && token.is_confirmed_at(observed_at_unix_ms))
    })
}

#[must_use]
pub fn event_tracking_target(event: &PumpEvent) -> Option<ObservationTarget> {
    match event {
        PumpEvent::Trade(event) => Some(ObservationTarget::PumpMint(event.mint.clone())),
        PumpEvent::Complete(event) => Some(ObservationTarget::PumpMint(event.mint.clone())),
        PumpEvent::CompletePumpAmmMigration(event) => {
            Some(ObservationTarget::PumpMint(event.mint.clone()))
        }
        PumpEvent::PumpSwapBuy(event) => Some(ObservationTarget::PumpSwapPool(event.pool.clone())),
        PumpEvent::PumpSwapSell(event) => Some(ObservationTarget::PumpSwapPool(event.pool.clone())),
        PumpEvent::Create(_) | PumpEvent::PumpSwapCreatePool(_) => None,
    }
}

/// Walk Solana execution logs and keep data emitted only while `program_id` is
/// the active invocation. A depth-one `invoke` is the exact top-level
/// transaction instruction boundary; nested CPIs inherit that index.
pub fn scope_program_data_logs(
    program_id: &str,
    slot: u64,
    transaction_index: Option<u64>,
    signature: &str,
    logs: &[String],
) -> Result<Vec<ScopedProgramData>, LogScopeError> {
    if signature.trim().is_empty() {
        return Err(LogScopeError::EmptySignature);
    }

    let mut stack: Vec<(String, u16)> = Vec::new();
    let mut next_instruction_index = 0_u16;
    let mut event_indexes = BTreeMap::<u16, u16>::new();
    let mut scoped = Vec::new();

    for log in logs {
        if let Some((invoked_program, depth)) = parse_invoke(log) {
            if depth == 0 || depth > stack.len().saturating_add(1) {
                return Err(LogScopeError::InvalidInvocationDepth { depth });
            }

            if depth == 1 {
                stack.clear();
                let instruction_index = next_instruction_index;
                next_instruction_index = next_instruction_index
                    .checked_add(1)
                    .ok_or(LogScopeError::TooManyInstructions(u16::MAX))?;
                stack.push((invoked_program.to_owned(), instruction_index));
            } else {
                stack.truncate(depth - 1);
                let instruction_index = stack
                    .last()
                    .map(|(_, instruction_index)| *instruction_index)
                    .ok_or(LogScopeError::InvalidInvocationDepth { depth })?;
                stack.push((invoked_program.to_owned(), instruction_index));
            }
            continue;
        }

        if let Some(exited_program) = parse_exit(log) {
            if stack
                .last()
                .is_some_and(|(active_program, _)| active_program == exited_program)
            {
                stack.pop();
            }
            continue;
        }

        if !log.starts_with("Program data: ") {
            continue;
        }
        let Some((active_program, instruction_index)) = stack.last() else {
            continue;
        };
        if active_program != program_id {
            continue;
        }

        let event_index = event_indexes.entry(*instruction_index).or_default();
        let coordinate = ChainCoordinate {
            slot,
            transaction_index,
            signature: signature.to_owned(),
            instruction_index: *instruction_index,
            event_index: *event_index,
        };
        *event_index = event_index
            .checked_add(1)
            .ok_or(LogScopeError::TooManyEvents(u16::MAX))?;
        scoped.push(ScopedProgramData {
            coordinate,
            log: log.clone(),
        });
    }

    Ok(scoped)
}

fn parse_invoke(log: &str) -> Option<(&str, usize)> {
    let suffix = log.strip_prefix("Program ")?;
    let (program_id, depth) = suffix.split_once(" invoke [")?;
    let depth = depth.strip_suffix(']')?.parse().ok()?;
    Some((program_id, depth))
}

fn parse_exit(log: &str) -> Option<&str> {
    let suffix = log.strip_prefix("Program ")?;
    suffix.strip_suffix(" success").or_else(|| {
        suffix
            .split_once(" failed:")
            .map(|(program_id, _)| program_id)
    })
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use soldisco_discovery_engine::ObservationWindowRegistry;
    use soldisco_domain::{MarketIdentity, Network, Venue};
    use soldisco_solana_rpc::ProgramLogNotification;
    use soldisco_source_pump::{
        COMPLETE_EVENT_DISCRIMINATOR, CREATE_EVENT_DISCRIMINATOR, CREATE_POOL_EVENT_DISCRIMINATOR,
        PUMP_PROGRAM_ID, PUMP_SWAP_BUY_EVENT_DISCRIMINATOR, PUMP_SWAP_PROGRAM_ID, PumpEvent,
        decode_anchor_event,
    };

    use super::{
        IntakeDropReason, LogScopeError, NotificationDisposition, classify_notification,
        scope_program_data_logs,
    };

    const OTHER: &str = "other";

    fn push_string(bytes: &mut Vec<u8>, value: &str) {
        bytes.extend(
            u32::try_from(value.len())
                .expect("fixture string fits in u32")
                .to_le_bytes(),
        );
        bytes.extend(value.as_bytes());
    }

    fn push_pubkey(bytes: &mut Vec<u8>, marker: u8) {
        bytes.extend([marker; 32]);
    }

    fn create_event(timestamp: i64) -> Vec<u8> {
        let mut bytes = CREATE_EVENT_DISCRIMINATOR.to_vec();
        push_string(&mut bytes, "Fixture Coin");
        push_string(&mut bytes, "FIX");
        push_string(&mut bytes, "https://example.invalid/fixture.json");
        for marker in 1_u8..=4 {
            push_pubkey(&mut bytes, marker);
        }
        bytes.extend(timestamp.to_le_bytes());
        for value in 1_u64..=4 {
            bytes.extend((value * 1_000).to_le_bytes());
        }
        push_pubkey(&mut bytes, 5);
        bytes.push(0);
        bytes.push(0);
        push_pubkey(&mut bytes, 6);
        bytes.extend(5_000_u64.to_le_bytes());
        bytes
    }

    fn complete_event(timestamp: i64) -> Vec<u8> {
        let mut bytes = COMPLETE_EVENT_DISCRIMINATOR.to_vec();
        push_pubkey(&mut bytes, 1);
        push_pubkey(&mut bytes, 2);
        push_pubkey(&mut bytes, 3);
        bytes.extend(timestamp.to_le_bytes());
        push_pubkey(&mut bytes, 9);
        bytes
    }

    fn pump_swap_create_pool_event(timestamp: i64) -> Vec<u8> {
        let mut bytes = CREATE_POOL_EVENT_DISCRIMINATOR.to_vec();
        bytes.extend(timestamp.to_le_bytes());
        bytes.extend(7_u16.to_le_bytes());
        push_pubkey(&mut bytes, 1);
        push_pubkey(&mut bytes, 2);
        push_pubkey(&mut bytes, 3);
        bytes.push(6);
        bytes.push(9);
        for value in 1_u64..=7 {
            bytes.extend((value * 10).to_le_bytes());
        }
        bytes.push(254);
        for marker in [1_u8, 5, 6, 7, 8] {
            push_pubkey(&mut bytes, marker);
        }
        bytes.push(1);
        bytes
    }

    fn pump_swap_buy_event(timestamp: i64) -> Vec<u8> {
        let mut bytes = PUMP_SWAP_BUY_EVENT_DISCRIMINATOR.to_vec();
        bytes.extend(timestamp.to_le_bytes());
        for value in 1_u64..=13 {
            bytes.extend((value * 100).to_le_bytes());
        }
        for marker in 1_u8..=7 {
            push_pubkey(&mut bytes, marker);
        }
        bytes.extend(14_u64.to_le_bytes());
        bytes.extend(15_u64.to_le_bytes());
        bytes.push(1);
        for value in 16_u64..=18 {
            bytes.extend(value.to_le_bytes());
        }
        bytes.extend(timestamp.to_le_bytes());
        bytes.extend(19_u64.to_le_bytes());
        push_string(&mut bytes, "buy_exact_quote_in");
        for value in 20_u64..=23 {
            bytes.extend(value.to_le_bytes());
        }
        bytes.extend((-24_i128).to_le_bytes());
        bytes.push(1);
        bytes.extend(25_u64.to_le_bytes());
        bytes
    }

    fn notification(event: Vec<u8>) -> ProgramLogNotification {
        notification_events(vec![event])
    }

    fn notification_events(events: Vec<Vec<u8>>) -> ProgramLogNotification {
        notification_for_program(PUMP_PROGRAM_ID, events)
    }

    fn notification_for_program(program_id: &str, events: Vec<Vec<u8>>) -> ProgramLogNotification {
        let mut log_messages = vec![format!("Program {program_id} invoke [1]")];
        log_messages.extend(
            events
                .into_iter()
                .map(|event| format!("Program data: {}", STANDARD.encode(event))),
        );
        log_messages.push(format!("Program {program_id} success"));
        ProgramLogNotification {
            subscription_id: 1,
            slot: 42,
            signature: "signature".to_owned(),
            log_messages,
            transaction_error: None,
        }
    }

    #[test]
    fn fresh_discovery_is_fetched_and_stale_discovery_is_dropped() {
        let windows = ObservationWindowRegistry::new(8);
        let now = 1_720_000_010_000_i64;

        assert!(matches!(
            classify_notification(
                &notification(create_event(1_720_000_009)),
                &windows,
                Duration::from_secs(5),
                now,
                now,
            ),
            NotificationDisposition::FetchDiscovery { .. }
        ));
        assert_eq!(
            classify_notification(
                &notification(create_event(1_720_000_000)),
                &windows,
                Duration::from_secs(5),
                now,
                now,
            ),
            NotificationDisposition::Drop(IntakeDropReason::StaleDiscovery)
        );
    }

    #[test]
    fn lifecycle_events_are_collected_only_during_the_matching_window() {
        let evidence = complete_event(1_720_000_001);
        let decoded = decode_anchor_event(
            PUMP_PROGRAM_ID,
            soldisco_domain::ChainCoordinate {
                slot: 42,
                transaction_index: None,
                signature: "signature".to_owned(),
                instruction_index: 0,
                event_index: 0,
            },
            &evidence,
        )
        .expect("complete fixture should decode");
        let PumpEvent::Complete(event) = decoded.event else {
            panic!("expected complete event");
        };
        let market = MarketIdentity {
            network: Network::SolanaMainnet,
            mint: event.mint,
            venue: Venue::PumpBondingCurve,
            market_address: "curve".to_owned(),
            quote_mint: Some(event.quote_mint),
        };
        let windows = ObservationWindowRegistry::new(8);
        let provision = windows.provision_target(
            soldisco_discovery_engine::ObservationTarget::for_market(&market)
                .expect("Pump market target"),
            market.mint,
            1_720_000_000_000,
            Duration::from_secs(5),
        );

        let disposition = classify_notification(
            &notification(evidence.clone()),
            &windows,
            Duration::from_secs(5),
            1_720_000_004_000,
            1_720_000_004_000,
        );
        let NotificationDisposition::CollectTrackedActivity(tokens) = disposition else {
            panic!("provisional window should capture early activity");
        };
        assert_eq!(tokens, vec![provision.token.clone()]);
        assert!(!tokens[0].is_confirmed_at(1_720_000_004_000));
        assert!(provision.token.confirm());
        assert_eq!(
            classify_notification(
                &notification(evidence),
                &windows,
                Duration::from_secs(5),
                1_720_000_005_000,
                1_720_000_005_000,
            ),
            NotificationDisposition::Drop(IntakeDropReason::Irrelevant)
        );
    }

    #[test]
    fn failed_notifications_never_reach_discovery_fetching() {
        let mut failed = notification(create_event(1_720_000_009));
        failed.transaction_error = Some("failed".to_owned());

        assert_eq!(
            classify_notification(
                &failed,
                &ObservationWindowRegistry::new(8),
                Duration::from_secs(5),
                1_720_000_010_000,
                1_720_000_010_000,
            ),
            NotificationDisposition::Drop(IntakeDropReason::FailedTransaction)
        );
    }

    #[test]
    fn discovery_keeps_activity_tokens_for_other_open_markets() {
        let now = 1_720_000_010_000_i64;
        let activity_evidence = complete_event(1_720_000_009);
        let decoded = decode_anchor_event(
            PUMP_PROGRAM_ID,
            soldisco_domain::ChainCoordinate {
                slot: 42,
                transaction_index: None,
                signature: "activity".to_owned(),
                instruction_index: 0,
                event_index: 0,
            },
            &activity_evidence,
        )
        .expect("complete fixture");
        let PumpEvent::Complete(activity) = decoded.event else {
            panic!("expected lifecycle event");
        };
        let windows = ObservationWindowRegistry::new(8);
        let provision = windows.provision_target(
            soldisco_discovery_engine::ObservationTarget::PumpMint(activity.mint.clone()),
            activity.mint,
            now.saturating_sub(1_000),
            Duration::from_secs(5),
        );
        assert!(provision.token.confirm());

        let disposition = classify_notification(
            &notification_events(vec![create_event(1_720_000_009), activity_evidence]),
            &windows,
            Duration::from_secs(5),
            now,
            now,
        );
        let NotificationDisposition::FetchDiscovery {
            seeds,
            tracked_windows,
        } = disposition
        else {
            panic!("fresh creation should trigger discovery");
        };

        assert_eq!(seeds.len(), 1);
        assert_eq!(tracked_windows, vec![provision.token]);
    }

    #[test]
    fn pump_swap_pool_discovery_routes_matching_buy_activity() {
        let now = 1_720_000_010_000_i64;
        let pool_evidence = pump_swap_create_pool_event(1_720_000_009);
        let discovery = classify_notification(
            &notification_for_program(PUMP_SWAP_PROGRAM_ID, vec![pool_evidence.clone()]),
            &ObservationWindowRegistry::new(8),
            Duration::from_secs(5),
            now,
            now,
        );
        let NotificationDisposition::FetchDiscovery { seeds, .. } = discovery else {
            panic!("fresh PumpSwap pool should trigger discovery");
        };
        assert_eq!(seeds.len(), 1);

        let windows = ObservationWindowRegistry::new(8);
        let seed = seeds.into_iter().next().expect("pool seed");
        let provision =
            windows.provision_target(seed.target, seed.mint, now, Duration::from_secs(5));
        assert!(provision.token.confirm());
        let activity = classify_notification(
            &notification_for_program(
                PUMP_SWAP_PROGRAM_ID,
                vec![pump_swap_buy_event(1_720_000_010)],
            ),
            &windows,
            Duration::from_secs(5),
            now.saturating_add(1),
            now.saturating_add(1),
        );

        let NotificationDisposition::CollectTrackedActivity(tokens) = activity else {
            panic!("matching PumpSwap buy should be tracked");
        };
        assert_eq!(tokens, vec![provision.token]);
    }

    #[test]
    fn scopes_nested_events_to_their_top_level_instruction() {
        let logs = vec![
            format!("Program {OTHER} invoke [1]"),
            format!("Program {PUMP_PROGRAM_ID} invoke [2]"),
            "Program data: first".to_owned(),
            format!("Program {PUMP_PROGRAM_ID} success"),
            format!("Program {OTHER} success"),
            format!("Program {PUMP_PROGRAM_ID} invoke [1]"),
            "Program data: second".to_owned(),
            "Program data: third".to_owned(),
            format!("Program {PUMP_PROGRAM_ID} success"),
        ];

        let scoped = scope_program_data_logs(PUMP_PROGRAM_ID, 42, None, "signature", &logs)
            .expect("valid execution logs");

        assert_eq!(scoped.len(), 3);
        assert_eq!(scoped[0].coordinate.instruction_index, 0);
        assert_eq!(scoped[0].coordinate.event_index, 0);
        assert_eq!(scoped[1].coordinate.instruction_index, 1);
        assert_eq!(scoped[1].coordinate.event_index, 0);
        assert_eq!(scoped[2].coordinate.instruction_index, 1);
        assert_eq!(scoped[2].coordinate.event_index, 1);
    }

    #[test]
    fn rejects_impossible_invocation_depth() {
        let error = scope_program_data_logs(
            PUMP_PROGRAM_ID,
            1,
            None,
            "signature",
            &[format!("Program {PUMP_PROGRAM_ID} invoke [2]")],
        )
        .expect_err("depth two requires a parent");

        assert_eq!(error, LogScopeError::InvalidInvocationDepth { depth: 2 });
    }
}
