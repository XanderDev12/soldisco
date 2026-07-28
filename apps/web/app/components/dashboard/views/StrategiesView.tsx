import { SectionHeader } from "./SectionViewPrimitives";

type StrategiesViewProps = {
  onUpload: () => void;
};

export function StrategiesView({ onUpload }: StrategiesViewProps) {
  return (
    <>
      <SectionHeader
        eyebrow="STRATEGY WORKSPACE"
        title="Strategies"
        description="Review validated strategy manifests and activation state."
        stats={["VALIDATED", "ACTIVE", "NEEDS REVIEW"]}
      />
      <div className="section-view__body">
        <div className="section-card">
          <div className="section-card__head">
            <div>
              <h2>Strategy library</h2>
              <p>Validated manifests</p>
            </div>
            <div className="control-card__actions">
              <button type="button" onClick={onUpload}>
                Upload strategy
              </button>
            </div>
          </div>
          <div className="table-scroll">
            <table className="section-table">
              <thead>
                <tr>
                  <th scope="col">Strategy</th>
                  <th scope="col">Version</th>
                  <th scope="col">Validation</th>
                  <th scope="col">Replay</th>
                  <th scope="col">State</th>
                </tr>
              </thead>
              <tbody />
            </table>
          </div>
          <div className="section-empty">
            <span aria-hidden="true">⌘</span>
            <h2>No validated strategies</h2>
            <p>Upload and validate a strategy before it can be activated.</p>
          </div>
        </div>
      </div>
    </>
  );
}
