"use client";

import { useMemo, useState, type FormEvent } from "react";
import type {
  PrefilterDefaultsResponse,
  UpdatePrefilterDefaultsRequest,
} from "../../../lib/soldisco-api/contracts";
import { NumberSettingField } from "./NumberSettingField";
import {
  freshnessFields,
  prefilterDraftHasChanges,
  rpcFields,
  toPrefilterDefaultsDraft,
  validatePrefilterDefaultsDraft,
  type PrefilterDefaultsField,
} from "./prefilterDefaultsDraft";

type PrefilterDefaultsFormProps = {
  settings: PrefilterDefaultsResponse;
  backendConnected: boolean;
  streamStopped: boolean;
  settingsState: "READY" | "REFRESHING" | "SAVING" | "STALE";
  saving: boolean;
  errorMessage: string | null;
  saveNotice: string | null;
  onRefresh: () => void;
  onSave: (
    request: UpdatePrefilterDefaultsRequest,
  ) => Promise<PrefilterDefaultsResponse | null>;
};

export function PrefilterDefaultsForm({
  settings,
  backendConnected,
  streamStopped,
  settingsState,
  saving,
  errorMessage,
  saveNotice,
  onRefresh,
  onSave,
}: PrefilterDefaultsFormProps) {
  const [draft, setDraft] = useState(() =>
    toPrefilterDefaultsDraft(settings.values),
  );
  const validation = useMemo(
    () => validatePrefilterDefaultsDraft(draft, settings.bounds),
    [draft, settings.bounds],
  );
  const dirty = prefilterDraftHasChanges(
    draft,
    settings.values,
    validation,
  );
  const canEdit =
    backendConnected &&
    streamStopped &&
    settingsState === "READY" &&
    !saving;
  const canSave = canEdit && dirty && validation.valid;

  function changeField(field: PrefilterDefaultsField, value: string) {
    setDraft((current) => ({ ...current, [field]: value }));
  }

  function discardChanges() {
    setDraft(toPrefilterDefaultsDraft(settings.values));
  }

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!canSave || !validation.valid) return;

    await onSave({
      expected_revision: settings.revision,
      values: validation.values,
    });
  }

  return (
    <form
      className="prefilter-defaults__form"
      onSubmit={(event) => void submit(event)}
    >
      <div className="settings-groups">
        <fieldset disabled={!canEdit}>
          <legend>Freshness and observation</legend>
          <div className="settings-fields">
            {freshnessFields.map((definition) => (
              <NumberSettingField
                key={definition.field}
                definition={definition}
                bounds={settings.bounds}
                value={draft[definition.field]}
                error={validation.errors[definition.field]}
                disabled={!canEdit}
                onChange={(value) =>
                  changeField(definition.field, value)
                }
              />
            ))}
          </div>
        </fieldset>

        <fieldset disabled={!canEdit}>
          <legend>RPC admission</legend>
          <div className="settings-fields settings-fields--rpc">
            {rpcFields.map((definition) => (
              <NumberSettingField
                key={definition.field}
                definition={definition}
                bounds={settings.bounds}
                value={draft[definition.field]}
                error={validation.errors[definition.field]}
                disabled={!canEdit}
                onChange={(value) =>
                  changeField(definition.field, value)
                }
              />
            ))}
          </div>
        </fieldset>
      </div>

      <div className="settings-apply" role="status" aria-live="polite">
        <div>
          <strong>
            {settingsState === "STALE"
              ? "Retained settings are stale"
              : settingsState === "SAVING"
                ? "Saving the new collector defaults"
                : settingsState === "REFRESHING"
                  ? "Refreshing authoritative settings"
                  : streamStopped
                    ? "Applies on next stream start"
                    : "Stop the stream to edit"}
          </strong>
          <p>
            {settingsState === "STALE"
              ? "Reload the authoritative revision before editing or saving."
              : settingsState === "SAVING"
                ? "The complete revision is being stored atomically."
                : settingsState === "REFRESHING"
                  ? "Editing resumes after the current revision is confirmed."
                  : streamStopped
                    ? "Saving does not start the collector. The next Start uses the new revision."
                    : "The running collector keeps the settings it started with."}
          </p>
          {errorMessage !== null && (
            <span className="settings-apply__error">{errorMessage}</span>
          )}
          {saveNotice !== null && (
            <span className="settings-apply__success">{saveNotice}</span>
          )}
        </div>
        <div className="control-card__actions">
          {errorMessage !== null && (
            <button type="button" onClick={onRefresh} disabled={saving}>
              Reload settings
            </button>
          )}
          <button
            type="button"
            onClick={discardChanges}
            disabled={!canEdit || !dirty}
          >
            Discard
          </button>
          <button type="submit" disabled={!canSave}>
            {saving ? "Saving…" : "Save defaults"}
          </button>
        </div>
      </div>
    </form>
  );
}
