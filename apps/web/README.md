# Soldisco web

The Soldisco web app is the interface foundation for a stream-first Solana
token discovery workspace. It is built with Next-compatible Vinext, React,
TypeScript, and Tailwind's CSS toolchain.

## Current behavior

- Starts with an empty, approved-only discovery feed until a source and
  deterministic screen are connected.
- Shows pending, approved, rejected, and flow-rate counters plus a compact
  rejection-reason log without placing failed candidates in the main feed.
- Provides accessible views for discovery, positions, orders, alerts,
  strategies, replays, and local controls.
- Includes a session-local start/stop control that never claims a discovery
  source is connected.
- Separates deterministic first-pass status, risk value, discovery rating, and
  strategy match.
- Includes an inspector with overview, risk, signal, trade, and position views.
- Provides adjustable navigation and inspector regions with device-local layout
  persistence.
- Keeps Paper and Live trading views distinct: Paper records simulated entry
  and exit prices without wallet controls, while Live alone exposes
  wallet-dependent actions. Neither mode enables execution yet.
- Does not yet call Axiom, an RPC provider, a wallet, a quote service, or a
  transaction service.

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
runs the production dependency audit.

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

Define the provenance, replay, candidate-window, and projection contracts, then
add a read-only Axiom candidate adapter and minimal deterministic RPC gate.
Wallet connectivity, quotes, signing, and transaction submission remain
separate later milestones.
