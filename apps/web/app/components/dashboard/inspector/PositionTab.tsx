import type { Token } from "../types";

export interface PositionTabProps {
  token: Token;
  onViewTrade: () => void;
}

export function PositionTab({ token, onViewTrade }: PositionTabProps) {
  return (
    <div className="empty-position">
      <span className="empty-position__icon">◎</span>
      <h3>Position data unavailable</h3>
      <p>Connect a wallet to load holdings for {token.symbol}.</p>
      <button type="button" onClick={onViewTrade}>
        View trade controls
      </button>
    </div>
  );
}
