"use client";

import { useState } from "react";
import type { UpdateQualificationDefaultsRequest } from "../../../lib/soldisco-api/contracts";
import { QualificationDefaultsForm } from "./QualificationDefaultsForm";
import { useQualificationDefaults } from "./useQualificationDefaults";

type QualificationDefaultsSectionProps = {
  apiBaseUrl: string;
  backendConnected: boolean;
};

export function QualificationDefaultsSection({
  apiBaseUrl,
  backendConnected,
}: QualificationDefaultsSectionProps) {
  const { settings, status, errorMessage, refresh, save } =
    useQualificationDefaults(backendConnected, apiBaseUrl);
  const [saveNotice, setSaveNotice] = useState<string | null>(null);

  async function saveDefaults(
    request: UpdateQualificationDefaultsRequest,
  ) {
    setSaveNotice(null);
    const response = await save(request);
    if (response !== null) {
      setSaveNotice(
        "Saved permanently in the local database for new windows.",
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
          <h2>Qualification Defaults</h2>
          <p>
            Global evidence thresholds for completed observation windows
          </p>
        </div>
        <span className="status-chip">{statusLabel}</span>
      </div>

      <div className="prefilter-defaults__policy">
        <div>
          <span className="eyebrow">INITIAL QUALIFICATION</span>
          <p>
            These rules reduce unusable early flow. A pass is not a safety
            guarantee or trade recommendation.
          </p>
        </div>
        <ul>
          <li>Only complete observation windows can pass</li>
          <li>Each window keeps its starting ruleset revision</li>
          <li>Unknown or incomplete evidence cannot qualify</li>
          <li>Saved revisions persist across local restarts</li>
        </ul>
      </div>

      {settings === null || !backendConnected ? (
        <div className="settings-state" role="status" aria-live="polite">
          <strong>
            {status === "LOADING"
              ? "Loading qualification defaults…"
              : "Qualification defaults unavailable"}
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
        <QualificationDefaultsForm
          key={settings.revision}
          settings={settings}
          backendConnected={backendConnected}
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
