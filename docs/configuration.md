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
SOLANA_LIVE_FETCH_MAX_ATTEMPTS
SOLANA_SUBSCRIPTION_IDLE_TIMEOUT_MS
COLLECTOR_QUEUE_CAPACITY
RECOVERY_PAGE_SIZE
RECOVERY_MAX_RECORDS
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
| `SOLANA_RPC_HTTP_URL` | Transaction reads and checkpoint recovery |
| `SOLANA_RPC_WS_URL` | Pump and PumpSwap program-log PubSub |
| `SOLANA_COMMITMENT` | Explicit `confirmed` or `finalized` read level; `getTransaction` cannot use `processed` |
| `SOLANA_REQUEST_TIMEOUT_MS` | Positive per-request/connect timeout |
| `SOLANA_RECONNECT_DELAY_MS` | Positive initial reconnect delay; retries back off |
| `SOLANA_RPC_MAX_IN_FLIGHT` | Ordered concurrent authoritative transaction fetches, from 1 through 128 |
| `SOLANA_LIVE_FETCH_MAX_ATTEMPTS` | Finite attempts for a notified transaction, from 1 through 20 |
| `SOLANA_SUBSCRIPTION_IDLE_TIMEOUT_MS` | Reconnect a silent/half-open PubSub subscription after this interval |
| `COLLECTOR_QUEUE_CAPACITY` | Positive bound for the in-process transaction queue |
| `RECOVERY_PAGE_SIZE` | Signatures per recovery page, from 1 through 1,000 |
| `RECOVERY_MAX_RECORDS` | Per-recovery safety cap, at least the page size and at most 100,000 |
| `STREAM_START_TIMEOUT_MS` | Maximum command wait for an initial stream result |

## Implemented browser variables

```text
NEXT_PUBLIC_SOLDISCO_API_URL
NEXT_PUBLIC_SOLDISCO_WEB_ORIGIN
```

The web scripts load the root ignored `.env`. The API URL must remain local,
and `NEXT_PUBLIC_SOLDISCO_WEB_ORIGIN` must exactly match both the page's origin
and server `WEB_ORIGIN`; the UI refuses to connect when they diverge. These
variables are intentionally browser-visible and must never contain credentials.
Start and stop commands also send the fixed local-control header
`X-Soldisco-Control: soldisco-local-ui-v1`; the Rust server rejects browser
origins other than `WEB_ORIGIN` and request authorities other than the
configured `API_HOST:API_PORT`.

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
begin. A later network switch against the same database fails closed. WebSocket
notifications are corroborated transaction-by-transaction through that
verified HTTP RPC because Solana PubSub has no equivalent genesis method. The
configured commitment is used for PubSub, authoritative transaction retrieval,
and recovery.

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

Transaction queues, concurrent RPC work, fetch attempts, recovery pages,
recovery records, snapshots, retention batches, timeouts, and reconnect timing
are bounded through the variables above. Terminal raw history and replaceable
projection events are pruned without deleting discovery-token/activity
aggregates, checkpoints, pool identities, or collection-gap records. Pending
or leased work is never pruned.

If `pg_database_size` reaches `DATABASE_MAX_BYTES`, collection fails closed.
Deleting rows lets PostgreSQL reuse space but does not necessarily reduce its
physical size immediately; raising the limit or database maintenance is an
explicit operator action, not a silent automatic override.

This database-size guard is not a whole-machine disk monitor. It does not
measure free filesystem space, PostgreSQL WAL, other databases, Docker disk
images, or frontend/Rust build caches. Keep `DATABASE_MAX_BYTES` comfortably
below the machine's remaining capacity and monitor local free space separately.
Current discovery-token, market, activity, checkpoint, pool,
rejection-summary, and collection-gap projections are retained so the
collector can resolve later facts. Their lifecycle and the in-memory
active-market working set must become bounded before unattended, high-volume,
long-running use.

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
