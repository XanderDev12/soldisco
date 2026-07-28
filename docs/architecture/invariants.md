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
- Rejected candidates, source outages, missing data, and corrections are
  retained to prevent survivorship bias.

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
- Provider adapters expose observations; they do not contain strategy rules.
- RPC readers remain provider-neutral and read-only.
- Uploaded strategies are declarative data, never arbitrary executable code.
- The initial backend is a modular monolith. Module boundaries remain explicit
  even when they share one process.
