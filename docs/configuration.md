# Configuration

The initial runtime is local. The Rust server validates configuration at
startup and fails with a useful error instead of silently applying production
defaults. Actual secrets and connection strings must stay out of source
control.

## Implemented server variables

```text
APP_ENV
LOG_LEVEL

API_HOST
API_PORT
WEB_ORIGIN

DATABASE_URL
DATABASE_MAX_CONNECTIONS
DATABASE_MAX_BYTES
DISCOVERY_SNAPSHOT_LIMIT
RETENTION_TERMINAL_HISTORY_HOURS
RETENTION_PROJECTION_EVENTS_HOURS
RETENTION_QUARANTINE_HOURS
RETENTION_INTERVAL_MS
RETENTION_BATCH_SIZE

SOLANA_NETWORK
SOLANA_RPC_HTTP_URL
SOLANA_RPC_WS_URL
SOLANA_COMMITMENT
SOLANA_REQUEST_TIMEOUT_MS
SOLANA_RECONNECT_DELAY_MS
SOLANA_RPC_MAX_IN_FLIGHT
SOLANA_DISCOVERY_RPC_REQUESTS_PER_SECOND
SOLANA_DISCOVERY_RPC_RATE_LIMIT_COOLDOWN_MS
SOLANA_SUBSCRIPTION_IDLE_TIMEOUT_MS
DISCOVERY_MAX_EVENT_AGE_MS
DISCOVERY_OBSERVATION_WINDOW_MS
DISCOVERY_MAX_ACTIVE_WINDOWS
COLLECTOR_QUEUE_CAPACITY
STREAM_START_TIMEOUT_MS
```

The current Rust foundation loads and validates:

| Name | Local intent |
| --- | --- |
| `APP_ENV` | Explicit development environment |
| `LOG_LEVEL` | Local structured-log filter |
| `API_HOST` | Loopback only, normally `127.0.0.1` |
| `API_PORT` | Rust HTTP/SSE port, normally `8080` |
| `WEB_ORIGIN` | Exact local browser origin, normally `http://localhost:3000` |
| `DATABASE_URL` | Local PostgreSQL connection used only by Rust |
| `DATABASE_MAX_CONNECTIONS` | Small bounded SQLx connection pool |
| `DATABASE_MAX_BYTES` | Hard local database safety limit; defaults to 5 GiB |
| `DISCOVERY_SNAPSHOT_LIMIT` | Latest visible candidates returned to one browser snapshot; defaults to 500, maximum 5,000 |
| `RETENTION_TERMINAL_HISTORY_HOURS` | Age after which terminal observation/work history can be pruned; defaults to 24 hours |
| `RETENTION_PROJECTION_EVENTS_HOURS` | Retention for replaceable projection-notification history; defaults to 24 hours |
| `RETENTION_QUARANTINE_HOURS` | Retention for malformed intake evidence; defaults to 168 hours |
| `RETENTION_INTERVAL_MS` | Maintenance cadence, from 1 second through 1 hour |
| `RETENTION_BATCH_SIZE` | Rows removed per bounded table batch, from 1 through 10,000 |
| `SOLANA_NETWORK` | Explicit `mainnet`/`mainnet-beta` or `devnet` identity |
| `SOLANA_RPC_HTTP_URL` | Genesis verification and one-shot discovery transaction reads |
| `SOLANA_RPC_WS_URL` | Pump and PumpSwap program-log PubSub |
| `SOLANA_COMMITMENT` | Explicit `confirmed` or `finalized` read level; `getTransaction` cannot use `processed` |
| `SOLANA_REQUEST_TIMEOUT_MS` | Per-request/connect timeout from 1 ms through 5 minutes; it cannot exceed `DISCOVERY_MAX_EVENT_AGE_MS` |
| `SOLANA_RECONNECT_DELAY_MS` | Positive initial PubSub reconnect delay; connection retries back off |
| `SOLANA_RPC_MAX_IN_FLIGHT` | One global bound shared by Pump and PumpSwap one-shot discovery fetches, from 1 through 128; defaults to 4 |
| `SOLANA_DISCOVERY_RPC_REQUESTS_PER_SECOND` | Global start-rate limit for distinct one-shot discovery reads across Pump and PumpSwap, from 1 through 1,000; defaults to 1 |
| `SOLANA_DISCOVERY_RPC_RATE_LIMIT_COOLDOWN_MS` | Shared delay applied to later distinct signatures after a provider rate-limit response, from 100 ms through 5 minutes; defaults to 5 seconds |
| `SOLANA_SUBSCRIPTION_IDLE_TIMEOUT_MS` | Reconnect a silent/half-open PubSub subscription after this interval |
| `DISCOVERY_MAX_EVENT_AGE_MS` | Maximum age of a direct creation event before it is discarded without HTTP, from 1 second through 5 minutes; defaults to 15 seconds |
| `DISCOVERY_OBSERVATION_WINDOW_MS` | Non-extending provisional activity window opened at fresh discovery receipt and confirmed after successful normalization, from 1 second through 1 hour; defaults to 60 seconds and cannot be shorter than `SOLANA_REQUEST_TIMEOUT_MS` |
| `DISCOVERY_MAX_ACTIVE_WINDOWS` | In-memory bound for simultaneous mint/pool observation windows, from 1 through 100,000; defaults to 128 |
| `COLLECTOR_QUEUE_CAPACITY` | Bound reused for ahead-of-HTTP live notification work, the collector-to-processor queue, and provisional-activity holding, from 1 through 100,000; defaults to 2,048. Pending activity gets a release deadline equal to the discovery-age allowance plus the HTTP timeout, without extending its receipt-time observation window |
| `STREAM_START_TIMEOUT_MS` | Maximum command wait for an initial stream result |

## Browser-local API port

The frontend has no environment variable for its API endpoint or page origin.
It always constructs the versioned API base as
`http://127.0.0.1:<port>/api/v1`. Only the port is adjustable: it must be a
browser-safe whole number from `1` through `65535`, defaults to `8080`, and is
stored under a versioned browser `localStorage` key,
`soldisco.local-api-port.v1`. Fetch-restricted ports are rejected. Port `80` is
also rejected because URL normalization would omit it from the request
authority while the Rust control guard requires the exact configured
`API_HOST:API_PORT`. The same resolved endpoint is shared by Discovery
snapshots and SSE, stream commands, token inspection, Prefilter Defaults, and
Qualification Defaults. Clearing browser storage restores port `8080`; if
storage is blocked, the safe in-memory/default behavior remains available.

Changing this browser preference does not rebind or restart the Rust server.
The selected port must equal the backend `API_PORT`. Changing `API_PORT`
requires a backend restart, after which the browser preference must be updated
to match. The frontend host remains fixed at `127.0.0.1` and the path remains
fixed at `/api/v1`; this is not a free-form URL field and cannot contain
credentials.

`API_HOST`, `API_PORT`, and `WEB_ORIGIN` remain backend environment
configuration. For this browser client, `API_HOST` must be `127.0.0.1`, while
`WEB_ORIGIN` must exactly equal the actual page origin (normally
`http://localhost:3000`). Start, stop, and settings writes send the fixed
local-control header `X-Soldisco-Control: soldisco-local-ui-v1`. The Rust server
still rejects request authorities other than its configured
`API_HOST:API_PORT`, and CORS still rejects browser origins other than
`WEB_ORIGIN`. A port preference changes neither security check.

## Implemented Compose variables

```text
SOLDISCO_POSTGRES_DB
SOLDISCO_POSTGRES_USER
SOLDISCO_POSTGRES_PASSWORD
SOLDISCO_POSTGRES_PORT
```

These configure only the optional local PostgreSQL container. The root
`.env.example` is the single local template for Compose and the Rust process.
After copying it to `.env`, the `SOLDISCO_POSTGRES_PASSWORD` value and password
embedded in `DATABASE_URL` must match. The real `.env` is ignored by Git.

## Persisted Prefilter Defaults

The following validated environment values bootstrap one global
`prefilter_defaults` row the first time the migrated database starts:

```text
DISCOVERY_MAX_EVENT_AGE_MS
DISCOVERY_OBSERVATION_WINDOW_MS
DISCOVERY_MAX_ACTIVE_WINDOWS
SOLANA_DISCOVERY_RPC_REQUESTS_PER_SECOND
SOLANA_RPC_MAX_IN_FLIGHT
SOLANA_REQUEST_TIMEOUT_MS
SOLANA_DISCOVERY_RPC_RATE_LIMIT_COOLDOWN_MS
```

After that first initialization, PostgreSQL is authoritative. Later
environment edits do not silently replace settings saved from the Controls
view. The browser reads the current values and supported integer bounds from
`GET /api/v1/settings/prefilter-defaults`. It replaces the complete coherent
set through the locally authorized
`PUT /api/v1/settings/prefilter-defaults`, including the revision it read.
Stale revisions fail with a conflict instead of overwriting another browser
session.

These settings can be changed only while the discovery stream is explicitly
`STOPPED`. They affect observation-window identity, RPC admission, or request
lifetime, so the saved revision applies on the next stream start. Each start
reads the persisted row and constructs a fresh pipeline and discovery-RPC
gate. The server never performs a hidden automatic restart or creates an
unreported collection gap just to apply a setting.

“Fresh” in this context is strictly an intake-timing decision, not an
endorsement. A successful direct Pump `Create` or PumpSwap `CreatePool` event
must fall inside the maximum event age (with a bounded source clock-skew
allowance), remain fresh while waiting for global RPC admission, and return an
exact successful signature-and-slot match from the one-shot transaction read.
For PumpSwap, creation means a new supported pool and does not necessarily
mean the base mint itself was newly created.

Connection identities, credentials, database connection and pool settings,
network/commitment, local HTTP binding/origin, and logging bootstrap remain
environment-only process configuration. Strategy-specific thresholds, wallet
profiles, momentum windows, entries, exits, and sizing belong to versioned
strategy configuration rather than global Prefilter Defaults.

## Persisted Qualification Defaults

Qualification Defaults are global, strategy-neutral activity requirements for
the short discovery window. The server seeds one safe revision in PostgreSQL
only when no operator revision exists:

| Setting | Initial value | Supported range |
| --- | ---: | ---: |
| Minimum trades | 5 | 1–10,000 |
| Minimum unique traders | 3 | 1–10,000 and no greater than minimum trades |
| Minimum buys | 1 | 0–10,000 and no greater than minimum trades |
| Minimum sells | 1 | 0–10,000 and no greater than minimum trades |
| Minimum native-quote volume | 50,000,000 atomic units | 0–9,007,199,254,740,991 |
| Minimum stable-quote volume | 5,000,000 atomic units | 0–9,007,199,254,740,991 |
| Maximum single-wallet quote share | 9,000 basis points | 1,000–10,000 |

The browser reads values and bounds from
`GET /api/v1/settings/qualification-defaults` and replaces the complete set
through the locally authorized
`PUT /api/v1/settings/qualification-defaults`. Optimistic revisions reject a
stale browser write. Unlike Prefilter Defaults, these values may be saved while
the stream runs. Each confirmed durable discovery window pins the current
revision and complete value snapshot; a later edit applies only to subsequently
confirmed windows and never rewrites an open or finalized assessment. The API
therefore reports `NEW_WINDOWS` rather than requiring a stream restart.

Saved Prefilter and Qualification Defaults survive browser closure, Rust-server
restart, and computer restart because PostgreSQL is authoritative. The
requested Start/Stop intent is also stored there: Start saves
`requested_running = true`, Stop saves `false`, and server startup launches a
fresh supervised pipeline when the saved intent is true. Transient runtime
status such as `STARTING`, `DEGRADED`, or a task handle is recomputed rather
than persisted. These records are lost only if the local database or its volume
is deliberately deleted or reset. Future strategy-specific settings must use
their own versioned durable records rather than these global qualification
controls.

Three current interface preferences intentionally do not cross the backend
state boundary. The local API port, Paper/Live presentation choice, and
adjustable sidebar/inspector widths are validated and stored under versioned
keys in browser `localStorage`. Clearing that browser storage restores safe
defaults, and a blocked or full storage provider leaves the in-memory UI
usable. The API port only selects the fixed loopback endpoint described above;
the mode is presentation only and never authorizes wallet access, signing, or
execution. Unsaved settings form text, order-side/amount drafts, the active
destination, selected token, inspector tab, and open modals are transient and
reset with the page or session.

Qualification is deliberately narrower than risk screening. `PASS` means a
complete exact-market window met the configured activity and concentration
requirements. `REJECT` means one or more inexpensive requirements failed.
`UNKNOWN` means collection or required evidence was incomplete. None of these
decisions asserts that a token is safe, predicts ROI, recommends a purchase, or
authorizes trading.

## Solana RPC selection

The committed template uses Solana's public mainnet HTTP and WebSocket
endpoints so the local vertical slice can be started without a paid service.
Those endpoints can rate-limit, restrict subscriptions, or become unreliable
under sustained mainnet volume. Configure a dedicated provider's matching HTTP
and WebSocket URLs in the ignored `.env` for continuous use.

`SOLANA_NETWORK` is validated separately from the endpoint URLs; switching a
URL does not silently change chain identity. On the first requested pipeline
start, HTTP RPC `getGenesisHash` must match Soldisco's pinned official
mainnet/devnet identity before the database is immutably bound or ingestion can
begin. A later network switch against the same database fails closed. Only
successful, fresh creation logs selected by the prefilter are corroborated
through that verified HTTP RPC because Solana PubSub has no equivalent genesis
method. During one continuously running stream instance, duplicate delivery
across the Pump and PumpSwap subscriptions shares one selected-signature claim
for the full freshness horizon. Each selected signature receives at most one HTTP attempt,
and only if it remains fresh when global pacing and concurrency admission
allow the request to start. A discovery that ages out while waiting is skipped
without HTTP. A provider rate-limit response (HTTP/JSON-RPC `429` or JSON-RPC
`-32005`) delays later, distinct signatures through the shared cooldown but
never retries the failed signature. The configured commitment is used for
PubSub and authoritative discovery transaction retrieval.

One provider rate-limit response marks the aggregate stream `DEGRADED`
immediately. Three
consecutive other one-shot discovery-read failures do the same. A later
successful discovery read clears that transport condition when both PubSub
sources are ready.

The active collection policy is deliberately live-first. A failed HTTP read is
skipped, a disconnected PubSub source reconnects at the current head, and the
server does not run `getSignaturesForAddress` backfill. Recovery utilities and
tables remain reserved for a future explicitly selected completeness mode;
old checkpoints are not consumed by this collector.

Stopping and starting the stream, or restarting the Rust process, recreates the
in-memory signature claims. Neither action intentionally retries or backfills
transactions, but a fresh notification delivered again afterward can be
treated as a new live intake attempt.

Do not place a real password or RPC credential in this document or a committed
`.env` file. A committed `.env.example` may contain names and clearly fake
local examples only.

## Program identity

Pump, PumpSwap, and Raydium program IDs are security-relevant network
identities. Verified values belong in reviewed, network-specific Rust
configuration with tests; they are not arbitrary user-entered venue URLs.
Raydium support initially covers post-Pump enrichment for explicitly supported
CPMM, CLMM, and AMM v4 programs.

Changing a program identity or decoder version must be visible in provenance
and review. A config change must not make previously decoded evidence appear to
have used the new version.

## Storage and channel limits

Transaction queues, paced and concurrent RPC work, discovery age, active
windows, browser snapshots, retention batches, timeouts, and reconnect timing
are bounded through the variables above. Terminal raw observation/work history
and replaceable projection events are pruned without deleting current
discovery/activity projections, confirmed discovery windows, immutable feature
snapshots and assessments, reserved checkpoints, pool identities, or
collection-gap records. Pending or leased work and observations belonging to
an active window are never pruned.

If `pg_database_size` reaches `DATABASE_MAX_BYTES`, collection fails closed.
Deleting rows lets PostgreSQL reuse space but does not necessarily reduce its
physical size immediately; raising the limit or database maintenance is an
explicit operator action, not a silent automatic override.

This database-size guard is not a whole-machine disk monitor. It does not
measure free filesystem space, PostgreSQL WAL, other databases, Docker disk
images, or frontend/Rust build caches. Keep `DATABASE_MAX_BYTES` comfortably
below the machine's remaining capacity and monitor local free space separately.
Current discovery-token, market, activity, checkpoint, pool,
qualification-summary, and collection-gap projections are retained so later
facts can be resolved. Confirmed window identities, feature snapshots,
assessments, and rule results are also retained. Their lifecycle still needs an
archive/expiry policy before unattended, high-volume, long-running use; until
then, sustained collection grows PostgreSQL and eventually reaches the
configured fail-closed database-size limit.

HTTP discovery snapshots return the latest bounded candidates plus
`tokens_total` and `tokens_truncated`, so the browser never implies that a
partial response is the complete feed.

Tokio channels remain ephemeral regardless of their configured capacity.
Increasing a queue never replaces the persist-before-dispatch rule.

`SOLDISCO_TEST_DATABASE_URL` is test-only. GitHub CI supplies a disposable
PostgreSQL service so the migration and persistence integration test runs
rather than skipping. It is not used by the application process.

## Optional advisory AI

AI remains a later, optional integration:

```text
AI_API_KEY
AI_MODEL
```

Collection, deterministic processing, projections, and truthful component
health must continue when AI is unconfigured or unavailable. AI configuration
never grants approval or execution authority.

## Future execution names

```text
EXECUTION_MODE
EXECUTION_POLICY_ID
```

No environment variable grants signing authority. Interactive live execution
will still require explicit reviewed intent and browser-wallet signing.
Automated execution requires a separately approved authorization design rather
than an `EXECUTION_ENABLED` shortcut.

## Not selected

`NATS_URL`, cloud-database settings, container-hosting settings, and remote
telemetry exporters are not part of the initial runtime. The modular monolith
uses bounded in-process Tokio channels and local PostgreSQL. New infrastructure
configuration is added only after a measured requirement and an explicit
architecture decision.
