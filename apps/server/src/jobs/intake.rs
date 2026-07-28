use std::{collections::BTreeMap, time::Duration};

use soldisco_discovery_engine::{
    ObservationTarget, ObservationWindowRegistry, ObservationWindowToken, WindowIncompleteReason,
};
use soldisco_domain::{ChainCoordinate, Network};
use soldisco_solana_rpc::ProgramLogNotification;
use soldisco_source_pump::{
    DecodeError, PumpEvent, PumpProgram, decode_program_data_log, is_pinned_event_discriminator,
};
use thiserror::Error;

use super::pump_swap_pair::normalize_pump_swap_pair;

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
    #[error("Solana explicitly reported truncated transaction logs")]
    TruncatedLogs,
    #[error("program invocation depth {depth} is inconsistent with the log stack")]
    InvalidInvocationDepth { depth: usize },
    #[error("program {program_id} exited without being the active invocation")]
    UnexpectedProgramExit { program_id: String },
    #[error("transaction logs ended with {depth} unclosed program invocations")]
    UnclosedInvocations { depth: usize },
    #[error("top-level transaction contains more than {0} instructions")]
    TooManyInstructions(u16),
    #[error("one transaction instruction contains more than {0} program-data events")]
    TooManyEvents(u16),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct DirectEventCoverage {
    eligible_events: u16,
    invalid_event: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct SelfCpiCoverage {
    successful_event_calls: u16,
    pending_direct_event: bool,
    pairing_gap: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct InvocationFrame {
    program_id: String,
    instruction_index: u16,
    is_target_self_cpi: bool,
    silent_candidate: bool,
    candidate_for_pending_event: bool,
}

/// Classifies direct Anchor events already present in one successful PubSub
/// notification. For the two pinned Pump programs, a successful Anchor
/// silent event self-CPI is currently accompanied by one directly logged copy
/// of the same event in its top-level instruction. PubSub omits the CPI bytes,
/// so completeness is inferred only when those records pair exactly and in
/// order inside each instruction. Known event kinds must decode fully; known
/// but intentionally ignored IDL events remain structurally eligible, while a
/// discriminator absent from the pinned IDL fails semantic completeness. The
/// collector never spends HTTP capacity on activity transactions.
#[must_use]
pub fn classify_notification(
    notification: &ProgramLogNotification,
    windows: &ObservationWindowRegistry,
    network: Network,
    maximum_discovery_age: Duration,
    observed_at_unix_ms: i64,
    now_unix_ms: i64,
) -> NotificationDisposition {
    if !notification.succeeded() {
        return NotificationDisposition::Drop(IntakeDropReason::FailedTransaction);
    }

    let mut discoveries = Vec::new();
    let mut tracked_windows: Vec<ObservationWindowToken> = Vec::new();
    let mut stale_discovery = false;
    let mut malformed = false;
    for program in PUMP_PROGRAMS {
        if !notification_mentions_program(program.program_id(), &notification.log_messages) {
            continue;
        }
        let self_cpi_coverage =
            match successful_self_cpi_coverage(program.program_id(), &notification.log_messages) {
                Ok(coverage) => coverage,
                Err(_) => {
                    malformed = true;
                    record_source_coverage_gap(
                        program,
                        windows,
                        observed_at_unix_ms,
                        &notification.signature,
                        WindowIncompleteReason::DecodeGap,
                    );
                    continue;
                }
            };
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
                record_source_coverage_gap(
                    program,
                    windows,
                    observed_at_unix_ms,
                    &notification.signature,
                    WindowIncompleteReason::DecodeGap,
                );
                continue;
            }
        };

        let mut direct_coverage = BTreeMap::<u16, DirectEventCoverage>::new();
        let mut decoded_events = Vec::new();
        let mut has_decode_gap = false;
        for record in scoped {
            let coverage = direct_coverage
                .entry(record.coordinate.instruction_index)
                .or_default();
            let decoded =
                match decode_program_data_log(program.program_id(), record.coordinate, &record.log)
                {
                    Ok(decoded) => {
                        coverage.eligible_events = coverage
                            .eligible_events
                            .checked_add(1)
                            .expect("scoping already limits events per instruction to u16");
                        decoded
                    }
                    Err(DecodeError::UnknownDiscriminator {
                        program,
                        discriminator,
                    }) if is_pinned_event_discriminator(program, discriminator) => {
                        coverage.eligible_events = coverage
                            .eligible_events
                            .checked_add(1)
                            .expect("scoping already limits events per instruction to u16");
                        continue;
                    }
                    Err(_) => {
                        coverage.invalid_event = true;
                        has_decode_gap = true;
                        malformed = true;
                        continue;
                    }
                };
            decoded_events.push(decoded);
        }

        if has_decode_gap {
            record_source_coverage_gap(
                program,
                windows,
                observed_at_unix_ms,
                &notification.signature,
                WindowIncompleteReason::DecodeGap,
            );
        } else if event_cpi_coverage_is_incomplete(&self_cpi_coverage, &direct_coverage) {
            record_source_coverage_gap(
                program,
                windows,
                observed_at_unix_ms,
                &notification.signature,
                WindowIncompleteReason::CpiInstructionDataUnavailable,
            );
        }

        for decoded in decoded_events {
            if let Some(seed) = discovery_seed(&decoded.event, network) {
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

            if let Some(target) = event_tracking_target(&decoded.event)
                && !tracked_windows
                    .iter()
                    .any(|existing| existing.target() == &target)
                && let Some(token) = windows.admit_matching_token(&target, observed_at_unix_ms)
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
pub fn discovery_seed(event: &PumpEvent, network: Network) -> Option<DiscoverySeed> {
    match event {
        PumpEvent::Create(event) => Some(DiscoverySeed {
            target: ObservationTarget::PumpMint(event.mint.clone()),
            mint: event.mint.clone(),
            source_event_time_unix_ms: event.timestamp.saturating_mul(1_000),
        }),
        PumpEvent::PumpSwapCreatePool(event) => {
            let pair = normalize_pump_swap_pair(network, &event.base_mint, &event.quote_mint)?;
            Some(DiscoverySeed {
                target: ObservationTarget::PumpSwapPool(event.pool.clone()),
                mint: pair.token_mint,
                source_event_time_unix_ms: event.timestamp.saturating_mul(1_000),
            })
        }
        PumpEvent::Trade(_)
        | PumpEvent::Complete(_)
        | PumpEvent::CompletePumpAmmMigration(_)
        | PumpEvent::PumpSwapBuy(_)
        | PumpEvent::PumpSwapSell(_)
        | PumpEvent::PumpSwapDeposit(_)
        | PumpEvent::PumpSwapWithdraw(_) => None,
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
        PumpEvent::PumpSwapDeposit(event) => {
            Some(ObservationTarget::PumpSwapPool(event.pool.clone()))
        }
        PumpEvent::PumpSwapWithdraw(event) => {
            Some(ObservationTarget::PumpSwapPool(event.pool.clone()))
        }
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
        if is_log_truncation_marker(log) {
            return Err(LogScopeError::TruncatedLogs);
        }

        if let Some((invoked_program, depth)) = parse_invoke(log) {
            if depth == 0 || depth != stack.len().saturating_add(1) {
                return Err(LogScopeError::InvalidInvocationDepth { depth });
            }

            if depth == 1 {
                let instruction_index = next_instruction_index;
                next_instruction_index = next_instruction_index
                    .checked_add(1)
                    .ok_or(LogScopeError::TooManyInstructions(u16::MAX))?;
                stack.push((invoked_program.to_owned(), instruction_index));
            } else {
                let instruction_index = stack
                    .last()
                    .map(|(_, instruction_index)| *instruction_index)
                    .ok_or(LogScopeError::InvalidInvocationDepth { depth })?;
                stack.push((invoked_program.to_owned(), instruction_index));
            }
            continue;
        }

        if let Some((exited_program, _)) = parse_exit(log) {
            if stack
                .last()
                .is_none_or(|(active_program, _)| active_program != exited_program)
            {
                return Err(LogScopeError::UnexpectedProgramExit {
                    program_id: exited_program.to_owned(),
                });
            }
            stack.pop();
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

    if !stack.is_empty() {
        return Err(LogScopeError::UnclosedInvocations { depth: stack.len() });
    }

    Ok(scoped)
}

/// Corroborates direct events with silent self-CPIs per top-level instruction.
///
/// The pinned Pump programs currently emit a direct `Program data:` record and
/// then immediately invoke the same program silently to carry the Anchor event
/// CPI bytes. `logsSubscribe` exposes the invocation shape but not those bytes.
/// The direct record therefore remains canonical; the silent successful call
/// is only structural corroboration. A named, nested or payload-bearing
/// self-call is not an event candidate. Missing, failed, out-of-order or
/// unpaired candidates fail closed.
fn successful_self_cpi_coverage(
    program_id: &str,
    logs: &[String],
) -> Result<BTreeMap<u16, SelfCpiCoverage>, LogScopeError> {
    let mut stack: Vec<InvocationFrame> = Vec::new();
    let mut next_instruction_index = 0_u16;
    let mut coverage = BTreeMap::<u16, SelfCpiCoverage>::new();

    for log in logs {
        if is_log_truncation_marker(log) {
            return Err(LogScopeError::TruncatedLogs);
        }

        if let Some((invoked_program, depth)) = parse_invoke(log) {
            if depth == 0 || depth != stack.len().saturating_add(1) {
                return Err(LogScopeError::InvalidInvocationDepth { depth });
            }

            let instruction_index = if depth == 1 {
                let instruction_index = next_instruction_index;
                next_instruction_index = next_instruction_index
                    .checked_add(1)
                    .ok_or(LogScopeError::TooManyInstructions(u16::MAX))?;
                instruction_index
            } else {
                stack
                    .last()
                    .map(|frame| frame.instruction_index)
                    .ok_or(LogScopeError::InvalidInvocationDepth { depth })?
            };
            let is_target_self_cpi = invoked_program == program_id
                && stack.iter().any(|frame| frame.program_id == program_id);

            if let Some(parent) = stack.last_mut()
                && parent.is_target_self_cpi
            {
                parent.silent_candidate = false;
            }

            let candidate_for_pending_event = stack.last().is_some_and(|parent| {
                parent.program_id == program_id
                    && !parent.is_target_self_cpi
                    && is_target_self_cpi
                    && coverage
                        .get(&instruction_index)
                        .is_some_and(|entry| entry.pending_direct_event)
            });
            if stack
                .last()
                .is_some_and(|parent| parent.program_id == program_id && !parent.is_target_self_cpi)
            {
                let instruction = coverage.entry(instruction_index).or_default();
                if instruction.pending_direct_event && !candidate_for_pending_event {
                    instruction.pairing_gap = true;
                    instruction.pending_direct_event = false;
                }
            }

            stack.push(InvocationFrame {
                program_id: invoked_program.to_owned(),
                instruction_index,
                is_target_self_cpi,
                silent_candidate: is_target_self_cpi,
                candidate_for_pending_event,
            });
            continue;
        }

        if let Some((exited_program, succeeded)) = parse_exit(log) {
            if stack
                .last()
                .is_none_or(|frame| frame.program_id != exited_program)
            {
                return Err(LogScopeError::UnexpectedProgramExit {
                    program_id: exited_program.to_owned(),
                });
            }
            let frame = stack.pop().expect("the matching frame was just checked");
            let instruction = coverage.entry(frame.instruction_index).or_default();
            if frame.is_target_self_cpi {
                if frame.candidate_for_pending_event {
                    if succeeded && frame.silent_candidate && instruction.pending_direct_event {
                        instruction.successful_event_calls = instruction
                            .successful_event_calls
                            .checked_add(1)
                            .ok_or(LogScopeError::TooManyEvents(u16::MAX))?;
                    } else {
                        instruction.pairing_gap = true;
                    }
                    instruction.pending_direct_event = false;
                } else if succeeded && frame.silent_candidate {
                    instruction.pairing_gap = true;
                }
            } else if frame.program_id == program_id && instruction.pending_direct_event {
                instruction.pairing_gap = true;
                instruction.pending_direct_event = false;
            }
            continue;
        }

        if log.starts_with("Program data: ") {
            if let Some(frame) = stack.last_mut() {
                if frame.is_target_self_cpi {
                    frame.silent_candidate = false;
                } else if frame.program_id == program_id {
                    let instruction = coverage.entry(frame.instruction_index).or_default();
                    if instruction.pending_direct_event {
                        instruction.pairing_gap = true;
                    }
                    instruction.pending_direct_event = true;
                }
            }
            continue;
        }

        if let Some(frame) = stack.last_mut() {
            if frame.is_target_self_cpi {
                if !is_compute_consumed_log(&frame.program_id, log) {
                    frame.silent_candidate = false;
                }
            } else if frame.program_id == program_id {
                let instruction = coverage.entry(frame.instruction_index).or_default();
                if instruction.pending_direct_event
                    && !is_compute_consumed_log(&frame.program_id, log)
                {
                    instruction.pairing_gap = true;
                    instruction.pending_direct_event = false;
                }
            }
        }
    }

    if !stack.is_empty() {
        return Err(LogScopeError::UnclosedInvocations { depth: stack.len() });
    }

    Ok(coverage)
}

fn event_cpi_coverage_is_incomplete(
    self_cpi: &BTreeMap<u16, SelfCpiCoverage>,
    direct: &BTreeMap<u16, DirectEventCoverage>,
) -> bool {
    self_cpi.iter().any(|(instruction_index, cpi)| {
        let direct = direct.get(instruction_index).copied().unwrap_or_default();
        cpi.pairing_gap
            || cpi.pending_direct_event
            || direct.invalid_event
            || cpi.successful_event_calls != direct.eligible_events
    }) || direct.iter().any(|(instruction_index, direct)| {
        !self_cpi.contains_key(instruction_index)
            && (direct.invalid_event || direct.eligible_events > 0)
    })
}

fn is_compute_consumed_log(program_id: &str, log: &str) -> bool {
    log.strip_prefix("Program ")
        .and_then(|suffix| suffix.strip_prefix(program_id))
        .is_some_and(|suffix| {
            suffix.starts_with(" consumed ") && suffix.ends_with(" compute units")
        })
}

fn record_source_coverage_gap(
    program: PumpProgram,
    windows: &ObservationWindowRegistry,
    observed_at_unix_ms: i64,
    signature: &str,
    reason: WindowIncompleteReason,
) {
    let affected =
        windows.record_source_coverage_gap(program.source_program(), observed_at_unix_ms, reason);
    if affected > 0 {
        tracing::debug!(
            source = ?program.source_program(),
            signature,
            affected_windows = affected,
            reason = reason.as_str(),
            "PubSub source evidence was incomplete; affected windows cannot be complete"
        );
    }
}

fn notification_mentions_program(program_id: &str, logs: &[String]) -> bool {
    logs.iter().any(|log| {
        parse_invoke(log).is_some_and(|(invoked_program, _)| invoked_program == program_id)
    })
}

fn is_log_truncation_marker(log: &str) -> bool {
    log == "Log truncated" || log.starts_with("Log truncated: ")
}

fn parse_invoke(log: &str) -> Option<(&str, usize)> {
    let suffix = log.strip_prefix("Program ")?;
    let (program_id, depth) = suffix.split_once(" invoke [")?;
    if !is_plausible_program_id(program_id) {
        return None;
    }
    let depth = depth.strip_suffix(']')?.parse().ok()?;
    Some((program_id, depth))
}

fn parse_exit(log: &str) -> Option<(&str, bool)> {
    let suffix = log.strip_prefix("Program ")?;
    let exit = suffix
        .strip_suffix(" success")
        .map(|program_id| (program_id, true))
        .or_else(|| {
            suffix
                .split_once(" failed:")
                .map(|(program_id, _)| (program_id, false))
        })?;
    is_plausible_program_id(exit.0).then_some(exit)
}

fn is_plausible_program_id(program_id: &str) -> bool {
    !program_id.is_empty() && !program_id.bytes().any(|byte| byte.is_ascii_whitespace())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use soldisco_discovery_engine::{
        ObservationWindowRegistry, WRAPPED_SOL_MINT, WindowIncompleteReason,
    };
    use soldisco_domain::{MarketIdentity, Network, Venue};
    use soldisco_solana_rpc::ProgramLogNotification;
    use soldisco_source_pump::{
        COMPLETE_EVENT_DISCRIMINATOR, CREATE_EVENT_DISCRIMINATOR, CREATE_POOL_EVENT_DISCRIMINATOR,
        PUMP_PROGRAM_ID, PUMP_SWAP_BUY_EVENT_DISCRIMINATOR, PUMP_SWAP_DEPOSIT_EVENT_DISCRIMINATOR,
        PUMP_SWAP_PROGRAM_ID, PumpEvent, PumpSwapCreatePoolEvent, decode_anchor_event,
    };

    use super::{
        IntakeDropReason, LogScopeError, NotificationDisposition, classify_notification,
        discovery_seed, event_tracking_target, scope_program_data_logs,
        successful_self_cpi_coverage,
    };

    const OTHER: &str = "other";
    const WRAPPED_SOL_BYTES: [u8; 32] = [
        6, 155, 136, 87, 254, 171, 129, 132, 251, 104, 127, 99, 70, 24, 192, 53, 218, 196, 57, 220,
        26, 235, 59, 85, 152, 160, 240, 0, 0, 0, 0, 1,
    ];

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
        bytes.extend(WRAPPED_SOL_BYTES);
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

    fn pump_swap_deposit_event(timestamp: i64) -> Vec<u8> {
        let mut bytes = PUMP_SWAP_DEPOSIT_EVENT_DISCRIMINATOR.to_vec();
        bytes.extend(timestamp.to_le_bytes());
        for value in 1_u64..=10 {
            bytes.extend((value * 100).to_le_bytes());
        }
        for marker in 1_u8..=5 {
            push_pubkey(&mut bytes, marker);
        }
        bytes
    }

    fn pump_swap_pool(source_base_mint: &str, source_quote_mint: &str) -> PumpEvent {
        PumpEvent::PumpSwapCreatePool(PumpSwapCreatePoolEvent {
            timestamp: 100,
            index: 1,
            creator: "creator".to_owned(),
            base_mint: source_base_mint.to_owned(),
            quote_mint: source_quote_mint.to_owned(),
            base_mint_decimals: 9,
            quote_mint_decimals: 6,
            base_amount_in: 10,
            quote_amount_in: 20,
            pool_base_amount: 10,
            pool_quote_amount: 20,
            minimum_liquidity: 1,
            initial_liquidity: 2,
            lp_token_amount_out: 3,
            pool_bump: 254,
            pool: "pool".to_owned(),
            lp_mint: "lp-mint".to_owned(),
            user_base_token_account: "base-account".to_owned(),
            user_quote_token_account: "quote-account".to_owned(),
            coin_creator: "coin-creator".to_owned(),
            is_mayhem_mode: false,
        })
    }

    fn notification(event: Vec<u8>) -> ProgramLogNotification {
        notification_events(vec![event])
    }

    fn notification_events(events: Vec<Vec<u8>>) -> ProgramLogNotification {
        notification_for_program(PUMP_PROGRAM_ID, events)
    }

    fn notification_for_program(program_id: &str, events: Vec<Vec<u8>>) -> ProgramLogNotification {
        let mut log_messages = vec![
            format!("Program {program_id} invoke [1]"),
            "Program log: Instruction: Fixture".to_owned(),
        ];
        for event in events {
            log_messages.extend([
                format!("Program data: {}", STANDARD.encode(event)),
                format!("Program {program_id} invoke [2]"),
                format!("Program {program_id} consumed 1 of 2 compute units"),
                format!("Program {program_id} success"),
            ]);
        }
        log_messages.push(format!("Program {program_id} success"));
        ProgramLogNotification {
            subscription_id: 1,
            slot: 42,
            signature: "signature".to_owned(),
            log_messages,
            transaction_error: None,
        }
    }

    fn cpi_only_notification(program_id: &str) -> ProgramLogNotification {
        notification_with_direct_logs_and_cpis(program_id, Vec::new(), 1)
    }

    fn notification_with_event_cpis(
        program_id: &str,
        events: Vec<Vec<u8>>,
        cpi_calls: usize,
    ) -> ProgramLogNotification {
        notification_with_direct_logs_and_cpis(
            program_id,
            events
                .into_iter()
                .map(|event| format!("Program data: {}", STANDARD.encode(event)))
                .collect(),
            cpi_calls,
        )
    }

    fn notification_with_direct_logs_and_cpis(
        program_id: &str,
        direct_logs: Vec<String>,
        cpi_calls: usize,
    ) -> ProgramLogNotification {
        let mut log_messages = vec![
            format!("Program {program_id} invoke [1]"),
            "Program log: Instruction: Buy".to_owned(),
        ];
        log_messages.extend(direct_logs);
        for _ in 0..cpi_calls {
            log_messages.extend([
                format!("Program {program_id} invoke [2]"),
                format!("Program {program_id} consumed 1 of 2 compute units"),
                format!("Program {program_id} success"),
            ]);
        }
        log_messages.push(format!("Program {program_id} success"));
        ProgramLogNotification {
            subscription_id: 1,
            slot: 42,
            signature: "event-cpi".to_owned(),
            log_messages,
            transaction_error: None,
        }
    }

    fn provision_confirmed_event_window(
        windows: &ObservationWindowRegistry,
        event: &[u8],
        now: i64,
    ) -> soldisco_discovery_engine::ObservationWindowToken {
        let decoded = decode_anchor_event(
            PUMP_PROGRAM_ID,
            soldisco_domain::ChainCoordinate {
                slot: 42,
                transaction_index: None,
                signature: "fixture".to_owned(),
                instruction_index: 0,
                event_index: 0,
            },
            event,
        )
        .expect("event fixture should decode");
        let target = event_tracking_target(&decoded.event).expect("activity event target");
        let mint = match &target {
            soldisco_discovery_engine::ObservationTarget::PumpMint(mint) => mint.clone(),
            soldisco_discovery_engine::ObservationTarget::PumpSwapPool(pool) => pool.clone(),
        };
        let provision = windows.provision_target(
            target,
            mint,
            now.saturating_sub(1_000),
            Duration::from_secs(5),
        );
        assert!(provision.token.confirm());
        provision.token
    }

    #[test]
    fn fresh_discovery_is_fetched_and_stale_discovery_is_dropped() {
        let windows = ObservationWindowRegistry::new(8);
        let now = 1_720_000_010_000_i64;

        assert!(matches!(
            classify_notification(
                &notification(create_event(1_720_000_009)),
                &windows,
                Network::SolanaMainnet,
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
                Network::SolanaMainnet,
                Duration::from_secs(5),
                now,
                now,
            ),
            NotificationDisposition::Drop(IntakeDropReason::StaleDiscovery)
        );
    }

    #[test]
    fn pump_swap_discovery_seed_uses_the_non_quote_mint_in_either_source_orientation() {
        let standard = discovery_seed(
            &pump_swap_pool("token", WRAPPED_SOL_MINT),
            Network::SolanaMainnet,
        )
        .expect("standard source orientation should be discoverable");
        let reversed = discovery_seed(
            &pump_swap_pool(WRAPPED_SOL_MINT, "token"),
            Network::SolanaMainnet,
        )
        .expect("reversed source orientation should be discoverable");

        assert_eq!(standard.mint, "token");
        assert_eq!(reversed.mint, "token");
        assert_eq!(standard.target, reversed.target);
        assert!(
            discovery_seed(
                &pump_swap_pool("token-a", "token-b"),
                Network::SolanaMainnet,
            )
            .is_none(),
            "unsupported token/token pools must not consume a discovery window"
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
            Network::SolanaMainnet,
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
                Network::SolanaMainnet,
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
                Network::SolanaMainnet,
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
            Network::SolanaMainnet,
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
            Network::SolanaMainnet,
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
            Network::SolanaMainnet,
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
    fn pump_swap_liquidity_activity_routes_to_the_matching_pool_window() {
        let now = 1_720_000_010_000_i64;
        let discovery = classify_notification(
            &notification_for_program(
                PUMP_SWAP_PROGRAM_ID,
                vec![pump_swap_create_pool_event(1_720_000_009)],
            ),
            &ObservationWindowRegistry::new(8),
            Network::SolanaMainnet,
            Duration::from_secs(5),
            now,
            now,
        );
        let NotificationDisposition::FetchDiscovery { mut seeds, .. } = discovery else {
            panic!("fresh PumpSwap pool should trigger discovery");
        };
        let seed = seeds.pop().expect("one pool seed");
        let windows = ObservationWindowRegistry::new(8);
        let provision =
            windows.provision_target(seed.target, seed.mint, now, Duration::from_secs(5));
        assert!(provision.token.confirm());

        let activity = classify_notification(
            &notification_for_program(
                PUMP_SWAP_PROGRAM_ID,
                vec![pump_swap_deposit_event(1_720_000_010)],
            ),
            &windows,
            Network::SolanaMainnet,
            Duration::from_secs(5),
            now.saturating_add(1),
            now.saturating_add(1),
        );

        let NotificationDisposition::CollectTrackedActivity(tokens) = activity else {
            panic!("matching PumpSwap deposit should be tracked");
        };
        assert_eq!(tokens, vec![provision.token]);
    }

    #[test]
    fn cpi_only_activity_marks_source_windows_incomplete_without_http_classification() {
        let now = 1_720_000_010_000_i64;
        let windows = ObservationWindowRegistry::new(2);
        let pump = windows.provision_target(
            soldisco_discovery_engine::ObservationTarget::PumpMint("mint-a".to_owned()),
            "mint-a".to_owned(),
            now.saturating_sub(1_000),
            Duration::from_secs(5),
        );
        let swap = windows.provision_target(
            soldisco_discovery_engine::ObservationTarget::PumpSwapPool("pool-b".to_owned()),
            "mint-b".to_owned(),
            now.saturating_sub(1_000),
            Duration::from_secs(5),
        );
        assert!(pump.token.confirm());
        assert!(swap.token.confirm());

        assert_eq!(
            classify_notification(
                &cpi_only_notification(PUMP_PROGRAM_ID),
                &windows,
                Network::SolanaMainnet,
                Duration::from_secs(5),
                now,
                now,
            ),
            NotificationDisposition::Drop(IntakeDropReason::Irrelevant)
        );
        assert_eq!(
            pump.token.incomplete_reason(),
            Some(WindowIncompleteReason::CpiInstructionDataUnavailable)
        );
        assert_eq!(swap.token.incomplete_reason(), None);
    }

    #[test]
    fn paired_event_cpi_uses_the_decoded_direct_copy_without_a_coverage_gap() {
        let now = 1_720_000_010_000_i64;
        let evidence = complete_event(1_720_000_010);
        let windows = ObservationWindowRegistry::new(1);
        let token = provision_confirmed_event_window(&windows, &evidence, now);

        let disposition = classify_notification(
            &notification_with_event_cpis(PUMP_PROGRAM_ID, vec![evidence], 1),
            &windows,
            Network::SolanaMainnet,
            Duration::from_secs(5),
            now,
            now,
        );

        let NotificationDisposition::CollectTrackedActivity(tokens) = disposition else {
            panic!("the direct event copy should remain usable activity");
        };
        assert_eq!(tokens, vec![token.clone()]);
        assert_eq!(token.incomplete_reason(), None);
    }

    #[test]
    fn event_cpi_cardinality_mismatch_marks_source_windows_incomplete() {
        let now = 1_720_000_010_000_i64;
        let evidence = complete_event(1_720_000_010);
        let windows = ObservationWindowRegistry::new(1);
        let token = provision_confirmed_event_window(&windows, &evidence, now);

        let disposition = classify_notification(
            &notification_with_event_cpis(PUMP_PROGRAM_ID, vec![evidence], 2),
            &windows,
            Network::SolanaMainnet,
            Duration::from_secs(5),
            now,
            now,
        );

        assert!(matches!(
            disposition,
            NotificationDisposition::CollectTrackedActivity(_)
        ));
        assert_eq!(
            token.incomplete_reason(),
            Some(WindowIncompleteReason::CpiInstructionDataUnavailable)
        );
    }

    #[test]
    fn malformed_known_event_marks_decode_gap_and_cannot_pair_with_event_cpi() {
        let now = 1_720_000_010_000_i64;
        let valid = complete_event(1_720_000_010);
        let mut malformed = valid.clone();
        malformed.pop();
        let windows = ObservationWindowRegistry::new(1);
        let token = provision_confirmed_event_window(&windows, &valid, now);

        assert_eq!(
            classify_notification(
                &notification_with_event_cpis(PUMP_PROGRAM_ID, vec![malformed], 1),
                &windows,
                Network::SolanaMainnet,
                Duration::from_secs(5),
                now,
                now,
            ),
            NotificationDisposition::Drop(IntakeDropReason::MalformedLogs)
        );
        assert_eq!(
            token.incomplete_reason(),
            Some(WindowIncompleteReason::DecodeGap)
        );
    }

    #[test]
    fn invalid_base64_and_short_direct_data_cannot_pair_with_event_cpi() {
        let now = 1_720_000_010_000_i64;
        let invalid_direct_logs = [
            "Program data: !!!".to_owned(),
            format!("Program data: {}", STANDARD.encode([1_u8, 2, 3])),
        ];

        for direct_log in invalid_direct_logs {
            let windows = ObservationWindowRegistry::new(1);
            let token = windows
                .provision_target(
                    soldisco_discovery_engine::ObservationTarget::PumpMint("mint-a".to_owned()),
                    "mint-a".to_owned(),
                    now.saturating_sub(1_000),
                    Duration::from_secs(5),
                )
                .token;
            assert!(token.confirm());

            assert_eq!(
                classify_notification(
                    &notification_with_direct_logs_and_cpis(PUMP_PROGRAM_ID, vec![direct_log], 1,),
                    &windows,
                    Network::SolanaMainnet,
                    Duration::from_secs(5),
                    now,
                    now,
                ),
                NotificationDisposition::Drop(IntakeDropReason::MalformedLogs)
            );
            assert_eq!(
                token.incomplete_reason(),
                Some(WindowIncompleteReason::DecodeGap)
            );
        }
    }

    #[test]
    fn decode_gap_is_retained_for_a_discovery_provisioned_after_classification() {
        let now = 1_720_000_010_000_i64;
        let mut malformed = complete_event(1_720_000_010);
        malformed.pop();
        let windows = ObservationWindowRegistry::new(1);

        let disposition = classify_notification(
            &notification_events(vec![create_event(1_720_000_010), malformed]),
            &windows,
            Network::SolanaMainnet,
            Duration::from_secs(5),
            now,
            now,
        );
        let NotificationDisposition::FetchDiscovery { mut seeds, .. } = disposition else {
            panic!("the valid discovery remains classifiable");
        };
        let seed = seeds.pop().expect("one discovery seed");
        let provision =
            windows.provision_target(seed.target, seed.mint, now, Duration::from_secs(5));

        assert_eq!(
            provision.token.incomplete_reason(),
            Some(WindowIncompleteReason::DecodeGap),
            "same-notification provisioning must inherit the exact source gap"
        );
    }

    #[test]
    fn pinned_but_unconsumed_event_can_pair_and_remain_irrelevant() {
        let now = 1_720_000_010_000_i64;
        let windows = ObservationWindowRegistry::new(1);
        let token = windows
            .provision_target(
                soldisco_discovery_engine::ObservationTarget::PumpMint("mint-a".to_owned()),
                "mint-a".to_owned(),
                now.saturating_sub(1_000),
                Duration::from_secs(5),
            )
            .token;
        assert!(token.confirm());

        assert_eq!(
            classify_notification(
                &notification_with_event_cpis(
                    PUMP_PROGRAM_ID,
                    vec![vec![64, 69, 192, 104, 29, 30, 25, 107]],
                    1,
                ),
                &windows,
                Network::SolanaMainnet,
                Duration::from_secs(5),
                now,
                now,
            ),
            NotificationDisposition::Drop(IntakeDropReason::Irrelevant)
        );
        assert_eq!(token.incomplete_reason(), None);
    }

    #[test]
    fn future_event_discriminator_fails_semantic_completeness() {
        let now = 1_720_000_010_000_i64;
        let windows = ObservationWindowRegistry::new(1);
        let token = windows
            .provision_target(
                soldisco_discovery_engine::ObservationTarget::PumpMint("mint-a".to_owned()),
                "mint-a".to_owned(),
                now.saturating_sub(1_000),
                Duration::from_secs(5),
            )
            .token;
        assert!(token.confirm());

        assert_eq!(
            classify_notification(
                &notification_with_event_cpis(PUMP_PROGRAM_ID, vec![vec![255; 8]], 1),
                &windows,
                Network::SolanaMainnet,
                Duration::from_secs(5),
                now,
                now,
            ),
            NotificationDisposition::Drop(IntakeDropReason::MalformedLogs)
        );
        assert_eq!(
            token.incomplete_reason(),
            Some(WindowIncompleteReason::DecodeGap)
        );
    }

    #[test]
    fn named_or_out_of_order_self_cpi_cannot_corroborate_a_direct_event() {
        let now = 1_720_000_010_000_i64;
        let evidence = complete_event(1_720_000_010);

        let named = ProgramLogNotification {
            subscription_id: 1,
            slot: 42,
            signature: "named-self-cpi".to_owned(),
            log_messages: vec![
                format!("Program {PUMP_PROGRAM_ID} invoke [1]"),
                format!("Program data: {}", STANDARD.encode(&evidence)),
                format!("Program {PUMP_PROGRAM_ID} invoke [2]"),
                "Program log: Instruction: Named".to_owned(),
                format!("Program {PUMP_PROGRAM_ID} success"),
                format!("Program {PUMP_PROGRAM_ID} success"),
            ],
            transaction_error: None,
        };
        let out_of_order = ProgramLogNotification {
            subscription_id: 1,
            slot: 42,
            signature: "out-of-order-self-cpi".to_owned(),
            log_messages: vec![
                format!("Program {PUMP_PROGRAM_ID} invoke [1]"),
                format!("Program data: {}", STANDARD.encode(&evidence)),
                "Program log: intervening work".to_owned(),
                format!("Program {PUMP_PROGRAM_ID} invoke [2]"),
                format!("Program {PUMP_PROGRAM_ID} success"),
                format!("Program {PUMP_PROGRAM_ID} success"),
            ],
            transaction_error: None,
        };

        for notification in [named, out_of_order] {
            let windows = ObservationWindowRegistry::new(1);
            let token = provision_confirmed_event_window(&windows, &evidence, now);
            assert!(matches!(
                classify_notification(
                    &notification,
                    &windows,
                    Network::SolanaMainnet,
                    Duration::from_secs(5),
                    now,
                    now,
                ),
                NotificationDisposition::CollectTrackedActivity(_)
            ));
            assert_eq!(
                token.incomplete_reason(),
                Some(WindowIncompleteReason::CpiInstructionDataUnavailable)
            );
        }
    }

    #[test]
    fn nested_program_call_is_only_a_gap_when_it_is_a_self_cpi() {
        let now = 1_720_000_010_000_i64;
        let windows = ObservationWindowRegistry::new(1);
        let token = windows
            .provision_target(
                soldisco_discovery_engine::ObservationTarget::PumpMint("mint-a".to_owned()),
                "mint-a".to_owned(),
                now.saturating_sub(1_000),
                Duration::from_secs(5),
            )
            .token;
        assert!(token.confirm());

        let aggregator_notification = ProgramLogNotification {
            subscription_id: 1,
            slot: 42,
            signature: "aggregator".to_owned(),
            log_messages: vec![
                format!("Program {OTHER} invoke [1]"),
                format!("Program {PUMP_PROGRAM_ID} invoke [2]"),
                format!("Program {PUMP_PROGRAM_ID} success"),
                format!("Program {OTHER} success"),
            ],
            transaction_error: None,
        };
        assert!(
            matches!(
                classify_notification(
                    &aggregator_notification,
                    &windows,
                    Network::SolanaMainnet,
                    Duration::from_secs(5),
                    now,
                    now,
                ),
                NotificationDisposition::Drop(IntakeDropReason::Irrelevant)
            ),
            "a single invocation beneath an aggregator is not a self-CPI"
        );
        assert_eq!(token.incomplete_reason(), None);
    }

    #[test]
    fn explicit_log_truncation_after_direct_evidence_fails_closed() {
        let now = 1_720_000_010_000_i64;
        let evidence = complete_event(1_720_000_010);
        let windows = ObservationWindowRegistry::new(1);
        let token = provision_confirmed_event_window(&windows, &evidence, now);
        let mut notification = notification_with_event_cpis(PUMP_PROGRAM_ID, vec![evidence], 1);
        notification.log_messages.insert(
            notification.log_messages.len() - 1,
            "Log truncated".to_owned(),
        );

        assert_eq!(
            classify_notification(
                &notification,
                &windows,
                Network::SolanaMainnet,
                Duration::from_secs(5),
                now,
                now,
            ),
            NotificationDisposition::Drop(IntakeDropReason::MalformedLogs)
        );
        assert_eq!(
            token.incomplete_reason(),
            Some(WindowIncompleteReason::DecodeGap)
        );
    }

    #[test]
    fn both_log_walkers_reject_mismatched_exits_and_unclosed_stacks() {
        let mismatched_exit = vec![
            format!("Program {PUMP_PROGRAM_ID} invoke [1]"),
            format!("Program {OTHER} success"),
        ];
        let unclosed = vec![format!("Program {PUMP_PROGRAM_ID} invoke [1]")];

        for logs in [&mismatched_exit, &unclosed] {
            assert!(scope_program_data_logs(PUMP_PROGRAM_ID, 42, None, "signature", logs).is_err());
            assert!(successful_self_cpi_coverage(PUMP_PROGRAM_ID, logs).is_err());
        }
        assert_eq!(
            scope_program_data_logs(PUMP_PROGRAM_ID, 42, None, "signature", &mismatched_exit,),
            Err(LogScopeError::UnexpectedProgramExit {
                program_id: OTHER.to_owned(),
            })
        );
        assert_eq!(
            successful_self_cpi_coverage(PUMP_PROGRAM_ID, &unclosed),
            Err(LogScopeError::UnclosedInvocations { depth: 1 })
        );
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
