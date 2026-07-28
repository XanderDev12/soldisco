CREATE TABLE prefilter_defaults (
    singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (singleton),
    revision BIGINT NOT NULL DEFAULT 1 CHECK (revision > 0),
    max_event_age_ms BIGINT NOT NULL
        CHECK (max_event_age_ms BETWEEN 1000 AND 300000),
    observation_window_ms BIGINT NOT NULL
        CHECK (observation_window_ms BETWEEN 1000 AND 3600000),
    max_active_windows BIGINT NOT NULL
        CHECK (max_active_windows BETWEEN 1 AND 100000),
    rpc_requests_per_second BIGINT NOT NULL
        CHECK (rpc_requests_per_second BETWEEN 1 AND 1000),
    rpc_max_in_flight BIGINT NOT NULL
        CHECK (rpc_max_in_flight BETWEEN 1 AND 128),
    rpc_request_timeout_ms BIGINT NOT NULL
        CHECK (rpc_request_timeout_ms BETWEEN 1 AND 300000),
    rpc_rate_limit_cooldown_ms BIGINT NOT NULL
        CHECK (rpc_rate_limit_cooldown_ms BETWEEN 100 AND 300000),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (rpc_request_timeout_ms <= max_event_age_ms),
    CHECK (observation_window_ms >= rpc_request_timeout_ms)
);

-- The Rust server inserts this singleton after validating the environment.
-- This lets environment values seed a database exactly once without silently
-- overwriting later operator changes.
