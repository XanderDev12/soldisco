# API

Browser-facing boundary for token streams, token details, strategies, replays, and system status.

The API will expose projections produced elsewhere; it will not own discovery,
scoring, strategy logic, wallet keys, or transaction signing. Its first
implementation will provide a read-only stream of normalized source updates.

Axiom, RPC, persistence, authentication, and execution endpoints are deferred.
