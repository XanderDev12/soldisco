# Wallet intelligence

Wallet-conditioned strategies require historical, versioned evidence rather
than a permanent `trusted` boolean.

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
of the initial Axiom and RPC first pass. They must be designed before enabling
Wallet-Conditioned Momentum.
