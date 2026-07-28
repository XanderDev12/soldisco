# Backend stack and local runtime

This document is the canonical decision for Soldisco's initial backend. It
defines the selected tools, process boundaries, repository layout, and local
communication paths. It does not claim that every integration described here
is already implemented.

## Runtime topology

```text
React/TypeScript UI               Rust modular monolith             PostgreSQL
localhost:3000  --HTTP + SSE----> 127.0.0.1:8080 --SQLx only------> 127.0.0.1:5432
                                          |
                                          +--HTTP RPC-------------> Solana
                                          +--WebSocket PubSub-----> Solana
```

All three Soldisco components are intended to run on the user's computer. The
Rust process is the only database client. The browser never receives database
credentials and does not infer domain truth from local UI state. The server and
database foundation exist; the current web app is not connected to them yet.

The existing Sites deployment remains a disconnected UI preview. Hosting the
Rust collector or database is explicitly deferred; this architecture makes no
cloud-service or paid-service assumption.

## Selected technologies

| Responsibility | Selection | Purpose |
| --- | --- | --- |
| Browser interface | Existing React and TypeScript app | Render projections and collect explicit user intent |
| Backend language | Stable Rust | Continuous decoding, concurrency, deterministic processing, and security-sensitive future work |
| Async runtime | Tokio | Supervised background tasks, bounded channels, timers, and graceful shutdown |
| HTTP layer | Axum with Tower middleware | Commands, snapshots, health, browser-safe errors, and SSE |
| Browser live updates | Server-Sent Events | One-way server-to-browser projection updates with reconnect behavior |
| Persistence | Local PostgreSQL | Durable observations, checkpoints, work state, decisions, snapshots, and projections |
| Database access | SQLx | Parameterized Rust queries, transactions, connection pooling, and migrations |
| Solana access | HTTP RPC plus WebSocket PubSub | Live program activity, account reads, transaction retrieval, and missed-event recovery |
| Serialization | Serde | Internal and browser-facing typed payloads |
| Observability | `tracing` | Structured local logs and component health |

No external event bus, service mesh, graph database, or cache is selected for
the first backend.

## Repository layout

```text
apps/
├── web/                         Existing React/TypeScript interface
└── server/                      The single Rust executable
    └── src/
        ├── main.rs              Process startup and graceful shutdown
        ├── config.rs            Validated environment configuration
        ├── state.rs             Shared service and repository handles
        ├── supervisor.rs        Truthful stream lifecycle and cancellation
        ├── http/
        │   ├── router.rs        Composes the browser-facing routes
        │   ├── error.rs         Stable, browser-safe API errors
        │   └── routes/
        │       ├── health.rs    Database and aggregate stream health
        │       ├── stream.rs    Start, stop, and stream-status commands
        │       ├── discovery.rs Approved feed and screening snapshot
        │       ├── tokens.rs    Evidence and token-inspector snapshots
        │       └── events.rs    SSE projection stream
        └── jobs/                Typed messages; live tasks are deferred
            ├── collector.rs     Pump/PumpSwap subscription boundary
            ├── recovery.rs      Recovery-request boundary
            ├── screening.rs     Versioned screening-request boundary
            ├── raydium.rs       Optional venue-enrichment request
            ├── projection.rs    Projection-change boundary
            └── retention.rs     Retention-policy boundary

crates/
├── domain/                      Provider-independent facts and transitions
├── api-contracts/               Serde browser DTOs and live-event shapes
├── source-pump/                 Verified IDs and decoder-facing envelopes
├── source-raydium/              Verified IDs and venue-evidence contracts
├── solana-rpc/                  Provider-neutral read/recovery trait
├── discovery-engine/            Initial rolling metrics and qualification
├── risk-engine/                 Fail-closed rules and separate score types
├── persistence/                 SQLx repositories and migrations
└── projections/                 UI-specific rebuildable read models
```

The crates are compile-time ownership boundaries, not separately deployed
services. They compile into `apps/server`, which is initially the only Rust
process.

Strategy, wallet intelligence, paper portfolio, PnL, and execution boundaries
are added when those milestones begin. Live execution will not be folded into
the discovery process merely because both are written in Rust.

## What the API is

The API is the browser-facing portion of `apps/server`; it is not a second
backend service.

Regular HTTP handles finite interactions:

- start or stop collection
- request stream and component health
- load the latest Discovery snapshot
- inspect a token and its evidence
- change validated configuration later

The initial route contract is:

```text
GET  /api/v1/health
GET  /api/v1/stream
POST /api/v1/stream/start
POST /api/v1/stream/stop
GET  /api/v1/discovery
GET  /api/v1/tokens/{mint}
GET  /api/v1/events
```

Stop is already an idempotent command: it ends requested collection without
terminating the HTTP server or deleting durable state. The Start route is
present but truthfully returns `PUMP_COLLECTOR_NOT_IMPLEMENTED` until the Pump
collector exists; it does not report a fake running state. Once intake lands,
Start will become an idempotent supervisor command. Route handlers request
state changes through the supervisor; they do not spawn untracked collectors
themselves.

SSE keeps one long-lived HTTP response open so the server can publish:

- approved-token projection changes
- Pending, Approved, Rejected, and flow-counter changes
- compact rejection-reason changes
- collector, RPC, recovery, and database health changes

SSE is not the durable event log and does not replay from `Last-Event-ID`. Each
connection begins with `RESYNC_REQUIRED`; the browser then loads an
authoritative snapshot before trusting later projection-change notifications.
Commands still use ordinary HTTP. WebSockets between the browser and Soldisco
are unnecessary until a measured two-way streaming requirement appears.

Collector, recovery, screening, enrichment, database, and projection health
advance independently through explicit states such as stopped, starting,
running, degraded, and error. A single green global label must not hide a
failed component.

## What workers and messaging are

A worker is a supervised Tokio task inside `apps/server`. It is background work
that continues without an open browser request:

- the collector maintains Solana subscriptions
- recovery closes gaps after disconnects or downtime
- screening evaluates queued candidates
- Raydium enrichment resolves and checks exact pools when applicable
- projection updates UI-facing read models
- retention monitors local storage policy

Workers exchange small, strongly typed messages through bounded Tokio channels.
Bounded capacity creates backpressure rather than allowing an RPC burst to
consume unlimited memory.

Channels are ephemeral. They are never the system of record and never the only
copy of unfinished work. The reliability sequence is:

1. normalize enough information to identify the chain fact
2. commit the observation and durable work state in PostgreSQL
3. publish a lightweight in-process message
4. process the work idempotently
5. record the result and advance the durable checkpoint
6. publish a rebuildable projection update

If the process exits between steps, restart recovery reads PostgreSQL and
continues from the durable checkpoint.

## Pump, PumpSwap, and Raydium boundaries

`source-pump` is the initial discovery source. It decodes relevant Pump and
PumpSwap creations, trades, curve completion, and migration facts from Solana
transactions and logs. Live PubSub reduces latency; HTTP RPC supplies account
evidence and closes missed-history gaps.

`source-raydium` is an optional post-Pump venue-evidence layer. For a candidate
already known through Pump intake, it may resolve exact Raydium CPMM, CLMM, or
AMM v4 pools and produce venue-scoped liquidity, price, volume, and flow
evidence. It does not initially scan every Raydium pool as an independent
discovery firehose.

Each pool remains separately identified by program, pool address, token pair,
and observation coordinates. Metrics from PumpSwap and Raydium, or from
multiple Raydium pools, are not silently merged. Absence of a Raydium pool is
recorded as unavailable evidence and is not automatically a rejection unless a
versioned rule or strategy explicitly requires that venue.

## PostgreSQL ownership

PostgreSQL is local durable memory, not a browser datastore. Only the
`persistence` crate contains SQLx-specific code and migrations. Other crates
depend on repository interfaces or domain types rather than embedding SQL.

The database stores compact facts required for correctness and replay:

- chain observations and decoder versions
- transaction, instruction, slot, and exact-market identity
- deduplication keys and finality/correction relationships
- collector and recovery checkpoints
- rolling-measurement inputs and immutable snapshots
- deterministic evidence, rule versions, scores, and reason codes
- candidate-window and worker state
- rebuildable browser projections
- strategy, paper, and execution records in later milestones

The system does not need to retain every full RPC response forever. A
versioned retention policy may remove bulky redundant payloads after preserving
the normalized evidence, hash/reference, and chain coordinates required to
retrieve or audit the fact. Retention never silently rewrites history or makes
an incomplete replay appear complete.

## Local-only operating assumptions

- The web app binds to `localhost:3000`.
- The API binds to loopback at `127.0.0.1:8080` by default.
- PostgreSQL binds locally at `127.0.0.1:5432`.
- CORS allows the configured local web origin, not arbitrary sites.
- Solana RPC endpoints are outbound dependencies of the Rust server.
- Secrets and local connection strings stay out of Git.
- Closing the Rust process stops collection; recovery resumes from durable
  checkpoints the next time it starts.
- No cloud deployment, managed database, Docker hosting, or uptime promise is
  part of the current milestone.

## Deferred deployment

A future deployment can package the same Rust binary and connect it to a
managed PostgreSQL service without changing the domain boundaries. That work
will require explicit decisions about authentication, TLS, secret storage,
backups, RPC capacity, retention, observability, cost, and access control. It is
not part of the current implementation.
