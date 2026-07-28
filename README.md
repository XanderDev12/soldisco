# Soldisco

Soldisco is a local-first Solana token discovery and strategy-research
workspace. The existing React/TypeScript interface remains the browser layer.
A single Rust application now collects Pump and PumpSwap activity directly
from Solana, persists normalized evidence in local PostgreSQL, and publishes a
read-only discovery projection to the interface. Deterministic screening,
Raydium enrichment, strategies, AI analysis, and trading are later milestones.

## Selected local topology

```text
React UI                    Rust/Axum server                 PostgreSQL
localhost:3000 --HTTP/SSE-> 127.0.0.1:8080 --SQLx only----> 127.0.0.1:5432
                                  |
                                  +--HTTP/WebSocket RPC----> Solana
```

There is no cloud backend in the current plan. The separately hosted Sites
build is a UI preview and remains disconnected from this local runtime. The
local React app is connected to the Rust API.

## Implemented local vertical slice

1. The UI starts or stops the supervised stream through locally guarded HTTP.
2. Start verifies the HTTP RPC's pinned Solana genesis identity before the
   database is bound or Pump/PumpSwap PubSub intake opens.
3. Successful PubSub notifications are decoded just far enough to identify
   fresh Pump token creation or PumpSwap pool creation. During one running
   server process, each selected signature can receive at most one globally
   deduplicated, paced, bounded-concurrent authoritative `getTransaction` call.
   A discovery that ages past its freshness deadline while waiting for
   admission is skipped before HTTP; failures, stale events, duplicate
   subscription delivery, and unrelated firehose traffic are also skipped
   without retrying the transaction or reconnecting a healthy subscription.
4. Normalized observations and durable work are committed to PostgreSQL before
   downstream processing. Chain identity deduplicates accepted live data, and
   the database is immutably bound to the configured Solana network before
   ingestion.
5. A fresh discovery log immediately opens a short provisional in-memory
   observation window so initial trades cannot race past HTTP corroboration.
   The window is confirmed only after the one-shot transaction normalizes
   successfully; otherwise it and its queued activity are cancelled. Matching
   Pump mint or PumpSwap pool activity is then routed directly from the
   existing program-log feeds and persisted without an HTTP fetch. This
   milestone is deliberately live-first: reconnects resume at the current head
   and do not backfill missed history.
6. The discovery worker projects structurally valid candidates and their
   venue-scoped activity. HTTP supplies bounded authoritative snapshots with
   explicit truncation metadata; coalesced named `soldisco` SSE events tell the
   browser when to refresh them.
7. Attributable malformed or unresolved-market evidence that reaches the
   authoritative decoder enters bounded quarantine. Maintenance prunes
   eligible terminal raw history in bounded batches, while storage-limit and
   network-identity failures enter terminal `ERROR`.

The current projection deliberately runs in `OBSERVE_ALL` mode. Every
structurally valid decoded Pump or PumpSwap candidate can appear as `OBSERVED`;
there are no pass thresholds, approvals, rejections, risk scores, or
opportunity scores yet. Unsupported data is ignored and attributable malformed
evidence is quarantined; neither is treated as a candidate. This makes the
connected pipeline observable without pretending that the future safety gate
exists.

This local milestone also has an explicit operating boundary: the 5 GiB
database guard is not a whole-machine free-space/WAL monitor, observation
windows are intentionally in-memory, and durable market/discovery aggregates
are not yet archived. Keep disk headroom available and do not treat
`OBSERVE_ALL` collection as an unattended, indefinite deployment.

## Workspace direction

- `apps/web` — implemented React/TypeScript discovery console
- `apps/server` — one Rust executable containing HTTP and background-job
  orchestration
- `crates/domain` — provider-independent domain facts and state transitions
- `crates/source-pump` — strict Pump and PumpSwap event decoding
- `crates/source-raydium` — optional post-Pump Raydium venue resolution and
  decoding
- `crates/solana-rpc` — provider-neutral Solana HTTP, PubSub, health, and
  reserved recovery behavior
- `crates/discovery-engine` — bounded observation windows, rolling metrics, and
  future cheap qualification
- `crates/risk-engine` — deterministic evidence, rules, and scoring
- `crates/persistence` — the only SQLx and PostgreSQL implementation boundary
- `crates/projections` — rebuildable browser read-model boundary
- `crates/api-contracts` — Rust-owned browser response and event shapes
- `docs` — product, architecture, configuration, and milestone decisions

These crates compile into one server binary. Background workers are supervised
Tokio tasks inside that process, not microservices. Their queues, active
windows, and signals are ephemeral performance tools; PostgreSQL is the durable
observation and work source. The database commit happens before downstream work
or UI publication.

Raydium remains a planned post-Pump venue-evidence layer, not a replacement for
Pump intake and not an excuse to combine liquidity or price across unrelated
pools. Future risk, strategy, AI, portfolio, paper-trading, and execution work
will be added only when those milestones begin. Execution remains separately
isolated because it has materially different security and authorization
consequences. No collector, risk, strategy, AI, projection, or portfolio
component may sign or submit transactions.

## Current scope

The discovery console is wired to authoritative local stream state, discovery
snapshots, token inspection, start/stop commands, and named SSE notifications.
The current feed shows only real `OBSERVED` candidates decoded by the backend;
it does not synthesize data or manufacture unavailable risk and rating values.
The remaining workspace views are UI shells for later milestones. Paper views
have no wallet dependency; wallet controls belong only to future Live mode.

Window-based qualification, deterministic scam/rug screening, approval and
rejection projections, Raydium enrichment, strategy configuration, wallet
connections, quotes, purchases, sales, and position reconciliation are not
implemented. The current observation window gathers activity only; it does not
make a decision.

## Run locally

Provide PostgreSQL 17 locally, either with the optional Compose setup or a
native installation, then create the one uncommitted local environment file:

```bash
cp .env.example .env
```

Replace both fake password values in `.env` with the same local-only password.
Start the database once. Use `npm run db:up` only for the optional Compose
database; otherwise start the native PostgreSQL service. Then run the server
and web commands in separate terminals from the repository root:

```bash
npm run dev:server
npm run dev:web
```

The three processes bind only to `127.0.0.1:5432`,
`127.0.0.1:8080`, and `localhost:3000` by default. Open
`http://localhost:3000`, then use Start Stream to begin collection.

The committed example uses Solana's public mainnet endpoints so initial setup
does not require a paid provider. Discovery reads default to one globally
paced request per second and a five-second shared cooldown after a provider
rate-limit response; the
rate-limited signature is still never retried. Public endpoints may also
restrict high-volume subscriptions, so configure dedicated matching HTTP and
WebSocket RPC URLs for sustained mainnet collection. RPC credentials belong
only in the ignored local `.env`.

The repository pins Node.js in `.nvmrc` because the frontend still uses Node.
With `nvm` installed, run `nvm use` before installing frontend dependencies.
The Rust toolchain is pinned by `rust-toolchain.toml`; Cargo installs it through
rustup on first use.

Run root-level `npm run verify` before syncing every substantial feature. It
checks Rust formatting, lints, type-checks, tests, and production-builds the
implemented web and Rust workspaces, then audits production frontend
dependencies. The web test command owns its one production build, so
verification does not build it twice. GitHub CI also supplies disposable
PostgreSQL so the migration/persistence integration test runs on every pull
request.

See the [documentation index](docs/README.md), [backend stack
decision](docs/architecture/backend-stack.md), [architecture
overview](docs/architecture/overview.md), and [milestones](docs/milestones.md)
before adding integrations.
