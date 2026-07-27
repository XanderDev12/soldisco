export interface TraceContext {
  traceId: string;
  spanId: string;
  parentSpanId: string | null;
  correlationId: string;
}

export interface StructuredLog {
  timestamp: string;
  level: "debug" | "info" | "warn" | "error";
  service: string;
  event: string;
  message: string;
  trace: TraceContext | null;
  attributes: Readonly<Record<string, string | number | boolean | null>>;
}

export interface MetricPoint {
  name: string;
  kind: "COUNTER" | "GAUGE" | "HISTOGRAM";
  value: number;
  unit: string;
  labels: Readonly<Record<string, string>>;
  observedAt: string;
}

export interface ServiceHealth {
  service: string;
  status: "UP" | "DEGRADED" | "DOWN";
  checks: Array<{
    name: string;
    status: "PASS" | "WARN" | "FAIL";
    message: string | null;
  }>;
  checkedAt: string;
}

export interface TelemetrySink {
  log(entry: StructuredLog): void;
  metric(point: MetricPoint): void;
}
