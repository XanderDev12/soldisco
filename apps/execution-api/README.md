# Execution API

Reserved boundary for converting explicit user-approved trade intents into verified and simulated transaction proposals.

It must remain isolated from discovery and strategy decisions. Live execution will be non-custodial, policy-gated, auditable, and disabled by default. The backend must never receive seed phrases or private keys.

Execution may become an independently deployed security boundary earlier than
other modules. Interactive live mode and unattended automated mode are separate
authorization designs; neither is implemented.

Wallet integration, routing venues, quotes, simulation, signing, and submission are deferred.
