# Solana RPC (superseded TypeScript scaffold)

This package contains the earlier TypeScript RPC sketch. Provider-neutral Solana
HTTP, PubSub, reconnect, timeout, health, and reserved explicit-recovery
behavior now belongs in the Rust `crates/solana-rpc` crate. The active
live-first collector reconnects at the current head without backfill.

The Rust crate is read-only for the discovery milestone. Transaction
construction, signing, submission, and private keys do not belong there. Do not
add new implementation to this TypeScript package.
