import { SectionHeader } from "./SectionViewPrimitives";

type PositionsViewProps = {
  onWallet: () => void;
};

export function PositionsView({ onWallet }: PositionsViewProps) {
  return (
    <>
      <SectionHeader
        eyebrow="PORTFOLIO"
        title="Positions"
        description="Review reconciled holdings and position valuations."
        stats={["OPEN POSITIONS", "MARK VALUE", "REALIZED PNL"]}
      />
      <div className="section-view__body">
        <div className="section-card">
          <div className="section-card__head">
            <div>
              <h2>Held positions</h2>
              <p>Wallet-backed portfolio records</p>
            </div>
            <span className="status-chip">Unavailable</span>
          </div>
          <div className="section-empty">
            <span aria-hidden="true">◎</span>
            <h2>Position data unavailable</h2>
            <p>Connect a wallet to request holdings for this workspace.</p>
            <div className="control-card__actions">
              <button type="button" onClick={onWallet}>
                Connect wallet
              </button>
            </div>
          </div>
        </div>
      </div>
    </>
  );
}
