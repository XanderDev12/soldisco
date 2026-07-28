import { executionModePresentation } from "../executionModePresentation";
import type { ExecutionMode, Token } from "../types";

export interface PositionTabProps {
  token: Token;
  mode: ExecutionMode;
  onViewTrade: () => void;
}

export function PositionTab({
  token,
  mode,
  onViewTrade,
}: PositionTabProps) {
  const presentation = executionModePresentation[mode].inspectorPosition;

  return (
    <div className="empty-position">
      <span className="empty-position__icon">◎</span>
      <h3>{presentation.emptyTitle}</h3>
      <p>{presentation.emptyDetail(token.symbol)}</p>
      <button type="button" onClick={onViewTrade}>
        {presentation.action}
      </button>
    </div>
  );
}
