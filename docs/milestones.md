# Milestones

## 1. UI skeleton — complete

- Implement the stream-first console with honest disconnected states.
- Show only first-pass-approved candidates in the discovery feed.
- Summarize pending and rejected candidates through truthful counters and a
  compact rejection-reason log.
- Display risk, rating, strategy match, momentum, and data freshness.
- Keep token and position trackers empty until authoritative sources connect.
- Add persistent, user-adjustable workspace regions.
- Keep one Discovery destination and one Positions destination; avoid separate
  first-pass, watchlist, or duplicate positions surfaces.
- Add UI-only start and stop controls while keeping discovery disconnected.
- Keep wallet and execution controls explicitly disabled.

## 2. Architecture and reproducibility contracts — current

- Record permanent authority, safety, and fail-closed invariants.
- Define source provenance, event/receipt time, chain coordinates, market
  identity, deduplication, correction, finality, and raw observation rules.
- Define live-observation and cursor-based backfill boundaries.
- Define candidate-window and immutable feature-snapshot contracts.
- Treat the global pipeline stage as a derived UI projection over independent
  component statuses.
- Add immutable strategy-evaluation identity and snapshot references.
- State explicitly that the initial backend is a modular monolith.
- Feed the web application through a typed, read-only stream boundary.
- Add isolated test records for healthy, risky, incomplete, and failed candidates
  without shipping them in the product UI.

## 3. Source intake and raw history

- Add an Axiom adapter behind a source interface.
- Preserve source filter versions, match reasons, triggering wallets/groups,
  cursors, source IDs, and raw evidence references.
- Record rejected candidates, duplicate observations, missing data, and source
  outages.
- Support live intake and deterministic historical recovery.

## 4. Deterministic first pass

- Add provider-neutral Solana RPC access.
- Implement a minimal, explainable set of scam and rug filters.
- Preserve reason codes, rule versions, evidence, and freshness.
- Identify exact network, mint, pool or market, venue, and migration state where
  the check depends on market data.

## 5. Candidate monitoring and advisory analysis

- Open, update, expire, and invalidate time-bounded candidate windows.
- Produce versioned market-feature snapshots across explicit windows.
- Project independent discovery, check, risk, feature, AI, and strategy states.
- Run bounded AI analysis asynchronously and advisory-only.

## 6. Wallet intelligence and strategies

- Define a declarative, versioned strategy manifest.
- Add upload validation, replay, activation, and toggles.
- Define time-versioned wallet profiles, scores, typed relationships, and
  cluster snapshots with anti-lookahead rules.
- Implement Wallet-Conditioned Momentum only after those inputs are available.
- Evaluate active candidate windows repeatedly from immutable snapshots.

## 7. Replay, paper trading, and portfolio projections

- Rebuild projections and strategy decisions from as-of information.
- Simulate processing latency, fees, priority fees, slippage, price impact,
  failures, sellability, and exit constraints.
- Produce trade proposals without signing or submission.
- Reconcile simulated fills into positions and PnL.
- Add exposure and loss-limit controls.

## 8. Interactive live execution

- Add non-custodial browser-wallet signing.
- Verify and simulate transactions before presenting them for signature.
- Add execution policies, audit events, confirmation, and reconciliation.
- Require explicit user review for every transaction.

## 9. Automated execution — separately approved future scope

- Design bounded, revocable, auditable signing authority without backend seed
  phrases or raw exportable private keys.
- Add capital, strategy, venue, rate, exposure, and loss limits.
- Add independent kill switches and authorization-expiry behavior.
- Do not enable this milestone by extending an interactive-mode flag.
