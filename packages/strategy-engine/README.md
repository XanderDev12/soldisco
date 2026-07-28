# Strategy engine

Pure evaluation boundaries. Results express matches, non-matches, missing
dependencies, and errors; they never create trade intents or orders.

The generic feature maps are temporary scaffolding. Persisted evaluations will
have immutable evaluation IDs and reference versioned feature snapshots so
replays can reconstruct exactly what each strategy knew.
