# Engine (superseded planning stub)

This directory described the earlier TypeScript backend boundary. It is not a
runnable service and is superseded by the Rust crates:

- `crates/source-pump` decodes Pump and PumpSwap events.
- `crates/source-raydium` decodes supported venue evidence when applicable.
- `crates/solana-rpc` owns provider-neutral Solana access plus reserved
  explicit-recovery primitives; active collection is live-first with no
  backfill.
- `crates/discovery-engine` owns rolling metrics and inexpensive qualification.
- `crates/risk-engine` owns deterministic evidence and scoring.
- `crates/projections` owns browser-facing read models.

Optional Raydium pool and market evidence is a post-Pump venue-enrichment layer.
It can add liquidity, activity, and volatility checks for mints that trade
there, but it does not replace the Pump/PumpSwap collector or determine approval
by itself.

Every decision remains explainable through reason codes, source timestamps,
evidence, and definition versions. Do not add new implementation here.
