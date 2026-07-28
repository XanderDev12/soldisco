# Execution model

Discovery and strategy evaluation never possess execution authority.

## Shared lifecycle

The lifecycle is deliberately explicit:

1. an identified strategy evaluation or manual action creates a proposal
2. deterministic policy evaluates exposure, risk, limits, and kill switches
3. a quote records venue, price impact, fees, minimum output, and expiry
4. transaction preparation validates the intended route and simulates it
5. an authorized signer approves the exact prepared transaction
6. submission, confirmation, finality, fill reconciliation, and portfolio
   projection produce separate records

Optimistic UI state is never treated as a confirmed fill or position.

## Paper mode

Paper trading comes first. It uses recorded market conditions and realistic
latency, quote expiry, priority fees, slippage, price impact, failed entries,
failed exits, and sellability constraints. Paper fills are clearly separated
from live facts.

## Interactive live mode

Interactive live trading requires:

- an explicit user-reviewed intent for each transaction
- deterministic policy approval and an active kill switch
- a fresh quote and successful simulation
- browser-wallet signing of the exact prepared transaction
- confirmation and reconciliation before portfolio updates

The backend never receives a seed phrase or raw exportable private key.

## Automated mode

Unattended execution is a separate future security architecture, not a switch
on interactive mode. It requires explicit approval of bounded capital,
strategy and venue allowlists, revocation, expiring or delegated authority,
auditability, rate and loss limits, and independent kill switches.

Automated mode remains out of scope until paper and interactive-live evidence
justify its design.
