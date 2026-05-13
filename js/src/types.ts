/** Quota dimension — what resource is being tracked. */
export enum Dimension {
  Requests = "requests",
  Tokens = "tokens",
  Concurrency = "concurrency",
  CostUsd = "cost_usd",
  Bytes = "bytes",
}

/** Time window for quota reset. */
export enum Window {
  Second = "second",
  Minute = "minute",
  Hour = "hour",
  Day = "day",
}

const WINDOW_NANOS: Record<Window, number> = {
  [Window.Second]: 1_000_000_000,
  [Window.Minute]: 60_000_000_000,
  [Window.Hour]: 3_600_000_000_000,
  [Window.Day]: 86_400_000_000_000,
};

const WINDOW_SECS: Record<Window, number> = {
  [Window.Second]: 1,
  [Window.Minute]: 60,
  [Window.Hour]: 3_600,
  [Window.Day]: 86_400,
};

export function windowAsNanos(w: Window): number {
  return WINDOW_NANOS[w];
}

export function windowAsSecs(w: Window): number {
  return WINDOW_SECS[w];
}

/** Classified response status for health tracking. */
export enum StatusClass {
  Success = "success",
  RateLimited = "rate_limited",
  Forbidden = "forbidden",
  ServerError = "server_error",
  Timeout = "timeout",
  ClientError = "client_error",
}
