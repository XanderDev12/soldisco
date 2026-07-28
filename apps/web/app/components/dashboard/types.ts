import type {
  DiscoveryTokenViewModel,
  RejectionSummaryViewModel,
  ScreeningSummaryViewModel,
} from "../../lib/soldisco-api/viewModels";

export type DashboardView =
  | "discovery"
  | "positions"
  | "orders"
  | "alerts"
  | "strategies"
  | "replays"
  | "controls";

export type SectionView = Exclude<DashboardView, "discovery">;
export type ExecutionMode = "Paper" | "Live";
export type TradeSide = "Buy" | "Sell";
export type TokenStatus = "Observed" | "Qualified" | "Approved";
export type InspectorTab =
  | "Overview"
  | "Risk"
  | "Signals"
  | "Trade"
  | "Position";
export type LayoutKey = "sidebar" | "inspector";
export type LayoutPreferences = Record<LayoutKey, number>;

export type Token = DiscoveryTokenViewModel;
export type ScreeningSummary = ScreeningSummaryViewModel;
export type RejectionLogEntry = RejectionSummaryViewModel;
