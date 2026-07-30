import type {
  DiscoveryMode,
  DiscoveryStage,
  DiscoveryWindowSummary,
  StreamStatus,
} from "./contracts";
import type { LiveConnectionStatus } from "./events";

export type BackendConnectionStatus =
  | "IDLE"
  | "CONNECTING"
  | "CONNECTED"
  | "UNAVAILABLE"
  | "LOCAL_ONLY";

export type StreamCommand = "START" | "STOP";

export type StreamControlViewModel = {
  status: StreamStatus | null;
  statusLabel: string;
  eyebrow: string;
  detail: string;
  indicatorClass: "live-dot" | "paused-dot" | "offline-dot";
  requestedRunning: boolean;
  running: boolean;
  command: StreamCommand | null;
  canCommand: boolean;
  buttonLabel: string;
  shouldStop: boolean;
};

export type BackendStatusViewModel = {
  apiBaseUrl: string;
  connection: BackendConnectionStatus;
  connectionLabel: string;
  overall: string | null;
  database: string | null;
  stream: StreamControlViewModel;
  liveUpdates: LiveConnectionStatus;
  errorCode: string | null;
  errorMessage: string | null;
};

export type DiscoveryActivityViewModel = {
  trades: number;
  buys: number;
  sells: number;
  uniqueTraders: number;
  baseVolumeUnits: string;
  quoteVolumeUnits: string;
};

export type DiscoveryTokenViewModel = {
  id: string;
  mint: string;
  name: string | null;
  symbol: string | null;
  primaryVenue: string;
  marketAddress: string;
  quoteMint: string | null;
  sourceProgram: string;
  stage: DiscoveryStage;
  stageLabel: "Observed" | "Qualified" | "Approved";
  lastEventKind: string;
  observedSlot: number;
  firstObservedAt: string;
  lastObservedAt: string;
  latestSignature: string;
  activity: DiscoveryActivityViewModel;
  qualification: DiscoveryWindowSummary | null;
  riskScore: number | null;
  opportunityScore: number | null;
};

export type ScreeningSummaryViewModel = {
  mode: DiscoveryMode;
  observed: number | null;
  pending: number | null;
  approved: number | null;
  rejected: number | null;
  qualified: number | null;
  qualificationPending: number | null;
  qualificationRejected: number | null;
  qualificationUnknown: number | null;
  processingFailures: number | null;
  ratePerMinute: number | null;
};

export type RejectionSummaryViewModel = {
  reasonCode: string;
  count: number;
  lastSeenAt: string;
};

export type DiscoveryViewModel = {
  sequence: number;
  mode: DiscoveryMode;
  tokens: DiscoveryTokenViewModel[];
  tokensTotal: number;
  tokensTruncated: boolean;
  summary: ScreeningSummaryViewModel;
  rejectionReasons: RejectionSummaryViewModel[];
};
