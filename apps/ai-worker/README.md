# AI worker (deferred legacy boundary)

This directory is an earlier TypeScript planning marker, not a runnable worker.
AI triage is outside the Rust backend foundation milestone.

If advisory AI is added, it should begin as a bounded asynchronous job supervised
inside `apps/server`. It can consume evidence snapshots and return
schema-validated notes, but it cannot approve or reject tokens, change
deterministic risk, authorize trades, access signing material, or block the
live stream.

Separation into another process is justified only by measured cost, latency, or
isolation requirements. Do not add new implementation here.
