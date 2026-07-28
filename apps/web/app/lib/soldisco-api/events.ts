import type { LiveEnvelope } from "./contracts";
import { normalizeApiError, type SoldiscoApiError } from "./errors";
import { parseLiveEnvelope } from "./parsers";

export type LiveConnectionStatus =
  | "IDLE"
  | "CONNECTING"
  | "OPEN"
  | "RETRYING"
  | "CLOSED";

type EventSourceLike = {
  onopen: ((event: Event) => void) | null;
  onerror: ((event: Event) => void) | null;
  addEventListener: (
    type: string,
    listener: (event: MessageEvent<string>) => void,
  ) => void;
  close: () => void;
};

export type EventSourceFactory = (url: string) => EventSourceLike;

export type LiveEventCallbacks = {
  onEnvelope: (envelope: LiveEnvelope) => void;
  onStatus: (status: LiveConnectionStatus) => void;
  onError: (error: SoldiscoApiError) => void;
};

const browserEventSource: EventSourceFactory = (url) =>
  new EventSource(url) as unknown as EventSourceLike;

export function subscribeToSoldiscoEvents(
  baseUrl: string,
  callbacks: LiveEventCallbacks,
  createEventSource: EventSourceFactory = browserEventSource,
): () => void {
  callbacks.onStatus("CONNECTING");

  let source: EventSourceLike;
  try {
    source = createEventSource(`${baseUrl.replace(/\/$/, "")}/events`);
  } catch (error) {
    callbacks.onError(normalizeApiError(error));
    callbacks.onStatus("CLOSED");
    return () => undefined;
  }

  let closed = false;
  source.onopen = () => callbacks.onStatus("OPEN");
  source.onerror = () => {
    if (!closed) callbacks.onStatus("RETRYING");
  };
  source.addEventListener("soldisco", (event) => {
    try {
      callbacks.onEnvelope(parseLiveEnvelope(JSON.parse(event.data)));
    } catch (error) {
      callbacks.onError(normalizeApiError(error));
    }
  });

  return () => {
    closed = true;
    source.close();
    callbacks.onStatus("CLOSED");
  };
}
