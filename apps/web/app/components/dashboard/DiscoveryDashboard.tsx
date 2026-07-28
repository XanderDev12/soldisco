"use client";

import { useRef, useState } from "react";
import { BackendStatusNotice } from "./BackendStatusNotice";
import { DashboardTopbar } from "./DashboardTopbar";
import { Sidebar } from "./Sidebar";
import { StrategyUploadModal } from "./StrategyUploadModal";
import { TokenStreamView } from "./TokenStreamView";
import type {
  DashboardView,
  ExecutionMode,
  InspectorTab,
  ScreeningSummary,
  TradeSide,
} from "./types";
import { useDiscoveryBackend } from "./useDiscoveryBackend";
import {
  defaultLayout,
  layoutLimits,
  useDashboardLayout,
} from "./useDashboardLayout";
import { useCompactNavigation } from "./useCompactNavigation";
import { useExecutionModePreference } from "./useExecutionModePreference";
import { WalletUnavailableToast } from "./WalletUnavailableToast";
import { DashboardSectionView } from "./views/DashboardSectionView";

const unloadedSummary: ScreeningSummary = {
  mode: "OBSERVE_ALL",
  observed: null,
  pending: null,
  approved: null,
  rejected: null,
  qualified: null,
  qualificationPending: null,
  qualificationRejected: null,
  qualificationUnknown: null,
  processingFailures: null,
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
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [mode, setModePreference] = useExecutionModePreference();
  const [inspectorTab, setInspectorTab] =
    useState<InspectorTab>("Overview");
  const [sideNavOpen, setSideNavOpen] = useState(false);
  const [uploadOpen, setUploadOpen] = useState(false);
  const [walletMessage, setWalletMessage] = useState(false);
  const [tradeDrafts, setTradeDrafts] =
    useState(initialTradeDrafts);
  const mobileMenuButtonRef = useRef<HTMLButtonElement>(null);
  const compactNavigation = useCompactNavigation();
  const {
    backend,
    discovery,
    discoveryStale,
    toggleStream,
  } = useDiscoveryBackend();
  const {
    activeResize,
    beginResize,
    handleResizeKey,
    layout,
    layoutStyle,
    setLayoutValue,
  } = useDashboardLayout();

  const tokens = discovery?.tokens ?? [];
  const selectedToken =
    tokens.find((token) => token.id === selectedId) ??
    tokens[0] ??
    null;
  const tradeDraft = tradeDrafts[mode];

  function openView(view: DashboardView) {
    setActiveView(view);
    setSideNavOpen(false);
    window.requestAnimationFrame(() => {
      document.getElementById("view-title")?.focus();
    });
  }

  function openMobileNavigation() {
    setSideNavOpen(true);
    window.requestAnimationFrame(() => {
      document.getElementById(`nav-${activeView}`)?.focus();
    });
  }

  function closeMobileNavigation() {
    setSideNavOpen(false);
    window.requestAnimationFrame(() => {
      mobileMenuButtonRef.current?.focus();
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

  function changeMode(nextMode: ExecutionMode) {
    setModePreference(nextMode);
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
        backend={backend}
        compact={compactNavigation}
        sideNavOpen={sideNavOpen}
        layout={layout}
        layoutLimits={layoutLimits}
        defaultLayout={defaultLayout}
        onOpenView={openView}
        onCloseMobileNav={closeMobileNavigation}
        onBeginResize={beginResize}
        onResizeKey={handleResizeKey}
        onSetLayoutValue={setLayoutValue}
      />

      <section className="workspace">
        <DashboardTopbar
          sideNavOpen={sideNavOpen}
          mobileMenuButtonRef={mobileMenuButtonRef}
          stream={backend.stream}
          mode={mode}
          onOpenMobileNav={openMobileNavigation}
          onToggleStream={() => void toggleStream()}
          onUpload={() => setUploadOpen(true)}
          onModeChange={changeMode}
          onWallet={showWalletUnavailable}
        />
        <BackendStatusNotice backend={backend} />

        {activeView === "discovery" ? (
          <TokenStreamView
            tokens={tokens}
            tokensTotal={discovery?.tokensTotal ?? 0}
            tokensTruncated={discovery?.tokensTruncated ?? false}
            dataStale={discoveryStale}
            screeningSummary={discovery?.summary ?? unloadedSummary}
            rejectionLog={discovery?.rejectionReasons ?? []}
            selectedToken={selectedToken}
            backend={backend}
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
            backend={backend}
            mode={mode}
            onToggleStream={() => void toggleStream()}
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
