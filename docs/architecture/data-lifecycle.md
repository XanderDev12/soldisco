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
the observable universe and interpretation that produced it. Each confirmed
window also pins the exact Prefilter and Qualification Defaults revisions and
values used to define and evaluate it.

## Live-first intake

The Pump/PumpSwap collector uses Solana WebSocket PubSub for low-latency live
activity. PubSub is not a completeness guarantee, and this milestone chooses
latency over catch-up:

- failed notifications are discarded before HTTP
- only direct fresh Pump creation and PumpSwap pool-creation logs are eligible
  for `getTransaction`
- within one continuously running stream instance, duplicate Pump/PumpSwap
  subscription delivery shares one signature claim for the full freshness
  horizon
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

A deterministic chain coordinate prevents the same fact from being processed
twice. Retained observations permit exactly one in-place transport correction:
a non-opening observation atomically keeps the earliest compatible receipt and
updates existing window membership to that canonical time. Duplicate delivery
may otherwise vary only in transport metadata that does not redefine the fact:
receipt order can differ, commitment can differ, and one provider can add or
omit a transaction index when neither known index conflicts. Once an
observation has opened a durable window, its receipt and the window bounds
remain frozen. Stored commitment, optional transaction index, normalized
payload, source details, and canonical direct-event bytes are otherwise not
rewritten. Different slot, two different known transaction indexes, decoded
payload, market identity, provenance, or raw evidence under the same coordinate
is a conflict and fails closed. Explicit retention may delete an entire
eligible terminal row, and future corrections must create linked facts rather
than overwrite retained history. Relationships for finality changes, chain
reorganizations, and decoder or normalization corrections are still planned.

Local PostgreSQL is selected as the durable store. Only the Rust
`persistence` crate accesses it through SQLx. Browser code, source decoders,
discovery rules, and risk rules do not contain database credentials or
PostgreSQL queries.

Prefilter Defaults, append-only Qualification Defaults revisions, and
requested-running stream intent are stored in PostgreSQL, so browser, server,
and computer restarts do not reset them unless the database itself is removed.
Prefilter changes require the stream to be stopped. Qualification Defaults can
change while collection runs; each newly confirmed window pins the then-current
revision and values, so a later edit never rewrites an open or finalized
window. Requested-running intent is restored by launching a fresh supervised
pipeline; actual task handles and health states remain runtime facts.

The browser separately stores only the Paper/Live presentation choice and
adjustable sidebar/inspector widths in versioned `localStorage` entries. These
preferences do not define domain state or grant execution authority. Order
drafts, unsaved form text, active navigation, selection, tabs, and modals are
transient rather than replayable or durable state.

The server commits a normalized discovery, its confirmed bounded window, pinned
settings, and durable work state before notifying downstream workers. Matching
activity observations are durably linked to the exact window. The collector
transaction queue is bounded, and all in-process signaling is ephemeral.
Workers reclaim incomplete leased work from PostgreSQL and process it
idempotently after a restart.

The database stores normalized facts plus complete versioned source details and
compact base64 source evidence, rather than every full RPC response forever.
Implemented maintenance removes eligible terminal observation/work history,
replaceable projection events, and quarantine records after their configured
ages in bounded batches. It does not prune observations while their window is
active. It also retains confirmed windows, frozen feature snapshots,
assessments, rule results, pending or leased work, current
discovery/activity aggregates, checkpoints, market mappings, rejection
summaries, and collection gaps. The default terminal raw-history window is 24
hours. A qualification audit therefore remains durable after some underlying
source details age out, but later inspection or replay must disclose that the
raw evidence is no longer complete.

The first requested collection start verifies the configured HTTP RPC's pinned
genesis hash, then immutably binds the database to that Solana network before
ingestion. A mainnet/devnet switch requires a separate database. Collection
fails closed when physical database size reaches the configured limit.

The limit measures `pg_database_size`, not remaining filesystem capacity, WAL,
other databases, Docker storage, or build artifacts. Local operation still
requires independent free-space monitoring. Current market and discovery
aggregates plus window snapshots and assessments remain durable; they are not
yet archived or expired. Before collection can run indefinitely, aggregate and
qualification-history expiry or archival must be defined without orphaning
later events or invalidating their audit trail.

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

Each fresh direct Pump token-creation or PumpSwap pool-creation log first opens
one short, non-extending provisional receipt-time window. This lets activity
arriving during the one-shot HTTP read retain its correct membership. The
window becomes a durable confirmed record only after the authoritative
discovery normalizes and persists; confirmation records its exact network,
program, venue, market, mint, quote asset, target identity, open and close
times, collector run, and pinned Prefilter and Qualification Defaults. A
missing, failed, stale, invalid, or rate-limited read cancels the provisional
window and invalidates already queued activity.

Creation plus initial activity in the same authoritative transaction is
processed discovery-first. While confirmed, the collector decodes matching
Pump mint or PumpSwap pool activity directly from the two existing global
PubSub feeds and persists those trade/lifecycle facts without an activity
`getTransaction`. The canonical event is the direct `Program data:` copy. With
the pinned program log shape, Soldisco accepts a silent successful self-CPI
only when it immediately pairs 1:1 with that direct event in the same
top-level instruction, and still counts each direct event only once.

A CPI-only event, an unpaired or out-of-order silent self-CPI, a future
discriminator, malformed known event data, an explicit truncation marker, or
an invalid or unbalanced invocation stack is a source-coverage gap. Because
PubSub does not include CPI instruction bytes and the affected mint/pool cannot
always be identified safely, every unfinalized window for that same Pump or
PumpSwap source whose interval contains the receipt is marked incomplete. The
other source is unaffected. Soldisco does not issue activity HTTP reads to
recover these gaps. Window membership remains the half-open Soldisco
receipt-time interval `[opened_at, closes_at)`; delayed discovery HTTP
completion or later worker processing does not move a fact into or out of that
interval.

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
evicts the soonest-closing window. If bounded collector capacity is saturated,
socket-buffered notifications are timestamped only when capacity becomes
available and can consequently fall outside the interval.

At close, a window with no known gap cannot claim complete finalization until
its source has delivered a notification at or beyond the half-open close
boundary and every receipt-time admission has reached a terminal processing
outcome. The source-progress watermark does not claim historical completeness;
it prevents an optimistic close from overtaking queued work or
disconnect/idle detection. Cancellation and finalization claims share one
registry lock. Cancellation that wins first freezes an incomplete reason and
therefore `UNKNOWN`; a claim that wins freezes the prior completeness for that
attempt, so a later stop or disconnect cannot rewrite it. An unsuccessful
attempt releases the claim and is ordered again on retry.

The persistence transaction also refuses to finalize while any structural
projection work belonging to that same window remains `PENDING` or
`PROCESSING`, or while the window's durable evidence changes. The worker
retries after that work settles. PostgreSQL rebuilds the canonical snapshot and
rules from the locked member observations and pinned settings before committing
the snapshot, assessment, rule results, counters, and current-token transition
together.

The immutable feature snapshot includes trade, buy, sell, quote/base volume,
unique trader, wallet-concentration, price, reserve, creator-volume,
completion, and migration evidence where available. The pinned rules cover
minimum trades, traders, buys, sells, quote volume, supported quote asset, and
maximum single-wallet quote share. A complete window produces per-rule
`PASS`/`REJECT` results and an overall `PASS` or `REJECT`. Stop, source
disconnection, source-coverage gap, queue or processing failure, capacity
eviction, or pipeline/server restart makes an affected confirmed window
incomplete, so every rule and the overall assessment is `UNKNOWN`. There is no
historical source backfill.

The provisional registry and in-process queues remain ephemeral; confirmed
window identity, exact persisted members, pinned settings, snapshots,
assessments, and rule results survive browser, server, and computer restarts.
Finalization never overwrites a newer token projection when an older window has
already been superseded, but the older window's audit is still completed.

A qualification `PASS` changes the token to `QUALIFIED`. This is only an
activity-quality gate for the bounded window. It is not a deterministic
scam/rug safety approval, ROI prediction, recommendation, strategy match, or
trade authorization.

The direct-log prefilter intentionally does not spend HTTP capacity on a
transaction whose qualifying creation is visible only through Anchor event CPI
instruction data. Once a direct creation qualifies and its transaction is
fetched, supported CPI instruction bytes can corroborate its canonical direct
event. They never replace the direct bytes or create a second durable event
identity.

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

The implemented qualification assessment references an immutable bounded-window
feature snapshot rather than a mutable aggregate. Its window identity, evidence
cutoff, pinned settings, completeness, computed values, availability, and
reason codes remain auditable.

Future strategy evaluation will reference additional immutable snapshot IDs.
Those snapshot families can include:

- market features: trades, volume, net flow, liquidity, unique buyers, holder
  behavior, price extension, venue/program identity, and estimated entry/exit
  impact
- wallet intelligence: wallet scores, historical lead time, typed
  relationships, and time-versioned clusters
- optional structured model predictions, if a trained and calibrated model is
  introduced later

## Projections

The implemented read model defaults to `QUALIFIED_ONLY`: the visible Discovery
feed contains current `QUALIFIED` tokens and reserves inclusion of future
`APPROVED` tokens. `OBSERVE_ALL` remains a diagnostic mode for pipeline
validation. PostgreSQL also drives observed, qualification-pending, qualified,
qualification-rejected, qualification-unknown, and structural-processing
counters plus per-reason rejection summaries. A window can reject multiple
rules, so reason counts can exceed the number of rejected windows.

`QUALIFIED` is distinct from future `APPROVED`. Later read models add
deterministic scam/rug risk decisions, Raydium evidence, approval, strategies,
alerts, orders, and positions. Component statuses remain independent. The
single stage shown by the UI is derived from those statuses and can be rebuilt.

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
