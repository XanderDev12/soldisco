# Milestones

## 1. UI foundation — complete

- Implement the stream-first console with honest unavailable-source states.
- Keep token and position trackers empty until authoritative sources connect.
- Add persistent, user-adjustable workspace regions.
- Keep one Discovery destination and one Positions destination; avoid separate
  first-pass, watchlist, or duplicate positions surfaces.
- Keep wallet and execution controls unable to connect, sign, or trade.

## 2. Rust backend foundation — complete

- Record permanent authority, safety, and fail-closed invariants.
- Define source provenance, event/receipt time, chain coordinates, market
  identity, deduplication, correction, finality, and raw observation rules.
- Add one Cargo workspace with `apps/server` and focused internal crates.
- Start one Tokio/Axum process with validated configuration, structured logs,
  health, graceful shutdown, and supervised background-job boundaries.
- Add local PostgreSQL through SQLx migrations and a backend-only persistence
  boundary.
- Run the PostgreSQL migration and persistence integration test against a
  disposable PostgreSQL 17 service in GitHub CI.
- Establish the local topology: React `:3000`, Rust `:8080`, PostgreSQL `:5432`.
- Define browser contracts for finite HTTP commands/snapshots and one-way SSE
  projection updates.
- Keep the hosted Sites UI explicitly disconnected from the local backend.

## 3. Connected Pump discovery slice — complete

- Wire the local React UI to Rust health, stream, Discovery, token, and
  start/stop HTTP routes.
- Subscribe independently to verified Pump and PumpSwap program activity
  through WebSocket PubSub.
- Treat every success or failure PubSub message as notification delivery only;
  bounded ordered-concurrent HTTP RPC must corroborate its signature, exact
  slot, and transaction status before state advances.
- Bound authoritative fetch attempts, reconnect silent subscriptions through an
  idle watchdog, and relaunch failed pipeline attempts with capped backoff.
- Strictly decode the supported current-IDL Pump creation, trade, completion,
  and migration events and PumpSwap pool-creation, buy, and sell events.
- Decode supported Anchor CPI events from transaction instruction data rather
  than assuming every event is present in text logs.
- Preserve exact transaction instruction/event coordinates, optional provider
  transaction index, market/quote identity, receipt time, decoder version, and
  compact exact source evidence.
- Persist normalized observations and leased durable work before projection.
- Deduplicate live and recovered facts using chain identity.
- Verify pinned HTTP genesis identity, then immutably bind each database to one
  configured Solana network before ingestion.
- Attempt bounded missed-history recovery from monotonic PostgreSQL checkpoints
  before live release; persist an explicit gap and resume `DEGRADED` when that
  bound cannot reach the checkpoint.
- Quarantine attributable malformed event or log evidence without admitting it
  as a candidate or blocking checkpoint progress.
- Project every structurally valid candidate as `OBSERVED` in `OBSERVE_ALL`
  mode, with cumulative venue-scoped trades, buy/sell counts, atomic volume,
  and unique-trader activity.
- Publish coalesced named `soldisco` SSE notifications; on connection or buffer
  loss, require the browser to refresh bounded authoritative HTTP snapshots
  with explicit total/truncation metadata.
- Supervise start, reconnection, degraded state, stop, cancellation, and
  requested-running restoration.
- Prune eligible terminal history in bounded batches and fail collection closed
  at the configured database-size limit, which defaults to 5 GiB.
- Keep synthetic decoder data in fixtures and tests, never in product state.

This slice deliberately has no threshold, deterministic pass, approval,
rejection, risk score, or opportunity score. Unknown discriminators are ignored
and attributable malformed evidence is quarantined; neither becomes an observed
candidate.

## 4. Rolling discovery metrics and qualification — next

- Maintain bounded rolling windows for trades, volume, buy/sell balance,
  unique wallets, price movement, curve progress, and migration state.
- Preserve exact market and venue identity for every market-dependent value.
- Add inexpensive, versioned qualification rules to reduce deeper RPC work.
- Persist the inputs and result before publishing Pending, Approved, or
  Rejected projections.
- Replace the all-markets in-memory registry with a bounded active cache backed
  by durable on-demand identity lookup.
- Define versioned expiry or archival for inactive discovery, market, activity,
  pool, and rejection-summary projections without orphaning later facts.
- Add operator-visible whole-machine free-space/WAL monitoring for unattended
  local collection; the current database-size guard alone is not a disk
  monitor.
- Extend retention policies to the new rolling windows and immutable feature
  inputs without weakening the existing local storage guard.

## 5. Deterministic risk and Raydium enrichment

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

## 6. Candidate monitoring and advisory analysis

- Open, update, expire, and invalidate time-bounded candidate windows.
- Produce versioned market-feature snapshots across explicit windows.
- Project independent discovery, check, risk, feature, AI, and strategy states.
- Run bounded AI analysis asynchronously and advisory-only.

## 7. Wallet intelligence and strategies

- Define a declarative, versioned strategy manifest.
- Add upload validation, replay, activation, and toggles.
- Define time-versioned wallet profiles, scores, typed relationships, and
  cluster snapshots with anti-lookahead rules.
- Implement Wallet-Conditioned Momentum only after those inputs are available.
- Evaluate active candidate windows repeatedly from immutable snapshots.

## 8. Replay, paper trading, and portfolio projections

- Rebuild projections and strategy decisions from as-of information.
- Simulate processing latency, fees, priority fees, slippage, price impact,
  failures, sellability, and exit constraints.
- Produce trade proposals without signing or submission.
- Reconcile simulated fills into positions and PnL.
- Add exposure and loss-limit controls.

## 9. Interactive live execution

- Add non-custodial browser-wallet signing.
- Verify and simulate transactions before presenting them for signature.
- Add execution policies, audit events, confirmation, and reconciliation.
- Require explicit user review for every transaction.

## 10. Automated execution — separately approved future scope

- Design bounded, revocable, auditable signing authority without backend seed
  phrases or raw exportable private keys.
- Add capital, strategy, venue, rate, exposure, and loss limits.
- Add independent kill switches and authorization-expiry behavior.
- Do not enable this milestone by extending an interactive-mode flag.

Cloud deployment is intentionally not a current milestone. If it is requested
later, it must separately address authentication, TLS, secrets, PostgreSQL
backups, RPC capacity, retention, cost ceilings, and access control without
changing the safety invariants.
