# Execution domain (deferred TypeScript scaffold)

This package records the earlier TypeScript execution model. Live execution is
not part of the discovery server and its future domain types belong in an
isolated Rust execution boundary.

A strategy signal, trade proposal, user-approved intent, order, prepared
transaction, submitted transaction, and fill remain distinct records. Paper,
interactive-live, and automated modes have different authority requirements.
No signer or private-key contract is defined here, and no new backend
implementation should be added here.
