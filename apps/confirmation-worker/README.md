# Confirmation worker

Reserved logical job for observing submitted transaction signatures, tracking
confirmation state, and emitting normalized execution outcomes.

It will not submit transactions or infer fills from UI state. RPC selection, retry policy, finality requirements, and reconciliation contracts are deferred.

It begins inside the modular backend process and can be separated only when an
operational requirement justifies another deployment.
