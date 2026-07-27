export const walletConditionedMomentumManifest = {
  schemaVersion: 1,
  strategyId: "wallet-conditioned-momentum",
  version: "0.1.0-draft",
  displayName: "Wallet-Conditioned Momentum",
  description: "Inactive advisory strategy pending a trusted-wallet dataset.",
  mode: "ADVISORY",
  requiredDependencies: ["trusted-wallet-set"],
} as const;

export interface WalletConditionedMomentumInput {
  mint: string;
  evaluatedAt: string;
  trustedWalletSetId: string | null;
  trustedWalletActivityAvailable: boolean;
  momentumFeaturesAvailable: boolean;
}

export interface WalletConditionedMomentumResult {
  strategyId: "wallet-conditioned-momentum";
  strategyVersion: "0.1.0-draft";
  status: "NOT_EVALUABLE";
  score: null;
  reasonCodes: ["TRUSTED_WALLETS_NOT_CONFIGURED"];
  missingDependencies: ["trusted-wallet-set"];
  evaluatedAt: string;
}

export function evaluateWalletConditionedMomentum(
  input: WalletConditionedMomentumInput,
): WalletConditionedMomentumResult {
  return {
    strategyId: "wallet-conditioned-momentum",
    strategyVersion: "0.1.0-draft",
    status: "NOT_EVALUABLE",
    score: null,
    reasonCodes: ["TRUSTED_WALLETS_NOT_CONFIGURED"],
    missingDependencies: ["trusted-wallet-set"],
    evaluatedAt: input.evaluatedAt,
  };
}
