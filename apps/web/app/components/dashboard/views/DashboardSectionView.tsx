"use client";

import type { ExecutionMode, SectionView } from "../types";
import type { BackendStatusViewModel } from "../../../lib/soldisco-api/viewModels";
import { AlertsView } from "./AlertsView";
import { ControlsView } from "./ControlsView";
import { OrdersView } from "./OrdersView";
import { PositionsView } from "./PositionsView";
import { ReplaysView } from "./ReplaysView";
import { StrategiesView } from "./StrategiesView";

export type DashboardSectionViewProps = {
  view: SectionView;
  backend: BackendStatusViewModel;
  apiPort: number;
  mode: ExecutionMode;
  onToggleStream: () => void;
  onConnectApi: (port: number) => boolean;
  onModeChange: (mode: ExecutionMode) => void;
  onWallet: () => void;
  onUpload: () => void;
};

function SectionContent({
  view,
  backend,
  apiPort,
  mode,
  onToggleStream,
  onConnectApi,
  onModeChange,
  onWallet,
  onUpload,
}: DashboardSectionViewProps) {
  switch (view) {
    case "positions":
      return <PositionsView mode={mode} onWallet={onWallet} />;
    case "orders":
      return <OrdersView mode={mode} />;
    case "alerts":
      return <AlertsView />;
    case "strategies":
      return <StrategiesView onUpload={onUpload} />;
    case "replays":
      return <ReplaysView />;
    case "controls":
      return (
        <ControlsView
          backend={backend}
          apiPort={apiPort}
          mode={mode}
          onToggleStream={onToggleStream}
          onConnectApi={onConnectApi}
          onModeChange={onModeChange}
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
