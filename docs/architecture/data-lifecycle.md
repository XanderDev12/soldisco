# Data lifecycle and replay model

This document defines the information that must survive ingestion so later
analysis can be reproduced without lookahead or hidden source filtering.

## Source observations

Each normalized observation will eventually preserve:

- immutable observation ID and schema version
- source program and decoder identity/version
- raw evidence hash/reference and compact normalized decoder evidence
- Solana network, mint, exact pool or market, venue, and migration state when
  known
- source event time and Soldisco receipt time
- slot, block time, transaction signature, instruction/log coordinates,
  commitment, and finality when applicable

Pump, PumpSwap, and each supported Raydium program are distinct source-program
identities. Unavailable coordinates are explicit rather than invented. The
decoder version and qualification-rule version remain visible because later
performance is conditional on the observable universe and interpretation that
produced it.

## Live intake and recovery

The Pump/PumpSwap collector uses Solana WebSocket PubSub for low-latency live
activity. PubSub is not a completeness guarantee. The server persists an
observed-through checkpoint and uses Solana HTTP RPC to retrieve missed
transactions after disconnects, server downtime, or ambiguous delivery.

Live and recovered observations use the same chain identity and normalization
rules so deterministic deduplication can merge them. Recovery advances a
checkpoint only after the relevant facts and durable downstream work have been
committed.

## Persistence and correction

Raw observations are append-only. A deterministic identity key prevents the
same chain fact from being processed twice. Finality changes, chain
reorganizations, and decoder or normalization corrections create linked
correction events; they do not overwrite history.

Local PostgreSQL is selected as the durable store. Only the Rust
`persistence` crate accesses it through SQLx. Browser code, source decoders,
discovery rules, and risk rules do not contain database credentials or
PostgreSQL queries.

The server commits an observation and its durable work state before publishing
a message to a bounded Tokio channel. Channels are ephemeral and may be empty
after a restart. Workers reclaim incomplete work from PostgreSQL and process it
idempotently.

The database stores compact facts rather than every full RPC response forever.
An explicit, versioned retention process may remove redundant bulky payloads
only after preserving the normalized evidence, evidence hash/reference, chain
coordinates, and decoder version needed for audit and replay. Any replay made
incomplete by retention must say so.

## Venue identity and Raydium enrichment

Market-dependent facts always reference an exact venue and market:

- Pump bonding curve
- PumpSwap pool
- Raydium CPMM pool
- Raydium CLMM pool
- Raydium AMM v4 pool

Raydium enrichment starts from a relevant Pump candidate and resolves supported
pools for that mint. Each pool produces independent observations and snapshots.
Price, liquidity, volume, and flow are never silently combined across
PumpSwap, Raydium program families, or multiple pools.

A missing Raydium pool is an explicit unavailable observation. It does not
become a rejection unless the active, versioned rule requires Raydium evidence.
Scanning arbitrary Raydium-only mints is not part of the initial intake.

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
  behavior, price extension, venue/program identity, and estimated entry/exit
  impact
- wallet intelligence: wallet scores, historical lead time, typed
  relationships, and time-versioned clusters
- optional structured model predictions, if a trained and calibrated model is
  introduced later

## Projections

Read models derive the approved-only discovery feed, screening counters,
rejection log, inspector, strategies, alerts, orders, and positions from
recorded PostgreSQL facts. Component statuses remain independent. The single
stage shown by the UI is derived from those statuses and can be rebuilt.

HTTP snapshot responses are authoritative at the projection boundary. SSE
events notify the local browser that projections changed; SSE is not the
durable fact log and is not a replay feed. Every connection starts with a
`RESYNC_REQUIRED` notification. The browser refreshes a snapshot before
trusting subsequent live changes.

## Replay

A replay selects an as-of point and freezes:

- source-program, decoder, qualification, and risk-rule versions
- chain finality and information observable by that slot
- feature-engine, wallet-score, cluster, strategy, model, and policy versions
- simulated processing and execution latency
- quote age, slippage, fees, priority fees, price impact, failed transactions,
  sellability, and exit assumptions

Missing source data stays missing. Replays retain rejected candidates and
outages so results do not silently benefit from survivorship or selection bias.
Venue evidence discovered later cannot be applied to an earlier evaluation
unless it was observable by that evaluation's cutoff.
