ALTER TABLE observation_work
    ADD COLUMN lease_owner TEXT,
    ADD COLUMN lease_expires_at TIMESTAMPTZ,
    ADD CONSTRAINT observation_work_lease_pair_check CHECK (
        (lease_owner IS NULL AND lease_expires_at IS NULL)
        OR
        (lease_owner IS NOT NULL AND lease_expires_at IS NOT NULL)
    ),
    ADD CONSTRAINT observation_work_processing_lease_check CHECK (
        status = 'PROCESSING'
        OR
        (lease_owner IS NULL AND lease_expires_at IS NULL)
    );

CREATE INDEX observation_work_claim_idx
    ON observation_work (work_kind, available_at, id)
    WHERE status IN ('PENDING', 'PROCESSING');

ALTER TABLE screening_runs
    ADD COLUMN observation_work_id BIGINT
        REFERENCES observation_work (id) ON DELETE SET NULL,
    ADD COLUMN screening_record JSONB NOT NULL DEFAULT '{}'::JSONB;

CREATE UNIQUE INDEX screening_runs_observation_work_idx
    ON screening_runs (observation_work_id)
    WHERE observation_work_id IS NOT NULL;

CREATE TABLE discovery_projection_state (
    singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (singleton),
    mode TEXT NOT NULL DEFAULT 'OBSERVE_ALL'
        CHECK (mode IN ('OBSERVE_ALL', 'APPROVED_ONLY')),
    sequence BIGINT NOT NULL DEFAULT 0 CHECK (sequence >= 0),
    observed BIGINT NOT NULL DEFAULT 0 CHECK (observed >= 0),
    pending BIGINT NOT NULL DEFAULT 0 CHECK (pending >= 0),
    approved BIGINT NOT NULL DEFAULT 0 CHECK (approved >= 0),
    rejected BIGINT NOT NULL DEFAULT 0 CHECK (rejected >= 0),
    flow_per_minute BIGINT CHECK (flow_per_minute >= 0),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

INSERT INTO discovery_projection_state (singleton)
VALUES (TRUE);

CREATE TABLE discovery_tokens (
    mint TEXT PRIMARY KEY,
    stage TEXT NOT NULL CHECK (stage IN ('OBSERVED', 'APPROVED')),
    network TEXT NOT NULL,
    source_program TEXT NOT NULL,
    venue TEXT NOT NULL,
    market_address TEXT NOT NULL,
    quote_mint TEXT,
    event_kind TEXT NOT NULL,
    observed_slot BIGINT NOT NULL CHECK (observed_slot >= 0),
    observed_signature TEXT NOT NULL,
    instruction_index INTEGER NOT NULL CHECK (instruction_index >= 0),
    event_index INTEGER NOT NULL CHECK (event_index >= 0),
    decision_source TEXT NOT NULL,
    decision_version TEXT NOT NULL,
    token JSONB NOT NULL,
    source_work_id BIGINT
        REFERENCES observation_work (id) ON DELETE SET NULL,
    first_observed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (BTRIM(mint) <> ''),
    CHECK (BTRIM(market_address) <> ''),
    CHECK (quote_mint IS NULL OR BTRIM(quote_mint) <> ''),
    CHECK (BTRIM(observed_signature) <> ''),
    CHECK (BTRIM(decision_source) <> ''),
    CHECK (BTRIM(decision_version) <> ''),
    CHECK (token ->> 'mint' = mint),
    CHECK (token ->> 'stage' = stage)
);

CREATE INDEX discovery_tokens_stage_slot_idx
    ON discovery_tokens (stage, observed_slot DESC, mint);

CREATE INDEX discovery_tokens_source_slot_idx
    ON discovery_tokens (network, source_program, observed_slot DESC);

CREATE TABLE discovery_activity (
    network TEXT NOT NULL,
    source_program TEXT NOT NULL,
    venue TEXT NOT NULL,
    market_address TEXT NOT NULL,
    mint TEXT NOT NULL
        REFERENCES discovery_tokens (mint) ON DELETE CASCADE,
    quote_mint TEXT NOT NULL DEFAULT '',
    trades BIGINT NOT NULL DEFAULT 0 CHECK (trades >= 0),
    buys BIGINT NOT NULL DEFAULT 0 CHECK (buys >= 0),
    sells BIGINT NOT NULL DEFAULT 0 CHECK (sells >= 0),
    base_volume_units NUMERIC(39, 0) NOT NULL DEFAULT 0
        CHECK (base_volume_units >= 0),
    quote_volume_units NUMERIC(39, 0) NOT NULL DEFAULT 0
        CHECK (quote_volume_units >= 0),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (trades = buys + sells),
    CHECK (BTRIM(market_address) <> ''),
    CHECK (BTRIM(mint) <> ''),
    PRIMARY KEY (
        network,
        source_program,
        venue,
        market_address,
        mint,
        quote_mint
    )
);

CREATE TABLE discovery_traders (
    network TEXT NOT NULL,
    source_program TEXT NOT NULL,
    venue TEXT NOT NULL,
    market_address TEXT NOT NULL,
    mint TEXT NOT NULL,
    quote_mint TEXT NOT NULL DEFAULT '',
    wallet TEXT NOT NULL CHECK (BTRIM(wallet) <> ''),
    first_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (
        network,
        source_program,
        venue,
        market_address,
        mint,
        quote_mint,
        wallet
    ),
    FOREIGN KEY (
        network,
        source_program,
        venue,
        market_address,
        mint,
        quote_mint
    ) REFERENCES discovery_activity (
        network,
        source_program,
        venue,
        market_address,
        mint,
        quote_mint
    ) ON DELETE CASCADE
);

CREATE TABLE discovery_rejection_summaries (
    reason_code TEXT PRIMARY KEY CHECK (BTRIM(reason_code) <> ''),
    count BIGINT NOT NULL DEFAULT 0 CHECK (count > 0),
    last_seen_unix_ms BIGINT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX projection_events_discovery_sequence_idx
    ON projection_events (sequence DESC)
    WHERE projection_name = 'DISCOVERY';

CREATE TABLE pump_swap_pools (
    network TEXT NOT NULL,
    pool_address TEXT NOT NULL CHECK (BTRIM(pool_address) <> ''),
    base_mint TEXT NOT NULL CHECK (BTRIM(base_mint) <> ''),
    quote_mint TEXT NOT NULL CHECK (BTRIM(quote_mint) <> ''),
    observed_slot BIGINT NOT NULL CHECK (observed_slot >= 0),
    observed_signature TEXT NOT NULL CHECK (BTRIM(observed_signature) <> ''),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (network, pool_address),
    CHECK (base_mint <> quote_mint)
);

CREATE INDEX pump_swap_pools_base_mint_idx
    ON pump_swap_pools (network, base_mint);
