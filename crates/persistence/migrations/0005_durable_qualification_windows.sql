-- Collector throughput and candidate qualification are independent state.
-- Preserve the old operational counts while giving candidate decisions their
-- own rebuildable counters.
ALTER TABLE discovery_projection_state
    RENAME COLUMN pending TO queued_facts;

ALTER TABLE discovery_projection_state
    RENAME COLUMN rejected TO processing_failures;

ALTER TABLE discovery_projection_state
    ADD COLUMN qualified BIGINT NOT NULL DEFAULT 0 CHECK (qualified >= 0),
    ADD COLUMN qualification_pending BIGINT NOT NULL DEFAULT 0
        CHECK (qualification_pending >= 0),
    ADD COLUMN qualification_rejected BIGINT NOT NULL DEFAULT 0
        CHECK (qualification_rejected >= 0),
    ADD COLUMN qualification_unknown BIGINT NOT NULL DEFAULT 0
        CHECK (qualification_unknown >= 0);

ALTER TABLE discovery_projection_state
    DROP CONSTRAINT discovery_projection_state_mode_check;

ALTER TABLE discovery_projection_state
    ADD CONSTRAINT discovery_projection_state_mode_check
        CHECK (mode IN ('OBSERVE_ALL', 'QUALIFIED_ONLY', 'APPROVED_ONLY'));

ALTER TABLE discovery_tokens
    DROP CONSTRAINT discovery_tokens_stage_check;

ALTER TABLE discovery_tokens
    ADD CONSTRAINT discovery_tokens_stage_check
        CHECK (stage IN ('OBSERVED', 'QUALIFIED', 'APPROVED'));

-- Releases before qualification could contain an APPROVED test projection,
-- but no bounded-window PASS provenance existed yet. Fail closed on upgrade:
-- preserve the candidate and its evidence while demoting that unproven stage.
UPDATE discovery_tokens
SET stage = 'OBSERVED',
    decision_source = 'QUALIFICATION_V1_MIGRATION',
    decision_version = 'legacy-stage-demoted',
    token = JSONB_SET(
        JSONB_SET(token, '{stage}', '"OBSERVED"'::JSONB, TRUE),
        '{qualification}',
        'null'::JSONB,
        TRUE
    ),
    updated_at = NOW()
WHERE stage = 'APPROVED';

-- Existing OBSERVED JSON predates the optional field. Store it explicitly as
-- well as accepting its absence in the Rust contract for rolling upgrades.
UPDATE discovery_tokens
SET token = JSONB_SET(token, '{qualification}', 'null'::JSONB, TRUE),
    updated_at = NOW()
WHERE NOT token ? 'qualification';

UPDATE discovery_projection_state
SET approved = 0,
    qualified = 0,
    updated_at = NOW()
WHERE singleton = TRUE;

ALTER TABLE discovery_rejection_summaries
    RENAME TO discovery_processing_failure_summaries;

CREATE TABLE qualification_rejection_summaries (
    reason_code TEXT PRIMARY KEY CHECK (BTRIM(reason_code) <> ''),
    count BIGINT NOT NULL DEFAULT 0 CHECK (count > 0),
    last_seen_unix_ms BIGINT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE qualification_unknown_summaries (
    reason_code TEXT PRIMARY KEY CHECK (BTRIM(reason_code) <> ''),
    count BIGINT NOT NULL DEFAULT 0 CHECK (count > 0),
    last_seen_unix_ms BIGINT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Qualification settings are append-only. Every window pins both the revision
-- and the complete value snapshot, so changing Controls cannot rewrite an
-- already-open candidate's rules.
CREATE TABLE qualification_defaults_revisions (
    revision BIGSERIAL PRIMARY KEY,
    minimum_trades BIGINT NOT NULL CHECK (minimum_trades BETWEEN 1 AND 10000),
    minimum_unique_traders BIGINT NOT NULL
        CHECK (minimum_unique_traders BETWEEN 1 AND 10000),
    minimum_buys BIGINT NOT NULL CHECK (minimum_buys BETWEEN 0 AND 10000),
    minimum_sells BIGINT NOT NULL CHECK (minimum_sells BETWEEN 0 AND 10000),
    minimum_native_quote_volume_units NUMERIC(16, 0) NOT NULL
        CHECK (
            minimum_native_quote_volume_units BETWEEN 0 AND 9007199254740991
        ),
    minimum_stable_quote_volume_units NUMERIC(16, 0) NOT NULL
        CHECK (
            minimum_stable_quote_volume_units BETWEEN 0 AND 9007199254740991
        ),
    maximum_single_wallet_quote_share_bps INTEGER NOT NULL
        CHECK (
            maximum_single_wallet_quote_share_bps BETWEEN 1000 AND 10000
        ),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (minimum_unique_traders <= minimum_trades),
    CHECK (minimum_buys <= minimum_trades),
    CHECK (minimum_sells <= minimum_trades)
);

CREATE TABLE qualification_defaults_current (
    singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (singleton),
    revision BIGINT NOT NULL UNIQUE
        REFERENCES qualification_defaults_revisions (revision)
);

-- No row is inserted by SQL. The Rust server validates and seeds the safe
-- initial revision once, exactly like Prefilter Defaults.

CREATE TABLE discovery_windows (
    id BIGSERIAL PRIMARY KEY,
    opening_observation_id BIGINT UNIQUE
        REFERENCES chain_observations (id) ON DELETE SET NULL,
    opening_signature TEXT NOT NULL CHECK (BTRIM(opening_signature) <> ''),
    opening_instruction_index INTEGER NOT NULL
        CHECK (opening_instruction_index >= 0),
    opening_event_index INTEGER NOT NULL CHECK (opening_event_index >= 0),
    network TEXT NOT NULL,
    source_program TEXT NOT NULL,
    venue TEXT NOT NULL,
    market_address TEXT NOT NULL CHECK (BTRIM(market_address) <> ''),
    mint TEXT NOT NULL CHECK (BTRIM(mint) <> ''),
    quote_mint TEXT NOT NULL DEFAULT '',
    target_kind TEXT NOT NULL
        CHECK (target_kind IN ('PUMP_MINT', 'PUMP_SWAP_POOL')),
    target_address TEXT NOT NULL CHECK (BTRIM(target_address) <> ''),
    opened_at_unix_ms BIGINT NOT NULL,
    closes_at_unix_ms BIGINT NOT NULL,
    collector_run_id TEXT NOT NULL CHECK (BTRIM(collector_run_id) <> ''),
    prefilter_revision BIGINT NOT NULL CHECK (prefilter_revision > 0),
    prefilter_values JSONB NOT NULL,
    qualification_revision BIGINT NOT NULL
        REFERENCES qualification_defaults_revisions (revision),
    qualification_values JSONB NOT NULL,
    status TEXT NOT NULL DEFAULT 'ACTIVE'
        CHECK (status IN ('ACTIVE', 'FINALIZED')),
    completeness TEXT
        CHECK (completeness IS NULL OR completeness IN ('COMPLETE', 'INCOMPLETE')),
    completeness_reason TEXT,
    finalized_at_unix_ms BIGINT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (closes_at_unix_ms > opened_at_unix_ms),
    CHECK (
        (status = 'ACTIVE'
            AND completeness IS NULL
            AND finalized_at_unix_ms IS NULL)
        OR
        (status = 'FINALIZED'
            AND completeness IS NOT NULL
            AND finalized_at_unix_ms IS NOT NULL)
    ),
    CHECK (
        (
            completeness = 'INCOMPLETE'
            AND completeness_reason IS NOT NULL
            AND BTRIM(completeness_reason) <> ''
        )
        OR
        (completeness IS DISTINCT FROM 'INCOMPLETE'
            AND completeness_reason IS NULL)
    ),
    CHECK (
        (
            target_kind = 'PUMP_MINT'
            AND source_program = 'PUMP'
            AND venue = 'PUMP_BONDING_CURVE'
            AND target_address = mint
        )
        OR
        (
            target_kind = 'PUMP_SWAP_POOL'
            AND source_program = 'PUMP_SWAP'
            AND venue = 'PUMP_SWAP'
            AND target_address = market_address
        )
    ),
    UNIQUE (
        network,
        source_program,
        opening_signature,
        opening_instruction_index,
        opening_event_index,
        target_kind,
        target_address
    )
);

CREATE INDEX discovery_windows_active_close_idx
    ON discovery_windows (closes_at_unix_ms, id)
    WHERE status = 'ACTIVE';

CREATE INDEX discovery_windows_mint_created_idx
    ON discovery_windows (mint, created_at DESC);

CREATE TRIGGER discovery_windows_bound_network
    BEFORE INSERT OR UPDATE OF network ON discovery_windows
    FOR EACH ROW EXECUTE FUNCTION soldisco_enforce_bound_network();

CREATE TABLE discovery_window_observations (
    window_id BIGINT NOT NULL
        REFERENCES discovery_windows (id) ON DELETE CASCADE,
    observation_id BIGINT NOT NULL
        REFERENCES chain_observations (id) ON DELETE CASCADE,
    admitted_at_unix_ms BIGINT NOT NULL,
    PRIMARY KEY (window_id, observation_id)
);

CREATE INDEX discovery_window_observations_observation_idx
    ON discovery_window_observations (observation_id, window_id);

CREATE TABLE discovery_window_snapshots (
    id BIGSERIAL PRIMARY KEY,
    window_id BIGINT NOT NULL UNIQUE
        REFERENCES discovery_windows (id) ON DELETE CASCADE,
    feature_version TEXT NOT NULL CHECK (BTRIM(feature_version) <> ''),
    observation_count BIGINT NOT NULL CHECK (observation_count >= 0),
    snapshot JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE qualification_assessments (
    id BIGSERIAL PRIMARY KEY,
    window_id BIGINT NOT NULL UNIQUE
        REFERENCES discovery_windows (id) ON DELETE CASCADE,
    window_snapshot_id BIGINT NOT NULL UNIQUE
        REFERENCES discovery_window_snapshots (id) ON DELETE CASCADE,
    ruleset_revision BIGINT NOT NULL
        REFERENCES qualification_defaults_revisions (revision),
    decision TEXT NOT NULL CHECK (decision IN ('PASS', 'REJECT', 'UNKNOWN')),
    assessment JSONB NOT NULL,
    summary JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX qualification_assessments_decision_created_idx
    ON qualification_assessments (decision, created_at DESC);

CREATE TABLE qualification_rule_results (
    id BIGSERIAL PRIMARY KEY,
    assessment_id BIGINT NOT NULL
        REFERENCES qualification_assessments (id) ON DELETE CASCADE,
    rule_id TEXT NOT NULL CHECK (BTRIM(rule_id) <> ''),
    rule_version TEXT NOT NULL CHECK (BTRIM(rule_version) <> ''),
    decision TEXT NOT NULL CHECK (decision IN ('PASS', 'REJECT', 'UNKNOWN')),
    reason_code TEXT NOT NULL CHECK (BTRIM(reason_code) <> ''),
    evidence_reference TEXT,
    observed_slot BIGINT CHECK (observed_slot >= 0),
    UNIQUE (assessment_id, rule_id)
);

-- Existing observations predate durable windows and intentionally remain
-- visible only in OBSERVE_ALL. New successful qualification results will move
-- exact current candidates into QUALIFIED.
UPDATE discovery_projection_state
SET mode = 'QUALIFIED_ONLY',
    sequence = sequence + 1,
    updated_at = NOW()
WHERE singleton = TRUE;
