import type { RejectionLogEntry } from "./types";

type RejectionLogProps = {
  entries: RejectionLogEntry[];
  totalRejected: number | null;
};

export function RejectionLog({
  entries,
  totalRejected,
}: RejectionLogProps) {
  const visibleEntries = entries.slice(0, 25);

  return (
    <details className="rejection-log">
      <summary>
        Activity rejection log
        <span>{totalRejected ?? "—"}</span>
      </summary>
      <div className="rejection-log__panel">
        <div className="rejection-log__head">
          <div>
            <strong>Activity qualification rejections</strong>
            <span>Cumulative reasons from finalized observation windows</span>
          </div>
          {entries.length > visibleEntries.length && (
            <small>Showing {visibleEntries.length} reasons</small>
          )}
        </div>

        {visibleEntries.length === 0 ? (
          <div className="rejection-log__empty">
            No activity qualification rejections recorded.
          </div>
        ) : (
          <ol className="rejection-log__entries">
            {visibleEntries.map((entry) => (
              <li key={entry.reasonCode}>
                <div>
                  <strong>{entry.reasonCode}</strong>
                  <span>Rejection reason</span>
                </div>
                <strong className="rejection-log__count">
                  {entry.count}
                </strong>
                <time dateTime={entry.lastSeenAt}>
                  {entry.lastSeenAt}
                </time>
              </li>
            ))}
          </ol>
        )}
      </div>
    </details>
  );
}
