# Product flow

## End-to-end

1. **Source intake** — starting the stream consumes live Axiom observations;
   cursor-based reads recover missed history.
2. **Raw preservation** — every candidate is stored with source provenance,
   Solana coordinates, market identity, event time, receipt time, and raw
   evidence.
3. **Deterministic first pass** — provider-neutral RPC checks identify invalid,
   incomplete, scam-like, and rug-like candidates.
4. **Candidate monitoring** — approved candidates enter expiring windows that
   track trades, volume, liquidity, price, holders, and wallet activity.
5. **Feature production** — immutable market, wallet-score, and wallet-cluster
   snapshots capture exactly what was knowable at an evaluation time.
6. **Advisory AI** — asynchronous AI summarizes bounded evidence and uncertainty
   without approving tokens, blocking ingestion, or authorizing trades.
7. **Strategy evaluation** — each enabled strategy repeatedly evaluates fresh
   snapshots and emits an identified, versioned result with reason codes.
8. **Trading** — matches may create paper proposals. Interactive live trading
   later adds deterministic policy, quotes, simulation, explicit user review,
   and browser-wallet signing.
9. **Projection and replay** — confirmed facts update orders, positions, PnL,
   alerts, and time-correct strategy replays.

## Site destinations

- **Token Stream** — every incoming candidate and its current projected status
- **Initial Approval** — first-pass checks and rejection reason codes
- **Watchlist** — candidates retained for manual or strategy monitoring
- **Inspector** — overview, risk evidence, signals, trade, and position details
- **Strategies** — upload, validate, replay, activate, and toggle versions
- **Orders** — paper and live proposal and transaction lifecycle
- **Positions** — reconciled holdings, exposure, PnL, and exit controls
- **Alerts** — risk changes, matches, stale data, outages, and execution events
- **Replays** — historical evaluation with original information boundaries
- **Controls** — stream state, source health, operating mode, and kill switches

The UI receives read-only projections. It does not define domain truth, perform
scoring, or infer fills.
