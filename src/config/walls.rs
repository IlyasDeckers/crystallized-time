//! Domain-wall detection parameters (the physics-side config) and
//! domain-wall MIDI output parameters (the routing-side config).
//!
//! The two live together because they exist only because of each
//! other and are always used as a pair — wall physics has no purpose
//! without wall MIDI, and wall MIDI has no input without wall physics.

/// Domain-wall detection parameters.
#[derive(Clone, Debug)]
pub struct WallConfig {
    /// Whether wall detection is active.
    pub enabled: bool,
    /// Greedy match radius in position units.
    pub match_radius: f64,
    /// Minimum position change to fire a Moved event.
    pub move_threshold: f64,
    /// Interpolate wall position from sz magnitudes.
    pub interpolate_position: bool,
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