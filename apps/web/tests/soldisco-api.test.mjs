import assert from "node:assert/strict";
import { after, before, test } from "node:test";
import { createServer } from "vite";

const apiRoot = new URL("../app/lib/soldisco-api/", import.meta.url);
let vite;

before(async () => {
  vite = await createServer({
    configFile: false,
    appType: "custom",
    server: { middlewareMode: true },
  });
});

after(async () => {
  await vite.close();
});

async function loadApiModule(path) {
  return vite.ssrLoadModule(new URL(path, apiRoot).pathname);
}

async function loadDashboardModule(path) {
  return vite.ssrLoadModule(
    new URL(`../app/components/dashboard/${path}`, import.meta.url).pathname,
  );
}

function discoveryFixture() {
  return {
    sequence: 8,
    mode: "OBSERVE_ALL",
    tokens: [
      {
        mint: "mint-1",
        name: null,
        symbol: "ONE",
        primary_venue: "PUMP_BONDING_CURVE",
        market_address: "market-1",
        quote_mint: null,
        source_program: "PUMP",
        stage: "OBSERVED",
        last_event_kind: "TRADE",
        observed_slot: 42,
        first_observed_unix_ms: 1_700_000_000_000,
        last_observed_unix_ms: 1_700_000_001_000,
        latest_signature: "signature-1",
        activity: {
          trades: 3,
          buys: 2,
          sells: 1,
          unique_traders: 2,
          base_volume_units: "9007199254740993",
          quote_volume_units: "2500",
        },
        risk_score: null,
        opportunity_score: null,
      },
    ],
    tokens_total: 1,
    tokens_truncated: false,
    counters: {
      observed: 1,
      pending: 0,
      approved: 0,
      rejected: 0,
      flow_per_minute: null,
    },
    rejection_reasons: [],
  };
}

test("parses and maps the exact observe-all browser contract", async () => {
  const [{ parseDiscoverySnapshot }, { mapDiscoverySnapshot }] =
    await Promise.all([
      loadApiModule("parsers.ts"),
      loadApiModule("mappers.ts"),
    ]);

  const parsed = parseDiscoverySnapshot(discoveryFixture());
  const mapped = mapDiscoverySnapshot(parsed);

  assert.equal(mapped.mode, "OBSERVE_ALL");
  assert.equal(mapped.tokensTotal, 1);
  assert.equal(mapped.tokensTruncated, false);
  assert.equal(mapped.tokens[0].stageLabel, "Observed");
  assert.equal(mapped.tokens[0].primaryVenue, "Pump bonding curve");
  assert.equal(mapped.tokens[0].quoteMint, null);
  assert.equal(mapped.tokens[0].activity.baseVolumeUnits, "9007199254740993");
  assert.equal(mapped.tokens[0].riskScore, null);
  assert.equal(mapped.summary.ratePerMinute, null);

  const invalid = discoveryFixture();
  invalid.tokens[0].risk_score = 101;
  assert.throws(
    () => parseDiscoverySnapshot(invalid),
    /\$\.tokens\[0\]\.risk_score/,
  );
});

test("derives truthful controls from full stream state", async () => {
  const { mapBackendStatus, mapStreamControl } =
    await loadApiModule("mappers.ts");

  const degraded = mapStreamControl(
    { status: "DEGRADED", requested_running: true },
    null,
    true,
  );
  assert.equal(degraded.shouldStop, true);
  assert.equal(degraded.buttonLabel, "Stop stream");
  assert.equal(degraded.running, false);
  assert.equal(degraded.canCommand, true);

  const starting = mapStreamControl(
    { status: "STARTING", requested_running: true },
    "START",
    true,
  );
  assert.equal(starting.buttonLabel, "Starting…");
  assert.equal(starting.canCommand, false);

  const unavailable = mapStreamControl(null, null, false);
  assert.equal(unavailable.statusLabel, "Unavailable");
  assert.equal(unavailable.canCommand, false);

  const staleBackend = mapBackendStatus({
    connection: "UNAVAILABLE",
    health: { status: "UP", database: "UP", stream: "RUNNING" },
    healthFresh: false,
    stream: { status: "RUNNING", requested_running: true },
    streamFresh: false,
    liveUpdates: "RETRYING",
    command: null,
    error: null,
  });
  assert.equal(staleBackend.overall, null);
  assert.equal(staleBackend.database, null);
  assert.equal(staleBackend.stream.status, null);
  assert.equal(staleBackend.stream.statusLabel, "Unavailable");
});

test("preserves structured API errors and falls back safely", async () => {
  const { errorFromResponse } = await loadApiModule("errors.ts");

  const structured = await errorFromResponse(
    new Response(
      JSON.stringify({
        error: {
          code: "PUMP_COLLECTOR_UNAVAILABLE",
          message: "Pump collection is unavailable.",
        },
      }),
      {
        status: 503,
        headers: { "content-type": "application/json" },
      },
    ),
  );
  assert.equal(structured.code, "PUMP_COLLECTOR_UNAVAILABLE");
  assert.equal(structured.status, 503);
  assert.equal(structured.message, "Pump collection is unavailable.");

  const fallback = await errorFromResponse(
    new Response("not json", { status: 502 }),
  );
  assert.equal(fallback.code, "HTTP_ERROR");
  assert.equal(fallback.status, 502);
});

test("uses versioned routes and authenticates only local control commands", async () => {
  const { SoldiscoApiClient } = await loadApiModule("client.ts");
  const requests = [];
  const client = new SoldiscoApiClient(
    "http://127.0.0.1:8080/api/v1",
    async (url, init) => {
      requests.push([url, init.method, init.headers]);

      if (url.endsWith("/health")) {
        return Response.json(
          { status: "DEGRADED", database: "DOWN", stream: "STOPPED" },
          { status: 503 },
        );
      }
      if (url.endsWith("/stream/start")) {
        return Response.json({ status: "STARTING", changed: true });
      }
      if (url.endsWith("/stream/stop")) {
        return Response.json(
          {
            error: {
              code: "COLLECTOR_UNAVAILABLE",
              message: "Collector unavailable.",
            },
          },
          { status: 503 },
        );
      }
      throw new Error(`Unexpected test URL: ${url}`);
    },
  );

  const health = await client.health;
  assert.equal(health.database, "DOWN");
  const started = await client.startStream();
  assert.equal(started.status, "STARTING");
  await assert.rejects(
    client.stopStream(),
    (error) =>
      error.code === "COLLECTOR_UNAVAILABLE" &&
      error.status === 503,
  );
  assert.deepEqual(requests, [
    [
      "http://127.0.0.1:8080/api/v1/health",
      "GET",
      { Accept: "application/json" },
    ],
    [
      "http://127.0.0.1:8080/api/v1/stream/start",
      "POST",
      {
        Accept: "application/json",
        "X-Soldisco-Control": "soldisco-local-ui-v1",
      },
    ],
    [
      "http://127.0.0.1:8080/api/v1/stream/stop",
      "POST",
      {
        Accept: "application/json",
        "X-Soldisco-Control": "soldisco-local-ui-v1",
      },
    ],
  ]);
});

test("listens to the named soldisco event and reports reconnect state", async () => {
  const { subscribeToSoldiscoEvents } = await loadApiModule("events.ts");
  const statuses = [];
  const envelopes = [];
  const errors = [];
  let source;
  let sourceUrl;

  const close = subscribeToSoldiscoEvents(
    "http://127.0.0.1:8080/api/v1",
    {
      onEnvelope: (envelope) => envelopes.push(envelope),
      onStatus: (status) => statuses.push(status),
      onError: (error) => errors.push(error),
    },
    (url) => {
      sourceUrl = url;
      source = {
        onopen: null,
        onerror: null,
        listeners: new Map(),
        addEventListener(type, listener) {
          this.listeners.set(type, listener);
        },
        close() {
          this.closed = true;
        },
      };
      return source;
    },
  );

  assert.equal(sourceUrl, "http://127.0.0.1:8080/api/v1/events");
  assert.deepEqual(statuses, ["CONNECTING"]);

  source.onopen(new Event("open"));
  source.listeners.get("soldisco")({
    data: JSON.stringify({
      sequence: 4,
      event: { type: "DISCOVERY_PROJECTION_CHANGED" },
    }),
  });
  source.onerror(new Event("error"));

  assert.equal(envelopes[0].event.type, "DISCOVERY_PROJECTION_CHANGED");
  assert.deepEqual(statuses, ["CONNECTING", "OPEN", "RETRYING"]);
  assert.equal(errors.length, 0);

  close();
  assert.equal(source.closed, true);
  assert.equal(statuses.at(-1), "CLOSED");
});

test("coalesces discovery bursts and rejects regressing snapshots", async () => {
  const { createDiscoveryRefreshCoordinator } =
    await loadApiModule("discoveryRefresh.ts");
  const pending = [];
  const accepted = [];
  let readCount = 0;

  function deferred() {
    let resolve;
    const promise = new Promise((complete) => {
      resolve = complete;
    });
    return { promise, resolve };
  }

  const first = deferred();
  const second = deferred();
  pending.push(first.promise, second.promise);

  const coordinator = createDiscoveryRefreshCoordinator(
    () => {
      readCount += 1;
      return pending.shift();
    },
    (snapshot) => accepted.push(snapshot.sequence),
  );

  const burstOne = coordinator.refresh();
  const burstTwo = coordinator.refresh();
  const burstThree = coordinator.refresh();
  assert.strictEqual(burstOne, burstTwo);
  assert.strictEqual(burstTwo, burstThree);
  assert.equal(readCount, 1);

  first.resolve({ ...discoveryFixture(), sequence: 8 });
  await Promise.resolve();
  await Promise.resolve();
  assert.equal(readCount, 2);

  second.resolve({ ...discoveryFixture(), sequence: 9 });
  await burstOne;
  assert.deepEqual(accepted, [8, 9]);

  pending.push(
    Promise.resolve({ ...discoveryFixture(), sequence: 7 }),
  );
  await coordinator.refresh();
  assert.deepEqual(accepted, [8, 9]);
  coordinator.dispose();
});

test("accepts only the latest response generation for each surface", async () => {
  const { createResponseFreshnessCoordinator } =
    await loadApiModule("responseFreshness.ts");
  const coordinator = createResponseFreshnessCoordinator();

  const firstStream = coordinator.begin("stream");
  const health = coordinator.begin("health");
  const secondStream = coordinator.begin("stream");

  assert.equal(coordinator.isCurrent(firstStream), false);
  assert.equal(coordinator.isCurrent(health), true);
  assert.equal(coordinator.isCurrent(secondStream), true);

  coordinator.invalidate();
  assert.equal(coordinator.isCurrent(health), false);
  assert.equal(coordinator.isCurrent(secondStream), false);
});

test("describes degraded and transitional empty stream states honestly", async () => {
  const { describeEmptyStream } =
    await loadDashboardModule("emptyStreamState.ts");

  assert.deepEqual(
    describeEmptyStream({
      backendConnected: true,
      streamStatus: "STARTING",
      requestedRunning: true,
    }),
    {
      title: "Collector is starting",
      detail:
        "The backend is restoring its state and opening the Pump data sources.",
    },
  );
  assert.match(
    describeEmptyStream({
      backendConnected: true,
      streamStatus: "DEGRADED",
      requestedRunning: true,
    }).detail,
    /still requested/,
  );
  assert.match(
    describeEmptyStream({
      backendConnected: true,
      streamStatus: "ERROR",
      requestedRunning: false,
    }).title,
    /error/i,
  );
});

test("refuses local API traffic from hosted pages", async () => {
  const { resolveLocalApiUrl } = await loadApiModule("config.ts");

  assert.equal(
    resolveLocalApiUrl("http://localhost:3000").baseUrl,
    "http://127.0.0.1:8080/api/v1",
  );
  assert.equal(
    resolveLocalApiUrl("https://soldisco.example.com").reason,
    "NON_LOCAL_PAGE",
  );
  assert.equal(
    resolveLocalApiUrl(
      "http://localhost:3000",
      "https://api.example.com",
    ).reason,
    "NON_LOCAL_API",
  );
  assert.equal(
    resolveLocalApiUrl("http://127.0.0.1:3000").reason,
    "LOCAL_ORIGIN_MISMATCH",
  );
  assert.equal(
    resolveLocalApiUrl(
      "http://127.0.0.1:3000",
      "http://127.0.0.1:8080/api/v1",
      "http://127.0.0.1:3000",
    ).reason,
    "LOCAL",
  );
});
