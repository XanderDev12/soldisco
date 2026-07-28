import type { ScreeningSummary } from "./types";

type DiscoveryCountersProps = {
  summary: ScreeningSummary;
};

export type DiscoveryCounterItem = {
  label: string;
  value: string;
  tone: string;
};

export function buildDiscoveryCounters(
  summary: ScreeningSummary,
): DiscoveryCounterItem[] {
  const feedCounter =
    summary.mode === "OBSERVE_ALL"
      ? {
          label: "Current observed",
          value:
            summary.observed === null ? "—" : String(summary.observed),
          tone: "observed",
        }
      : summary.mode === "QUALIFIED_ONLY"
        ? {
            label: "Current qualified",
            value:
              summary.qualified === null
                ? "—"
                : String(summary.qualified),
            tone: "approved",
          }
        : {
          label: "Current approved",
          value:
            summary.approved === null ? "—" : String(summary.approved),
          tone: "approved",
        };
  return [
    feedCounter,
    {
      label: "Open windows",
      value:
        summary.qualificationPending === null
          ? "—"
          : String(summary.qualificationPending),
      tone: "pending",
    },
    {
      label: "Activity rejects",
      value:
        summary.qualificationRejected === null
          ? "—"
          : String(summary.qualificationRejected),
      tone: "rejected",
    },
    {
      label: "Unknown windows",
      value:
        summary.qualificationUnknown === null
          ? "—"
          : String(summary.qualificationUnknown),
      tone: "unknown",
    },
    {
      label: "Processing failures",
      value:
        summary.processingFailures === null
          ? "—"
          : String(summary.processingFailures),
      tone: "failure",
    },
    {
      label: "Events/min",
      value:
        summary.ratePerMinute === null
          ? "— / min"
          : `${summary.ratePerMinute} / min`,
      tone: "rate",
    },
  ];
}

export function DiscoveryCounters({
  summary,
}: DiscoveryCountersProps) {
  const counters = buildDiscoveryCounters(summary);

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
