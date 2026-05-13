use crate::chain::SpinChain;
use crate::clock::ClockEmitter;
use crate::config::{
    apply_smoothing, Config, PhysicsConfig, PhysicsTargets, SmoothingAlphas, SmoothingConfig,
};
use crate::events::EventDetector;
use crate::midi::MidiSender;
use crate::osc::OutboundEvent;
use crate::osc_io::OscSink;
use crate::wall_midi::WallVoiceAllocator;
use crate::walls::WallDetector;
use arc_swap::ArcSwap;
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use crate::input::MidiInputListener;
use crate::perturbation::PerturbationRouter;

pub struct Runtime {
    chain: SpinChain,
    detector: EventDetector,
    clock_emitter: ClockEmitter,
    wall_detector: WallDetector,
    wall_voicer: WallVoiceAllocator,
    midi_sender: MidiSender,
    osc_sink: Option<OscSink>,
    input_listener: Option<MidiInputListener>,
    perturbation_router: Option<PerturbationRouter>,

    physics_arc: Arc<ArcSwap<PhysicsConfig>>,
    targets: Arc<RwLock<PhysicsTargets>>,
    alphas: SmoothingAlphas,
    rng: StdRng,

    /// Cached for use by run_until's pacing loop.
    tick_duration: Duration,
    gate_length_ms: u64,
    n_sites: usize,

    debug: bool,
}

impl Runtime {
    /// Assemble the pipeline. Takes ownership of the MIDI sender and the
    /// optional OSC sink; both were built upstream because they may fail
    /// in ways main.rs reports differently.
    pub fn build(
        config: &Config,
        midi_sender: MidiSender,
        osc_sink: Option<OscSink>,
        targets: Arc<RwLock<PhysicsTargets>>,
        input_listener: Option<MidiInputListener>,
        perturbation_router: Option<PerturbationRouter>,
    ) -> Self {
        let mut rng = StdRng::seed_from_u64(config.seed);

        let physics_arc = Arc::new(ArcSwap::from_pointee(config.physics.clone()));
        let chain = SpinChain::new(Arc::clone(&physics_arc), &mut rng);

        let detector = EventDetector::new(config.events.clone(), &chain);
        let clock_emitter = ClockEmitter::new(config.clock.clone(), &chain);
        let wall_detector = WallDetector::new(config.walls.clone());
        let wall_voicer = WallVoiceAllocator::new(config.wall_midi.clone(), &config.physics);

        let dt_real_secs =
            config.tempo.drive_period_secs / config.physics.ticks_per_period as f64;
        let alphas = SmoothingAlphas::from_config(&SmoothingConfig::default(), dt_real_secs);

        let tick_duration = Duration::from_secs_f64(dt_real_secs);

        Self {
            chain,
            detector,
            clock_emitter,
            wall_detector,
            wall_voicer,
            midi_sender,
            osc_sink,
            input_listener,
            perturbation_router,
            physics_arc,
            targets,
            alphas,
            rng,
            tick_duration,
            gate_length_ms: config.midi.gate_length_ms,
            n_sites: config.physics.n_sites,
            debug: false
        }
    }

    /// Run for `total_ticks`, paced to wall clock, exiting early if
    /// `running` flips to false. Returns once the loop ends.
    pub fn run_until(
        &mut self,
        total_ticks: u64,
        running: &std::sync::atomic::AtomicBool,
    ) {
        use std::sync::atomic::Ordering;

        let start = Instant::now();
        for tick in 1..=total_ticks {
            if !running.load(Ordering::Acquire) {
                break;
            }

            self.step(tick);

            let target = start + self.tick_duration * tick as u32;
            let now = Instant::now();
            if target > now {
                std::thread::sleep(target - now);
            }
        }
    }

    /// One simulation tick. Order matters: smooth params, step physics,
    /// detect events, run the clock, run wall detection, flush OSC.
    fn step(&mut self, tick: u64) {
        self.advance_smoothing();
        self.chain.step(&mut self.rng);
        self.apply_input_perturbations();
        self.emit_site_events();
        self.clock_emitter
            .tick(&self.chain, &self.midi_sender, self.osc_sink.as_mut());
        self.process_walls();
        self.flush_osc();
    }

    /// Move the live physics snapshot toward the OSC targets by one tick's
    /// worth of exponential smoothing. No-op when everything is at target.
    fn advance_smoothing(&self) {
        let current = self.physics_arc.load();
        let targets_snapshot = match self.targets.read() {
            Ok(t) => t.clone(),
            Err(_) => {
                eprintln!("warning: physics targets lock poisoned, skipping smoothing");
                return;
            }
        };
        if let Some(new_cfg) = apply_smoothing(&current, &targets_snapshot, &self.alphas) {
            drop(current);
            self.physics_arc.store(Arc::new(new_cfg));
        }
    }

    fn emit_site_events(&mut self) {
        let events = self.detector.check(&self.chain);
        for event in events {
            self.midi_sender.send_gate(event);
            if let Some(sink) = self.osc_sink.as_mut() {
                sink.push(OutboundEvent::SiteEvent {
                    site: event.site as u8,
                    voice: event.voice,
                    intensity: event.intensity,
                });
            }
        }
    }

    fn process_walls(&mut self) {
        let wall_events = self.wall_detector.check(&self.chain);
        for event in &wall_events {
            self.wall_voicer
                .handle(event, &self.midi_sender, self.osc_sink.as_mut());
        }
    }

    fn flush_osc(&mut self) {
        let Some(sink) = self.osc_sink.as_mut() else { return };
        let spins: Vec<f64> = self.chain.spins.iter().map(|s| s[2]).collect();
        sink.push_state_if_due(
            &spins,
            self.chain.global_magnetization(),
            self.wall_detector.wall_count(),
        );
        sink.flush_tick();
    }

    /// Drain any pending MIDI input messages and apply the resulting
    /// perturbations to the chain. No-op if no input is configured or no
    /// messages arrived this tick.
    fn apply_input_perturbations(&mut self) {
        let (Some(listener), Some(router)) =
            (self.input_listener.as_ref(), self.perturbation_router.as_ref())
        else {
            return;
        };

        for msg in listener.poll() {
            if let Some((site, kind)) = router.route(msg, self.n_sites) {
                self.chain.perturb(site, kind);
            }
        }
    }

    /// Clean shutdown: All Notes Off / All Sound Off on used channels.
    pub fn shutdown(self) {
        self.midi_sender.shutdown();
        std::thread::sleep(Duration::from_millis(self.gate_length_ms + 50));
    }
}