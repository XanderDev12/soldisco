# Soldisco

Soldisco is a local-first Solana token discovery and strategy-research
workspace. The existing React/TypeScript interface remains the browser layer.
A single Rust application now collects Pump and PumpSwap activity directly
from Solana, persists normalized evidence in local PostgreSQL, and publishes a
read-only discovery projection to the interface. Bounded-window activity
qualification is implemented; deterministic scam/rug screening, Raydium
enrichment, strategies, AI analysis, and trading remain later milestones.

## Selected local topology

```text
React UI                    Rust/Axum server                 PostgreSQL
localhost:3000 --HTTP/SSE-> 127.0.0.1:8080 --SQLx only----> 127.0.0.1:5432
                                  |
                                  +--HTTP/WebSocket RPC----> Solana
```

The diagram shows the defaults. The browser host and API path remain fixed at
`127.0.0.1` and `/api/v1`; its versioned local port preference defaults to
`8080` and must match the restarted server's `API_PORT`.

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
   downstream processing. Chain identity deduplicates accepted live data.
   Compatible replay keeps the canonical receipt (the earliest compatible
   receipt for ordinary facts, while an existing window opener stays frozen)
   and canonical direct-event evidence; reuse of the identity with different
   immutable evidence fails closed. The database is immutably bound to the
   configured Solana network before ingestion.
5. A fresh discovery log immediately opens a short provisional in-memory
   observation window so initial trades cannot race past HTTP corroboration.
   Successful normalization atomically creates its durable PostgreSQL window,
   pins the Prefilter and Qualification Defaults revisions, and records exact
   receipt-time membership for matching Pump mint or PumpSwap pool facts.
   Failed discovery cancels the provisional window. This milestone remains
   live-first: reconnects resume at the current head without backfill.
6. Window activity is counted once from each canonical direct Pump/PumpSwap
   event. The pinned programs' silent event self-CPI is accepted only when it
   immediately pairs 1:1 with that direct event inside the same top-level
   instruction; it corroborates rather than duplicates the event. CPI-only,
   out-of-order, mismatched, malformed, truncated, or unbalanced same-source
   logs make affected windows incomplete without an activity `getTransaction`
   call.
7. When the non-extending interval closes, a complete finalization waits for
   source progress through the close boundary, every admitted batch to settle,
   and same-window projection work to finish. Stop/disconnect cancellation and
   finalization claims are serialized: cancellation first produces `UNKNOWN`;
   an already eligible claim first freezes its prior completeness. The server
   then freezes versioned trades, buy/sell volume, unique-wallet,
   wallet-concentration, price, reserve, and lifecycle evidence. Versioned
   rules produce `PASS`, `REJECT`, or `UNKNOWN` before the result, rule
   evidence, counters, and projection commit together.
8. The default `QUALIFIED_ONLY` discovery feed shows current candidates whose
   complete window passed this inexpensive activity-quality gate. HTTP supplies
   bounded authoritative snapshots with explicit truncation metadata;
   coalesced named `soldisco` SSE events tell the browser when to refresh.
9. Attributable malformed or unresolved-market evidence that reaches the
   authoritative decoder enters bounded quarantine. Maintenance prunes
   eligible terminal raw history in bounded batches, while storage-limit and
   network-identity failures enter terminal `ERROR`.

`QUALIFIED` means only that a complete, exact-market observation window met the
configured activity thresholds. It is not scam clearance, a safety or ROI
approval, a recommendation, a strategy match, or permission to trade.
`REJECT` means the inexpensive qualification rules were not met; `UNKNOWN`
means the window or required evidence was incomplete. Risk and opportunity
scores remain unavailable. Diagnostic `OBSERVE_ALL` mode still exists for
pipeline validation, where structurally valid candidates remain `OBSERVED`.

This local milestone also has an explicit operating boundary: the 5 GiB
database guard is not a whole-machine free-space/WAL monitor. Durable windows,
feature snapshots, assessments, and current market/discovery aggregates are
not yet archived, so continuous mainnet collection grows PostgreSQL until the
guard stops intake. Keep disk headroom available and do not treat the current
local runtime as an unattended, indefinite deployment.

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
- `crates/discovery-engine` — bounded observation windows, immutable metrics,
  and versioned activity qualification
- `crates/risk-engine` — deterministic evidence, rules, and scoring
- `crates/persistence` — the only SQLx and PostgreSQL implementation boundary
- `crates/projections` — rebuildable browser read-model boundary
- `crates/api-contracts` — Rust-owned browser response and event shapes
- `docs` — product, architecture, configuration, and milestone decisions

These crates compile into one server binary. Background workers are supervised
Tokio tasks inside that process, not microservices. Their queues, provisional
window tokens, and signals are ephemeral performance tools; PostgreSQL owns
confirmed window identity, admitted observations, snapshots, assessments, and
unfinished work. The database commit happens before downstream work or UI
publication.

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
The default feed shows only real `QUALIFIED` candidates produced by the
backend; it does not synthesize data or manufacture unavailable risk and rating
values. Prefilter and Qualification Defaults persist in PostgreSQL across
browser, server, and computer restarts unless the local database is
deliberately deleted. The requested Start/Stop intent is PostgreSQL state too,
so the Rust server can restore a requested-running stream after restart.
The local API port, execution-mode presentation, and adjustable
sidebar/inspector widths are safe browser-local preferences stored in
`localStorage`. The port only selects
`http://127.0.0.1:<port>/api/v1`; it must match the backend `API_PORT` and does
not rebind the server. None of these preferences grants wallet or execution
authority. Unsaved settings drafts, order drafts, current navigation,
selection, and modal state remain transient. The remaining workspace views are
UI shells for later milestones. Paper views have no wallet dependency; wallet
controls belong only to future Live mode.

Deterministic scam/rug screening, safety approval, Raydium enrichment, strategy
configuration, wallet connections, quotes, purchases, sales, and position
reconciliation are not implemented.

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
If `API_PORT` is changed, restart the Rust server and set the browser's local
API port preference to the same value.

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
