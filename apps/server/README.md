# Rust server

`soldisco-server` is the single local backend process. It composes the focused
workspace crates; the job modules are internal ownership boundaries, not
separate services.

## Implemented foundation

- validated loopback-only HTTP configuration
- SQLx connection pooling and startup migrations
- structured JSON logging and graceful shutdown
- database-aware health
- authoritative empty Discovery snapshots
- approved-token lookup with an honest not-found response
- stream state and an idempotent Stop command
- SSE projection notifications with an initial resync instruction
- typed job messages for collector, recovery, screening, Raydium enrichment,
  projection, and retention work

`POST /api/v1/stream/start` intentionally returns
`PUMP_COLLECTOR_NOT_IMPLEMENTED` until live Pump/PumpSwap intake exists. The
server does not claim an empty task is a running market stream.

## Module map

- `config.rs` validates environment input without exposing the database URL.
- `state.rs` owns database, projection, event-bus, and supervisor handles.
- `supervisor.rs` owns desired/actual stream lifecycle and cancellation.
- `http/` contains transport-only routing, errors, and route handlers.
- `jobs/` contains strongly typed worker boundaries that later supervised
  Tokio tasks will consume.

The server never accepts seed phrases or private keys, never signs
transactions, and never exposes PostgreSQL to the browser.
