# Product flow

## End-to-end

1. **Start locally** — the React interface sends a command to the Rust server;
   the server, not the browser, owns collector state.
2. **Pump intake** — WebSocket PubSub observes relevant Pump and PumpSwap
   activity while HTTP RPC retrieves transaction/account evidence and recovers
   missed slots or signatures.
3. **Durable handoff** — each normalized observation is committed to local
   PostgreSQL with source, decoder, Solana coordinates, exact market identity,
   event time, and receipt time before a Tokio worker message is published.
4. **Discovery metrics** — rolling trades, volume, buy/sell balance, unique
   wallets, price movement, curve state, and migration state support an
   inexpensive initial qualification.
5. **Deterministic first pass** — provider-neutral RPC checks identify invalid,
   incomplete, scam-like, and rug-like candidates and return explainable
   `PASS`, `REJECT`, or `UNKNOWN` results.
6. **Raydium venue enrichment** — when an eligible Pump candidate has a
   supported Raydium market, exact CPMM, CLMM, or AMM v4 pools are evaluated as
   separate venue evidence. Metrics are not blindly merged across pools.
7. **Candidate monitoring** — approved candidates enter expiring windows that
   continue tracking trades, volume, liquidity, price, holders, and wallet
   activity.
8. **Feature production** — immutable market, wallet-score, and wallet-cluster
   snapshots capture exactly what was knowable at an evaluation time.
9. **Advisory AI** — optional asynchronous AI summarizes bounded evidence and
   uncertainty without approving tokens, blocking ingestion, or authorizing
   trades.
10. **Strategy evaluation** — each enabled strategy repeatedly evaluates fresh
    snapshots and emits an identified, versioned result with reason codes.
11. **Trading** — matches may later create paper proposals. Interactive Live
    trading later adds deterministic policy, quotes, simulation, explicit user
    review, and browser-wallet signing.
12. **Projection and replay** — recorded facts update rebuildable feed,
    counters, orders, positions, PnL, alerts, and time-correct replays.

Absence of a Raydium venue is an explicit unavailable result, not an automatic
failure unless a versioned rule requires it. Initial collection is driven by
Pump and PumpSwap; scanning arbitrary Raydium-only mints is separate future
scope.

## Site destinations

- **Discovery** — approved candidates only, with screening counters and a
  compact rejection-reason log
- **Inspector** — overview, risk evidence, signals, trade, and position details
- **Strategies** — upload, validate, replay, activate, and toggle versions
- **Orders** — one destination with mode-specific Paper records or Live
  transaction lifecycle
- **Positions** — one destination with separate Paper ledger projections or
  Live reconciled holdings, exposure, PnL, and exit controls
- **Alerts** — risk changes, matches, stale data, outages, and execution events
- **Replays** — historical evaluation with original information boundaries
- **Controls** — local stream state; Pump, RPC, recovery, database, and Raydium
  enrichment health; operating mode; and future kill switches

The UI receives read-only projections. It does not define domain truth, perform
scoring, or infer fills. Paper mode never presents wallet connection as a
dependency; wallet controls belong only to Live mode.

The selected topology is local: `localhost:3000` will send HTTP commands and
receive snapshots/SSE from `127.0.0.1:8080`. The Rust server alone will talk to
PostgreSQL and Solana. The HTTP/SSE server foundation exists, but the current
web app and hosted Sites preview both remain disconnected from it.
