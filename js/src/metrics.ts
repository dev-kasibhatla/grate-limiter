/** Engine-level metrics counters. */
export class Metrics {
  private _selects = 0;
  private _observations = 0;
  private _cooldownsTriggered = 0;
  private _noProviderAvailable = 0;

  get selects(): number {
    return this._selects;
  }
  get observations(): number {
    return this._observations;
  }
  get cooldownsTriggered(): number {
    return this._cooldownsTriggered;
  }
  get noProviderAvailable(): number {
    return this._noProviderAvailable;
  }

  /** @internal */
  incSelects(): void {
    this._selects++;
  }
  /** @internal */
  incObservations(): void {
    this._observations++;
  }
  /** @internal */
  incCooldowns(): void {
    this._cooldownsTriggered++;
  }
  /** @internal */
  incNoProvider(): void {
    this._noProviderAvailable++;
  }
}
