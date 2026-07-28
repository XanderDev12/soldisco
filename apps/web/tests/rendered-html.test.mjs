import assert from "node:assert/strict";
import { access, readFile } from "node:fs/promises";
import test from "node:test";

const webRoot = new URL("../", import.meta.url);
const dashboardRoot = new URL(
  "../app/components/dashboard/",
  import.meta.url,
);
const dashboardSourceFiles = [
  "DiscoveryDashboard.tsx",
  "DashboardTopbar.tsx",
  "DiscoveryCounters.tsx",
  "RejectionLog.tsx",
  "Sidebar.tsx",
  "StrategyUploadModal.tsx",
  "TokenBadges.tsx",
  "TokenInspector.tsx",
  "TokenStreamHeader.tsx",
  "TokenStreamView.tsx",
  "TokenTable.tsx",
  "TradeTicket.tsx",
  "WalletUnavailableToast.tsx",
  "executionModePresentation.ts",
  "navigation.ts",
  "types.ts",
  "useDashboardLayout.ts",
  "inspector/OverviewTab.tsx",
  "inspector/PositionTab.tsx",
  "inspector/RiskTab.tsx",
  "inspector/SignalsTab.tsx",
  "views/AlertsView.tsx",
  "views/ControlsView.tsx",
  "views/DashboardSectionView.tsx",
  "views/OrdersView.tsx",
  "views/PositionsView.tsx",
  "views/ReplaysView.tsx",
  "views/SectionViewPrimitives.tsx",
  "views/StrategiesView.tsx",
];

async function readDashboardSources() {
  return Object.fromEntries(
    await Promise.all(
      dashboardSourceFiles.map(async (path) => [
        path,
        await readFile(new URL(path, dashboardRoot), "utf8"),
      ]),
    ),
  );
}

async function render() {
  const workerUrl = new URL("../dist/server/index.js", import.meta.url);
  workerUrl.searchParams.set("test", `${process.pid}-${Date.now()}`);
  const { default: worker } = await import(workerUrl.href);

  return worker.fetch(
    new Request("http://localhost/", {
      headers: { accept: "text/html" },
    }),
    {
      ASSETS: {
        fetch: async () => new Response("Not found", { status: 404 }),
      },
    },
    {
      waitUntil() {},
      passThroughOnException() {},
    },
  );
}

test("server-renders the Soldisco discovery console", async () => {
  const response = await render();
  assert.equal(response.status, 200);
  assert.match(response.headers.get("content-type") ?? "", /^text\/html\b/i);

  const html = await response.text();
  assert.match(html, /<title>SolDisco — Solana Discovery Console<\/title>/i);
  assert.match(html, /SOLDISCO/);
  assert.match(html, /Discovery/);
  assert.match(html, /DISCOVERY STOPPED/);
  assert.match(html, /Start the stream when you are ready to screen candidates/);
  assert.match(html, /Pending/);
  assert.match(html, /Approved/);
  assert.match(html, /Rejected/);
  assert.match(html, /Flow rate/);
  assert.match(html, /Rejection log/);
  assert.match(html, /No strategy active/);
  assert.match(html, /Resize navigation/);
  assert.match(html, /Resize token inspector/);
  assert.doesNotMatch(html, /Resize positions tray/);
  const views = [
    ["discovery", "Discovery"],
    ["positions", "Positions"],
    ["orders", "Orders"],
    ["alerts", "Alerts"],
    ["strategies", "Strategies"],
    ["replays", "Replays"],
    ["controls", "Controls"],
  ];
  for (const [id, label] of views) {
    assert.match(html, new RegExp(`id="nav-${id}"`));
    assert.match(html, new RegExp(`<span>${label}</span>`));
  }
  assert.equal(
    (html.match(/aria-controls="dashboard-view"/g) ?? []).length,
    views.length,
  );
  assert.equal((html.match(/aria-current="page"/g) ?? []).length, 1);
  assert.match(
    html,
    /<button(?=[^>]*id="nav-discovery")(?=[^>]*aria-current="page")[^>]*>/,
  );
  assert.match(html, /id="dashboard-view"/);
  assert.match(html, /id="view-title"/);
  assert.match(html, /Start stream/);
  assert.match(html, /id="stream-status"[^>]*aria-live="polite"/);
  assert.match(html, /Stream stopped/);
  assert.doesNotMatch(
    html,
    /Reset panel sizes|↺ Layout|Local workspace|Development build/,
  );
  assert.doesNotMatch(html, />Inactive</);
  assert.doesNotMatch(
    html,
    /Connect wallet|wallet connection|wallet-backed|live holdings/i,
  );
  assert.doesNotMatch(
    html,
    /DEMO ENVIRONMENT|Fictional market data|Demo fixture|System healthy|Updated now|fixture source/i,
  );
  assert.doesNotMatch(html, /Ⅱ Pause|coming soon|placeholder/i);
  assert.doesNotMatch(html, /\b(?:NOVA|PXFRG|LUMA|ORBIT|PEBBLE|BLIP|TIDAL|MOSS|PIXEL)\b/i);
  assert.doesNotMatch(html, /codex-preview/i);
  assert.doesNotMatch(html, /Your site is taking shape/);
  assert.doesNotMatch(html, /react-loading-skeleton/);
});

test("keeps empty trackers and execution boundaries explicit", async () => {
  const [sources, page, layout, packageJson, styles, uiContract] =
    await Promise.all([
      readDashboardSources(),
      readFile(new URL("../app/page.tsx", import.meta.url), "utf8"),
      readFile(new URL("../app/layout.tsx", import.meta.url), "utf8"),
      readFile(new URL("../package.json", import.meta.url), "utf8"),
      readFile(new URL("../app/globals.css", import.meta.url), "utf8"),
      readFile(new URL("../../../packages/ui/src/index.ts", import.meta.url), "utf8"),
    ]);
  const dashboard = sources["DiscoveryDashboard.tsx"];
  const allDashboardSource = Object.values(sources).join("\n");

  assert.match(page, /<DiscoveryDashboard \/>/);
  assert.match(page, /components\/dashboard\/DiscoveryDashboard/);
  assert.match(layout, /SolDisco — Solana Discovery Console/);
  assert.match(
    allDashboardSource,
    /No route, quote, wallet, or execution service is connected/,
  );
  assert.match(
    sources["executionModePresentation.ts"],
    /Record paper \$\{side\} · unavailable/,
  );
  assert.match(
    sources["executionModePresentation.ts"],
    /Review \$\{side\} · Live mode/,
  );
  assert.match(sources["useDashboardLayout.ts"], /soldisco\.layout\.v1/);
  assert.match(allDashboardSource, /role="separator"/);
  assert.match(sources["inspector/RiskTab.tsx"], /token\.checks\.length/);
  assert.match(dashboard, /setActiveView\(view\)/);
  assert.match(
    sources["Sidebar.tsx"],
    /onClick=\{\(\) => onOpenView\(item\.id\)\}/,
  );
  assert.match(dashboard, /setStreamRunning\(\(running\) => !running\)/);
  assert.match(
    dashboard,
    /initialTradeDrafts: Record<ExecutionMode, TradeDraft>/,
  );
  assert.match(dashboard, /const tradeDraft = tradeDrafts\[mode\]/);
  assert.match(dashboard, /\[mode\]: \{ \.\.\.current\[mode\], side \}/);
  assert.match(dashboard, /\[mode\]: \{ \.\.\.current\[mode\], amount \}/);
  assert.doesNotMatch(allDashboardSource, /item\.active/);
  assert.match(allDashboardSource, /disabled/);
  assert.match(sources["DiscoveryCounters.tsx"], /summary\.pending/);
  assert.match(sources["DiscoveryCounters.tsx"], /summary\.approved/);
  assert.match(sources["DiscoveryCounters.tsx"], /summary\.rejected/);
  assert.match(sources["RejectionLog.tsx"], /Initial-screen failures/);
  assert.match(sources["TokenTable.tsx"], /approvedTokens\.map/);
  assert.doesNotMatch(sources["TokenTable.tsx"], /<th>First pass<\/th>/);
  assert.match(
    sources["executionModePresentation.ts"],
    /No paper positions recorded/,
  );
  assert.match(
    sources["executionModePresentation.ts"],
    /Live position data unavailable/,
  );
  assert.match(
    sources["executionModePresentation.ts"],
    /No paper orders recorded/,
  );
  assert.match(
    sources["executionModePresentation.ts"],
    /Live order data unavailable/,
  );
  assert.match(
    sources["executionModePresentation.ts"],
    /Paper:[\s\S]*requiresWallet: false/,
  );
  assert.match(
    sources["executionModePresentation.ts"],
    /Live:[\s\S]*requiresWallet: true/,
  );
  assert.match(sources["DashboardTopbar.tsx"], /mode === "Live"/);
  assert.match(
    sources["TradeTicket.tsx"],
    /presentation\.requiresWallet/,
  );
  assert.match(
    sources["views/PositionsView.tsx"],
    /presentation\.requiresWallet/,
  );
  assert.match(sources["views/AlertsView.tsx"], /No alerts configured/);
  assert.match(sources["views/StrategiesView.tsx"], /No validated strategies/);
  assert.match(sources["views/ReplaysView.tsx"], /No replay data/);
  assert.match(sources["views/ControlsView.tsx"], /Discovery source/);
  assert.doesNotMatch(
    allDashboardSource,
    /\bfetch\s*\(|\bWebSocket\s*\(|\bEventSource\b|\bMath\.random\b|\bsetInterval\b/,
  );
  assert.doesNotMatch(
    allDashboardSource,
    /DEMO ENVIRONMENT|Fictional market data|Demo fixture|System healthy|Updated now|demo tokens|fixture source|Demo data|Demo evaluation|demo wallet|Planned metrics|DEMO TICKET|3 checks/i,
  );
  assert.doesNotMatch(
    allDashboardSource,
    /"(?:NOVA|PXFRG|LUMA|ORBIT|PEBBLE|BLIP|TIDAL|MOSS|PIXEL)"|\$(?:84\.20|42\.80|127\.00)|[+-]\$(?:6\.42|1\.06|5\.36)|\b(?:18\.4|41\.7|1\.2)\b/i,
  );
  assert.doesNotMatch(
    styles,
    /token-logo--(?:luma|orbit|pebble|blip|tidal|nova|moss|pixel)/i,
  );
  assert.doesNotMatch(allDashboardSource, /Add to watchlist|Watchlist/);
  assert.doesNotMatch(allDashboardSource, /PositionsTray|positions-tray/);
  assert.doesNotMatch(
    allDashboardSource,
    /onResetLayout|resetLayout|layout-reset|strategy-control__state|sidebar__footer/,
  );
  assert.doesNotMatch(
    allDashboardSource,
    /Reset panel sizes|↺ Layout|Local workspace|Development build/,
  );
  assert.doesNotMatch(styles, /layout-reset|strategy-control__state|sidebar__footer/);
  assert.doesNotMatch(packageJson, /react-loading-skeleton/);
  assert.match(
    uiContract,
    /mode: "PAPER";[\s\S]*requiresWalletConfirmation: false/,
  );
  assert.match(
    uiContract,
    /mode: "LIVE";[\s\S]*requiresWalletConfirmation: true/,
  );
  assert.match(
    uiContract,
    /type HeldPositionViewModel =[\s\S]*mode: "PAPER"[\s\S]*mode: "LIVE"/,
  );
  assert.match(
    uiContract,
    /executionMode: "PAPER";[\s\S]*Extract<TradeTicketViewModel, \{ mode: "PAPER" \}>/,
  );
  assert.match(
    uiContract,
    /executionMode: "LIVE";[\s\S]*Extract<TradeTicketViewModel, \{ mode: "LIVE" \}>/,
  );

  await assert.rejects(access(new URL("../app/_sites-preview", import.meta.url)));
  await assert.rejects(
    access(new URL("../app/components/DiscoveryDashboard.tsx", import.meta.url)),
  );
  await assert.rejects(
    access(new URL("../app/components/DashboardSectionView.tsx", import.meta.url)),
  );
  for (const removedPath of [
    "PositionsTray.tsx",
    "views/InitialApprovalView.tsx",
    "views/WatchlistView.tsx",
  ]) {
    await assert.rejects(access(new URL(removedPath, dashboardRoot)));
  }
  await access(new URL(".openai/hosting.json", webRoot));
});

test("keeps dashboard UI split across focused modules", async () => {
  const sources = await readDashboardSources();
  const lineLimits = {
    "DiscoveryDashboard.tsx": 240,
    "TokenStreamView.tsx": 180,
    "TokenInspector.tsx": 180,
    "views/DashboardSectionView.tsx": 120,
  };

  assert.equal(Object.keys(sources).length, dashboardSourceFiles.length);
  for (const [path, maximumLines] of Object.entries(lineLimits)) {
    const lineCount = sources[path].split("\n").length;
    assert.ok(
      lineCount <= maximumLines,
      `${path} should stay focused (${lineCount}/${maximumLines} lines)`,
    );
  }

  for (const path of [
    "views/PositionsView.tsx",
    "views/OrdersView.tsx",
    "views/AlertsView.tsx",
    "views/StrategiesView.tsx",
    "views/ReplaysView.tsx",
    "views/ControlsView.tsx",
    "inspector/OverviewTab.tsx",
    "inspector/RiskTab.tsx",
    "inspector/SignalsTab.tsx",
    "inspector/PositionTab.tsx",
  ]) {
    await access(new URL(path, dashboardRoot));
  }
});
