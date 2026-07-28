# AI worker

Logical asynchronous boundary for fast, structured, advisory token triage.

The worker will consume bounded observations and return schema-validated notes. It cannot approve or reject tokens, change deterministic risk, authorize trades, access signing material, or block the live stream.

It may begin as an asynchronous job inside the modular backend and be deployed
separately only when cost, latency, or isolation requirements justify it. A
future predictive model is a separate structured and versioned concept, not LLM
commentary.

Model selection, prompts, API access, and production behavior are deferred.
