export interface DiscoveryRowViewModel {
  mint: string;
  symbol: string;
  name: string;
  discoveredAtLabel: string;
  stageLabel: string;
  firstPass: {
    status: "APPROVED";
    label: string;
  };
  risk: {
    value: number | null;
    tone: "neutral" | "healthy" | "caution" | "danger";
  };
  rating: "A" | "B" | "C" | "D" | "F" | "UNRATED";
  strategyMatch: "STRONG" | "MODERATE" | "NONE" | "NOT_EVALUABLE";
  momentumLabel: string;
  held: boolean;
  updatedAt: string;
}

export interface ScreeningSummaryViewModel {
  pending: number;
  approved: number;
  rejected: number;
  ratePerMinute: number | null;
  updatedAt: string | null;
}

export interface RejectionLogEntryViewModel {
  rejectionId: string;
  mint: string;
  symbol: string | null;
  reasonCodes: string[];
  rejectedAt: string;
}

export interface StrategyToggleViewModel {
  strategyId: string;
  displayName: string;
  version: string;
  enabled: boolean;
  emphasized: boolean;
  status: "READY" | "NOT_EVALUABLE" | "ERROR";
  statusLabel: string;
}

interface TradeTicketBaseViewModel {
  mint: string | null;
  side: "BUY" | "SELL";
  amount: string;
  quoteId: string | null;
  expectedReceived: string | null;
  minimumReceived: string | null;
  slippageBps: number;
  priceImpactBps: number | null;
  totalFeesLabel: string | null;
  quoteExpiresAt: string | null;
  simulationStatus: "NOT_RUN" | "PASSED" | "FAILED";
}

export type TradeTicketViewModel =
  | (TradeTicketBaseViewModel & {
      mode: "PAPER";
      state:
        | "CLOSED"
        | "DRAFT"
        | "QUOTING"
        | "REVIEW"
        | "RECORDED"
        | "ERROR";
      requiresWalletConfirmation: false;
    })
  | (TradeTicketBaseViewModel & {
      mode: "LIVE";
      state:
        | "CLOSED"
        | "DRAFT"
        | "QUOTING"
        | "REVIEW"
        | "AWAITING_SIGNATURE"
        | "SUBMITTED"
        | "ERROR";
      requiresWalletConfirmation: true;
    });

interface HeldPositionBaseViewModel {
  positionId: string;
  mint: string;
  symbol: string;
  quantityLabel: string;
  costBasisLabel: string;
  markValueLabel: string | null;
  executableExitValueLabel: string | null;
  unrealizedPnlLabel: string | null;
  realizedPnlLabel: string;
  riskValue: number | null;
  positionAgeLabel: string;
  valuedAt: string | null;
}

export type HeldPositionViewModel =
  | (HeldPositionBaseViewModel & { mode: "PAPER" })
  | (HeldPositionBaseViewModel & { mode: "LIVE" });

interface CommandCenterBaseViewModel {
  activeStrategyId: string | null;
  strategies: StrategyToggleViewModel[];
  discovery: DiscoveryRowViewModel[];
  screening: ScreeningSummaryViewModel;
  rejectionLog: RejectionLogEntryViewModel[];
  selectedMint: string | null;
  executionPaused: boolean;
}

export type CommandCenterViewModel =
  | (CommandCenterBaseViewModel & {
      executionMode: "PAPER";
      tradeTicket: Extract<TradeTicketViewModel, { mode: "PAPER" }>;
      heldPositions: Array<
        Extract<HeldPositionViewModel, { mode: "PAPER" }>
      >;
    })
  | (CommandCenterBaseViewModel & {
      executionMode: "LIVE";
      tradeTicket: Extract<TradeTicketViewModel, { mode: "LIVE" }>;
      heldPositions: Array<
        Extract<HeldPositionViewModel, { mode: "LIVE" }>
      >;
    });
