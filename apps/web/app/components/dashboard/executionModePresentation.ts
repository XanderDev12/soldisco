import type { ExecutionMode } from "./types";

type ModePresentation = {
  requiresWallet: boolean;
  controlsDetail: string;
  orders: {
    eyebrow: string;
    title: string;
    description: string;
    stats: readonly string[];
    recordTitle: string;
    recordDetail: string;
    columns: readonly string[];
    emptyTitle: string;
    emptyDetail: string;
  };
  positions: {
    eyebrow: string;
    title: string;
    description: string;
    stats: readonly string[];
    recordTitle: string;
    recordDetail: string;
    status: string;
    emptyTitle: string;
    emptyDetail: string;
  };
  inspectorPosition: {
    emptyTitle: string;
    emptyDetail: (symbol: string) => string;
    action: string;
  };
  trade: {
    safetyLabel: string;
    safetyDetail: string;
    reviewLabel: (side: string) => string;
    footnote: string;
  };
};

export const executionModePresentation = {
  Paper: {
    requiresWallet: false,
    controlsDetail: "Simulated ledger and price recording",
    orders: {
      eyebrow: "PAPER ACTIVITY",
      title: "Paper Orders",
      description:
        "Review simulated buy and sell decisions without submitting transactions.",
      stats: ["OPEN PAPER", "COMPLETED", "FAILED"],
      recordTitle: "Paper order records",
      recordDetail: "Simulated entries and exits",
      columns: ["Token", "Side", "Amount", "Status", "Recorded"],
      emptyTitle: "No paper orders recorded",
      emptyDetail:
        "Simulated buys and sells will appear when the paper ledger is connected.",
    },
    positions: {
      eyebrow: "PAPER PORTFOLIO",
      title: "Paper Positions",
      description:
        "Review simulated holdings and recorded entry and exit prices.",
      stats: ["OPEN PAPER", "MARK VALUE", "REALIZED PNL"],
      recordTitle: "Paper positions",
      recordDetail: "Simulated holdings and price records",
      status: "Empty",
      emptyTitle: "No paper positions recorded",
      emptyDetail:
        "A simulated buy will create a paper position without submitting a transaction.",
    },
    inspectorPosition: {
      emptyTitle: "No paper position recorded",
      emptyDetail: (symbol) =>
        `A simulated buy for ${symbol} will create a paper position here.`,
      action: "Open paper ticket",
    },
    trade: {
      safetyLabel: "PAPER RECORDING UNAVAILABLE",
      safetyDetail: "No quote or paper ledger service is connected.",
      reviewLabel: (side) => `Record paper ${side} · unavailable`,
      footnote:
        "Paper mode will record simulated entry and exit prices without signing or submitting a transaction.",
    },
  },
  Live: {
    requiresWallet: true,
    controlsDetail: "Wallet-authorized execution",
    orders: {
      eyebrow: "LIVE ACTIVITY",
      title: "Live Orders",
      description:
        "Review submitted transaction intent and execution outcomes.",
      stats: ["OPEN LIVE", "COMPLETED", "FAILED"],
      recordTitle: "Live order records",
      recordDetail: "Execution service not connected",
      columns: ["Token", "Side", "Amount", "Status", "Submitted"],
      emptyTitle: "Live order data unavailable",
      emptyDetail: "No execution service is connected.",
    },
    positions: {
      eyebrow: "LIVE PORTFOLIO",
      title: "Live Positions",
      description:
        "Review wallet-backed holdings and reconciled position valuations.",
      stats: ["OPEN LIVE", "MARK VALUE", "REALIZED PNL"],
      recordTitle: "Held positions",
      recordDetail: "Wallet-backed portfolio records",
      status: "Unavailable",
      emptyTitle: "Live position data unavailable",
      emptyDetail: "Connect a wallet to request live holdings.",
    },
    inspectorPosition: {
      emptyTitle: "Live position data unavailable",
      emptyDetail: (symbol) =>
        `Connect a wallet to load live holdings for ${symbol}.`,
      action: "View live trade controls",
    },
    trade: {
      safetyLabel: "LIVE EXECUTION LOCKED",
      safetyDetail:
        "No route, quote, wallet, or execution service is connected.",
      reviewLabel: (side) => `Review ${side} · Live mode`,
      footnote:
        "Final quotes, simulation, and explicit wallet confirmation will be required before execution.",
    },
  },
} satisfies Record<ExecutionMode, ModePresentation>;
