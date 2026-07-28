# Milestones

## 1. UI skeleton — complete

- Implement the stream-first console with honest disconnected states.
- Show only first-pass-approved candidates in the discovery feed.
- Summarize pending and rejected candidates through truthful counters and a
  compact rejection-reason log.
- Display risk, rating, strategy match, momentum, and data freshness.
- Keep token and position trackers empty until authoritative sources connect.
- Add persistent, user-adjustable workspace regions.
- Keep one Discovery destination and one Positions destination; avoid separate
  first-pass, watchlist, or duplicate positions surfaces.
- Add UI-only start and stop controls while keeping discovery disconnected.
- Keep wallet and execution controls explicitly disabled.

## 2. Rust backend foundation — current

- Record permanent authority, safety, and fail-closed invariants.
- Define source provenance, event/receipt time, chain coordinates, market
  identity, deduplication, correction, finality, and raw observation rules.
- Add one Cargo workspace with `apps/server` and focused internal crates.
- Start one Tokio/Axum process with validated configuration, structured logs,
  health, graceful shutdown, and supervised background-job boundaries.
- Add local PostgreSQL through SQLx migrations and a backend-only persistence
  boundary.
- Establish the local topology: React `:3000`, Rust `:8080`, PostgreSQL `:5432`.
- Define browser contracts for finite HTTP commands/snapshots and one-way SSE
  projection updates.
- Keep the hosted Sites UI explicitly disconnected from the local backend.

Completion means the empty backend can compile, start, report truthful
component health, connect to local PostgreSQL when configured, and stop cleanly.
It does not require live token data.

The code foundation is implemented: the Cargo workspace compiles, SQLx owns the
initial migration and atomic observation/work handoff, Axum exposes the route
shell, SSE is notification-only, and Start refuses to claim a collector exists.
Runtime database validation still requires a local PostgreSQL installation;
Docker was not available on the implementation machine during this pass.

## 3. Pump and PumpSwap intake

- Verify supported Pump and PumpSwap program identities and decoder versions.
- Subscribe to relevant Solana program activity through WebSocket PubSub.
- Decode creation, trade, curve-completion, and migration evidence needed by
  discovery.
- Persist observations and durable work state before in-process dispatch.
- Deduplicate live and recovered facts using chain identity.
- Recover gaps with HTTP RPC and saved checkpoints after disconnects or
  downtime.
- Record malformed observations, duplicates, missing data, corrections,
  finality changes, and source/RPC outages.
- Build recorded transaction fixtures for deterministic decoder and recovery
  tests without shipping synthetic product data.

## 4. Rolling discovery metrics

- Maintain bounded rolling windows for trades, volume, buy/sell balance,
  unique wallets, price movement, curve progress, and migration state.
- Preserve exact market and venue identity for every market-dependent value.
- Add inexpensive, versioned qualification rules to reduce deeper RPC work.
- Persist the inputs and result before publishing Pending, Approved, or
  Rejected projections.

## 5. Deterministic risk and Raydium enrichment

- Add provider-neutral Solana RPC access.
- Implement a minimal, explainable set of scam and rug filters.
- Preserve reason codes, rule versions, evidence, and freshness.
- Return `PASS`, `REJECT`, or `UNKNOWN`; missing required evidence fails closed.
- Resolve supported Raydium CPMM, CLMM, and AMM v4 pools only for relevant Pump
  candidates.
- Keep each PumpSwap or Raydium pool's price, liquidity, volume, and activity
  independently identified.
- Treat absent Raydium evidence as unavailable unless a versioned rule requires
  that venue.
- Keep arbitrary Raydium-only discovery out of this milestone.

## 6. Connect the local UI

- Replace UI-only Start/Stop state with Rust HTTP commands.
- Load authoritative stream, discovery, counter, rejection, and token snapshots
  over HTTP.
- Publish rebuildable projection changes over SSE with heartbeat and reconnect
  behavior.
- On reconnect, refresh the snapshot before trusting subsequent live updates.
- Show independent database, Pump collector, RPC, recovery, screening, and
  Raydium-enrichment health.
- Keep the feed approved-only and keep synthetic records confined to tests.

## 7. Candidate monitoring and advisory analysis

- Open, update, expire, and invalidate time-bounded candidate windows.
- Produce versioned market-feature snapshots across explicit windows.
- Project independent discovery, check, risk, feature, AI, and strategy states.
- Run bounded AI analysis asynchronously and advisory-only.

## 8. Wallet intelligence and strategies

- Define a declarative, versioned strategy manifest.
- Add upload validation, replay, activation, and toggles.
- Define time-versioned wallet profiles, scores, typed relationships, and
  cluster snapshots with anti-lookahead rules.
- Implement Wallet-Conditioned Momentum only after those inputs are available.
- Evaluate active candidate windows repeatedly from immutable snapshots.

## 9. Replay, paper trading, and portfolio projections

- Rebuild projections and strategy decisions from as-of information.
- Simulate processing latency, fees, priority fees, slippage, price impact,
  failures, sellability, and exit constraints.
- Produce trade proposals without signing or submission.
- Reconcile simulated fills into positions and PnL.
- Add exposure and loss-limit controls.

## 10. Interactive live execution

- Add non-custodial browser-wallet signing.
- Verify and simulate transactions before presenting them for signature.
- Add execution policies, audit events, confirmation, and reconciliation.
- Require explicit user review for every transaction.

## 11. Automated execution — separately approved future scope

- Design bounded, revocable, auditable signing authority without backend seed
  phrases or raw exportable private keys.
- Add capital, strategy, venue, rate, exposure, and loss limits.
- Add independent kill switches and authorization-expiry behavior.
- Do not enable this milestone by extending an interactive-mode flag.

Cloud deployment is intentionally not a current milestone. If it is requested
later, it must separately address authentication, TLS, secrets, PostgreSQL
backups, RPC capacity, retention, cost ceilings, and access control without
changing the safety invariants.
