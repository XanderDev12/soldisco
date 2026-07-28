"use client";

import { useState } from "react";
import { DashboardTopbar } from "./DashboardTopbar";
import { Sidebar } from "./Sidebar";
import { StrategyUploadModal } from "./StrategyUploadModal";
import { TokenStreamView } from "./TokenStreamView";
import type {
  DashboardView,
  ExecutionMode,
  InspectorTab,
  RejectionLogEntry,
  ScreeningSummary,
  Token,
  TradeSide,
} from "./types";
import {
  defaultLayout,
  layoutLimits,
  useDashboardLayout,
} from "./useDashboardLayout";
import { WalletUnavailableToast } from "./WalletUnavailableToast";
import { DashboardSectionView } from "./views/DashboardSectionView";

const approvedTokens: Token[] = [];
const rejectionLog: RejectionLogEntry[] = [];
const screeningSummary: ScreeningSummary = {
  pending: 0,
  approved: approvedTokens.length,
  rejected: rejectionLog.length,
  ratePerMinute: null,
};

type TradeDraft = {
  side: TradeSide;
  amount: string;
};

const initialTradeDrafts: Record<ExecutionMode, TradeDraft> = {
  Paper: { side: "Buy", amount: "0.10" },
  Live: { side: "Buy", amount: "0.10" },
};

export function DiscoveryDashboard() {
  const [activeView, setActiveView] =
    useState<DashboardView>("discovery");
  const [streamRunning, setStreamRunning] = useState(false);
  const [selectedId, setSelectedId] = useState<string | null>(
    approvedTokens[0]?.id ?? null,
  );
  const [mode, setMode] = useState<ExecutionMode>("Paper");
  const [inspectorTab, setInspectorTab] =
    useState<InspectorTab>("Overview");
  const [sideNavOpen, setSideNavOpen] = useState(false);
  const [uploadOpen, setUploadOpen] = useState(false);
  const [walletMessage, setWalletMessage] = useState(false);
  const [tradeDrafts, setTradeDrafts] =
    useState(initialTradeDrafts);
  const {
    activeResize,
    beginResize,
    handleResizeKey,
    layout,
    layoutStyle,
    setLayoutValue,
  } = useDashboardLayout();

  const selectedToken =
    approvedTokens.find((token) => token.id === selectedId) ?? null;
  const tradeDraft = tradeDrafts[mode];

  function openView(view: DashboardView) {
    setActiveView(view);
    setSideNavOpen(false);
    window.requestAnimationFrame(() => {
      document.getElementById("view-title")?.focus();
    });
  }

  function selectToken(id: string) {
    setSelectedId(id);
    setInspectorTab("Overview");
  }

  function showWalletUnavailable() {
    setWalletMessage(true);
    window.setTimeout(() => setWalletMessage(false), 2600);
  }

  function toggleStream() {
    setStreamRunning((running) => !running);
  }

  function changeMode(nextMode: ExecutionMode) {
    setMode(nextMode);
    if (nextMode === "Paper") setWalletMessage(false);
  }

  function changeOrderSide(side: TradeSide) {
    setTradeDrafts((current) => ({
      ...current,
      [mode]: { ...current[mode], side },
    }));
  }

  function changeOrderAmount(amount: string) {
    setTradeDrafts((current) => ({
      ...current,
      [mode]: { ...current[mode], amount },
    }));
  }

  return (
    <main
      className="dashboard-shell"
      data-resizing={activeResize ?? undefined}
      style={layoutStyle}
    >
      <Sidebar
        activeView={activeView}
        streamRunning={streamRunning}
        sideNavOpen={sideNavOpen}
        layout={layout}
        layoutLimits={layoutLimits}
        defaultLayout={defaultLayout}
        onOpenView={openView}
        onCloseMobileNav={() => setSideNavOpen(false)}
        onBeginResize={beginResize}
        onResizeKey={handleResizeKey}
        onSetLayoutValue={setLayoutValue}
      />

      <section className="workspace">
        <DashboardTopbar
          sideNavOpen={sideNavOpen}
          streamRunning={streamRunning}
          mode={mode}
          onOpenMobileNav={() => setSideNavOpen(true)}
          onToggleStream={toggleStream}
          onUpload={() => setUploadOpen(true)}
          onModeChange={changeMode}
          onWallet={showWalletUnavailable}
        />

        {activeView === "discovery" ? (
          <TokenStreamView
            approvedTokens={approvedTokens}
            screeningSummary={screeningSummary}
            rejectionLog={rejectionLog}
            selectedToken={selectedToken}
            streamRunning={streamRunning}
            onSelectToken={selectToken}
            onOpenControls={() => openView("controls")}
            inspectorSize={layout.inspector}
            inspectorMin={layoutLimits.inspector.min}
            inspectorMax={layoutLimits.inspector.max}
            onInspectorPointerDown={(event) =>
              beginResize("inspector", event)
            }
            onInspectorKeyDown={(event) =>
              handleResizeKey("inspector", event)
            }
            onResetInspectorSize={() =>
              setLayoutValue("inspector", defaultLayout.inspector)
            }
            inspectorTab={inspectorTab}
            onInspectorTabChange={setInspectorTab}
            orderSide={tradeDraft.side}
            onOrderSideChange={changeOrderSide}
            orderAmount={tradeDraft.amount}
            onOrderAmountChange={changeOrderAmount}
            mode={mode}
            onWallet={showWalletUnavailable}
          />
        ) : (
          <DashboardSectionView
            view={activeView}
            streamRunning={streamRunning}
            mode={mode}
            onToggleStream={toggleStream}
            onModeChange={changeMode}
            onWallet={showWalletUnavailable}
            onUpload={() => setUploadOpen(true)}
          />
        )}
      </section>

      {mode === "Live" && walletMessage && <WalletUnavailableToast />}
      {uploadOpen && (
        <StrategyUploadModal onClose={() => setUploadOpen(false)} />
      )}
    </main>
  );
}
