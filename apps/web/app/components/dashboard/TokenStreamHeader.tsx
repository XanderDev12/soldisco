type TokenStreamHeaderProps = {
  streamRunning: boolean;
};

export function TokenStreamHeader({
  streamRunning,
}: TokenStreamHeaderProps) {
  return (
    <div className="content-header">
      <div>
        <div className="eyebrow">
          <i className={streamRunning ? "live-dot" : "offline-dot"} />
          {streamRunning ? "STREAM ACTIVE" : "DISCOVERY STOPPED"}
        </div>
        <h1 id="view-title" tabIndex={-1}>
          Discovery
        </h1>
        <p>
          {streamRunning
            ? "Approved candidates will appear as they clear initial screening."
            : "Start the stream when you are ready to screen candidates."}
        </p>
      </div>
    </div>
  );
}
