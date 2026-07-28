# Confirmation worker (deferred legacy boundary)

This directory is an earlier TypeScript planning marker, not a runnable worker.
Transaction submission and confirmation are outside the discovery backend.

When live execution is implemented, confirmation will be a supervised Rust job
that observes submitted signatures, tracks finality, and records normalized
outcomes. It will not infer fills from UI state.

Because execution is a later isolated security boundary, no implementation
should be added here.
