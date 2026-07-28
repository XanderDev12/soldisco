import type { RejectionLogEntry } from "./types";

type RejectionLogProps = {
  entries: RejectionLogEntry[];
  totalRejected: number;
};

export function RejectionLog({
  entries,
  totalRejected,
}: RejectionLogProps) {
  const visibleEntries = entries.slice(0, 25);

  return (
    <details className="rejection-log">
      <summary>
        Rejection log
        <span>{totalRejected}</span>
      </summary>
      <div className="rejection-log__panel">
        <div className="rejection-log__head">
          <div>
            <strong>Initial-screen failures</strong>
            <span>Compact reason records only</span>
          </div>
          {totalRejected > visibleEntries.length && (
            <small>Showing latest {visibleEntries.length}</small>
          )}
        </div>

        {visibleEntries.length === 0 ? (
          <div className="rejection-log__empty">
            No rejected candidates recorded.
          </div>
        ) : (
          <ol className="rejection-log__entries">
            {visibleEntries.map((entry) => (
              <li key={entry.id}>
                <div>
                  <strong>{entry.symbol ?? "Unknown token"}</strong>
                  <span>{entry.mint}</span>
                </div>
                <ul>
                  {entry.reasonCodes.map((reasonCode) => (
                    <li key={reasonCode}>{reasonCode}</li>
                  ))}
                </ul>
                <time dateTime={entry.rejectedAt}>
                  {entry.rejectedAt}
                </time>
              </li>
            ))}
          </ol>
        )}
      </div>
    </details>
  );
}
