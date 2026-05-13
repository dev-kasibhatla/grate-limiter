/// Traffic pattern configuration for simulation.
#[derive(Debug, Clone)]
pub enum TrafficPattern {
    /// Constant rate of requests per second.
    Steady { rps: u64 },
    /// Sudden burst followed by calm.
    Bursty {
        base_rps: u64,
        burst_rps: u64,
        burst_duration_steps: u64,
        burst_interval_steps: u64,
    },
    /// Linearly increasing traffic.
    Ramp {
        start_rps: u64,
        end_rps: u64,
        ramp_steps: u64,
    },
    /// Custom function.
    Custom(Vec<u64>),
}

impl TrafficPattern {
    /// Get the number of requests to send at the given simulation step.
    /// Each step represents 100ms.
    pub fn requests_at(&self, step: u64) -> u64 {
        match self {
            TrafficPattern::Steady { rps } => rps / 10, // per 100ms
            TrafficPattern::Bursty {
                base_rps,
                burst_rps,
                burst_duration_steps,
                burst_interval_steps,
            } => {
                let in_cycle = step % burst_interval_steps;
                if in_cycle < *burst_duration_steps {
                    burst_rps / 10
                } else {
                    base_rps / 10
                }
            }
            TrafficPattern::Ramp {
                start_rps,
                end_rps,
                ramp_steps,
            } => {
                let progress = (step as f64 / *ramp_steps as f64).min(1.0);
                let rps = *start_rps as f64 + (*end_rps as f64 - *start_rps as f64) * progress;
                (rps / 10.0) as u64
            }
            TrafficPattern::Custom(rates) => {
                if step < rates.len() as u64 {
                    rates[step as usize]
                } else {
                    *rates.last().unwrap_or(&0)
                }
            }
        }
    }
}

/// A traffic generator that produces requests according to a pattern.
pub struct TrafficGenerator {
    pub pattern: TrafficPattern,
    pub step: u64,
}

impl TrafficGenerator {
    pub fn new(pattern: TrafficPattern) -> Self {
        Self { pattern, step: 0 }
    }

    pub fn next_batch(&mut self) -> u64 {
        let requests = self.pattern.requests_at(self.step);
        self.step += 1;
        requests
    }
}

/// Predefined load profiles for simulation.
#[derive(Debug, Clone, Copy)]
pub enum LoadProfile {
    /// Stable RPS.
    Steady,
    /// Sudden spikes.
    Bursty,
    /// Providers degrade sequentially.
    CascadingFailure,
    /// Synchronized retries.
    ThunderingHerd,
    /// Sustained overload.
    QuotaExhaustion,
}

impl LoadProfile {
    pub fn to_traffic_pattern(self) -> TrafficPattern {
        match self {
            LoadProfile::Steady => TrafficPattern::Steady { rps: 100 },
            LoadProfile::Bursty => TrafficPattern::Bursty {
                base_rps: 50,
                burst_rps: 500,
                burst_duration_steps: 10,
                burst_interval_steps: 100,
            },
            LoadProfile::CascadingFailure => TrafficPattern::Ramp {
                start_rps: 50,
                end_rps: 300,
                ramp_steps: 100,
            },
            LoadProfile::ThunderingHerd => TrafficPattern::Bursty {
                base_rps: 10,
                burst_rps: 1000,
                burst_duration_steps: 5,
                burst_interval_steps: 50,
            },
            LoadProfile::QuotaExhaustion => TrafficPattern::Steady { rps: 500 },
        }
    }
}
