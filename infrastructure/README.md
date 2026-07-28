# Infrastructure

This directory records deferred operational decisions and will eventually hold
real local-development or deployment assets. It is intentionally documentation
only today and has no runtime effect.

The initial backend is a modular monolith using in-process composition.
Repository folders do not imply separate services, containers, or a service
mesh.

Current decision markers:

- `postgres` — possible relational observation and projection storage
- `nats` — possible future durable event transport
- `telemetry` — future logs, metrics, and traces

No database, event bus, container, or observability backend has been selected or
configured. A subdirectory remains only to preserve its decision context; real
infrastructure should be introduced when a milestone requires it and operational
ownership is clear.
