import type { QualificationDefaultsBounds } from "../../../lib/soldisco-api/contracts";
import {
  formatQualificationBounds,
  type QualificationFieldDefinition,
} from "./qualificationDefaultsDraft";

type QualificationNumberSettingFieldProps = {
  definition: QualificationFieldDefinition;
  bounds: QualificationDefaultsBounds;
  value: string;
  error: string | null;
  disabled: boolean;
  onChange: (value: string) => void;
};

export function QualificationNumberSettingField({
  definition,
  bounds,
  value,
  error,
  disabled,
  onChange,
}: QualificationNumberSettingFieldProps) {
  const inputId = `qualification-${definition.field}`;
  const detailId = `${inputId}-detail`;
  const errorId = `${inputId}-error`;

  return (
    <div className="setting-field">
      <label htmlFor={inputId}>{definition.label}</label>
      <p id={detailId}>{definition.description}</p>
      <div className="setting-field__input">
        <input
          id={inputId}
          type="number"
          inputMode={definition.inputMode}
          step={definition.step}
          value={value}
          disabled={disabled}
          aria-invalid={error !== null}
          aria-describedby={
            error === null ? detailId : `${detailId} ${errorId}`
          }
          onChange={(event) => onChange(event.target.value)}
        />
        <span>{definition.unit}</span>
      </div>
      <small>{formatQualificationBounds(definition, bounds)}</small>
      {error !== null && (
        <span id={errorId} className="setting-field__error" role="alert">
          {error}
        </span>
      )}
    </div>
  );
}
