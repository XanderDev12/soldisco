import type { TokenStatus } from "./types";

export function StatusBadge({ status }: { status: TokenStatus }) {
  return (
    <span className={`status status--${status.toLowerCase()}`}>
      <span className="status__dot" />
      {status}
    </span>
  );
}

export function RiskBadge({ value }: { value: number }) {
  const level = value <= 35 ? "low" : value <= 60 ? "medium" : "high";

  return (
    <span className={`risk-badge risk-badge--${level}`}>
      <span className="risk-badge__bar">
        <i style={{ width: `${value}%` }} />
      </span>
      {value}
    </span>
  );
}
