import {
  buildLocalApiBaseUrl,
  isLocalHttpUrl,
  parseLocalApiPort,
} from "./config";

const LOCAL_CONTROL_HEADER_NAME = "X-Soldisco-Control";
const LOCAL_CONTROL_HEADER_VALUE = "soldisco-local-ui-v1";
const MAX_REQUEST_BODY_BYTES = 64 * 1024;
const forwardedRequestHeaders = [
  "accept",
  "content-type",
  "last-event-id",
] as const;
const forwardedResponseHeaders = [
  "cache-control",
  "content-type",
  "etag",
  "last-modified",
  "retry-after",
] as const;

type UpstreamFetcher = (
  input: string,
  init: RequestInit,
) => Promise<Response>;

function apiError(
  status: number,
  code: string,
  message: string,
  headers: HeadersInit = {},
): Response {
  return Response.json(
    { error: { code, message } },
    {
      status,
      headers: {
        "Cache-Control": "no-store",
        ...headers,
      },
    },
  );
}

function allowedMethods(path: readonly string[]): readonly string[] {
  const route = path.join("/");
  if (
    route === "health" ||
    route === "stream" ||
    route === "discovery" ||
    route === "events"
  ) {
    return ["GET"];
  }
  if (route === "stream/start" || route === "stream/stop") {
    return ["POST"];
  }
  if (
    route === "settings/prefilter-defaults" ||
    route === "settings/qualification-defaults"
  ) {
    return ["GET", "PUT"];
  }
  if (
    path.length === 2 &&
    path[0] === "tokens" &&
    /^[1-9A-HJ-NP-Za-km-z]{32,44}$/.test(path[1] ?? "")
  ) {
    return ["GET"];
  }
  return [];
}

function validateLocalControlRequest(
  request: Request,
  requestUrl: URL,
): Response | null {
  if (
    request.headers.get(LOCAL_CONTROL_HEADER_NAME) !==
    LOCAL_CONTROL_HEADER_VALUE
  ) {
    return apiError(
      403,
      "LOCAL_CONTROL_FORBIDDEN",
      "The local control header is missing or invalid.",
    );
  }
  if (request.headers.get("origin") !== requestUrl.origin) {
    return apiError(
      403,
      "LOCAL_CONTROL_ORIGIN_FORBIDDEN",
      "Local control requests must originate from this interface.",
    );
  }
  return null;
}

function validateBrowserRequestSource(
  request: Request,
  requestUrl: URL,
): Response | null {
  const origin = request.headers.get("origin");
  if (origin !== null && origin !== requestUrl.origin) {
    return apiError(
      403,
      "LOCAL_API_GATEWAY_ORIGIN_FORBIDDEN",
      "Browser requests to the local API gateway must be same-origin.",
    );
  }

  const fetchSite = request.headers.get("sec-fetch-site");
  if (
    fetchSite !== null &&
    fetchSite !== "same-origin" &&
    fetchSite !== "none"
  ) {
    return apiError(
      403,
      "LOCAL_API_GATEWAY_FETCH_SITE_FORBIDDEN",
      "Cross-site browser requests to the local API gateway are forbidden.",
    );
  }

  return null;
}

function buildUpstreamHeaders(request: Request, localControl: boolean) {
  const headers = new Headers();
  for (const name of forwardedRequestHeaders) {
    const value = request.headers.get(name);
    if (value !== null) headers.set(name, value);
  }
  if (localControl) {
    headers.set(LOCAL_CONTROL_HEADER_NAME, LOCAL_CONTROL_HEADER_VALUE);
  }
  return headers;
}

function buildBrowserResponse(upstream: Response): Response {
  if (upstream.status >= 300 && upstream.status < 400) {
    return apiError(
      502,
      "LOCAL_API_UPSTREAM_REDIRECT",
      "The local Rust backend returned an unexpected redirect.",
    );
  }

  const headers = new Headers();
  for (const name of forwardedResponseHeaders) {
    const value = upstream.headers.get(name);
    if (value !== null) headers.set(name, value);
  }
  if (!headers.has("cache-control")) {
    headers.set("Cache-Control", "no-store");
  }
  if (
    headers.get("content-type")?.toLowerCase().startsWith(
      "text/event-stream",
    )
  ) {
    headers.set("Cache-Control", "no-cache");
    headers.set("X-Accel-Buffering", "no");
  }

  return new Response(upstream.body, {
    status: upstream.status,
    statusText: upstream.statusText,
    headers,
  });
}

export async function proxyLocalApiRequest(
  request: Request,
  portValue: string,
  path: readonly string[],
  fetcher: UpstreamFetcher = fetch,
): Promise<Response> {
  let requestUrl: URL;
  try {
    requestUrl = new URL(request.url);
  } catch {
    return apiError(
      400,
      "INVALID_LOCAL_API_REQUEST",
      "The local API gateway received an invalid request URL.",
    );
  }
  if (!isLocalHttpUrl(requestUrl)) {
    return apiError(
      403,
      "LOCAL_API_GATEWAY_LOCAL_ONLY",
      "The local API gateway is available only from a loopback web origin.",
    );
  }

  const sourceRejection = validateBrowserRequestSource(
    request,
    requestUrl,
  );
  if (sourceRejection !== null) return sourceRejection;

  const port = parseLocalApiPort(portValue);
  if (port === null) {
    return apiError(
      400,
      "INVALID_LOCAL_API_PORT",
      "The local API port is invalid or browser-restricted.",
    );
  }

  const methods = allowedMethods(path);
  if (methods.length === 0) {
    return apiError(
      404,
      "LOCAL_API_ROUTE_NOT_ALLOWED",
      "That route is not exposed by the local API gateway.",
    );
  }

  const method = request.method.toUpperCase();
  if (!methods.includes(method)) {
    return apiError(
      405,
      "LOCAL_API_METHOD_NOT_ALLOWED",
      "That method is not allowed for this local API route.",
      { Allow: methods.join(", ") },
    );
  }

  const localControl = method === "POST" || method === "PUT";
  if (localControl) {
    const rejection = validateLocalControlRequest(request, requestUrl);
    if (rejection !== null) return rejection;
  }

  const declaredLength = Number(request.headers.get("content-length"));
  if (
    Number.isFinite(declaredLength) &&
    declaredLength > MAX_REQUEST_BODY_BYTES
  ) {
    return apiError(
      413,
      "LOCAL_API_REQUEST_TOO_LARGE",
      "The local API request body is too large.",
    );
  }

  let body: ArrayBuffer | undefined;
  if (method !== "GET") {
    body = await request.arrayBuffer();
    if (body.byteLength > MAX_REQUEST_BODY_BYTES) {
      return apiError(
        413,
        "LOCAL_API_REQUEST_TOO_LARGE",
        "The local API request body is too large.",
      );
    }
  }

  const upstreamPath = path.map(encodeURIComponent).join("/");
  const upstreamUrl = `${buildLocalApiBaseUrl(port)}/${upstreamPath}`;

  try {
    const upstream = await fetcher(upstreamUrl, {
      method,
      headers: buildUpstreamHeaders(request, localControl),
      body,
      cache: "no-store",
      redirect: "manual",
      signal: request.signal,
    });
    return buildBrowserResponse(upstream);
  } catch {
    return apiError(
      502,
      "LOCAL_API_UPSTREAM_UNREACHABLE",
      `The local Rust backend could not be reached on port ${port}.`,
    );
  }
}
