import type { DashboardView } from "./types";

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
