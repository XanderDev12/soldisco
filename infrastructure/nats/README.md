# NATS plan

NATS is not selected or required. The first Rust backend is one process and uses
bounded Tokio channels for fast in-process handoff plus PostgreSQL for
durability and restart recovery.

An external broker should be considered only if independently deployed
consumers become necessary and measured throughput or reliability requirements
cannot be met by the modular application. No `NATS_URL`, NATS container, or
broker-specific contract belongs in the initial configuration.
