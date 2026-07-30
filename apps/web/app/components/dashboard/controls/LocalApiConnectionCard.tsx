"use client";

import { useState, type FormEvent } from "react";
import {
  buildLocalApiBaseUrl,
  buildLocalApiProxyBaseUrl,
  parseLocalApiPort,
} from "../../../lib/soldisco-api/config";
import type { BackendStatusViewModel } from "../../../lib/soldisco-api/viewModels";

type LocalApiConnectionCardProps = {
  activePort: number;
  backend: BackendStatusViewModel;
  onConnect: (port: number) => boolean;
};

type PortDraftState = {
  activePort: number;
  value: string;
  submitted: boolean;
  storageError: boolean;
};

const connectionLabels = {
  IDLE: "Not attempted",
  CONNECTING: "Attempting",
  CONNECTED: "Connected",
  UNAVAILABLE: "Failed",
  LOCAL_ONLY: "Local only",
} as const;

export function LocalApiConnectionCard({
  activePort,
  backend,
  onConnect,
}: LocalApiConnectionCardProps) {
  const [draft, setDraft] = useState<PortDraftState>({
    activePort,
    value: String(activePort),
    submitted: false,
    storageError: false,
  });
  if (draft.activePort !== activePort) {
    setDraft({
      activePort,
      value: String(activePort),
      submitted: false,
      storageError: false,
    });
  }

  const portDraft = draft.value;
  const parsedPort = parseLocalApiPort(portDraft);
  const portError =
    draft.submitted && parsedPort === null
      ? "Enter a browser-safe whole-number port from 1 to 65535."
      : null;
  const activeRustBaseUrl = buildLocalApiBaseUrl(activePort);
  const activeProxyBaseUrl = buildLocalApiProxyBaseUrl(activePort);
  const draftRustBaseUrl =
    parsedPort === null
      ? null
      : buildLocalApiBaseUrl(parsedPort);

  function connect(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setDraft((current) => ({
      ...current,
      submitted: true,
      storageError: false,
    }));
    if (parsedPort === null) return;
    if (!onConnect(parsedPort)) {
      setDraft((current) => ({ ...current, storageError: true }));
    }
  }

  const statusMessage =
    backend.connection === "CONNECTING"
      ? `Requests sent through ${activeProxyBaseUrl} to ${activeRustBaseUrl}.`
      : backend.connection === "CONNECTED"
        ? `Confirmed connected to ${activeRustBaseUrl}.`
        : backend.connection === "UNAVAILABLE"
          ? `The local gateway could not reach ${activeRustBaseUrl}.`
          : backend.connection === "LOCAL_ONLY"
            ? "No request was sent; open this interface from a local web origin."
            : `Ready to connect through ${activeProxyBaseUrl}.`;

  return (
    <article className="control-card control-card--wide local-api-connection">
      <div className="section-card__head">
        <div>
          <h2>Local API Connection</h2>
          <p>Same-origin gateway to the loopback-only Rust API</p>
        </div>
        <span className="status-chip">
          {connectionLabels[backend.connection]}
        </span>
      </div>

      <form
        className="local-api-connection__form"
        onSubmit={connect}
        noValidate
      >
        <div className="local-api-connection__field">
          <label htmlFor="local-api-port">Rust API port</label>
          <p>
            The browser stays on this site&apos;s origin while the local
            gateway forwards approved routes to the Rust server.
          </p>
          <div className="local-api-connection__input">
            <span>http://127.0.0.1:</span>
            <input
              id="local-api-port"
              name="local-api-port"
              type="text"
              inputMode="numeric"
              pattern="[0-9]*"
              autoComplete="off"
              value={portDraft}
              aria-invalid={portError !== null}
              aria-describedby={
                portError === null
                  ? "local-api-port-help"
                  : "local-api-port-help local-api-port-error"
              }
              onChange={(event) => {
                setDraft({
                  activePort,
                  value: event.target.value,
                  submitted: false,
                  storageError: false,
                });
              }}
            />
            <span>/api/v1</span>
          </div>
          <small id="local-api-port-help">
            Default 8080 · browser-restricted ports are rejected · must match
            the backend API_PORT
          </small>
          {portError !== null && (
            <span
              id="local-api-port-error"
              className="setting-field__error"
              role="alert"
            >
              {portError}
            </span>
          )}
        </div>

        <div className="local-api-connection__attempt">
          <span className="eyebrow">LOCAL GATEWAY</span>
          <code>{backend.apiBaseUrl}</code>
          <span className="eyebrow local-api-connection__target-label">
            RUST TARGET
          </span>
          <code>{activeRustBaseUrl}</code>
          <p
            className={
              backend.connection === "UNAVAILABLE" ||
              backend.connection === "LOCAL_ONLY"
                ? "local-api-connection__status local-api-connection__status--error"
                : "local-api-connection__status"
            }
            role={
              backend.connection === "UNAVAILABLE" ? "alert" : "status"
            }
            aria-live="polite"
          >
            {statusMessage}
          </p>
          {draftRustBaseUrl !== null &&
            draftRustBaseUrl !== activeRustBaseUrl && (
              <p>Rust target after save: {draftRustBaseUrl}</p>
            )}
          {draft.storageError && (
            <p
              className="local-api-connection__status local-api-connection__status--error"
              role="alert"
            >
              Browser storage is unavailable, so the port was not changed.
            </p>
          )}
        </div>

        <div className="local-api-connection__actions">
          <p>
            Changing this value only selects where the local gateway forwards.
            Restart Rust after changing its API_PORT, then save and connect
            here.
          </p>
          <div className="control-card__actions">
            <button
              type="submit"
              disabled={backend.connection === "CONNECTING"}
            >
              {backend.connection === "CONNECTING"
                ? "Attempting…"
                : "Save & connect"}
            </button>
          </div>
        </div>
      </form>
    </article>
  );
}
