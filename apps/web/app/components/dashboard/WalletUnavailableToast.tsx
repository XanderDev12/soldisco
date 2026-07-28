export function WalletUnavailableToast() {
  return (
    <div className="toast" role="status">
      <span>▰</span>
      <div>
        <strong>Wallet connection is not enabled</strong>
        <p>This interface cannot sign or submit transactions.</p>
      </div>
    </div>
  );
}
