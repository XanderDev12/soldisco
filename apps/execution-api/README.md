# Execution API

Reserved boundary for converting explicit user-approved trade intents into verified and simulated transaction proposals.

It must remain isolated from discovery and strategy decisions. Live execution will be non-custodial, policy-gated, auditable, and disabled by default. The backend must never receive seed phrases or private keys.

Wallet integration, routing venues, quotes, simulation, signing, and submission are deferred.
