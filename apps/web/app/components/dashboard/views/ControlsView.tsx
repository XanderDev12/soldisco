import type { ExecutionMode } from "../types";
import { ExecutionModeCard } from "../controls/ExecutionModeCard";
import { LocalApiConnectionCard } from "../controls/LocalApiConnectionCard";
import { PrefilterDefaultsSection } from "../controls/PrefilterDefaultsSection";
import { QualificationDefaultsSection } from "../controls/QualificationDefaultsSection";
import { StreamControlCard } from "../controls/StreamControlCard";
import { SectionHeader } from "./SectionViewPrimitives";
import type { BackendStatusViewModel } from "../../../lib/soldisco-api/viewModels";

type ControlsViewProps = {
  backend: BackendStatusViewModel;
  apiPort: number;
  mode: ExecutionMode;
  onToggleStream: () => void;
  onConnectApi: (port: number) => boolean;
  onModeChange: (mode: ExecutionMode) => void;
};

export function ControlsView({
  backend,
  apiPort,
  mode,
  onToggleStream,
  onConnectApi,
  onModeChange,
}: ControlsViewProps) {
  return (
    <>
      <SectionHeader
        eyebrow="WORKSPACE SETTINGS"
        title="Controls"
        description="Manage local stream, execution mode, collector defaults, and qualification rules."
        stats={[]}
      />
      <div className="section-view__body section-view__grid controls-grid">
        <LocalApiConnectionCard
          activePort={apiPort}
          backend={backend}
          onConnect={onConnectApi}
        />
        <StreamControlCard
          backend={backend}
          onToggleStream={onToggleStream}
        />
        <ExecutionModeCard mode={mode} onModeChange={onModeChange} />
        <PrefilterDefaultsSection
          key={`prefilter-${backend.apiBaseUrl}`}
          apiBaseUrl={backend.apiBaseUrl}
          backendConnected={backend.connection === "CONNECTED"}
          streamStatus={backend.stream.status}
        />
        <QualificationDefaultsSection
          key={`qualification-${backend.apiBaseUrl}`}
          apiBaseUrl={backend.apiBaseUrl}
          backendConnected={backend.connection === "CONNECTED"}
        />
      </div>
    </>
  );
}
