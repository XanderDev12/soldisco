ALTER TABLE chain_observations
    ADD COLUMN transaction_index BIGINT CHECK (transaction_index >= 0);

ALTER TABLE recovery_checkpoints
    ADD COLUMN last_transaction_index BIGINT
        CHECK (last_transaction_index >= 0);

CREATE TABLE database_network_binding (
    singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (singleton),
    network TEXT NOT NULL
        CHECK (network IN ('SOLANA_MAINNET', 'SOLANA_DEVNET')),
    bound_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

DO $$
DECLARE
    detected_network TEXT;
    detected_count INTEGER;
BEGIN
    SELECT MIN(network), COUNT(*)
      INTO detected_network, detected_count
      FROM (
        SELECT DISTINCT network FROM chain_observations
        UNION
        SELECT DISTINCT network FROM recovery_checkpoints
        UNION
        SELECT DISTINCT network FROM discovery_tokens
        UNION
        SELECT DISTINCT network FROM discovery_activity
        UNION
        SELECT DISTINCT network FROM discovery_traders
        UNION
        SELECT DISTINCT network FROM pump_swap_pools
      ) AS existing_networks;

    IF detected_count > 1 THEN
        RAISE EXCEPTION
            'existing database contains more than one Solana network';
    ELSIF detected_count = 1 THEN
        INSERT INTO database_network_binding (singleton, network)
        VALUES (TRUE, detected_network);
    END IF;
END
$$;

CREATE TABLE intake_quarantine (
    id BIGSERIAL PRIMARY KEY,
    network TEXT NOT NULL,
    source_program TEXT NOT NULL,
    slot BIGINT NOT NULL CHECK (slot >= 0),
    transaction_index BIGINT CHECK (transaction_index >= 0),
    signature TEXT NOT NULL CHECK (BTRIM(signature) <> ''),
    instruction_index INTEGER NOT NULL CHECK (instruction_index >= 0),
    event_index INTEGER NOT NULL CHECK (event_index >= 0),
    decoder_version TEXT NOT NULL
        CHECK (
            BTRIM(decoder_version) <> ''
            AND OCTET_LENGTH(decoder_version) <= 128
        ),
    reason_code TEXT NOT NULL
        CHECK (
            BTRIM(reason_code) <> ''
            AND OCTET_LENGTH(reason_code) <= 128
        ),
    reason_detail TEXT NOT NULL
        CHECK (OCTET_LENGTH(reason_detail) <= 2048),
    evidence_base64 TEXT NOT NULL
        CHECK (
            BTRIM(evidence_base64) <> ''
            AND OCTET_LENGTH(evidence_base64) <= 16384
            AND evidence_base64 !~ '[^A-Za-z0-9+/=]'
            AND MOD(OCTET_LENGTH(evidence_base64), 4) = 0
        ),
    occurrences BIGINT NOT NULL DEFAULT 1 CHECK (occurrences > 0),
    first_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (
        network,
        source_program,
        slot,
        signature,
        instruction_index,
        event_index
    )
);

CREATE INDEX intake_quarantine_retention_idx
    ON intake_quarantine (last_seen_at, id);

CREATE TABLE collection_gaps (
    id BIGSERIAL PRIMARY KEY,
    network TEXT NOT NULL,
    source_program TEXT NOT NULL,
    reason_code TEXT NOT NULL
        CHECK (
            BTRIM(reason_code) <> ''
            AND OCTET_LENGTH(reason_code) <= 128
        ),
    prior_checkpoint_slot BIGINT NOT NULL
        CHECK (prior_checkpoint_slot >= 0),
    prior_checkpoint_transaction_index BIGINT
        CHECK (prior_checkpoint_transaction_index >= 0),
    prior_checkpoint_signature TEXT NOT NULL
        CHECK (BTRIM(prior_checkpoint_signature) <> ''),
    detected_at TIMESTAMPTZ NOT NULL,
    details TEXT NOT NULL
        CHECK (BTRIM(details) <> '' AND OCTET_LENGTH(details) <= 2048),
    resolved_through_slot BIGINT CHECK (resolved_through_slot >= 0),
    resolved_through_transaction_index BIGINT
        CHECK (resolved_through_transaction_index >= 0),
    resolved_through_signature TEXT,
    resolved_at TIMESTAMPTZ,
    CHECK (
        (
            resolved_through_slot IS NULL
            AND resolved_through_transaction_index IS NULL
            AND resolved_through_signature IS NULL
            AND resolved_at IS NULL
        )
        OR
        (
            resolved_through_slot IS NOT NULL
            AND resolved_through_signature IS NOT NULL
            AND BTRIM(resolved_through_signature) <> ''
            AND resolved_at IS NOT NULL
        )
    )
);

CREATE UNIQUE INDEX collection_gaps_active_identity_idx
    ON collection_gaps (
        network,
        source_program,
        reason_code,
        prior_checkpoint_slot,
        prior_checkpoint_signature
    )
    WHERE resolved_through_slot IS NULL;

CREATE INDEX collection_gaps_active_source_idx
    ON collection_gaps (network, source_program, detected_at, id)
    WHERE resolved_through_slot IS NULL;

CREATE INDEX chain_observations_retention_idx
    ON chain_observations (first_seen_at, id);

CREATE INDEX observation_work_terminal_retention_idx
    ON observation_work (observation_id, finished_at)
    WHERE status IN ('COMPLETE', 'FAILED');

CREATE INDEX projection_events_retention_idx
    ON projection_events (created_at, sequence);

CREATE FUNCTION soldisco_enforce_bound_network()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    bound_network TEXT;
BEGIN
    SELECT network
      INTO bound_network
      FROM database_network_binding
      WHERE singleton = TRUE;

    IF bound_network IS NULL THEN
        RAISE EXCEPTION
            'database must be bound to a Solana network before ingestion';
    END IF;

    IF NEW.network <> bound_network THEN
        RAISE EXCEPTION
            'network % does not match database binding %',
            NEW.network,
            bound_network;
    END IF;

    RETURN NEW;
END
$$;

CREATE FUNCTION soldisco_prevent_network_rebinding()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION
            'database Solana network binding is immutable; use a separate database';
    END IF;

    IF NEW.network <> OLD.network THEN
        RAISE EXCEPTION
            'database Solana network binding is immutable; use a separate database';
    END IF;

    RETURN NEW;
END
$$;

CREATE TRIGGER database_network_binding_immutable
    BEFORE UPDATE OR DELETE ON database_network_binding
    FOR EACH ROW EXECUTE FUNCTION soldisco_prevent_network_rebinding();

CREATE TRIGGER chain_observations_bound_network
    BEFORE INSERT OR UPDATE OF network ON chain_observations
    FOR EACH ROW EXECUTE FUNCTION soldisco_enforce_bound_network();

CREATE TRIGGER recovery_checkpoints_bound_network
    BEFORE INSERT OR UPDATE OF network ON recovery_checkpoints
    FOR EACH ROW EXECUTE FUNCTION soldisco_enforce_bound_network();

CREATE TRIGGER discovery_tokens_bound_network
    BEFORE INSERT OR UPDATE OF network ON discovery_tokens
    FOR EACH ROW EXECUTE FUNCTION soldisco_enforce_bound_network();

CREATE TRIGGER discovery_activity_bound_network
    BEFORE INSERT OR UPDATE OF network ON discovery_activity
    FOR EACH ROW EXECUTE FUNCTION soldisco_enforce_bound_network();

CREATE TRIGGER discovery_traders_bound_network
    BEFORE INSERT OR UPDATE OF network ON discovery_traders
    FOR EACH ROW EXECUTE FUNCTION soldisco_enforce_bound_network();

CREATE TRIGGER pump_swap_pools_bound_network
    BEFORE INSERT OR UPDATE OF network ON pump_swap_pools
    FOR EACH ROW EXECUTE FUNCTION soldisco_enforce_bound_network();

CREATE TRIGGER intake_quarantine_bound_network
    BEFORE INSERT OR UPDATE OF network ON intake_quarantine
    FOR EACH ROW EXECUTE FUNCTION soldisco_enforce_bound_network();

CREATE TRIGGER collection_gaps_bound_network
    BEFORE INSERT OR UPDATE OF network ON collection_gaps
    FOR EACH ROW EXECUTE FUNCTION soldisco_enforce_bound_network();
