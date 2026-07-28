# Wallet-Conditioned Momentum

Inactive definition for the first strategy. Trusted-wallet discovery is
intentionally out of scope, so this strategy returns `NOT_EVALUABLE` and cannot
emit trade instructions until its required dataset is configured.

Its manifest and evaluator are draft scaffolding, not a validated live
strategy. Before activation they must consume immutable market and wallet
snapshots, carry evaluation identity, pass deterministic replay, and satisfy the
shared declarative schema.

See the full [strategy design](../../../docs/strategies/wallet-conditioned-momentum.md)
and [wallet-intelligence model](../../../docs/data/wallet-intelligence.md).
