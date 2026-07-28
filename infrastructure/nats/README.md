# NATS plan

NATS is not selected or required by the current architecture. The first backend
uses in-process module calls and append-only persistence.

An event bus should be considered only after observation identity, replay,
projection rebuilding, delivery guarantees, dead-letter handling, and measured
throughput needs are established. The reserved `NATS_URL` configuration name is
not a deployment decision.
