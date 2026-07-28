export function SignalsTab() {
  return (
    <>
      <div className="strategy-summary">
        <span className="strategy-summary__icon">⌁</span>
        <div>
          <span>STRATEGY</span>
          <h3>No strategy active</h3>
          <p>Upload and validate a strategy before signals are evaluated.</p>
        </div>
      </div>
      <section className="inspector-section">
        <div className="section-title">
          <h3>Signal conditions</h3>
          <span>Not evaluated</span>
        </div>
        <div className="signal-list">
          <div>
            <span>Trusted wallets</span>
            <strong className="signal-muted">Not configured</strong>
          </div>
          <div>
            <span>Momentum · 30s</span>
            <strong className="signal-muted">Not evaluated</strong>
          </div>
          <div>
            <span>Buy / sell ratio</span>
            <strong className="signal-muted">Not evaluated</strong>
          </div>
          <div>
            <span>Volume acceleration</span>
            <strong className="signal-muted">Not evaluated</strong>
          </div>
        </div>
      </section>
      <div className="notice notice--violet">
        Strategy evaluation remains off until a validated strategy is active.
      </div>
    </>
  );
}
