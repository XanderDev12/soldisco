# Product flow

## End-to-end

1. **Start locally** — the React interface sends a locally guarded command to
   the Rust server. The server verifies the configured HTTP RPC's pinned Solana
   genesis identity before binding the database or opening intake; the browser
   never owns collector state.
2. **Pump intake** — WebSocket PubSub observes relevant Pump and PumpSwap
   notifications. The server ignores failed transactions and predecodes direct
   program-data logs. Fresh Pump creation or PumpSwap pool-creation events
   provision an immediate activity window and, within the running server
   process, can receive at most one globally deduplicated, paced HTTP
   transaction request. A discovery that ages out before request admission is
   cancelled and skipped without HTTP. An attempted response must match the
   successful notification's signature and exact slot. A miss, timeout, or
   rate limit cancels that provisional window and skips the discovery without
   retrying it or disturbing the live WebSocket. A provider rate-limit response
   delays only later, distinct signatures.
3. **Durable handoff** — each normalized observation is committed to local
   PostgreSQL with source, decoder, Solana coordinates, exact market identity,
   event time, receipt time, and compact evidence before downstream work is
   notified. Attributable malformed event or log evidence, plus historical
   facts whose exact market/quote cannot be resolved, enter quarantine; unknown
   discriminators are ignored. Quarantined unresolved facts are not
   automatically reprocessed yet. A compatible duplicate retains one canonical
   receipt and direct evidence without duplicate work or membership; an
   existing window opener remains frozen. Conflicting immutable evidence under
   the same chain coordinate fails closed.
4. **Structural discovery (implemented)** — every structurally valid decoded
   candidate is first `OBSERVED`. Its configurable provisional window preserves
   immediate and same-transaction activity while the authoritative read is
   pending. Successful normalization atomically creates one durable,
   non-extending PostgreSQL window with exact mint, venue, market, quote,
   receipt-time bounds, collector run, and pinned settings revisions. Failed
   discovery cancels the provisional token.
5. **Window evidence (implemented)** — while the window is open, matching Pump
   mint or PumpSwap pool events are decoded directly from the existing PubSub
   feeds and persisted with exact half-open receipt-time membership, without
   activity HTTP. The direct program-data event is counted once; its
   immediately following silent event self-CPI only corroborates it under
   ordered 1:1 pairing in the same instruction. CPI-only, out-of-order,
   mismatched, malformed, truncated, or unbalanced logs make same-source
   windows incomplete. The frozen versioned snapshot contains trade samples,
   buy/sell counts and atomic volumes, unique traders/buyers/sellers,
   per-wallet flow, concentration, price movement, liquidity deposits and
   withdrawals, reserve samples, and observed completion or migration.
6. **Initial qualification (implemented)** — a complete snapshot is evaluated
   only after source progress reaches the window close, all receipt-time
   admissions settle, and same-window structural projection work finishes. It
   is then evaluated against its pinned Qualification Defaults revision.
   Versioned rules cover completeness, supported quote assets, minimum
   trades/traders/buys/sells and quote volume, and maximum single-wallet quote
   share. `PASS` advances the current exact-market candidate to `QUALIFIED`;
   `REJECT` records unmet activity-quality rules; `UNKNOWN` records incomplete
   or unavailable evidence. Snapshot, assessment, rule evidence, counters,
   summaries, and any projection change commit together.
7. **Truthful interruption handling (implemented)** — stream stop, source
   disconnect, capacity eviction, queue overflow, or pipeline/process restart
   that wins before finalization makes an affected window explicitly
   incomplete. Its retained facts remain useful, but the result is `UNKNOWN`;
   missing events are never treated as observed zeros. A finalization claim
   that wins first freezes its prior completeness for that attempt. Superseded
   exact-market windows retain their audit result without overwriting the newer
   current-token projection.
8. **Operational safety (implemented)** — global Prefilter Defaults are
   initialized once from validated local environment values, persisted in
   PostgreSQL with optimistic revisions, and editable from Controls only while
   the stream is explicitly stopped. A later Start loads the current revision
   into a fresh pipeline and RPC gate. Qualification Defaults are independently
   revisioned in PostgreSQL, editable while running, and apply only to new
   durable windows; existing windows keep their pinned values. Both setting
   groups and requested-running stream intent survive browser, server, and
   computer restarts unless the database is deliberately reset. Paper/Live
   presentation and adjustable sidebar/inspector widths persist separately in
   browser `localStorage` without authorization power; order drafts, unsaved
   form text, and transient navigation do not persist.
9. **Discovery projection (implemented)** — the default `QUALIFIED_ONLY` feed
   shows only current `QUALIFIED` candidates. Counters separately report current
   observed/qualified tokens, queued structural facts, active qualification
   windows, historical qualification rejections/unknowns, and processing
   failures. Diagnostic `OBSERVE_ALL` remains available. HTTP snapshots are
   bounded and authoritative; coalesced named `soldisco` SSE notifications
   trigger refreshes.
10. **Deterministic risk pass (planned)** — provider-neutral RPC checks identify
   invalid, incomplete, scam-like, and rug-like candidates and return
   explainable `PASS`, `REJECT`, or `UNKNOWN` results.
11. **Raydium venue enrichment (planned)** — when an eligible Pump candidate has a
   supported Raydium market, exact CPMM, CLMM, or AMM v4 pools are evaluated as
   separate venue evidence. Metrics are not blindly merged across pools.
12. **Candidate monitoring (planned)** — approved candidates later enter richer
   persisted windows that continue tracking liquidity, price, holders, wallet
   activity, and strategy-specific features. These are separate from the
   implemented short pre-decision activity window.
13. **Feature production (planned)** — immutable market, wallet-score, and wallet-cluster
   snapshots capture exactly what was knowable at an evaluation time.
14. **Advisory AI (planned)** — optional asynchronous AI summarizes bounded evidence and
   uncertainty without approving tokens, blocking ingestion, or authorizing
   trades.
15. **Strategy evaluation (planned)** — each enabled strategy repeatedly evaluates fresh
    snapshots and emits an identified, versioned result with reason codes.
16. **Trading (planned)** — matches may later create paper proposals. Interactive Live
    trading later adds deterministic policy, quotes, simulation, explicit user
    review, and browser-wallet signing.

`QUALIFIED` is deliberately not called safe or approved: it means only that a
complete exact-market window met the configured activity-quality gate. It is
not a scam ruling, ROI forecast, purchase recommendation, strategy match, or
trade authorization.

Terminal raw observations, full versioned source details, and compact decoder
evidence follow the configured retention policy. Frozen window features and
qualification audits remain durable, but future replay tooling must state when
the underlying raw history has aged out. It cannot treat an incomplete retained
window or feature snapshot as complete source history.

When Raydium enrichment exists, absence of a venue will be an explicit
unavailable result, not an automatic failure unless a versioned rule requires
it. Initial collection is driven by Pump and PumpSwap; scanning arbitrary
Raydium-only mints is separate future scope.

## Site destinations

The modular UI retains these destinations, but only local stream control,
Discovery, and current token facts are connected in this milestone:

- **Discovery** — current `QUALIFIED` candidates, truthful pipeline and
  qualification counters, and a compact activity-rule rejection log
- **Inspector** — current window summary and qualification evidence; later risk,
  signals, trade, and position details
- **Strategies** — upload, validate, replay, activate, and toggle versions
- **Orders** — one destination with mode-specific Paper records or Live
  transaction lifecycle
- **Positions** — one destination with separate Paper ledger projections or
  Live reconciled holdings, exposure, PnL, and exit controls
- **Alerts** — risk changes, matches, stale data, outages, and execution events
- **Replays** — historical evaluation with original information boundaries
- **Controls** — current local stream/database state plus persisted Prefilter
  and Qualification Defaults; later per-source RPC, recovery, risk, Raydium
  health, operating mode, and kill switches

The UI receives read-only projections. It does not define domain truth, perform
scoring, or infer fills. Paper mode never presents wallet connection as a
dependency; wallet controls belong only to Live mode.

The selected topology is local: `localhost:3000` sends HTTP commands and
receives authoritative snapshots plus named SSE notifications from
`127.0.0.1:8080`. The Rust server alone talks to PostgreSQL and Solana. The
hosted Sites preview remains disconnected because it has no remotely deployed
Rust backend.
