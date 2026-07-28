# Infrastructure

This directory owns reproducible local runtime dependencies and records
explicitly deferred operational decisions.

The selected development topology is local-first:

```text
React web app :3000 -> Rust server :8080 -> PostgreSQL :5432
                                      |
                                      +-> Solana RPC HTTP and WebSocket
```

The Rust backend is one modular process. The current foundation defines typed
collector, recovery, screening, projection, Raydium, and retention job
boundaries. Their future supervised tasks will communicate through bounded
in-process Tokio channels; only the SSE broadcast channel exists now.
Repository folders do not imply separate services or containers.

Current decisions:

- `postgres` contains the optional Docker Compose setup for a local-only
  PostgreSQL instance. PostgreSQL is the durable system of record and is
  accessed only by the Rust backend.
- `nats` records why an external message broker is not required.
- `telemetry` records the logging and health boundary for the Rust server.

No hosted backend or database is part of the current milestone. Deployment can
be designed later without changing the domain or persistence boundaries.
