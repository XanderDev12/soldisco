# Architecture invariants

These rules apply regardless of provider, database, model, strategy, or
deployment topology.

## Authority and safety

- Deterministic checks and execution policies are authoritative.
- `OBSERVED` means only that supported evidence decoded and normalized
  structurally; it is never a safety pass, approval, recommendation, or trade
  signal.
- `OBSERVE_ALL` may expose pre-screening candidates for pipeline validation, but
  it cannot manufacture thresholds, decisions, or scores that have not run.
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
  represented for the duration required by their explicit retention policy;
  replay must disclose evidence that has aged out.

## Durability and recovery

- PostgreSQL is the initial durable system of record and is accessed only
  through the Rust persistence boundary.
- Each database is immutably bound to one configured Solana network before
  ingestion; HTTP genesis identity is pinned and verified before the first
  binding, and switching networks requires a separate database.
- An observation and its durable work state are committed before downstream
  in-process dispatch.
- Bounded Tokio channels provide backpressure but are ephemeral and never the
  only copy of unfinished work.
- Live WebSocket delivery is not presumed complete. HTTP RPC recovery resumes
  from persisted checkpoints and shares identity rules with live intake.
- Recovery that cannot reach its durable checkpoint records a retained,
  explicit collection gap and reports degraded health rather than implying
  completeness.
- Attributable malformed source evidence is quarantined and never admitted as
  an `OBSERVED` candidate.
- A handled transaction advances its checkpoint only after every attributable
  fact has a durable observation or quarantine disposition.
- Workers and projections are idempotent and rebuildable from durable facts.
- Retention is explicit and versioned. It cannot make missing replay evidence
  appear complete.

## Reproducibility

- Retained raw observations are never mutated in place. Corrections are new
  linked facts, while explicit retention may delete whole eligible terminal
  rows.
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
