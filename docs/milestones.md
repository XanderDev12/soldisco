# Milestones

## 1. UI foundation — complete

- Implement the stream-first console with honest unavailable-source states.
- Keep token and position trackers empty until authoritative sources connect.
- Add browser-local, persistent, user-adjustable sidebar and inspector regions.
- Remember Paper/Live presentation without treating it as authorization; keep
  order drafts and transient navigation session-only.
- Persist only a validated browser-local API port for the fixed
  `http://127.0.0.1:<port>/api/v1` endpoint; keep endpoint/origin variables out
  of the frontend build.
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
- Establish the default local topology: React `:3000`, Rust `:8080`,
  PostgreSQL `:5432`; a custom browser API port must match the restarted Rust
  `API_PORT`.
- Define browser contracts for finite HTTP commands/snapshots and one-way SSE
  projection updates.
- Keep the hosted Sites UI explicitly disconnected from the local backend.

## 3. Connected Pump discovery slice — complete

- Wire the local React UI to Rust health, stream, Discovery, token, and
  start/stop HTTP routes.
- Subscribe independently to verified Pump and PumpSwap program activity
  through WebSocket PubSub.
- Prefilter successful direct logs for fresh Pump creation or PumpSwap
  pool-creation events; discard failed, stale, and irrelevant firehose traffic
  before HTTP.
- During one continuously running stream instance, give each selected
  signature at most one authoritative `getTransaction` attempt after cross-subscription
  deduplication, under global request pacing and a concurrency bound shared by
  both sources. A discovery that ages out before admission is skipped without
  HTTP. A miss skips the attempted signature without retry or WebSocket
  teardown; a provider rate-limit response delays only later signatures.
- Reconnect silent subscriptions through an idle watchdog in explicit
  live-first mode with no historical backfill.
- Strictly decode the supported pinned-IDL Pump creation, trade, completion,
  and migration events and PumpSwap pool-creation, buy, sell, deposit, and
  withdrawal events.
- Decode supported Anchor CPI events from transaction instruction data rather
  than assuming every event is present in text logs.
- Keep direct program-data evidence canonical and pair the immediately
  following silent event self-CPIs 1:1 without double counting. Mark
  same-source windows incomplete for CPI-only, out-of-order, mismatched,
  malformed, future-discriminator, truncated, or unbalanced logs without
  adding activity HTTP reads.
- Preserve exact transaction instruction/event coordinates, optional provider
  transaction index, market/quote identity, receipt time, decoder version, and
  compact exact source evidence.
- Persist normalized observations and leased durable work before projection.
- Deduplicate accepted live facts using chain identity, preserving one
  canonical receipt and direct evidence for compatible replay without duplicate
  work or membership, while failing closed on divergent immutable evidence.
- Verify pinned HTTP genesis identity, then immutably bind each database to one
  configured Solana network before ingestion.
- Quarantine attributable malformed event or log evidence without admitting it
  as a candidate.
- Project every structurally valid candidate as `OBSERVED` for diagnostic
  `OBSERVE_ALL` inspection.
- Provision a configurable, capacity-bounded, non-extending in-memory window
  immediately for each fresh direct discovery log; retain receipt-time
  activity while HTTP is pending, then confirm it only after successful
  normalization or cancel it on failure. Route confirmed Pump mint/PumpSwap
  pool activity directly from PubSub into venue-scoped trades, buy/sell counts,
  atomic volume, and unique-trader activity.
- Publish coalesced named `soldisco` SSE notifications; on connection or buffer
  loss, require the browser to refresh bounded authoritative HTTP snapshots
  with explicit total/truncation metadata.
- Supervise start, reconnection, degraded state, stop, cancellation, and
  requested-running restoration.
- Prune eligible terminal history in bounded batches and fail collection closed
  at the configured database-size limit, which defaults to 5 GiB.
- Keep synthetic decoder data in fixtures and tests, never in product state.

This milestone established structural observation without claiming a safety
decision. Milestone 4 now adds a separate activity-quality qualification gate.
Pinned but intentionally unused global events are classified; future
discriminators and attributable malformed evidence fail closed into quarantine.

## 4. Durable discovery metrics and qualification — complete

- Confirm each provisional token as a durable PostgreSQL window only after its
  authoritative discovery observation commits.
- Pin exact market identity, half-open receipt-time bounds, collector run,
  Prefilter revision/value snapshot, and Qualification revision/value snapshot.
- Persist exact admitted observations before freezing versioned trades,
  buy/sell volume, unique-wallet, wallet-concentration, price, reserve, curve
  completion, and migration evidence.
- Apply inexpensive, versioned rules for window completeness, supported quote
  assets, minimum activity/volume, and maximum wallet concentration.
- Return and persist `PASS`, `REJECT`, or `UNKNOWN` with rule IDs, versions,
  reason codes, evidence, and evaluation time.
- Treat interruption, stream stop, source loss, queue overflow, and capacity
  eviction as explicit incomplete evidence that produces `UNKNOWN`, never a
  false zero-activity rejection.
- Commit snapshots, assessments, rule results, counters, rejection summaries,
  and current-token projection changes atomically.
- Publish only current `QUALIFIED`/future `APPROVED` candidates in the default
  `QUALIFIED_ONLY` feed while retaining diagnostic `OBSERVE_ALL`.
- Persist globally editable Qualification Defaults with optimistic revisions;
  each window pins its revision, so later edits affect new windows only.
- Require source progress through the close and settlement of all admitted work
  before a complete finalization claim; order cancellation against that claim
  so cancellation first yields `UNKNOWN`.
- Wait for same-window structural projection work to settle and rebuild the
  canonical final evidence from PostgreSQL before committing qualification.

`QUALIFIED` is an activity-quality admission result, not scam clearance, a risk
or ROI rating, a recommendation, or execution authority. Terminal raw source
observations still follow bounded retention, while window snapshots and
assessments are retained. Expiry/archive policy for those durable records and
whole-machine free-space/WAL monitoring remain operating-hardening work before
unattended collection.

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
