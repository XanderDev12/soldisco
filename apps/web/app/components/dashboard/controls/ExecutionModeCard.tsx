import { executionModePresentation } from "../executionModePresentation";
import type { ExecutionMode } from "../types";

type ExecutionModeCardProps = {
  mode: ExecutionMode;
  onModeChange: (mode: ExecutionMode) => void;
};

export function ExecutionModeCard({
  mode,
  onModeChange,
}: ExecutionModeCardProps) {
  return (
    <article className="control-card">
      <div className="section-card__head">
        <div>
          <h2>Execution mode</h2>
          <p>{executionModePresentation[mode].controlsDetail}</p>
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
  );
}
