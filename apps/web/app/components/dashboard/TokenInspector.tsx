import { OverviewTab } from "./inspector/OverviewTab";
import { PositionTab } from "./inspector/PositionTab";
import { RiskTab } from "./inspector/RiskTab";
import { SignalsTab } from "./inspector/SignalsTab";
import { TradeTicket } from "./TradeTicket";
import type {
  ExecutionMode,
  InspectorTab,
  Token,
  TradeSide,
} from "./types";

type TokenInspectorProps = {
  token: Token | null;
  tab: InspectorTab;
  onTabChange: (tab: InspectorTab) => void;
  orderSide: TradeSide;
  onOrderSideChange: (side: TradeSide) => void;
  orderAmount: string;
  onOrderAmountChange: (amount: string) => void;
  mode: ExecutionMode;
  onWallet: () => void;
};

const inspectorTabs: InspectorTab[] = [
  "Overview",
  "Risk",
  "Signals",
  "Trade",
  "Position",
];

export function TokenInspector({
  token,
  tab,
  onTabChange,
  orderSide,
  onOrderSideChange,
  orderAmount,
  onOrderAmountChange,
  mode,
  onWallet,
}: TokenInspectorProps) {
  if (!token) {
    return (
      <aside id="inspector-panel" className="inspector">
        <div className="empty-state">
          No token selected. The inspector will populate when the stream
          receives its first approved token.
        </div>
      </aside>
    );
  }

  return (
    <aside id="inspector-panel" className="inspector">
      <div className="inspector__head">
        <div className="token-name token-name--large">
          <span
            className={`token-logo token-logo--large token-logo--${token.id}`}
          >
            {token.symbol.slice(0, 1)}
          </span>
          <div>
            <strong>{token.name}</strong>
            <span>
              {token.symbol} · {token.mint}
            </span>
          </div>
        </div>
        <div className="inspector__head-actions">
          <button type="button" aria-label="More actions">···</button>
        </div>
        <div className="price-block">
          <strong>{token.price}</strong>
          <span className={token.moveUp ? "positive" : "negative"}>
            {token.move} <small>2m</small>
          </span>
        </div>
      </div>

      <div className="inspector-tabs" role="tablist">
        {inspectorTabs.map((item) => (
          <button
            type="button"
            role="tab"
            aria-selected={tab === item}
            className={tab === item ? "is-active" : ""}
            key={item}
            onClick={() => onTabChange(item)}
          >
            {item}
          </button>
        ))}
      </div>

      <div className="inspector__body">
        {tab === "Overview" && <OverviewTab token={token} />}
        {tab === "Risk" && <RiskTab token={token} />}
        {tab === "Signals" && <SignalsTab />}
        {tab === "Trade" && (
          <TradeTicket
            token={token}
            side={orderSide}
            setSide={onOrderSideChange}
            amount={orderAmount}
            setAmount={onOrderAmountChange}
            mode={mode}
            onWallet={onWallet}
          />
        )}
        {tab === "Position" && (
          <PositionTab
            token={token}
            mode={mode}
            onViewTrade={() => onTabChange("Trade")}
          />
        )}
      </div>

      <div className="inspector__actions">
        <button
          type="button"
          className="primary-action"
          onClick={() => onTabChange("Trade")}
        >
          Review trade
        </button>
      </div>
    </aside>
  );
}
