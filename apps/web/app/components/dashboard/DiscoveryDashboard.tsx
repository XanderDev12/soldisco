"use client";

import { useMemo, useState } from "react";
import { DashboardTopbar } from "./DashboardTopbar";
import { PositionsTray } from "./PositionsTray";
import { Sidebar } from "./Sidebar";
import { StrategyUploadModal } from "./StrategyUploadModal";
import { TokenStreamView } from "./TokenStreamView";
import type {
  DashboardView,
  ExecutionMode,
  InspectorTab,
  Token,
  TokenFilter,
  TradeSide,
} from "./types";
import {
  defaultLayout,
  layoutLimits,
  useDashboardLayout,
} from "./useDashboardLayout";
import { WalletUnavailableToast } from "./WalletUnavailableToast";
import { DashboardSectionView } from "./views/DashboardSectionView";

const tokens: Token[] = [];

export function DiscoveryDashboard() {
  const [activeView, setActiveView] =
    useState<DashboardView>("token-stream");
  const [streamRunning, setStreamRunning] = useState(false);
  const [selectedId, setSelectedId] = useState<string | null>(
    tokens[0]?.id ?? null,
  );
  const [filter, setFilter] = useState<TokenFilter>("All");
  const [mode, setMode] = useState<ExecutionMode>("Paper");
  const [inspectorTab, setInspectorTab] =
    useState<InspectorTab>("Overview");
  const [sideNavOpen, setSideNavOpen] = useState(false);
  const [uploadOpen, setUploadOpen] = useState(false);
  const [walletMessage, setWalletMessage] = useState(false);
  const [orderSide, setOrderSide] = useState<TradeSide>("Buy");
  const [orderAmount, setOrderAmount] = useState("0.10");
  const {
    activeResize,
    beginResize,
    handleResizeKey,
    layout,
    layoutStyle,
    resetLayout,
    setLayoutValue,
    toggleTray,
    trayExpanded,
  } = useDashboardLayout();

  const selectedToken =
    tokens.find((token) => token.id === selectedId) ?? null;
  const visibleTokens = useMemo(
    () =>
      filter === "All"
        ? tokens
        : tokens.filter((token) => token.status === filter),
    [filter],
  );

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
          onResetLayout={resetLayout}
          onUpload={() => setUploadOpen(true)}
          onModeChange={setMode}
          onWallet={showWalletUnavailable}
        />

        {activeView === "token-stream" ? (
          <TokenStreamView
            tokens={tokens}
            visibleTokens={visibleTokens}
            selectedToken={selectedToken}
            streamRunning={streamRunning}
            filter={filter}
            onFilterChange={setFilter}
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
            orderSide={orderSide}
            onOrderSideChange={setOrderSide}
            orderAmount={orderAmount}
            onOrderAmountChange={setOrderAmount}
            mode={mode}
            onWallet={showWalletUnavailable}
          />
        ) : (
          <DashboardSectionView
            view={activeView}
            streamRunning={streamRunning}
            mode={mode}
            onToggleStream={toggleStream}
            onModeChange={setMode}
            onWallet={showWalletUnavailable}
            onUpload={() => setUploadOpen(true)}
            onResetLayout={resetLayout}
          />
        )}

        <PositionsTray
          mode={mode}
          trayExpanded={trayExpanded}
          layout={layout}
          layoutLimits={layoutLimits}
          defaultLayout={defaultLayout}
          onToggleTray={toggleTray}
          onBeginResize={beginResize}
          onResizeKey={handleResizeKey}
          onSetLayoutValue={setLayoutValue}
        />
      </section>

      {walletMessage && <WalletUnavailableToast />}
      {uploadOpen && (
        <StrategyUploadModal onClose={() => setUploadOpen(false)} />
      )}
    </main>
  );
}
