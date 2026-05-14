use crate::input::MidiInputListener;
use crate::midi::MidiSender;
use crate::osc_io::OscSink;
use crate::perturbation::PerturbationRouter;
use crystallized_time::chain_id::ChainId;
use crystallized_time::config::{
    Config, PhysicsTargets, SmoothingAlphas, SmoothingConfig,
};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

mod pipeline;
use pipeline::ChainPipeline;

pub struct Runtime {
    pipelines: Vec<ChainPipeline>,
    midi_sender: MidiSender,
    osc_sink: Option<OscSink>,
    alphas: SmoothingAlphas,
    tick_duration: Duration,
    gate_length_ms: u64,
}

impl Runtime {
    pub fn build(
        config: &Config,
        midi_sender: MidiSender,
        osc_sink: Option<OscSink>,
        targets_a: Arc<RwLock<PhysicsTargets>>,
        targets_b: Option<Arc<RwLock<PhysicsTargets>>>,
        input_listener: Option<MidiInputListener>,
        perturbation_router: Option<PerturbationRouter>,
    ) -> Self {
        let dt_real_secs =
            config.tempo.drive_period_secs / config.chain_a.physics.ticks_per_period as f64;
        let alphas = SmoothingAlphas::from_config(&SmoothingConfig::default(), dt_real_secs);
        let tick_duration = Duration::from_secs_f64(dt_real_secs);
        let mut pipelines = Vec::with_capacity(2);

        let chain_a = ChainPipeline::new(
            ChainId::A,
            config.chain_a.physics.clone(),
            config.chain_a.events.clone(),
            config.chain_a.clock.clone(),
            config.chain_a.walls.clone(),
            config.chain_a.wall_midi.clone(),
            config.chain_a.seed,
            targets_a,
            input_listener,
            perturbation_router,
        );
        pipelines.push(chain_a);

        if let Some(b_cfg) = &config.chain_b {
            let targets_b = targets_b.expect(
                "chain_b is configured but no PhysicsTargets was provided"
            );
            let chain_b = ChainPipeline::new(
                ChainId::B,
                b_cfg.physics.clone(),
                b_cfg.events.clone(),
                b_cfg.clock.clone(),
                b_cfg.walls.clone(),
                b_cfg.wall_midi.clone(),
                b_cfg.seed,
                targets_b,
                None,
                None,
            );
            pipelines.push(chain_b);
        }

        Self {
            pipelines,
            midi_sender,
            osc_sink,
            alphas,
            tick_duration,
            gate_length_ms: config.chain_a.midi.gate_length_ms,
        }
    }

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

    fn step(&mut self, _tick: u64) {
        // Phase 1: smoothing. Each pipeline independent.
        for p in &self.pipelines {
            p.advance_smoothing(&self.alphas);
        }

        // Phase 2: physics. (Eventually: snapshot all <M>, then step each
        // chain using the snapshot as coupling input. For now: just step.)
        for p in &mut self.pipelines {
            p.step_physics();
        }

        // Phase 3: input. Each pipeline drains its own MIDI input.
        for p in &mut self.pipelines {
            p.apply_input_perturbations();
        }

        // Phase 4: emit. Site events, clock, walls. Per-pipeline.
        for p in &mut self.pipelines {
            p.emit_site_events(&self.midi_sender, self.osc_sink.as_mut());
            p.tick_clock(&self.midi_sender, self.osc_sink.as_mut());
            p.process_walls(&self.midi_sender, self.osc_sink.as_mut());
        }

        // Phase 5: state push + flush. One bundle per tick, shared across chains.
        if let Some(sink) = self.osc_sink.as_mut() {
            for p in &self.pipelines {
                p.push_state(sink);
            }
            sink.flush_tick();
        }
    }

    pub fn shutdown(self) {
        self.midi_sender.shutdown();
        std::thread::sleep(Duration::from_millis(self.gate_length_ms + 50));
    }
}