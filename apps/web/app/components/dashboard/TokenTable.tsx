import { DiscoveryCounters } from "./DiscoveryCounters";
import { RejectionLog } from "./RejectionLog";
import { MatchBadge, RiskBadge, Sparkline } from "./TokenBadges";
import type {
  RejectionLogEntry,
  ScreeningSummary,
  Token,
} from "./types";

type TokenTableProps = {
  approvedTokens: Token[];
  screeningSummary: ScreeningSummary;
  rejectionLog: RejectionLogEntry[];
  selectedToken: Token | null;
  streamRunning: boolean;
  onSelectToken: (id: string) => void;
  onOpenControls: () => void;
};

export function TokenTable({
  approvedTokens,
  screeningSummary,
  rejectionLog,
  selectedToken,
  streamRunning,
  onSelectToken,
  onOpenControls,
}: TokenTableProps) {
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
            <i className={streamRunning ? "paused-dot" : "offline-dot"} />
            {streamRunning
              ? "Active · No source connected"
              : "Stream stopped"}
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
        <table className="token-table">
          <thead>
            <tr>
              <th>Token</th>
              <th>Age</th>
              <th>Risk</th>
              <th>Rating</th>
              <th>Strategy match</th>
              <th>Momentum · 2m</th>
              <th>Volume</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {approvedTokens.map((token) => (
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
                    aria-label={`Inspect ${token.name}`}
                    onClick={(event) => {
                      event.stopPropagation();
                      onSelectToken(token.id);
                    }}
                  >
                    <span className={`token-logo token-logo--${token.id}`}>
                      {token.symbol.slice(0, 1)}
                    </span>
                    <div>
                      <strong>{token.name}</strong>
                      <span>
                        {token.symbol}
                        <i>·</i>
                        {token.mint}
                      </span>
                    </div>
                  </button>
                </td>
                <td className="mono subtle">{token.age}</td>
                <td>
                  <RiskBadge value={token.risk} />
                </td>
                <td>
                  <span
                    className={`rating rating--${token.rating.charAt(0)}`}
                  >
                    {token.rating}
                  </span>
                </td>
                <td>
                  <MatchBadge match={token.match} />
                </td>
                <td>
                  <div className="momentum-cell">
                    <Sparkline
                      values={token.momentum}
                      positive={token.moveUp}
                    />
                    <span
                      className={token.moveUp ? "positive" : "negative"}
                    >
                      {token.move}
                    </span>
                  </div>
                </td>
                <td className="mono">{token.volume}</td>
                <td>
                  <button
                    type="button"
                    className="row-more"
                    aria-label={`More actions for ${token.name}`}
                    onClick={(event) => event.stopPropagation()}
                  >
                    ···
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        {approvedTokens.length === 0 && (
          <div className="empty-state empty-state--stream">
            <span>⌁</span>
            <strong>
              {streamRunning
                ? "Waiting for approved candidates"
                : "Stream stopped"}
            </strong>
            <p>
              {streamRunning
                ? "Only candidates that pass initial screening appear here."
                : "Start the stream when you are ready to screen candidates."}
            </p>
          </div>
        )}
      </div>

      <footer className="stream-footer">
        <span>
          {approvedTokens.length} approved tokens · Stream{" "}
          {streamRunning ? "active" : "stopped"}
        </span>
        <span>
          {streamRunning
            ? "Pending and rejected candidates stay out of the feed"
            : "Deterministic gate not running"}
        </span>
      </footer>
    </section>
  );
}
