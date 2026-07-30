# Soldisco documentation

The documents are split by responsibility so implementation details do not
accumulate in one architecture file.

## Product and delivery

- [Product flow](product-flow.md) — end-to-end behavior and site features
- [Milestones](milestones.md) — implementation order and completion criteria
- [Configuration](configuration.md) — current and reserved configuration names

## Architecture

- [Backend stack](architecture/backend-stack.md) — selected local topology,
  Rust layout, HTTP/SSE boundary, workers, PostgreSQL, and Raydium enrichment
- [Overview](architecture/overview.md) — end-to-end component ownership and
  runtime shape
- [Invariants](architecture/invariants.md) — rules that implementations may not
  violate
- [Data lifecycle](architecture/data-lifecycle.md) — observations, snapshots,
  candidate windows, replay, and projections
- [Execution model](architecture/execution-model.md) — paper, interactive live,
  and separately designed automated modes

## Data and strategies

- [Wallet intelligence](data/wallet-intelligence.md) — time-versioned wallet
  profiles, relationships, clusters, and scores
- [Wallet-Conditioned Momentum](strategies/wallet-conditioned-momentum.md) —
  first strategy design and research requirements

The existing React/TypeScript application is the frontend source of truth, and
Rust-owned domain and API contracts are authoritative for connected backend
behavior. The implemented local slice covers Pump/PumpSwap collection,
durable bounded-window activity qualification, the default `QUALIFIED_ONLY`
discovery projection, PostgreSQL-persisted Controls settings and stream intent,
safe browser-local API-port/layout/mode preferences, and HTTP/SSE UI wiring.
Documents distinguish this inexpensive qualification gate from deferred
deterministic risk, Raydium, strategy, AI, trading, and wallet behavior.

The current runtime target is entirely local. By default, React runs on
`localhost:3000`, the Rust/Axum server on `127.0.0.1:8080`, and PostgreSQL on
`127.0.0.1:5432`. The validated browser-local API port can replace `8080` only
when it matches the restarted server's `API_PORT`; host `127.0.0.1` and path
`/api/v1` remain fixed. The hosted Sites UI is not connected to the local
backend.
