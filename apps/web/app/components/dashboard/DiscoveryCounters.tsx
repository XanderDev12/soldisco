import type { ScreeningSummary } from "./types";

type DiscoveryCountersProps = {
  summary: ScreeningSummary;
};

export function DiscoveryCounters({
  summary,
}: DiscoveryCountersProps) {
  const counters = [
    {
      label: "Pending",
      value: String(summary.pending),
      tone: "pending",
    },
    {
      label: "Approved",
      value: String(summary.approved),
      tone: "approved",
    },
    {
      label: "Rejected",
      value: String(summary.rejected),
      tone: "rejected",
    },
    {
      label: "Flow rate",
      value:
        summary.ratePerMinute === null
          ? "— / min"
          : `${summary.ratePerMinute} / min`,
      tone: "rate",
    },
  ] as const;

  return (
    <dl
      className="discovery-counters"
      aria-label="Discovery screening status"
    >
      {counters.map((counter) => (
        <div
          className={`discovery-counter discovery-counter--${counter.tone}`}
          key={counter.label}
        >
          <dt>{counter.label}</dt>
          <dd>{counter.value}</dd>
        </div>
      ))}
    </dl>
  );
}
