import type {
  KeyboardEvent,
  PointerEvent as ReactPointerEvent,
} from "react";
import type {
  ExecutionMode,
  InspectorTab,
  RejectionLogEntry,
  ScreeningSummary,
  Token,
  TradeSide,
} from "./types";
import type { BackendStatusViewModel } from "../../lib/soldisco-api/viewModels";
import { TokenInspector } from "./TokenInspector";
import { TokenStreamHeader } from "./TokenStreamHeader";
import { TokenTable } from "./TokenTable";

type TokenStreamViewProps = {
  tokens: Token[];
  tokensTotal: number;
  tokensTruncated: boolean;
  dataStale: boolean;
  screeningSummary: ScreeningSummary;
  rejectionLog: RejectionLogEntry[];
  selectedToken: Token | null;
  backend: BackendStatusViewModel;
  onSelectToken: (id: string) => void;
  onOpenControls: () => void;
  inspectorSize: number;
  inspectorMin: number;
  inspectorMax: number;
  onInspectorPointerDown: (
    event: ReactPointerEvent<HTMLButtonElement>,
  ) => void;
  onInspectorKeyDown: (event: KeyboardEvent<HTMLButtonElement>) => void;
  onResetInspectorSize: () => void;
  inspectorTab: InspectorTab;
  onInspectorTabChange: (tab: InspectorTab) => void;
  orderSide: TradeSide;
  onOrderSideChange: (side: TradeSide) => void;
  orderAmount: string;
  onOrderAmountChange: (amount: string) => void;
  mode: ExecutionMode;
  onWallet: () => void;
};

export function TokenStreamView({
  tokens,
  tokensTotal,
  tokensTruncated,
  dataStale,
  screeningSummary,
  rejectionLog,
  selectedToken,
  backend,
  onSelectToken,
  onOpenControls,
  inspectorSize,
  inspectorMin,
  inspectorMax,
  onInspectorPointerDown,
  onInspectorKeyDown,
  onResetInspectorSize,
  inspectorTab,
  onInspectorTabChange,
  orderSide,
  onOrderSideChange,
  orderAmount,
  onOrderAmountChange,
  mode,
  onWallet,
}: TokenStreamViewProps) {
  return (
    <section
      id="dashboard-view"
      className="dashboard-view dashboard-view--stream"
      aria-labelledby="view-title"
    >
      <TokenStreamHeader
        stream={backend.stream}
        discoveryMode={screeningSummary.mode}
        dataStale={dataStale}
      />

      <div className="stream-layout">
        <TokenTable
          tokens={tokens}
          tokensTotal={tokensTotal}
          tokensTruncated={tokensTruncated}
          dataStale={dataStale}
          screeningSummary={screeningSummary}
          rejectionLog={rejectionLog}
          selectedToken={selectedToken}
          streamStatus={backend.stream.status}
          streamRequestedRunning={backend.stream.requestedRunning}
          streamIndicatorClass={backend.stream.indicatorClass}
          streamStatusLabel={backend.stream.statusLabel}
          backendConnected={backend.connection === "CONNECTED"}
          onSelectToken={onSelectToken}
          onOpenControls={onOpenControls}
        />

        <button
          type="button"
          className="resize-handle resize-handle--inspector"
          role="separator"
          aria-label="Resize token inspector"
          aria-controls="stream-panel inspector-panel"
          aria-orientation="vertical"
          aria-valuemin={inspectorMin}
          aria-valuemax={inspectorMax}
          aria-valuenow={inspectorSize}
          onPointerDown={onInspectorPointerDown}
          onKeyDown={onInspectorKeyDown}
          onDoubleClick={onResetInspectorSize}
          title="Drag to resize · Double-click to reset"
        />

        <TokenInspector
          token={selectedToken}
          tab={inspectorTab}
          onTabChange={onInspectorTabChange}
          orderSide={orderSide}
          onOrderSideChange={onOrderSideChange}
          orderAmount={orderAmount}
          onOrderAmountChange={onOrderAmountChange}
          mode={mode}
          onWallet={onWallet}
        />
      </div>
    </section>
  );
}
