import type { Token } from "../types";
import { StatusBadge } from "../TokenBadges";
import { QualificationWindowEvidence } from "./QualificationWindowEvidence";

export interface OverviewTabProps {
  token: Token;
}

export function describeQualificationState(
  token: Pick<Token, "stage" | "qualification">,
): { detail: string; label: string } {
  if (token.stage === "QUALIFIED") {
    return {
      detail:
        "Passed the configured activity qualification. This is not a deterministic safety approval.",
      label: "Latest retained activity",
    };
  }
  if (token.stage === "APPROVED") {
    return {
      detail:
        "Promoted after an explicit deterministic safety pass. Review the available evidence before taking action.",
      label: "Approved stage",
    };
  }
  if (token.qualification?.decision === "REJECT") {
    return {
      detail:
        "The completed observation window failed one or more configured activity qualification rules.",
      label: "Qualification rejected",
    };
  }
  if (token.qualification?.decision === "UNKNOWN") {
    return {
      detail:
        "The observation window was incomplete, so the activity qualification result is unknown.",
      label: "Qualification unknown",
    };
  }
  return {
    detail:
      "Structurally decoded on-chain activity. The observation window is still awaiting qualification.",
    label: "Qualification pending",
  };
}

export function OverviewTab({ token }: OverviewTabProps) {
  const qualificationState = describeQualificationState(token);

  return (
    <>
      <div className="decision-card">
        <div className="decision-card__top">
          <StatusBadge status={token.stageLabel} />
          <span>Collector stage</span>
        </div>
        <p>{qualificationState.detail}</p>
        <div className="score-row">
          <div>
            <span>Source</span>
            <strong>{token.sourceProgram}</strong>
          </div>
          <div>
            <span>Venue</span>
            <strong>{token.primaryVenue}</strong>
          </div>
          <div>
            <span>Latest event</span>
            <strong>{token.lastEventKind}</strong>
          </div>
        </div>
      </div>

      {token.qualification !== null && (
        <QualificationWindowEvidence evidence={token.qualification} />
      )}

      <section className="inspector-section">
        <div className="section-title">
          <h3>Observation</h3>
          <span>Source coordinates</span>
        </div>
        <div className="metric-grid metric-grid--references">
          <div>
            <span>Observed slot</span>
            <strong>{token.observedSlot}</strong>
          </div>
          <div>
            <span>First observed</span>
            <strong>{token.firstObservedAt}</strong>
          </div>
          <div>
            <span>Last observed</span>
            <strong>{token.lastObservedAt}</strong>
          </div>
          <div>
            <span>Market address</span>
            <strong title={token.marketAddress}>
              {token.marketAddress}
            </strong>
          </div>
          {token.quoteMint && (
            <div>
              <span>Quote mint</span>
              <strong title={token.quoteMint}>{token.quoteMint}</strong>
            </div>
          )}
          <div>
            <span>Latest signature</span>
            <strong title={token.latestSignature}>
              {token.latestSignature}
            </strong>
          </div>
        </div>
      </section>

      <section className="inspector-section">
        <div className="section-title">
          <h3>Observed activity</h3>
          <span>{qualificationState.label}</span>
        </div>
        <div className="metric-grid">
          <div>
            <span>Trades</span>
            <strong>{token.activity.trades}</strong>
          </div>
          <div>
            <span>Unique traders</span>
            <strong>{token.activity.uniqueTraders}</strong>
          </div>
          <div>
            <span>Base units</span>
            <strong>{token.activity.baseVolumeUnits}</strong>
          </div>
          <div>
            <span>Quote units</span>
            <strong>{token.activity.quoteVolumeUnits}</strong>
          </div>
        </div>
        <div className="buy-sell-bar">
          <div
            style={{
              width: `${
                token.activity.buys + token.activity.sells > 0
                  ? (token.activity.buys /
                      (token.activity.buys + token.activity.sells)) *
                    100
                  : 0
              }%`,
            }}
          />
        </div>
        <div className="buy-sell-labels">
          <span>
            <i className="buy-dot" /> {token.activity.buys} buys
          </span>
          <span>
            {token.activity.sells} sells <i className="sell-dot" />
          </span>
        </div>
      </section>
    </>
  );
}
