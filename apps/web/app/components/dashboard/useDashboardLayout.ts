"use client";

import {
  type CSSProperties,
  type KeyboardEvent,
  type PointerEvent as ReactPointerEvent,
  useEffect,
  useRef,
  useState,
} from "react";
import type { LayoutKey, LayoutPreferences } from "./types";

export const layoutStorageKey = "soldisco.layout.v1";

type LayoutPreferenceStorage = Pick<
  Storage,
  "getItem" | "removeItem" | "setItem"
>;

export const defaultLayout: LayoutPreferences = {
  sidebar: 224,
  inspector: 354,
};

export const layoutLimits: Record<
  LayoutKey,
  { min: number; max: number }
> = {
  sidebar: { min: 180, max: 340 },
  inspector: { min: 290, max: 560 },
};

function clampLayoutValue(key: LayoutKey, value: number) {
  const { min, max } = layoutLimits[key];
  return Math.min(max, Math.max(min, Math.round(value)));
}

function isStoredLayout(value: unknown): value is LayoutPreferences {
  if (!value || typeof value !== "object") return false;

  return (["sidebar", "inspector"] as const).every(
    (key) => typeof (value as Record<string, unknown>)[key] === "number",
  );
}

function removeInvalidLayoutPreference(storage: LayoutPreferenceStorage) {
  try {
    storage.removeItem(layoutStorageKey);
  } catch {
    // A blocked storage provider must not prevent the dashboard from loading.
  }
}

export function readDashboardLayoutPreference(
  storage: LayoutPreferenceStorage,
): LayoutPreferences {
  try {
    const savedLayout = storage.getItem(layoutStorageKey);
    if (savedLayout === null) return defaultLayout;

    const parsedLayout: unknown = JSON.parse(savedLayout);
    if (!isStoredLayout(parsedLayout)) {
      removeInvalidLayoutPreference(storage);
      return defaultLayout;
    }

    return {
      sidebar: clampLayoutValue("sidebar", parsedLayout.sidebar),
      inspector: clampLayoutValue("inspector", parsedLayout.inspector),
    };
  } catch {
    removeInvalidLayoutPreference(storage);
    return defaultLayout;
  }
}

export function writeDashboardLayoutPreference(
  storage: Pick<LayoutPreferenceStorage, "setItem">,
  layout: LayoutPreferences,
): void {
  try {
    storage.setItem(layoutStorageKey, JSON.stringify(layout));
  } catch {
    // The in-memory layout remains usable when storage is full or blocked.
  }
}

export function flushDashboardLayoutPreference(
  storage: Pick<LayoutPreferenceStorage, "setItem">,
  layout: LayoutPreferences,
): void {
  writeDashboardLayoutPreference(storage, layout);
}

function readBrowserLayoutPreference(): LayoutPreferences {
  try {
    return readDashboardLayoutPreference(window.localStorage);
  } catch {
    return defaultLayout;
  }
}

function flushBrowserLayoutPreference(layout: LayoutPreferences): void {
  try {
    flushDashboardLayoutPreference(window.localStorage, layout);
  } catch {
    // Accessing localStorage itself can be blocked by browser policy.
  }
}

export function useDashboardLayout() {
  const [layout, setLayout] = useState<LayoutPreferences>(defaultLayout);
  const [layoutLoaded, setLayoutLoaded] = useState(false);
  const [activeResize, setActiveResize] = useState<LayoutKey | null>(null);
  const latestLayoutRef = useRef<LayoutPreferences>(defaultLayout);
  const layoutLoadedRef = useRef(false);

  useEffect(() => {
    const loadLayout = window.setTimeout(() => {
      const savedLayout = readBrowserLayoutPreference();
      latestLayoutRef.current = savedLayout;
      setLayout(savedLayout);
      layoutLoadedRef.current = true;
      setLayoutLoaded(true);
    }, 0);

    return () => window.clearTimeout(loadLayout);
  }, []);

  useEffect(() => {
    if (!layoutLoaded) return;
    const saveLayout = window.setTimeout(() => {
      flushBrowserLayoutPreference(layout);
    }, 120);

    return () => window.clearTimeout(saveLayout);
  }, [layout, layoutLoaded]);

  useEffect(() => {
    function flushLatestLayout() {
      if (!layoutLoadedRef.current) return;
      flushBrowserLayoutPreference(latestLayoutRef.current);
    }

    window.addEventListener("pagehide", flushLatestLayout);
    return () => {
      window.removeEventListener("pagehide", flushLatestLayout);
      flushLatestLayout();
    };
  }, []);

  function setLayoutValue(key: LayoutKey, value: number) {
    const next = {
      ...latestLayoutRef.current,
      [key]: clampLayoutValue(key, value),
    };
    latestLayoutRef.current = next;
    setLayout(next);
  }

  function beginResize(
    key: LayoutKey,
    event: ReactPointerEvent<HTMLButtonElement>,
  ) {
    if (event.button !== 0) return;

    const startCoordinate = event.clientX;
    const startValue = layout[key];

    event.preventDefault();
    setActiveResize(key);
    document.documentElement.style.cursor = "col-resize";
    document.documentElement.style.userSelect = "none";

    const handlePointerMove = (pointerEvent: PointerEvent) => {
      const movement = pointerEvent.clientX - startCoordinate;
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
    } else {
      if (event.key === "ArrowLeft") nextValue = layout[key] + step;
      if (event.key === "ArrowRight") nextValue = layout[key] - step;
    }

    if (nextValue === null) return;
    event.preventDefault();
    setLayoutValue(key, nextValue);
  }

  const layoutStyle = {
    "--sidebar-pref": `${layout.sidebar}px`,
    "--inspector-pref": `${layout.inspector}px`,
  } as CSSProperties;

  return {
    activeResize,
    beginResize,
    handleResizeKey,
    layout,
    layoutStyle,
    setLayoutValue,
  };
}
