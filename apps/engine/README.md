# Engine

Logical home for normalization, deduplication, enrichment orchestration,
deterministic first-pass checks, risk, ratings, candidate windows, immutable
features, and strategy evaluation.

Every output must be explainable through reason codes, source timestamps,
evidence and definition versions. AI output may be attached as advisory context
but cannot override deterministic results.

The first implementation belongs inside the modular backend process. The
boundary is about ownership and testability, not network deployment.

Axiom ingestion, RPC calls, and actual scam or rug rules are deferred.
