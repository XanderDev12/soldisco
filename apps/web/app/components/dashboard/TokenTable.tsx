import type { Token, TokenFilter } from "./types";
import {
  MatchBadge,
  RiskBadge,
  Sparkline,
  StatusBadge,
} from "./TokenBadges";

type TokenTableProps = {
  tokens: Token[];
  visibleTokens: Token[];
  selectedToken: Token | null;
  streamRunning: boolean;
  filter: TokenFilter;
  onFilterChange: (filter: TokenFilter) => void;
  onSelectToken: (id: string) => void;
  onOpenControls: () => void;
};

const filters: TokenFilter[] = [
  "All",
  "Approved",
  "Pending",
  "Rejected",
];

export function TokenTable({
  tokens,
  visibleTokens,
  selectedToken,
  streamRunning,
  filter,
  onFilterChange,
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
        <div className="filters" aria-label="Filter tokens">
          {filters.map((item) => {
            const count =
              item === "All"
                ? tokens.length
                : tokens.filter((token) => token.status === item).length;

            return (
              <button
                type="button"
                className={filter === item ? "is-active" : ""}
                key={item}
                onClick={() => onFilterChange(item)}
              >
                {item}
                <span>{count}</span>
              </button>
            );
          })}
        </div>
        <div className="stream-tools">
          <span className="last-update" role="status">
            <i className={streamRunning ? "paused-dot" : "offline-dot"} />
            {streamRunning
              ? "Active · No source connected"
              : "Stream stopped"}
          </span>
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
              <th>First pass</th>
              <th>Risk</th>
              <th>Rating</th>
              <th>Strategy match</th>
              <th>Momentum · 2m</th>
              <th>Volume</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {visibleTokens.map((token) => (
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
                  <StatusBadge status={token.status} />
                </td>
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
        {visibleTokens.length === 0 && (
          <div className="empty-state empty-state--stream">
            <span>⌁</span>
            <strong>
              {streamRunning ? "Waiting for a source" : "Stream stopped"}
            </strong>
            <p>
              {streamRunning
                ? "The stream is active, but no discovery source is connected."
                : "Start the stream when you are ready to receive candidates."}
            </p>
          </div>
        )}
      </div>

      <footer className="stream-footer">
        <span>
          {visibleTokens.length} tokens · Stream{" "}
          {streamRunning ? "active" : "stopped"}
        </span>
        <span>
          {streamRunning
            ? "Source disconnected · Gate not running"
            : "Deterministic gate not running"}
        </span>
      </footer>
    </section>
  );
}
