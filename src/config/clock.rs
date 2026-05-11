//! Substrate-derived MIDI clock parameters. The clock is a
//! gate-on-channel pulse emitted every time the chain's global
//! magnetization crosses zero.

#[derive(Clone, Debug)]
pub struct ClockConfig {
    pub enabled: bool,
    /// MIDI channel for clock pulses (0-15). Default: 15 (channel 16 in 1-based UI).
    pub channel: u8,
    /// Pitch for clock note-ons. Irrelevant for clock use; default C3.
    pub pitch: u8,
    /// Crossing threshold on <M>. Tighter than per-site since <M> is averaged.
    pub crossing_threshold: f64,
    /// Minimum ticks between clock pulses.
    pub debounce_ticks: u64,
    /// Clock gate length in milliseconds.
    pub gate_length_ms: u64,
}

impl Default for ClockConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            channel: 15,
            pitch: 48,
            crossing_threshold: 0.05,
            debounce_ticks: 2,
            gate_length_ms: 25,
        }
    }
}