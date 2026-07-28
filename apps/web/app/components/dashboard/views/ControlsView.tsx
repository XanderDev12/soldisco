import { executionModePresentation } from "../executionModePresentation";
import type { ExecutionMode } from "../types";
import { SectionHeader } from "./SectionViewPrimitives";
import type { BackendStatusViewModel } from "../../../lib/soldisco-api/viewModels";

type ControlsViewProps = {
  backend: BackendStatusViewModel;
  mode: ExecutionMode;
  onToggleStream: () => void;
  onModeChange: (mode: ExecutionMode) => void;
};

export function ControlsView({
  backend,
  mode,
  onToggleStream,
  onModeChange,
}: ControlsViewProps) {
  const modeDetail = executionModePresentation[mode].controlsDetail;

  return (
    <>
      <SectionHeader
        eyebrow="WORKSPACE SETTINGS"
        title="Controls"
        description="Manage local stream and execution-mode preferences."
        stats={["STREAM RATE", "FIRST-PASS RATE", "MEDIAN LATENCY"]}
      />
      <div className="section-view__body section-view__grid">
        <article className="control-card">
          <div className="section-card__head">
            <div>
              <h2>Stream</h2>
              <p>Discovery intake control</p>
            </div>
            <span className="status-chip">
              {backend.stream.statusLabel}
            </span>
          </div>
          <div className="status-list">
            <div>
              <span>Current state</span>
              <strong>{backend.stream.statusLabel}</strong>
            </div>
            <div>
              <span>Local API</span>
              <strong>{backend.connectionLabel}</strong>
            </div>
            <div>
              <span>Database</span>
              <strong>{backend.database ?? "Unavailable"}</strong>
            </div>
            <div>
              <span>Live updates</span>
              <strong>{backend.liveUpdates.toLowerCase()}</strong>
            </div>
            <div>
              <span>Discovery policy</span>
              <strong>Observe all · no threshold</strong>
            </div>
          </div>
          <div className="control-card__actions">
            <button
              type="button"
              aria-pressed={backend.stream.shouldStop}
              onClick={onToggleStream}
              disabled={!backend.stream.canCommand}
            >
              {backend.stream.buttonLabel}
            </button>
          </div>
        </article>

        <article className="control-card">
          <div className="section-card__head">
            <div>
              <h2>Execution mode</h2>
              <p>{modeDetail}</p>
            </div>
            <span className="status-chip">{mode}</span>
          </div>
          <div
            className="control-card__actions"
            role="group"
            aria-label="Execution mode"
          >
            {(["Paper", "Live"] as const).map((item) => (
              <button
                type="button"
                key={item}
                aria-pressed={mode === item}
                onClick={() => onModeChange(item)}
              >
                {item}
              </button>
            ))}
          </div>
        </article>

      </div>
    </>
  );
}
