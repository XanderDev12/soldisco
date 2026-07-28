import type { MatchLevel, TokenStatus } from "./types";

export function Sparkline({
  values,
  positive,
}: {
  values: number[];
  positive: boolean;
}) {
  return (
    <span
      className={`sparkline ${positive ? "sparkline--positive" : "sparkline--negative"}`}
      aria-label={positive ? "Upward momentum" : "Downward momentum"}
    >
      {values.map((value, index) => (
        <i key={index} style={{ height: `${Math.max(14, value)}%` }} />
      ))}
    </span>
  );
}

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

export function MatchBadge({ match }: { match: MatchLevel }) {
  return (
    <span className={`match match--${match.toLowerCase()}`}>{match}</span>
  );
}
