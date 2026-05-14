//! Zero-crossing detection on sigma_z, producing GateEvents.
//!
//! Stateful: tracks previous sz values and last-event ticks per output site
//! so it can debounce and detect signed crossings rather than raw threshold passes.

use crystallized_time::config::MidiConfig;
use crate::chain::SpinChain;
use crate::config::EventConfig;

/// A single gate-trigger event emitted by the substrate.
#[derive(Clone, Copy, Debug)]
pub struct GateEvent {
    /// Index of the site that fired (into the chain, not into output_sites).
    pub site: usize,
    /// Position of this site in output_sites — the "voice index".
    /// Determines the MIDI channel offset.
    pub voice: u8,
    /// Tick at which the event occurred.
    pub tick: u64,
    /// Strength of the crossing, in [0, 1].
    pub intensity: f32,

    pub channel: u8,
    pub pitch: u8,
}

/// Watches a chain and emits GateEvents on zero-crossings of sigma_z.
pub struct EventDetector {
    pub config: EventConfig,
    pub midi_config: MidiConfig,

    /// prev_sz[k] is the previous z-component for output_sites[k].
    prev_sz: Vec<f64>,
    /// last_event_tick[k] is the tick of the last emission for output_sites[k].
    last_event_tick: Vec<u64>,
}

impl EventDetector {
    pub fn new(config: EventConfig, midi_config: MidiConfig, chain: &SpinChain) -> Self {
        let prev_sz: Vec<f64> = config
            .output_sites
            .iter()
            .map(|&i| chain.sz(i))
            .collect();
        let last_event_tick: Vec<u64> = vec![0; config.output_sites.len()];
        Self {
            config,
            midi_config,
            prev_sz,
            last_event_tick,
        }
    }

    pub fn check(&mut self, chain: &SpinChain) -> Vec<GateEvent> {
        let mut events = Vec::new();
        for (k, &site) in self.config.output_sites.iter().enumerate() {
            let current_sz = chain.sz(site);
            let prev = self.prev_sz[k];
            let threshold = self.config.crossing_threshold;

            let crossed_up = prev < -threshold && current_sz > threshold;
            let crossed_down = prev > threshold && current_sz < -threshold;
            let crossed = crossed_up || crossed_down;

            let since_last = chain.tick.saturating_sub(self.last_event_tick[k]);
            let debounced = since_last > self.config.debounce_ticks;

            if crossed && debounced {
                let intensity = ((current_sz - prev).abs() as f32).min(1.0);
                let channel = self.midi_config.voice_channels.get(k).copied().unwrap_or(0);
                let pitch = self.midi_config.voice_pitches.get(k).copied().unwrap_or(48);
                events.push(GateEvent {
                    site,
                    voice: k as u8,
                    tick: chain.tick,
                    intensity,
                    channel,
                    pitch,
                });
                self.last_event_tick[k] = chain.tick;
            }

            self.prev_sz[k] = current_sz;
        }
        events
    }
}
