# Strategy schema (deferred TypeScript scaffold)

This package records the earlier TypeScript upload-format sketch. The eventual
versioned schema and validation rules belong beside the Rust strategy engine,
with browser-facing representations exposed through `crates/api-contracts`.

Uploaded strategies remain declarative data rather than executable JavaScript,
Python, or Rust and stay inactive until validation and replay checks pass. Do
not add new backend implementation here.
