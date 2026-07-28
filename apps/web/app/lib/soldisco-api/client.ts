import type {
  DiscoverySnapshot,
  DiscoveryToken,
  HealthResponse,
  PrefilterDefaultsResponse,
  StreamCommandResponse,
  StreamStateResponse,
  UpdatePrefilterDefaultsRequest,
} from "./contracts";
import { errorFromResponse, normalizeApiError } from "./errors";
import {
  parseDiscoverySnapshot,
  parseDiscoveryToken,
  parseHealth,
  parsePrefilterDefaults,
  parseStreamCommand,
  parseStreamState,
} from "./parsers";

type JsonParser<T> = (value: unknown) => T;
type Fetcher = typeof fetch;
const STREAM_COMMAND_TIMEOUT_MS = 30_000;
const LOCAL_CONTROL_HEADER_NAME = "X-Soldisco-Control";
const LOCAL_CONTROL_HEADER_VALUE = "soldisco-local-ui-v1";

export class SoldiscoApiClient {
  readonly baseUrl: string;
  readonly #fetcher: Fetcher;
  readonly #timeoutMs: number;

  constructor(baseUrl: string, fetcher: Fetcher = fetch, timeoutMs = 8_000) {
    this.baseUrl = baseUrl.replace(/\/$/, "");
    this.#fetcher = fetcher;
    this.#timeoutMs = timeoutMs;
  }

  get health(): Promise<HealthResponse> {
    return this.#request("/health", parseHealth, {
      acceptedErrorStatuses: [503],
    });
  }

  get stream(): Promise<StreamStateResponse> {
    return this.#request("/stream", parseStreamState);
  }

  get discovery(): Promise<DiscoverySnapshot> {
    return this.#request("/discovery", parseDiscoverySnapshot);
  }

  get prefilterDefaults(): Promise<PrefilterDefaultsResponse> {
    return this.#request(
      "/settings/prefilter-defaults",
      parsePrefilterDefaults,
    );
  }

  token(mint: string): Promise<DiscoveryToken> {
    return this.#request(
      `/tokens/${encodeURIComponent(mint)}`,
      parseDiscoveryToken,
    );
  }

  startStream(): Promise<StreamCommandResponse> {
    return this.#request("/stream/start", parseStreamCommand, {
      method: "POST",
      timeoutMs: STREAM_COMMAND_TIMEOUT_MS,
      localControl: true,
    });
  }

  stopStream(): Promise<StreamCommandResponse> {
    return this.#request("/stream/stop", parseStreamCommand, {
      method: "POST",
      timeoutMs: STREAM_COMMAND_TIMEOUT_MS,
      localControl: true,
    });
  }

  updatePrefilterDefaults(
    request: UpdatePrefilterDefaultsRequest,
  ): Promise<PrefilterDefaultsResponse> {
    return this.#request(
      "/settings/prefilter-defaults",
      parsePrefilterDefaults,
      {
        method: "PUT",
        localControl: true,
        jsonBody: request,
      },
    );
  }

  async #request<T>(
    path: string,
    parser: JsonParser<T>,
    options: {
      method?: "GET" | "POST" | "PUT";
      acceptedErrorStatuses?: number[];
      timeoutMs?: number;
      localControl?: boolean;
      jsonBody?: unknown;
    } = {},
  ): Promise<T> {
    const controller = new AbortController();
    const timeout = globalThis.setTimeout(
      () => controller.abort(),
      options.timeoutMs ?? this.#timeoutMs,
    );

    try {
      const response = await this.#fetcher(`${this.baseUrl}${path}`, {
        method: options.method ?? "GET",
        headers: {
          Accept: "application/json",
          ...(options.localControl
            ? {
                [LOCAL_CONTROL_HEADER_NAME]:
                  LOCAL_CONTROL_HEADER_VALUE,
              }
            : {}),
          ...(options.jsonBody === undefined
            ? {}
            : { "Content-Type": "application/json" }),
        },
        body:
          options.jsonBody === undefined
            ? undefined
            : JSON.stringify(options.jsonBody),
        cache: "no-store",
        signal: controller.signal,
      });

      if (
        !response.ok &&
        !options.acceptedErrorStatuses?.includes(response.status)
      ) {
        throw await errorFromResponse(response);
      }

      return parser(await response.json());
    } catch (error) {
      throw normalizeApiError(error);
    } finally {
      globalThis.clearTimeout(timeout);
    }
  }
}
