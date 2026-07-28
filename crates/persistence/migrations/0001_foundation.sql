CREATE TABLE stream_control (
    singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (singleton),
    requested_running BOOLEAN NOT NULL DEFAULT FALSE,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

INSERT INTO stream_control (singleton, requested_running)
VALUES (TRUE, FALSE);

CREATE TABLE chain_observations (
    id BIGSERIAL PRIMARY KEY,
    network TEXT NOT NULL,
    source_program TEXT NOT NULL,
    slot BIGINT NOT NULL CHECK (slot >= 0),
    signature TEXT NOT NULL,
    instruction_index INTEGER NOT NULL CHECK (instruction_index >= 0),
    event_index INTEGER NOT NULL CHECK (event_index >= 0),
    commitment TEXT NOT NULL,
    schema_version INTEGER NOT NULL CHECK (schema_version > 0),
    decoder_version TEXT NOT NULL,
    event_kind TEXT NOT NULL,
    mint TEXT NOT NULL,
    venue TEXT NOT NULL,
    market_address TEXT NOT NULL,
    quote_mint TEXT,
    source_event_time_unix_ms BIGINT,
    received_time_unix_ms BIGINT NOT NULL,
    raw_evidence_hash TEXT NOT NULL,
    normalized_observation JSONB NOT NULL,
    first_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (
        network,
        source_program,
        signature,
        instruction_index,
        event_index
    )
);

CREATE INDEX chain_observations_mint_slot_idx
    ON chain_observations (mint, slot DESC);

CREATE INDEX chain_observations_market_slot_idx
    ON chain_observations (venue, market_address, slot DESC);

CREATE TABLE observation_work (
    id BIGSERIAL PRIMARY KEY,
    observation_id BIGINT NOT NULL
        REFERENCES chain_observations (id) ON DELETE CASCADE,
    work_kind TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'PENDING'
        CHECK (status IN ('PENDING', 'PROCESSING', 'COMPLETE', 'FAILED')),
    attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    available_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    claimed_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    last_error_code TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (observation_id, work_kind)
);

CREATE INDEX observation_work_pending_idx
    ON observation_work (status, available_at, id);

CREATE TABLE recovery_checkpoints (
    id BIGSERIAL PRIMARY KEY,
    network TEXT NOT NULL,
    source_program TEXT NOT NULL,
    checkpoint_kind TEXT NOT NULL,
    last_slot BIGINT NOT NULL CHECK (last_slot >= 0),
    last_signature TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (network, source_program, checkpoint_kind)
);

CREATE TABLE screening_runs (
    id BIGSERIAL PRIMARY KEY,
    mint TEXT NOT NULL,
    venue TEXT NOT NULL,
    market_address TEXT NOT NULL,
    ruleset_version TEXT NOT NULL,
    decision TEXT NOT NULL CHECK (decision IN ('PASS', 'REJECT', 'UNKNOWN')),
    risk_score SMALLINT CHECK (risk_score BETWEEN 0 AND 100),
    opportunity_score SMALLINT CHECK (opportunity_score BETWEEN 0 AND 100),
    observed_slot BIGINT CHECK (observed_slot >= 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX screening_runs_mint_created_idx
    ON screening_runs (mint, created_at DESC);

CREATE TABLE screening_rule_results (
    id BIGSERIAL PRIMARY KEY,
    screening_run_id BIGINT NOT NULL
        REFERENCES screening_runs (id) ON DELETE CASCADE,
    rule_id TEXT NOT NULL,
    rule_version TEXT NOT NULL,
    decision TEXT NOT NULL CHECK (decision IN ('PASS', 'REJECT', 'UNKNOWN')),
    reason_code TEXT NOT NULL,
    evidence_reference TEXT,
    observed_slot BIGINT CHECK (observed_slot >= 0),
    UNIQUE (screening_run_id, rule_id)
);

CREATE TABLE projection_events (
    sequence BIGSERIAL PRIMARY KEY,
    projection_name TEXT NOT NULL,
    event_kind TEXT NOT NULL,
    payload JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX projection_events_name_sequence_idx
    ON projection_events (projection_name, sequence DESC);
