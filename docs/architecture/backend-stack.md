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
| Persistence | Local PostgreSQL | Durable observations, work state, reserved checkpoints, decisions, snapshots, and projections |
| Database access | SQLx | Parameterized Rust queries, transactions, connection pooling, and migrations |
| Solana access | HTTP RPC plus WebSocket PubSub | Live program activity, account reads, and one-shot discovery transaction retrieval |
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
        ├── config.rs            Validated boot config and first-run defaults
        ├── state.rs             Shared service and repository handles
        ├── supervisor.rs        Truthful stream lifecycle and cancellation
        ├── http/
        │   ├── router.rs        Composes the browser-facing routes
        │   ├── error.rs         Stable, browser-safe API errors
        │   └── routes/
        │       ├── health.rs    Database and aggregate stream health
        │       ├── stream.rs    Start, stop, and stream-status commands
        │       ├── settings.rs  Revisioned global Prefilter Defaults
        │       ├── discovery.rs OBSERVE_ALL feed and current counters
        │       ├── tokens.rs    Observed-token inspector snapshots
        │       └── events.rs    SSE projection stream
        └── jobs/
            ├── pipeline.rs      Supervised local pipeline composition
            ├── collector.rs     PubSub and one-shot discovery HTTP retrieval
            ├── discovery_rpc.rs Shared dedupe, pacing, and rate-limit cooldown
            ├── intake.rs        Log prefiltering and active-window routing
            ├── pending_activity.rs Bounded provisional-activity holding
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
├── solana-rpc/                  HTTP, PubSub, and reserved recovery clients
├── discovery-engine/            Active windows, metrics, and qualification
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
GET  /api/v1/settings/prefilter-defaults
PUT  /api/v1/settings/prefilter-defaults
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
state. The aggregate stream enters `DEGRADED` immediately on a provider
rate-limit response or after three
consecutive one-shot discovery-read failures, and returns to `RUNNING` after a
successful read while both PubSub sources are ready. More granular collector,
recovery, screening, enrichment, and projection health remains a later
contract; a future global label must not hide a failed component.

## What workers and messaging are

A worker is a supervised Tokio task inside `apps/server`. It is background work
that continues without an open browser request. The implemented workers are:

- Pump and PumpSwap sources maintain subscriptions with an idle watchdog and
  prefilter fresh creation logs
- a stream-instance discovery-RPC gate deduplicates signatures across both
  subscriptions for their full freshness horizon, paces request starts, bounds
  concurrency, and applies a cooldown to later signatures after a provider
  rate-limit response
- provisional observation windows immediately capture matching mint/pool
  activity directly from PubSub without HTTP, then confirm only after the
  one-shot discovery transaction normalizes successfully
- live notifications are timestamped and admitted up to the collector queue
  bound independently of the smaller HTTP concurrency limit; a separate
  bounded holding queue retains activity whose token is still provisional,
  with a resolution deadline separate from the observation close time
- the collector processor decodes both Pump programs, persists discoveries
  before same-transaction activity, and confirms or cancels provisional windows
- the discovery worker claims leased durable work and commits the
  `OBSERVE_ALL` projection
- the maintenance worker prunes eligible terminal history in bounded batches
  and enforces the configured database-size guard

Screening, Raydium enrichment, and richer projection work are reserved focused
boundaries, not active workers yet.

High-volume live notification work, provisional activity, and transaction
batches each use the configured bounded capacity, so an RPC backlog cannot
consume unlimited memory or restrict WebSocket polling to the smaller HTTP
concurrency limit. Small lifecycle, connection, and wake signals use focused
Tokio synchronization primitives. “Receipt time” begins when the subscription
record enters available collector work capacity; under full saturation,
socket-buffered records are admitted and timestamped later, then stale
discoveries fail closed instead of creating unbounded local intake.

Channels are ephemeral and are not the system of record. Raw live intake and
provisional activity can be lost before normalization in this explicitly
incomplete live-first mode. Once an observation and its downstream work are
committed, the in-process wake signal is never the only copy of unfinished
durable work. The reliability sequence is:

1. prefilter one successful fresh discovery and fetch its authoritative
   transaction once
2. quarantine attributable malformed evidence without admitting a candidate
3. commit normalized observations and durable work state in PostgreSQL
4. wake the discovery worker through an in-process signal
5. lease and process the work idempotently
6. commit the rebuildable projection and complete the work lease
7. publish a coalesced SSE projection-change notification

If the process exits between steps, durable observations and leased work remain
recoverable through PostgreSQL, but the live source resumes at the current head
and open in-memory observation windows are lost. Stream stop/start and a
supervised pipeline-attempt restart also recreate the registry. Capacity
eviction can truncate a window without a durable completeness marker. The
collector never claims that a live-first interval is complete.

## Pump, PumpSwap, and Raydium boundaries

`source-pump` is the implemented discovery source. Its strict current-IDL
decoder handles supported Pump creation, trade, curve completion, and migration
events plus PumpSwap pool creation, buy, and sell events. Unknown discriminators
are ignored; attributable malformed event or log evidence is quarantined.
Neither path creates an `OBSERVED` candidate.

PubSub is the low-latency discovery and activity source. Failed notifications,
irrelevant events, and stale creation events are discarded from their direct
logs. During one continuously running stream instance, a fresh Pump creation
or PumpSwap pool creation can receive at most one globally deduplicated and
paced HTTP transaction attempt with the same signature and exact slot. If it ages out
before request admission, the collector cancels its provisional window and
skips HTTP. If an attempted request fails, the collector also cancels that
window and moves on. A provider rate-limit response places later distinct
signatures into a shared cooldown but does not retry the failed signature.
Stopping and starting the stream, or restarting the process, recreates this
in-memory claim set without performing intentional retry or backfill. Accepted
discovery transactions retain full attributed program-data and supported
Anchor CPI evidence for both Pump programs; matching active-window activity is
decoded directly from PubSub.
Receipt-time tokens preserve activity that arrived while HTTP or queue work was
pending. A watchdog reconnects silent subscriptions at the current head without
missed-history recovery.

The prefilter is intentionally direct-log-only. A qualifying creation visible
only as an Anchor event CPI instruction does not trigger an HTTP read in this
live-first milestone.

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
- reserved collector/recovery checkpoints that live-first intake does not
  consume
- malformed intake quarantine and durable collection-gap records
- leased observation-work state
- PumpSwap pool identity needed to resolve later events
- venue-scoped cumulative activity and unique traders
- `OBSERVED` discovery tokens, counters, and rebuildable projection events

Finality/correction relationships, rolling immutable snapshots, deterministic
evidence and scores, durable approved-candidate windows, strategy records,
paper records, and execution records remain later milestones. The implemented
short pre-decision window is in-memory.

Active maintenance removes eligible terminal observation/work history,
replaceable projection events, and quarantine records after their configured
ages in bounded batches. Pending or leased work, current discovery/activity
aggregates, checkpoints, pool identities, rejection summaries, and collection
gaps are not pruned. The default terminal-history window is 24 hours, so replay
tooling must report when requested raw evidence is no longer retained.

Completed or migrated Pump markets retire from the active startup/live market
registry. PumpSwap pool identities and current token/activity/trader
projections remain durable. Structurally attributable historical facts whose
market or quote cannot be resolved are quarantined; they require a future
explicit replay/reprocessing path.

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
That means the present aggregate projection and in-memory window registry are
not yet suitable for indefinite mainnet collection. The next milestone needs
durable on-demand active-window identity and a versioned aggregate
archive/expiry policy before continuous deployment.

## Local-only operating assumptions

- The web app binds to `localhost:3000`.
- The API binds to loopback at `127.0.0.1:8080` by default.
- PostgreSQL binds locally at `127.0.0.1:5432`.
- CORS allows the configured local web origin, not arbitrary sites.
- Solana RPC endpoints are outbound dependencies of the Rust server.
- The public Solana endpoints in `.env.example` use one paced discovery read
  per second and a five-second shared cooldown after a provider rate-limit
  response. They may still
  rate-limit or restrict sustained mainnet subscriptions; dedicated
  configurable HTTP and WebSocket endpoints are recommended for continuous
  operation.
- Secrets and local connection strings stay out of Git.
- Closing the Rust process stops collection; the next start resumes at the
  current live head without backfill.
- No cloud deployment, managed database, Docker hosting, or uptime promise is
  part of the current milestone.

## Deferred deployment

A future deployment can package the same Rust binary and connect it to a
managed PostgreSQL service without changing the domain boundaries. That work
will require explicit decisions about authentication, TLS, secret storage,
backups, RPC capacity, retention, observability, cost, and access control. It is
not part of the current implementation.
