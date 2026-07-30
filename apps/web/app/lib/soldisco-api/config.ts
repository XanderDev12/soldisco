export const DEFAULT_LOCAL_API_PORT = 8080;
export const localApiPortPreferenceStorageKey =
  "soldisco.local-api-port.v1";

const LOCAL_API_HOST = "127.0.0.1";
const LOCAL_API_PATH = "/api/v1";
const LOCAL_API_PROXY_PATH = "/api/local-backend";
// Fetch-blocked ports are excluded. Port 80 is also excluded because URL
// normalization omits it from Host while the Rust guard expects host:port.
const unsupportedLocalApiPorts = new Set([
  1, 7, 9, 11, 13, 15, 17, 19, 20, 21, 22, 23, 25, 37, 42, 43, 53,
  69, 77, 79, 80, 87, 95, 101, 102, 103, 104, 109, 110, 111, 113,
  115, 117, 119, 123, 135, 137, 139, 143, 161, 179, 389, 427, 465,
  512, 513, 514, 515, 526, 530, 531, 532, 540, 548, 554, 556, 563,
  587, 601, 636, 989, 990, 993, 995, 1719, 1720, 1723, 2049, 3659,
  4045, 4190, 5060, 5061, 6000, 6566, 6665, 6666, 6667, 6668, 6669,
  6679, 6697, 10080,
]);
const localPageHostnames = new Set([
  "localhost",
  "127.0.0.1",
  "::1",
  "[::1]",
]);

type PreferenceStorage = Pick<Storage, "getItem" | "setItem">;

export type LocalApiResolution = {
  baseUrl: string | null;
  reason: "LOCAL" | "NON_LOCAL_PAGE" | "INVALID_PORT";
};

export function parseLocalApiPort(value: unknown): number | null {
  const candidate =
    typeof value === "number"
      ? value
      : typeof value === "string" && /^\d+$/.test(value.trim())
        ? Number(value.trim())
        : Number.NaN;

  return Number.isSafeInteger(candidate) &&
    candidate >= 1 &&
    candidate <= 65_535 &&
    !unsupportedLocalApiPorts.has(candidate)
    ? candidate
    : null;
}

export function buildLocalApiBaseUrl(
  port = DEFAULT_LOCAL_API_PORT,
): string {
  const parsedPort = parseLocalApiPort(port);
  if (parsedPort === null) {
    throw new RangeError(
      "The local API port must be a browser-safe integer from 1 to 65535.",
    );
  }
  return `http://${LOCAL_API_HOST}:${parsedPort}${LOCAL_API_PATH}`;
}

export function buildLocalApiProxyBaseUrl(
  port = DEFAULT_LOCAL_API_PORT,
): string {
  const parsedPort = parseLocalApiPort(port);
  if (parsedPort === null) {
    throw new RangeError(
      "The local API port must be a browser-safe integer from 1 to 65535.",
    );
  }
  return `${LOCAL_API_PROXY_PATH}/${parsedPort}${LOCAL_API_PATH}`;
}

export function isLocalHttpUrl(url: URL): boolean {
  return (
    url.protocol === "http:" &&
    localPageHostnames.has(url.hostname) &&
    url.username === "" &&
    url.password === ""
  );
}

export function readLocalApiPortPreference(
  storage: Pick<PreferenceStorage, "getItem">,
): number {
  try {
    return (
      parseLocalApiPort(
        storage.getItem(localApiPortPreferenceStorageKey),
      ) ?? DEFAULT_LOCAL_API_PORT
    );
  } catch {
    return DEFAULT_LOCAL_API_PORT;
  }
}

export function writeLocalApiPortPreference(
  storage: Pick<PreferenceStorage, "setItem">,
  port: number,
): boolean {
  const parsedPort = parseLocalApiPort(port);
  if (parsedPort === null) {
    throw new RangeError(
      "The local API port must be a browser-safe integer from 1 to 65535.",
    );
  }

  try {
    storage.setItem(
      localApiPortPreferenceStorageKey,
      String(parsedPort),
    );
    return true;
  } catch {
    return false;
  }
}

export function resolveLocalApiUrl(
  pageOrigin: string,
  port = DEFAULT_LOCAL_API_PORT,
): LocalApiResolution {
  const parsedPort = parseLocalApiPort(port);
  if (parsedPort === null) {
    return { baseUrl: null, reason: "INVALID_PORT" };
  }

  let pageUrl: URL;
  try {
    pageUrl = new URL(pageOrigin);
  } catch {
    return { baseUrl: null, reason: "NON_LOCAL_PAGE" };
  }

  if (
    !isLocalHttpUrl(pageUrl) ||
    pageUrl.pathname !== "/" ||
    pageUrl.search !== "" ||
    pageUrl.hash !== ""
  ) {
    return { baseUrl: null, reason: "NON_LOCAL_PAGE" };
  }

  return {
    baseUrl: buildLocalApiProxyBaseUrl(parsedPort),
    reason: "LOCAL",
  };
}
