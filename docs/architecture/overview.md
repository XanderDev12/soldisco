# Architecture overview

Soldisco is organized around a stream-first discovery pipeline with hard boundaries between observation, deterministic analysis, advisory AI, and future execution.

## Planned flow

1. A discovery adapter receives candidate tokens.
2. The engine normalizes and publishes every candidate to the live stream.
3. Deterministic first-pass checks assign an approval state and reason codes.
4. Risk and rating modules add explainable values with freshness metadata.
5. Enabled strategies evaluate approved candidates independently.
6. The API projects updates to the web application.
7. Future execution services turn explicit user intent into simulated, wallet-signed transactions.

The initial UI renders disconnected, empty trackers. Synthetic records belong
only in isolated tests and must not appear as live product state.

## Frontend boundaries

The web dashboard is organized by visible product responsibility. The
orchestration component owns shared state, while each sidebar view,
token-inspector tab, shell region, and substantial stream section lives in a
focused module. Shared primitives are reused without combining independent
screens into a single file. Regression tests verify the expected view files and
keep the orchestration layer within a small line-count budget.

## Boundaries

- The web app renders state and collects explicit user intent.
- The API owns browser-facing contracts, not discovery or scoring rules.
- The engine owns deterministic checks, risk, ratings, and strategy evaluation.
- The AI worker is advisory and cannot approve tokens, change deterministic risk, or authorize trades.
- Execution is isolated from discovery and strategy evaluation.
- Private keys and seed phrases must never enter the backend, logs, configuration, or AI context.

## Deferred integrations

- Axiom discovery and authentication
- Solana RPC providers and fallback policy
- Scam and rug-check implementations
- Event bus and database selection
- Wallet adapters and transaction signing
- Jupiter, Pump, or other execution venues
- Confirmation, reconciliation, holdings, and PnL calculations

These decisions should be recorded before implementation rather than embedded directly in UI components.
