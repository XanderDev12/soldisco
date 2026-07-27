# Confirmation worker

Reserved worker for observing submitted transaction signatures, tracking confirmation state, and emitting normalized execution outcomes.

It will not submit transactions or infer fills from UI state. RPC selection, retry policy, finality requirements, and reconciliation contracts are deferred.
