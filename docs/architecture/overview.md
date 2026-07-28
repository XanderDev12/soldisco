# Architecture overview

Soldisco is organized around a stream-first discovery pipeline with hard
boundaries between on-chain observation, deterministic analysis, advisory AI,
strategy evaluation, projection, and future execution.

## Selected local runtime

The existing React/TypeScript interface runs on `localhost:3000`. After the UI
integration milestone, it will send finite commands and snapshot requests to
one Rust/Axum process on `127.0.0.1:8080` and receive projection-change
notifications through SSE. The Rust foundation alone connects to local
PostgreSQL on `127.0.0.1:5432` through SQLx. Its Solana HTTP and WebSocket RPC
connections begin with the Pump intake milestone.

The separately hosted Sites build remains a disconnected UI preview. No
always-on or cloud backend is part of the current architecture.

## Planned flow

1. Solana PubSub emits relevant Pump or PumpSwap program activity.
2. The collector retrieves enough transaction/account data to identify and
   normalize the fact.
3. The persistence boundary records decoder provenance, Solana coordinates,
   exact market identity, event time, receipt time, and durable work state.
4. Only after commit does the server publish a bounded Tokio message for
   downstream work.
5. The discovery engine deduplicates facts and updates rolling market metrics.
6. Cheap qualification limits deeper RPC work.
7. Provider-neutral RPC readers supply deterministic risk evidence.
8. Supported Raydium CPMM, CLMM, or AMM v4 pools may add post-Pump,
   venue-specific evidence for the same mint.
9. Risk and opportunity modules add explainable, versioned results with
   freshness metadata.
10. Approved candidates enter time-bounded monitoring windows.
11. Market, wallet-score, and wallet-cluster snapshots later produce immutable
    strategy inputs.
12. Enabled strategies evaluate candidates repeatedly and independently.
13. Advisory AI may attach asynchronous commentary without blocking the flow.
14. Rebuildable projections publish approved tokens, counters, rejection
    summaries, and component health to the browser over HTTP/SSE.
15. Future paper and interactive-live execution consume explicit trade
    proposals through a separate policy boundary.

WebSocket live delivery is not assumed to be complete. Saved PostgreSQL
checkpoints and Solana HTTP RPC recover missed transactions after a disconnect
or process restart. Live and recovered facts share the same identity and
deduplication rules.

The initial UI renders disconnected, empty trackers. Synthetic records belong
only in isolated tests and must not appear as live product state.

## State model

The global pipeline stage is a UI convenience, not an authoritative workflow
state. Discovery, enrichment, first pass, risk, market-feature freshness,
wallet-feature freshness, AI, each strategy, and candidate-window status advance
independently. A slow or failed component must not erase otherwise valid state.

Every projection states whether data is available, missing, stale, invalid, or
in error. Rejected candidates, missing observations, and source outages remain
part of the historical record.

Raydium enrichment is independently available, missing, stale, invalid, or in
error for each exact pool. It does not overwrite PumpSwap evidence, and the
absence of a Raydium pool does not imply rejection unless a versioned rule
requires it.

## Frontend boundaries

The web dashboard is organized by visible product responsibility. The
orchestration component owns shared state, while each sidebar view,
token-inspector tab, shell region, and substantial stream section lives in a
focused module. Shared primitives are reused without combining independent
screens into a single file. Regression tests verify the expected view files and
keep the orchestration layer within a small line-count budget.

## Boundaries

- The web app renders state and collects explicit user intent.
- Axum routes own browser transport, not discovery or scoring rules.
- `api-contracts` owns browser-facing message shapes.
- `source-pump` decodes Pump and PumpSwap facts; it does not score or trade.
- `source-raydium` resolves and decodes supported venue facts; it does not
  approve a token or combine pools without explicit rules.
- `solana-rpc` owns provider-neutral transport, recovery, and health.
- `discovery-engine` owns rolling metrics and cheap qualification.
- `risk-engine` owns deterministic checks, risk, and opportunity ratings.
- `persistence` is the only SQLx/PostgreSQL implementation boundary.
- `projections` creates rebuildable UI read models.
- The AI job is advisory and cannot approve tokens, change deterministic risk,
  or authorize trades.
- Execution is isolated from discovery and strategy evaluation.
- Private keys and seed phrases must never enter the backend, logs,
  configuration, persistence, or AI context.

## Process model

Logical ownership boundaries do not require separate network services. The
first operating backend composes Axum routes, Pump/PumpSwap intake, recovery,
discovery metrics, deterministic risk, optional Raydium enrichment,
projections, retention, and observability in one Rust process.

Workers are supervised Tokio tasks connected by bounded in-process channels.
Channels are ephemeral and provide backpressure; they never replace PostgreSQL
durability. Observations and work state are committed before dispatch, and
workers process idempotently from durable identities.

Advisory AI, strategy, and paper portfolio work may later run as additional
jobs in the same modular monolith. Execution may be separated earlier because
its authorization and audit requirements are materially different.

An event bus, graph database, or service mesh is not required for the research
milestones. They may be introduced only when measured operational needs justify
them.

See [backend stack](backend-stack.md) for the exact repository and local-runtime
decision.

## Selected but not yet implemented

- Direct Pump and PumpSwap decoding and recovery
- Solana RPC endpoint selection, capacity, and fallback policy
- Retention enforcement for the implemented local PostgreSQL foundation
- Scam and rug-check rule implementations
- Supported Raydium CPMM, CLMM, and AMM v4 enrichment
- Frontend consumption of the implemented Rust HTTP/SSE boundary

## Deferred capabilities

- Arbitrary Raydium-only token discovery
- Wallet scoring and time-versioned clustering
- Wallet adapters and transaction signing
- Quote routing and execution venues
- Confirmation, reconciliation, holdings, and PnL calculations
- X or other broad social ingestion
- Cloud deployment and remote authentication

These decisions must be recorded before implementation rather than embedded
directly in UI components.

See [invariants](invariants.md), [data lifecycle](data-lifecycle.md), and the
[execution model](execution-model.md) for the rules behind this overview.
