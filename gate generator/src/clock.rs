//! Substrate-derived MIDI clock.
//!
//! Watches the chain's global magnetization <M> for zero-crossings; emits
//! a clock pulse (note-on with scheduled note-off) on a dedicated channel
//! every crossing. In the time-crystal phase this fires twice per crystal
//! period; outside the phase the clock degrades or stops, which is desired
//! behavior — the clock dying is a real signature of leaving the phase.

use crate::chain::SpinChain;
use crate::config::ClockConfig;
use crate::midi::MidiSender;

pub struct ClockEmitter {
    config: ClockConfig,
    /// Previous <M> value, for sign-change detection.
    prev_m: f64,
    /// Tick of the last emitted pulse (for debouncing).
    last_pulse_tick: u64,
}

impl ClockEmitter {
    pub fn new(config: ClockConfig, chain: &SpinChain) -> Self {
        Self {
            prev_m: chain.global_magnetization(),
            last_pulse_tick: 0,
            config,
        }
    }

    /// Inspect the chain's current <M>; emit a clock pulse if it crossed zero.
    pub fn tick(&mut self, chain: &SpinChain, sender: &MidiSender) {
        if !self.config.enabled {
            return;
        }

        let current_m = chain.global_magnetization();
        let threshold = self.config.crossing_threshold;

        // Sign change with a tiny minimum magnitude on the new side, so that
        // values jittering at exactly zero don't fire spuriously. Threshold
        // applies only to the post-crossing value.
        let sign_changed = self.prev_m.signum() != current_m.signum()
            && self.prev_m != 0.0;
        let above_floor = current_m.abs() > threshold;
        let crossed = sign_changed && above_floor;

        // let crossed_up = self.prev_m < -threshold && current_m > threshold;
        // let crossed_down = self.prev_m > threshold && current_m < -threshold;
        // let crossed = crossed_up || crossed_down;

        let since_last = chain.tick.saturating_sub(self.last_pulse_tick);
        let debounced = since_last > self.config.debounce_ticks;

        if crossed && debounced {
            sender.send_clock_pulse(
                self.config.channel,
                self.config.pitch,
                self.config.gate_length_ms,
            );
            self.last_pulse_tick = chain.tick;
        }

        self.prev_m = current_m;
    }
}