# Portfolio worker (deferred legacy boundary)

This directory is an earlier TypeScript planning marker, not a runnable worker.
Portfolio processing is deferred until paper trading.

Its future Rust implementation will derive holdings, cost basis, realized and
unrealized PnL, exposure, and estimated exit values from reconciled facts rather
than strategy signals or optimistic UI state.

It should begin as a supervised job inside the modular Rust application. Do not
add new implementation here.
