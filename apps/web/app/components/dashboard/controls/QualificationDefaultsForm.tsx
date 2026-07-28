"use client";

import { useMemo, useState, type FormEvent } from "react";
import type {
  QualificationDefaultsResponse,
  UpdateQualificationDefaultsRequest,
} from "../../../lib/soldisco-api/contracts";
import { QualificationNumberSettingField } from "./QualificationNumberSettingField";
import {
  activityQualificationFields,
  flowQualificationFields,
  qualificationDraftHasChanges,
  toQualificationDefaultsDraft,
  validateQualificationDefaultsDraft,
  type QualificationDefaultsField,
} from "./qualificationDefaultsDraft";

type QualificationDefaultsFormProps = {
  settings: QualificationDefaultsResponse;
  backendConnected: boolean;
  settingsState: "READY" | "REFRESHING" | "SAVING" | "STALE";
  saving: boolean;
  errorMessage: string | null;
  saveNotice: string | null;
  onRefresh: () => void;
  onSave: (
    request: UpdateQualificationDefaultsRequest,
  ) => Promise<QualificationDefaultsResponse | null>;
};

export function QualificationDefaultsForm({
  settings,
  backendConnected,
  settingsState,
  saving,
  errorMessage,
  saveNotice,
  onRefresh,
  onSave,
}: QualificationDefaultsFormProps) {
  const [draft, setDraft] = useState(() =>
    toQualificationDefaultsDraft(settings.values),
  );
  const validation = useMemo(
    () => validateQualificationDefaultsDraft(draft, settings.bounds),
    [draft, settings.bounds],
  );
  const dirty = qualificationDraftHasChanges(
    draft,
    settings.values,
    validation,
  );
  const canEdit =
    backendConnected && settingsState === "READY" && !saving;
  const canSave = canEdit && dirty && validation.valid;

  function changeField(
    field: QualificationDefaultsField,
    value: string,
  ) {
    setDraft((current) => ({ ...current, [field]: value }));
  }

  function discardChanges() {
    setDraft(toQualificationDefaultsDraft(settings.values));
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
          <legend>Window activity</legend>
          <div className="settings-fields settings-fields--qualification">
            {activityQualificationFields.map((definition) => (
              <QualificationNumberSettingField
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
          <legend>Quote flow</legend>
          <div className="settings-fields settings-fields--qualification">
            {flowQualificationFields.map((definition) => (
              <QualificationNumberSettingField
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
            {settingsState === "READY"
              ? "Applies to newly discovered windows"
              : settingsState === "STALE"
                ? "Retained settings are stale"
                : settingsState === "SAVING"
                  ? "Saving a new settings revision"
                  : "Refreshing authoritative settings"}
          </strong>
          <p>
            {settingsState === "READY"
              ? "Existing windows keep their pinned ruleset revision. Saving is safe while the stream is running."
              : settingsState === "STALE"
                ? "Reload the authoritative revision before editing or saving."
                : settingsState === "SAVING"
                  ? "Existing windows remain pinned while the new revision is stored."
                  : "Editing resumes after the current revision is confirmed."}
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
