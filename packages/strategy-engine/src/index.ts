export interface StrategyEvaluationContext {
  mint: string;
  evaluatedAt: string;
  firstPassStatus: "PENDING" | "APPROVED" | "REJECTED" | "ERROR";
  riskValue: number | null;
  ratingScore: number | null;
  numericFeatures: Readonly<Record<string, number | null>>;
  booleanFeatures: Readonly<Record<string, boolean | null>>;
  availableDependencies: readonly string[];
}

export type StrategyDecision =
  | {
      status: "MATCH";
      score: number;
      reasonCodes: string[];
    }
  | {
      status: "NO_MATCH";
      score: number | null;
      reasonCodes: string[];
    }
  | {
      status: "NOT_EVALUABLE";
      score: null;
      reasonCodes: string[];
      missingDependencies: string[];
    }
  | {
      status: "ERROR";
      score: null;
      reasonCodes: string[];
      errorCode: string;
    };

export interface StrategyEvaluation {
  strategyId: string;
  strategyVersion: string;
  engineVersion: string;
  mint: string;
  evaluatedAt: string;
  decision: StrategyDecision;
}

export interface StrategyEvaluator {
  readonly strategyId: string;
  readonly strategyVersion: string;
  evaluate(context: StrategyEvaluationContext): Promise<StrategyEvaluation>;
}
