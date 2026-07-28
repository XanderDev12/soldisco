# Soldisco

Soldisco is a local-first Solana token discovery and strategy-research
workspace. The existing React/TypeScript interface remains the browser layer.
A single Rust application will collect Pump and PumpSwap activity directly from
Solana, enrich eligible mints with exact Raydium venue data when available, run
deterministic checks, persist the evidence in local PostgreSQL, and publish
read-only projections to the interface.

## Selected local topology

```text
React UI                    Rust/Axum server                 PostgreSQL
localhost:3000 --HTTP/SSE-> 127.0.0.1:8080 --SQLx only----> 127.0.0.1:5432
                                  |
                                  +--HTTP/WebSocket RPC----> Solana
```

There is no cloud backend in the current plan. The separately hosted Sites
build is a UI preview and remains disconnected from this local runtime. The
diagram is the selected local target; the current web interface remains
disconnected from the implemented Rust/PostgreSQL foundation until the UI
integration milestone.

## Planned product flow

1. The collector subscribes to Pump and PumpSwap program activity and recovers
   missed chain history after interruptions.
2. Relevant chain observations are normalized, written to PostgreSQL, and only
   then dispatched to in-process workers.
3. The discovery engine maintains rolling trades, volume, unique-wallet,
   momentum, curve, and migration measurements.
4. Cheap qualification removes irrelevant activity before deeper RPC work.
5. The deterministic risk engine records explainable `PASS`, `REJECT`, or
   `UNKNOWN` results with evidence and freshness.
6. When a candidate has a Raydium market, a post-Pump enrichment layer resolves
   the exact CPMM, CLMM, or AMM v4 pool and evaluates that venue independently.
7. Approved candidates enter time-bounded monitoring windows and appear in the
   Discovery projection; rejected activity updates counters and reason logs.
8. Versioned market, wallet, and cluster snapshots later feed enabled
   strategies. Paper and live execution remain separate later milestones.

Raydium is an additional venue-evidence layer, not a replacement for Pump
intake and not an excuse to combine liquidity or price across unrelated pools.

## Workspace direction

- `apps/web` — implemented React/TypeScript discovery console
- `apps/server` — one Rust executable containing HTTP and background-job
  orchestration
- `crates/domain` — provider-independent domain facts and state transitions
- `crates/source-pump` — Pump and PumpSwap event decoding
- `crates/source-raydium` — optional post-Pump Raydium venue resolution and
  decoding
- `crates/solana-rpc` — provider-neutral Solana HTTP, PubSub, recovery, and
  health behavior
- `crates/discovery-engine` — rolling metrics and cheap qualification
- `crates/risk-engine` — deterministic evidence, rules, and scoring
- `crates/persistence` — the only SQLx and PostgreSQL implementation boundary
- `crates/projections` — approved feed, counters, rejection log, and inspector
  read models
- `crates/api-contracts` — Rust-owned browser response and event shapes
- `docs` — product, architecture, configuration, and milestone decisions

These crates compile into one server binary. Background workers are bounded
Tokio tasks inside that process, not microservices. Their channels are
ephemeral performance tools; PostgreSQL is the durable handoff and recovery
source. The database commit happens before an event is published to a worker or
the UI.

Future strategy, portfolio, paper-trading, and execution crates will be added
only when their milestones begin. Execution remains separately isolated because
it has materially different security and authorization consequences. No
collector, risk, strategy, AI, projection, or portfolio component may sign or
submit transactions.

## Current scope

The discovery console is in place. It includes an approved-only feed,
screening counters, a compact rejection log, accessible workspace views,
session-local stream controls, strategy controls, token inspection, trading
surfaces, position monitoring, and adjustable workspace regions. Paper views
have no wallet dependency; wallet controls belong only to Live mode.

The UI currently contains no authoritative token or position data. The Rust
workspace, local PostgreSQL migration, health/snapshot routes, honest
not-yet-available Start response, and notification-only SSE boundary are now in
place. Direct on-chain collection, deterministic screening, frontend API
integration, wallet connections, quotes, purchases, sales, and position
reconciliation remain implementation work.

## Run locally

Install Docker Desktop (or provide PostgreSQL 17 yourself), then create the one
uncommitted local environment file:

```bash
cp .env.example .env
```

Replace both fake password values in `.env` with the same local-only password.
Start the database once, then run the server and web commands in separate
terminals from the repository root:

```bash
npm run db:up
npm run dev:server
npm run dev:web
```

The three processes bind only to `127.0.0.1:5432`,
`127.0.0.1:8080`, and `localhost:3000` by default. The current web app remains
disconnected until the UI integration milestone, so the browser still shows
honest empty trackers.

The repository pins Node.js in `.nvmrc` because the frontend still uses Node.
With `nvm` installed, run `nvm use` before installing frontend dependencies.
The Rust toolchain is pinned by `rust-toolchain.toml`; Cargo installs it through
rustup on first use.

Run root-level `npm run verify` before syncing every substantial feature. It
formats, lints, type-checks, tests, builds, and audits the implemented web and
Rust workspaces.

See the [documentation index](docs/README.md), [backend stack
decision](docs/architecture/backend-stack.md), [architecture
overview](docs/architecture/overview.md), and [milestones](docs/milestones.md)
before adding integrations.
