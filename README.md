# Soldisco

Soldisco is a stream-first Solana token discovery workspace. The first
milestone establishes the interface and typed system boundaries; the token,
strategy, and position trackers remain empty until their authoritative sources
are connected.

## Planned product flow

1. Axiom supplies live candidates and historical recovery batches.
2. Every observation is recorded with source provenance, Solana coordinates,
   market identity, and both event and receipt time.
3. Provider-neutral RPC checks perform the deterministic scam and rug first
   pass.
4. Approved candidates enter time-bounded monitoring windows.
5. Versioned market, wallet, and cluster snapshots feed enabled strategies.
6. Advisory AI adds asynchronous commentary without changing deterministic
   results or authorizing trades.
7. Strategy evaluations produce explainable matches and, later, paper-trade
   proposals.
8. Confirmed activity projects into orders, positions, PnL, alerts, and
   deterministic replays.

## Workspace

- `apps/web` — implemented discovery console and future trading interface
- `apps/api`, `apps/engine`, and worker directories — documented ownership
  boundaries for the future backend
- `apps/execution-api` — separately isolated execution boundary
- `packages` — framework-neutral contracts and domain modules
- `docs` — architecture, configuration, and milestone notes
- `infrastructure` — deferred infrastructure decision markers

The first backend will be a modular monolith: API, ingestion, deterministic
analysis, projections, paper portfolio, and background jobs will run in one
process while preserving their package boundaries. The folders above do not
commit the project to separately deployed services. Execution remains more
strongly isolated because it has different security consequences.

No ingestion, strategy, AI, confirmation, or portfolio component may sign or
submit transactions.

## Current scope

The discovery console and typed service-contract foundation are in place. The
UI includes an approved-only discovery feed, screening counters, a compact
rejection log, deterministic risk/rating surfaces, accessible views for every
workspace destination, session-local stream controls, strategy controls, token
inspection, guarded buy/sell tickets, position monitoring, and persistent
adjustable sidebar and inspector sizes. Paper views use a separate simulated
ledger presentation with no wallet controls; wallet-dependent actions appear
only in Live mode.

Axiom ingestion, Solana RPC access, scam/rug checks, persistent event
infrastructure, wallet connections, quotes, purchases, sales, and position
reconciliation have not been implemented.

The next milestone formalizes provenance, replay, independent component state,
candidate windows, and versioned feature snapshots before connecting Axiom.

## Run locally

```bash
cd apps/web
npm run dev
```

The repository pins Node.js in `.nvmrc`. With `nvm` installed, run `nvm use`
before installing dependencies.

Run root-level `npm run verify` before syncing every substantial feature. It
performs linting, application and package type checks, a production build, the
test suite, and the production dependency audit.

## Feature workflow

Substantial changes are developed on a focused branch and synced only after the
complete local gate passes:

```bash
git switch -c agent/<feature-name>
npm ci --prefix apps/web
npm run verify
```

`npm run verify` adds the production dependency audit to the quality checks.
GitHub runs the same command for every push and pull request. Dependencies are
committed through `apps/web/package-lock.json`; installed modules and generated
build output remain local and are ignored.

See the [documentation index](docs/README.md), [architecture
overview](docs/architecture/overview.md), and [milestones](docs/milestones.md)
before adding integrations.
