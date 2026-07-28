# Product flow

## End-to-end

1. **Start locally** — the React interface sends a locally guarded command to
   the Rust server. The server verifies the configured HTTP RPC's pinned Solana
   genesis identity before binding the database or opening intake; the browser
   never owns collector state.
2. **Pump intake** — WebSocket PubSub observes relevant Pump and PumpSwap
   notifications. Every success or failure notification requires a matching
   authoritative HTTP transaction signature, exact slot, and status. Bounded
   ordered-concurrent HTTP RPC includes CPI instruction data and finite fetch
   attempts; a watchdog reconnects silent subscriptions and checkpoint
   recovery covers bounded missed signatures.
3. **Durable handoff** — each normalized observation is committed to local
   PostgreSQL with source, decoder, Solana coordinates, exact market identity,
   event time, receipt time, and compact evidence before downstream work is
   notified. Attributable malformed event or log evidence, plus historical
   facts whose exact market/quote cannot be resolved, enter quarantine before
   checkpoint advance; unknown discriminators are ignored. Quarantined
   unresolved facts are not automatically reprocessed yet. Unrecoverable
   bounded history becomes an explicit gap and keeps stream health degraded.
4. **Structural discovery (implemented)** — the current `OBSERVE_ALL`
   projection admits every structurally valid decoded candidate as `OBSERVED`
   and maintains venue-scoped cumulative trade, buy/sell, atomic-volume, and
   unique-trader activity. It applies no qualification threshold and produces
   no approval, rejection, risk score, or opportunity score.
5. **Operational safety (implemented)** — transient pipeline faults restart
   with capped backoff, while network-identity mismatch and storage-limit
   failures enter terminal `ERROR`; terminal history is pruned in bounded
   batches, and discovery snapshots state when they are truncated.
6. **Rolling qualification (planned)** — bounded rolling trades, volume,
   buy/sell balance, unique wallets, price movement, curve state, and migration
   state support an inexpensive initial qualification.
7. **Deterministic first pass (planned)** — provider-neutral RPC checks identify
   invalid, incomplete, scam-like, and rug-like candidates and return
   explainable `PASS`, `REJECT`, or `UNKNOWN` results.
8. **Raydium venue enrichment (planned)** — when an eligible Pump candidate has a
   supported Raydium market, exact CPMM, CLMM, or AMM v4 pools are evaluated as
   separate venue evidence. Metrics are not blindly merged across pools.
9. **Candidate monitoring (planned)** — approved candidates enter expiring windows that
   continue tracking trades, volume, liquidity, price, holders, and wallet
   activity.
10. **Feature production (planned)** — immutable market, wallet-score, and wallet-cluster
   snapshots capture exactly what was knowable at an evaluation time.
11. **Advisory AI (planned)** — optional asynchronous AI summarizes bounded evidence and
   uncertainty without approving tokens, blocking ingestion, or authorizing
   trades.
12. **Strategy evaluation (planned)** — each enabled strategy repeatedly evaluates fresh
    snapshots and emits an identified, versioned result with reason codes.
13. **Trading (planned)** — matches may later create paper proposals. Interactive Live
    trading later adds deterministic policy, quotes, simulation, explicit user
    review, and browser-wallet signing.
14. **Projection and replay** — implemented discovery facts update a bounded
    authoritative feed over HTTP; total/truncation metadata stays explicit and
    coalesced named `soldisco` SSE notifications trigger refreshes. Future facts
    update counters, orders, positions, PnL, alerts, and time-correct replays.

Future replay tooling must state when its requested raw history has aged out
under the configured retention policy. It cannot treat an incomplete retained
window as complete history.

When Raydium enrichment exists, absence of a venue will be an explicit
unavailable result, not an automatic failure unless a versioned rule requires
it. Initial collection is driven by Pump and PumpSwap; scanning arbitrary
Raydium-only mints is separate future scope.

## Site destinations

The modular UI retains these destinations, but only local stream control,
Discovery, and current token facts are connected in this milestone:

- **Discovery** — currently all structurally valid `OBSERVED` candidates; after
  screening exists, this becomes the approved-only feed with truthful counters
  and a compact rejection-reason log
- **Inspector** — overview, risk evidence, signals, trade, and position details
- **Strategies** — upload, validate, replay, activate, and toggle versions
- **Orders** — one destination with mode-specific Paper records or Live
  transaction lifecycle
- **Positions** — one destination with separate Paper ledger projections or
  Live reconciled holdings, exposure, PnL, and exit controls
- **Alerts** — risk changes, matches, stale data, outages, and execution events
- **Replays** — historical evaluation with original information boundaries
- **Controls** — current local stream/database state, later per-source RPC,
  recovery, screening, and Raydium health, operating mode, and future kill
  switches

The UI receives read-only projections. It does not define domain truth, perform
scoring, or infer fills. Paper mode never presents wallet connection as a
dependency; wallet controls belong only to Live mode.

The selected topology is local: `localhost:3000` sends HTTP commands and
receives authoritative snapshots plus named SSE notifications from
`127.0.0.1:8080`. The Rust server alone talks to PostgreSQL and Solana. The
hosted Sites preview remains disconnected because it has no remotely deployed
Rust backend.
