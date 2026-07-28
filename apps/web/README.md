# Soldisco web

The Soldisco web app is the interface foundation for a stream-first Solana
token discovery workspace. It is built with Next-compatible Vinext, React,
TypeScript, and Tailwind's CSS toolchain.

## Current behavior

- Connects to the local Rust server through versioned HTTP commands and
  snapshots plus named `soldisco` SSE invalidations.
- Shows the current `OBSERVE_ALL` stream of structurally valid Pump/PumpSwap
  discoveries and their directly tracked venue activity. This is not yet an
  approved-only feed.
- Shows truthful observed, pending, approved, rejected, and flow-rate counters;
  deterministic rejection data remains empty until that engine exists.
- Provides accessible views for discovery, positions, orders, alerts,
  strategies, replays, and local controls.
- Includes locally guarded start/stop controls derived from the supervised
  backend stream state, including degraded and transitional states.
- Separates deterministic first-pass status, risk value, discovery rating, and
  strategy match.
- Includes an inspector with overview, risk, signal, trade, and position views.
- Provides adjustable navigation and inspector regions with device-local layout
  persistence.
- Keeps Paper and Live trading views distinct: Paper records simulated entry
  and exit prices without wallet controls, while Live alone exposes
  wallet-dependent actions. Neither mode enables execution yet.
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

Add rolling qualification and the first versioned deterministic safety gate to
the connected Pump/PumpSwap discovery slice. Raydium can then provide optional
post-Pump venue enrichment for mints with relevant pools. Wallet connectivity,
quotes, signing, and transaction submission remain separate later milestones.
