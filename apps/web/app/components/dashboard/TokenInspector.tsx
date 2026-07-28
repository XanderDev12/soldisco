import { OverviewTab } from "./inspector/OverviewTab";
import { PositionTab } from "./inspector/PositionTab";
import { RiskTab } from "./inspector/RiskTab";
import { SignalsTab } from "./inspector/SignalsTab";
import { TradeTicket } from "./TradeTicket";
import type { KeyboardEvent } from "react";
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
          receives its first observed token.
        </div>
      </aside>
    );
  }

  const tokenLabel = token.name ?? token.symbol ?? "Unknown token";
  const tokenMonogram =
    token.symbol?.slice(0, 1) ?? token.name?.slice(0, 1) ?? "?";
  const activeTabId = `inspector-tab-${tab.toLowerCase()}`;
  const activePanelId = `inspector-tabpanel-${tab.toLowerCase()}`;

  function handleTabKeyDown(
    event: KeyboardEvent<HTMLButtonElement>,
    index: number,
  ) {
    let nextIndex = index;
    if (event.key === "ArrowRight") {
      nextIndex = (index + 1) % inspectorTabs.length;
    } else if (event.key === "ArrowLeft") {
      nextIndex =
        (index - 1 + inspectorTabs.length) % inspectorTabs.length;
    } else if (event.key === "Home") {
      nextIndex = 0;
    } else if (event.key === "End") {
      nextIndex = inspectorTabs.length - 1;
    } else {
      return;
    }

    event.preventDefault();
    const nextTab = inspectorTabs[nextIndex];
    onTabChange(nextTab);
    window.requestAnimationFrame(() => {
      document.getElementById(
        `inspector-tab-${nextTab.toLowerCase()}`,
      )?.focus();
    });
  }

  return (
    <aside id="inspector-panel" className="inspector">
      <div className="inspector__head">
        <div className="token-name token-name--large">
          <span className="token-logo token-logo--large">
            {tokenMonogram}
          </span>
          <div>
            <strong>{tokenLabel}</strong>
            <span>
              {token.symbol && `${token.symbol} · `}
              {token.mint}
            </span>
          </div>
        </div>
        <div className="price-block">
          <strong>{token.stageLabel}</strong>
        </div>
      </div>

      <div className="inspector-tabs" role="tablist">
        {inspectorTabs.map((item, index) => (
          <button
            type="button"
            role="tab"
            id={`inspector-tab-${item.toLowerCase()}`}
            aria-controls={`inspector-tabpanel-${item.toLowerCase()}`}
            aria-selected={tab === item}
            tabIndex={tab === item ? 0 : -1}
            className={tab === item ? "is-active" : ""}
            key={item}
            onClick={() => onTabChange(item)}
            onKeyDown={(event) => handleTabKeyDown(event, index)}
          >
            {item}
          </button>
        ))}
      </div>

      <div
        className="inspector__body"
        role="tabpanel"
        id={activePanelId}
        aria-labelledby={activeTabId}
        tabIndex={0}
      >
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
