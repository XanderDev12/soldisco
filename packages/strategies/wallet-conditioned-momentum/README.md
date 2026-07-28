# Wallet-Conditioned Momentum (deferred scaffold)

This is the first planned strategy, but its current TypeScript definition is
reference scaffolding. Trusted-wallet discovery is intentionally out of scope,
so it remains `NOT_EVALUABLE` and cannot emit trade proposals until the required
dataset is configured.

The eventual evaluator belongs in the Rust strategy engine. Before activation
it must consume immutable Pump/PumpSwap market snapshots, applicable Raydium
venue evidence, and wallet snapshots; carry evaluation identity; pass
deterministic replay; and satisfy the shared declarative schema.

See the full [strategy design](../../../docs/strategies/wallet-conditioned-momentum.md)
and [wallet-intelligence model](../../../docs/data/wallet-intelligence.md).
