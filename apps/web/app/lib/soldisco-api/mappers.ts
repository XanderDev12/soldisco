import type {
  DiscoverySnapshot,
  DiscoveryToken,
  HealthResponse,
  StreamStateResponse,
  StreamStatus,
} from "./contracts";
import type { SoldiscoApiError } from "./errors";
import type { LiveConnectionStatus } from "./events";
import type {
  BackendConnectionStatus,
  BackendStatusViewModel,
  DiscoveryTokenViewModel,
  DiscoveryViewModel,
  StreamCommand,
  StreamControlViewModel,
} from "./viewModels";

const statusLabels: Record<StreamStatus, string> = {
  STOPPED: "Stopped",
  STARTING: "Starting",
  RUNNING: "Running",
  DEGRADED: "Degraded",
  ERROR: "Error",
  STOPPING: "Stopping",
};

const statusDetails: Record<StreamStatus, string> = {
  STOPPED: "Start the stream when you are ready to observe candidates.",
  STARTING: "The collector is starting and restoring its local state.",
  RUNNING:
    "Structurally valid Pump and PumpSwap discoveries appear without an approval threshold.",
  DEGRADED:
    "Collection was requested, but one or more backend dependencies are impaired.",
  ERROR: "The collector stopped after encountering an error.",
  STOPPING: "The collector is stopping and saving its current state.",
};

const venueLabels: Record<DiscoveryToken["primary_venue"], string> = {
  PUMP_BONDING_CURVE: "Pump bonding curve",
  PUMP_SWAP: "PumpSwap",
  RAYDIUM_CPMM: "Raydium CPMM",
  RAYDIUM_CLMM: "Raydium CLMM",
  RAYDIUM_AMM_V4: "Raydium AMM v4",
};

const sourceLabels: Record<DiscoveryToken["source_program"], string> = {
  PUMP: "Pump",
  PUMP_SWAP: "PumpSwap",
  RAYDIUM_CPMM: "Raydium CPMM",
  RAYDIUM_CLMM: "Raydium CLMM",
  RAYDIUM_AMM_V4: "Raydium AMM v4",
};

function formatEventKind(value: string): string {
  return value
    .split("_")
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1).toLowerCase())
    .join(" ");
}

function formatInstant(unixMs: number): string {
  return new Date(unixMs).toISOString();
}

export function mapDiscoveryToken(
  token: DiscoveryToken,
): DiscoveryTokenViewModel {
  return {
    id: token.mint,
    mint: token.mint,
    name: token.name,
    symbol: token.symbol,
    primaryVenue: venueLabels[token.primary_venue],
    marketAddress: token.market_address,
    quoteMint: token.quote_mint,
    sourceProgram: sourceLabels[token.source_program],
    stage: token.stage,
    stageLabel:
      token.stage === "OBSERVED"
        ? "Observed"
        : token.stage === "QUALIFIED"
          ? "Qualified"
          : "Approved",
    lastEventKind: formatEventKind(token.last_event_kind),
    observedSlot: token.observed_slot,
    firstObservedAt: formatInstant(token.first_observed_unix_ms),
    lastObservedAt: formatInstant(token.last_observed_unix_ms),
    latestSignature: token.latest_signature,
    activity: {
      trades: token.activity.trades,
      buys: token.activity.buys,
      sells: token.activity.sells,
      uniqueTraders: token.activity.unique_traders,
      baseVolumeUnits: token.activity.base_volume_units,
      quoteVolumeUnits: token.activity.quote_volume_units,
    },
    qualification: token.qualification,
    riskScore: token.risk_score,
    opportunityScore: token.opportunity_score,
  };
}

export function mapDiscoverySnapshot(
  snapshot: DiscoverySnapshot,
): DiscoveryViewModel {
  return {
    sequence: snapshot.sequence,
    mode: snapshot.mode,
    tokens: snapshot.tokens.map(mapDiscoveryToken),
    tokensTotal: snapshot.tokens_total,
    tokensTruncated: snapshot.tokens_truncated,
    summary: {
      mode: snapshot.mode,
      observed: snapshot.counters.observed,
      pending: snapshot.counters.pending,
      approved: snapshot.counters.approved,
      rejected: snapshot.counters.rejected,
      qualified: snapshot.counters.qualified,
      qualificationPending: snapshot.counters.qualification_pending,
      qualificationRejected: snapshot.counters.qualification_rejected,
      qualificationUnknown: snapshot.counters.qualification_unknown,
      processingFailures: snapshot.counters.processing_failures,
      ratePerMinute: snapshot.counters.flow_per_minute,
    },
    rejectionReasons: snapshot.rejection_reasons.map((reason) => ({
      reasonCode: reason.reason_code,
      count: reason.count,
      lastSeenAt: formatInstant(reason.last_seen_unix_ms),
    })),
  };
}

export function mapStreamControl(
  state: StreamStateResponse | null,
  command: StreamCommand | null,
  canReachBackend: boolean,
): StreamControlViewModel {
  const status = state?.status ?? null;
  const shouldStop =
    state?.requested_running === true ||
    status === "STARTING" ||
    status === "RUNNING" ||
    status === "DEGRADED";
  const commandInFlight = command !== null;
  const statusLabel = status === null ? "Unavailable" : statusLabels[status];
  const transitionStatus =
    status === "STARTING" ||
    status === "STOPPING" ||
    status === "DEGRADED";

  return {
    status,
    statusLabel,
    eyebrow:
      status === null ? "BACKEND UNAVAILABLE" : `STREAM ${statusLabel.toUpperCase()}`,
    detail:
      status === null
        ? "Discovery remains empty until the local backend is reachable."
        : statusDetails[status],
    indicatorClass:
      status === "RUNNING"
        ? "live-dot"
        : transitionStatus
          ? "paused-dot"
          : "offline-dot",
    requestedRunning: state?.requested_running ?? false,
    running: status === "RUNNING",
    command,
    canCommand:
      canReachBackend &&
      state !== null &&
      !commandInFlight &&
      status !== "STARTING" &&
      status !== "STOPPING",
    buttonLabel:
      command === "START"
        ? "Starting…"
        : command === "STOP"
          ? "Stopping…"
          : shouldStop
            ? "Stop stream"
            : "Start stream",
    shouldStop,
  };
}

export function mapBackendStatus(input: {
  connection: BackendConnectionStatus;
  health: HealthResponse | null;
  healthFresh: boolean;
  stream: StreamStateResponse | null;
  streamFresh: boolean;
  liveUpdates: LiveConnectionStatus;
  command: StreamCommand | null;
  error: SoldiscoApiError | null;
}): BackendStatusViewModel {
  const connectionLabels: Record<BackendConnectionStatus, string> = {
    IDLE: "Not connected",
    CONNECTING: "Connecting",
    CONNECTED: "Connected",
    UNAVAILABLE: "Unavailable",
    LOCAL_ONLY: "Local access only",
  };
  const backendReachable = input.connection === "CONNECTED";
  const currentHealth =
    backendReachable && input.healthFresh ? input.health : null;
  const currentStream =
    backendReachable && input.streamFresh ? input.stream : null;

  return {
    connection: input.connection,
    connectionLabel: connectionLabels[input.connection],
    overall: currentHealth?.status ?? null,
    database: currentHealth?.database ?? null,
    stream: mapStreamControl(
      currentStream,
      input.command,
      backendReachable,
    ),
    liveUpdates: input.liveUpdates,
    errorCode: input.error?.code ?? null,
    errorMessage: input.error?.message ?? null,
  };
}
