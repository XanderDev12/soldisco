"use client";

import {
  useCallback,
  useReducer,
  useSyncExternalStore,
} from "react";
import {
  buildLocalApiBaseUrl,
  DEFAULT_LOCAL_API_PORT,
  localApiPortPreferenceStorageKey,
  parseLocalApiPort,
  readLocalApiPortPreference,
  writeLocalApiPortPreference,
} from "../../lib/soldisco-api/config";

const preferenceListeners = new Set<() => void>();
let cachedClientPort: number | null = null;
const subscribeToHydration = () => () => {};
const getHydratedClientSnapshot = () => true;
const getHydratedServerSnapshot = () => false;

function readBrowserPreference(): number {
  if (typeof window === "undefined") return DEFAULT_LOCAL_API_PORT;
  try {
    return readLocalApiPortPreference(window.localStorage);
  } catch {
    return DEFAULT_LOCAL_API_PORT;
  }
}

function getClientSnapshot(): number {
  cachedClientPort ??= readBrowserPreference();
  return cachedClientPort;
}

function getServerSnapshot(): number {
  return DEFAULT_LOCAL_API_PORT;
}

function notifyPreferenceListeners() {
  for (const listener of preferenceListeners) listener();
}

function handleStorage(event: StorageEvent) {
  if (
    event.key !== null &&
    event.key !== localApiPortPreferenceStorageKey
  ) {
    return;
  }

  const nextPort =
    parseLocalApiPort(event.newValue) ?? DEFAULT_LOCAL_API_PORT;
  if (cachedClientPort === nextPort) return;
  cachedClientPort = nextPort;
  notifyPreferenceListeners();
}

function subscribeToPreference(listener: () => void): () => void {
  if (typeof window === "undefined") return () => {};

  const wasEmpty = preferenceListeners.size === 0;
  preferenceListeners.add(listener);
  if (wasEmpty) window.addEventListener("storage", handleStorage);
  return () => {
    preferenceListeners.delete(listener);
    if (preferenceListeners.size === 0) {
      window.removeEventListener("storage", handleStorage);
    }
  };
}

function setLocalApiPortPreference(port: number): boolean {
  const parsedPort = parseLocalApiPort(port);
  if (parsedPort === null) {
    throw new RangeError(
      "The local API port must be a browser-safe integer from 1 to 65535.",
    );
  }

  if (getClientSnapshot() === parsedPort) return true;

  if (typeof window !== "undefined") {
    try {
      if (
        !writeLocalApiPortPreference(window.localStorage, parsedPort)
      ) {
        return false;
      }
    } catch {
      return false;
    }
  }

  cachedClientPort = parsedPort;
  notifyPreferenceListeners();
  return true;
}

export function useLocalApiPortPreference() {
  const ready = useSyncExternalStore(
    subscribeToHydration,
    getHydratedClientSnapshot,
    getHydratedServerSnapshot,
  );
  const port = useSyncExternalStore(
    subscribeToPreference,
    getClientSnapshot,
    getServerSnapshot,
  );
  const [attemptRevision, requestAttempt] = useReducer(
    (revision: number) => revision + 1,
    0,
  );

  const connect = useCallback((nextPort: number): boolean => {
    if (!setLocalApiPortPreference(nextPort)) return false;
    requestAttempt();
    return true;
  }, []);

  return {
    ready,
    port,
    baseUrl: buildLocalApiBaseUrl(port),
    attemptRevision,
    connect,
  };
}
