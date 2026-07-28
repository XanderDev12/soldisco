import type {
  DiscoveryActivity,
  DiscoveryCounters,
  DiscoveryMode,
  DiscoverySnapshot,
  DiscoveryStage,
  DiscoveryToken,
  HealthResponse,
  LiveEnvelope,
  LiveEvent,
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
  "APPROVED_ONLY",
] as const satisfies readonly DiscoveryMode[];

const discoveryStages = [
  "OBSERVED",
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

function nullableScoreAt(value: unknown, path: string): number | null {
  if (value === null) return null;
  const score = safeIntegerAt(value, path, 0);
  if (score > 100) {
    throw new ContractParseError(path, "an integer from 0 through 100");
  }
  return score;
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

export function parseDiscoveryToken(
  value: unknown,
  path = "$",
): DiscoveryToken {
  const token = recordAt(value, path);
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
    stage: enumAt(token.stage, `${path}.stage`, discoveryStages),
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
