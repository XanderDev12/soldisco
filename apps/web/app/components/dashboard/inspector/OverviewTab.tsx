import type { Token } from "../types";
import {
  MatchBadge,
  RiskBadge,
  Sparkline,
  StatusBadge,
} from "../TokenBadges";

export interface OverviewTabProps {
  token: Token;
}

export function OverviewTab({ token }: OverviewTabProps) {
  return (
    <>
      <div className="decision-card">
        <div className="decision-card__top">
          <StatusBadge status={token.status} />
          <span>Deterministic first pass</span>
        </div>
        <p>{token.reason}</p>
        <div className="score-row">
          <div>
            <span>Risk value</span>
            <RiskBadge value={token.risk} />
          </div>
          <div>
            <span>Discovery rating</span>
            <strong
              className={`rating rating--large rating--${token.rating.charAt(0)}`}
            >
              {token.rating}
            </strong>
          </div>
          <div>
            <span>Strategy match</span>
            <MatchBadge match={token.match} />
          </div>
        </div>
      </div>

      <section className="inspector-section">
        <div className="section-title">
          <h3>Market snapshot</h3>
          <span>On-chain data</span>
        </div>
        <div className="metric-grid">
          <div>
            <span>Volume · 5m</span>
            <strong>{token.volume}</strong>
          </div>
          <div>
            <span>Liquidity</span>
            <strong>{token.liquidity}</strong>
          </div>
          <div>
            <span>Holders</span>
            <strong>{token.holders}</strong>
          </div>
          <div>
            <span>Age</span>
            <strong>{token.age}</strong>
          </div>
        </div>
      </section>

      <section className="inspector-section">
        <div className="section-title">
          <h3>Flow momentum</h3>
          <span>2 minute window</span>
        </div>
        <div className="flow-chart">
          <Sparkline values={token.momentum} positive={token.moveUp} />
        </div>
        <div className="buy-sell-bar">
          <div
            style={{
              width: `${
                token.buys + token.sells > 0
                  ? (token.buys / (token.buys + token.sells)) * 100
                  : 0
              }%`,
            }}
          />
        </div>
        <div className="buy-sell-labels">
          <span>
            <i className="buy-dot" /> {token.buys} buys
          </span>
          <span>
            {token.sells} sells <i className="sell-dot" />
          </span>
        </div>
      </section>
    </>
  );
}
