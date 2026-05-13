//! MIDI-to-perturbation routing.
//!
//! Translates incoming note-on messages into `(site, PerturbationKind)`
//! pairs the chain can act on. The mapping is configured at construction
//! and doesn't change at runtime — to remap, edit the config file and
//! restart.
//!
//! This module is pure: it has no state beyond the immutable config, and
//! `route()` is a function of (message, n_sites) and the stored config.
//! Easy to test, easy to reason about.

use crate::chain::{Axis, PerturbationKind};
use crate::config::{PerturbationConfig, PerturbationKindConfig};
use crate::input::RawMidiMessage;

pub struct PerturbationRouter {
    config: PerturbationConfig,
}

impl PerturbationRouter {
    pub fn new(config: PerturbationConfig) -> Self {
        Self { config }
    }

    /// Try to translate one MIDI message into a chain perturbation.
    /// Returns None if the message isn't a perturbation trigger (wrong
    /// status, zero velocity, etc.).
    ///
    /// `n_sites` comes from the runtime — passed in rather than stored
    /// so a future runtime-resizable chain (which we don't have) would
    /// not need a router-rebuild.
    pub fn route(
        &self,
        msg: RawMidiMessage,
        n_sites: usize,
    ) -> Option<(usize, PerturbationKind)> {
        if n_sites == 0 {
            return None;
        }

        // Note-on on any channel. Note-off (0x80) and CC (0xB0) are dropped.
        if msg.status & 0xF0 != 0x90 {
            return None;
        }

        let note = msg.data1;
        let velocity = msg.data2;

        // Velocity-0 note-on is "note-off" in MIDI convention. Drop it.
        if velocity == 0 {
            return None;
        }

        // Site mapping: note relative to base_note, modulo n_sites.
        // Using i32 + rem_euclid handles notes below base_note cleanly:
        // (note=58, base=60, n=8) -> -2 -> 6, the second-from-last site.
        let offset = note as i32 - self.config.base_note as i32;
        let site = offset.rem_euclid(n_sites as i32) as usize;

        // Normalized velocity in [0, 1] for magnitude scaling.
        let v_norm = velocity as f64 / 127.0;
        let scale = v_norm * self.config.velocity_scale;

        let kind = match &self.config.kind {
            PerturbationKindConfig::Flip => {
                // Flip is binary: any positive velocity triggers it. Scale
                // is ignored — there's no "half a flip".
                PerturbationKind::Flip
            }
            PerturbationKindConfig::Rotate { axis, base_angle } => {
                PerturbationKind::Rotate {
                    axis: *axis,
                    angle: base_angle * scale,
                }
            }
            PerturbationKindConfig::FieldSpike { axis, base_magnitude } => {
                let m = base_magnitude * scale;
                let delta = match axis {
                    Axis::X => [m, 0.0, 0.0],
                    Axis::Y => [0.0, m, 0.0],
                    Axis::Z => [0.0, 0.0, m],
                };
                PerturbationKind::FieldSpike { delta }
            }
        };

        Some((site, kind))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(status: u8, data1: u8, data2: u8) -> RawMidiMessage {
        RawMidiMessage { status, data1, data2 }
    }

    fn rotate_router() -> PerturbationRouter {
        PerturbationRouter::new(PerturbationConfig {
            base_note: 60,
            kind: PerturbationKindConfig::Rotate {
                axis: Axis::X,
                base_angle: 1.0,
            },
            velocity_scale: 1.0,
        })
    }

    #[test]
    fn note_on_at_base_note_targets_site_zero() {
        let r = rotate_router();
        let (site, _) = r.route(raw(0x90, 60, 100), 8).unwrap();
        assert_eq!(site, 0);
    }

    #[test]
    fn higher_notes_walk_up_the_chain() {
        let r = rotate_router();
        assert_eq!(r.route(raw(0x90, 61, 100), 8).unwrap().0, 1);
        assert_eq!(r.route(raw(0x90, 67, 100), 8).unwrap().0, 7);
    }

    #[test]
    fn note_above_chain_wraps() {
        let r = rotate_router();
        // 68 - 60 = 8, mod 8 = 0
        assert_eq!(r.route(raw(0x90, 68, 100), 8).unwrap().0, 0);
    }

    #[test]
    fn note_below_base_wraps_with_rem_euclid() {
        let r = rotate_router();
        // 58 - 60 = -2, rem_euclid 8 = 6
        assert_eq!(r.route(raw(0x90, 58, 100), 8).unwrap().0, 6);
    }

    #[test]
    fn note_off_is_dropped() {
        let r = rotate_router();
        assert!(r.route(raw(0x80, 60, 100), 8).is_none());
    }

    #[test]
    fn velocity_zero_note_on_is_dropped() {
        let r = rotate_router();
        assert!(r.route(raw(0x90, 60, 0), 8).is_none());
    }

    #[test]
    fn control_change_is_dropped() {
        let r = rotate_router();
        assert!(r.route(raw(0xB0, 1, 64), 8).is_none());
    }

    #[test]
    fn rotate_angle_scales_with_velocity() {
        let r = rotate_router();
        let (_, kind_full) = r.route(raw(0x90, 60, 127), 8).unwrap();
        let (_, kind_half) = r.route(raw(0x90, 60, 64), 8).unwrap();

        let PerturbationKind::Rotate { angle: full, .. } = kind_full else {
            panic!("expected rotate");
        };
        let PerturbationKind::Rotate { angle: half, .. } = kind_half else {
            panic!("expected rotate");
        };

        // base_angle = 1.0, so full ≈ 1.0 and half ≈ 0.504 (64/127).
        assert!((full - 1.0).abs() < 0.01);
        assert!((half - 64.0 / 127.0).abs() < 0.01);
    }

    #[test]
    fn flip_ignores_velocity_magnitude() {
        let r = PerturbationRouter::new(PerturbationConfig {
            base_note: 60,
            kind: PerturbationKindConfig::Flip,
            velocity_scale: 1.0,
        });
        let (_, kind) = r.route(raw(0x90, 60, 1), 8).unwrap();
        assert!(matches!(kind, PerturbationKind::Flip));
    }

    #[test]
    fn field_spike_uses_correct_axis() {
        let r = PerturbationRouter::new(PerturbationConfig {
            base_note: 60,
            kind: PerturbationKindConfig::FieldSpike {
                axis: Axis::Z,
                base_magnitude: 2.0,
            },
            velocity_scale: 1.0,
        });
        let (_, kind) = r.route(raw(0x90, 60, 127), 8).unwrap();
        let PerturbationKind::FieldSpike { delta } = kind else {
            panic!("expected spike");
        };
        assert_eq!(delta[0], 0.0);
        assert_eq!(delta[1], 0.0);
        assert!((delta[2] - 2.0).abs() < 0.02);
    }
}