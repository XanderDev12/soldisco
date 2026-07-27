export interface EventEnvelope<TType extends string, TPayload> {
  eventId: string;
  eventType: TType;
  eventVersion: number;
  occurredAt: string;
  observedAt: string;
  correlationId: string;
  causationId: string | null;
  producer: string;
  payload: TPayload;
}

export interface TokenDiscoveredPayload {
  mint: string;
  source: string;
  symbol: string | null;
  name: string | null;
  sourceObservedAt: string;
}

export interface FirstPassCompletedPayload {
  mint: string;
  status: "APPROVED" | "REJECTED" | "ERROR";
  checks: Array<{
    checkId: string;
    outcome: "PASS" | "FAIL" | "UNKNOWN";
    reasonCode: string;
  }>;
}

export interface TokenScoredPayload {
  mint: string;
  risk: {
    value: number;
    band: "LOW" | "MEDIUM" | "HIGH" | "CRITICAL";
    reasonCodes: string[];
  };
  rating: {
    grade: "A" | "B" | "C" | "D" | "F" | "UNRATED";
    score: number | null;
    reasonCodes: string[];
  };
}

export interface AiAdvisoryCompletedPayload {
  mint: string;
  summary: string;
  flags: string[];
  advisoryOnly: true;
}

export interface StrategyEvaluatedPayload {
  mint: string;
  strategyId: string;
  strategyVersion: string;
  status: "MATCH" | "NO_MATCH" | "NOT_EVALUABLE" | "ERROR";
  score: number | null;
  reasonCodes: string[];
}

export type DiscoveryEvent =
  | EventEnvelope<"token.discovered", TokenDiscoveredPayload>
  | EventEnvelope<"token.first_pass.completed", FirstPassCompletedPayload>
  | EventEnvelope<"token.scored", TokenScoredPayload>
  | EventEnvelope<"token.ai_advisory.completed", AiAdvisoryCompletedPayload>
  | EventEnvelope<"strategy.evaluated", StrategyEvaluatedPayload>;

export interface TokenStreamRowContract {
  mint: string;
  symbol: string | null;
  name: string | null;
  discoveredAt: string;
  source: string;
  stage: "DISCOVERED" | "ENRICHING" | "FIRST_PASS_COMPLETE" | "STRATEGY_EVALUATED" | "ERROR";
  firstPassStatus: "PENDING" | "APPROVED" | "REJECTED" | "ERROR";
  riskValue: number | null;
  riskBand: "LOW" | "MEDIUM" | "HIGH" | "CRITICAL" | null;
  rating: "A" | "B" | "C" | "D" | "F" | "UNRATED";
  aiAdvisory: { status: "PENDING" | "READY" | "UNAVAILABLE"; summary: string | null } | null;
  strategyResults: Array<{
    strategyId: string;
    status: "MATCH" | "NO_MATCH" | "NOT_EVALUABLE" | "ERROR";
    score: number | null;
  }>;
  updatedAt: string;
}
