# Configuration names

Configuration is not wired yet. When integrations are added, use
environment-specific validation and keep secrets out of source control.

## Near-term integration candidates

```text
APP_ENV
LOG_LEVEL
WEB_ORIGIN
API_HOST
API_PORT

AXIOM_API_BASE_URL
AXIOM_API_TOKEN

SOLANA_RPC_HTTP_URL
SOLANA_RPC_WS_URL

AI_API_KEY
AI_MODEL
```

AI configuration is optional. Source intake and deterministic processing must
continue to expose truthful state when the AI worker is unavailable.

## Future execution candidates

```text
EXECUTION_MODE
SOLANA_NETWORK
EXECUTION_POLICY_ID
```

No environment variable grants execution authority. Interactive live execution
still requires explicit reviewed intent and browser-wallet signing. Automated
mode will use a separately designed authorization system rather than an
`EXECUTION_ENABLED` shortcut.

## Reserved implementation-dependent candidates

```text
DATABASE_URL
NATS_URL
OTEL_EXPORTER_OTLP_ENDPOINT
```

These names document possibilities, not selected products or deployment
requirements. The first backend is expected to use in-process module calls;
NATS or another event bus should be introduced only when a measured need
justifies it. Persistence technology remains undecided.

Do not add actual values to documentation or committed `.env` files. All names
above are reserved only and are not consumed by the skeleton.
