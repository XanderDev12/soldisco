import type { Token } from "../types";

export interface RiskTabProps {
  token: Token;
}

export function RiskTab({ token }: RiskTabProps) {
  return (
    <>
      <div className="risk-summary">
        <div
          className={`risk-orbit risk-orbit--${
            token.risk <= 35
              ? "low"
              : token.risk <= 60
                ? "medium"
                : "high"
          }`}
        >
          <strong>{token.risk}</strong>
          <span>/ 100</span>
        </div>
        <div>
          <span>DETERMINISTIC RISK</span>
          <h3>
            {token.risk <= 35
              ? "Low observed risk"
              : token.risk <= 60
                ? "Review required"
                : "Elevated risk"}
          </h3>
          <p>Based on the latest completed deterministic checks.</p>
        </div>
      </div>
      <section className="inspector-section">
        <div className="section-title">
          <h3>First-pass checks</h3>
          <span>{token.checks.length} checks</span>
        </div>
        <div className="check-list">
          {token.checks.map((check) => (
            <div key={check}>
              <span className="check-pass">✓</span>
              <span>{check}</span>
              <small>Passed</small>
            </div>
          ))}
        </div>
      </section>
      <div className="notice">
        Risk values are screening signals, not guarantees of safety or returns.
      </div>
    </>
  );
}
