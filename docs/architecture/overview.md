# Architecture overview

Soldisco is organized around a stream-first discovery pipeline with hard
boundaries between observation, deterministic analysis, advisory AI, strategy
evaluation, projection, and future execution.

## Planned flow

1. Axiom live intake or backfill emits a source observation.
2. The backend records the raw observation, source filter provenance, Solana
   coordinates, and exact market identity.
3. The engine normalizes and deduplicates the candidate.
4. Provider-neutral RPC readers supply deterministic first-pass evidence.
5. Risk and rating modules add explainable, versioned results with freshness
   metadata.
6. Approved candidates enter time-bounded monitoring windows.
7. Market, wallet-score, and wallet-cluster snapshots produce immutable
   strategy inputs.
8. Enabled strategies evaluate candidates repeatedly and independently.
9. Advisory AI may attach asynchronous commentary without blocking the flow.
10. The API projects independent component state to the web application.
11. Future paper and interactive-live execution consume explicit trade
    proposals through a separate policy boundary.

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

## Deployment model

Logical ownership boundaries do not require separate network services. The
first operating backend will compose API, ingestion, deterministic engine,
projections, paper portfolio, confirmation jobs, and observability in one
process. Advisory AI may run asynchronously. Execution may be separated earlier
because its authorization and audit requirements are materially different.

An event bus, graph database, or service mesh is not required for the research
milestones. They may be introduced only when measured operational needs justify
them.

## Deferred integrations

- Axiom discovery and authentication
- Solana RPC providers and fallback policy
- Scam and rug-check implementations
- Event bus and database selection
- Wallet scoring and time-versioned clustering
- Wallet adapters and transaction signing
- Jupiter, Pump, or other execution venues
- Confirmation, reconciliation, holdings, and PnL calculations
- X or other broad social ingestion

These decisions should be recorded before implementation rather than embedded directly in UI components.

See [invariants](invariants.md), [data lifecycle](data-lifecycle.md), and the
[execution model](execution-model.md) for the rules behind this overview.
