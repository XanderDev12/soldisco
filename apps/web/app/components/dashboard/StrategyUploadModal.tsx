type StrategyUploadModalProps = {
  onClose: () => void;
};

export function StrategyUploadModal({
  onClose,
}: StrategyUploadModalProps) {
  return (
    <div className="modal-backdrop" role="presentation">
      <div
        className="modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="upload-title"
      >
        <button
          type="button"
          className="modal__close"
          onClick={onClose}
          aria-label="Close upload dialog"
        >
          ×
        </button>
        <span className="modal__eyebrow">STRATEGY WORKSPACE</span>
        <h2 id="upload-title">Upload a strategy</h2>
        <p>
          Strategy files will be accepted after validation and sandboxing are
          connected.
        </p>
        <div className="upload-zone">
          <span>⇧</span>
          <strong>No strategy file selected</strong>
          <small>Expected format: declarative JSON manifest</small>
        </div>
        <div className="modal__notice">
          Uploaded strategies will be inactive until validation and a replay
          dry run succeed.
        </div>
        <div className="modal__actions">
          <button type="button" onClick={onClose}>Cancel</button>
          <button type="button" disabled>Choose file</button>
        </div>
      </div>
    </div>
  );
}
