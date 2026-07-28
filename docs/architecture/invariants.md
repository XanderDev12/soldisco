# Architecture invariants

These rules apply regardless of provider, database, model, strategy, or
deployment topology.

## Authority and safety

- Deterministic checks and execution policies are authoritative.
- AI output is advisory and cannot approve a token, change deterministic risk,
  create user intent, sign, or submit a transaction.
- A strategy match is evidence, not execution authority.
- Missing, stale, invalid, or contradictory required data fails closed.
- Seed phrases and raw exportable private keys never enter backend services,
  configuration, logs, telemetry, persistence, or AI context.

## Provenance and time

- Every score and decision references its source evidence, definition version,
  freshness, and calculation time.
- Source event time and Soldisco receipt time are separate fields.
- Solana-derived facts carry network and chain coordinates appropriate to the
  source.
- Token observations identify the exact market or pool when market-dependent
  facts are used; a mint alone is insufficient.
- Pump, PumpSwap, Raydium CPMM, Raydium CLMM, and Raydium AMM v4 evidence
  remains venue- and program-scoped. Values from different pools are not
  silently combined.
- Absence of a Raydium market is unavailable evidence, not an automatic
  rejection unless an identified, versioned rule requires it.
- Rejected candidates, source outages, missing data, and corrections are
  retained to prevent survivorship bias.

## Durability and recovery

- PostgreSQL is the initial durable system of record and is accessed only
  through the Rust persistence boundary.
- An observation and its durable work state are committed before downstream
  in-process dispatch.
- Bounded Tokio channels provide backpressure but are ephemeral and never the
  only copy of unfinished work.
- Live WebSocket delivery is not presumed complete. HTTP RPC recovery resumes
  from persisted checkpoints and shares identity rules with live intake.
- Workers and projections are idempotent and rebuildable from durable facts.
- Retention is explicit and versioned. It cannot make missing replay evidence
  appear complete.

## Reproducibility

- Raw observations are append-only. Corrections are new facts, not silent
  mutations.
- Feature, wallet-score, cluster, model, strategy, and policy versions are
  immutable once referenced by an evaluation.
- A replay uses only information observable at the simulated point in time.
  Later wallet relationships or labels cannot be applied retroactively.
- Replays include configurable processing latency, quote freshness, fees,
  slippage, price impact, failed transactions, and exit constraints.

## Boundaries

- The UI consumes projections and collects intent; it does not own domain truth.
- The local browser communicates with the Rust server through HTTP commands,
  snapshots, and SSE. It never connects directly to PostgreSQL.
- Source decoders expose observations; they do not contain strategy rules.
- RPC readers remain provider-neutral and read-only.
- Uploaded strategies are declarative data, never arbitrary executable code.
- The initial Rust backend is one modular monolith. Crate boundaries remain
  explicit even when they compile into one process.
- Raydium is post-Pump venue enrichment in the initial scope, not an
  unrestricted independent discovery feed.
- The hosted Sites UI remains disconnected from the local backend until a
  separately designed remote deployment exists.
- Discovery, risk, strategy, AI, projection, and portfolio components never
  possess signing authority.
