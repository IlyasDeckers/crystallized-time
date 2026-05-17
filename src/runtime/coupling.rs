//! Runtime state for inter-chain coupling.
//!
//! Owns OSC-tunable strengths (with smoothing), the shape selection,
//! and the per-tick injection that writes coupling fields into each
//! chain. Snapshot semantics: both chains' state is captured *before*
//! either chain steps, so coupling is symmetric in time.
//!
//! See src/config/coupling.rs for the TOML schema and Stage 3
//! reference doc for the musical motivation.

use crate::runtime::pipeline::ChainPipeline;
use crystallized_time::chain_id::ChainId;
use crystallized_time::config::{
    apply_smoothing_to_f64, CouplingConfig, CouplingShape, SmoothingAlphas,
};
use std::sync::{Arc, RwLock};

/// Per-tick snapshot of the state coupling needs. Built before any
/// chain steps; consumed after. Keeps coupling symmetric in time:
/// chain A sees chain B's *previous-tick* state, and vice versa.
pub struct CouplingSnapshot {
    pub magnetization_a: f64,
    pub magnetization_b: Option<f64>,
}

/// Mutable coupling strengths that OSC writes target.
/// Separate from the runtime's smoothed `current_*` values, mirroring
/// the `PhysicsTargets`/`PhysicsConfig` split used for physics.
#[derive(Clone, Debug)]
pub struct CouplingTargets {
    pub strength_ab: f64,
    pub strength_ba: f64,
}

impl CouplingTargets {
    pub fn from_config(c: &CouplingConfig) -> Self {
        Self {
            strength_ab: c.strength_ab,
            strength_ba: c.strength_ba,
        }
    }

    pub fn clamp_strength(v: f64) -> f64 {
        v.clamp(0.0, 2.0)
    }
}

/// Runtime coupling state owned by the `Runtime`. Holds the shape, the
/// shared targets handle (so OSC and the sim agree on what's being
/// written), and the smoothed live values.
pub struct CouplingState {
    pub shape: CouplingShape,
    pub targets: Arc<RwLock<CouplingTargets>>,
    /// Currently-active smoothed strengths. Updated each tick by
    /// `advance_smoothing`. The sim reads these (never the targets
    /// directly) so OSC writes don't produce audio steps.
    current_ab: f64,
    current_ba: f64,
    /// Whether we've warned about an unimplemented shape. Prevents
    /// log spam in the realtime loop.
    warned_unimplemented: bool,
}

#[allow(dead_code)]
impl CouplingState {
    pub fn new(config: &CouplingConfig) -> Self {
        Self::new_with_targets(
            config,
            Arc::new(RwLock::new(CouplingTargets::from_config(config))),
        )
    }

    pub fn new_with_targets(
        config: &CouplingConfig,
        targets: Arc<RwLock<CouplingTargets>>,
    ) -> Self {
        Self {
            shape: config.shape,
            targets,
            current_ab: config.strength_ab,
            current_ba: config.strength_ba,
            warned_unimplemented: false,
        }
    }

    /// Read-side accessors. The injector pulls these once per tick.
    pub fn current_ab(&self) -> f64 { self.current_ab }
    pub fn current_ba(&self) -> f64 { self.current_ba }

    /// Advance the smoothed strengths toward their targets by one tick's
    /// worth of exponential decay. Uses the kt alpha as a stand-in for
    /// "global parameter smoothing time" — coupling doesn't need its own
    /// time constant for now, and reusing kt's tau (1.5s) gives a smooth
    /// musical response without an OSC-jump click.
    pub fn advance_smoothing(&mut self, alphas: &SmoothingAlphas) {
        let targets = match self.targets.read() {
            Ok(t) => t.clone(),
            Err(_) => {
                eprintln!("warning: coupling targets lock poisoned, skipping smoothing");
                return;
            }
        };
        self.current_ab = apply_smoothing_to_f64(self.current_ab, targets.strength_ab, alphas.kt);
        self.current_ba = apply_smoothing_to_f64(self.current_ba, targets.strength_ba, alphas.kt);
    }

    /// Capture the magnetizations from each pipeline. Pipelines is
    /// borrowed shared here because we only read state.
    pub fn snapshot(&self, pipelines: &[ChainPipeline]) -> CouplingSnapshot {
        let mut m_a = 0.0;
        let mut m_b = None;
        for p in pipelines {
            let m = p.chain.global_magnetization();
            match p.id {
                ChainId::A => m_a = m,
                ChainId::B => m_b = Some(m),
            }
        }
        CouplingSnapshot {
            magnetization_a: m_a,
            magnetization_b: m_b,
        }
    }

    /// Inject coupling-derived field deltas into both chains. Pipelines
    /// is borrowed mutably because we write to each chain's pending
    /// fields.
    pub fn inject(
        &mut self,
        snapshot: &CouplingSnapshot,
        pipelines: &mut [ChainPipeline],
    ) {
        // Coupling needs two chains. Silently no-op if only one is
        // active (chain B was misconfigured or absent at startup).
        if snapshot.magnetization_b.is_none() {
            return;
        }

        match self.shape {
            CouplingShape::MeanFieldZ => self.inject_mean_field_z(snapshot, pipelines),
            CouplingShape::SitePaired | CouplingShape::SharedDrive => {
                if !self.warned_unimplemented {
                    eprintln!(
                        "warning: coupling shape {:?} is not yet implemented; running with no coupling",
                        self.shape
                    );
                    self.warned_unimplemented = true;
                }
            }
        }
    }

    fn inject_mean_field_z(
        &self,
        snapshot: &CouplingSnapshot,
        pipelines: &mut [ChainPipeline],
    ) {
        let m_a = snapshot.magnetization_a;
        let m_b = snapshot.magnetization_b.expect("checked by caller");
        let g_ab = self.current_ab; // A -> B
        let g_ba = self.current_ba; // B -> A

        for p in pipelines.iter_mut() {
            // Which strength applies and whose magnetization is the source.
            let (g, source_m) = match p.id {
                ChainId::A => (g_ba, m_b), // chain A feels chain B
                ChainId::B => (g_ab, m_a), // chain B feels chain A
            };
            if g == 0.0 {
                continue;
            }
            let delta = [0.0, 0.0, g * source_m];
            for site in 0..p.chain.spins.len() {
                p.chain.add_pending_field_delta(site, delta);
            }
        }
    }

    pub fn targets_handle(&self) -> Arc<RwLock<CouplingTargets>> {
        Arc::clone(&self.targets)
    }
}