import type {
  PrefilterDefaultsBounds,
  PrefilterDefaultsValues,
} from "../../../lib/soldisco-api/contracts";

export type PrefilterDefaultsField = keyof PrefilterDefaultsValues;
export type PrefilterDefaultsDraft = Record<PrefilterDefaultsField, string>;
export type PrefilterDefaultsErrors = Record<
  PrefilterDefaultsField,
  string | null
>;

export type PrefilterDefaultsValidation =
  | {
      valid: true;
      errors: PrefilterDefaultsErrors;
      values: PrefilterDefaultsValues;
    }
  | {
      valid: false;
      errors: PrefilterDefaultsErrors;
      values: null;
    };

export type PrefilterFieldDefinition = {
  field: PrefilterDefaultsField;
  label: string;
  description: string;
  unit: string;
  scale: 1 | 1_000;
  step: string;
  inputMode: "numeric" | "decimal";
};

export const freshnessFields: readonly PrefilterFieldDefinition[] = [
  {
    field: "max_event_age_ms",
    label: "Maximum creation age",
    description:
      "Discard a creation before HTTP if its source timestamp is older than this.",
    unit: "seconds",
    scale: 1_000,
    step: "0.1",
    inputMode: "decimal",
  },
  {
    field: "observation_window_ms",
    label: "Observation window",
    description:
      "Track matching mint or pool activity for this non-extending period.",
    unit: "seconds",
    scale: 1_000,
    step: "0.1",
    inputMode: "decimal",
  },
  {
    field: "max_active_windows",
    label: "Maximum active windows",
    description:
      "Bound the number of provisional and confirmed activity windows in memory.",
    unit: "windows",
    scale: 1,
    step: "1",
    inputMode: "numeric",
  },
] as const;

export const rpcFields: readonly PrefilterFieldDefinition[] = [
  {
    field: "rpc_requests_per_second",
    label: "Discovery RPC rate",
    description:
      "Limit how many distinct discovery transaction reads may begin each second.",
    unit: "req/s",
    scale: 1,
    step: "1",
    inputMode: "numeric",
  },
  {
    field: "rpc_max_in_flight",
    label: "Maximum requests in flight",
    description:
      "Bound concurrent one-shot discovery transaction reads across both sources.",
    unit: "requests",
    scale: 1,
    step: "1",
    inputMode: "numeric",
  },
  {
    field: "rpc_request_timeout_ms",
    label: "Request timeout",
    description:
      "Abandon one discovery read after this interval and continue with newer work.",
    unit: "seconds",
    scale: 1_000,
    step: "0.1",
    inputMode: "decimal",
  },
  {
    field: "rpc_rate_limit_cooldown_ms",
    label: "Rate-limit cooldown",
    description:
      "Delay later distinct signatures after the provider reports a rate limit.",
    unit: "seconds",
    scale: 1_000,
    step: "0.1",
    inputMode: "decimal",
  },
] as const;

const allFields = [...freshnessFields, ...rpcFields] as const;

function formatScaledValue(value: number, scale: 1 | 1_000): string {
  if (scale === 1) return String(value);
  const seconds = value / scale;
  return Number.isInteger(seconds)
    ? String(seconds)
    : seconds.toFixed(3).replace(/0+$/, "").replace(/\.$/, "");
}

export function toPrefilterDefaultsDraft(
  values: PrefilterDefaultsValues,
): PrefilterDefaultsDraft {
  return Object.fromEntries(
    allFields.map(({ field, scale }) => [
      field,
      formatScaledValue(values[field], scale),
    ]),
  ) as PrefilterDefaultsDraft;
}

function emptyErrors(): PrefilterDefaultsErrors {
  return Object.fromEntries(
    allFields.map(({ field }) => [field, null]),
  ) as PrefilterDefaultsErrors;
}

function parseDraftField(
  raw: string,
  definition: PrefilterFieldDefinition,
): number | null {
  const normalized = raw.trim();
  const pattern =
    definition.scale === 1
      ? /^(?:0|[1-9]\d*)$/
      : /^(?:0|[1-9]\d*)(?:\.\d{1,3})?$/;
  if (!pattern.test(normalized)) return null;

  const entered = Number(normalized);
  const scaled = entered * definition.scale;
  if (!Number.isSafeInteger(scaled)) return null;
  return scaled;
}

export function validatePrefilterDefaultsDraft(
  draft: PrefilterDefaultsDraft,
  bounds: PrefilterDefaultsBounds,
): PrefilterDefaultsValidation {
  const errors = emptyErrors();
  const entries: [PrefilterDefaultsField, number][] = [];

  for (const definition of allFields) {
    const value = parseDraftField(draft[definition.field], definition);
    const allowed = bounds[definition.field];
    if (value === null) {
      errors[definition.field] =
        definition.scale === 1
          ? "Enter a whole number."
          : "Enter seconds with no more than three decimal places.";
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

  const values = Object.fromEntries(entries) as PrefilterDefaultsValues;
  if (values.rpc_request_timeout_ms > values.max_event_age_ms) {
    errors.rpc_request_timeout_ms =
      "Request timeout cannot exceed the maximum creation age.";
  }
  if (values.observation_window_ms < values.rpc_request_timeout_ms) {
    errors.observation_window_ms =
      "Observation window cannot be shorter than the request timeout.";
  }
  if (Object.values(errors).some((error) => error !== null)) {
    return { valid: false, errors, values: null };
  }

  return {
    valid: true,
    errors,
    values,
  };
}

export function prefilterDraftHasChanges(
  draft: PrefilterDefaultsDraft,
  baseline: PrefilterDefaultsValues,
  validation: PrefilterDefaultsValidation,
): boolean {
  if (validation.valid) {
    return allFields.some(
      ({ field }) => validation.values[field] !== baseline[field],
    );
  }

  const baselineDraft = toPrefilterDefaultsDraft(baseline);
  return allFields.some(
    ({ field }) => draft[field] !== baselineDraft[field],
  );
}

export function formatPrefilterBounds(
  definition: PrefilterFieldDefinition,
  bounds: PrefilterDefaultsBounds,
): string {
  const allowed = bounds[definition.field];
  return `${formatScaledValue(allowed.minimum, definition.scale)}–${formatScaledValue(
    allowed.maximum,
    definition.scale,
  )} ${definition.unit}`;
}
