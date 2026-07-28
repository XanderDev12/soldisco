import { DiscoveryCounters } from "./DiscoveryCounters";
import { describeEmptyStream } from "./emptyStreamState";
import { RejectionLog } from "./RejectionLog";
import { StatusBadge } from "./TokenBadges";
import type { StreamStatus } from "../../lib/soldisco-api/contracts";
import type {
  RejectionLogEntry,
  ScreeningSummary,
  Token,
} from "./types";

type TokenTableProps = {
  tokens: Token[];
  tokensTotal: number;
  tokensTruncated: boolean;
  dataStale: boolean;
  screeningSummary: ScreeningSummary;
  rejectionLog: RejectionLogEntry[];
  selectedToken: Token | null;
  streamStatus: StreamStatus | null;
  streamRequestedRunning: boolean;
  streamIndicatorClass: "live-dot" | "paused-dot" | "offline-dot";
  streamStatusLabel: string;
  backendConnected: boolean;
  onSelectToken: (id: string) => void;
  onOpenControls: () => void;
};

export function TokenTable({
  tokens,
  tokensTotal,
  tokensTruncated,
  dataStale,
  screeningSummary,
  rejectionLog,
  selectedToken,
  streamStatus,
  streamRequestedRunning,
  streamIndicatorClass,
  streamStatusLabel,
  backendConnected,
  onSelectToken,
  onOpenControls,
}: TokenTableProps) {
  const emptyStream = describeEmptyStream({
    backendConnected,
    streamStatus,
    requestedRunning: streamRequestedRunning,
  });

  return (
    <section
      id="stream-panel"
      className="stream-card"
      aria-label="Token discovery stream"
    >
      <div className="stream-toolbar">
        <DiscoveryCounters summary={screeningSummary} />
        <div className="stream-tools">
          <span className="last-update" role="status">
            <i
              className={
                backendConnected
                  ? streamIndicatorClass
                  : "offline-dot"
              }
            />
            {backendConnected
              ? `Stream · ${streamStatusLabel}`
              : "Backend unavailable"}
          </span>
          <RejectionLog
            entries={rejectionLog}
            totalRejected={screeningSummary.rejected}
          />
          <button
            type="button"
            className="icon-button"
            aria-label="Open stream controls"
            onClick={onOpenControls}
          >
            ⚙
          </button>
        </div>
      </div>

      <div className="table-scroll">
        {dataStale && tokens.length > 0 && (
          <div className="stale-data-notice" role="status">
            Last-known discoveries · The local backend is not providing a
            current snapshot.
          </div>
        )}
        <table className="token-table">
          <thead>
            <tr>
              <th>Token</th>
              <th>Stage</th>
              <th>Venue</th>
              <th>Latest event</th>
              <th>Trades</th>
              <th>Buy / sell</th>
              <th>Slot</th>
            </tr>
          </thead>
          <tbody>
            {tokens.map((token) => {
              const tokenLabel =
                token.name ?? token.symbol ?? "Unknown token";
              const tokenMonogram =
                token.symbol?.slice(0, 1) ??
                token.name?.slice(0, 1) ??
                "?";

              return (
              <tr
                key={token.id}
                className={
                  selectedToken?.id === token.id ? "is-selected" : ""
                }
                onClick={() => onSelectToken(token.id)}
              >
                <td>
                  <button
                    type="button"
                    className="token-name token-select"
                    aria-label={`Inspect ${tokenLabel}`}
                    onClick={(event) => {
                      event.stopPropagation();
                      onSelectToken(token.id);
                    }}
                  >
                    <span className="token-logo">
                      {tokenMonogram}
                    </span>
                    <div>
                      <strong>{tokenLabel}</strong>
                      <span>
                        {token.symbol && `${token.symbol} · `}
                        {token.mint}
                      </span>
                    </div>
                  </button>
                </td>
                <td>
                  <StatusBadge status={token.stageLabel} />
                </td>
                <td>{token.primaryVenue}</td>
                <td>{token.lastEventKind}</td>
                <td className="mono">{token.activity.trades}</td>
                <td className="mono">
                  <span className="positive">{token.activity.buys}</span>
                  {" / "}
                  <span className="negative">{token.activity.sells}</span>
                </td>
                <td className="mono subtle">{token.observedSlot}</td>
              </tr>
              );
            })}
          </tbody>
        </table>
        {tokens.length === 0 && (
          <div className="empty-state empty-state--stream">
            <span>⌁</span>
            <strong>{emptyStream.title}</strong>
            <p>{emptyStream.detail}</p>
          </div>
        )}
      </div>

      <footer className="stream-footer">
        <span>
          {dataStale ? "Cached · " : ""}
          {tokensTruncated
            ? `Showing latest ${tokens.length} of ${tokensTotal} discoveries`
            : `${tokensTotal} discoveries`}
          {" · "}
          {backendConnected
            ? `Stream ${streamStatusLabel.toLowerCase()}`
            : "Backend unavailable"}
        </span>
        <span>
          {screeningSummary.mode === "OBSERVE_ALL"
            ? "Observe-all mode · No approval threshold applied"
            : "Approved-only mode · Explicit passes only"}
        </span>
      </footer>
    </section>
  );
}
