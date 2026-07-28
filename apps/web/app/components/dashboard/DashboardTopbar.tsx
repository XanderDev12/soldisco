import type { ExecutionMode } from "./types";

export interface DashboardTopbarProps {
  sideNavOpen: boolean;
  streamRunning: boolean;
  mode: ExecutionMode;
  onOpenMobileNav: () => void;
  onToggleStream: () => void;
  onResetLayout: () => void;
  onUpload: () => void;
  onModeChange: (mode: ExecutionMode) => void;
  onWallet: () => void;
}

export function DashboardTopbar({
  sideNavOpen,
  streamRunning,
  mode,
  onOpenMobileNav,
  onToggleStream,
  onResetLayout,
  onUpload,
  onModeChange,
  onWallet,
}: DashboardTopbarProps) {
  return (
    <>
      <header className="topbar">
        <button
          type="button"
          className="mobile-menu"
          onClick={onOpenMobileNav}
          aria-label="Open navigation"
          aria-controls="navigation-panel"
          aria-expanded={sideNavOpen}
        >
          ☰
        </button>

        <div className="strategy-control">
          <span className="strategy-control__label">STRATEGY</span>
          <span className="strategy-control__icon">⌁</span>
          <select aria-label="Active strategy" disabled>
            <option>No strategy active</option>
          </select>
          <span className="strategy-control__state">
            <i className="offline-dot" />
            Inactive
          </span>
        </div>

        <div className="topbar__actions">
          <button
            type="button"
            className={`stream-toggle ${streamRunning ? "stream-toggle--stop" : ""}`}
            onClick={onToggleStream}
            aria-describedby="stream-status"
          >
            <span aria-hidden="true">{streamRunning ? "■" : "▶"}</span>
            {streamRunning ? "Stop stream" : "Start stream"}
          </button>
          <span id="stream-status" className="sr-only" aria-live="polite">
            {streamRunning
              ? "Stream active. Waiting for a discovery source."
              : "Stream stopped."}
          </span>
          <button
            type="button"
            className="layout-reset"
            onClick={onResetLayout}
            title="Reset panel sizes"
          >
            ↺ Layout
          </button>
          <button type="button" className="upload-button" onClick={onUpload}>
            <span>＋</span>
            Upload strategy
          </button>

          <div className="mode-switch" aria-label="Execution mode">
            {(["Paper", "Live"] as const).map((item) => (
              <button
                type="button"
                key={item}
                className={mode === item ? "is-active" : ""}
                onClick={() => onModeChange(item)}
              >
                {item}
              </button>
            ))}
          </div>

          <button type="button" className="wallet-button" onClick={onWallet}>
            <span className="wallet-button__icon">▰</span>
            Connect wallet
          </button>
        </div>
      </header>

      {mode === "Live" && (
        <div className="guard-banner">
          <span>LIVE MODE PREVIEW</span>
          Execution remains locked until wallet and policy modules are added.
        </div>
      )}
    </>
  );
}
