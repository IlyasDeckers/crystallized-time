//! MIDI input and perturbation routing configuration.
//!
//! `InputConfig` describes the input port and the perturbation mapping.
//! When the `[input]` section is absent from the config file, no input
//! is opened and the chain runs in its existing autonomous mode.

use crate::chain::Axis;

#[derive(Clone, Debug)]
pub struct InputConfig {
    /// Perturbation behavior. Always present once the section is loaded;
    /// the spec's defaults apply when fields are omitted.
    pub perturbation: PerturbationConfig,
}

/// How an incoming MIDI note becomes a chain perturbation.
#[derive(Clone, Debug)]
pub struct PerturbationConfig {
    /// MIDI note number that maps to site 0. Other notes map to
    /// `site = (note - base_note).rem_euclid(n_sites)`.
    pub base_note: u8,
    /// What kind of perturbation an incoming note-on produces.
    pub kind: PerturbationKindConfig,
    /// Multiplier applied to the perturbation magnitude based on velocity.
    /// At velocity 127 the perturbation uses `base_magnitude` directly;
    /// at velocity 0 the magnitude is zero. Linear in between.
    pub velocity_scale: f64,
}

/// The three perturbation shapes the spec describes, in config form. Carries
/// the parameters each kind needs; converted at routing time into a
/// `chain::PerturbationKind` with the actual magnitude.
#[derive(Clone, Debug)]
pub enum PerturbationKindConfig {
    Flip,
    Rotate { axis: Axis, base_angle: f64 },
    FieldSpike { axis: Axis, base_magnitude: f64 },
}

impl Default for PerturbationConfig {
    fn default() -> Self {
        Self {
            base_note: 60, // middle C
            kind: PerturbationKindConfig::Rotate {
                axis: Axis::X,
                base_angle: 0.3,
            },
            velocity_scale: 1.0,
        }
    }
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            perturbation: PerturbationConfig::default(),
        }
    }
}