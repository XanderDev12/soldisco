export type TradeSide = "BUY" | "SELL";
export type ExecutionMode = "PAPER" | "LIVE";

export interface TradeProposal {
  proposalId: string;
  mint: string;
  side: TradeSide;
  mode: ExecutionMode;
  requestedAmount: string;
  requestedAmountUnit: "SOL" | "TOKEN" | "PERCENT_OF_POSITION";
  source: "MANUAL" | "STRATEGY";
  strategyEvaluationId: string | null;
  createdAt: string;
  /** A proposal has no execution authority. */
  requiresUserApproval: true;
}

export interface TradeIntent {
  intentId: string;
  proposalId: string;
  walletAddress: string;
  mint: string;
  side: TradeSide;
  mode: ExecutionMode;
  amount: string;
  amountUnit: "SOL" | "TOKEN";
  maxSlippageBps: number;
  approvedBy: string;
  approvedAt: string;
  idempotencyKey: string;
}

export interface ExecutionQuote {
  quoteId: string;
  venueId: string;
  inputMint: string;
  outputMint: string;
  inputAmount: string;
  expectedOutputAmount: string;
  minimumOutputAmount: string;
  priceImpactBps: number | null;
  feeAmount: string | null;
  expiresAt: string;
}

export interface ExecutionOrder {
  orderId: string;
  intentId: string;
  quote: ExecutionQuote;
  status: "CREATED" | "POLICY_APPROVED" | "POLICY_REJECTED" | "EXPIRED" | "CANCELLED";
  createdAt: string;
}

export interface PreparedTransaction {
  preparedTransactionId: string;
  orderId: string;
  unsignedMessageBase64: string;
  recentBlockhash: string;
  lastValidBlockHeight: number;
  validationDigest: string;
  simulationStatus: "NOT_RUN" | "PASSED" | "FAILED";
  preparedAt: string;
}

export interface SubmittedTransaction {
  submittedTransactionId: string;
  preparedTransactionId: string;
  signature: string;
  status:
    | "SUBMITTED"
    | "CONFIRMED"
    | "FINALIZED"
    | "FAILED"
    | "EXPIRED"
    | "UNKNOWN";
  submittedAt: string;
  confirmedAt: string | null;
  errorCode: string | null;
}

export interface ExecutionFill {
  fillId: string;
  submittedTransactionId: string;
  orderId: string;
  inputAmount: string;
  outputAmount: string;
  feeAmount: string;
  slot: number;
  recordedAt: string;
}
