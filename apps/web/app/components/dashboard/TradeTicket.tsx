import type { ExecutionMode, Token, TradeSide } from "./types";
import { RiskBadge } from "./TokenBadges";

type TradeTicketProps = {
  token: Token;
  side: TradeSide;
  setSide: (side: TradeSide) => void;
  amount: string;
  setAmount: (amount: string) => void;
  mode: ExecutionMode;
  onWallet: () => void;
};

export function TradeTicket({
  token,
  side,
  setSide,
  amount,
  setAmount,
  mode,
  onWallet,
}: TradeTicketProps) {
  const presets =
    side === "Buy"
      ? ["0.05", "0.10", "0.25", "0.50"]
      : ["25%", "50%", "75%", "100%"];

  return (
    <div className="trade-ticket">
      <div className="trade-safety">
        <span>EXECUTION LOCKED</span>
        <p>No route, quote, wallet, or execution service is connected.</p>
      </div>

      <div className="trade-side">
        <button
          type="button"
          className={side === "Buy" ? "is-active" : ""}
          onClick={() => setSide("Buy")}
        >
          Buy
        </button>
        <button
          type="button"
          className={side === "Sell" ? "is-active sell" : ""}
          onClick={() => setSide("Sell")}
        >
          Sell
        </button>
      </div>

      <div className="ticket-field">
        <div className="ticket-field__label">
          <span>{side === "Buy" ? "You pay" : "You sell"}</span>
          <span>Balance: —</span>
        </div>
        <div className="ticket-input">
          <input
            value={amount}
            onChange={(event) => setAmount(event.target.value)}
            inputMode="decimal"
            aria-label={
              side === "Buy" ? "SOL amount" : `${token.symbol} amount`
            }
          />
          <strong>{side === "Buy" ? "SOL" : token.symbol}</strong>
        </div>
        <div className="ticket-presets">
          {presets.map((preset) => (
            <button
              type="button"
              key={preset}
              onClick={() => setAmount(preset.replace("%", ""))}
            >
              {preset}
            </button>
          ))}
        </div>
      </div>

      <div className="quote-status">
        <div>
          <span>Expected output</span>
          <strong>Unavailable</strong>
        </div>
        <div>
          <span>Price impact</span>
          <strong>—</strong>
        </div>
        <div>
          <span>Minimum received</span>
          <strong>—</strong>
        </div>
        <div>
          <span>Route &amp; fees</span>
          <strong>Not available</strong>
        </div>
      </div>

      <div className="ticket-risk-line">
        <span>Current token risk</span>
        <RiskBadge value={token.risk} />
      </div>

      <button type="button" className="connect-ticket" onClick={onWallet}>
        Connect wallet to continue
      </button>
      <button type="button" className="review-disabled" disabled>
        Review {side.toLowerCase()} · {mode} mode
      </button>
      <p className="ticket-footnote">
        Final quotes, simulation, and explicit wallet confirmation will be
        required before execution.
      </p>
    </div>
  );
}
