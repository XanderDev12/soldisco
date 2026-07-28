# Soldisco web

The Soldisco web app is the interface foundation for a stream-first Solana
token discovery workspace. It is built with Next-compatible Vinext, React,
TypeScript, and Tailwind's CSS toolchain.

## Current behavior

- Starts with an empty, filterable token stream until discovery is connected.
- Provides accessible views for the token stream, initial approvals, watchlist,
  positions, orders, alerts, strategies, replays, and local controls.
- Includes a session-local start/stop control that never claims a discovery
  source is connected.
- Separates deterministic first-pass status, risk value, discovery rating, and
  strategy match.
- Includes an inspector with overview, risk, signal, trade, and position views.
- Provides adjustable navigation, inspector, and positions regions with
  device-local layout persistence.
- Models paper/live mode, wallet connection, strategy upload, buy/sell tickets,
  and position monitoring without enabling execution.
- Does not yet call Axiom, an RPC provider, a wallet, a quote service, or a
  transaction service.

## Run locally

Requires Node.js `>=22.13.0`.

```bash
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

Use `npm run quality` for the complete feature gate. It checks linting,
application and shared-package types, creates a production build, and verifies
the server-rendered dashboard and its safety boundaries.

## Next milestone

Add a read-only Axiom candidate adapter and a minimal deterministic gate behind
the service contracts in the repository root. Wallet connectivity, quotes,
signing, and transaction submission remain separate later milestones.
