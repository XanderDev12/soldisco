# Architecture overview

Soldisco is organized around a stream-first discovery pipeline with hard
boundaries between on-chain observation, deterministic analysis, advisory AI,
strategy evaluation, projection, and future execution.

## Selected local runtime

The existing React/TypeScript interface runs on `localhost:3000`. It sends
finite commands and snapshot requests to one Rust/Axum process on
`127.0.0.1:8080` and receives named `soldisco` projection-change notifications
through SSE. The Rust process alone connects to local PostgreSQL on
`127.0.0.1:5432` through SQLx and to configurable Solana HTTP and WebSocket RPC
endpoints.

The separately hosted Sites build remains a disconnected UI preview. No
always-on or cloud backend is part of the current architecture.

## Flow and implementation boundary

1. A requested stream start verifies the configured HTTP RPC's pinned Solana
   genesis identity before binding the database or opening intake.
2. Solana PubSub emits relevant Pump or PumpSwap program activity. Every
   success or failure notification is only a trigger: authoritative HTTP RPC
   must return the same signature, slot, and transaction status before the
   collector accepts it. Fetches are bounded and ordered-concurrent, retries
   are finite, and source receipt time is preserved.
3. The current-IDL decoder strictly attributes and decodes supported
   program-data logs and Anchor CPI event instructions.
4. The persistence boundary records decoder provenance, Solana coordinates,
   exact market identity, event time, receipt time, compact source evidence,
   and durable work state. Known malformed layouts go to quarantine.
5. Only after commit does the server wake idempotent downstream projection
   work; PostgreSQL remains the work source of truth.
6. The implemented discovery worker projects every structurally valid candidate
   as `OBSERVED` and updates venue-scoped cumulative activity.
7. Coalesced named SSE notifications tell the browser to refresh bounded,
   authoritative HTTP snapshots whose truncation metadata is explicit.
8. Future rolling metrics and cheap qualification limit deeper RPC work.
9. Future provider-neutral RPC readers supply deterministic risk evidence.
10. Supported Raydium CPMM, CLMM, or AMM v4 pools may later add post-Pump,
   venue-specific evidence for the same mint.
11. Risk and opportunity modules later add explainable, versioned results with
   freshness metadata.
12. Approved candidates later enter time-bounded monitoring windows.
13. Market, wallet-score, and wallet-cluster snapshots later produce immutable
    strategy inputs.
14. Enabled strategies evaluate candidates repeatedly and independently.
15. Advisory AI may attach asynchronous commentary without blocking the flow.
16. Rebuildable projections eventually publish approved tokens, counters,
    rejection summaries, and component health to the browser over HTTP/SSE.
17. Future paper and interactive-live execution consume explicit trade
    proposals through a separate policy boundary.

WebSocket live delivery is not assumed to be complete. Saved PostgreSQL
checkpoints and Solana HTTP RPC recover missed transactions after a disconnect
or process restart. Live and recovered facts share the same identity and
deduplication rules. A recovery bound or missing checkpoint history creates a
durable collection-gap record; collection resumes live but remains truthfully
`DEGRADED` until that gap is explicitly resolved.

The current local UI renders real `OBSERVED` discovery candidates when the
stream is running. Empty data means the durable projection is empty, not that
sample tokens were substituted. Synthetic records belong only in isolated
tests and must not appear as live product state.

## State model

The global pipeline stage is a UI convenience, not an authoritative workflow
state. As enrichment, risk, market features, wallet features, AI, strategies,
and candidate windows are added, their statuses must advance independently. A
slow or failed component must not erase otherwise valid state.

Richer projections must state whether data is available, missing, stale,
invalid, or in error. Future rejected candidates, missing observations, and
source outages must remain part of the historical record.

The current `OBSERVE_ALL` projection is intentionally pre-screening. It has no
thresholds and does not claim approval, rejection, risk, opportunity, or
strategy results. Unsupported or malformed decoder inputs are not admitted as
candidates; known malformed evidence is quarantined for bounded audit instead
of blocking the stream. A structurally attributable fact whose market cannot be
resolved is also durably quarantined before its checkpoint advances; automatic
reprocessing of that historical quarantine is future work.

When implemented, Raydium enrichment will be independently available, missing,
stale, invalid, or in error for each exact pool. It will not overwrite
PumpSwap evidence, and the absence of a Raydium pool will not imply rejection
unless a versioned rule requires it.

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
- `source-raydium` is the planned venue-fact boundary; it will not approve a
  token or combine pools without explicit rules.
- `solana-rpc` owns provider-neutral transport, recovery, and health.
- `discovery-engine` will own rolling metrics and cheap qualification when
  wired into the pipeline.
- `risk-engine` will own deterministic checks, risk, and opportunity ratings.
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
structural discovery projection, retention/storage maintenance, and
observability in one Rust process. Rolling qualification, deterministic risk,
and Raydium enrichment remain modules in this same process when implemented.

Workers are supervised Tokio tasks. High-volume paths use bounded in-process
channels for backpressure, while small control paths use focused Tokio
synchronization primitives. All in-process signaling is ephemeral and never
replaces PostgreSQL durability. Observations and work state are committed
before downstream notification, and workers process idempotently from durable
identities.

Advisory AI, strategy, and paper portfolio work may later run as additional
jobs in the same modular monolith. Execution may be separated earlier because
its authorization and audit requirements are materially different.

An event bus, graph database, or service mesh is not required for the research
milestones. They may be introduced only when measured operational needs justify
them.

See [backend stack](backend-stack.md) for the exact repository and local-runtime
decision.

## Selected but not yet implemented

- Dedicated-provider capacity and multi-endpoint RPC fallback policy
- Scam and rug-check rule implementations
- Supported Raydium CPMM, CLMM, and AMM v4 enrichment
- Rolling-window qualification and explicit approval/rejection projection
- Strategy configuration and evaluation

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
