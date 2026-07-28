import type { DiscoveryWindowSummary } from "../../../lib/soldisco-api/contracts";

type QualificationWindowEvidenceProps = {
  evidence: DiscoveryWindowSummary;
};

type EvidenceMetricProps = {
  label: string;
  value: string | number | null;
};

function EvidenceMetric({ label, value }: EvidenceMetricProps) {
  const displayValue = value === null ? "Unavailable" : String(value);
  return (
    <div>
      <span>{label}</span>
      <strong title={displayValue}>{displayValue}</strong>
    </div>
  );
}

function formatInstant(unixMs: number): string {
  return new Date(unixMs).toISOString();
}

function formatBasisPoints(value: number | null): string | null {
  if (value === null) return null;
  const sign = value > 0 ? "+" : "";
  return `${sign}${(value / 100).toFixed(2)}% (${sign}${value} bps)`;
}

export function QualificationWindowEvidence({
  evidence,
}: QualificationWindowEvidenceProps) {
  return (
    <section className="inspector-section qualification-evidence">
      <div className="section-title">
        <h3>Qualification evidence</h3>
        <span>
          {evidence.decision} · {evidence.completeness}
        </span>
      </div>

      <div className="metric-grid metric-grid--qualification">
        <EvidenceMetric
          label="Ruleset revision"
          value={evidence.ruleset_revision}
        />
        <EvidenceMetric
          label="Window revision"
          value={evidence.window_revision}
        />
        <EvidenceMetric
          label="Wallet concentration"
          value={formatBasisPoints(
            evidence.maximum_single_wallet_quote_share_bps,
          )}
        />
        <EvidenceMetric
          label="Price change"
          value={formatBasisPoints(evidence.price_change_bps)}
        />
        <EvidenceMetric
          label="Opened"
          value={formatInstant(evidence.opened_unix_ms)}
        />
        <EvidenceMetric
          label="Closed"
          value={formatInstant(evidence.closed_unix_ms)}
        />
        <EvidenceMetric
          label="Evaluated"
          value={formatInstant(evidence.evaluated_unix_ms)}
        />
      </div>

      <div className="qualification-evidence__group">
        <span className="eyebrow">WINDOW ACTIVITY</span>
        <div className="metric-grid metric-grid--qualification">
          <EvidenceMetric label="Trades" value={evidence.trades} />
          <EvidenceMetric label="Buys" value={evidence.buys} />
          <EvidenceMetric label="Sells" value={evidence.sells} />
          <EvidenceMetric
            label="Unique traders"
            value={evidence.unique_traders}
          />
          <EvidenceMetric
            label="Unique buyers"
            value={evidence.unique_buyers}
          />
          <EvidenceMetric
            label="Unique sellers"
            value={evidence.unique_sellers}
          />
          <EvidenceMetric
            label="Buy base units"
            value={evidence.buy_base_volume_units}
          />
          <EvidenceMetric
            label="Sell base units"
            value={evidence.sell_base_volume_units}
          />
          <EvidenceMetric
            label="Buy quote units"
            value={evidence.buy_quote_volume_units}
          />
          <EvidenceMetric
            label="Sell quote units"
            value={evidence.sell_quote_volume_units}
          />
        </div>
      </div>

      <div className="qualification-evidence__group">
        <span className="eyebrow">RESERVE SNAPSHOTS</span>
        <div className="metric-grid metric-grid--references">
          <EvidenceMetric
            label="First base reserve"
            value={evidence.first_base_reserve_units}
          />
          <EvidenceMetric
            label="First quote reserve"
            value={evidence.first_quote_reserve_units}
          />
          <EvidenceMetric
            label="Latest base reserve"
            value={evidence.latest_base_reserve_units}
          />
          <EvidenceMetric
            label="Latest quote reserve"
            value={evidence.latest_quote_reserve_units}
          />
        </div>
      </div>

      <div className="qualification-evidence__reasons">
        <span className="eyebrow">DECISION REASONS</span>
        {evidence.reason_codes.length === 0 ? (
          <p>No reason codes returned.</p>
        ) : (
          <ul>
            {evidence.reason_codes.map((reason) => (
              <li key={reason}>{reason}</li>
            ))}
          </ul>
        )}
      </div>
    </section>
  );
}
