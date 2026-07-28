const DEFAULT_LOCAL_API_URL = "http://127.0.0.1:8080/api/v1";
const DEFAULT_LOCAL_WEB_ORIGIN = "http://localhost:3000";
const localHostnames = new Set([
  "localhost",
  "127.0.0.1",
  "::1",
  "[::1]",
]);

export type LocalApiResolution = {
  baseUrl: string | null;
  reason:
    | "LOCAL"
    | "NON_LOCAL_PAGE"
    | "INVALID_URL"
    | "NON_LOCAL_API"
    | "INVALID_WEB_ORIGIN"
    | "LOCAL_ORIGIN_MISMATCH";
};

export function resolveLocalApiUrl(
  pageOrigin: string,
  configuredUrl = DEFAULT_LOCAL_API_URL,
  configuredWebOrigin = DEFAULT_LOCAL_WEB_ORIGIN,
): LocalApiResolution {
  let pageUrl: URL;
  try {
    pageUrl = new URL(pageOrigin);
  } catch {
    return { baseUrl: null, reason: "NON_LOCAL_PAGE" };
  }
  if (
    !["http:", "https:"].includes(pageUrl.protocol) ||
    !localHostnames.has(pageUrl.hostname)
  ) {
    return { baseUrl: null, reason: "NON_LOCAL_PAGE" };
  }

  let webOrigin: URL;
  try {
    webOrigin = new URL(configuredWebOrigin);
  } catch {
    return { baseUrl: null, reason: "INVALID_WEB_ORIGIN" };
  }
  if (
    !["http:", "https:"].includes(webOrigin.protocol) ||
    !localHostnames.has(webOrigin.hostname) ||
    webOrigin.username !== "" ||
    webOrigin.password !== "" ||
    webOrigin.pathname !== "/" ||
    webOrigin.search !== "" ||
    webOrigin.hash !== ""
  ) {
    return { baseUrl: null, reason: "INVALID_WEB_ORIGIN" };
  }
  if (pageUrl.origin !== webOrigin.origin) {
    return { baseUrl: null, reason: "LOCAL_ORIGIN_MISMATCH" };
  }

  let url: URL;
  try {
    url = new URL(configuredUrl);
  } catch {
    return { baseUrl: null, reason: "INVALID_URL" };
  }

  if (
    !["http:", "https:"].includes(url.protocol) ||
    !localHostnames.has(url.hostname)
  ) {
    return { baseUrl: null, reason: "NON_LOCAL_API" };
  }

  return {
    baseUrl: url.toString().replace(/\/$/, ""),
    reason: "LOCAL",
  };
}

export function resolveBrowserApiUrl(): LocalApiResolution {
  if (typeof window === "undefined") {
    return { baseUrl: null, reason: "NON_LOCAL_PAGE" };
  }

  const configuredUrl =
    process.env.NEXT_PUBLIC_SOLDISCO_API_URL?.trim() ||
    DEFAULT_LOCAL_API_URL;
  const configuredWebOrigin =
    process.env.NEXT_PUBLIC_SOLDISCO_WEB_ORIGIN?.trim() ||
    DEFAULT_LOCAL_WEB_ORIGIN;
  return resolveLocalApiUrl(
    window.location.origin,
    configuredUrl,
    configuredWebOrigin,
  );
}
