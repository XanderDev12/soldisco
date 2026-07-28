# API

Logical browser-facing boundary for token streams, token details, strategies,
replays, and system status.

The API will expose projections produced elsewhere; it will not own discovery,
scoring, strategy logic, wallet keys, or transaction signing. Its first
implementation will provide a read-only stream of normalized source updates.

During the research milestones this boundary will be composed into the single
backend process; this README does not reserve an independently deployed
service.

Axiom, RPC, persistence, authentication, and execution endpoints are deferred.
