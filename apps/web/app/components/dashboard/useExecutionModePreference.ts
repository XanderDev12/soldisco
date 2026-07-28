"use client";

import { useSyncExternalStore } from "react";
import type { ExecutionMode } from "./types";

type PreferenceStorage = Pick<Storage, "getItem" | "setItem">;

export const executionModePreferenceStorageKey =
  "soldisco.execution-mode-presentation.v1";

const fallbackExecutionMode: ExecutionMode = "Paper";
const preferenceListeners = new Set<() => void>();
let cachedClientMode: ExecutionMode | null = null;

/**
 * This value controls presentation only. It must never be used as wallet,
 * signing, execution, or future live-trading authorization.
 */
export function parseExecutionModePreference(
  value: string | null,
): ExecutionMode {
  return value === "Paper" || value === "Live"
    ? value
    : fallbackExecutionMode;
}

export function readExecutionModePreference(
  storage: Pick<PreferenceStorage, "getItem">,
): ExecutionMode {
  try {
    return parseExecutionModePreference(
      storage.getItem(executionModePreferenceStorageKey),
    );
  } catch {
    return fallbackExecutionMode;
  }
}

export function writeExecutionModePreference(
  storage: Pick<PreferenceStorage, "setItem">,
  mode: ExecutionMode,
): void {
  try {
    storage.setItem(executionModePreferenceStorageKey, mode);
  } catch {
    // The in-memory presentation choice still works when storage is blocked.
  }
}

function readBrowserPreference(): ExecutionMode {
  if (typeof window === "undefined") return fallbackExecutionMode;
  try {
    return readExecutionModePreference(window.localStorage);
  } catch {
    return fallbackExecutionMode;
  }
}

function getClientSnapshot(): ExecutionMode {
  cachedClientMode ??= readBrowserPreference();
  return cachedClientMode;
}

function getServerSnapshot(): ExecutionMode {
  return fallbackExecutionMode;
}

function notifyPreferenceListeners() {
  for (const listener of preferenceListeners) listener();
}

function handleStorage(event: StorageEvent) {
  if (
    event.key !== null &&
    event.key !== executionModePreferenceStorageKey
  ) {
    return;
  }

  const nextMode = parseExecutionModePreference(event.newValue);
  if (cachedClientMode === nextMode) return;
  cachedClientMode = nextMode;
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

function setExecutionModePreference(mode: ExecutionMode) {
  const changed = getClientSnapshot() !== mode;
  cachedClientMode = mode;
  if (typeof window !== "undefined") {
    try {
      writeExecutionModePreference(window.localStorage, mode);
    } catch {
      // Accessing localStorage itself can be blocked by browser policy.
    }
  }
  if (changed) notifyPreferenceListeners();
}

export function useExecutionModePreference() {
  const mode = useSyncExternalStore(
    subscribeToPreference,
    getClientSnapshot,
    getServerSnapshot,
  );
  return [mode, setExecutionModePreference] as const;
}
