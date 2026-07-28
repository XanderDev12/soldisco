import type {
  DiscoveryActivity,
  DiscoveryCounters,
  DiscoveryMode,
  DiscoverySnapshot,
  DiscoveryStage,
  DiscoveryToken,
  DiscoveryWindowSummary,
  HealthResponse,
  IntegerSettingBounds,
  LiveEnvelope,
  LiveEvent,
  PrefilterDefaultsBounds,
  PrefilterDefaultsResponse,
  PrefilterDefaultsValues,
  QualificationDefaultsBounds,
  QualificationDefaultsResponse,
  QualificationDefaultsValues,
  RejectionSummary,
  SourceProgram,
  StreamCommandResponse,
  StreamStateResponse,
  StreamStatus,
  Venue,
} from "./contracts";

export class ContractParseError extends Error {
  readonly path: string;

  constructor(path: string, expected: string) {
    super(`Invalid API response at ${path}: expected ${expected}.`);
    this.name = "ContractParseError";
    this.path = path;
  }
}

const streamStatuses = [
  "STOPPED",
  "STARTING",
  "RUNNING",
  "DEGRADED",
  "ERROR",
  "STOPPING",
] as const satisfies readonly StreamStatus[];

const discoveryModes = [
  "OBSERVE_ALL",
  "QUALIFIED_ONLY",
  "APPROVED_ONLY",
] as const satisfies readonly DiscoveryMode[];

const discoveryStages = [
  "OBSERVED",
  "QUALIFIED",
  "APPROVED",
] as const satisfies readonly DiscoveryStage[];

const venues = [
  "PUMP_BONDING_CURVE",
  "PUMP_SWAP",
  "RAYDIUM_CPMM",
  "RAYDIUM_CLMM",
  "RAYDIUM_AMM_V4",
] as const satisfies readonly Venue[];

const sourcePrograms = [
  "PUMP",
  "PUMP_SWAP",
  "RAYDIUM_CPMM",
  "RAYDIUM_CLMM",
  "RAYDIUM_AMM_V4",
] as const satisfies readonly SourceProgram[];

const prefilterDefaultFields = [
  "max_event_age_ms",
  "observation_window_ms",
  "max_active_windows",
  "rpc_requests_per_second",
  "rpc_max_in_flight",
  "rpc_request_timeout_ms",
  "rpc_rate_limit_cooldown_ms",
] as const satisfies readonly (keyof PrefilterDefaultsValues)[];

const qualificationDefaultFields = [
  "minimum_trades",
  "minimum_unique_traders",
  "minimum_buys",
  "minimum_sells",
  "minimum_native_quote_volume_units",
  "minimum_stable_quote_volume_units",
  "maximum_single_wallet_quote_share_bps",
] as const satisfies readonly (keyof QualificationDefaultsValues)[];

function recordAt(value: unknown, path: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new ContractParseError(path, "an object");
  }
  return value as Record<string, unknown>;
}

function stringAt(value: unknown, path: string): string {
  if (typeof value !== "string") {
    throw new ContractParseError(path, "a string");
  }
  return value;
}

function nullableStringAt(value: unknown, path: string): string | null {
  if (value === null) return null;
  return stringAt(value, path);
}

function booleanAt(value: unknown, path: string): boolean {
  if (typeof value !== "boolean") {
    throw new ContractParseError(path, "a boolean");
  }
  return value;
}

function safeIntegerAt(
  value: unknown,
  path: string,
  minimum = Number.MIN_SAFE_INTEGER,
): number {
  if (
    typeof value !== "number" ||
    !Number.isSafeInteger(value) ||
    value < minimum
  ) {
    throw new ContractParseError(path, "a safe integer");
  }
  return value;
}

function nullableCountAt(value: unknown, path: string): number | null {
  if (value === null) return null;
  return safeIntegerAt(value, path, 0);
}

function nullableSafeIntegerAt(
  value: unknown,
  path: string,
): number | null {
  if (value === null) return null;
  return safeIntegerAt(value, path);
}

function nullableScoreAt(value: unknown, path: string): number | null {
  if (value === null) return null;
  const score = safeIntegerAt(value, path, 0);
  if (score > 100) {
    throw new ContractParseError(path, "an integer from 0 through 100");
  }
  return score;
}

function parseIntegerBounds(
  value: unknown,
  path: string,
): IntegerSettingBounds {
  const bounds = recordAt(value, path);
  const minimum = safeIntegerAt(bounds.minimum, `${path}.minimum`, 0);
  const maximum = safeIntegerAt(bounds.maximum, `${path}.maximum`, 0);
  if (maximum < minimum) {
    throw new ContractParseError(
      `${path}.maximum`,
      "an integer greater than or equal to minimum",
    );
  }
  return { minimum, maximum };
}

function parseQualificationBounds(
  value: unknown,
  path: string,
): QualificationDefaultsBounds {
  const source = recordAt(value, path);
  return Object.fromEntries(
    qualificationDefaultFields.map((field) => [
      field,
      parseIntegerBounds(source[field], `${path}.${field}`),
    ]),
  ) as QualificationDefaultsBounds;
}

function parseQualificationValues(
  value: unknown,
  bounds: QualificationDefaultsBounds,
  path: string,
): QualificationDefaultsValues {
  const source = recordAt(value, path);
  const values = Object.fromEntries(
    qualificationDefaultFields.map((field) => {
      const fieldPath = `${path}.${field}`;
      const parsed = safeIntegerAt(source[field], fieldPath, 0);
      const allowed = bounds[field];
      if (parsed < allowed.minimum || parsed > allowed.maximum) {
        throw new ContractParseError(
          fieldPath,
          `an integer from ${allowed.minimum} through ${allowed.maximum}`,
        );
      }
      return [field, parsed];
    }),
  ) as QualificationDefaultsValues;

  for (const field of [
    "minimum_unique_traders",
    "minimum_buys",
    "minimum_sells",
  ] as const) {
    if (values[field] > values.minimum_trades) {
      throw new ContractParseError(
        `${path}.${field}`,
        "a value no greater than minimum_trades",
      );
    }
  }

  return values;
}

function parsePrefilterBounds(
  value: unknown,
  path: string,
): PrefilterDefaultsBounds {
  const source = recordAt(value, path);
  return Object.fromEntries(
    prefilterDefaultFields.map((field) => [
      field,
      parseIntegerBounds(source[field], `${path}.${field}`),
    ]),
  ) as PrefilterDefaultsBounds;
}

function parsePrefilterValues(
  value: unknown,
  bounds: PrefilterDefaultsBounds,
  path: string,
): PrefilterDefaultsValues {
  const source = recordAt(value, path);
  const values = Object.fromEntries(
    prefilterDefaultFields.map((field) => {
      const fieldPath = `${path}.${field}`;
      const parsed = safeIntegerAt(source[field], fieldPath, 1);
      const allowed = bounds[field];
      if (parsed < allowed.minimum || parsed > allowed.maximum) {
        throw new ContractParseError(
          fieldPath,
          `an integer from ${allowed.minimum} through ${allowed.maximum}`,
        );
      }
      return [field, parsed];
    }),
  ) as PrefilterDefaultsValues;
  if (values.rpc_request_timeout_ms > values.max_event_age_ms) {
    throw new ContractParseError(
      `${path}.rpc_request_timeout_ms`,
      "a timeout no greater than max_event_age_ms",
    );
  }
  if (values.observation_window_ms < values.rpc_request_timeout_ms) {
    throw new ContractParseError(
      `${path}.observation_window_ms`,
      "a duration no shorter than rpc_request_timeout_ms",
    );
  }
  return values;
}

function enumAt<const Value extends string>(
  value: unknown,
  path: string,
  allowed: readonly Value[],
): Value {
  if (
    typeof value !== "string" ||
    !allowed.includes(value as Value)
  ) {
    throw new ContractParseError(path, allowed.join(" or "));
  }
  return value as Value;
}

function arrayAt<T>(
  value: unknown,
  path: string,
  parseItem: (item: unknown, itemPath: string) => T,
): T[] {
  if (!Array.isArray(value)) {
    throw new ContractParseError(path, "an array");
  }
  return value.map((item, index) => parseItem(item, `${path}[${index}]`));
}

export function parseStreamStatus(
  value: unknown,
  path = "$",
): StreamStatus {
  return enumAt(value, path, streamStatuses);
}

function parseActivity(value: unknown, path: string): DiscoveryActivity {
  const activity = recordAt(value, path);
  return {
    trades: safeIntegerAt(activity.trades, `${path}.trades`, 0),
    buys: safeIntegerAt(activity.buys, `${path}.buys`, 0),
    sells: safeIntegerAt(activity.sells, `${path}.sells`, 0),
    unique_traders: safeIntegerAt(
      activity.unique_traders,
      `${path}.unique_traders`,
      0,
    ),
    base_volume_units: stringAt(
      activity.base_volume_units,
      `${path}.base_volume_units`,
    ),
    quote_volume_units: stringAt(
      activity.quote_volume_units,
      `${path}.quote_volume_units`,
    ),
  };
}

function parseQualificationSummary(
  value: unknown,
  path: string,
): DiscoveryWindowSummary {
  const summary = recordAt(value, path);
  const maximumWalletShare = nullableCountAt(
    summary.maximum_single_wallet_quote_share_bps,
    `${path}.maximum_single_wallet_quote_share_bps`,
  );
  if (maximumWalletShare !== null && maximumWalletShare > 10_000) {
    throw new ContractParseError(
      `${path}.maximum_single_wallet_quote_share_bps`,
      "an integer from 0 through 10000 or null",
    );
  }

  return {
    window_revision: safeIntegerAt(
      summary.window_revision,
      `${path}.window_revision`,
      1,
    ),
    ruleset_revision: safeIntegerAt(
      summary.ruleset_revision,
      `${path}.ruleset_revision`,
      1,
    ),
    opened_unix_ms: safeIntegerAt(
      summary.opened_unix_ms,
      `${path}.opened_unix_ms`,
      0,
    ),
    closed_unix_ms: safeIntegerAt(
      summary.closed_unix_ms,
      `${path}.closed_unix_ms`,
      0,
    ),
    evaluated_unix_ms: safeIntegerAt(
      summary.evaluated_unix_ms,
      `${path}.evaluated_unix_ms`,
      0,
    ),
    decision: enumAt(summary.decision, `${path}.decision`, [
      "PASS",
      "REJECT",
      "UNKNOWN",
    ] as const),
    completeness: enumAt(summary.completeness, `${path}.completeness`, [
      "COMPLETE",
      "INCOMPLETE",
    ] as const),
    reason_codes: arrayAt(
      summary.reason_codes,
      `${path}.reason_codes`,
      stringAt,
    ),
    trades: safeIntegerAt(summary.trades, `${path}.trades`, 0),
    buys: safeIntegerAt(summary.buys, `${path}.buys`, 0),
    sells: safeIntegerAt(summary.sells, `${path}.sells`, 0),
    unique_traders: safeIntegerAt(
      summary.unique_traders,
      `${path}.unique_traders`,
      0,
    ),
    unique_buyers: safeIntegerAt(
      summary.unique_buyers,
      `${path}.unique_buyers`,
      0,
    ),
    unique_sellers: safeIntegerAt(
      summary.unique_sellers,
      `${path}.unique_sellers`,
      0,
    ),
    buy_base_volume_units: stringAt(
      summary.buy_base_volume_units,
      `${path}.buy_base_volume_units`,
    ),
    sell_base_volume_units: stringAt(
      summary.sell_base_volume_units,
      `${path}.sell_base_volume_units`,
    ),
    buy_quote_volume_units: stringAt(
      summary.buy_quote_volume_units,
      `${path}.buy_quote_volume_units`,
    ),
    sell_quote_volume_units: stringAt(
      summary.sell_quote_volume_units,
      `${path}.sell_quote_volume_units`,
    ),
    maximum_single_wallet_quote_share_bps: maximumWalletShare,
    price_change_bps: nullableSafeIntegerAt(
      summary.price_change_bps,
      `${path}.price_change_bps`,
    ),
    first_base_reserve_units: nullableStringAt(
      summary.first_base_reserve_units,
      `${path}.first_base_reserve_units`,
    ),
    first_quote_reserve_units: nullableStringAt(
      summary.first_quote_reserve_units,
      `${path}.first_quote_reserve_units`,
    ),
    latest_base_reserve_units: nullableStringAt(
      summary.latest_base_reserve_units,
      `${path}.latest_base_reserve_units`,
    ),
    latest_quote_reserve_units: nullableStringAt(
      summary.latest_quote_reserve_units,
      `${path}.latest_quote_reserve_units`,
    ),
  };
}

export function parseDiscoveryToken(
  value: unknown,
  path = "$",
): DiscoveryToken {
  const token = recordAt(value, path);
  const stage = enumAt(token.stage, `${path}.stage`, discoveryStages);
  const qualification =
    token.qualification === null || token.qualification === undefined
      ? null
      : parseQualificationSummary(
          token.qualification,
          `${path}.qualification`,
        );
  if (
    (stage === "QUALIFIED" || stage === "APPROVED") &&
    (qualification === null ||
      qualification.decision !== "PASS" ||
      qualification.completeness !== "COMPLETE")
  ) {
    throw new ContractParseError(
      `${path}.qualification`,
      "complete PASS evidence for a QUALIFIED or APPROVED token",
    );
  }

  return {
    mint: stringAt(token.mint, `${path}.mint`),
    name: nullableStringAt(token.name, `${path}.name`),
    symbol: nullableStringAt(token.symbol, `${path}.symbol`),
    primary_venue: enumAt(
      token.primary_venue,
      `${path}.primary_venue`,
      venues,
    ),
    market_address: stringAt(
      token.market_address,
      `${path}.market_address`,
    ),
    quote_mint: nullableStringAt(token.quote_mint, `${path}.quote_mint`),
    source_program: enumAt(
      token.source_program,
      `${path}.source_program`,
      sourcePrograms,
    ),
    stage,
    last_event_kind: stringAt(
      token.last_event_kind,
      `${path}.last_event_kind`,
    ),
    observed_slot: safeIntegerAt(
      token.observed_slot,
      `${path}.observed_slot`,
      0,
    ),
    first_observed_unix_ms: safeIntegerAt(
      token.first_observed_unix_ms,
      `${path}.first_observed_unix_ms`,
    ),
    last_observed_unix_ms: safeIntegerAt(
      token.last_observed_unix_ms,
      `${path}.last_observed_unix_ms`,
    ),
    latest_signature: stringAt(
      token.latest_signature,
      `${path}.latest_signature`,
    ),
    activity: parseActivity(token.activity, `${path}.activity`),
    qualification,
    risk_score: nullableScoreAt(token.risk_score, `${path}.risk_score`),
    opportunity_score: nullableScoreAt(
      token.opportunity_score,
      `${path}.opportunity_score`,
    ),
  };
}

function parseCounters(value: unknown, path: string): DiscoveryCounters {
  const counters = recordAt(value, path);
  return {
    observed: safeIntegerAt(counters.observed, `${path}.observed`, 0),
    pending: safeIntegerAt(counters.pending, `${path}.pending`, 0),
    approved: safeIntegerAt(counters.approved, `${path}.approved`, 0),
    rejected: safeIntegerAt(counters.rejected, `${path}.rejected`, 0),
    qualified: safeIntegerAt(
      counters.qualified,
      `${path}.qualified`,
      0,
    ),
    qualification_pending: safeIntegerAt(
      counters.qualification_pending,
      `${path}.qualification_pending`,
      0,
    ),
    qualification_rejected: safeIntegerAt(
      counters.qualification_rejected,
      `${path}.qualification_rejected`,
      0,
    ),
    qualification_unknown: safeIntegerAt(
      counters.qualification_unknown,
      `${path}.qualification_unknown`,
      0,
    ),
    processing_failures: safeIntegerAt(
      counters.processing_failures,
      `${path}.processing_failures`,
      0,
    ),
    flow_per_minute: nullableCountAt(
      counters.flow_per_minute,
      `${path}.flow_per_minute`,
    ),
  };
}

function parseRejection(value: unknown, path: string): RejectionSummary {
  const rejection = recordAt(value, path);
  return {
    reason_code: stringAt(rejection.reason_code, `${path}.reason_code`),
    count: safeIntegerAt(rejection.count, `${path}.count`, 0),
    last_seen_unix_ms: safeIntegerAt(
      rejection.last_seen_unix_ms,
      `${path}.last_seen_unix_ms`,
    ),
  };
}

export function parseDiscoverySnapshot(
  value: unknown,
): DiscoverySnapshot {
  const snapshot = recordAt(value, "$");
  return {
    sequence: safeIntegerAt(snapshot.sequence, "$.sequence", 0),
    mode: enumAt(snapshot.mode, "$.mode", discoveryModes),
    tokens: arrayAt(snapshot.tokens, "$.tokens", parseDiscoveryToken),
    tokens_total: safeIntegerAt(
      snapshot.tokens_total,
      "$.tokens_total",
      0,
    ),
    tokens_truncated: booleanAt(
      snapshot.tokens_truncated,
      "$.tokens_truncated",
    ),
    counters: parseCounters(snapshot.counters, "$.counters"),
    rejection_reasons: arrayAt(
      snapshot.rejection_reasons,
      "$.rejection_reasons",
      parseRejection,
    ),
  };
}

export function parseStreamState(value: unknown): StreamStateResponse {
  const state = recordAt(value, "$");
  return {
    status: parseStreamStatus(state.status, "$.status"),
    requested_running: booleanAt(
      state.requested_running,
      "$.requested_running",
    ),
  };
}

export function parseStreamCommand(
  value: unknown,
): StreamCommandResponse {
  const command = recordAt(value, "$");
  return {
    status: parseStreamStatus(command.status, "$.status"),
    changed: booleanAt(command.changed, "$.changed"),
  };
}

export function parseHealth(value: unknown): HealthResponse {
  const health = recordAt(value, "$");
  return {
    status: stringAt(health.status, "$.status"),
    database: stringAt(health.database, "$.database"),
    stream: parseStreamStatus(health.stream, "$.stream"),
  };
}

export function parsePrefilterDefaults(
  value: unknown,
): PrefilterDefaultsResponse {
  const response = recordAt(value, "$");
  const bounds = parsePrefilterBounds(response.bounds, "$.bounds");
  return {
    revision: safeIntegerAt(response.revision, "$.revision", 1),
    values: parsePrefilterValues(response.values, bounds, "$.values"),
    bounds,
    apply_requirement: enumAt(
      response.apply_requirement,
      "$.apply_requirement",
      ["STREAM_RESTART"] as const,
    ),
  };
}

export function parseQualificationDefaults(
  value: unknown,
): QualificationDefaultsResponse {
  const response = recordAt(value, "$");
  const bounds = parseQualificationBounds(response.bounds, "$.bounds");
  return {
    revision: safeIntegerAt(response.revision, "$.revision", 1),
    values: parseQualificationValues(response.values, bounds, "$.values"),
    bounds,
    apply_requirement: enumAt(
      response.apply_requirement,
      "$.apply_requirement",
      ["NEW_WINDOWS"] as const,
    ),
  };
}

function parseLiveEvent(value: unknown, path: string): LiveEvent {
  const event = recordAt(value, path);
  const type = stringAt(event.type, `${path}.type`);

  switch (type) {
    case "STREAM_STATUS_CHANGED":
      return {
        type,
        payload: parseStreamStatus(event.payload, `${path}.payload`),
      };
    case "DISCOVERY_PROJECTION_CHANGED":
    case "RESYNC_REQUIRED":
      return { type };
    default:
      throw new ContractParseError(
        `${path}.type`,
        "a supported live-event type",
      );
  }
}

export function parseLiveEnvelope(value: unknown): LiveEnvelope {
  const envelope = recordAt(value, "$");
  return {
    sequence: safeIntegerAt(envelope.sequence, "$.sequence", 0),
    event: parseLiveEvent(envelope.event, "$.event"),
  };
}
