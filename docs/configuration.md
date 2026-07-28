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

## Deferred intake variables

```text
SOLANA_NETWORK
SOLANA_RPC_HTTP_URL
SOLANA_RPC_WS_URL
SOLANA_COMMITMENT
```

The Pump intake milestone will implement and validate these. `SOLANA_NETWORK`
will identify the cluster explicitly rather than inferring it from an endpoint;
the HTTP URL will support reads and recovery, the WebSocket URL will support
live PubSub, and commitment will remain explicit.

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

Collector queues, RPC concurrency, recovery batch size, and retention limits
must be bounded. Exact names are added only with their implementations, but the
configuration model must support:

- maximum in-flight RPC work
- bounded per-stage Tokio channel capacity
- retry and reconnect limits
- recovery page or batch size
- SSE heartbeat and client buffer limits
- local storage warning and retention thresholds

Tokio channels remain ephemeral regardless of their configured capacity.
Increasing a queue never replaces the persist-before-dispatch rule.

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
