import type { StreamControlViewModel } from "../../lib/soldisco-api/viewModels";

type TokenStreamHeaderProps = {
  stream: StreamControlViewModel;
};

export function TokenStreamHeader({ stream }: TokenStreamHeaderProps) {
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
        <p>{stream.detail}</p>
      </div>
    </div>
  );
}
