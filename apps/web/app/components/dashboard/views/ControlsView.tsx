import type { ExecutionMode } from "../types";
import { SectionHeader } from "./SectionViewPrimitives";

type ControlsViewProps = {
  streamRunning: boolean;
  mode: ExecutionMode;
  onToggleStream: () => void;
  onModeChange: (mode: ExecutionMode) => void;
  onResetLayout: () => void;
};

export function ControlsView({
  streamRunning,
  mode,
  onToggleStream,
  onModeChange,
  onResetLayout,
}: ControlsViewProps) {
  return (
    <>
      <SectionHeader
        eyebrow="WORKSPACE SETTINGS"
        title="Controls"
        description="Manage local stream, execution-mode, and layout preferences."
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
              {streamRunning ? "Active · no source" : "Stopped"}
            </span>
          </div>
          <div className="status-list">
            <div>
              <span>Current state</span>
              <strong>{streamRunning ? "Active" : "Stopped"}</strong>
            </div>
            <div>
              <span>Discovery source</span>
              <strong>Not connected</strong>
            </div>
            <div>
              <span>Deterministic gate</span>
              <strong>Not running</strong>
            </div>
          </div>
          <div className="control-card__actions">
            <button
              type="button"
              aria-pressed={streamRunning}
              onClick={onToggleStream}
            >
              {streamRunning ? "Stop stream" : "Start stream"}
            </button>
          </div>
        </article>

        <article className="control-card">
          <div className="section-card__head">
            <div>
              <h2>Execution mode</h2>
              <p>Interface review mode</p>
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

        <article className="control-card">
          <div className="section-card__head">
            <div>
              <h2>Panel layout</h2>
              <p>Device-local panel sizing</p>
            </div>
            <span className="status-chip">Saved locally</span>
          </div>
          <div className="control-card__actions">
            <button type="button" onClick={onResetLayout}>
              Reset panel sizes
            </button>
          </div>
        </article>
      </div>
    </>
  );
}
