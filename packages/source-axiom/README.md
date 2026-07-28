# Axiom discovery source

Provider boundary for token discovery observations. This package exposes contracts
only; authentication, scraping, browser automation, and concrete API calls are not
implemented.

The operating adapter will separate a stream-like live observation boundary
from cursor-based historical backfill. Polling may implement live intake
internally, but source filter provenance, raw evidence references, and
deterministic deduplication remain visible to the engine.
