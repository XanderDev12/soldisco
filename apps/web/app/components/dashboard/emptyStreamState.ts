import type { StreamStatus } from "../../lib/soldisco-api/contracts";

type EmptyStreamStateInput = {
  backendConnected: boolean;
  streamStatus: StreamStatus | null;
  requestedRunning: boolean;
};

export type EmptyStreamState = {
  title: string;
  detail: string;
};

export function describeEmptyStream({
  backendConnected,
  streamStatus,
  requestedRunning,
}: EmptyStreamStateInput): EmptyStreamState {
  if (!backendConnected || streamStatus === null) {
    return {
      title: "Waiting for the local backend",
      detail: "Discovery will remain empty until the API is reachable.",
    };
  }

  switch (streamStatus) {
    case "STARTING":
      return {
        title: "Collector is starting",
        detail:
          "The backend is restoring its state and opening the Pump data sources.",
      };
    case "RUNNING":
      return {
        title: "Waiting for decoded discoveries",
        detail:
          "Structurally valid Pump and PumpSwap events will appear here.",
      };
    case "DEGRADED":
      return {
        title: "Collector is running in a degraded state",
        detail: requestedRunning
          ? "Collection is still requested, but at least one backend dependency needs attention."
          : "The backend reports impaired collection without an active run request.",
      };
    case "ERROR":
      return {
        title: "Collector stopped with an error",
        detail:
          "Review the backend error above, then retry after the dependency is healthy.",
      };
    case "STOPPING":
      return {
        title: "Collector is stopping",
        detail:
          "No new discoveries will appear while the backend saves its current state.",
      };
    case "STOPPED":
      return requestedRunning
        ? {
            title: "Collector start is still requested",
            detail:
              "The backend has not entered a running state. Review its status before retrying.",
          }
        : {
            title: "Stream stopped",
            detail:
              "Start the stream when you are ready to observe candidates.",
          };
  }
}
