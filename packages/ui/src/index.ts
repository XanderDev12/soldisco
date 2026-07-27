export interface StreamRowViewModel {
  mint: string;
  symbol: string;
  name: string;
  discoveredAtLabel: string;
  stageLabel: string;
  firstPass: {
    status: "PENDING" | "APPROVED" | "REJECTED" | "ERROR";
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

export interface StrategyToggleViewModel {
  strategyId: string;
  displayName: string;
  version: string;
  enabled: boolean;
  emphasized: boolean;
  status: "READY" | "NOT_EVALUABLE" | "ERROR";
  statusLabel: string;
}

export interface TradeTicketViewModel {
  state: "CLOSED" | "DRAFT" | "QUOTING" | "REVIEW" | "AWAITING_SIGNATURE" | "SUBMITTED" | "ERROR";
  mode: "PAPER" | "LIVE";
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
  requiresWalletConfirmation: true;
}

export interface HeldPositionViewModel {
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

export interface CommandCenterViewModel {
  activeStrategyId: string | null;
  strategies: StrategyToggleViewModel[];
  stream: StreamRowViewModel[];
  selectedMint: string | null;
  tradeTicket: TradeTicketViewModel;
  heldPositions: HeldPositionViewModel[];
  executionMode: "PAPER" | "LIVE";
  executionPaused: boolean;
}
