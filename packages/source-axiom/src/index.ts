export interface DiscoveryCursor {
  value: string;
}

export interface SourceTokenObservation {
  source: "axiom";
  mint: string;
  symbol: string | null;
  name: string | null;
  observedAt: string;
  sourceRecordId: string;
  raw: Readonly<Record<string, unknown>>;
}

export interface DiscoveryBatch {
  observations: SourceTokenObservation[];
  nextCursor: DiscoveryCursor | null;
}

export interface DiscoverySourceHealth {
  status: "UP" | "DEGRADED" | "DOWN";
  checkedAt: string;
  message: string | null;
}

export interface DiscoverySource {
  readonly id: "axiom";
  read(cursor: DiscoveryCursor | null, limit: number): Promise<DiscoveryBatch>;
  health(): Promise<DiscoverySourceHealth>;
}

export interface AxiomSourceConfig {
  enabled: boolean;
  pollIntervalMs: number;
  batchSize: number;
  /** Name of a secret reference; never a key value. */
  credentialRef: string | null;
}
