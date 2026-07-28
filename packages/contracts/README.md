# Contracts (superseded TypeScript scaffold)

This package contains the earlier TypeScript contract sketch. Rust is now the
backend source of truth: domain and durable event types belong in
`crates/domain`, while browser response and SSE shapes belong in
`crates/api-contracts`.

The TypeScript source remains temporarily for reference and typechecking while
the UI transition is planned. New backend contracts must not be added here;
frontend types should eventually be generated from the versioned Rust API.
