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

/// Domain-wall detection parameters.
#[derive(Clone, Debug)]
pub struct WallConfig {
    /// Whether wall detection is active.
    pub enabled: bool,
    /// Greedy match radius in position units (Step 3 uses this).
    pub match_radius: f64,
    /// Minimum position change to fire a Moved event (Step 3 uses this).
    pub move_threshold: f64,
    /// Interpolate wall position from sz magnitudes (Step 4 uses this).
    pub interpolate_position: bool,
}

/// Domain-wall MIDI output parameters.
#[derive(Clone, Debug)]
pub struct WallMidiConfig {
    pub channel_low: u8,
    pub channel_high: u8,
    pub pitch_low: u8,
    pub pitch_high: u8,
    pub motion_cc: Option<u8>,
    /// When true, wall motion produces new note-on/note-off pairs as the
    /// pitch changes (gate-and-CV-friendly). When false, pitch is set at
    /// note-on and held; motion is sent via `motion_cc`.
    pub repitch_on_move: bool,
}

impl Default for WallMidiConfig {
    fn default() -> Self {
        Self {
            channel_low: 4,
            channel_high: 7,
            pitch_low: 36,
            pitch_high: 60,
            motion_cc: Some(1),
            repitch_on_move: false,
        }
    }
}

impl Default for WallConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            match_radius: 1.0,
            move_threshold: 0.1,
            interpolate_position: true,
        }
    }
}

/// Mutable target values for the four live-tunable physics parameters.
/// Writers (the OSC receiver thread) update these via `RwLock::write()`;
/// the simulation thread reads them once per tick via `RwLock::read()`.
///
/// Values are clamped to per-parameter bounds on write (see `clamp_kt`
/// etc.). Reads always return in-bounds values.
#[derive(Clone, Debug)]
pub struct PhysicsTargets {
    pub kt: f64,
    pub eps: f64,
    pub j: f64,
    pub w: f64,
}

impl PhysicsTargets {
    /// Build targets that match an initial `PhysicsConfig` exactly, so the
    /// chain starts at the configured values and doesn't smooth toward
    /// anything until an external writer changes a target.
    pub fn from_physics(config: &PhysicsConfig) -> Self {
        Self {
            kt: config.kt,
            eps: config.eps,
            j: config.j,
            w: config.w,
        }
    }

    pub fn clamp_kt(v: f64) -> f64  { v.clamp(0.0, 2.0) }
    pub fn clamp_eps(v: f64) -> f64 { v.clamp(0.0, 0.5) }
    pub fn clamp_j(v: f64) -> f64   { v.clamp(0.0, 3.0) }
    pub fn clamp_w(v: f64) -> f64   { v.clamp(0.0, 5.0) }
}

/// Per-parameter smoothing time constants, in seconds. `tau` is the
/// time it takes to cover ~63% of the remaining gap to the target.
/// After `3 * tau` seconds the value is essentially at target.
#[derive(Clone, Debug)]
pub struct SmoothingConfig {
    pub kt_tau_secs: f64,
    pub eps_tau_secs: f64,
    pub j_tau_secs: f64,
    pub w_tau_secs: f64,
}

impl Default for SmoothingConfig {
    fn default() -> Self {
        Self {
            kt_tau_secs: 1.5,
            eps_tau_secs: 1.0,
            j_tau_secs: 2.0,
            w_tau_secs: 2.0,
        }
    }
}

/// Pre-computed per-tick smoothing coefficients. Each `alpha` is
/// `1 - exp(-dt_real / tau)` for the corresponding parameter.
///
/// Computed once at startup from `SmoothingConfig` and the nominal tick
/// duration; doesn't change for the run. Per spec, smoothing uses the
/// nominal tick duration (drive_period / ticks_per_period), so the
/// smoothing rate is coupled to BPM — a higher BPM means parameters
/// reach their targets in fewer wall-clock seconds.
#[derive(Clone, Debug)]
pub struct SmoothingAlphas {
    pub kt: f64,
    pub eps: f64,
    pub j: f64,
    pub w: f64,
}

impl SmoothingAlphas {
    pub fn from_config(smoothing: &SmoothingConfig, dt_real_secs: f64) -> Self {
        // alpha = 1 - exp(-dt / tau). If tau is zero or negative, treat as
        // "no smoothing" (alpha = 1.0) so targets land instantly.
        let alpha = |tau: f64| -> f64 {
            if tau <= 0.0 { 1.0 } else { 1.0 - (-dt_real_secs / tau).exp() }
        };
        Self {
            kt: alpha(smoothing.kt_tau_secs),
            eps: alpha(smoothing.eps_tau_secs),
            j: alpha(smoothing.j_tau_secs),
            w: alpha(smoothing.w_tau_secs),
        }
    }
}

/// OSC-specific configuration. Listen port and send address are CLI-only
/// (they have no useful defaults); this struct holds only the knobs that
/// have non-trivial defaults worth keeping out of `main.rs`.
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

/// Compute the next physics snapshot by exponentially approaching the
/// targets. Returns `Some(new_config)` if any parameter moved by more
/// than `EPSILON`, `None` if all four are effectively at their targets.
///
/// Returning `None` lets the caller skip the ArcSwap store (and the
/// `Arc::new` allocation) on steady-state ticks — at rest with no OSC
/// traffic, this function returns `None` every tick and the loop does
/// zero work for parameter management.
pub fn apply_smoothing(
    current: &PhysicsConfig,
    targets: &PhysicsTargets,
    alphas: &SmoothingAlphas,
) -> Option<PhysicsConfig> {
    const EPSILON: f64 = 1e-9;

    let new_kt  = current.kt  + (targets.kt  - current.kt)  * alphas.kt;
    let new_eps = current.eps + (targets.eps - current.eps) * alphas.eps;
    let new_j   = current.j   + (targets.j   - current.j)   * alphas.j;
    let new_w   = current.w   + (targets.w   - current.w)   * alphas.w;

    let changed =
        (new_kt  - current.kt ).abs() > EPSILON ||
            (new_eps - current.eps).abs() > EPSILON ||
            (new_j   - current.j  ).abs() > EPSILON ||
            (new_w   - current.w  ).abs() > EPSILON;

    if !changed {
        return None;
    }

    let mut next = current.clone();
    next.kt  = new_kt;
    next.eps = new_eps;
    next.j   = new_j;
    next.w   = new_w;
    Some(next)
}

/// All configuration combined.
#[derive(Clone, Debug, Default)]
pub struct Config {
    pub physics: PhysicsConfig,
    pub events: EventConfig,
    pub midi: MidiConfig,
    pub tempo: TempoConfig,
    pub clock: ClockConfig,
    pub walls: WallConfig,
    pub wall_midi: WallMidiConfig,
    pub osc: OscConfig,
    pub seed: u64,
}
