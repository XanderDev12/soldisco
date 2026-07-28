import type { BackendStatusViewModel } from "../../lib/soldisco-api/viewModels";

type BackendStatusNoticeProps = {
  backend: BackendStatusViewModel;
};

export function BackendStatusNotice({
  backend,
}: BackendStatusNoticeProps) {
  if (backend.errorMessage) {
    return (
      <div className="backend-notice backend-notice--error" role="alert">
        <strong>{backend.errorCode}</strong>
        <span>{backend.errorMessage}</span>
      </div>
    );
  }

  if (backend.connection === "CONNECTING") {
    return (
      <div className="backend-notice" role="status">
        Connecting to the local Rust backend…
      </div>
    );
  }

  if (backend.connection === "LOCAL_ONLY") {
    return (
      <div className="backend-notice" role="status">
        Backend controls are available only from the local workspace.
      </div>
    );
  }

  if (backend.connection === "UNAVAILABLE") {
    return (
      <div className="backend-notice backend-notice--error" role="alert">
        The local Rust backend is unavailable.
      </div>
    );
  }

  return null;
}
