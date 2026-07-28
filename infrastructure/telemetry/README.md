# Telemetry plan

The Rust foundation uses structured `tracing` logs and reports PostgreSQL plus
aggregate supervised-stream health. Pump/PumpSwap collector connection state
and sustained one-shot discovery-RPC failures feed that aggregate status.
Screening, Raydium, and any future explicit recovery mode need more granular
health contracts when implemented. A separate observability service is not
required for local development.

Telemetry must preserve source, transaction, slot, and correlation identity
without recording database passwords, RPC credentials, seed phrases, private
keys, raw signed transactions, or unnecessarily sensitive wallet evidence.
Metrics and external trace storage can be added later if local diagnostics stop
being sufficient.
