"use client";

import type { ExecutionMode, SectionView } from "../types";
import { AlertsView } from "./AlertsView";
import { ControlsView } from "./ControlsView";
import { InitialApprovalView } from "./InitialApprovalView";
import { OrdersView } from "./OrdersView";
import { PositionsView } from "./PositionsView";
import { ReplaysView } from "./ReplaysView";
import { StrategiesView } from "./StrategiesView";
import { WatchlistView } from "./WatchlistView";

export type DashboardSectionViewProps = {
  view: SectionView;
  streamRunning: boolean;
  mode: ExecutionMode;
  onToggleStream: () => void;
  onModeChange: (mode: ExecutionMode) => void;
  onWallet: () => void;
  onUpload: () => void;
  onResetLayout: () => void;
};

function SectionContent({
  view,
  streamRunning,
  mode,
  onToggleStream,
  onModeChange,
  onWallet,
  onUpload,
  onResetLayout,
}: DashboardSectionViewProps) {
  switch (view) {
    case "initial-approval":
      return <InitialApprovalView />;
    case "watchlist":
      return <WatchlistView />;
    case "positions":
      return <PositionsView onWallet={onWallet} />;
    case "orders":
      return <OrdersView />;
    case "alerts":
      return <AlertsView />;
    case "strategies":
      return <StrategiesView onUpload={onUpload} />;
    case "replays":
      return <ReplaysView />;
    case "controls":
      return (
        <ControlsView
          streamRunning={streamRunning}
          mode={mode}
          onToggleStream={onToggleStream}
          onModeChange={onModeChange}
          onResetLayout={onResetLayout}
        />
      );
    default:
      return view satisfies never;
  }
}

export function DashboardSectionView(props: DashboardSectionViewProps) {
  return (
    <section
      id="dashboard-view"
      className="section-view"
      aria-labelledby="view-title"
    >
      <SectionContent {...props} />
    </section>
  );
}
