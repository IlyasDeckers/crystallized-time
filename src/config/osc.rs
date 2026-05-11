//! OSC-specific configuration. Listen port and send address are CLI-only
//! (they have no useful defaults); this struct holds only the knobs that
//! have non-trivial defaults worth keeping out of `main.rs`.

#[derive(Clone, Debug)]
pub struct OscConfig {
    /// Target rate for state messages, in Hz. Throttling is wall-clock-based,
    /// so the rate is honest regardless of BPM. At default 120 BPM × 25
    /// ticks/period = 50 ticks/sec, every tick ships state. At higher tick
    /// rates the throttle starts skipping ticks.
    pub state_rate_hz: f64,
    /// When false, state messages are not pushed even if --osc-send is set.
    /// Events still flow. Useful for bandwidth-sensitive setups where the
    /// receiver only needs event triggers.
    pub enable_state: bool,
}

impl Default for OscConfig {
    fn default() -> Self {
        Self {
            state_rate_hz: 50.0,
            enable_state: true,
        }
    }
}