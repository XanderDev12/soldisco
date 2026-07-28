import type {
  QualificationDefaultsBounds,
  QualificationDefaultsValues,
} from "../../../lib/soldisco-api/contracts";

export type QualificationDefaultsField =
  keyof QualificationDefaultsValues;
export type QualificationDefaultsDraft = Record<
  QualificationDefaultsField,
  string
>;
export type QualificationDefaultsErrors = Record<
  QualificationDefaultsField,
  string | null
>;

export type QualificationDefaultsValidation =
  | {
      valid: true;
      errors: QualificationDefaultsErrors;
      values: QualificationDefaultsValues;
    }
  | {
      valid: false;
      errors: QualificationDefaultsErrors;
      values: null;
    };

export type QualificationFieldDefinition = {
  field: QualificationDefaultsField;
  label: string;
  description: string;
  unit: string;
  scale: 1 | 100;
  step: string;
  inputMode: "numeric" | "decimal";
};

export const activityQualificationFields: readonly QualificationFieldDefinition[] =
  [
    {
      field: "minimum_trades",
      label: "Minimum trades",
      description:
        "Require this many decoded trades during the complete observation window.",
      unit: "trades",
      scale: 1,
      step: "1",
      inputMode: "numeric",
    },
    {
      field: "minimum_unique_traders",
      label: "Minimum unique traders",
      description:
        "Require activity from this many distinct wallets during the window.",
      unit: "wallets",
      scale: 1,
      step: "1",
      inputMode: "numeric",
    },
    {
      field: "minimum_buys",
      label: "Minimum buys",
      description:
        "Require this many decoded buy events before a window may qualify.",
      unit: "buys",
      scale: 1,
      step: "1",
      inputMode: "numeric",
    },
    {
      field: "minimum_sells",
      label: "Minimum sells",
      description:
        "Require observed sell activity; this is evidence, not a sellability guarantee.",
      unit: "sells",
      scale: 1,
      step: "1",
      inputMode: "numeric",
    },
  ] as const;

export const flowQualificationFields: readonly QualificationFieldDefinition[] =
  [
    {
      field: "minimum_native_quote_volume_units",
      label: "Minimum native quote volume",
      description:
        "Require this many atomic native-quote units across decoded window trades.",
      unit: "lamports",
      scale: 1,
      step: "1",
      inputMode: "numeric",
    },
    {
      field: "minimum_stable_quote_volume_units",
      label: "Minimum stable quote volume",
      description:
        "Require this many atomic stable-quote units when the market uses a supported stable mint.",
      unit: "base units",
      scale: 1,
      step: "1",
      inputMode: "numeric",
    },
    {
      field: "maximum_single_wallet_quote_share_bps",
      label: "Maximum single-wallet share",
      description:
        "Reject windows where one wallet contributes more than this share of quote flow.",
      unit: "%",
      scale: 100,
      step: "0.01",
      inputMode: "decimal",
    },
  ] as const;

const allFields = [
  ...activityQualificationFields,
  ...flowQualificationFields,
] as const;

function formatScaledValue(value: number, scale: 1 | 100): string {
  if (scale === 1) return String(value);
  const percentage = value / scale;
  return Number.isInteger(percentage)
    ? String(percentage)
    : percentage.toFixed(2).replace(/0+$/, "").replace(/\.$/, "");
}

export function toQualificationDefaultsDraft(
  values: QualificationDefaultsValues,
): QualificationDefaultsDraft {
  return Object.fromEntries(
    allFields.map(({ field, scale }) => [
      field,
      formatScaledValue(values[field], scale),
    ]),
  ) as QualificationDefaultsDraft;
}

function emptyErrors(): QualificationDefaultsErrors {
  return Object.fromEntries(
    allFields.map(({ field }) => [field, null]),
  ) as QualificationDefaultsErrors;
}

function parseDraftField(
  raw: string,
  definition: QualificationFieldDefinition,
): number | null {
  const normalized = raw.trim();
  const pattern =
    definition.scale === 1
      ? /^(?:0|[1-9]\d*)$/
      : /^(?:0|[1-9]\d*)(?:\.\d{1,2})?$/;
  if (!pattern.test(normalized)) return null;

  const entered = Number(normalized);
  const scaled = entered * definition.scale;
  if (!Number.isSafeInteger(scaled)) return null;
  return scaled;
}

export function validateQualificationDefaultsDraft(
  draft: QualificationDefaultsDraft,
  bounds: QualificationDefaultsBounds,
): QualificationDefaultsValidation {
  const errors = emptyErrors();
  const entries: [QualificationDefaultsField, number][] = [];

  for (const definition of allFields) {
    const value = parseDraftField(draft[definition.field], definition);
    const allowed = bounds[definition.field];
    if (value === null) {
      errors[definition.field] =
        definition.scale === 1
          ? "Enter a whole number."
          : "Enter a percentage with no more than two decimal places.";
      continue;
    }
    if (value < allowed.minimum || value > allowed.maximum) {
      const minimum = formatScaledValue(allowed.minimum, definition.scale);
      const maximum = formatScaledValue(allowed.maximum, definition.scale);
      errors[definition.field] =
        `Enter a value from ${minimum} through ${maximum} ${definition.unit}.`;
      continue;
    }
    entries.push([definition.field, value]);
  }

  if (Object.values(errors).some((error) => error !== null)) {
    return { valid: false, errors, values: null };
  }

  const values = Object.fromEntries(
    entries,
  ) as QualificationDefaultsValues;
  for (const field of [
    "minimum_unique_traders",
    "minimum_buys",
    "minimum_sells",
  ] as const) {
    if (values[field] > values.minimum_trades) {
      errors[field] = "Cannot exceed the minimum trades value.";
    }
  }

  if (Object.values(errors).some((error) => error !== null)) {
    return { valid: false, errors, values: null };
  }

  return { valid: true, errors, values };
}

export function qualificationDraftHasChanges(
  draft: QualificationDefaultsDraft,
  baseline: QualificationDefaultsValues,
  validation: QualificationDefaultsValidation,
): boolean {
  if (validation.valid) {
    return allFields.some(
      ({ field }) => validation.values[field] !== baseline[field],
    );
  }

  const baselineDraft = toQualificationDefaultsDraft(baseline);
  return allFields.some(
    ({ field }) => draft[field] !== baselineDraft[field],
  );
}

export function formatQualificationBounds(
  definition: QualificationFieldDefinition,
  bounds: QualificationDefaultsBounds,
): string {
  const allowed = bounds[definition.field];
  return `${formatScaledValue(
    allowed.minimum,
    definition.scale,
  )}–${formatScaledValue(
    allowed.maximum,
    definition.scale,
  )} ${definition.unit}`;
}
