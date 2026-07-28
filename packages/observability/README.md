# Observability (superseded TypeScript scaffold)

This package contains the earlier TypeScript telemetry sketch. The running
backend uses Rust `tracing` and health types owned by the Rust server and its
contracts.

Sensitive wallet material, database passwords, RPC credentials, and signing
material must never be attached to telemetry. Do not add new backend
implementation here.
