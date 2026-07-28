# Local PostgreSQL

PostgreSQL is the selected durable store for the local Rust backend. The React
application never connects to it directly.

The Compose configuration binds PostgreSQL only to `127.0.0.1`, persists its
data in a named Docker volume, and requires a password supplied through the
uncommitted root `.env` file. The repository contains only a clearly fake
example.
It pins the supported official `postgres:17.10-alpine3.24` image instead of a
floating `latest` tag so separate local setups use the same database build.
Dependabot may propose reviewed Compose-image updates; it does not apply them
silently.

Docker is optional tooling for this setup; it is not installed or started by
the project.

## First-time setup

From the repository root:

```bash
cp .env.example .env
```

Edit `.env` and replace both occurrences of `replace-with-local-password` with
the same local-only password. This one file configures both Compose and the
Rust server. Then, if Docker is installed:

```bash
npm run db:up
```

The defaults expose PostgreSQL at `127.0.0.1:5432`, create a database named
`soldisco`, and create a role named `soldisco`. Configure the Rust server with
a matching `DATABASE_URL`, for example:

```text
postgres://soldisco:<your-local-password>@127.0.0.1:5432/soldisco
```

Useful commands:

```bash
npm run db:logs
npm run db:down
```

`db:down` stops the database but preserves the named volume. Deliberately
removing that volume destroys local database data and is therefore not part of
the standard scripts.

SQL schema changes belong to
`crates/persistence/migrations`. The initial migration creates durable chain
identity, a pending-work handoff, recovery checkpoints, deterministic rule
results, and projection events. Retention jobs will later bound raw
high-volume event storage while preserving the facts required for recovery and
audit.
