import type { Token } from "../types";

export interface RiskTabProps {
  token: Token;
}

export function RiskTab({ token }: RiskTabProps) {
  if (token.riskScore === null) {
    return (
      <>
        <div className="risk-summary">
          <div className="risk-orbit">
            <strong>—</strong>
            <span>/ 100</span>
          </div>
          <div>
            <span>DETERMINISTIC RISK</span>
            <h3>Not evaluated</h3>
            <p>
              The observe-all collector does not run safety thresholds or
              assign a risk value.
            </p>
          </div>
        </div>
        <div className="notice">
          Observation is not approval and does not indicate that a token is
          safe to trade.
        </div>
      </>
    );
  }

  return (
    <>
      <div className="risk-summary">
        <div
          className={`risk-orbit risk-orbit--${
            token.riskScore <= 35
              ? "low"
              : token.riskScore <= 60
                ? "medium"
                : "high"
          }`}
        >
          <strong>{token.riskScore}</strong>
          <span>/ 100</span>
        </div>
        <div>
          <span>DETERMINISTIC RISK</span>
          <h3>
            {token.riskScore <= 35
              ? "Low observed risk"
              : token.riskScore <= 60
                ? "Review required"
                : "Elevated risk"}
          </h3>
          <p>Based on the latest completed deterministic checks.</p>
        </div>
      </div>
      <div className="notice">
        Risk values are screening signals, not guarantees of safety or returns.
      </div>
    </>
  );
}
