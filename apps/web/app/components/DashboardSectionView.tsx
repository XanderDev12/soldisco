"use client";

export type DashboardView =
  | "token-stream"
  | "initial-approval"
  | "watchlist"
  | "positions"
  | "orders"
  | "alerts"
  | "strategies"
  | "replays"
  | "controls";

type NavGroup = {
  label: "DISCOVERY" | "TRADING" | "SYSTEM";
  items: ReadonlyArray<{
    icon: string;
    name: string;
    id: DashboardView;
  }>;
};

export const navGroups = [
  {
    label: "DISCOVERY",
    items: [
      { icon: "⌁", name: "Token stream", id: "token-stream" },
      { icon: "✓", name: "Initial approval", id: "initial-approval" },
      { icon: "◇", name: "Watchlist", id: "watchlist" },
    ],
  },
  {
    label: "TRADING",
    items: [
      { icon: "↗", name: "Positions", id: "positions" },
      { icon: "≡", name: "Orders", id: "orders" },
      { icon: "◌", name: "Alerts", id: "alerts" },
    ],
  },
  {
    label: "SYSTEM",
    items: [
      { icon: "⌘", name: "Strategies", id: "strategies" },
      { icon: "↺", name: "Replays", id: "replays" },
      { icon: "⚙", name: "Controls", id: "controls" },
    ],
  },
] as const satisfies ReadonlyArray<NavGroup>;

type SectionView = Exclude<DashboardView, "token-stream">;

type DashboardSectionViewProps = {
  view: SectionView;
  streamRunning: boolean;
  mode: "Paper" | "Live";
  onToggleStream: () => void;
  onModeChange: (mode: "Paper" | "Live") => void;
  onWallet: () => void;
  onUpload: () => void;
  onResetLayout: () => void;
};

type ViewContent = {
  eyebrow: string;
  title: string;
  description: string;
  stats: readonly string[];
  cardTitle: string;
  cardDetail: string;
  columns: readonly string[];
  emptyTitle: string;
  emptyDetail: string;
};

const viewContent: Record<
  Exclude<SectionView, "positions" | "strategies" | "controls">,
  ViewContent
> = {
  "initial-approval": {
    eyebrow: "DETERMINISTIC SCREENING",
    title: "Initial approval",
    description:
      "Review completed first-pass decisions and their recorded reason codes.",
    stats: ["APPROVED", "REJECTED", "PENDING"],
    cardTitle: "First-pass results",
    cardDetail: "Completed assessments",
    columns: ["Token", "Decision", "Risk", "Rating", "Checks", "Assessed"],
    emptyTitle: "No first-pass results",
    emptyDetail:
      "Results will appear after candidates complete deterministic screening.",
  },
  watchlist: {
    eyebrow: "SAVED TOKENS",
    title: "Watchlist",
    description: "Keep selected discovery candidates together for review.",
    stats: ["WATCHED", "APPROVED", "STRATEGY MATCHES"],
    cardTitle: "Watched tokens",
    cardDetail: "Saved in this workspace",
    columns: ["Token", "First pass", "Risk", "Rating", "Strategy", "Added"],
    emptyTitle: "Watchlist is empty",
    emptyDetail:
      "Add a token from the discovery stream when candidates arrive.",
  },
  orders: {
    eyebrow: "ORDER ACTIVITY",
    title: "Orders",
    description: "Review order intent and submission outcomes when execution is connected.",
    stats: ["OPEN", "COMPLETED", "FAILED"],
    cardTitle: "Order records",
    cardDetail: "Execution service not connected",
    columns: ["Token", "Side", "Mode", "Amount", "Status", "Submitted"],
    emptyTitle: "Order data unavailable",
    emptyDetail: "No execution service is connected.",
  },
  alerts: {
    eyebrow: "MONITORING RULES",
    title: "Alerts",
    description: "Review configured conditions for token and position events.",
    stats: ["ACTIVE", "TRIGGERED", "MUTED"],
    cardTitle: "Alert rules",
    cardDetail: "No rules configured",
    columns: ["Alert", "Condition", "Scope", "State", "Last triggered"],
    emptyTitle: "No alerts configured",
    emptyDetail: "There are no alert rules in this workspace.",
  },
  replays: {
    eyebrow: "RECORDED SESSIONS",
    title: "Replays",
    description:
      "Review deterministic and strategy decisions from recorded stream events.",
    stats: ["SESSIONS", "EVENTS", "LAST RUN"],
    cardTitle: "Replay history",
    cardDetail: "Recorded event sessions",
    columns: ["Session", "Window", "Events", "Strategy", "Status", "Created"],
    emptyTitle: "No replay data",
    emptyDetail: "A replay requires recorded discovery events.",
  },
};

function SectionHeader({
  eyebrow,
  title,
  description,
  stats,
}: Pick<ViewContent, "eyebrow" | "title" | "description" | "stats">) {
  return (
    <header className="section-view__header">
      <div>
        <span className="eyebrow">{eyebrow}</span>
        <h1 id="view-title" tabIndex={-1}>{title}</h1>
        <p>{description}</p>
      </div>
      <dl>
        {stats.map((stat) => (
          <div key={stat}>
            <dt>{stat}</dt>
            <dd aria-label={`${stat.toLowerCase()} unavailable`}>—</dd>
          </div>
        ))}
      </dl>
    </header>
  );
}

function EmptyTable({
  title,
  detail,
  columns,
  emptyTitle,
  emptyDetail,
}: Pick<
  ViewContent,
  "columns" | "emptyTitle" | "emptyDetail"
> & {
  title: string;
  detail: string;
}) {
  return (
    <div className="section-card">
      <div className="section-card__head">
        <div>
          <h2>{title}</h2>
          <p>{detail}</p>
        </div>
        <span className="status-chip">Empty</span>
      </div>
      <div className="table-scroll">
        <table className="section-table">
          <thead>
            <tr>
              {columns.map((column) => (
                <th key={column} scope="col">
                  {column}
                </th>
              ))}
            </tr>
          </thead>
          <tbody />
        </table>
      </div>
      <div className="section-empty">
        <span aria-hidden="true">◇</span>
        <h2>{emptyTitle}</h2>
        <p>{emptyDetail}</p>
      </div>
    </div>
  );
}

function PositionsView({ onWallet }: Pick<DashboardSectionViewProps, "onWallet">) {
  return (
    <>
      <SectionHeader
        eyebrow="PORTFOLIO"
        title="Positions"
        description="Review reconciled holdings and position valuations."
        stats={["OPEN POSITIONS", "MARK VALUE", "REALIZED PNL"]}
      />
      <div className="section-view__body">
        <div className="section-card">
          <div className="section-card__head">
            <div>
              <h2>Held positions</h2>
              <p>Wallet-backed portfolio records</p>
            </div>
            <span className="status-chip">Unavailable</span>
          </div>
          <div className="section-empty">
            <span aria-hidden="true">◎</span>
            <h2>Position data unavailable</h2>
            <p>Connect a wallet to request holdings for this workspace.</p>
            <div className="control-card__actions">
              <button type="button" onClick={onWallet}>
                Connect wallet
              </button>
            </div>
          </div>
        </div>
      </div>
    </>
  );
}

function StrategiesView({
  onUpload,
}: Pick<DashboardSectionViewProps, "onUpload">) {
  return (
    <>
      <SectionHeader
        eyebrow="STRATEGY WORKSPACE"
        title="Strategies"
        description="Review validated strategy manifests and activation state."
        stats={["VALIDATED", "ACTIVE", "NEEDS REVIEW"]}
      />
      <div className="section-view__body">
        <div className="section-card">
          <div className="section-card__head">
            <div>
              <h2>Strategy library</h2>
              <p>Validated manifests</p>
            </div>
            <div className="control-card__actions">
              <button type="button" onClick={onUpload}>
                Upload strategy
              </button>
            </div>
          </div>
          <div className="table-scroll">
            <table className="section-table">
              <thead>
                <tr>
                  <th scope="col">Strategy</th>
                  <th scope="col">Version</th>
                  <th scope="col">Validation</th>
                  <th scope="col">Replay</th>
                  <th scope="col">State</th>
                </tr>
              </thead>
              <tbody />
            </table>
          </div>
          <div className="section-empty">
            <span aria-hidden="true">⌘</span>
            <h2>No validated strategies</h2>
            <p>Upload and validate a strategy before it can be activated.</p>
          </div>
        </div>
      </div>
    </>
  );
}

function ControlsView({
  streamRunning,
  mode,
  onToggleStream,
  onModeChange,
  onResetLayout,
}: Pick<
  DashboardSectionViewProps,
  | "streamRunning"
  | "mode"
  | "onToggleStream"
  | "onModeChange"
  | "onResetLayout"
>) {
  return (
    <>
      <SectionHeader
        eyebrow="WORKSPACE SETTINGS"
        title="Controls"
        description="Manage local stream, execution-mode, and layout preferences."
        stats={["STREAM RATE", "FIRST-PASS RATE", "MEDIAN LATENCY"]}
      />
      <div className="section-view__body section-view__grid">
        <article className="control-card">
          <div className="section-card__head">
            <div>
              <h2>Stream</h2>
              <p>Discovery intake control</p>
            </div>
            <span className="status-chip">
              {streamRunning ? "Active · no source" : "Stopped"}
            </span>
          </div>
          <div className="status-list">
            <div>
              <span>Current state</span>
              <strong>{streamRunning ? "Active" : "Stopped"}</strong>
            </div>
            <div>
              <span>Discovery source</span>
              <strong>Not connected</strong>
            </div>
            <div>
              <span>Deterministic gate</span>
              <strong>Not running</strong>
            </div>
          </div>
          <div className="control-card__actions">
            <button
              type="button"
              aria-pressed={streamRunning}
              onClick={onToggleStream}
            >
              {streamRunning ? "Stop stream" : "Start stream"}
            </button>
          </div>
        </article>

        <article className="control-card">
          <div className="section-card__head">
            <div>
              <h2>Execution mode</h2>
              <p>Interface review mode</p>
            </div>
            <span className="status-chip">{mode}</span>
          </div>
          <div
            className="control-card__actions"
            role="group"
            aria-label="Execution mode"
          >
            {(["Paper", "Live"] as const).map((item) => (
              <button
                type="button"
                key={item}
                aria-pressed={mode === item}
                onClick={() => onModeChange(item)}
              >
                {item}
              </button>
            ))}
          </div>
        </article>

        <article className="control-card">
          <div className="section-card__head">
            <div>
              <h2>Panel layout</h2>
              <p>Device-local panel sizing</p>
            </div>
            <span className="status-chip">Saved locally</span>
          </div>
          <div className="control-card__actions">
            <button type="button" onClick={onResetLayout}>
              Reset panel sizes
            </button>
          </div>
        </article>
      </div>
    </>
  );
}

export function DashboardSectionView({
  view,
  streamRunning,
  mode,
  onToggleStream,
  onModeChange,
  onWallet,
  onUpload,
  onResetLayout,
}: DashboardSectionViewProps) {
  let content;

  if (view === "positions") {
    content = <PositionsView onWallet={onWallet} />;
  } else if (view === "strategies") {
    content = <StrategiesView onUpload={onUpload} />;
  } else if (view === "controls") {
    content = (
      <ControlsView
        streamRunning={streamRunning}
        mode={mode}
        onToggleStream={onToggleStream}
        onModeChange={onModeChange}
        onResetLayout={onResetLayout}
      />
    );
  } else {
    const details = viewContent[view];
    content = (
      <>
        <SectionHeader {...details} />
        <div className="section-view__body">
          <EmptyTable
            title={details.cardTitle}
            detail={details.cardDetail}
            columns={details.columns}
            emptyTitle={details.emptyTitle}
            emptyDetail={details.emptyDetail}
          />
        </div>
      </>
    );
  }

  return (
    <section
      id="dashboard-view"
      className="section-view"
      aria-labelledby="view-title"
    >
      {content}
    </section>
  );
}
