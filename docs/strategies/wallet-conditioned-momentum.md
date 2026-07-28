# Wallet-Conditioned Momentum

Wallet-Conditioned Momentum is the first planned strategy. It remains inactive
until wallet intelligence and market-feature snapshots are available and
replayable.

## Intended flow

1. activity from a qualifying wallet or time-versioned cluster opens a
   candidate window
2. deterministic market observations measure whether independent demand and
   liquidity confirm the move
3. repeated strategy evaluations reference immutable feature snapshots
4. an explainable match may create a paper-trade proposal
5. execution policy independently decides whether the proposal is admissible

## Likely feature families

- wallet score, sample size, and historical lead time
- triggering cluster ID/version and number of independent qualifying clusters
- related-wallet share of incoming volume
- net flow over multiple windows
- trade and unique-buyer acceleration
- buy/sell balance
- liquidity change and migration state
- price extension since the triggering wallet entered
- creator and initial-holder sell flow
- estimated entry impact, exit impact, transaction costs, and priority fees

Exact thresholds, windows, and exit rules remain research decisions. They must
not be embedded as unexplained constants in the UI or source adapter.

## AI relationship

An LLM may asynchronously summarize evidence and uncertainty, but its prose is
not a strategy input with execution authority. A future predictive model would
produce a structured, versioned estimate tied to a feature snapshot; the
deterministic strategy and execution policy would still apply the thresholds.

## Activation requirements

The strategy stays inactive until:

- its declarative manifest validates against the shared schema
- every evaluation has an immutable ID and snapshot references
- source provenance and replay semantics are implemented
- time-versioned wallet scores and clusters are available
- paper replays include realistic latency, costs, failures, and exits
- prospective paper results pass predefined acceptance criteria
