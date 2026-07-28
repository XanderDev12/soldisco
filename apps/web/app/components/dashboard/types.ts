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
export type TokenStatus = "Approved" | "Pending" | "Rejected";
export type MatchLevel = "Strong" | "Moderate" | "None" | "Evaluating";
export type InspectorTab =
  | "Overview"
  | "Risk"
  | "Signals"
  | "Trade"
  | "Position";
export type LayoutKey = "sidebar" | "inspector";
export type LayoutPreferences = Record<LayoutKey, number>;

export type Token = {
  id: string;
  name: string;
  symbol: string;
  mint: string;
  age: string;
  status: "Approved";
  risk: number;
  rating: string;
  match: MatchLevel;
  price: string;
  move: string;
  moveUp: boolean;
  volume: string;
  liquidity: string;
  holders: string;
  buys: number;
  sells: number;
  momentum: number[];
  reason: string;
  checks: string[];
};

export type ScreeningSummary = {
  pending: number;
  approved: number;
  rejected: number;
  ratePerMinute: number | null;
};

export type RejectionLogEntry = {
  id: string;
  mint: string;
  symbol: string | null;
  reasonCodes: string[];
  rejectedAt: string;
};
