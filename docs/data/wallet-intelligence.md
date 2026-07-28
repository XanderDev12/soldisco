# Wallet intelligence

Wallet-conditioned strategies require historical, versioned evidence rather
than a permanent `trusted` boolean.

Wallet intelligence is not part of the initial Pump/PumpSwap collector or
deterministic first pass. The collector may preserve wallet addresses that are
already present in relevant chain events, but it does not label those wallets
as trusted.

## Planned records

- observed wallet trade
- wallet profile snapshot
- typed wallet relationship observation
- wallet cluster snapshot
- wallet score snapshot

A cluster snapshot records a stable cluster ID, version, effective time,
as-of slot, member wallets, typed edge evidence, confidence, and algorithm
version. Discovering a relationship later must not alter an earlier replay.

## Wallet scores

A wallet score is specific to:

- strategy and score version
- observation and evaluation window
- sample size
- historical lead time
- gross and executable performance after costs
- calculation time and observation cutoff

Scores expire and can be unavailable, stale, or invalid. They are not permanent
reputation labels.

## Relationship controls

Clustering must account for common funding sources, exchanges, routers,
program-owned accounts, copy-trading, and other shared hubs that can create
false relationships. Strategy features distinguish independent qualifying
clusters from related-wallet volume.

Wallet lists, labels, graph databases, and clustering algorithms are not part
of the initial Pump/PumpSwap, Solana RPC, or Raydium-enrichment first pass. They
must be designed before enabling Wallet-Conditioned Momentum.

Venue identity remains part of every observed wallet trade. Activity through a
Pump bonding curve, PumpSwap pool, Raydium CPMM pool, Raydium CLMM pool, or
Raydium AMM v4 pool cannot be treated as equivalent without a versioned
feature definition. Wallet scores use only evidence available by their as-of
slot, regardless of when a later pool or wallet relationship is discovered.

The first implementation stores these records in local PostgreSQL through the
Rust persistence boundary. Browser state and SSE delivery are not historical
wallet evidence.
