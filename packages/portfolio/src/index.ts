export interface PositionLot {
  lotId: string;
  sourceFillId: string;
  acquiredAt: string;
  rawQuantity: string;
  remainingRawQuantity: string;
  costBasisSol: string;
  originatingStrategyId: string | null;
  originatingSignalId: string | null;
}

export interface Position {
  positionId: string;
  walletAddress: string;
  mint: string;
  status: "OPEN" | "CLOSED" | "RECONCILIATION_REQUIRED";
  rawQuantity: string;
  spendableRawQuantity: string;
  costBasisSol: string;
  realizedPnlSol: string;
  lots: PositionLot[];
  openedAt: string;
  closedAt: string | null;
  reconciledAt: string;
}

export interface PositionValuation {
  positionId: string;
  markValueSol: string | null;
  executableExitValueSol: string | null;
  estimatedExitPriceImpactBps: number | null;
  quoteExpiresAt: string | null;
  valuedAt: string;
}

export interface PortfolioSnapshot {
  walletAddress: string;
  solBalance: string;
  positions: Position[];
  totalMarkValueSol: string | null;
  totalExecutableExitValueSol: string | null;
  capturedAt: string;
}

export interface PortfolioReader {
  getPosition(walletAddress: string, mint: string): Promise<Position | null>;
  getSnapshot(walletAddress: string): Promise<PortfolioSnapshot>;
}
