import type { ExecutionMode } from "../types";
import { ExecutionModeCard } from "../controls/ExecutionModeCard";
import { PrefilterDefaultsSection } from "../controls/PrefilterDefaultsSection";
import { StreamControlCard } from "../controls/StreamControlCard";
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
  return (
    <>
      <SectionHeader
        eyebrow="WORKSPACE SETTINGS"
        title="Controls"
        description="Manage local stream, execution mode, and global collector defaults."
        stats={[]}
      />
      <div className="section-view__body section-view__grid controls-grid">
        <StreamControlCard
          backend={backend}
          onToggleStream={onToggleStream}
        />
        <ExecutionModeCard mode={mode} onModeChange={onModeChange} />
        <PrefilterDefaultsSection
          backendConnected={backend.connection === "CONNECTED"}
          streamStatus={backend.stream.status}
        />
      </div>
    </>
  );
}
