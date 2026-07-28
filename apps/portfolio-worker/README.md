# Portfolio worker

Reserved logical job for deriving holdings, cost basis, realized and unrealized
PnL, exposure, and estimated exit values from reconciled activity.

It will consume confirmed facts rather than strategy signals or optimistic UI state. Pricing, accounting rules, RPC access, and persistence are deferred.

It begins inside the modular backend process; its ownership boundary does not
require an independent service.
