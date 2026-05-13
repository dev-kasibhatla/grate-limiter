#![no_main]
use libfuzzer_sys::fuzz_target;
use grate_limiter::*;

fuzz_target!(|data: &[u8]| {
    // Fuzz JSON config parsing
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = serde_json::from_str::<ProviderConfig>(s);
        let _ = serde_json::from_str::<CapabilityConfig>(s);
        let _ = serde_json::from_str::<Observation>(s);
        let _ = serde_json::from_str::<EngineConfig>(s);
    }
});
