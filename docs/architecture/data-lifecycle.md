# Data lifecycle and replay model

This document defines the information that must survive ingestion so later
analysis can be reproduced without lookahead or hidden source filtering.

## Source observations

Each implemented Pump/PumpSwap observation preserves:

- immutable observation ID and schema version
- source program and decoder identity/version
- raw evidence hash and a base64 encoding of the exact decoder input bytes
- Solana network, mint, exact pool or market, venue, and migration state when
  known
- source event time and Soldisco receipt time
- slot, optional provider transaction index, transaction signature,
  instruction/event coordinates, and commitment when available in the current
  source contract

Pump, PumpSwap, and each supported Raydium program are distinct source-program
identities. Unavailable coordinates are explicit rather than invented. The
decoder version remains visible because later performance is conditional on
the observable universe and interpretation that produced it. Qualification
rule versions join that provenance when qualification is implemented.

## Live-first intake

The Pump/PumpSwap collector uses Solana WebSocket PubSub for low-latency live
activity. PubSub is not a completeness guarantee, and this milestone chooses
latency over catch-up:

- failed notifications are discarded before HTTP
- only direct fresh Pump creation and PumpSwap pool-creation logs are eligible
  for `getTransaction`
- within one running server process, duplicate Pump/PumpSwap subscription
  delivery shares one signature claim for the full freshness horizon
- each selected signature receives at most one globally paced HTTP attempt in
  that process
- a discovery that ages out while waiting for request admission is skipped
  without HTTP
- stale, unavailable, timed-out, rate-limited, or invalid responses are skipped
- a provider rate-limit response delays later distinct signatures but never
  retries the failed signature
- a disconnected source reconnects at the current head with no historical
  `getSignaturesForAddress` scan

Accepted facts still use exact chain identity and deterministic deduplication.
Attributable malformed event, program-data, or log-scope evidence that reaches
the authoritative decoder is written to deduplicated intake quarantine with
decoder version, reason, coordinate, and bounded evidence. Unknown
discriminators are ignored rather than treated as candidates. A structurally
attributable fact whose exact market or quote cannot be resolved also receives
a durable quarantine disposition. That evidence is not automatically
reprocessed in this milestone.

Existing checkpoint, recovery, and collection-gap schemas are retained as
reserved infrastructure. The live-first collector neither reads nor advances
those exact-history checkpoints. A future recovery mode must be explicit and
must not imply that a live-first interval was complete.

## Persistence and correction

While retained, raw observations are never updated in place. A deterministic
identity key prevents the same chain fact from being processed twice. Explicit
retention may delete an entire eligible terminal row, and future corrections
must create linked facts rather than overwrite retained history. Relationships
for finality changes, chain reorganizations, and decoder or normalization
corrections are still planned.

Local PostgreSQL is selected as the durable store. Only the Rust
`persistence` crate accesses it through SQLx. Browser code, source decoders,
discovery rules, and risk rules do not contain database credentials or
PostgreSQL queries.

The server commits an observation and its durable work state before notifying
the discovery worker. The collector transaction queue is bounded, and all
in-process signaling is ephemeral. Workers reclaim incomplete leased work from
PostgreSQL and process it idempotently after a restart.

The database stores compact facts rather than every full RPC response forever.
Implemented maintenance removes terminal observation/work history,
replaceable projection events, and quarantine records after their configured
ages in bounded batches. It never removes pending or leased work, current
discovery/activity aggregates, checkpoints, market mappings, rejection
summaries, or collection gaps. The default terminal-history window is 24
hours, so future replay tooling must state when requested raw history is no
longer retained.

The first requested collection start verifies the configured HTTP RPC's pinned
genesis hash, then immutably binds the database to that Solana network before
ingestion. A mainnet/devnet switch requires a separate database. Collection
fails closed when physical database size reaches the configured limit.

The limit measures `pg_database_size`, not remaining filesystem capacity, WAL,
other databases, Docker storage, or build artifacts. Local operation still
requires independent free-space monitoring. Current market and discovery
aggregates remain durable so later trades can resolve exact venue identity;
they are not yet archived or expired. Before collection can run indefinitely,
aggregate expiry or archival must be defined without orphaning later events.

## Venue identity and Raydium enrichment

Market-dependent facts always reference an exact venue and market:

- Pump bonding curve
- PumpSwap pool
- Raydium CPMM pool
- Raydium CLMM pool
- Raydium AMM v4 pool

Planned Raydium enrichment starts from a relevant Pump candidate and resolves
supported pools for that mint. Each pool produces independent observations and
snapshots. Price, liquidity, volume, and flow are never silently combined
across PumpSwap, Raydium program families, or multiple pools.

A missing Raydium pool is an explicit unavailable observation. It does not
become a rejection unless the active, versioned rule requires Raydium evidence.
Scanning arbitrary Raydium-only mints is not part of the initial intake.

## Pre-decision observation windows

Each fresh direct Pump token-creation or PumpSwap pool-creation log opens one
short, non-extending provisional in-memory window immediately at receipt. This
lets activity arriving during the one-shot HTTP read retain its receipt-time
membership. The token becomes confirmed only after the authoritative discovery
normalizes and persists; a missing, failed, stale, invalid, or rate-limited
read cancels the provisional token and invalidates already queued activity.
Creation plus initial activity in the same authoritative transaction is
processed discovery-first. While confirmed, the collector decodes matching
Pump mint or PumpSwap pool activity directly from the two existing global
PubSub feeds and persists those trade/lifecycle facts without
`getTransaction`.

WebSocket notification work is bounded separately from HTTP concurrency, so a
few paced or slow discovery reads do not immediately stop socket polling.
Activity that reaches the processor while its token is still provisional waits
in a bounded holding queue. Confirmation or cancellation releases it. A
separate holding deadline, derived from the discovery-age allowance plus the
HTTP timeout, can also release it for deterministic drop without cancelling
the discovery globally. This preserves valid receipt-time activity when the
observation window closes while its one-shot read is still resolving. If that
queue reaches its configured capacity, newer provisional activity is dropped
and logged rather than allowing unbounded memory growth.

Window expiration stops further activity intake for that market. A replayed
creation does not extend the close time, and capacity pressure deterministically
evicts the soonest-closing window. Window membership is based on the time a
notification is admitted into available collector work capacity, not delayed
HTTP or processor completion time. If that bounded capacity is saturated,
socket-buffered notifications are timestamped only when capacity becomes
available and can consequently fail the freshness check. The current registry
is intentionally ephemeral: stream stop/start, a supervised pipeline-attempt
restart, or a server restart drops open windows and does not backfill them.
Capacity eviction also truncates evidence without a durable completeness
marker. Persisted activity gathered before any interruption remains valid but
incomplete. These windows gather inputs only; they do not pass, reject, score,
or approve a token.

The direct-log prefilter intentionally does not spend HTTP capacity on a
transaction whose qualifying creation is visible only through Anchor event
CPI instruction data. Once a direct creation qualifies and its transaction is
fetched, supported direct and CPI evidence for both Pump programs is decoded.

## Future approved-candidate windows

A future approved candidate can open a versioned monitoring window with:

- stable window ID, mint, market, opening trigger, and source provenance
- opened, observed-through, expiry, and closed timestamps
- explicit active, expired, completed, or invalid status
- references to the observations and snapshot versions available at each
  evaluation

Market and wallet updates can produce repeated evaluations during one window.
`STRATEGY_EVALUATED` is therefore not a terminal domain stage.

## Immutable snapshots

A future strategy evaluation references immutable snapshot IDs rather than an
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

The implemented read model derives an `OBSERVE_ALL` discovery feed, cumulative
activity, counters, and token-inspector data from recorded PostgreSQL facts.
Every admitted candidate is structurally valid and remains `OBSERVED`; no
threshold, approval, rejection, risk score, or opportunity score exists yet.

Later read models derive the approved-only feed, screening counters, rejection
log, strategies, alerts, orders, and positions from recorded facts. Component
statuses remain independent. The single stage shown by the UI is derived from
those statuses and can be rebuilt.

HTTP snapshot responses are authoritative at the projection boundary. They
return the latest bounded token set plus total/truncation metadata. SSE events
are coalesced invalidations that notify the local browser to refresh; SSE is
not the durable fact log and is not a replay feed. Every connection starts
with a `RESYNC_REQUIRED` notification. The browser single-flights refreshes,
rejects regressing snapshot sequences, and refreshes before trusting
subsequent live changes.

## Replay

A replay selects an as-of point and freezes:

- source-program, decoder, qualification, and risk-rule versions
- chain finality and information observable by that slot
- feature-engine, wallet-score, cluster, strategy, model, and policy versions
- simulated processing and execution latency
- quote age, slippage, fees, priority fees, price impact, failed transactions,
  sellability, and exit assumptions

Missing source data stays missing. Replay logic must include retained rejected
candidates and outage or collection-gap evidence so results do not silently
benefit from survivorship or selection bias. If requested raw history has aged
out under retention, the replay must declare itself incomplete. Venue evidence
discovered later cannot be applied to an earlier evaluation unless it was
observable by that evaluation's cutoff.
