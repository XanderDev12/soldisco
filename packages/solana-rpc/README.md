# Solana RPC (superseded TypeScript scaffold)

This package contains the earlier TypeScript RPC sketch. Provider-neutral Solana
HTTP, PubSub, reconnect, recovery, timeout, and health behavior now belongs in
the Rust `crates/solana-rpc` crate.

The Rust crate is read-only for the discovery milestone. Transaction
construction, signing, submission, and private keys do not belong there. Do not
add new implementation to this TypeScript package.
