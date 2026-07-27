export interface PositionCostInput {
  positionId: string;
  rawQuantity: string;
  remainingCostBasisSol: string;
  realizedPnlSol: string;
}

export interface ValuationInput {
  basis: "LAST_PRICE" | "EXECUTABLE_EXIT_QUOTE";
  grossValueSol: string;
  estimatedFeesSol: string;
  estimatedPriceImpactSol: string;
  observedAt: string;
  expiresAt: string | null;
}

export interface PnlResult {
  positionId: string;
  valuationBasis: "LAST_PRICE" | "EXECUTABLE_EXIT_QUOTE";
  grossValueSol: string;
  netValueSol: string;
  remainingCostBasisSol: string;
  unrealizedPnlSol: string;
  realizedPnlSol: string;
  calculatedAt: string;
  priceObservedAt: string;
  quoteExpiresAt: string | null;
}

export interface PnlCalculator {
  calculate(
    position: PositionCostInput,
    valuation: ValuationInput,
    calculatedAt: string,
  ): PnlResult;
}
