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
- supervised, idempotent Start and Stop commands with durable requested state
- successful-log prefiltering for fresh Pump creation and PumpSwap pool
  creation before HTTP
- at most one `getTransaction` attempt for each selected signature during the
  running server process, with discoveries that age out before admission
  skipped without HTTP, duplicate Pump/PumpSwap subscription delivery
  deduplicated, and one global request pace, rate-limit cooldown, and
  concurrency limit shared across both sources
- live-first reconnects with no transaction retry or historical backfill, plus
  a silent-subscription watchdog
- configurable bounded provisional observation windows that capture immediate
  mint/pool activity directly from PubSub, then confirm it only after the
  authoritative discovery normalizes successfully
- strict current-IDL decoding of supported logs and Anchor CPI events
- attributable malformed-event and log evidence quarantine with compact source
  evidence
- normalized observation and durable-work persistence before projection work
- venue-scoped Pump/PumpSwap activity and `OBSERVE_ALL` discovery projections
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

The current discovery worker is intentionally structural only. It records
valid decoded candidates as `OBSERVED`; it does not apply thresholds, approve
or reject tokens, or produce risk/opportunity scores.

## Module map

- `config.rs` validates environment input without exposing the database URL.
- `state.rs` owns database, event-bus, and supervisor handles.
- `supervisor.rs` owns desired/actual stream lifecycle and cancellation.
- `http/` contains transport-only routing, errors, and route handlers.
- `jobs/pipeline.rs` composes and supervises the collector processor and
  discovery worker.
- `jobs/collector.rs` owns live subscriptions, one-shot discovery transaction
  retrieval, and live-first reconnection.
- `jobs/discovery_rpc.rs` owns global discovery-signature deduplication,
  request pacing, concurrency admission, and shared rate-limit cooldown.
- `jobs/intake.rs` owns direct-log scoping, discovery freshness, and active
  window routing.
- `jobs/pending_activity.rs` owns bounded holding for receipt-time activity
  whose provisional discovery has not resolved yet; its holding deadline does
  not cancel the global discovery token or redefine window membership.
- `jobs/recovery.rs` reserves focused recovery contracts for a future explicit
  recovery mode; it is not active in the live-first collector.
- `jobs/normalization.rs` maps decoded events to domain observations.
- `jobs/discovery.rs` claims durable work and commits the `OBSERVE_ALL`
  projection.
- `jobs/maintenance.rs` owns terminal-history retention and the storage guard.
- the remaining focused job modules reserve later screening, Raydium, and
  richer projection contracts without making those features active.

The server never accepts seed phrases or private keys, never signs
transactions, and never exposes PostgreSQL to the browser. The public Solana
RPC defaults are paced at one discovery read per second and enter a shared
five-second cooldown after a provider rate-limit response, without retrying the
failed signature. They
may still limit sustained mainnet collection; use locally configured dedicated
RPC URLs when needed.
