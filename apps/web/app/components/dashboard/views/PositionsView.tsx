import { executionModePresentation } from "../executionModePresentation";
import type { ExecutionMode } from "../types";
import { SectionHeader } from "./SectionViewPrimitives";

type PositionsViewProps = {
  mode: ExecutionMode;
  onWallet: () => void;
};

export function PositionsView({ mode, onWallet }: PositionsViewProps) {
  const presentation = executionModePresentation[mode];
  const content = presentation.positions;

  return (
    <>
      <SectionHeader
        eyebrow={content.eyebrow}
        title={content.title}
        description={content.description}
        stats={content.stats}
      />
      <div className="section-view__body">
        <div className="section-card">
          <div className="section-card__head">
            <div>
              <h2>{content.recordTitle}</h2>
              <p>{content.recordDetail}</p>
            </div>
            <span className="status-chip">{content.status}</span>
          </div>
          <div className="section-empty">
            <span aria-hidden="true">◎</span>
            <h2>{content.emptyTitle}</h2>
            <p>{content.emptyDetail}</p>
            {presentation.requiresWallet && (
              <div className="control-card__actions">
                <button type="button" onClick={onWallet}>
                  Connect wallet
                </button>
              </div>
            )}
          </div>
        </div>
      </div>
    </>
  );
}
