//! Configuration for the substrate, output, and OSC layers.
//!
//! Each sub-module owns one cohesive group of configuration types
//! (physics, events, MIDI output, tempo, clock, walls, OSC). The
//! top-level `Config` here is the assembled bundle the program uses.
//!
//! `config_file` is the TOML deserialization front-end — it produces
//! the same `Config` that the rest of the program already knows how
//! to consume. See its module docs for the file schema.
//!
//! All public types are re-exported, so other modules import as
//! `use crate::config::{Config, PhysicsConfig, ...};` regardless of
//! which sub-module each type lives in.

mod clock;
pub mod config_file;
mod events;
mod midi;
mod osc;
mod physics;
mod tempo;
mod walls;
mod input;

pub use clock::ClockConfig;
pub use events::EventConfig;
pub use input::{InputConfig, PerturbationConfig, PerturbationKindConfig};
pub use midi::MidiConfig;
pub use osc::OscConfig;
pub use physics::{
    apply_smoothing, PhysicsConfig, PhysicsTargets, SmoothingAlphas, SmoothingConfig,
};
pub use tempo::TempoConfig;
pub use walls::{WallConfig, WallMidiConfig};

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
    pub input: Option<InputConfig>,
    pub seed: u64,
}