"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { SoldiscoApiClient } from "../../lib/soldisco-api/client";
import { resolveBrowserApiUrl } from "../../lib/soldisco-api/config";
import type {
  DiscoverySnapshot,
  HealthResponse,
  LiveEnvelope,
  StreamStateResponse,
} from "../../lib/soldisco-api/contracts";
import {
  normalizeApiError,
  SoldiscoApiError,
} from "../../lib/soldisco-api/errors";
import {
  createDiscoveryRefreshCoordinator,
  type DiscoveryRefreshCoordinator,
} from "../../lib/soldisco-api/discoveryRefresh";
import { createResponseFreshnessCoordinator } from "../../lib/soldisco-api/responseFreshness";
import {
  subscribeToSoldiscoEvents,
  type LiveConnectionStatus,
} from "../../lib/soldisco-api/events";
import {
  mapBackendStatus,
  mapDiscoverySnapshot,
} from "../../lib/soldisco-api/mappers";
import type {
  BackendConnectionStatus,
  StreamCommand,
} from "../../lib/soldisco-api/viewModels";

type SettledBackendRequests = [
  PromiseSettledResult<HealthResponse>,
  PromiseSettledResult<StreamStateResponse>,
  PromiseSettledResult<DiscoverySnapshot>,
];

type ErrorSurface =
  | "configuration"
  | "health"
  | "stream"
  | "discovery"
  | "events"
  | "command";

type SurfaceErrors = Record<ErrorSurface, SoldiscoApiError | null>;
type ResponseSurface = "connection" | "health" | "stream";

const initialErrors: SurfaceErrors = {
  configuration: null,
  health: null,
  stream: null,
  discovery: null,
  events: null,
  command: null,
};

const errorPriority: ErrorSurface[] = [
  "configuration",
  "command",
  "stream",
  "discovery",
  "health",
  "events",
];

export function useDiscoveryBackend() {
  const clientRef = useRef<SoldiscoApiClient | null>(null);
  const discoveryRefreshRef =
    useRef<DiscoveryRefreshCoordinator | null>(null);
  const responseFreshnessRef = useRef(
    createResponseFreshnessCoordinator<ResponseSurface>(),
  );
  const refreshAllRef = useRef<Promise<void> | null>(null);
  const refreshAllQueuedRef = useRef(false);
  const mountedRef = useRef(false);
  const [connection, setConnection] =
    useState<BackendConnectionStatus>("IDLE");
  const [health, setHealth] = useState<HealthResponse | null>(null);
  const [stream, setStream] = useState<StreamStateResponse | null>(null);
  const [snapshot, setSnapshot] =
    useState<DiscoverySnapshot | null>(null);
  const [liveUpdates, setLiveUpdates] =
    useState<LiveConnectionStatus>("IDLE");
  const [command, setCommand] = useState<StreamCommand | null>(null);
  const [errors, setErrors] = useState<SurfaceErrors>(initialErrors);

  const setSurfaceError = useCallback(
    (surface: ErrorSurface, error: SoldiscoApiError | null) => {
      if (!mountedRef.current) return;
      setErrors((current) => {
        if (current[surface] === error) return current;
        return { ...current, [surface]: error };
      });
    },
    [],
  );

  const acceptError = useCallback((
    surface: ErrorSurface,
    reason: unknown,
  ) => {
    setSurfaceError(surface, normalizeApiError(reason));
  }, [setSurfaceError]);

  const refreshDiscovery = useCallback(async () => {
    const coordinator = discoveryRefreshRef.current;
    if (!coordinator) return;

    try {
      await coordinator.refresh();
      if (!mountedRef.current) return;
      setSurfaceError("discovery", null);
    } catch (reason) {
      acceptError("discovery", reason);
    }
  }, [acceptError, setSurfaceError]);

  const refreshAll = useCallback(() => {
    if (refreshAllRef.current !== null) {
      refreshAllQueuedRef.current = true;
      return refreshAllRef.current;
    }

    const client = clientRef.current;
    const coordinator = discoveryRefreshRef.current;
    if (!client || !coordinator) return Promise.resolve();

    const refresh: Promise<void> = (async () => {
      do {
        refreshAllQueuedRef.current = false;
        const healthTicket =
          responseFreshnessRef.current.begin("health");
        const streamTicket =
          responseFreshnessRef.current.begin("stream");
        const connectionTicket =
          responseFreshnessRef.current.begin("connection");
        const results = (await Promise.allSettled([
          client.health,
          client.stream,
          coordinator.refresh(),
        ])) as SettledBackendRequests;
        if (!mountedRef.current || clientRef.current !== client) return;

        const [healthResult, streamResult, discoveryResult] = results;
        if (responseFreshnessRef.current.isCurrent(healthTicket)) {
          if (healthResult.status === "fulfilled") {
            setHealth(healthResult.value);
            setSurfaceError("health", null);
          } else {
            acceptError("health", healthResult.reason);
          }
        }
        if (responseFreshnessRef.current.isCurrent(streamTicket)) {
          if (streamResult.status === "fulfilled") {
            setStream(streamResult.value);
            setSurfaceError("stream", null);
          } else {
            acceptError("stream", streamResult.reason);
          }
        }
        if (discoveryResult.status === "fulfilled") {
          setSurfaceError("discovery", null);
        } else {
          acceptError("discovery", discoveryResult.reason);
        }

        if (responseFreshnessRef.current.isCurrent(connectionTicket)) {
          const anySuccess = results.some(
            (result) => result.status === "fulfilled",
          );
          setConnection(anySuccess ? "CONNECTED" : "UNAVAILABLE");
        }
      } while (
        refreshAllQueuedRef.current &&
        mountedRef.current &&
        clientRef.current === client
      );
    })().finally(() => {
      if (refreshAllRef.current === refresh) {
        refreshAllRef.current = null;
      }
    });

    refreshAllRef.current = refresh;
    return refresh;
  }, [acceptError, setSurfaceError]);

  const refreshStream = useCallback(async () => {
    const client = clientRef.current;
    if (!client) return;
    const streamTicket = responseFreshnessRef.current.begin("stream");
    const healthTicket = responseFreshnessRef.current.begin("health");
    const connectionTicket =
      responseFreshnessRef.current.begin("connection");

    const [streamResult, healthResult] = await Promise.allSettled([
      client.stream,
      client.health,
    ]);
    if (!mountedRef.current || clientRef.current !== client) return;

    if (responseFreshnessRef.current.isCurrent(streamTicket)) {
      if (streamResult.status === "fulfilled") {
        setStream(streamResult.value);
        setSurfaceError("stream", null);
      } else {
        acceptError("stream", streamResult.reason);
      }
    }
    if (responseFreshnessRef.current.isCurrent(healthTicket)) {
      if (healthResult.status === "fulfilled") {
        setHealth(healthResult.value);
        setSurfaceError("health", null);
      } else {
        acceptError("health", healthResult.reason);
      }
    }

    if (responseFreshnessRef.current.isCurrent(connectionTicket)) {
      const anySuccess =
        streamResult.status === "fulfilled" ||
        healthResult.status === "fulfilled";
      setConnection(anySuccess ? "CONNECTED" : "UNAVAILABLE");
    }
  }, [acceptError, setSurfaceError]);

  const handleLiveEnvelope = useCallback(
    (envelope: LiveEnvelope) => {
      setSurfaceError("events", null);
      switch (envelope.event.type) {
        case "STREAM_STATUS_CHANGED":
          void refreshStream();
          break;
        case "DISCOVERY_PROJECTION_CHANGED":
          void refreshDiscovery();
          break;
        case "RESYNC_REQUIRED":
          void refreshAll();
          break;
      }
    },
    [refreshAll, refreshDiscovery, refreshStream, setSurfaceError],
  );

  useEffect(() => {
    mountedRef.current = true;
    const responseFreshness = responseFreshnessRef.current;
    const resolution = resolveBrowserApiUrl();

    if (resolution.baseUrl === null) {
      queueMicrotask(() => {
        if (!mountedRef.current) return;
        setConnection(
          resolution.reason === "NON_LOCAL_PAGE"
            ? "LOCAL_ONLY"
            : "UNAVAILABLE",
        );
        if (
          resolution.reason === "INVALID_URL" ||
          resolution.reason === "NON_LOCAL_API"
        ) {
          setSurfaceError(
            "configuration",
            new SoldiscoApiError(
              "INVALID_LOCAL_API_URL",
              "The configured API URL must be a valid localhost URL.",
            ),
          );
        } else if (
          resolution.reason === "INVALID_WEB_ORIGIN" ||
          resolution.reason === "LOCAL_ORIGIN_MISMATCH"
        ) {
          setSurfaceError(
            "configuration",
            new SoldiscoApiError(
              "MISALIGNED_LOCAL_WEB_ORIGIN",
              "Open the UI from its configured local web origin and keep it aligned with the backend WEB_ORIGIN.",
            ),
          );
        }
      });
      return () => {
        mountedRef.current = false;
      };
    }

    const client = new SoldiscoApiClient(resolution.baseUrl);
    const discoveryCoordinator = createDiscoveryRefreshCoordinator(
      () => client.discovery,
      (nextSnapshot) => {
        if (!mountedRef.current || clientRef.current !== client) return;
        setSnapshot((current) =>
          current === null || nextSnapshot.sequence >= current.sequence
            ? nextSnapshot
            : current,
        );
      },
    );
    clientRef.current = client;
    discoveryRefreshRef.current = discoveryCoordinator;
    queueMicrotask(() => {
      if (mountedRef.current) setConnection("CONNECTING");
    });
    void refreshAll();

    const unsubscribe = subscribeToSoldiscoEvents(resolution.baseUrl, {
      onEnvelope: handleLiveEnvelope,
      onStatus: (status) => {
        if (!mountedRef.current) return;
        setLiveUpdates(status);
        if (status === "OPEN") {
          setSurfaceError("events", null);
          void refreshAll();
        } else if (status === "RETRYING") {
          void refreshAll();
        }
      },
      onError: (reason) => acceptError("events", reason),
    });

    return () => {
      mountedRef.current = false;
      unsubscribe();
      discoveryCoordinator.dispose();
      refreshAllRef.current = null;
      refreshAllQueuedRef.current = false;
      responseFreshness.invalidate();
      if (discoveryRefreshRef.current === discoveryCoordinator) {
        discoveryRefreshRef.current = null;
      }
      if (clientRef.current === client) clientRef.current = null;
    };
  }, [
    acceptError,
    handleLiveEnvelope,
    refreshAll,
    setSurfaceError,
  ]);

  const toggleStream = useCallback(async () => {
    const client = clientRef.current;
    if (!client || !stream || command !== null) return;

    const nextCommand: StreamCommand =
      stream.requested_running ||
      stream.status === "STARTING" ||
      stream.status === "RUNNING" ||
      stream.status === "DEGRADED"
        ? "STOP"
        : "START";
    setCommand(nextCommand);
    setSurfaceError("command", null);

    let commandError: SoldiscoApiError | null = null;
    try {
      if (nextCommand === "STOP") {
        await client.stopStream();
      } else {
        await client.startStream();
      }
    } catch (reason) {
      commandError = normalizeApiError(reason);
    } finally {
      await refreshStream();
      if (mountedRef.current) {
        setSurfaceError("command", commandError);
        setCommand(null);
      }
    }
  }, [command, refreshStream, setSurfaceError, stream]);

  const error =
    errorPriority
      .map((surface) => errors[surface])
      .find((candidate) => candidate !== null) ?? null;

  const backend = useMemo(
    () =>
      mapBackendStatus({
        connection,
        health,
        stream,
        liveUpdates,
        command,
        error,
        healthFresh: errors.health === null,
        streamFresh: errors.stream === null,
      }),
    [
      command,
      connection,
      error,
      errors.health,
      errors.stream,
      health,
      liveUpdates,
      stream,
    ],
  );

  const discovery = useMemo(
    () => (snapshot === null ? null : mapDiscoverySnapshot(snapshot)),
    [snapshot],
  );

  return {
    backend,
    discovery,
    discoveryStale:
      snapshot !== null &&
      (connection !== "CONNECTED" || errors.discovery !== null),
    refresh: refreshAll,
    toggleStream,
  };
}
