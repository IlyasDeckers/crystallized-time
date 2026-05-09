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
            eps: 0.05,
            j: 1.2,
            w: 2.0,
            kt: 0.05,
            ticks_per_period: 25,
        }
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
    /// MIDI note pitch sent on every gate (irrelevant for pure gates).
    pub pitch: u8,
    /// Gate length in milliseconds (note-on to note-off delay).
    pub gate_length_ms: u64,
    /// Base MIDI channel (0-15). Site i goes to base_channel + i.
    pub base_channel: u8,
}

impl Default for MidiConfig {
    fn default() -> Self {
        Self {
            pitch: 48, // C3
            gate_length_ms: 50,
            base_channel: 0,
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
    /// Seed for the substrate's RNG (initial spins, fields, couplings, thermal noise).
    pub seed: u64,
}
