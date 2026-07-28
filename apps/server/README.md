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
- Pump and PumpSwap PubSub collection plus HTTP transaction retrieval
- bounded ordered-concurrent HTTP retrieval, finite fetch retries, and a
  silent-subscription watchdog
- checkpointed HTTP recovery before live events are released; when bounded
  recovery cannot reach the durable checkpoint, the gap is recorded and live
  collection resumes in `DEGRADED`
- strict current-IDL decoding of supported logs and Anchor CPI events
- attributable malformed-event and log evidence quarantine with compact source
  evidence
- normalized observation and durable-work persistence before projection work
- venue-scoped Pump/PumpSwap activity and `OBSERVE_ALL` discovery projections
- bounded authoritative Discovery snapshots and authoritative token lookup from
  PostgreSQL
- coalesced named `soldisco` SSE notifications with an initial resync
  instruction
- automatic retry of transient pipeline faults, batched retention, and a local
  database-size guard that enters terminal `ERROR`

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
- `jobs/collector.rs` owns live subscriptions, authoritative transaction
  retrieval, reconnection, and recovery.
- `jobs/recovery.rs` reserves focused recovery contracts; active checkpoint
  recovery currently lives in `jobs/collector.rs`.
- `jobs/normalization.rs` maps decoded events to domain observations.
- `jobs/discovery.rs` claims durable work and commits the `OBSERVE_ALL`
  projection.
- `jobs/maintenance.rs` owns terminal-history retention and the storage guard.
- the remaining focused job modules reserve later screening, Raydium, and
  richer projection contracts without making those features active.

The server never accepts seed phrases or private keys, never signs
transactions, and never exposes PostgreSQL to the browser. The public Solana
RPC defaults are suitable for initial local testing but may rate-limit sustained
mainnet collection; use locally configured dedicated RPC URLs when needed.
