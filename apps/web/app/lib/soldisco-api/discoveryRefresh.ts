import type { DiscoverySnapshot } from "./contracts";

export type DiscoveryRefreshCoordinator = {
  refresh: () => Promise<DiscoverySnapshot>;
  dispose: () => void;
};

export function createDiscoveryRefreshCoordinator(
  readSnapshot: () => Promise<DiscoverySnapshot>,
  acceptSnapshot: (snapshot: DiscoverySnapshot) => void,
): DiscoveryRefreshCoordinator {
  let disposed = false;
  let highestAcceptedSequence = -1;
  let queued = false;
  let inFlight: Promise<DiscoverySnapshot> | null = null;

  async function run(): Promise<DiscoverySnapshot> {
    let latestSnapshot: DiscoverySnapshot | null = null;
    let latestError: unknown = null;

    do {
      queued = false;

      try {
        const nextSnapshot = await readSnapshot();
        latestSnapshot = nextSnapshot;
        latestError = null;

        if (
          !disposed &&
          nextSnapshot.sequence >= highestAcceptedSequence
        ) {
          highestAcceptedSequence = nextSnapshot.sequence;
          acceptSnapshot(nextSnapshot);
        }
      } catch (error) {
        latestError = error;
      }
    } while (queued && !disposed);

    if (latestError !== null) throw latestError;
    if (latestSnapshot === null) {
      throw new Error("Discovery refresh stopped before a snapshot arrived.");
    }

    return latestSnapshot;
  }

  return {
    refresh() {
      if (inFlight !== null) {
        queued = true;
        return inFlight;
      }

      inFlight = run().finally(() => {
        inFlight = null;
      });
      return inFlight;
    },
    dispose() {
      disposed = true;
      queued = false;
    },
  };
}
