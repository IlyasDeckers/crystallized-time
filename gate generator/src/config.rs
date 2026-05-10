//! Parameters for the simulation and MIDI output.
//!
//! Defaults match the JS prototype's time-crystal phase. Override via CLI in main.rs.

/// Physics parameters governing the spin chain.
#[derive(Clone, Debug)]
pub struct PhysicsConfig {
    /// Number of sites in the chain.
    pub n_sites: usize,
    /// Integration step in simulation units.
    pub dt: f64,
    /// Drive imperfection (epsilon). Pulse angle is (1 - eps) * pi.
    pub eps: f64,
    /// Coupling strength J.
    pub j: f64,
    /// Disorder width W (range of random local Z-fields).
    pub w: f64,
    /// Effective temperature kT (thermal noise strength).
    pub kt: f64,
    /// Number of integration ticks per drive period.
    /// 25 by default — chosen so dt * ticks_per_period = 1.0 sim time unit.
    pub ticks_per_period: u32,
}

impl Default for PhysicsConfig {
    fn default() -> Self {
        Self {
            n_sites: 8,
            dt: 0.04,
            eps: 0.01,
            j: 1.2,
            w: 2.0,
            kt: 0.1,
            ticks_per_period: 25,
        }
    }
}

/// Output topology — how voices are routed to MIDI channels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum OutputMode {
    OneChannelPerChain,
    ChannelPerSite,
}

impl Default for OutputMode {
    fn default() -> Self {
        OutputMode::OneChannelPerChain
    }
}

/// Event detection parameters.
#[derive(Clone, Debug)]
pub struct EventConfig {
    /// Sites whose sigma_z crossings produce MIDI events.
    pub output_sites: Vec<usize>,
    /// Crossing threshold — sz must move from below -threshold to above +threshold (or vice versa).
    pub crossing_threshold: f64,
    /// Minimum ticks between events on the same site.
    pub debounce_ticks: u64,
}

impl Default for EventConfig {
    fn default() -> Self {
        Self {
            output_sites: vec![0, 2, 4, 6],
            crossing_threshold: 0.15,
            debounce_ticks: 4,
        }
    }
}

/// MIDI output parameters.
#[derive(Clone, Debug)]
pub struct MidiConfig {
    /// MIDI note pitch for ChannelPerSite mode (where pitch is irrelevant — gate signals).
    pub pitch: u8,
    /// Per-voice MIDI pitches for OneChannelPerChain mode.
    /// Length must match the number of output sites.
    /// Default: Cmaj7 voicing (C3, E3, G3, B3).
    pub voice_pitches: Vec<u8>,
    /// Gate length in milliseconds (note-on to note-off delay).
    pub gate_length_ms: u64,
    /// Base MIDI channel (0-15).
    /// In OneChannelPerChain: chain's channel.
    /// In ChannelPerSite: voice 0's channel; voice k goes to base_channel + k.
    pub base_channel: u8,
    /// Output topology.
    pub mode: OutputMode,
}

impl Default for MidiConfig {
    fn default() -> Self {
        Self {
            pitch: 48,
            voice_pitches: vec![48, 52, 55, 59], // C3, E3, G3, B3 — Cmaj7
            gate_length_ms: 50,
            base_channel: 0,
            mode: OutputMode::default(),
        }
    }
}

/// Substrate-derived MIDI clock parameters. The clock is a gate-on-channel
/// pulse emitted every time the chain's global magnetization crosses zero.
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

/// Wall-clock pacing.
#[derive(Clone, Debug)]
pub struct TempoConfig {
    /// Drive period in seconds. Tick duration = drive_period / ticks_per_period.
    pub drive_period_secs: f64,
}

impl TempoConfig {
    pub fn from_bpm(bpm: f64) -> Self {
        Self {
            drive_period_secs: 60.0 / bpm,
        }
    }

    pub fn bpm(&self) -> f64 {
        60.0 / self.drive_period_secs
    }
}

impl Default for TempoConfig {
    fn default() -> Self {
        Self {
            drive_period_secs: 0.5, // 120 BPM
        }
    }
}

/// All configuration combined.
#[derive(Clone, Debug, Default)]
pub struct Config {
    pub physics: PhysicsConfig,
    pub events: EventConfig,
    pub midi: MidiConfig,
    pub tempo: TempoConfig,
    pub clock: ClockConfig,
    pub seed: u64,
}
