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
        <span>
          Attempt through {backend.apiBaseUrl}: {backend.errorMessage}
        </span>
      </div>
    );
  }

  if (backend.connection === "CONNECTING") {
    return (
      <div className="backend-notice" role="status">
        Request sent through local gateway {backend.apiBaseUrl}…
      </div>
    );
  }

  if (backend.connection === "LOCAL_ONLY") {
    return (
      <div className="backend-notice" role="status">
        No request sent through {backend.apiBaseUrl}. Backend controls are
        available only from the local workspace.
      </div>
    );
  }

  if (backend.connection === "UNAVAILABLE") {
    return (
      <div className="backend-notice backend-notice--error" role="alert">
        The local gateway attempt through {backend.apiBaseUrl} failed.
      </div>
    );
  }

  return null;
}
