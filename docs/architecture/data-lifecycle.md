# Data lifecycle and replay model

This document defines the information that must survive ingestion so later
analysis can be reproduced without lookahead or hidden source filtering.

## Source observations

Each normalized observation will eventually preserve:

- immutable observation ID and schema version
- source, source record ID, cursor or sequence, and raw evidence hash/reference
- source filter ID and version, match reasons, and the wallet or group that
  triggered the candidate when supplied
- Solana network, mint, exact pool or market, venue, and migration state when
  known
- source event time and Soldisco receipt time
- slot, block time, transaction signature, instruction/log coordinates,
  commitment, and finality when applicable

Provider capabilities differ, so unavailable coordinates are explicit rather
than invented. Every source filter remains visible because strategy performance
is conditional on the universe that filter produced.

## Live intake and recovery

The engine consumes a stream-like sequence of observations. A provider adapter
may implement that sequence by polling, WebSocket, local client, or another
mechanism.

Historical recovery is a separate cursor-based backfill capability. Live and
backfill observations use the same identity and normalization rules so
deterministic deduplication can merge them.

## Persistence and correction

Raw observations are append-only. A deterministic identity key prevents the
same upstream fact from being processed twice. Finality changes, provider
corrections, and normalization corrections create linked correction events;
they do not overwrite history.

Database and event-bus products are deliberately undecided. Ordinary relational
storage and in-process delivery are sufficient for the first operating
backend, provided raw facts and rebuildable projections remain separate.

## Candidate windows

An approved candidate can open a versioned monitoring window with:

- stable window ID, mint, market, opening trigger, and source provenance
- opened, observed-through, expiry, and closed timestamps
- explicit active, expired, completed, or invalid status
- references to the observations and snapshot versions available at each
  evaluation

Market and wallet updates can produce repeated evaluations during one window.
`STRATEGY_EVALUATED` is therefore not a terminal domain stage.

## Immutable snapshots

A strategy evaluation references immutable snapshot IDs rather than an
unversioned feature bag. Each feature records its definition version, window,
value, availability status, reason code, and observation cutoff.

Snapshot families include:

- market features: trades, volume, net flow, liquidity, unique buyers, holder
  behavior, price extension, and estimated entry/exit impact
- wallet intelligence: wallet scores, historical lead time, typed
  relationships, and time-versioned clusters
- optional structured model predictions, if a trained and calibrated model is
  introduced later

## Projections

Read models derive the token stream, inspector, strategies, alerts, orders, and
positions from recorded facts. Component statuses remain independent. The
single stage shown by the UI is derived from those statuses and can be rebuilt.

## Replay

A replay selects an as-of point and freezes:

- source and filter versions
- chain finality and information observable by that slot
- feature-engine, wallet-score, cluster, strategy, model, and policy versions
- simulated processing and execution latency
- quote age, slippage, fees, priority fees, price impact, failed transactions,
  sellability, and exit assumptions

Missing source data stays missing. Replays retain rejected candidates and
outages so results do not silently benefit from survivorship or selection bias.
