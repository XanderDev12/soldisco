# Milestones

## 1. UI skeleton — complete

- Implement the stream-first console with honest disconnected states.
- Show every incoming token and its pending, approved, rejected, or error state.
- Display risk, rating, strategy match, momentum, and data freshness.
- Keep token and position trackers empty until authoritative sources connect.
- Add persistent, user-adjustable workspace regions.
- Make every sidebar destination accessible with truthful empty states.
- Add UI-only start and stop controls while keeping discovery disconnected.
- Keep wallet and execution controls explicitly disabled.

## 2. Contracts and projection pipeline

- Define versioned token, check-result, score, and strategy-result contracts.
- Feed the web application through a typed, read-only stream boundary.
- Add isolated test records for healthy, risky, incomplete, and failed candidates
  without shipping them in the product UI.

## 3. Basic discovery and first-pass checks

- Add an Axiom adapter behind a source interface.
- Add provider-neutral Solana RPC access.
- Implement a minimal, explainable set of scam and rug filters.
- Preserve raw observations and reason codes for audit and replay.

## 4. Strategies

- Define a declarative, versioned strategy manifest.
- Add upload validation, replay, activation, and toggles.
- Implement wallet-conditioned momentum only after trusted-wallet data is designed.

## 5. Paper trading and portfolio projections

- Produce trade proposals without signing or submission.
- Reconcile simulated fills into positions and PnL.
- Add exposure and loss-limit controls.

## 6. Explicit live execution

- Add non-custodial browser-wallet signing.
- Verify and simulate transactions before presenting them for signature.
- Add execution policies, audit events, confirmation, and reconciliation.
- Keep automation disabled until separately designed and approved.
