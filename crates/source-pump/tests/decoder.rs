use base64::{Engine as _, engine::general_purpose::STANDARD};
use soldisco_domain::ChainCoordinate;
use soldisco_source_pump::{
    ANCHOR_EVENT_CPI_DISCRIMINATOR, COMPLETE_EVENT_DISCRIMINATOR,
    COMPLETE_PUMP_AMM_MIGRATION_EVENT_DISCRIMINATOR, CREATE_EVENT_DISCRIMINATOR,
    CREATE_POOL_EVENT_DISCRIMINATOR, DecodeError, PUMP_PROGRAM_ID,
    PUMP_SWAP_BUY_EVENT_DISCRIMINATOR, PUMP_SWAP_DEPOSIT_EVENT_DISCRIMINATOR, PUMP_SWAP_PROGRAM_ID,
    PUMP_SWAP_SELL_EVENT_DISCRIMINATOR, PUMP_SWAP_WITHDRAW_EVENT_DISCRIMINATOR, PumpEvent,
    TRADE_EVENT_DISCRIMINATOR, TradeDirection, decode_anchor_event, decode_cpi_event,
    decode_instruction_events, decode_instruction_program_data_logs, decode_program_data_log,
    is_anchor_event_cpi,
};

fn coordinate(event_index: u16) -> ChainCoordinate {
    ChainCoordinate {
        slot: 42,
        transaction_index: None,
        signature: "fixture-signature".to_owned(),
        instruction_index: 3,
        event_index,
    }
}

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

fn pump_create_fixture() -> Vec<u8> {
    let mut bytes = CREATE_EVENT_DISCRIMINATOR.to_vec();
    push_string(&mut bytes, "Fixture Coin");
    push_string(&mut bytes, "FIX");
    push_string(&mut bytes, "https://example.invalid/fixture.json");
    push_pubkey(&mut bytes, 1);
    push_pubkey(&mut bytes, 2);
    push_pubkey(&mut bytes, 3);
    push_pubkey(&mut bytes, 4);
    bytes.extend(1_720_000_000_i64.to_le_bytes());
    bytes.extend(1_000_u64.to_le_bytes());
    bytes.extend(2_000_u64.to_le_bytes());
    bytes.extend(3_000_u64.to_le_bytes());
    bytes.extend(4_000_u64.to_le_bytes());
    push_pubkey(&mut bytes, 5);
    bytes.push(1);
    bytes.push(0);
    push_pubkey(&mut bytes, 6);
    bytes.extend(5_000_u64.to_le_bytes());
    bytes
}

fn pump_trade_fixture() -> Vec<u8> {
    let mut bytes = TRADE_EVENT_DISCRIMINATOR.to_vec();
    push_pubkey(&mut bytes, 1);
    bytes.extend(111_u64.to_le_bytes());
    bytes.extend(222_u64.to_le_bytes());
    bytes.push(1);
    push_pubkey(&mut bytes, 2);
    bytes.extend(1_720_000_001_i64.to_le_bytes());
    for value in 1_u64..=4 {
        bytes.extend((value * 1_000).to_le_bytes());
    }
    push_pubkey(&mut bytes, 3);
    bytes.extend(100_u64.to_le_bytes());
    bytes.extend(11_u64.to_le_bytes());
    push_pubkey(&mut bytes, 4);
    bytes.extend(25_u64.to_le_bytes());
    bytes.extend(3_u64.to_le_bytes());
    bytes.push(1);
    bytes.extend(1_u64.to_le_bytes());
    bytes.extend(2_u64.to_le_bytes());
    bytes.extend(3_u64.to_le_bytes());
    bytes.extend(1_720_000_001_i64.to_le_bytes());
    push_string(&mut bytes, "buy_v2");
    bytes.push(0);
    bytes.extend(4_u64.to_le_bytes());
    bytes.extend(5_u64.to_le_bytes());
    bytes.extend(6_u64.to_le_bytes());
    bytes.extend(7_u64.to_le_bytes());
    bytes.extend(2_u32.to_le_bytes());
    push_pubkey(&mut bytes, 7);
    bytes.extend(6_000_u16.to_le_bytes());
    push_pubkey(&mut bytes, 8);
    bytes.extend(4_000_u16.to_le_bytes());
    push_pubkey(&mut bytes, 9);
    bytes.extend(333_u64.to_le_bytes());
    bytes.extend(4_444_u64.to_le_bytes());
    bytes.extend(5_555_u64.to_le_bytes());
    bytes
}

fn pump_complete_fixture() -> Vec<u8> {
    let mut bytes = COMPLETE_EVENT_DISCRIMINATOR.to_vec();
    push_pubkey(&mut bytes, 1);
    push_pubkey(&mut bytes, 2);
    push_pubkey(&mut bytes, 3);
    bytes.extend(1_720_000_002_i64.to_le_bytes());
    push_pubkey(&mut bytes, 9);
    bytes
}

fn pump_migration_fixture() -> Vec<u8> {
    let mut bytes = COMPLETE_PUMP_AMM_MIGRATION_EVENT_DISCRIMINATOR.to_vec();
    push_pubkey(&mut bytes, 1);
    push_pubkey(&mut bytes, 2);
    bytes.extend(10_u64.to_le_bytes());
    bytes.extend(20_u64.to_le_bytes());
    bytes.extend(30_u64.to_le_bytes());
    push_pubkey(&mut bytes, 3);
    bytes.extend(1_720_000_003_i64.to_le_bytes());
    push_pubkey(&mut bytes, 4);
    push_pubkey(&mut bytes, 9);
    bytes
}

fn pump_swap_create_pool_fixture() -> Vec<u8> {
    let mut bytes = CREATE_POOL_EVENT_DISCRIMINATOR.to_vec();
    bytes.extend(1_720_000_004_i64.to_le_bytes());
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
    for marker in 4_u8..=8 {
        push_pubkey(&mut bytes, marker);
    }
    bytes.push(1);
    bytes
}

fn pump_swap_buy_fixture() -> Vec<u8> {
    let mut bytes = PUMP_SWAP_BUY_EVENT_DISCRIMINATOR.to_vec();
    bytes.extend(1_720_000_005_i64.to_le_bytes());
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
    bytes.extend(1_720_000_005_i64.to_le_bytes());
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

fn pump_swap_sell_fixture() -> Vec<u8> {
    let mut bytes = PUMP_SWAP_SELL_EVENT_DISCRIMINATOR.to_vec();
    bytes.extend(1_720_000_006_i64.to_le_bytes());
    for value in 1_u64..=13 {
        bytes.extend((value * 100).to_le_bytes());
    }
    for marker in 1_u8..=7 {
        push_pubkey(&mut bytes, marker);
    }
    for value in 14_u64..=19 {
        bytes.extend(value.to_le_bytes());
    }
    bytes.extend((-20_i128).to_le_bytes());
    bytes.push(0);
    bytes.extend(21_u64.to_le_bytes());
    bytes
}

fn pump_swap_deposit_fixture() -> Vec<u8> {
    let mut bytes = PUMP_SWAP_DEPOSIT_EVENT_DISCRIMINATOR.to_vec();
    bytes.extend(1_720_000_007_i64.to_le_bytes());
    for value in 1_u64..=10 {
        bytes.extend((value * 100).to_le_bytes());
    }
    for marker in 1_u8..=5 {
        push_pubkey(&mut bytes, marker);
    }
    bytes
}

fn pump_swap_withdraw_fixture() -> Vec<u8> {
    let mut bytes = PUMP_SWAP_WITHDRAW_EVENT_DISCRIMINATOR.to_vec();
    bytes.extend(1_720_000_008_i64.to_le_bytes());
    for value in 11_u64..=20 {
        bytes.extend((value * 100).to_le_bytes());
    }
    for marker in 6_u8..=10 {
        push_pubkey(&mut bytes, marker);
    }
    bytes
}

#[test]
fn decodes_current_pump_create_and_quote_mint_layout() {
    let decoded =
        decode_anchor_event(PUMP_PROGRAM_ID, coordinate(0), &pump_create_fixture()).unwrap();

    let PumpEvent::Create(event) = decoded.event else {
        panic!("expected create event");
    };
    assert_eq!(event.name, "Fixture Coin");
    assert_eq!(event.symbol, "FIX");
    assert_eq!(event.virtual_quote_reserves, 5_000);
    assert_ne!(event.mint, event.quote_mint);
    assert_eq!(decoded.coordinate, coordinate(0));
}

#[test]
fn pump_trade_uses_canonical_quote_fields_without_assuming_sol() {
    let decoded =
        decode_anchor_event(PUMP_PROGRAM_ID, coordinate(0), &pump_trade_fixture()).unwrap();

    let PumpEvent::Trade(event) = decoded.event else {
        panic!("expected trade event");
    };
    assert_eq!(event.direction, TradeDirection::Buy);
    assert_eq!(event.legacy_sol_amount, 111);
    assert_eq!(event.quote_amount, 333);
    assert_eq!(event.instruction_name, "buy_v2");
    assert_eq!(event.shareholders.len(), 2);
    assert_eq!(event.shareholders[0].share_basis_points, 6_000);
    assert_ne!(event.mint, event.quote_mint);
}

#[test]
fn decodes_complete_and_migration_with_quote_identity() {
    let complete =
        decode_anchor_event(PUMP_PROGRAM_ID, coordinate(0), &pump_complete_fixture()).unwrap();
    let migration =
        decode_anchor_event(PUMP_PROGRAM_ID, coordinate(1), &pump_migration_fixture()).unwrap();

    let PumpEvent::Complete(complete) = complete.event else {
        panic!("expected complete event");
    };
    let PumpEvent::CompletePumpAmmMigration(migration) = migration.event else {
        panic!("expected migration event");
    };
    assert_ne!(complete.mint, complete.quote_mint);
    assert_eq!(migration.mint_amount, 10);
    assert_eq!(migration.legacy_sol_amount, 20);
    assert_ne!(migration.pool, migration.quote_mint);
}

#[test]
fn decodes_current_pump_swap_pool_buy_and_sell_layouts() {
    let pool = decode_anchor_event(
        PUMP_SWAP_PROGRAM_ID,
        coordinate(0),
        &pump_swap_create_pool_fixture(),
    )
    .unwrap();
    let buy = decode_anchor_event(
        PUMP_SWAP_PROGRAM_ID,
        coordinate(1),
        &pump_swap_buy_fixture(),
    )
    .unwrap();
    let sell = decode_anchor_event(
        PUMP_SWAP_PROGRAM_ID,
        coordinate(2),
        &pump_swap_sell_fixture(),
    )
    .unwrap();

    let PumpEvent::PumpSwapCreatePool(pool) = pool.event else {
        panic!("expected pool event");
    };
    let PumpEvent::PumpSwapBuy(buy) = buy.event else {
        panic!("expected buy event");
    };
    let PumpEvent::PumpSwapSell(sell) = sell.event else {
        panic!("expected sell event");
    };
    assert_ne!(pool.base_mint, pool.quote_mint);
    assert_eq!(pool.base_mint_decimals, 6);
    assert_eq!(buy.quote_amount_in, 700);
    assert_eq!(buy.instruction_name, "buy_exact_quote_in");
    assert_eq!(buy.virtual_quote_reserves, -24);
    assert_eq!(sell.quote_amount_out, 700);
    assert_eq!(sell.virtual_quote_reserves, -20);
}

#[test]
fn decodes_strict_pinned_pump_swap_liquidity_layouts() {
    let deposit = decode_anchor_event(
        PUMP_SWAP_PROGRAM_ID,
        coordinate(3),
        &pump_swap_deposit_fixture(),
    )
    .expect("pinned DepositEvent layout should decode");
    let withdraw = decode_anchor_event(
        PUMP_SWAP_PROGRAM_ID,
        coordinate(4),
        &pump_swap_withdraw_fixture(),
    )
    .expect("pinned WithdrawEvent layout should decode");

    let PumpEvent::PumpSwapDeposit(deposit) = deposit.event else {
        panic!("expected deposit event");
    };
    let PumpEvent::PumpSwapWithdraw(withdraw) = withdraw.event else {
        panic!("expected withdraw event");
    };

    assert_eq!(deposit.lp_token_amount_out, 100);
    assert_eq!(deposit.pool_base_token_reserves, 600);
    assert_eq!(deposit.pool_quote_token_reserves, 700);
    assert_eq!(deposit.base_amount_in, 800);
    assert_eq!(deposit.quote_amount_in, 900);
    assert_eq!(deposit.lp_mint_supply, 1_000);
    assert_ne!(deposit.pool, deposit.user);

    assert_eq!(withdraw.lp_token_amount_in, 1_100);
    assert_eq!(withdraw.pool_base_token_reserves, 1_600);
    assert_eq!(withdraw.pool_quote_token_reserves, 1_700);
    assert_eq!(withdraw.base_amount_out, 1_800);
    assert_eq!(withdraw.quote_amount_out, 1_900);
    assert_eq!(withdraw.lp_mint_supply, 2_000);
    assert_ne!(withdraw.pool, withdraw.user);
}

#[test]
fn pump_swap_liquidity_decoders_reject_truncated_and_trailing_bytes() {
    let deposit = pump_swap_deposit_fixture();
    let truncated = decode_anchor_event(
        PUMP_SWAP_PROGRAM_ID,
        coordinate(0),
        &deposit[..deposit.len() - 1],
    )
    .expect_err("truncated DepositEvent must fail closed");
    assert!(matches!(truncated, DecodeError::Truncated { .. }));

    let mut withdraw = pump_swap_withdraw_fixture();
    withdraw.push(0);
    let trailing = decode_anchor_event(PUMP_SWAP_PROGRAM_ID, coordinate(1), &withdraw)
        .expect_err("trailing WithdrawEvent bytes must fail closed");
    assert!(matches!(trailing, DecodeError::TrailingBytes(1)));
}

#[test]
fn decodes_cpi_and_multiple_log_events_with_exact_coordinates() {
    let create = pump_create_fixture();
    let trade = pump_trade_fixture();
    let payloads: [&[u8]; 2] = [&create, &trade];
    let binary = decode_instruction_events(PUMP_PROGRAM_ID, 91, "signature", 5, &payloads).unwrap();

    let create_log = format!("Program data: {}", STANDARD.encode(&create));
    let trade_log = format!("Program data: {}", STANDARD.encode(&trade));
    let logs = [
        "Program log: Instruction: Buy",
        create_log.as_str(),
        "Program log: harmless",
        trade_log.as_str(),
    ];
    let logged =
        decode_instruction_program_data_logs(PUMP_PROGRAM_ID, 91, "signature", 5, &logs).unwrap();

    assert_eq!(binary, logged);
    assert_eq!(binary[0].coordinate.event_index, 0);
    assert_eq!(binary[1].coordinate.event_index, 1);

    let mut cpi = ANCHOR_EVENT_CPI_DISCRIMINATOR.to_vec();
    cpi.extend(pump_complete_fixture());
    assert!(is_anchor_event_cpi(&cpi));
    let decoded = decode_cpi_event(PUMP_PROGRAM_ID, coordinate(2), &cpi).unwrap();
    assert!(matches!(decoded.event, PumpEvent::Complete(_)));
    assert!(matches!(
        decode_cpi_event(PUMP_PROGRAM_ID, coordinate(3), &pump_complete_fixture()),
        Err(DecodeError::MissingCpiDiscriminator)
    ));
}

#[test]
fn program_data_helper_rejects_invalid_log_and_base64() {
    let missing_prefix =
        decode_program_data_log(PUMP_PROGRAM_ID, coordinate(0), "Program log: nope").unwrap_err();
    assert!(matches!(missing_prefix, DecodeError::InvalidProgramDataLog));

    let invalid_base64 =
        decode_program_data_log(PUMP_PROGRAM_ID, coordinate(0), "Program data: ***").unwrap_err();
    assert!(matches!(invalid_base64, DecodeError::InvalidBase64(_)));
}

#[test]
fn fails_closed_for_wrong_program_unknown_layout_and_malformed_bytes() {
    let wrong_program =
        decode_anchor_event(PUMP_SWAP_PROGRAM_ID, coordinate(0), &pump_create_fixture())
            .unwrap_err();
    assert!(matches!(
        wrong_program,
        DecodeError::UnknownDiscriminator { .. }
    ));

    let unsupported =
        decode_anchor_event("not-pump", coordinate(0), &pump_create_fixture()).unwrap_err();
    assert!(matches!(unsupported, DecodeError::UnsupportedProgram(_)));

    let truncated =
        decode_anchor_event(PUMP_PROGRAM_ID, coordinate(0), &pump_trade_fixture()[..20])
            .unwrap_err();
    assert!(matches!(truncated, DecodeError::Truncated { .. }));

    let mut trailing = pump_complete_fixture();
    trailing.push(0);
    let trailing = decode_anchor_event(PUMP_PROGRAM_ID, coordinate(0), &trailing).unwrap_err();
    assert!(matches!(trailing, DecodeError::TrailingBytes(1)));
}

#[test]
fn rejects_invalid_boolean_instead_of_coercing_it() {
    let mut fixture = pump_create_fixture();
    let mayhem_offset = fixture.len() - 42;
    fixture[mayhem_offset] = 2;

    let error = decode_anchor_event(PUMP_PROGRAM_ID, coordinate(0), &fixture).unwrap_err();
    assert!(matches!(
        error,
        DecodeError::InvalidBoolean {
            field: "is_mayhem_mode",
            value: 2
        }
    ));
}
