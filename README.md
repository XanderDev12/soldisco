# Soldisco

Soldisco is a stream-first Solana token discovery workspace. The first
milestone establishes the interface and typed system boundaries; the token,
strategy, and position trackers remain empty until their authoritative sources
are connected.

## Workspace

- `apps/web` — discovery console and future trading interface
- `apps/api` — browser-facing API and live stream boundary
- `apps/engine` — deterministic discovery pipeline boundary
- `apps/ai-worker` — advisory AI triage boundary
- `apps/execution-api` — future user-approved execution boundary
- `apps/confirmation-worker` — future transaction confirmation boundary
- `apps/portfolio-worker` — future holdings and PnL boundary
- `docs` — architecture, configuration, and milestone notes
- `infrastructure` — plans for local service dependencies

Each service is intentionally independent. No worker, strategy, or AI component may sign or submit transactions.

## Current scope

The discovery console and typed service-contract foundation are in place. The
UI includes empty stream filters, deterministic status/risk/rating surfaces,
strategy controls, token inspection, guarded buy/sell tickets, position
monitoring, and persistent adjustable panel sizes.

Axiom ingestion, Solana RPC access, scam/rug checks, persistent event
infrastructure, wallet connections, quotes, purchases, sales, and position
reconciliation have not been implemented.

## Run locally

```bash
cd apps/web
npm run dev
```

Run `npm run quality` before syncing every substantial feature. It performs
linting, application and package type checks, a production build, and the test
suite.

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

See [the architecture overview](docs/architecture/overview.md) and [milestones](docs/milestones.md) before adding integrations.
