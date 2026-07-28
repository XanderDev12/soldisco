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
          All coins
        </h1>
        <p>
          {streamRunning
            ? "The stream is active and waiting for a discovery source."
            : "Start the stream when you are ready to receive candidates."}
        </p>
      </div>
      <div className="content-header__stats">
        <div>
          <span>STREAM RATE</span>
          <strong>
            — <small>/ min</small>
          </strong>
        </div>
        <div>
          <span>FIRST-PASS RATE</span>
          <strong>
            —<small>%</small>
          </strong>
        </div>
        <div>
          <span>MEDIAN LATENCY</span>
          <strong>
            — <small>sec</small>
          </strong>
        </div>
      </div>
    </div>
  );
}
