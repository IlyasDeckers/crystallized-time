//! One chain and its per-chain plumbing: detectors, emitters,
//! physics targets, RNG, optional MIDI input. Runtime owns a Vec
//! of these; coupling and multi-chain orchestration live in Runtime.

use crate::clock::ClockEmitter;
use crate::events::EventDetector;
use crate::input::MidiInputListener;
use crate::midi::MidiSender;
use crate::osc::OutboundEvent;
use crate::osc_io::OscSink;
use crate::perturbation::PerturbationRouter;
use crate::wall_midi::WallVoiceAllocator;
use crate::walls::WallDetector;
use arc_swap::ArcSwap;
use crystallized_time::chain::SpinChain;
use crystallized_time::chain_id::ChainId;
use crystallized_time::config::{
    apply_smoothing, PhysicsConfig, PhysicsTargets, SmoothingAlphas,
};
use rand::rngs::StdRng;
use std::sync::{Arc, RwLock};

pub struct ChainPipeline {
    pub id: ChainId,
    pub chain: SpinChain,
    pub physics_arc: Arc<ArcSwap<PhysicsConfig>>,
    pub targets: Arc<RwLock<PhysicsTargets>>,

    detector: EventDetector,
    clock_emitter: ClockEmitter,
    wall_detector: WallDetector,
    wall_voicer: WallVoiceAllocator,
    rng: StdRng,

    input_listener: Option<MidiInputListener>,
    perturbation_router: Option<PerturbationRouter>,

    n_sites: usize,
}

impl ChainPipeline {
    pub fn new(
        id: ChainId,
        physics: PhysicsConfig,
        events_cfg: crystallized_time::config::EventConfig,
        midi_cfg: crystallized_time::config::MidiConfig,
        clock_cfg: crystallized_time::config::ClockConfig,
        walls_cfg: crystallized_time::config::WallConfig,
        wall_midi_cfg: crystallized_time::config::WallMidiConfig,
        seed: u64,
        targets: Arc<RwLock<PhysicsTargets>>,
        input_listener: Option<MidiInputListener>,
        perturbation_router: Option<PerturbationRouter>,
    ) -> Self {
        use rand::SeedableRng;

        let mut rng = StdRng::seed_from_u64(seed);
        let physics_arc = Arc::new(ArcSwap::from_pointee(physics.clone()));
        let chain = SpinChain::new(Arc::clone(&physics_arc), &mut rng);

        let detector = EventDetector::new(events_cfg, midi_cfg, &chain);
        let clock_emitter = ClockEmitter::new(clock_cfg, &chain, id);
        let wall_detector = WallDetector::new(walls_cfg);
        let wall_voicer = WallVoiceAllocator::new(wall_midi_cfg, &physics, id);

        Self {
            id,
            chain,
            physics_arc,
            targets,
            detector,
            clock_emitter,
            wall_detector,
            wall_voicer,
            rng,
            input_listener,
            perturbation_router,
            n_sites: physics.n_sites,
        }
    }

    pub fn advance_smoothing(&self, alphas: &SmoothingAlphas) {
        let current = self.physics_arc.load();
        let targets_snapshot = match self.targets.read() {
            Ok(t) => t.clone(),
            Err(_) => {
                eprintln!(
                    "warning: {} physics targets lock poisoned, skipping smoothing",
                    self.id.label()
                );
                return;
            }
        };
        if let Some(new_cfg) = apply_smoothing(&current, &targets_snapshot, alphas) {
            drop(current);
            self.physics_arc.store(Arc::new(new_cfg));
        }
    }

    pub fn step_physics(&mut self) {
        self.chain.step(&mut self.rng);
    }

    pub fn apply_input_perturbations(&mut self) {
        let (Some(listener), Some(router)) = (
            self.input_listener.as_ref(),
            self.perturbation_router.as_ref(),
        ) else {
            return;
        };

        for msg in listener.poll() {
            if let Some((site, kind)) = router.route(msg, self.n_sites) {
                self.chain.perturb(site, kind);
            }
        }
    }

    pub fn emit_site_events(
        &mut self,
        midi: &MidiSender,
        mut osc: Option<&mut OscSink>,
    ) {
        let events = self.detector.check(&self.chain);
        for event in events {
            midi.send_gate(event);
            if let Some(sink) = osc.as_deref_mut() {
                sink.push(OutboundEvent::SiteEvent {
                    chain: self.id,
                    site: event.site as u8,
                    voice: event.voice,
                    intensity: event.intensity,
                });
            }
        }
    }

    pub fn tick_clock(
        &mut self,
        midi: &MidiSender,
        osc: Option<&mut OscSink>,
    ) {
        self.clock_emitter.tick(&self.chain, midi, osc);
    }

    pub fn process_walls(
        &mut self,
        midi: &MidiSender,
        mut osc: Option<&mut OscSink>,
    ) {
        let wall_events = self.wall_detector.check(&self.chain);
        for event in &wall_events {
            self.wall_voicer.handle(event, midi, osc.as_deref_mut());
        }
    }

    pub fn push_state(&self, sink: &mut OscSink) {
        let spins: Vec<f64> = self.chain.spins.iter().map(|s| s[2]).collect();
        sink.push_state_if_due(
            self.id,
            &spins,
            self.chain.global_magnetization(),
            self.wall_detector.wall_count(),
        );
    }
}