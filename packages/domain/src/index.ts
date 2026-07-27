export type Chain = "solana";
export type MintAddress = string;
export type IsoDateTime = string;

export type PipelineStage =
  | "DISCOVERED"
  | "ENRICHING"
  | "FIRST_PASS_COMPLETE"
  | "STRATEGY_EVALUATED"
  | "ERROR";

export type FirstPassStatus = "PENDING" | "APPROVED" | "REJECTED" | "ERROR";
export type CheckOutcome = "PASS" | "FAIL" | "UNKNOWN";

export interface DeterministicCheckResult {
  checkId: string;
  outcome: CheckOutcome;
  reasonCode: string;
  summary: string;
  observedAt: IsoDateTime;
}

export interface FirstPassAssessment {
  status: FirstPassStatus;
  checks: DeterministicCheckResult[];
  completedAt: IsoDateTime | null;
}

export interface RiskAssessment {
  /** 0 is lowest observed risk; 100 is highest observed risk. */
  value: number;
  band: "LOW" | "MEDIUM" | "HIGH" | "CRITICAL";
  reasonCodes: string[];
  assessedAt: IsoDateTime;
}

export interface QualityRating {
  /** Discovery quality only; never a return prediction. */
  grade: "A" | "B" | "C" | "D" | "F" | "UNRATED";
  score: number | null;
  reasonCodes: string[];
  assessedAt: IsoDateTime | null;
}

export interface AiAdvisory {
  status: "PENDING" | "READY" | "UNAVAILABLE";
  summary: string | null;
  flags: string[];
  generatedAt: IsoDateTime | null;
  /** AI output is commentary and cannot approve a token or authorize a trade. */
  advisoryOnly: true;
}

export type StrategyEvaluationStatus = "MATCH" | "NO_MATCH" | "NOT_EVALUABLE" | "ERROR";

export interface StrategyDecisionSummary {
  strategyId: string;
  strategyVersion: string;
  status: StrategyEvaluationStatus;
  score: number | null;
  reasonCodes: string[];
  evaluatedAt: IsoDateTime;
}

export interface TokenIdentity {
  chain: Chain;
  mint: MintAddress;
  symbol: string | null;
  name: string | null;
}

export interface TokenStreamRow {
  token: TokenIdentity;
  discoveredAt: IsoDateTime;
  source: string;
  stage: PipelineStage;
  firstPass: FirstPassAssessment;
  risk: RiskAssessment | null;
  rating: QualityRating | null;
  aiAdvisory: AiAdvisory | null;
  strategies: StrategyDecisionSummary[];
  dataFreshnessAt: IsoDateTime;
}
