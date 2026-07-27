# Configuration names

Configuration is not wired yet. When integrations are added, use environment-specific validation and keep secrets out of source control.

Example names:

```text
APP_ENV
LOG_LEVEL
WEB_ORIGIN
API_HOST
API_PORT
DATABASE_URL
NATS_URL
OTEL_EXPORTER_OTLP_ENDPOINT

AXIOM_API_BASE_URL
AXIOM_API_TOKEN

SOLANA_RPC_HTTP_URL
SOLANA_RPC_WS_URL

AI_API_KEY
AI_MODEL

EXECUTION_ENABLED
SOLANA_NETWORK
```

Do not add actual values to documentation or committed `.env` files. Axiom, RPC, AI, and execution variables are reserved names only and are not consumed by the skeleton.
