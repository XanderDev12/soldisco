# API (superseded planning stub)

This directory is retained temporarily to explain the earlier TypeScript
layout. It is not a runnable service and is superseded by the Axum HTTP modules
inside `apps/server`.

The Rust API exposes commands and snapshots through ordinary HTTP and sends
live discovery projections to the React application through Server-Sent Events
(SSE). It does not own discovery, risk, strategy logic, wallet keys, or
transaction signing.

Do not add new implementation here. Browser-facing Rust contracts belong in
`crates/api-contracts`, and route orchestration belongs in `apps/server`.
