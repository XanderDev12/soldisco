# Rust server

`soldisco-server` is the single local backend process. It composes the focused
workspace crates; the job modules are internal ownership boundaries, not
separate services.

## Implemented local vertical slice

- validated loopback-only HTTP configuration
- SQLx connection pooling and startup migrations
- pinned HTTP genesis verification followed by immutable binding of each
  database to one configured Solana network before ingestion
- structured JSON logging and graceful shutdown
- database-aware health
- supervised, idempotent Start and Stop commands whose requested-running intent
  is persisted in PostgreSQL and restored after a server restart
- revisioned global Prefilter Defaults persisted in PostgreSQL, editable only
  while explicitly stopped and applied through a fresh pipeline/RPC gate on
  the next Start
- append-only Qualification Defaults revisions persisted in PostgreSQL,
  editable while running and pinned with their complete values by each new
  durable window
- successful-log prefiltering for fresh Pump creation and PumpSwap pool
  creation before HTTP
- at most one `getTransaction` attempt for each selected signature during the
  continuously running stream instance, with discoveries that age out before
  admission skipped without HTTP, duplicate Pump/PumpSwap subscription delivery
  deduplicated, and one global request pace, rate-limit cooldown, and
  concurrency limit shared across both sources
- live-first reconnects with no transaction retry or historical backfill, plus
  a silent-subscription watchdog
- configurable bounded provisional observation windows that capture immediate
  mint/pool activity directly from PubSub, then atomically become durable only
  after the authoritative discovery normalizes successfully
- exact half-open receipt-time membership for each durable Pump mint or
  PumpSwap pool window
- strict pinned-IDL classification with the direct Pump/PumpSwap program-data
  event as canonical evidence; its immediately following silent event self-CPI
  corroborates that event only under ordered 1:1 per-instruction pairing and
  is never counted again
- same-source incomplete-window handling for CPI-only, out-of-order,
  mismatched, future-discriminator, malformed, truncated, or unbalanced PubSub
  logs, without spending activity HTTP capacity
- attributable malformed-event and log evidence quarantine with compact source
  evidence
- immutable chain-evidence conflict detection: compatible replay keeps one
  canonical receipt and direct evidence without duplicating work or membership,
  while divergent reuse of the same chain coordinate fails closed
- normalized observation and durable-work persistence before projection work
- versioned immutable window features for trades, buy/sell flow, unique
  wallets, wallet concentration, price, reserves, completion, and migration
- versioned activity qualification with persisted `PASS`, `REJECT`, or
  `UNKNOWN` assessments and rule evidence
- a default `QUALIFIED_ONLY` projection with diagnostic `OBSERVE_ALL`, truthful
  qualification counters, and rejection summaries
- bounded authoritative Discovery snapshots and authoritative token lookup from
  PostgreSQL
- coalesced named `soldisco` SSE notifications with an initial resync
  instruction
- automatic restart of transient pipeline-level faults, batched retention, and
  a local database-size guard that enters terminal `ERROR`

`POST /api/v1/stream/start` now launches the collector and reports the actual
supervised state: `STARTING`, `RUNNING`, `DEGRADED`, or `ERROR`. Stop cancels
the collection tasks without terminating the HTTP server or deleting durable
observations. Both commands require the fixed local-control header, the exact
configured Host, and—when a browser supplies it—the exact configured Origin.
`GET /api/v1/settings/prefilter-defaults` exposes the current values and
supported bounds. Its locally guarded `PUT` replacement requires the stream to
be stopped and an exact expected revision, preventing hidden restarts and
lost updates between browser sessions.
`GET /api/v1/settings/qualification-defaults` and its locally guarded `PUT`
counterpart expose the independent qualification policy. A coherent optimistic
revision can be saved while collection runs; open windows retain their pinned
revision and only new windows use the update.

The discovery worker remains structural and records valid decoded candidates
as `OBSERVED`. The separate qualification finalizer freezes exact-window
evidence only after the source has progressed through the half-open close,
every admitted batch has settled, and the window's structural projection work
is no longer pending or processing. Stop/disconnect cancellation and the
finalization claim share one ordering boundary: cancellation first freezes an
incomplete `UNKNOWN`; an already eligible claim first keeps its frozen prior
completeness. The finalizer may then advance the current exact-market candidate
to `QUALIFIED`. That stage is only an activity-quality pass. It is not scam
clearance, safety approval, an ROI rating, a recommendation, or permission to
trade. Risk and opportunity scores remain unavailable.

## Module map

- `config.rs` validates environment input without exposing the database URL.
- `state.rs` owns database, event-bus, and supervisor handles.
- `supervisor.rs` owns desired/actual stream lifecycle and cancellation. It
  also serializes Start, Stop, and persisted Prefilter Defaults changes so a
  settings update cannot race stream launch.
- `http/` contains transport-only routing, errors, and route handlers.
- `jobs/pipeline.rs` composes and supervises the collector processor,
  discovery worker, qualification finalizer, and maintenance.
- `jobs/collector.rs` owns live subscriptions, one-shot discovery transaction
  retrieval, live-first reconnection, per-source progress, and settlement of
  every receipt-time admission even when in-flight work is cancelled.
- `jobs/discovery_rpc.rs` owns global discovery-signature deduplication,
  request pacing, concurrency admission, and shared rate-limit cooldown.
- `jobs/intake.rs` owns direct-log scoping, ordered silent event self-CPI
  corroboration, discovery freshness, same-source coverage gaps, and active
  window routing.
- `jobs/pending_activity.rs` owns bounded holding for receipt-time activity
  whose provisional discovery has not resolved yet; its holding deadline does
  not cancel the global discovery token or redefine window membership.
- `jobs/recovery.rs` reserves focused recovery contracts for a future explicit
  recovery mode; it is not active in the live-first collector.
- `jobs/normalization.rs` maps decoded events to domain observations.
- `jobs/discovery.rs` claims durable work and commits the structural projection
  used by both diagnostic and qualified views.
- `jobs/qualification.rs` freezes closed durable windows, evaluates their
  pinned rules, persists immutable audit records, and updates the qualified
  projection atomically. It retries while same-window structural projection
  work is unsettled. Interrupted windows finalize as incomplete `UNKNOWN`;
  superseded markets keep their audit without overwriting the newer token
  projection.
- `jobs/maintenance.rs` owns terminal-history retention and the storage guard.
- the remaining focused job modules reserve later deterministic risk, Raydium,
  strategy, and richer projection work without making those features active.

The server never accepts seed phrases or private keys, never signs
transactions, and never exposes PostgreSQL to the browser. The public Solana
RPC defaults are paced at one discovery read per second and enter a shared
five-second cooldown after a provider rate-limit response, without retrying the
failed signature. They
may still limit sustained mainnet collection; use locally configured dedicated
RPC URLs when needed.

Terminal normalized observations, complete versioned `source_details`, and
compact source evidence are pruned under the configured raw-history policy.
Frozen feature snapshots and qualification audits are retained, so they remain
useful after raw expiry but are not a substitute for complete source replay.
Durable windows, snapshots, assessments, and rule results do not yet have an
archive policy and will grow PostgreSQL until the local size guard stops
collection.

Prefilter Defaults, Qualification Defaults, and requested-running stream intent
are backend settings and therefore live in PostgreSQL. The browser's
execution-mode presentation and adjustable dashboard widths live separately
in `localStorage`; order drafts and transient navigation are deliberately not
backend settings or execution authorization.
