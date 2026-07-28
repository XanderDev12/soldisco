export type StreamStatus =
  | "STOPPED"
  | "STARTING"
  | "RUNNING"
  | "DEGRADED"
  | "ERROR"
  | "STOPPING";

export type DiscoveryMode =
  | "OBSERVE_ALL"
  | "QUALIFIED_ONLY"
  | "APPROVED_ONLY";
export type DiscoveryStage = "OBSERVED" | "QUALIFIED" | "APPROVED";

export type Venue =
  | "PUMP_BONDING_CURVE"
  | "PUMP_SWAP"
  | "RAYDIUM_CPMM"
  | "RAYDIUM_CLMM"
  | "RAYDIUM_AMM_V4";

export type SourceProgram =
  | "PUMP"
  | "PUMP_SWAP"
  | "RAYDIUM_CPMM"
  | "RAYDIUM_CLMM"
  | "RAYDIUM_AMM_V4";

export type DiscoveryActivity = {
  trades: number;
  buys: number;
  sells: number;
  unique_traders: number;
  base_volume_units: string;
  quote_volume_units: string;
};

export type QualificationDecision = "PASS" | "REJECT" | "UNKNOWN";
export type ObservationCompleteness = "COMPLETE" | "INCOMPLETE";

export type DiscoveryWindowSummary = {
  window_revision: number;
  ruleset_revision: number;
  opened_unix_ms: number;
  closed_unix_ms: number;
  evaluated_unix_ms: number;
  decision: QualificationDecision;
  completeness: ObservationCompleteness;
  reason_codes: string[];
  trades: number;
  buys: number;
  sells: number;
  unique_traders: number;
  unique_buyers: number;
  unique_sellers: number;
  buy_base_volume_units: string;
  sell_base_volume_units: string;
  buy_quote_volume_units: string;
  sell_quote_volume_units: string;
  maximum_single_wallet_quote_share_bps: number | null;
  price_change_bps: number | null;
  first_base_reserve_units: string | null;
  first_quote_reserve_units: string | null;
  latest_base_reserve_units: string | null;
  latest_quote_reserve_units: string | null;
};

export type DiscoveryToken = {
  mint: string;
  name: string | null;
  symbol: string | null;
  primary_venue: Venue;
  market_address: string;
  quote_mint: string | null;
  source_program: SourceProgram;
  stage: DiscoveryStage;
  last_event_kind: string;
  observed_slot: number;
  first_observed_unix_ms: number;
  last_observed_unix_ms: number;
  latest_signature: string;
  activity: DiscoveryActivity;
  qualification: DiscoveryWindowSummary | null;
  risk_score: number | null;
  opportunity_score: number | null;
};

export type DiscoveryCounters = {
  observed: number;
  pending: number;
  approved: number;
  rejected: number;
  qualified: number;
  qualification_pending: number;
  qualification_rejected: number;
  qualification_unknown: number;
  processing_failures: number;
  flow_per_minute: number | null;
};

export type RejectionSummary = {
  reason_code: string;
  count: number;
  last_seen_unix_ms: number;
};

export type DiscoverySnapshot = {
  sequence: number;
  mode: DiscoveryMode;
  tokens: DiscoveryToken[];
  tokens_total: number;
  tokens_truncated: boolean;
  counters: DiscoveryCounters;
  rejection_reasons: RejectionSummary[];
};

export type StreamCommandResponse = {
  status: StreamStatus;
  changed: boolean;
};

export type StreamStateResponse = {
  status: StreamStatus;
  requested_running: boolean;
};

export type PrefilterDefaultsValues = {
  max_event_age_ms: number;
  observation_window_ms: number;
  max_active_windows: number;
  rpc_requests_per_second: number;
  rpc_max_in_flight: number;
  rpc_request_timeout_ms: number;
  rpc_rate_limit_cooldown_ms: number;
};

export type IntegerSettingBounds = {
  minimum: number;
  maximum: number;
};

export type PrefilterDefaultsBounds = {
  [Field in keyof PrefilterDefaultsValues]: IntegerSettingBounds;
};

export type PrefilterDefaultsResponse = {
  revision: number;
  values: PrefilterDefaultsValues;
  bounds: PrefilterDefaultsBounds;
  apply_requirement: "STREAM_RESTART";
};

export type UpdatePrefilterDefaultsRequest = {
  expected_revision: number;
  values: PrefilterDefaultsValues;
};

export type QualificationDefaultsValues = {
  minimum_trades: number;
  minimum_unique_traders: number;
  minimum_buys: number;
  minimum_sells: number;
  minimum_native_quote_volume_units: number;
  minimum_stable_quote_volume_units: number;
  maximum_single_wallet_quote_share_bps: number;
};

export type QualificationDefaultsBounds = {
  [Field in keyof QualificationDefaultsValues]: IntegerSettingBounds;
};

export type QualificationDefaultsResponse = {
  revision: number;
  values: QualificationDefaultsValues;
  bounds: QualificationDefaultsBounds;
  apply_requirement: "NEW_WINDOWS";
};

export type UpdateQualificationDefaultsRequest = {
  expected_revision: number;
  values: QualificationDefaultsValues;
};

export type HealthResponse = {
  status: string;
  database: string;
  stream: StreamStatus;
};

export type LiveEvent =
  | {
      type: "STREAM_STATUS_CHANGED";
      payload: StreamStatus;
    }
  | {
      type: "DISCOVERY_PROJECTION_CHANGED";
    }
  | {
      type: "RESYNC_REQUIRED";
    };

export type LiveEnvelope = {
  sequence: number;
  event: LiveEvent;
};

export type ApiErrorBody = {
  error: {
    code: string;
    message: string;
  };
};
