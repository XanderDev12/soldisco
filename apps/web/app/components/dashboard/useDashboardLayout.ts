"use client";

import {
  type CSSProperties,
  type KeyboardEvent,
  type PointerEvent as ReactPointerEvent,
  useEffect,
  useState,
} from "react";
import type { LayoutKey, LayoutPreferences } from "./types";

const layoutStorageKey = "soldisco.layout.v1";

export const defaultLayout: LayoutPreferences = {
  sidebar: 224,
  inspector: 354,
  tray: 64,
};

export const layoutLimits: Record<
  LayoutKey,
  { min: number; max: number }
> = {
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

export function useDashboardLayout() {
  const [layout, setLayout] = useState<LayoutPreferences>(defaultLayout);
  const [layoutLoaded, setLayoutLoaded] = useState(false);
  const [activeResize, setActiveResize] = useState<LayoutKey | null>(null);
  const [expandedTraySize, setExpandedTraySize] = useState(160);

  useEffect(() => {
    const loadLayout = window.setTimeout(() => {
      try {
        const savedLayout = window.localStorage.getItem(layoutStorageKey);
        if (savedLayout) {
          const parsedLayout: unknown = JSON.parse(savedLayout);
          if (isStoredLayout(parsedLayout)) {
            setLayout({
              sidebar: clampLayoutValue("sidebar", parsedLayout.sidebar),
              inspector: clampLayoutValue(
                "inspector",
                parsedLayout.inspector,
              ),
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

  const trayExpanded = layout.tray > defaultLayout.tray + 8;

  function toggleTray() {
    if (trayExpanded) {
      setExpandedTraySize(layout.tray);
      setLayoutValue("tray", defaultLayout.tray);
      return;
    }

    setLayoutValue("tray", expandedTraySize);
  }

  const layoutStyle = {
    "--sidebar-pref": `${layout.sidebar}px`,
    "--inspector-pref": `${layout.inspector}px`,
    "--tray-pref": `${layout.tray}px`,
  } as CSSProperties;

  return {
    activeResize,
    beginResize,
    handleResizeKey,
    layout,
    layoutStyle,
    resetLayout,
    setLayoutValue,
    toggleTray,
    trayExpanded,
  };
}
