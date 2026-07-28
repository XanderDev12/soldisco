import assert from "node:assert/strict";
import { access, readFile } from "node:fs/promises";
import test from "node:test";

const webRoot = new URL("../", import.meta.url);

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
  assert.match(html, /All coins/);
  assert.match(html, /DISCOVERY STOPPED/);
  assert.match(html, /Start the stream when you are ready to receive candidates/);
  assert.match(html, /No strategy active/);
  assert.match(html, /Position data unavailable/);
  assert.match(html, /Resize navigation/);
  assert.match(html, /Resize token inspector/);
  assert.match(html, /Resize positions tray/);
  const views = [
    ["token-stream", "Token stream"],
    ["initial-approval", "Initial approval"],
    ["watchlist", "Watchlist"],
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
    /<button(?=[^>]*id="nav-token-stream")(?=[^>]*aria-current="page")[^>]*>/,
  );
  assert.match(html, /id="dashboard-view"/);
  assert.match(html, /id="view-title"/);
  assert.match(html, /Start stream/);
  assert.match(html, /id="stream-status"[^>]*aria-live="polite"/);
  assert.match(html, /Stream stopped/);
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
  const [dashboard, sectionViews, page, layout, packageJson, styles] =
    await Promise.all([
      readFile(new URL("../app/components/DiscoveryDashboard.tsx", import.meta.url), "utf8"),
      readFile(new URL("../app/components/DashboardSectionView.tsx", import.meta.url), "utf8"),
      readFile(new URL("../app/page.tsx", import.meta.url), "utf8"),
      readFile(new URL("../app/layout.tsx", import.meta.url), "utf8"),
      readFile(new URL("../package.json", import.meta.url), "utf8"),
      readFile(new URL("../app/globals.css", import.meta.url), "utf8"),
    ]);

  assert.match(page, /<DiscoveryDashboard \/>/);
  assert.match(layout, /SolDisco — Solana Discovery Console/);
  assert.match(dashboard, /No route, quote, wallet, or execution service is connected/);
  assert.match(dashboard, /Review \{side\.toLowerCase\(\)\}/);
  assert.match(dashboard, /soldisco\.layout\.v1/);
  assert.match(dashboard, /role="separator"/);
  assert.match(dashboard, /selected\.checks\.length/);
  assert.match(dashboard, /setActiveView\(view\)/);
  assert.match(dashboard, /onClick=\{\(\) => openView\(item\.id\)\}/);
  assert.match(dashboard, /setStreamRunning\(\(running\) => !running\)/);
  assert.doesNotMatch(dashboard, /item\.active/);
  assert.match(dashboard, /disabled/);
  assert.match(sectionViews, /No first-pass results/);
  assert.match(sectionViews, /Watchlist is empty/);
  assert.match(sectionViews, /Position data unavailable/);
  assert.match(sectionViews, /Order data unavailable/);
  assert.match(sectionViews, /No alerts configured/);
  assert.match(sectionViews, /No validated strategies/);
  assert.match(sectionViews, /No replay data/);
  assert.match(sectionViews, /Discovery source/);
  assert.doesNotMatch(dashboard, /\bfetch\s*\(/);
  assert.doesNotMatch(dashboard, /\bWebSocket\s*\(/);
  assert.doesNotMatch(sectionViews, /\bfetch\s*\(|\bWebSocket\s*\(/);
  assert.doesNotMatch(dashboard, /\bEventSource\b|\bMath\.random\b|\bsetInterval\b/);
  assert.doesNotMatch(sectionViews, /\bEventSource\b|\bMath\.random\b|\bsetInterval\b/);
  assert.doesNotMatch(
    dashboard,
    /DEMO ENVIRONMENT|Fictional market data|Demo fixture|System healthy|Updated now|demo tokens|fixture source|Demo data|Demo evaluation|demo wallet|Planned metrics|DEMO TICKET|3 checks/i,
  );
  assert.doesNotMatch(
    dashboard,
    /"(?:NOVA|PXFRG|LUMA|ORBIT|PEBBLE|BLIP|TIDAL|MOSS|PIXEL)"|\$(?:84\.20|42\.80|127\.00)|[+-]\$(?:6\.42|1\.06|5\.36)|\b(?:18\.4|41\.7|1\.2)\b/i,
  );
  assert.doesNotMatch(
    styles,
    /token-logo--(?:luma|orbit|pebble|blip|tidal|nova|moss|pixel)/i,
  );
  assert.doesNotMatch(packageJson, /react-loading-skeleton/);

  await assert.rejects(access(new URL("../app/_sites-preview", import.meta.url)));
  await access(new URL(".openai/hosting.json", webRoot));
});
