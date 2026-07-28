# Execution API (deferred legacy boundary)

This directory is an earlier TypeScript planning marker and is not a runnable
service. Live execution is intentionally absent from the discovery backend.

A later isolated Rust executable may convert explicit user-approved trade
intents into verified and simulated transaction proposals. It must remain
separate from discovery and strategy decisions. Live execution will be
non-custodial, policy-gated, auditable, and disabled by default; the backend
must never receive seed phrases or private keys.

Interactive live mode and unattended automated mode remain separate future
authorization designs. Do not add new implementation here.
