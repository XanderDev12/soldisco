import type {
  KeyboardEvent,
  PointerEvent as ReactPointerEvent,
} from "react";
import type {
  ExecutionMode,
  LayoutKey,
  LayoutPreferences,
} from "./types";

type LayoutLimits = Record<LayoutKey, { min: number; max: number }>;

export interface PositionsTrayProps {
  mode: ExecutionMode;
  trayExpanded: boolean;
  layout: LayoutPreferences;
  layoutLimits: LayoutLimits;
  defaultLayout: LayoutPreferences;
  onToggleTray: () => void;
  onBeginResize: (
    key: LayoutKey,
    event: ReactPointerEvent<HTMLButtonElement>,
  ) => void;
  onResizeKey: (
    key: LayoutKey,
    event: KeyboardEvent<HTMLButtonElement>,
  ) => void;
  onSetLayoutValue: (key: LayoutKey, value: number) => void;
}

export function PositionsTray({
  mode,
  trayExpanded,
  layout,
  layoutLimits,
  defaultLayout,
  onToggleTray,
  onBeginResize,
  onResizeKey,
  onSetLayoutValue,
}: PositionsTrayProps) {
  return (
    <div
      id="positions-panel"
      className={`positions-tray ${trayExpanded ? "positions-tray--expanded" : ""}`}
    >
      <button
        type="button"
        className="resize-handle resize-handle--tray"
        role="separator"
        aria-label="Resize positions tray"
        aria-controls="positions-panel"
        aria-orientation="horizontal"
        aria-valuemin={layoutLimits.tray.min}
        aria-valuemax={layoutLimits.tray.max}
        aria-valuenow={layout.tray}
        onPointerDown={(event) => onBeginResize("tray", event)}
        onKeyDown={(event) => onResizeKey("tray", event)}
        onDoubleClick={() => onSetLayoutValue("tray", defaultLayout.tray)}
        title="Drag to resize · Double-click to reset"
      />
      <button
        type="button"
        className="positions-tray__label"
        onClick={onToggleTray}
        aria-expanded={trayExpanded}
        aria-controls="positions-tray-content"
      >
        <span className="positions-icon" aria-hidden="true">
          ↗
        </span>
        <span>
          <strong>Positions</strong>
          <small>No position data</small>
        </span>
        <span className="tray-chevron" aria-hidden="true">
          {trayExpanded ? "⌄" : "⌃"}
        </span>
      </button>
      <div id="positions-tray-content" className="positions-empty">
        Position data unavailable.
      </div>
      <span className="paper-tag">{mode.toUpperCase()}</span>
    </div>
  );
}
