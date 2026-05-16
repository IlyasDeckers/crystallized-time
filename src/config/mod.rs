//! Configuration for the substrate, output, and OSC layers.

mod clock;
pub mod config_file;
mod coupling;
mod events;
mod midi;
mod osc;
mod physics;
mod tempo;
mod walls;
mod input;

// use coupling::CouplingState;
// use pipeline::ChainPipeline;

pub use clock::ClockConfig;
pub use coupling::{CouplingConfig, CouplingShape};
pub use events::EventConfig;
pub use input::{InputConfig, PerturbationConfig, PerturbationKindConfig};
pub use midi::MidiConfig;
pub use osc::OscConfig;
pub use physics::{
    apply_smoothing, apply_smoothing_to_f64,
    PhysicsConfig, PhysicsTargets, SmoothingAlphas, SmoothingConfig,
};
pub use tempo::TempoConfig;
pub use walls::{WallConfig, WallMidiConfig};

#[derive(Clone, Debug, Default)]
pub struct Config {
    pub chain_a: ChainConfig,
    pub chain_b: Option<ChainConfig>,
    pub coupling: Option<CouplingConfig>,
    pub tempo: TempoConfig,
    pub osc: OscConfig,
    pub input: Option<InputConfig>,
}

#[derive(Clone, Debug, Default)]
pub struct ChainConfig {
    pub physics: PhysicsConfig,
    pub events: EventConfig,
    pub midi: MidiConfig,
    pub clock: ClockConfig,
    pub walls: WallConfig,
    pub wall_midi: WallMidiConfig,
    pub seed: u64,
}