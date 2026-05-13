/** All errors produced by grate-limiter. */
export class GrateLimiterError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "GrateLimiterError";
  }
}

export class UnknownCapability extends GrateLimiterError {
  constructor(public readonly capability: string) {
    super(`unknown capability: ${capability}`);
    this.name = "UnknownCapability";
  }
}

export class UnknownProvider extends GrateLimiterError {
  constructor(public readonly provider: string) {
    super(`unknown provider: ${provider}`);
    this.name = "UnknownProvider";
  }
}

export class NoAvailableProviders extends GrateLimiterError {
  constructor(public readonly capability: string) {
    super(`no available providers for capability: ${capability}`);
    this.name = "NoAvailableProviders";
  }
}
