# Soldisco web

The Soldisco web app is the interface foundation for a stream-first Solana
token discovery workspace. It is built with Next-compatible Vinext, React,
TypeScript, and Tailwind's CSS toolchain.

## Current behavior

- Connects to the local Rust server through versioned HTTP commands and
  snapshots plus named `soldisco` SSE invalidations.
- Shows the default `QUALIFIED_ONLY` stream of real Pump/PumpSwap candidates
  whose complete bounded activity window passed its pinned qualification rules.
  Diagnostic `OBSERVE_ALL` remains available through the API.
- Shows separately scoped current-candidate, active-window, cumulative
  activity-qualification-result, processing-failure, and event-throughput
  counters plus qualification rejection summaries.
- Provides accessible views for discovery, positions, orders, alerts,
  strategies, replays, and local controls.
- Includes locally guarded start/stop controls derived from the supervised
  backend stream state, including degraded and transitional states.
- Separates deterministic first-pass status, risk value, discovery rating, and
  strategy match.
- Includes an inspector with overview, risk, signal, trade, and position views.
- Provides adjustable navigation and inspector regions with device-local layout
  persistence in browser `localStorage`.
- Uses the fixed browser endpoint shape
  `http://127.0.0.1:<port>/api/v1` and persists only its validated,
  browser-safe port in `localStorage`. The default is `8080`; the selected
  value is shared by every HTTP/SSE consumer and must match the restarted
  backend's `API_PORT`.
- Provides Prefilter and Qualification Defaults controls whose saved values and
  revisions are owned by PostgreSQL. Unsaved form text is transient.
- Keeps Paper and Live trading views distinct: Paper records simulated entry
  and exit prices without wallet controls, while Live alone exposes
  wallet-dependent actions. The selected presentation persists in
  `localStorage`, but grants no wallet, signing, or execution authority.
- Keeps order drafts, active navigation, token selection, inspector tabs, and
  modal state transient rather than presenting unfinished UI state as durable
  trading data.
- Never calls Solana or PostgreSQL directly. Raydium enrichment, deterministic
  screening, wallet connectivity, quotes, signing, and transaction submission
  are not connected yet.

## Run locally

Requires Node.js `>=24.18.0`. The repository `.nvmrc` pins the exact version
used by CI.

```bash
nvm use
npm install
npm run dev
```

Open [http://localhost:3000](http://localhost:3000).

The web build needs no public endpoint or origin environment variables. If the
Rust server is restarted on a non-default `API_PORT`, update the browser-local
API port preference; the UI never accepts a different host, path, or
credential-bearing URL.

## Verify

```bash
npm run lint
npm run typecheck
npm test
```

From the repository root, use `npm run verify` for the complete feature gate.
It checks linting, application and shared-package types, creates a production
build, verifies the server-rendered dashboard and its safety boundaries, and
runs the production dependency audit. The same command also checks Rust
formatting, Clippy warnings, tests, and a complete workspace build.

## UI module layout

The dashboard uses feature-oriented modules under `app/components/dashboard`:

- `DiscoveryDashboard.tsx` owns orchestration and shared UI state only.
- `Sidebar.tsx` and `DashboardTopbar.tsx` own shell regions.
- `TokenStreamView.tsx`, `TokenTable.tsx`, `DiscoveryCounters.tsx`,
  `RejectionLog.tsx`, and `TokenInspector.tsx` compose the discovery surface.
- `views/` contains one file for every sidebar destination.
- `inspector/` contains one file for every token-inspector tab.
- Shared types, navigation metadata, badges, layout behavior, tickets, and
  overlays remain in focused supporting modules. Mode-specific copy and
  requirements live in `executionModePresentation.ts`.

New tabs and substantial independent sections should be added as their own
modules rather than folded into the orchestration shell.

## Next milestone

Add the first versioned deterministic scam/rug safety gate and optional
post-Pump Raydium venue enrichment. `QUALIFIED` currently means activity quality
only; it is not safety clearance, a recommendation, or authorization to trade.
Wallet connectivity, quotes, signing, and transaction submission remain
separate later milestones.
