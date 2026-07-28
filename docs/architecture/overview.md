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
2. Solana PubSub emits Pump and PumpSwap program activity. Failed transactions,
   stale discoveries, and irrelevant events are discarded from their direct
   logs. Fresh token or pool creation immediately provisions a short activity
   window and, within the continuously running stream instance, can receive at
   most one globally deduplicated, paced, and concurrency-bounded HTTP transaction
   request. A discovery that ages out before admission is cancelled and skipped
   without HTTP; an attempted request failure cancels the window and skips that
   signature.
3. The current-IDL decoder strictly attributes the accepted discovery
   transaction's direct logs and Anchor CPI event instructions while keeping
   the direct program-data copy as the one canonical event identity.
4. The persistence boundary records decoder provenance, Solana coordinates,
   exact market identity, event time, receipt time, compact source evidence,
   and durable work state. Known malformed layouts go to quarantine.
5. Only after commit does the server wake idempotent downstream projection
   work; PostgreSQL remains the work source of truth.
6. The discovery worker projects every structurally valid candidate as
   `OBSERVED`. Successful normalization atomically confirms a durable,
   exact-market window with half-open receipt-time membership and pinned
   Prefilter and Qualification Defaults revisions.
7. Matching mint/pool activity, including activity received during the one-shot
   read, routes directly from PubSub into that window without activity HTTP.
   The immediately following silent event self-CPI is paired 1:1 inside the
   same instruction and corroborates rather than duplicates its canonical
   direct event. CPI-only, out-of-order, mismatched, malformed, truncated, or
   unbalanced source logs make same-source windows incomplete.
8. At close, a complete claim waits for source progress through the boundary,
   settlement of every admitted batch, and same-window projection work.
   Cancellation and finalization are ordered so cancellation first produces
   `UNKNOWN`, while a claim first freezes completeness for that attempt.
   Immutable versioned trade, flow, wallet, price, reserve, and lifecycle
   features are then frozen.
9. Versioned qualification rules commit a `PASS`, `REJECT`, or `UNKNOWN`
   assessment and evidence. A complete pass advances only the current
   exact-market candidate to `QUALIFIED`; interrupted or incomplete windows are
   `UNKNOWN`.
10. The default `QUALIFIED_ONLY` projection publishes current qualified
   candidates, separate counters, and qualification rejection summaries.
   Coalesced named SSE notifications tell the browser to refresh bounded,
   authoritative HTTP snapshots whose truncation metadata is explicit.
11. Future provider-neutral RPC readers supply deterministic risk evidence.
12. Supported Raydium CPMM, CLMM, or AMM v4 pools may later add post-Pump,
   venue-specific evidence for the same mint.
13. Risk and opportunity modules later add explainable, versioned results with
   freshness metadata.
14. Approved candidates later enter richer durable monitoring windows; these
    are distinct from the short implemented pre-decision activity window.
15. Market, wallet-score, and wallet-cluster snapshots later produce immutable
    strategy inputs.
16. Enabled strategies evaluate candidates repeatedly and independently.
17. Advisory AI may attach asynchronous commentary without blocking the flow.
18. Future risk, strategy, portfolio, and execution projections extend the
    current qualified feed without changing its historical evidence.
19. Future paper and interactive-live execution consume explicit trade
    proposals through a separate policy boundary.

WebSocket live delivery is explicitly not complete in the current live-first
mode. After a disconnect, stream restart, pipeline-attempt restart, or process
restart, each source resumes at the current head without HTTP backfill. The
ephemeral routing registry is recreated, but confirmed window identities and
committed members remain in PostgreSQL. Affected windows finalize with explicit
incomplete provenance and `UNKNOWN`, rather than treating missed activity as
zero. One-shot transaction failures are still skipped. A shared rate-limit
cooldown delays later distinct signatures but never retries the failed one.
Reserved checkpoint/recovery components are not consumed unless a future
explicit recovery mode is designed.

The current local UI renders real `QUALIFIED` candidates by default. Empty data
means no current candidate passed the gate, not that sample tokens were
substituted. Diagnostic `OBSERVE_ALL` can expose structurally valid `OBSERVED`
candidates for pipeline validation. Synthetic records belong only in isolated
tests and must not appear as live product state.

## State model

The global pipeline stage is a UI convenience, not an authoritative workflow
state. As enrichment, risk, market features, wallet features, AI, strategies,
and candidate windows are added, their statuses must advance independently. A
slow or failed component must not erase otherwise valid state.

Richer projections must state whether data is available, missing, stale,
invalid, or in error. Future rejected candidates, missing observations, and
source outages must remain part of the historical record.

Prefilter Defaults, Qualification Defaults, and requested-running stream intent
are PostgreSQL state, not browser-session state. Prefilter edits require a
stopped stream and apply at the next start. Qualification edits use optimistic
append-only revisions; each durable window pins one revision and complete
values, so later edits affect new windows only. Server startup restores a true
requested-running intent through a fresh supervised pipeline rather than
persisting runtime task state.

`OBSERVED`, `QUALIFIED`, and future `APPROVED` are distinct. `OBSERVED` means
structurally admitted. `QUALIFIED` means only that one complete exact-market
window met the global activity and concentration rules. It is not scam
clearance, safety approval, an ROI estimate, a recommendation, or strategy
evidence. Unsupported or malformed decoder inputs are not admitted as
candidates; known malformed evidence is quarantined for bounded audit instead
of blocking the stream. A structurally attributable fact whose market cannot
be resolved is also durably quarantined; automatic reprocessing of that
historical quarantine is future work.

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

The Paper/Live execution-mode presentation and adjustable sidebar/inspector
widths are validated browser-local preferences. They persist in
`localStorage` without granting wallet, signing, or execution authority.
Unsaved settings text, order drafts, active navigation, token selection, tabs,
and modals remain transient.

## Boundaries

- The web app renders state and collects explicit user intent.
- Axum routes own browser transport, not discovery or scoring rules.
- `api-contracts` owns browser-facing message shapes.
- `source-pump` decodes Pump and PumpSwap facts; it does not score or trade.
- `source-raydium` is the planned venue-fact boundary; it will not approve a
  token or combine pools without explicit rules.
- `solana-rpc` owns provider-neutral transport, health, and reserved recovery
  primitives.
- `discovery-engine` owns bounded observation windows, immutable feature
  snapshots, and strategy-neutral activity qualification.
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
first operating backend composes Axum routes, live-first Pump/PumpSwap intake,
structural discovery, durable bounded-window qualification, projection,
retention/storage maintenance, and observability in one Rust process.
Deterministic risk and Raydium enrichment remain modules in this same process
when implemented.

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
