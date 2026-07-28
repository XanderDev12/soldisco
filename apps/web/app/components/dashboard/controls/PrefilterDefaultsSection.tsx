"use client";

import { useState } from "react";
import type { StreamStatus } from "../../../lib/soldisco-api/contracts";
import type { UpdatePrefilterDefaultsRequest } from "../../../lib/soldisco-api/contracts";
import { PrefilterDefaultsForm } from "./PrefilterDefaultsForm";
import { usePrefilterDefaults } from "./usePrefilterDefaults";

type PrefilterDefaultsSectionProps = {
  backendConnected: boolean;
  streamStatus: StreamStatus | null;
};

const fixedPolicies = [
  "Successful transactions only",
  "Direct Pump Create or PumpSwap CreatePool logs only",
  "One discovery attempt per signature",
  "No transaction retry or historical backfill",
] as const;

export function PrefilterDefaultsSection({
  backendConnected,
  streamStatus,
}: PrefilterDefaultsSectionProps) {
  const { settings, status, errorMessage, refresh, save } =
    usePrefilterDefaults(backendConnected);
  const [saveNotice, setSaveNotice] = useState<string | null>(null);
  const streamStopped = streamStatus === "STOPPED";

  async function saveDefaults(request: UpdatePrefilterDefaultsRequest) {
    setSaveNotice(null);
    const response = await save(request);
    if (response !== null) {
      setSaveNotice(
        "Saved. These defaults take effect the next time the stream starts.",
      );
    }
    return response;
  }

  function refreshDefaults() {
    setSaveNotice(null);
    void refresh();
  }

  const settingsState =
    status === "READY"
      ? "READY"
      : status === "ERROR"
        ? "STALE"
        : status === "SAVING"
          ? "SAVING"
          : "REFRESHING";
  const statusLabel = !backendConnected
    ? "Unavailable"
    : status === "LOADING"
      ? settings === null
        ? "Loading"
        : "Refreshing"
      : status === "SAVING"
        ? "Saving"
        : status === "ERROR" && settings !== null
          ? `Revision ${settings.revision} · Stale`
          : settings === null
            ? "Unavailable"
            : `Revision ${settings.revision}`;

  return (
    <article className="control-card control-card--wide prefilter-defaults">
      <div className="section-card__head">
        <div>
          <h2>Prefilter-Defaults</h2>
          <p>Global discovery admission and early-observation behavior</p>
        </div>
        <span className="status-chip">{statusLabel}</span>
      </div>

      <div className="prefilter-defaults__policy">
        <div>
          <span className="eyebrow">FIXED LIVE-FIRST POLICY</span>
          <p>
            These safety and throughput rules are intentionally not
            configurable.
          </p>
        </div>
        <ul>
          {fixedPolicies.map((policy) => (
            <li key={policy}>{policy}</li>
          ))}
        </ul>
      </div>

      {settings === null || !backendConnected ? (
        <div className="settings-state" role="status" aria-live="polite">
          <strong>
            {status === "LOADING"
              ? "Loading server defaults…"
              : "Prefilter defaults unavailable"}
          </strong>
          <p>
            {errorMessage ??
              (backendConnected
                ? "The backend has not returned its settings contract."
                : "Connect the local backend to load authoritative values.")}
          </p>
          {backendConnected && status !== "LOADING" && (
            <button type="button" onClick={refreshDefaults}>
              Retry
            </button>
          )}
        </div>
      ) : (
        <PrefilterDefaultsForm
          key={settings.revision}
          settings={settings}
          backendConnected={backendConnected}
          streamStopped={streamStopped}
          settingsState={settingsState}
          saving={status === "SAVING"}
          errorMessage={errorMessage}
          saveNotice={saveNotice}
          onRefresh={refreshDefaults}
          onSave={saveDefaults}
        />
      )}
    </article>
  );
}
