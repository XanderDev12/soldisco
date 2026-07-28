# Backend stack and local runtime

This document is the canonical decision for Soldisco's initial backend. It
defines the selected tools, process boundaries, repository layout, and local
communication paths. It distinguishes the implemented Pump discovery slice
from later screening, enrichment, strategy, and execution work.

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
database foundation, Pump/PumpSwap intake, and local web connection are
implemented.

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
        │       ├── discovery.rs OBSERVE_ALL feed and current counters
        │       ├── tokens.rs    Observed-token inspector snapshots
        │       └── events.rs    SSE projection stream
        └── jobs/
            ├── pipeline.rs      Supervised local pipeline composition
            ├── collector.rs     PubSub, HTTP retrieval, and recovery
            ├── normalization.rs Decoded events to domain observations
            ├── discovery.rs     Durable OBSERVE_ALL projection worker
            ├── maintenance.rs   Active retention and storage-size guard
            ├── recovery.rs      Reserved focused recovery contracts
            ├── screening.rs     Reserved screening boundary
            ├── raydium.rs       Reserved venue-enrichment boundary
            └── projection.rs    Projection-change boundary

crates/
├── domain/                      Provider-independent facts and transitions
├── api-contracts/               Serde browser DTOs and live-event shapes
├── source-pump/                 Strict current-IDL event decoder
├── source-raydium/              Verified IDs and venue-evidence contracts
├── solana-rpc/                  HTTP, PubSub, and bounded recovery clients
├── discovery-engine/            Unwired rolling-metric/qualification boundary
├── risk-engine/                 Unwired fail-closed risk boundary
├── persistence/                 SQLx repositories and migrations
└── projections/                 Rebuildable read-model contracts
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
- load the latest bounded Discovery snapshot with total/truncation metadata
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

Start and Stop are idempotent supervisor commands. Start launches one tracked
pipeline and reports its actual `STARTING`, `RUNNING`, `DEGRADED`, or `ERROR`
state. Stop cancels its child tasks without terminating the HTTP server or
deleting durable state. Requested-running state is durable and can be restored
when the server restarts. Route handlers request state changes through the
supervisor; they do not spawn untracked collectors themselves. Both commands
require `X-Soldisco-Control: soldisco-local-ui-v1` and the exact configured
Host. A supplied browser Origin must also exactly match `WEB_ORIGIN`.

SSE keeps one long-lived HTTP response open so the server can publish a named
`soldisco` event for:

- stream-status changes
- discovery-projection changes
- required snapshot resynchronization

SSE is not the durable event log and does not replay from `Last-Event-ID`. Each
connection begins with `RESYNC_REQUIRED`; the browser then loads an
authoritative snapshot before trusting later projection-change notifications.
Busy projection invalidations are coalesced; the browser single-flights
refreshes and rejects regressing snapshot sequences. Commands still use
ordinary HTTP. WebSockets between the browser and Soldisco are unnecessary
until a measured two-way streaming requirement appears.

The current health contract exposes database and aggregate supervised-stream
state. More granular collector, recovery, screening, enrichment, and projection
health remains a later contract; a future global label must not hide a failed
component.

## What workers and messaging are

A worker is a supervised Tokio task inside `apps/server`. It is background work
that continues without an open browser request. The implemented workers are:

- Pump and PumpSwap sources maintain subscriptions with an idle watchdog,
  retrieve transactions through bounded ordered concurrency and finite retries,
  and attempt bounded checkpoint recovery
- the collector processor decodes, normalizes, persists, and advances
  checkpoints
- the discovery worker claims leased durable work and commits the
  `OBSERVE_ALL` projection
- the maintenance worker prunes eligible terminal history in bounded batches
  and enforces the configured database-size guard

Screening, Raydium enrichment, and richer projection work are reserved focused
boundaries, not active workers yet.

High-volume transaction batches cross a bounded, strongly typed Tokio channel,
so an RPC burst cannot consume unlimited memory. Small lifecycle, connection,
and wake signals use focused Tokio synchronization primitives.

Channels are ephemeral. They are never the system of record and never the only
copy of unfinished work. The reliability sequence is:

1. fetch the authoritative transaction and strictly decode supported evidence
2. quarantine attributable malformed evidence without admitting a candidate
3. commit normalized observations and durable work state in PostgreSQL
4. wake the discovery worker through an in-process signal
5. lease and process the work idempotently
6. commit the rebuildable projection and complete the work lease
7. advance the source checkpoint only after the handled transaction batch is
   durable
8. publish a coalesced SSE projection-change notification

If the process exits between steps, restart recovery reads PostgreSQL and
continues from the durable checkpoint. If that checkpoint is outside the
configured recovery bound or is no longer returned by the provider, Soldisco
records a durable gap and resumes live collection in `DEGRADED` rather than
claiming complete history.

## Pump, PumpSwap, and Raydium boundaries

`source-pump` is the implemented discovery source. Its strict current-IDL
decoder handles supported Pump creation, trade, curve completion, and migration
events plus PumpSwap pool creation, buy, and sell events. Unknown discriminators
are ignored; attributable malformed event or log evidence is quarantined.
Neither path creates an `OBSERVED` candidate.

PubSub is low-latency notification delivery, not authoritative transaction
content. Every success or failure notification requires a matching HTTP
transaction with the same signature, exact slot, and status before it can
advance state. Live and recovery fetches use bounded ordered concurrency with
finite retries, and a watchdog reconnects silent subscriptions. The collector
then decodes attributed program-data logs and supported Anchor CPI event
instructions with exact transaction coordinates. HTTP RPC attempts bounded
missed-history recovery from persisted checkpoints; unrecoverable bounded
history becomes an explicit gap.

`source-raydium` is a planned optional post-Pump venue-evidence layer. For a
candidate already known through Pump intake, it may resolve exact Raydium CPMM,
CLMM, or AMM v4 pools and produce venue-scoped liquidity, price, volume, and
flow evidence. It does not initially scan every Raydium pool as an independent
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

The implemented database slice stores:

- one immutable configured Solana-network binding
- chain observations, decoder versions, exact source-evidence bytes encoded as
  base64, and their hashes
- transaction signature, optional provider transaction index, instruction,
  event, slot, and exact-market identity
- deterministic chain-identity deduplication keys
- collector and recovery checkpoints
- malformed intake quarantine and durable collection-gap records
- leased observation-work state
- PumpSwap pool identity needed to resolve later events
- venue-scoped cumulative activity and unique traders
- `OBSERVED` discovery tokens, counters, and rebuildable projection events

Finality/correction relationships, rolling immutable snapshots, deterministic
evidence and scores, candidate windows, strategy records, paper records, and
execution records remain later milestones.

Active maintenance removes eligible terminal observation/work history,
replaceable projection events, and quarantine records after their configured
ages in bounded batches. Pending or leased work, current discovery/activity
aggregates, checkpoints, pool identities, rejection summaries, and collection
gaps are not pruned. The default terminal-history window is 24 hours, so replay
tooling must report when requested raw evidence is no longer retained.

Completed or migrated Pump markets retire from the active startup/live market
registry. PumpSwap pool identities and current token/activity/trader
projections remain durable. Structurally attributable historical facts whose
market or quote cannot be resolved are quarantined before checkpoint advance;
they require a future explicit replay/reprocessing path.

Before collection starts and on each maintenance interval, Soldisco compares
`pg_database_size` with `DATABASE_MAX_BYTES`, which defaults to 5 GiB. Reaching
the limit fails collection closed while the HTTP API remains available.
Deleted rows may be reused by PostgreSQL without immediately reducing the
physical database size.

The guard covers the configured database, not filesystem free space, WAL,
other databases, Docker storage, or build caches. Operators must preserve
machine-level headroom separately. Discovery-token, market, activity,
checkpoint, pool, rejection-summary, and gap projections are intentionally
retained in this milestone because later events depend on their identities.
That means the present aggregate projection and in-memory market registry are
not yet suitable for indefinite unfiltered mainnet collection. The next
milestone needs a durable on-demand market lookup, a bounded active cache, and
a versioned aggregate archive/expiry policy before continuous deployment.

## Local-only operating assumptions

- The web app binds to `localhost:3000`.
- The API binds to loopback at `127.0.0.1:8080` by default.
- PostgreSQL binds locally at `127.0.0.1:5432`.
- CORS allows the configured local web origin, not arbitrary sites.
- Solana RPC endpoints are outbound dependencies of the Rust server.
- The public Solana endpoints in `.env.example` may rate-limit or restrict
  sustained mainnet subscriptions; dedicated configurable HTTP and WebSocket
  endpoints are recommended for continuous operation.
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
