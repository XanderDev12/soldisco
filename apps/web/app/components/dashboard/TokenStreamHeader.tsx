import type { StreamControlViewModel } from "../../lib/soldisco-api/viewModels";
import type { DiscoveryMode } from "../../lib/soldisco-api/contracts";

type TokenStreamHeaderProps = {
  stream: StreamControlViewModel;
  discoveryMode: DiscoveryMode;
  dataStale: boolean;
};

function runningDetail(mode: DiscoveryMode): string {
  switch (mode) {
    case "OBSERVE_ALL":
      return "Structurally valid Pump and PumpSwap discoveries appear as they are decoded.";
    case "QUALIFIED_ONLY":
      return "Only candidates that pass a complete observation-window qualification appear here.";
    case "APPROVED_ONLY":
      return "Only candidates with an explicit deterministic safety pass appear here.";
  }
}

export function TokenStreamHeader({
  stream,
  discoveryMode,
  dataStale,
}: TokenStreamHeaderProps) {
  const detail = dataStale
    ? "Discovery updates are stale. The collector status may be current while the visible feed is not."
    : stream.status === "RUNNING"
      ? runningDetail(discoveryMode)
      : stream.detail;

  return (
    <div className="content-header">
      <div>
        <div className="eyebrow">
          <i className={stream.indicatorClass} />
          {stream.eyebrow}
        </div>
        <h1 id="view-title" tabIndex={-1}>
          Discovery
        </h1>
        <p>{detail}</p>
      </div>
    </div>
  );
}
