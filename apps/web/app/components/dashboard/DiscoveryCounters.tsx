import type { ScreeningSummary } from "./types";

type DiscoveryCountersProps = {
  summary: ScreeningSummary;
};

export function DiscoveryCounters({
  summary,
}: DiscoveryCountersProps) {
  const feedCounter =
    summary.mode === "OBSERVE_ALL"
      ? {
          label: "Observed",
          value:
            summary.observed === null ? "—" : String(summary.observed),
          tone: "observed",
        }
      : {
          label: "Approved",
          value:
            summary.approved === null ? "—" : String(summary.approved),
          tone: "approved",
        };
  const counters = [
    feedCounter,
    {
      label: "Queued facts",
      value:
        summary.pending === null ? "—" : String(summary.pending),
      tone: "pending",
    },
    {
      label: "Rejected",
      value:
        summary.rejected === null ? "—" : String(summary.rejected),
      tone: "rejected",
    },
    {
      label: "Processed rate",
      value:
        summary.ratePerMinute === null
          ? "— / min"
          : `${summary.ratePerMinute} / min`,
      tone: "rate",
    },
  ];

  return (
    <dl
      className="discovery-counters"
      aria-label="Discovery collection status"
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
