# AI worker

Isolated worker for fast, structured, advisory token triage.

The worker will consume bounded observations and return schema-validated notes. It cannot approve or reject tokens, change deterministic risk, authorize trades, access signing material, or block the live stream.

Model selection, prompts, API access, and production behavior are deferred.
