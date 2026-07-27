export type StrategyInputKey =
  | "first_pass_status"
  | "risk_value"
  | "rating_score"
  | "price_change"
  | "trade_count"
  | "buy_sell_ratio"
  | "trusted_wallet_activity";

export interface StrategyCondition {
  input: StrategyInputKey;
  operator: "EQ" | "NE" | "GT" | "GTE" | "LT" | "LTE" | "IN";
  value: string | number | boolean | Array<string | number>;
  windowSeconds: number | null;
}

export interface StrategyManifest {
  schemaVersion: 1;
  strategyId: string;
  version: string;
  displayName: string;
  description: string;
  enabledByDefault: false;
  mode: "ADVISORY";
  conditions: StrategyCondition[];
  requiredDependencies: string[];
  metadata: Record<string, string>;
}

export interface StrategyUploadValidation {
  valid: boolean;
  manifest: StrategyManifest | null;
  errors: Array<{
    path: string;
    code: string;
    message: string;
  }>;
}

export interface StrategyVersionRecord {
  strategyId: string;
  version: string;
  contentHash: string;
  engineVersion: string;
  status: "UPLOADED" | "VALIDATED" | "REPLAY_PASSED" | "ACTIVE" | "INACTIVE" | "REJECTED";
  createdAt: string;
}
