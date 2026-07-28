import type { ApiErrorBody } from "./contracts";
import { ContractParseError } from "./parsers";

export class SoldiscoApiError extends Error {
  readonly code: string;
  readonly status: number | null;

  constructor(
    code: string,
    message: string,
    status: number | null = null,
    options?: ErrorOptions,
  ) {
    super(message, options);
    this.name = "SoldiscoApiError";
    this.code = code;
    this.status = status;
  }
}

function parseApiErrorBody(value: unknown): ApiErrorBody | null {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    return null;
  }
  const error = (value as Record<string, unknown>).error;
  if (typeof error !== "object" || error === null || Array.isArray(error)) {
    return null;
  }
  const { code, message } = error as Record<string, unknown>;
  if (typeof code !== "string" || typeof message !== "string") return null;
  return { error: { code, message } };
}

export async function errorFromResponse(
  response: Response,
): Promise<SoldiscoApiError> {
  let body: unknown;
  try {
    body = await response.json();
  } catch {
    body = null;
  }

  const parsed = parseApiErrorBody(body);
  if (parsed) {
    return new SoldiscoApiError(
      parsed.error.code,
      parsed.error.message,
      response.status,
    );
  }

  return new SoldiscoApiError(
    "HTTP_ERROR",
    `The local backend returned HTTP ${response.status}.`,
    response.status,
  );
}

export function normalizeApiError(error: unknown): SoldiscoApiError {
  if (error instanceof SoldiscoApiError) return error;
  if (error instanceof ContractParseError) {
    return new SoldiscoApiError(
      "INVALID_API_RESPONSE",
      error.message,
      null,
      { cause: error },
    );
  }
  if (error instanceof SyntaxError) {
    return new SoldiscoApiError(
      "INVALID_API_RESPONSE",
      "The local backend returned invalid JSON.",
      null,
      { cause: error },
    );
  }
  if (error instanceof DOMException && error.name === "AbortError") {
    return new SoldiscoApiError(
      "REQUEST_TIMEOUT",
      "The local backend did not respond in time.",
      null,
      { cause: error },
    );
  }
  return new SoldiscoApiError(
    "BACKEND_UNREACHABLE",
    "The local backend could not be reached.",
    null,
    { cause: error },
  );
}
