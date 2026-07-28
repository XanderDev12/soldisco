export type ResponseTicket<Key extends string> = {
  key: Key;
  generation: number;
};

export type ResponseFreshnessCoordinator<Key extends string> = {
  begin: (key: Key) => ResponseTicket<Key>;
  isCurrent: (ticket: ResponseTicket<Key>) => boolean;
  invalidate: () => void;
};

export function createResponseFreshnessCoordinator<
  Key extends string,
>(): ResponseFreshnessCoordinator<Key> {
  let nextGeneration = 0;
  const latestByKey = new Map<Key, number>();

  return {
    begin(key) {
      nextGeneration += 1;
      latestByKey.set(key, nextGeneration);
      return { key, generation: nextGeneration };
    },
    isCurrent(ticket) {
      return latestByKey.get(ticket.key) === ticket.generation;
    },
    invalidate() {
      nextGeneration += 1;
      latestByKey.clear();
    },
  };
}
