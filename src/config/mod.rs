//! Configuration for the substrate, output, and OSC layers.
//!
//! Each sub-module owns one cohesive group of configuration types
//! (physics, events, MIDI output, tempo, clock, walls, OSC). The
//! top-level `Config` here is the assembled bundle the program uses.
//!
//! All public types are re-exported, so other modules import as
//! `use crate::config::{Config, PhysicsConfig, ...};` regardless of
//! which sub-module each type lives in.

mod clock;
mod events;
mod midi;
mod osc;
mod physics;
mod tempo;
mod walls;

pub use clock::ClockConfig;
pub use events::EventConfig;
pub use midi::MidiConfig;
pub use osc::OscConfig;
pub use physics::{
    apply_smoothing, PhysicsConfig, PhysicsTargets, SmoothingAlphas, SmoothingConfig,
};
pub use tempo::TempoConfig;
pub use walls::{WallConfig, WallMidiConfig};

/// Output topology — how voices are routed to MIDI channels.
///
/// Lives at the top level rather than in `midi.rs` because it's
/// referenced both by `MidiConfig` and by the CLI in `main.rs`.
/// Keeping it here avoids `config::midi::OutputMode` paths and
/// avoids a sibling import for a type the config tree owns.
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

/// All configuration combined. The shape `main.rs` builds and that
/// the simulation loop reads from.
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