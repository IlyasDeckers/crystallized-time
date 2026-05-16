//! Inter-chain coupling configuration.
//!
//! When the [coupling] TOML section is absent, no coupling runs and
//! both chains evolve independently. When present, the runtime
//! applies the configured shape on every tick, with strengths
//! tunable via OSC.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CouplingShape {
    /// Each chain feels the other's mean magnetization as a uniform
    /// z-field on every site. Simplest symmetric coupling. The
    /// starting point recommended by the Stage 3 reference doc.
    MeanFieldZ,
    /// Site i of chain A coupled to site i of chain B via a z-z term.
    /// Stronger and more local than mean-field. Requires both chains
    /// to have the same length, or a rule for handling unequal lengths.
    /// Stub: parses but runtime doesn't implement yet.
    SitePaired,
    /// Each chain's state modulates the other's drive epsilon. Affects
    /// lock strength, not instantaneous state. Stub: parses but runtime
    /// doesn't implement yet.
    SharedDrive,
}

#[derive(Clone, Debug)]
pub struct CouplingConfig {
    pub shape: CouplingShape,
    /// Chain A's influence on chain B. Range 0.0..=2.0 (clamped on
    /// OSC writes).
    pub strength_ab: f64,
    /// Chain B's influence on chain A. Same range.
    pub strength_ba: f64,
}

impl Default for CouplingConfig {
    fn default() -> Self {
        Self {
            shape: CouplingShape::MeanFieldZ,
            strength_ab: 0.0,
            strength_ba: 0.0,
        }
    }
}