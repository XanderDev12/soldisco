import assert from "node:assert/strict";
import { after, before, test } from "node:test";
import { act, createElement } from "react";
import { JSDOM } from "jsdom";
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
        qualification: null,
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
      qualified: 0,
      qualification_pending: 1,
      qualification_rejected: 0,
      qualification_unknown: 0,
      processing_failures: 0,
      flow_per_minute: null,
    },
    rejection_reasons: [],
  };
}

function qualificationSummaryFixture() {
  return {
    window_revision: 1,
    ruleset_revision: 4,
    opened_unix_ms: 1_700_000_000_000,
    closed_unix_ms: 1_700_000_060_000,
    evaluated_unix_ms: 1_700_000_060_100,
    decision: "PASS",
    completeness: "COMPLETE",
    reason_codes: [],
    trades: 8,
    buys: 5,
    sells: 3,
    unique_traders: 6,
    unique_buyers: 4,
    unique_sellers: 3,
    buy_base_volume_units: "3000",
    sell_base_volume_units: "1000",
    buy_quote_volume_units: "500000000",
    sell_quote_volume_units: "200000000",
    maximum_single_wallet_quote_share_bps: 4200,
    price_change_bps: -125,
    first_base_reserve_units: "1000000",
    first_quote_reserve_units: "500000",
    latest_base_reserve_units: "999000",
    latest_quote_reserve_units: "501000",
  };
}

function prefilterDefaultsFixture() {
  return {
    revision: 3,
    values: {
      max_event_age_ms: 15_000,
      observation_window_ms: 60_000,
      max_active_windows: 128,
      rpc_requests_per_second: 1,
      rpc_max_in_flight: 4,
      rpc_request_timeout_ms: 5_000,
      rpc_rate_limit_cooldown_ms: 5_000,
    },
    bounds: {
      max_event_age_ms: { minimum: 1_000, maximum: 300_000 },
      observation_window_ms: { minimum: 1_000, maximum: 3_600_000 },
      max_active_windows: { minimum: 1, maximum: 100_000 },
      rpc_requests_per_second: { minimum: 1, maximum: 1_000 },
      rpc_max_in_flight: { minimum: 1, maximum: 128 },
      rpc_request_timeout_ms: { minimum: 1, maximum: 300_000 },
      rpc_rate_limit_cooldown_ms: { minimum: 100, maximum: 300_000 },
    },
    apply_requirement: "STREAM_RESTART",
  };
}

function qualificationDefaultsFixture() {
  return {
    revision: 4,
    values: {
      minimum_trades: 8,
      minimum_unique_traders: 4,
      minimum_buys: 3,
      minimum_sells: 1,
      minimum_native_quote_volume_units: 500_000_000,
      minimum_stable_quote_volume_units: 50_000_000,
      maximum_single_wallet_quote_share_bps: 6500,
    },
    bounds: {
      minimum_trades: { minimum: 1, maximum: 10_000 },
      minimum_unique_traders: { minimum: 1, maximum: 10_000 },
      minimum_buys: { minimum: 0, maximum: 10_000 },
      minimum_sells: { minimum: 0, maximum: 10_000 },
      minimum_native_quote_volume_units: {
        minimum: 0,
        maximum: 9_000_000_000_000,
      },
      minimum_stable_quote_volume_units: {
        minimum: 0,
        maximum: 9_000_000_000_000,
      },
      maximum_single_wallet_quote_share_bps: {
        minimum: 1_000,
        maximum: 10_000,
      },
    },
    apply_requirement: "NEW_WINDOWS",
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

  const qualified = discoveryFixture();
  qualified.mode = "QUALIFIED_ONLY";
  qualified.tokens[0].stage = "QUALIFIED";
  qualified.tokens[0].qualification = qualificationSummaryFixture();
  qualified.counters.qualified = 1;
  qualified.counters.qualification_pending = 0;
  const qualifiedMapped = mapDiscoverySnapshot(
    parseDiscoverySnapshot(qualified),
  );
  assert.equal(qualifiedMapped.mode, "QUALIFIED_ONLY");
  assert.equal(qualifiedMapped.tokens[0].stageLabel, "Qualified");
  assert.equal(
    qualifiedMapped.tokens[0].qualification.ruleset_revision,
    4,
  );
  assert.equal(
    qualifiedMapped.tokens[0].qualification.price_change_bps,
    -125,
  );

  const impossibleShare = discoveryFixture();
  impossibleShare.tokens[0].qualification = qualificationSummaryFixture();
  impossibleShare.tokens[0].qualification.maximum_single_wallet_quote_share_bps =
    10_001;
  assert.throws(
    () => parseDiscoverySnapshot(impossibleShare),
    /maximum_single_wallet_quote_share_bps/,
  );

  const missingQualification = discoveryFixture();
  missingQualification.tokens[0].stage = "QUALIFIED";
  assert.throws(
    () => parseDiscoverySnapshot(missingQualification),
    /\$\.tokens\[0\]\.qualification/,
  );

  const rejectedQualification = discoveryFixture();
  rejectedQualification.tokens[0].stage = "QUALIFIED";
  rejectedQualification.tokens[0].qualification =
    qualificationSummaryFixture();
  rejectedQualification.tokens[0].qualification.decision = "REJECT";
  assert.throws(
    () => parseDiscoverySnapshot(rejectedQualification),
    /complete PASS evidence/,
  );

  const unqualifiedApproval = discoveryFixture();
  unqualifiedApproval.tokens[0].stage = "APPROVED";
  assert.throws(
    () => parseDiscoverySnapshot(unqualifiedApproval),
    /complete PASS evidence/,
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
    apiBaseUrl: "http://127.0.0.1:8080/api/v1",
    connection: "UNAVAILABLE",
    health: { status: "UP", database: "UP", stream: "RUNNING" },
    healthFresh: false,
    stream: { status: "RUNNING", requested_running: true },
    streamFresh: false,
    liveUpdates: "RETRYING",
    command: null,
    error: null,
  });
  assert.equal(
    staleBackend.apiBaseUrl,
    "http://127.0.0.1:8080/api/v1",
  );
  assert.equal(staleBackend.overall, null);
  assert.equal(staleBackend.database, null);
  assert.equal(staleBackend.stream.status, null);
  assert.equal(staleBackend.stream.statusLabel, "Unavailable");
});

test("strictly parses bounded prefilter defaults", async () => {
  const { parsePrefilterDefaults } = await loadApiModule("parsers.ts");
  const parsed = parsePrefilterDefaults(prefilterDefaultsFixture());

  assert.equal(parsed.revision, 3);
  assert.equal(parsed.values.max_event_age_ms, 15_000);
  assert.equal(parsed.bounds.rpc_max_in_flight.maximum, 128);
  assert.equal(parsed.apply_requirement, "STREAM_RESTART");

  const outOfBounds = prefilterDefaultsFixture();
  outOfBounds.values.max_event_age_ms = 300_001;
  assert.throws(
    () => parsePrefilterDefaults(outOfBounds),
    /\$\.values\.max_event_age_ms/,
  );

  const invertedBounds = prefilterDefaultsFixture();
  invertedBounds.bounds.rpc_max_in_flight = {
    minimum: 10,
    maximum: 5,
  };
  assert.throws(
    () => parsePrefilterDefaults(invertedBounds),
    /\$\.bounds\.rpc_max_in_flight\.maximum/,
  );

  const unsupportedApply = prefilterDefaultsFixture();
  unsupportedApply.apply_requirement = "LIVE";
  assert.throws(
    () => parsePrefilterDefaults(unsupportedApply),
    /\$\.apply_requirement/,
  );

  const impossibleTimeout = prefilterDefaultsFixture();
  impossibleTimeout.values.rpc_request_timeout_ms = 16_000;
  assert.throws(
    () => parsePrefilterDefaults(impossibleTimeout),
    /\$\.values\.rpc_request_timeout_ms/,
  );

  const impossibleWindow = prefilterDefaultsFixture();
  impossibleWindow.values.observation_window_ms = 4_000;
  assert.throws(
    () => parsePrefilterDefaults(impossibleWindow),
    /\$\.values\.observation_window_ms/,
  );
});

test("strictly parses bounded qualification defaults", async () => {
  const { parseQualificationDefaults } =
    await loadApiModule("parsers.ts");
  const parsed = parseQualificationDefaults(
    qualificationDefaultsFixture(),
  );

  assert.equal(parsed.revision, 4);
  assert.equal(parsed.values.minimum_trades, 8);
  assert.equal(
    parsed.values.maximum_single_wallet_quote_share_bps,
    6500,
  );
  assert.equal(parsed.apply_requirement, "NEW_WINDOWS");

  const relationshipViolation = qualificationDefaultsFixture();
  relationshipViolation.values.minimum_unique_traders = 9;
  assert.throws(
    () => parseQualificationDefaults(relationshipViolation),
    /\$\.values\.minimum_unique_traders/,
  );

  const unsupportedApply = qualificationDefaultsFixture();
  unsupportedApply.apply_requirement = "STREAM_RESTART";
  assert.throws(
    () => parseQualificationDefaults(unsupportedApply),
    /\$\.apply_requirement/,
  );

  const outOfBounds = qualificationDefaultsFixture();
  outOfBounds.values.maximum_single_wallet_quote_share_bps = 10_001;
  assert.throws(
    () => parseQualificationDefaults(outOfBounds),
    /\$\.values\.maximum_single_wallet_quote_share_bps/,
  );
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

test("reads and updates prefilter defaults through the guarded contract", async () => {
  const { SoldiscoApiClient } = await loadApiModule("client.ts");
  const requests = [];
  const client = new SoldiscoApiClient(
    "http://127.0.0.1:8080/api/v1",
    async (url, init) => {
      requests.push({
        url,
        method: init.method,
        headers: init.headers,
        body: init.body,
      });
      return Response.json(prefilterDefaultsFixture());
    },
  );

  const current = await client.prefilterDefaults;
  const request = {
    expected_revision: current.revision,
    values: {
      ...current.values,
      max_event_age_ms: 20_000,
    },
  };
  await client.updatePrefilterDefaults(request);

  assert.deepEqual(requests, [
    {
      url: "http://127.0.0.1:8080/api/v1/settings/prefilter-defaults",
      method: "GET",
      headers: { Accept: "application/json" },
      body: undefined,
    },
    {
      url: "http://127.0.0.1:8080/api/v1/settings/prefilter-defaults",
      method: "PUT",
      headers: {
        Accept: "application/json",
        "X-Soldisco-Control": "soldisco-local-ui-v1",
        "Content-Type": "application/json",
      },
      body: JSON.stringify(request),
    },
  ]);
});

test("reads and updates qualification defaults through the guarded contract", async () => {
  const { SoldiscoApiClient } = await loadApiModule("client.ts");
  const requests = [];
  const client = new SoldiscoApiClient(
    "http://127.0.0.1:8080/api/v1",
    async (url, init) => {
      requests.push({
        url,
        method: init.method,
        headers: init.headers,
        body: init.body,
      });
      return Response.json(qualificationDefaultsFixture());
    },
  );

  const current = await client.qualificationDefaults;
  const request = {
    expected_revision: current.revision,
    values: {
      ...current.values,
      minimum_trades: 10,
    },
  };
  await client.updateQualificationDefaults(request);

  assert.deepEqual(requests, [
    {
      url: "http://127.0.0.1:8080/api/v1/settings/qualification-defaults",
      method: "GET",
      headers: { Accept: "application/json" },
      body: undefined,
    },
    {
      url: "http://127.0.0.1:8080/api/v1/settings/qualification-defaults",
      method: "PUT",
      headers: {
        Accept: "application/json",
        "X-Soldisco-Control": "soldisco-local-ui-v1",
        "Content-Type": "application/json",
      },
      body: JSON.stringify(request),
    },
  ]);
});

test("validates human-readable prefilter drafts without losing milliseconds", async () => {
  const {
    prefilterDraftHasChanges,
    toPrefilterDefaultsDraft,
    validatePrefilterDefaultsDraft,
  } = await loadDashboardModule("controls/prefilterDefaultsDraft.ts");
  const settings = prefilterDefaultsFixture();
  const baseline = toPrefilterDefaultsDraft(settings.values);

  assert.equal(baseline.max_event_age_ms, "15");
  assert.equal(baseline.observation_window_ms, "60");
  assert.equal(baseline.rpc_requests_per_second, "1");

  const equivalent = {
    ...baseline,
    max_event_age_ms: "15.0",
  };
  const equivalentValidation = validatePrefilterDefaultsDraft(
    equivalent,
    settings.bounds,
  );
  assert.equal(equivalentValidation.valid, true);
  assert.equal(
    prefilterDraftHasChanges(
      equivalent,
      settings.values,
      equivalentValidation,
    ),
    false,
  );

  const changed = {
    ...baseline,
    max_event_age_ms: "20.125",
  };
  const changedValidation = validatePrefilterDefaultsDraft(
    changed,
    settings.bounds,
  );
  assert.equal(changedValidation.valid, true);
  assert.equal(changedValidation.values.max_event_age_ms, 20_125);
  assert.equal(
    prefilterDraftHasChanges(changed, settings.values, changedValidation),
    true,
  );

  const tooPrecise = {
    ...baseline,
    rpc_request_timeout_ms: "1.0001",
  };
  const invalidValidation = validatePrefilterDefaultsDraft(
    tooPrecise,
    settings.bounds,
  );
  assert.equal(invalidValidation.valid, false);
  assert.match(
    invalidValidation.errors.rpc_request_timeout_ms,
    /three decimal places/,
  );

  const unsafeRelationship = {
    ...baseline,
    max_event_age_ms: "4",
  };
  const unsafeValidation = validatePrefilterDefaultsDraft(
    unsafeRelationship,
    settings.bounds,
  );
  assert.equal(unsafeValidation.valid, false);
  assert.match(
    unsafeValidation.errors.rpc_request_timeout_ms,
    /cannot exceed the maximum creation age/,
  );
});

test("validates qualification relationships and basis-point display values", async () => {
  const {
    qualificationDraftHasChanges,
    toQualificationDefaultsDraft,
    validateQualificationDefaultsDraft,
  } = await loadDashboardModule("controls/qualificationDefaultsDraft.ts");
  const settings = qualificationDefaultsFixture();
  const baseline = toQualificationDefaultsDraft(settings.values);

  assert.equal(baseline.minimum_trades, "8");
  assert.equal(baseline.maximum_single_wallet_quote_share_bps, "65");

  const equivalent = {
    ...baseline,
    maximum_single_wallet_quote_share_bps: "65.00",
  };
  const equivalentValidation = validateQualificationDefaultsDraft(
    equivalent,
    settings.bounds,
  );
  assert.equal(equivalentValidation.valid, true);
  assert.equal(
    qualificationDraftHasChanges(
      equivalent,
      settings.values,
      equivalentValidation,
    ),
    false,
  );

  const changed = {
    ...baseline,
    maximum_single_wallet_quote_share_bps: "62.25",
  };
  const changedValidation = validateQualificationDefaultsDraft(
    changed,
    settings.bounds,
  );
  assert.equal(changedValidation.valid, true);
  assert.equal(
    changedValidation.values.maximum_single_wallet_quote_share_bps,
    6225,
  );

  const impossibleCounts = {
    ...baseline,
    minimum_sells: "9",
  };
  const impossibleValidation = validateQualificationDefaultsDraft(
    impossibleCounts,
    settings.bounds,
  );
  assert.equal(impossibleValidation.valid, false);
  assert.match(
    impossibleValidation.errors.minimum_sells,
    /Cannot exceed the minimum trades/,
  );

  const tooPrecise = {
    ...baseline,
    maximum_single_wallet_quote_share_bps: "62.251",
  };
  const preciseValidation = validateQualificationDefaultsDraft(
    tooPrecise,
    settings.bounds,
  );
  assert.equal(preciseValidation.valid, false);
  assert.match(
    preciseValidation.errors.maximum_single_wallet_quote_share_bps,
    /two decimal places/,
  );
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
      mode: "QUALIFIED_ONLY",
      dataStale: false,
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
      mode: "QUALIFIED_ONLY",
      dataStale: false,
    }).detail,
    /still requested/,
  );
  assert.match(
    describeEmptyStream({
      backendConnected: true,
      streamStatus: "ERROR",
      requestedRunning: false,
      mode: "QUALIFIED_ONLY",
      dataStale: false,
    }).title,
    /error/i,
  );

  assert.match(
    describeEmptyStream({
      backendConnected: true,
      streamStatus: "RUNNING",
      requestedRunning: true,
      mode: "QUALIFIED_ONLY",
      dataStale: false,
    }).title,
    /qualified discoveries/i,
  );
  assert.match(
    describeEmptyStream({
      backendConnected: true,
      streamStatus: "RUNNING",
      requestedRunning: true,
      mode: "QUALIFIED_ONLY",
      dataStale: true,
    }).title,
    /stale/i,
  );
});

test("presents qualification outcome counters without legacy ambiguity", async () => {
  const { buildDiscoveryCounters } =
    await loadDashboardModule("DiscoveryCounters.tsx");
  const counters = buildDiscoveryCounters({
    mode: "QUALIFIED_ONLY",
    observed: 20,
    pending: 99,
    approved: 0,
    rejected: 98,
    qualified: 3,
    qualificationPending: 4,
    qualificationRejected: 5,
    qualificationUnknown: 6,
    processingFailures: 7,
    ratePerMinute: 8,
  });

  assert.deepEqual(
    counters.map(({ label, value }) => [label, value]),
    [
      ["Current qualified", "3"],
      ["Open windows", "4"],
      ["Activity rejects", "5"],
      ["Unknown windows", "6"],
      ["Processing failures", "7"],
      ["Events/min", "8 / min"],
    ],
  );
});

test("labels terminal observed qualification outcomes instead of pending", async () => {
  const { describeQualificationState } = await loadDashboardModule(
    "inspector/OverviewTab.tsx",
  );

  assert.equal(
    describeQualificationState({
      stage: "OBSERVED",
      qualification: {
        ...qualificationSummaryFixture(),
        decision: "REJECT",
      },
    }).label,
    "Qualification rejected",
  );
  assert.equal(
    describeQualificationState({
      stage: "OBSERVED",
      qualification: {
        ...qualificationSummaryFixture(),
        decision: "UNKNOWN",
        completeness: "INCOMPLETE",
      },
    }).label,
    "Qualification unknown",
  );
  assert.equal(
    describeQualificationState({
      stage: "OBSERVED",
      qualification: null,
    }).label,
    "Qualification pending",
  );
});

test("flushes the latest dashboard layout and tolerates storage failures", async () => {
  const {
    defaultLayout,
    flushDashboardLayoutPreference,
    layoutStorageKey,
    readDashboardLayoutPreference,
    writeDashboardLayoutPreference,
  } = await loadDashboardModule("useDashboardLayout.ts");
  const stored = new Map();
  const storage = {
    getItem(key) {
      return stored.get(key) ?? null;
    },
    removeItem(key) {
      stored.delete(key);
    },
    setItem(key, value) {
      stored.set(key, value);
    },
  };

  assert.deepEqual(readDashboardLayoutPreference(storage), defaultLayout);

  const latestLayout = { sidebar: 286, inspector: 472 };
  flushDashboardLayoutPreference(storage, latestLayout);
  assert.equal(
    stored.get(layoutStorageKey),
    JSON.stringify(latestLayout),
  );
  assert.deepEqual(
    readDashboardLayoutPreference(storage),
    latestLayout,
  );

  stored.set(
    layoutStorageKey,
    JSON.stringify({ sidebar: -1, inspector: 50_000 }),
  );
  assert.deepEqual(readDashboardLayoutPreference(storage), {
    sidebar: 180,
    inspector: 560,
  });

  const blockedStorage = {
    getItem() {
      throw new Error("storage blocked");
    },
    removeItem() {
      throw new Error("storage blocked");
    },
    setItem() {
      throw new Error("storage blocked");
    },
  };
  assert.doesNotThrow(() =>
    writeDashboardLayoutPreference(blockedStorage, latestLayout),
  );
  assert.doesNotThrow(() =>
    flushDashboardLayoutPreference(blockedStorage, latestLayout),
  );
  assert.deepEqual(
    readDashboardLayoutPreference(blockedStorage),
    defaultLayout,
  );
});

test("validates and stores only the execution-mode presentation preference", async () => {
  const {
    executionModePreferenceStorageKey,
    parseExecutionModePreference,
    readExecutionModePreference,
    writeExecutionModePreference,
  } = await loadDashboardModule("useExecutionModePreference.ts");

  assert.equal(
    executionModePreferenceStorageKey,
    "soldisco.execution-mode-presentation.v1",
  );
  assert.equal(parseExecutionModePreference(null), "Paper");
  assert.equal(parseExecutionModePreference("corrupt"), "Paper");
  assert.equal(parseExecutionModePreference('"Live"'), "Paper");
  assert.equal(parseExecutionModePreference("Paper"), "Paper");
  assert.equal(parseExecutionModePreference("Live"), "Live");
  assert.equal(
    readExecutionModePreference({
      getItem() {
        throw new Error("storage blocked");
      },
    }),
    "Paper",
  );

  const writes = [];
  assert.doesNotThrow(() =>
    writeExecutionModePreference(
      {
        setItem(key, value) {
          writes.push([key, value]);
        },
      },
      "Live",
    ),
  );
  assert.deepEqual(writes, [
    ["soldisco.execution-mode-presentation.v1", "Live"],
  ]);
  assert.doesNotThrow(() =>
    writeExecutionModePreference(
      {
        setItem() {
          throw new Error("storage blocked");
        },
      },
      "Paper",
    ),
  );
});

test("validates and persists the shared loopback API port", async () => {
  const {
    DEFAULT_LOCAL_API_PORT,
    buildLocalApiBaseUrl,
    localApiPortPreferenceStorageKey,
    parseLocalApiPort,
    readLocalApiPortPreference,
    resolveLocalApiUrl,
    writeLocalApiPortPreference,
  } = await loadApiModule("config.ts");

  assert.equal(DEFAULT_LOCAL_API_PORT, 8080);
  assert.equal(
    localApiPortPreferenceStorageKey,
    "soldisco.local-api-port.v1",
  );
  assert.equal(buildLocalApiBaseUrl(), "http://127.0.0.1:8080/api/v1");
  assert.equal(buildLocalApiBaseUrl(2), "http://127.0.0.1:2/api/v1");
  assert.equal(
    buildLocalApiBaseUrl(65_535),
    "http://127.0.0.1:65535/api/v1",
  );

  for (const candidate of [2, 8080, 65_535, "8080", " 8080 "]) {
    assert.notEqual(parseLocalApiPort(candidate), null);
  }
  for (const candidate of [
    null,
    "",
    "1e3",
    "8080.5",
    "not-a-port",
    0,
    1,
    80,
    2049,
    6000,
    6667,
    10080,
    65_536,
    8080.5,
  ]) {
    assert.equal(parseLocalApiPort(candidate), null);
  }
  assert.throws(
    () => buildLocalApiBaseUrl(80),
    /browser-safe integer from 1 to 65535/,
  );

  const values = new Map();
  const storage = {
    getItem(key) {
      return values.get(key) ?? null;
    },
    setItem(key, value) {
      values.set(key, value);
    },
  };
  assert.equal(readLocalApiPortPreference(storage), 8080);
  assert.equal(writeLocalApiPortPreference(storage, 9123), true);
  assert.equal(values.get(localApiPortPreferenceStorageKey), "9123");
  assert.equal(readLocalApiPortPreference(storage), 9123);
  values.set(localApiPortPreferenceStorageKey, "corrupt");
  assert.equal(readLocalApiPortPreference(storage), 8080);
  assert.equal(
    readLocalApiPortPreference({
      getItem() {
        throw new Error("storage blocked");
      },
    }),
    8080,
  );
  assert.equal(
    writeLocalApiPortPreference(
      {
        setItem() {
          throw new Error("storage blocked");
        },
      },
      8080,
    ),
    false,
  );

  for (const pageOrigin of [
    "http://localhost:3000",
    "http://127.0.0.1:3000",
    "http://[::1]:3000",
  ]) {
    assert.deepEqual(resolveLocalApiUrl(pageOrigin, 9123), {
      baseUrl: "http://127.0.0.1:9123/api/v1",
      reason: "LOCAL",
    });
  }
  for (const pageOrigin of [
    "https://soldisco.example.com",
    "https://localhost:3000",
    "http://192.168.1.10:3000",
    "file:///tmp/index.html",
    "http://user:password@localhost:3000",
    "http://localhost:3000/path",
    "not a URL",
  ]) {
    assert.equal(
      resolveLocalApiUrl(pageOrigin, 8080).reason,
      "NON_LOCAL_PAGE",
    );
  }
  assert.equal(
    resolveLocalApiUrl("http://localhost:3000", 0).reason,
    "INVALID_PORT",
  );
});

test("hydrates and reconnects every browser consumer without stale endpoint leakage", async () => {
  const mockedGlobalNames = [
    "window",
    "document",
    "self",
    "navigator",
    "HTMLElement",
    "Node",
    "Event",
    "MessageEvent",
    "StorageEvent",
    "MutationObserver",
    "getComputedStyle",
    "fetch",
    "EventSource",
    "IS_REACT_ACT_ENVIRONMENT",
  ];
  const originalGlobals = new Map(
    mockedGlobalNames.map(
      (name) => [name, Object.getOwnPropertyDescriptor(globalThis, name)],
    ),
  );
  const dom = new JSDOM("<!doctype html><div id=\"root\"></div>", {
    url: "http://localhost:3000",
  });
  dom.window.localStorage.setItem("soldisco.local-api-port.v1", "9123");
  const requestUrls = [];
  const deferredOldRequests = [];
  const eventSources = [];
  let root;
  let latest;

  class FakeEventSource {
    constructor(url) {
      this.url = url;
      this.onopen = null;
      this.onerror = null;
      this.closed = false;
      eventSources.push(this);
    }

    addEventListener() {}

    close() {
      this.closed = true;
    }
  }

  function responseFor(url, generation) {
    const pathname = new URL(url).pathname;
    let body;
    if (pathname === "/api/v1/health") {
      body = { status: "UP", database: "UP", stream: "STOPPED" };
    } else if (pathname === "/api/v1/stream") {
      body = { status: "STOPPED", requested_running: false };
    } else if (pathname === "/api/v1/discovery") {
      body = discoveryFixture();
      body.sequence = generation === "old" ? 999 : 1;
      body.tokens[0].symbol = generation === "old" ? "OLD" : "NEW";
    } else if (
      pathname === "/api/v1/settings/prefilter-defaults"
    ) {
      body = prefilterDefaultsFixture();
    } else if (
      pathname === "/api/v1/settings/qualification-defaults"
    ) {
      body = qualificationDefaultsFixture();
    } else {
      throw new Error(`Unexpected test request: ${url}`);
    }

    return new Response(JSON.stringify(body), {
      status: 200,
      headers: { "Content-Type": "application/json" },
    });
  }

  Object.defineProperties(globalThis, {
    window: {
      configurable: true,
      writable: true,
      value: dom.window,
    },
    document: {
      configurable: true,
      writable: true,
      value: dom.window.document,
    },
    self: {
      configurable: true,
      writable: true,
      value: dom.window,
    },
    navigator: {
      configurable: true,
      writable: true,
      value: dom.window.navigator,
    },
    HTMLElement: {
      configurable: true,
      writable: true,
      value: dom.window.HTMLElement,
    },
    Node: {
      configurable: true,
      writable: true,
      value: dom.window.Node,
    },
    Event: {
      configurable: true,
      writable: true,
      value: dom.window.Event,
    },
    MessageEvent: {
      configurable: true,
      writable: true,
      value: dom.window.MessageEvent,
    },
    StorageEvent: {
      configurable: true,
      writable: true,
      value: dom.window.StorageEvent,
    },
    MutationObserver: {
      configurable: true,
      writable: true,
      value: dom.window.MutationObserver,
    },
    getComputedStyle: {
      configurable: true,
      writable: true,
      value: dom.window.getComputedStyle.bind(dom.window),
    },
    EventSource: {
      configurable: true,
      writable: true,
      value: FakeEventSource,
    },
    IS_REACT_ACT_ENVIRONMENT: {
      configurable: true,
      writable: true,
      value: true,
    },
    fetch: {
      configurable: true,
      writable: true,
      value: (input) => {
        const url = String(input);
        requestUrls.push(url);
        if (url.includes("127.0.0.1:9123")) {
          return new Promise((resolve) => {
            deferredOldRequests.push({ url, resolve });
          });
        }
        return Promise.resolve(responseFor(url, "new"));
      },
    },
  });
  Object.defineProperty(dom.window, "EventSource", {
    configurable: true,
    writable: true,
    value: FakeEventSource,
  });

  async function flushReact() {
    for (let index = 0; index < 4; index += 1) {
      await new Promise((resolve) => setImmediate(resolve));
    }
  }

  try {
    const [
      { createRoot },
      { useDiscoveryBackend },
      { useLocalApiPortPreference },
      { usePrefilterDefaults },
      { useQualificationDefaults },
    ] = await Promise.all([
      import("react-dom/client"),
      loadDashboardModule("useDiscoveryBackend.ts"),
      loadDashboardModule("useLocalApiPortPreference.ts"),
      loadDashboardModule("controls/usePrefilterDefaults.ts"),
      loadDashboardModule("controls/useQualificationDefaults.ts"),
    ]);

    function Probe() {
      const localApi = useLocalApiPortPreference();
      const runtime = useDiscoveryBackend({
        apiPort: localApi.ready ? localApi.port : null,
        attemptRevision: localApi.attemptRevision,
      });
      const connected = runtime.backend.connection === "CONNECTED";
      const prefilter = usePrefilterDefaults(
        connected,
        localApi.baseUrl,
      );
      const qualification = useQualificationDefaults(
        connected,
        localApi.baseUrl,
      );
      latest = { localApi, runtime, prefilter, qualification };
      return null;
    }

    const container = dom.window.document.getElementById("root");
    assert.ok(container);
    root = createRoot(container);
    await act(async () => {
      root.render(createElement(Probe));
      await flushReact();
    });

    assert.equal(latest.localApi.port, 9123);
    assert.equal(
      latest.runtime.backend.apiBaseUrl,
      "http://127.0.0.1:9123/api/v1",
    );
    assert.equal(latest.runtime.backend.connection, "CONNECTING");
    assert.equal(requestUrls.length, 3);
    assert.ok(
      requestUrls.every((url) => url.includes("127.0.0.1:9123")),
    );
    assert.equal(eventSources.length, 1);

    const switchedAt = requestUrls.length;
    await act(async () => {
      assert.equal(latest.localApi.connect(9333), true);
      await flushReact();
    });

    assert.equal(
      dom.window.localStorage.getItem("soldisco.local-api-port.v1"),
      "9333",
    );
    assert.equal(eventSources[0].closed, true);
    assert.equal(eventSources.length, 2);
    assert.equal(latest.runtime.backend.connection, "CONNECTED");
    assert.equal(
      latest.runtime.backend.apiBaseUrl,
      "http://127.0.0.1:9333/api/v1",
    );
    assert.equal(latest.runtime.discovery.tokens[0].symbol, "NEW");
    assert.ok(
      requestUrls
        .slice(switchedAt)
        .every((url) => url.includes("127.0.0.1:9333")),
    );
    assert.ok(
      requestUrls.some((url) =>
        url.endsWith("/settings/prefilter-defaults"),
      ),
    );
    assert.ok(
      requestUrls.some((url) =>
        url.endsWith("/settings/qualification-defaults"),
      ),
    );

    await act(async () => {
      for (const request of deferredOldRequests) {
        request.resolve(responseFor(request.url, "old"));
      }
      await flushReact();
    });
    assert.equal(
      latest.runtime.backend.apiBaseUrl,
      "http://127.0.0.1:9333/api/v1",
    );
    assert.equal(latest.runtime.discovery.tokens[0].symbol, "NEW");

    const retriedAt = requestUrls.length;
    await act(async () => {
      assert.equal(latest.localApi.connect(9333), true);
      await flushReact();
    });
    assert.equal(eventSources[1].closed, true);
    assert.equal(eventSources.length, 3);
    assert.equal(latest.runtime.backend.connection, "CONNECTED");
    assert.ok(
      requestUrls
        .slice(retriedAt)
        .every((url) => url.includes("127.0.0.1:9333")),
    );
  } finally {
    if (root) {
      await act(async () => {
        root.unmount();
        await flushReact();
      });
    }
    dom.window.close();
    for (const [name, descriptor] of originalGlobals) {
      if (descriptor === undefined) {
        delete globalThis[name];
      } else {
        Object.defineProperty(globalThis, name, descriptor);
      }
    }
  }
});
