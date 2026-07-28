export type DashboardView =
  | "token-stream"
  | "initial-approval"
  | "watchlist"
  | "positions"
  | "orders"
  | "alerts"
  | "strategies"
  | "replays"
  | "controls";

export type SectionView = Exclude<DashboardView, "token-stream">;
export type ExecutionMode = "Paper" | "Live";
export type TradeSide = "Buy" | "Sell";
export type TokenStatus = "Approved" | "Pending" | "Rejected";
export type MatchLevel = "Strong" | "Moderate" | "None" | "Evaluating";
export type TokenFilter = "All" | TokenStatus;
export type InspectorTab =
  | "Overview"
  | "Risk"
  | "Signals"
  | "Trade"
  | "Position";
export type LayoutKey = "sidebar" | "inspector" | "tray";
export type LayoutPreferences = Record<LayoutKey, number>;

export type Token = {
  id: string;
  name: string;
  symbol: string;
  mint: string;
  age: string;
  status: TokenStatus;
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
