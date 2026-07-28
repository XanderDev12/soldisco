# Strategy engine (deferred TypeScript scaffold)

This package records the earlier TypeScript evaluator shape. Strategy execution
is deferred until the Pump/PumpSwap collector, optional Raydium enrichment,
deterministic screening, replay, and monitoring foundations are reliable.

Its future implementation belongs in a Rust `crates/strategy-engine` crate and
will compile into the same server binary initially. Evaluations must reference
immutable, versioned feature snapshots and can emit matches or proposals, never
orders. Do not add new backend implementation here.
