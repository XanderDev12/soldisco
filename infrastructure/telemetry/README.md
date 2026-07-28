# Telemetry plan

The Rust foundation uses structured `tracing` logs and reports PostgreSQL plus
aggregate stream health. Pump collector, Solana RPC, and recovery health are
added with those live jobs. A separate observability service is not required
for local development.

Telemetry must preserve source, transaction, slot, and correlation identity
without recording database passwords, RPC credentials, seed phrases, private
keys, raw signed transactions, or unnecessarily sensitive wallet evidence.
Metrics and external trace storage can be added later if local diagnostics stop
being sufficient.
