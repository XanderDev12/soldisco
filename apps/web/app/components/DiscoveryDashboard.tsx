"use client";

import {
  type CSSProperties,
  type KeyboardEvent,
  type PointerEvent as ReactPointerEvent,
  useEffect,
  useMemo,
  useState,
} from "react";

type TokenStatus = "Approved" | "Pending" | "Rejected";
type MatchLevel = "Strong" | "Moderate" | "None" | "Evaluating";
type Filter = "All" | TokenStatus;
type InspectorTab = "Overview" | "Risk" | "Signals" | "Trade" | "Position";
type LayoutKey = "sidebar" | "inspector" | "tray";
type LayoutPreferences = Record<LayoutKey, number>;

type Token = {
  id: string;
  name: string;
  symbol: string;
  mint: string;
  age: string;
  status: TokenStatus;
  risk: number;
  rating: string;
  match: MatchLevel;
  price: string;
  move: string;
  moveUp: boolean;
  volume: string;
  liquidity: string;
  holders: string;
  buys: number;
  sells: number;
  momentum: number[];
  reason: string;
  checks: string[];
};

const tokens: Token[] = [];
const layoutStorageKey = "soldisco.layout.v1";
const defaultLayout: LayoutPreferences = {
  sidebar: 224,
  inspector: 354,
  tray: 64,
};
const layoutLimits: Record<LayoutKey, { min: number; max: number }> = {
  sidebar: { min: 180, max: 340 },
  inspector: { min: 290, max: 560 },
  tray: { min: 56, max: 240 },
};

function clampLayoutValue(key: LayoutKey, value: number) {
  const { min, max } = layoutLimits[key];
  return Math.min(max, Math.max(min, Math.round(value)));
}

function isStoredLayout(value: unknown): value is LayoutPreferences {
  if (!value || typeof value !== "object") return false;

  return (["sidebar", "inspector", "tray"] as const).every(
    (key) => typeof (value as Record<string, unknown>)[key] === "number",
  );
}

const filters: Filter[] = ["All", "Approved", "Pending", "Rejected"];
const inspectorTabs: InspectorTab[] = [
  "Overview",
  "Risk",
  "Signals",
  "Trade",
  "Position",
];

const navGroups = [
  {
    label: "DISCOVERY",
    items: [
      { icon: "⌁", name: "Token stream", active: true },
      { icon: "✓", name: "Initial approval" },
      { icon: "◇", name: "Watchlist" },
    ],
  },
  {
    label: "TRADING",
    items: [
      { icon: "↗", name: "Positions" },
      { icon: "≡", name: "Orders" },
      { icon: "◌", name: "Alerts" },
    ],
  },
  {
    label: "SYSTEM",
    items: [
      { icon: "⌘", name: "Strategies" },
      { icon: "↺", name: "Replays" },
      { icon: "⚙", name: "Controls" },
    ],
  },
];

function Sparkline({
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

function StatusBadge({ status }: { status: TokenStatus }) {
  return (
    <span className={`status status--${status.toLowerCase()}`}>
      <span className="status__dot" />
      {status}
    </span>
  );
}

function RiskBadge({ value }: { value: number }) {
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

function MatchBadge({ match }: { match: MatchLevel }) {
  return (
    <span className={`match match--${match.toLowerCase()}`}>{match}</span>
  );
}

export function DiscoveryDashboard() {
  const [selectedId, setSelectedId] = useState<string | null>(
    tokens[0]?.id ?? null,
  );
  const [filter, setFilter] = useState<Filter>("All");
  const [mode, setMode] = useState<"Paper" | "Live">("Paper");
  const [tab, setTab] = useState<InspectorTab>("Overview");
  const [sideNavOpen, setSideNavOpen] = useState(false);
  const [uploadOpen, setUploadOpen] = useState(false);
  const [walletMessage, setWalletMessage] = useState(false);
  const [orderSide, setOrderSide] = useState<"Buy" | "Sell">("Buy");
  const [orderAmount, setOrderAmount] = useState("0.10");
  const [layout, setLayout] = useState<LayoutPreferences>(defaultLayout);
  const [layoutLoaded, setLayoutLoaded] = useState(false);
  const [activeResize, setActiveResize] = useState<LayoutKey | null>(null);

  const selected = tokens.find((token) => token.id === selectedId) ?? null;
  const visibleTokens = useMemo(
    () =>
      filter === "All"
        ? tokens
        : tokens.filter((token) => token.status === filter),
    [filter],
  );

  useEffect(() => {
    const loadLayout = window.setTimeout(() => {
      try {
        const savedLayout = window.localStorage.getItem(layoutStorageKey);
        if (savedLayout) {
          const parsedLayout: unknown = JSON.parse(savedLayout);
          if (isStoredLayout(parsedLayout)) {
            setLayout({
              sidebar: clampLayoutValue("sidebar", parsedLayout.sidebar),
              inspector: clampLayoutValue("inspector", parsedLayout.inspector),
              tray: clampLayoutValue("tray", parsedLayout.tray),
            });
          }
        }
      } catch {
        window.localStorage.removeItem(layoutStorageKey);
      } finally {
        setLayoutLoaded(true);
      }
    }, 0);

    return () => window.clearTimeout(loadLayout);
  }, []);

  useEffect(() => {
    if (!layoutLoaded) return;
    const saveLayout = window.setTimeout(() => {
      window.localStorage.setItem(layoutStorageKey, JSON.stringify(layout));
    }, 120);

    return () => window.clearTimeout(saveLayout);
  }, [layout, layoutLoaded]);

  function selectToken(id: string) {
    setSelectedId(id);
    setTab("Overview");
  }

  function setLayoutValue(key: LayoutKey, value: number) {
    setLayout((current) => ({
      ...current,
      [key]: clampLayoutValue(key, value),
    }));
  }

  function beginResize(
    key: LayoutKey,
    event: ReactPointerEvent<HTMLButtonElement>,
  ) {
    if (event.button !== 0) return;

    const startCoordinate = key === "tray" ? event.clientY : event.clientX;
    const startValue = layout[key];
    const cursor = key === "tray" ? "row-resize" : "col-resize";

    event.preventDefault();
    setActiveResize(key);
    document.documentElement.style.cursor = cursor;
    document.documentElement.style.userSelect = "none";

    const handlePointerMove = (pointerEvent: PointerEvent) => {
      const coordinate =
        key === "tray" ? pointerEvent.clientY : pointerEvent.clientX;
      const movement = coordinate - startCoordinate;
      const nextValue =
        key === "sidebar" ? startValue + movement : startValue - movement;
      setLayoutValue(key, nextValue);
    };

    const finishResize = () => {
      setActiveResize(null);
      document.documentElement.style.cursor = "";
      document.documentElement.style.userSelect = "";
      window.removeEventListener("pointermove", handlePointerMove);
      window.removeEventListener("pointerup", finishResize);
      window.removeEventListener("pointercancel", finishResize);
    };

    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", finishResize);
    window.addEventListener("pointercancel", finishResize);
  }

  function handleResizeKey(
    key: LayoutKey,
    event: KeyboardEvent<HTMLButtonElement>,
  ) {
    const step = event.shiftKey ? 24 : 8;
    let nextValue: number | null = null;

    if (event.key === "Home") nextValue = layoutLimits[key].min;
    if (event.key === "End") nextValue = layoutLimits[key].max;

    if (key === "sidebar") {
      if (event.key === "ArrowLeft") nextValue = layout[key] - step;
      if (event.key === "ArrowRight") nextValue = layout[key] + step;
    } else if (key === "inspector") {
      if (event.key === "ArrowLeft") nextValue = layout[key] + step;
      if (event.key === "ArrowRight") nextValue = layout[key] - step;
    } else {
      if (event.key === "ArrowUp") nextValue = layout[key] + step;
      if (event.key === "ArrowDown") nextValue = layout[key] - step;
    }

    if (nextValue === null) return;
    event.preventDefault();
    setLayoutValue(key, nextValue);
  }

  function resetLayout() {
    setLayout(defaultLayout);
  }

  const layoutStyle = {
    "--sidebar-pref": `${layout.sidebar}px`,
    "--inspector-pref": `${layout.inspector}px`,
    "--tray-pref": `${layout.tray}px`,
  } as CSSProperties;

  return (
    <main
      className="dashboard-shell"
      data-resizing={activeResize ?? undefined}
      style={layoutStyle}
    >
      <aside
        id="navigation-panel"
        className={`sidebar ${sideNavOpen ? "sidebar--open" : ""}`}
      >
        <div className="brand">
          <span className="brand__mark">
            <i />
            <i />
            <i />
          </span>
          <div>
            <strong>SOLDISCO</strong>
            <span>Discovery console</span>
          </div>
        </div>

        <nav className="sidebar__nav" aria-label="Primary navigation">
          {navGroups.map((group) => (
            <div className="nav-group" key={group.label}>
              <p>{group.label}</p>
              {group.items.map((item) => (
                <button
                  type="button"
                  className={`nav-item ${item.active ? "nav-item--active" : ""}`}
                  key={item.name}
                  onClick={() => setSideNavOpen(false)}
                >
                  <span className="nav-item__icon">{item.icon}</span>
                  <span>{item.name}</span>
                </button>
              ))}
            </div>
          ))}
        </nav>

        <div className="system-card">
          <div className="system-card__head">
            <span>
              <i className="offline-dot" />
              Services disconnected
            </span>
            <strong>—</strong>
          </div>
          <div className="system-card__row">
            <span>RPC</span>
            <span>Not connected</span>
          </div>
          <div className="system-card__row">
            <span>Stream</span>
            <span>Not connected</span>
          </div>
        </div>

        <div className="sidebar__footer">
          <span className="avatar">GD</span>
          <div>
            <strong>Local workspace</strong>
            <span>Development build</span>
          </div>
          <button type="button" aria-label="Workspace menu">
            ···
          </button>
        </div>
        <button
          type="button"
          className="resize-handle resize-handle--sidebar"
          role="separator"
          aria-label="Resize navigation"
          aria-controls="navigation-panel"
          aria-orientation="vertical"
          aria-valuemin={layoutLimits.sidebar.min}
          aria-valuemax={layoutLimits.sidebar.max}
          aria-valuenow={layout.sidebar}
          onPointerDown={(event) => beginResize("sidebar", event)}
          onKeyDown={(event) => handleResizeKey("sidebar", event)}
          onDoubleClick={() =>
            setLayoutValue("sidebar", defaultLayout.sidebar)
          }
          title="Drag to resize · Double-click to reset"
        />
      </aside>

      {sideNavOpen && (
        <button
          type="button"
          className="mobile-scrim"
          onClick={() => setSideNavOpen(false)}
          aria-label="Close navigation"
        />
      )}

      <section className="workspace">
        <header className="topbar">
          <button
            type="button"
            className="mobile-menu"
            onClick={() => setSideNavOpen(true)}
            aria-label="Open navigation"
          >
            ☰
          </button>

          <div className="strategy-control">
            <span className="strategy-control__label">STRATEGY</span>
            <span className="strategy-control__icon">⌁</span>
            <select aria-label="Active strategy">
              <option>No strategy active</option>
              <option disabled>Wallet-Conditioned Momentum — not configured</option>
              <option disabled>Liquidity Breakout — coming soon</option>
              <option disabled>Mean Reversion — coming soon</option>
            </select>
            <span className="strategy-control__state">
              <i className="offline-dot" />
              Inactive
            </span>
          </div>

          <div className="topbar__actions">
            <button
              type="button"
              className="layout-reset"
              onClick={resetLayout}
              title="Reset panel sizes"
            >
              ↺ Layout
            </button>
            <button
              type="button"
              className="upload-button"
              onClick={() => setUploadOpen(true)}
            >
              <span>＋</span>
              Upload strategy
            </button>

            <div className="mode-switch" aria-label="Execution mode">
              {(["Paper", "Live"] as const).map((item) => (
                <button
                  type="button"
                  key={item}
                  className={mode === item ? "is-active" : ""}
                  onClick={() => setMode(item)}
                >
                  {item}
                </button>
              ))}
            </div>

            <button
              type="button"
              className="wallet-button"
              onClick={() => {
                setWalletMessage(true);
                window.setTimeout(() => setWalletMessage(false), 2600);
              }}
            >
              <span className="wallet-button__icon">▰</span>
              Connect wallet
            </button>
          </div>
        </header>

        {mode === "Live" && (
          <div className="guard-banner">
            <span>LIVE MODE PREVIEW</span>
            Execution remains locked until wallet and policy modules are added.
          </div>
        )}

        <div className="content-header">
          <div>
            <div className="eyebrow">
              <i className="offline-dot" />
              DISCOVERY IDLE
            </div>
            <h1>All coins</h1>
            <p>Tokens will appear here after the discovery stream is connected.</p>
          </div>
          <div className="content-header__stats">
            <div>
              <span>STREAM RATE</span>
              <strong>— <small>/ min</small></strong>
            </div>
            <div>
              <span>FIRST-PASS RATE</span>
              <strong>—<small>%</small></strong>
            </div>
            <div>
              <span>MEDIAN LATENCY</span>
              <strong>— <small>sec</small></strong>
            </div>
          </div>
        </div>

        <div className="stream-layout">
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
                      onClick={() => setFilter(item)}
                    >
                      {item}
                      <span>{count}</span>
                    </button>
                  );
                })}
              </div>
              <div className="stream-tools">
                <span className="last-update">
                  <i className="offline-dot" />
                  Waiting for stream
                </span>
                <button
                  type="button"
                  className="pause-button"
                  disabled
                >
                  Ⅱ Pause
                </button>
                <button type="button" className="icon-button" aria-label="Stream settings">
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
                      className={selected?.id === token.id ? "is-selected" : ""}
                      onClick={() => selectToken(token.id)}
                    >
                      <td>
                        <button
                          type="button"
                          className="token-name token-select"
                          aria-label={`Inspect ${token.name}`}
                          onClick={(event) => {
                            event.stopPropagation();
                            selectToken(token.id);
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
                      <td><StatusBadge status={token.status} /></td>
                      <td><RiskBadge value={token.risk} /></td>
                      <td>
                        <span className={`rating rating--${token.rating.charAt(0)}`}>
                          {token.rating}
                        </span>
                      </td>
                      <td><MatchBadge match={token.match} /></td>
                      <td>
                        <div className="momentum-cell">
                          <Sparkline
                            values={token.momentum}
                            positive={token.moveUp}
                          />
                          <span className={token.moveUp ? "positive" : "negative"}>
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
                  <strong>No tokens yet</strong>
                  <p>The stream is empty until a discovery source is connected.</p>
                </div>
              )}
            </div>

            <footer className="stream-footer">
              <span>{visibleTokens.length} tokens · Stream disconnected</span>
              <span>Deterministic gate not running</span>
            </footer>
          </section>

          <button
            type="button"
            className="resize-handle resize-handle--inspector"
            role="separator"
            aria-label="Resize token inspector"
            aria-controls="stream-panel inspector-panel"
            aria-orientation="vertical"
            aria-valuemin={layoutLimits.inspector.min}
            aria-valuemax={layoutLimits.inspector.max}
            aria-valuenow={layout.inspector}
            onPointerDown={(event) => beginResize("inspector", event)}
            onKeyDown={(event) => handleResizeKey("inspector", event)}
            onDoubleClick={() =>
              setLayoutValue("inspector", defaultLayout.inspector)
            }
            title="Drag to resize · Double-click to reset"
          />

          {selected ? (
            <aside id="inspector-panel" className="inspector">
            <div className="inspector__head">
              <div className="token-name token-name--large">
                <span className={`token-logo token-logo--large token-logo--${selected.id}`}>
                  {selected.symbol.slice(0, 1)}
                </span>
                <div>
                  <strong>{selected.name}</strong>
                  <span>{selected.symbol} · {selected.mint}</span>
                </div>
              </div>
              <div className="inspector__head-actions">
                <button type="button" aria-label="Add to watchlist">☆</button>
                <button type="button" aria-label="More actions">···</button>
              </div>
              <div className="price-block">
                <strong>{selected.price}</strong>
                <span className={selected.moveUp ? "positive" : "negative"}>
                  {selected.move} <small>2m</small>
                </span>
              </div>
            </div>

            <div className="inspector-tabs" role="tablist">
              {inspectorTabs.map((item) => (
                <button
                  type="button"
                  role="tab"
                  aria-selected={tab === item}
                  className={tab === item ? "is-active" : ""}
                  key={item}
                  onClick={() => setTab(item)}
                >
                  {item}
                </button>
              ))}
            </div>

            <div className="inspector__body">
              {tab === "Overview" && (
                <>
                  <div className="decision-card">
                    <div className="decision-card__top">
                      <StatusBadge status={selected.status} />
                      <span>Deterministic first pass</span>
                    </div>
                    <p>{selected.reason}</p>
                    <div className="score-row">
                      <div>
                        <span>Risk value</span>
                        <RiskBadge value={selected.risk} />
                      </div>
                      <div>
                        <span>Discovery rating</span>
                        <strong className={`rating rating--large rating--${selected.rating.charAt(0)}`}>
                          {selected.rating}
                        </strong>
                      </div>
                      <div>
                        <span>Strategy match</span>
                        <MatchBadge match={selected.match} />
                      </div>
                    </div>
                  </div>

                  <section className="inspector-section">
                    <div className="section-title">
                      <h3>Market snapshot</h3>
                      <span>On-chain data</span>
                    </div>
                    <div className="metric-grid">
                      <div><span>Volume · 5m</span><strong>{selected.volume}</strong></div>
                      <div><span>Liquidity</span><strong>{selected.liquidity}</strong></div>
                      <div><span>Holders</span><strong>{selected.holders}</strong></div>
                      <div><span>Age</span><strong>{selected.age}</strong></div>
                    </div>
                  </section>

                  <section className="inspector-section">
                    <div className="section-title">
                      <h3>Flow momentum</h3>
                      <span>2 minute window</span>
                    </div>
                    <div className="flow-chart">
                      <Sparkline values={selected.momentum} positive={selected.moveUp} />
                    </div>
                    <div className="buy-sell-bar">
                      <div
                        style={{
                          width: `${
                            selected.buys + selected.sells > 0
                              ? (selected.buys /
                                  (selected.buys + selected.sells)) *
                                100
                              : 0
                          }%`,
                        }}
                      />
                    </div>
                    <div className="buy-sell-labels">
                      <span><i className="buy-dot" /> {selected.buys} buys</span>
                      <span>{selected.sells} sells <i className="sell-dot" /></span>
                    </div>
                  </section>
                </>
              )}

              {tab === "Risk" && (
                <>
                  <div className="risk-summary">
                    <div className={`risk-orbit risk-orbit--${selected.risk <= 35 ? "low" : selected.risk <= 60 ? "medium" : "high"}`}>
                      <strong>{selected.risk}</strong>
                      <span>/ 100</span>
                    </div>
                    <div>
                      <span>DETERMINISTIC RISK</span>
                      <h3>{selected.risk <= 35 ? "Low observed risk" : selected.risk <= 60 ? "Review required" : "Elevated risk"}</h3>
                      <p>Based on the latest completed deterministic checks.</p>
                    </div>
                  </div>
                  <section className="inspector-section">
                    <div className="section-title">
                      <h3>First-pass checks</h3>
                      <span>{selected.checks.length} checks</span>
                    </div>
                    <div className="check-list">
                      {selected.checks.map((check, index) => (
                        <div key={check}>
                          <span className={selected.status === "Rejected" && index > 0 ? "check-fail" : "check-pass"}>
                            {selected.status === "Rejected" && index > 0 ? "!" : "✓"}
                          </span>
                          <span>{check}</span>
                          <small>{selected.status === "Rejected" && index > 0 ? "Flagged" : "Passed"}</small>
                        </div>
                      ))}
                    </div>
                  </section>
                  <div className="notice">
                    Risk values are screening signals, not guarantees of safety or returns.
                  </div>
                </>
              )}

              {tab === "Signals" && (
                <>
                  <div className="strategy-summary">
                    <span className="strategy-summary__icon">⌁</span>
                    <div>
                      <span>STRATEGY</span>
                      <h3>No strategy active</h3>
                      <p>Upload and validate a strategy before signals are evaluated.</p>
                    </div>
                  </div>
                  <section className="inspector-section">
                    <div className="section-title"><h3>Signal conditions</h3><span>Not evaluated</span></div>
                    <div className="signal-list">
                      <div><span>Trusted wallets</span><strong className="signal-muted">Not configured</strong></div>
                      <div><span>Momentum · 30s</span><strong className="signal-muted">Not evaluated</strong></div>
                      <div><span>Buy / sell ratio</span><strong className="signal-muted">Not evaluated</strong></div>
                      <div><span>Volume acceleration</span><strong className="signal-muted">Not evaluated</strong></div>
                    </div>
                  </section>
                  <div className="notice notice--violet">
                    Strategy evaluation remains off until a validated strategy is active.
                  </div>
                </>
              )}

              {tab === "Trade" && (
                <TradeTicket
                  token={selected}
                  side={orderSide}
                  setSide={setOrderSide}
                  amount={orderAmount}
                  setAmount={setOrderAmount}
                  mode={mode}
                  onWallet={() => {
                    setWalletMessage(true);
                    window.setTimeout(() => setWalletMessage(false), 2600);
                  }}
                />
              )}

              {tab === "Position" && (
                <>
                  <div className="empty-position">
                    <span className="empty-position__icon">◎</span>
                    <h3>Position data unavailable</h3>
                    <p>Connect a wallet to load holdings for {selected.symbol}.</p>
                    <button type="button" onClick={() => setTab("Trade")}>
                      View trade controls
                    </button>
                  </div>
                </>
              )}
            </div>

            <div className="inspector__actions">
              <button type="button" className="secondary-action">Add to watchlist</button>
              <button type="button" className="primary-action" onClick={() => setTab("Trade")}>
                Review trade
              </button>
            </div>
          </aside>
          ) : (
            <aside id="inspector-panel" className="inspector">
              <div className="empty-state">
                No token selected. The inspector will populate when the stream
                receives its first token.
              </div>
            </aside>
          )}
        </div>

        <div id="positions-panel" className="positions-tray">
          <button
            type="button"
            className="resize-handle resize-handle--tray"
            role="separator"
            aria-label="Resize positions tray"
            aria-controls="positions-panel"
            aria-orientation="horizontal"
            aria-valuemin={layoutLimits.tray.min}
            aria-valuemax={layoutLimits.tray.max}
            aria-valuenow={layout.tray}
            onPointerDown={(event) => beginResize("tray", event)}
            onKeyDown={(event) => handleResizeKey("tray", event)}
            onDoubleClick={() => setLayoutValue("tray", defaultLayout.tray)}
            title="Drag to resize · Double-click to reset"
          />
          <button type="button" className="positions-tray__label">
            <span className="positions-icon">↗</span>
            <span>
              <strong>Positions</strong>
              <small>0 open positions</small>
            </span>
            <span className="tray-chevron">⌃</span>
          </button>
          <div className="positions-empty">Position data unavailable.</div>
          <span className="paper-tag">PAPER</span>
        </div>
      </section>

      {walletMessage && (
        <div className="toast" role="status">
          <span>▰</span>
          <div>
            <strong>Wallet connection is not enabled</strong>
            <p>This skeleton cannot sign or submit transactions.</p>
          </div>
        </div>
      )}

      {uploadOpen && (
        <div className="modal-backdrop" role="presentation">
          <div className="modal" role="dialog" aria-modal="true" aria-labelledby="upload-title">
            <button
              type="button"
              className="modal__close"
              onClick={() => setUploadOpen(false)}
              aria-label="Close upload dialog"
            >
              ×
            </button>
            <span className="modal__eyebrow">STRATEGY WORKSPACE</span>
            <h2 id="upload-title">Upload a strategy</h2>
            <p>
              Strategy files will be accepted after validation and sandboxing
              are connected.
            </p>
            <div className="upload-zone">
              <span>⇧</span>
              <strong>No strategy file selected</strong>
              <small>Expected format: declarative JSON manifest</small>
            </div>
            <div className="modal__notice">
              Uploaded strategies will be inactive until validation and a replay dry run succeed.
            </div>
            <div className="modal__actions">
              <button type="button" onClick={() => setUploadOpen(false)}>Cancel</button>
              <button type="button" disabled>Choose file</button>
            </div>
          </div>
        </div>
      )}
    </main>
  );
}

function TradeTicket({
  token,
  side,
  setSide,
  amount,
  setAmount,
  mode,
  onWallet,
}: {
  token: Token;
  side: "Buy" | "Sell";
  setSide: (side: "Buy" | "Sell") => void;
  amount: string;
  setAmount: (amount: string) => void;
  mode: "Paper" | "Live";
  onWallet: () => void;
}) {
  const presets = side === "Buy" ? ["0.05", "0.10", "0.25", "0.50"] : ["25%", "50%", "75%", "100%"];
  return (
    <div className="trade-ticket">
      <div className="trade-safety">
        <span>EXECUTION LOCKED</span>
        <p>No route, quote, wallet, or execution service is connected.</p>
      </div>

      <div className="trade-side">
        <button type="button" className={side === "Buy" ? "is-active" : ""} onClick={() => setSide("Buy")}>
          Buy
        </button>
        <button type="button" className={side === "Sell" ? "is-active sell" : ""} onClick={() => setSide("Sell")}>
          Sell
        </button>
      </div>

      <div className="ticket-field">
        <div className="ticket-field__label">
          <span>{side === "Buy" ? "You pay" : "You sell"}</span>
          <span>Balance: —</span>
        </div>
        <div className="ticket-input">
          <input
            value={amount}
            onChange={(event) => setAmount(event.target.value)}
            inputMode="decimal"
            aria-label={side === "Buy" ? "SOL amount" : `${token.symbol} amount`}
          />
          <strong>{side === "Buy" ? "SOL" : token.symbol}</strong>
        </div>
        <div className="ticket-presets">
          {presets.map((preset) => (
            <button type="button" key={preset} onClick={() => setAmount(preset.replace("%", ""))}>
              {preset}
            </button>
          ))}
        </div>
      </div>

      <div className="quote-status">
        <div><span>Expected output</span><strong>Unavailable</strong></div>
        <div><span>Price impact</span><strong>—</strong></div>
        <div><span>Minimum received</span><strong>—</strong></div>
        <div><span>Route & fees</span><strong>Not available</strong></div>
      </div>

      <div className="ticket-risk-line">
        <span>Current token risk</span>
        <RiskBadge value={token.risk} />
      </div>

      <button type="button" className="connect-ticket" onClick={onWallet}>
        Connect wallet to continue
      </button>
      <button type="button" className="review-disabled" disabled>
        Review {side.toLowerCase()} · {mode} mode
      </button>
      <p className="ticket-footnote">
        Final quotes, simulation, and explicit wallet confirmation will be required before execution.
      </p>
    </div>
  );
}
