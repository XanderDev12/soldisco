import type { RefObject } from "react";
import type { StreamControlViewModel } from "../../lib/soldisco-api/viewModels";
import { ExecutionModeSwitch } from "./ExecutionModeSwitch";
import type { ExecutionMode } from "./types";

export interface DashboardTopbarProps {
  sideNavOpen: boolean;
  mobileMenuButtonRef: RefObject<HTMLButtonElement | null>;
  stream: StreamControlViewModel;
  mode: ExecutionMode;
  onOpenMobileNav: () => void;
  onToggleStream: () => void;
  onUpload: () => void;
  onModeChange: (mode: ExecutionMode) => void;
  onWallet: () => void;
}

export function DashboardTopbar({
  sideNavOpen,
  mobileMenuButtonRef,
  stream,
  mode,
  onOpenMobileNav,
  onToggleStream,
  onUpload,
  onModeChange,
  onWallet,
}: DashboardTopbarProps) {
  return (
    <>
      <header className="topbar">
        <button
          ref={mobileMenuButtonRef}
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
        </div>

        <div className="topbar__actions">
          <button
            type="button"
            className={`stream-toggle ${stream.shouldStop ? "stream-toggle--stop" : ""}`}
            onClick={onToggleStream}
            aria-describedby="stream-status"
            disabled={!stream.canCommand}
          >
            <span aria-hidden="true">{stream.shouldStop ? "■" : "▶"}</span>
            {stream.buttonLabel}
          </button>
          <span id="stream-status" className="sr-only" aria-live="polite">
            Stream {stream.statusLabel.toLowerCase()}.
            {stream.requestedRunning ? " Running requested." : ""}
          </span>
          <button
            type="button"
            className="upload-button"
            onClick={onUpload}
            aria-haspopup="dialog"
            aria-controls="strategy-upload-dialog"
          >
            <span>＋</span>
            Upload strategy
          </button>

          <ExecutionModeSwitch
            mode={mode}
            onModeChange={onModeChange}
          />

          {mode === "Live" && (
            <button type="button" className="wallet-button" onClick={onWallet}>
              <span className="wallet-button__icon">▰</span>
              Connect wallet
            </button>
          )}
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
