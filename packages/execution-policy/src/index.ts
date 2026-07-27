export interface ExecutionPolicy {
  policyId: string;
  version: string;
  enabled: boolean;
  allowedModes: Array<"PAPER" | "LIVE">;
  /** Decimal SOL strings avoid floating-point arithmetic at execution boundaries. */
  maxTradeValueSol: string;
  maxDailyValueSol: string;
  maxPositionValueSol: string;
  maxSlippageBps: number;
  maxPriceImpactBps: number;
  minimumWalletReserveSol: string;
  requireFirstPassApproval: boolean;
  maximumRiskValue: number;
  killSwitchEngaged: boolean;
}

export interface ExecutionPolicyContext {
  intentId: string;
  mode: "PAPER" | "LIVE";
  firstPassStatus: "PENDING" | "APPROVED" | "REJECTED" | "ERROR";
  riskValue: number | null;
  proposedValueSol: string;
  currentPositionValueSol: string;
  dailyExecutedValueSol: string;
  walletBalanceSol: string;
  slippageBps: number;
  priceImpactBps: number | null;
  evaluatedAt: string;
}

export type ExecutionPolicyDecision =
  | {
      outcome: "APPROVED";
      policyId: string;
      policyVersion: string;
      reasonCodes: string[];
      evaluatedAt: string;
    }
  | {
      outcome: "REJECTED";
      policyId: string;
      policyVersion: string;
      reasonCodes: string[];
      evaluatedAt: string;
    };

export interface ExecutionPolicyEvaluator {
  evaluate(
    policy: ExecutionPolicy,
    context: ExecutionPolicyContext,
  ): Promise<ExecutionPolicyDecision>;
}
