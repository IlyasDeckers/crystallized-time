//! Substrate-derived MIDI clock.
//!
//! Watches the chain's global magnetization <M> for zero-crossings; emits
//! a clock pulse (note-on with scheduled note-off) on a dedicated channel
//! every crossing. In the time-crystal phase this fires twice per crystal
//! period; outside the phase the clock degrades or stops, which is desired
//! behavior — the clock dying is a real signature of leaving the phase.

use crystallized_time::chain_id::ChainId;
use crate::chain::SpinChain;
use crate::config::ClockConfig;
use crate::midi::MidiSender;

pub struct ClockEmitter {
    config: ClockConfig,
    /// Previous <M> value, for sign-change detection.
    prev_m: f64,
    /// Tick of the last emitted pulse (for debouncing).
    last_pulse_tick: u64,

    chain_id: ChainId,
}

impl ClockEmitter {
    pub fn new(config: ClockConfig, chain: &SpinChain, chain_id: ChainId) -> Self {
        Self {
            chain_id,
            prev_m: chain.global_magnetization(),
            last_pulse_tick: 0,
            config,
        }
    }

    /// Inspect the chain's current <M>; emit a clock pulse if it crossed zero.
    /// If `osc_sink` is provided, also push a /clock/pulse event for every
    /// MIDI pulse fired.
    pub fn tick(
        &mut self,
        chain: &SpinChain,
        sender: &MidiSender,
        osc_sink: Option<&mut crate::osc_io::OscSink>,
    ) {
        if !self.config.enabled {
            self.prev_m = chain.global_magnetization();
            return;
        }

        let current_m = chain.global_magnetization();

        if self.should_pulse(current_m, chain.tick) {
            sender.send_clock_pulse(
                self.config.channel,
                self.config.pitch,
                self.config.gate_length_ms,
            );
            if let Some(sink) = osc_sink {
                sink.push(crate::osc::OutboundEvent::ClockPulse {
                    chain: self.chain_id,
                    magnetization: current_m,
                });
            }
            self.last_pulse_tick = chain.tick;
        }

        self.prev_m = current_m;
    }

    /// Pure decision: should this tick emit a pulse?
    ///
    /// Symmetric threshold band, matching `EventDetector`: <M> must travel
    /// from clearly negative (below -threshold) to clearly positive (above
    /// +threshold), or vice versa. Oscillations inside [-threshold, +threshold]
    /// — the noisy-near-zero region a thermalized chain produces — never fire.
    ///
    /// Factored out of `tick` so the logic can be unit-tested without a real
    /// `SpinChain` or `MidiSender`.
    fn should_pulse(&self, current_m: f64, tick: u64) -> bool {
        let threshold = self.config.crossing_threshold;

        let crossed_up = self.prev_m < -threshold && current_m > threshold;
        let crossed_down = self.prev_m > threshold && current_m < -threshold;
        let crossed = crossed_up || crossed_down;

        let since_last = tick.saturating_sub(self.last_pulse_tick);
        let debounced = since_last > self.config.debounce_ticks;

        crossed && debounced
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_emitter(threshold: f64, debounce: u64) -> ClockEmitter {
        // We bypass `new()` here because constructing a real `SpinChain`
        // would drag in physics config + RNG; all we need to exercise
        // `should_pulse` is the three fields it reads.
        ClockEmitter {
            config: ClockConfig {
                enabled: true,
                channel: 0,
                pitch: 48,
                crossing_threshold: threshold,
                debounce_ticks: debounce,
                gate_length_ms: 25,
            },
            prev_m: 0.0,
            last_pulse_tick: 0,
            chain_id: ChainId::A,
        }
    }

    #[test]
    fn fires_on_clean_negative_to_positive_crossing() {
        let mut e = make_emitter(0.05, 2);
        e.prev_m = -0.4;
        // Current well above +threshold, well past debounce.
        assert!(e.should_pulse(0.4, 100));
    }

    #[test]
    fn fires_on_clean_positive_to_negative_crossing() {
        let mut e = make_emitter(0.05, 2);
        e.prev_m = 0.4;
        assert!(e.should_pulse(-0.4, 100));
    }

    #[test]
    fn rejects_crossing_when_prev_is_inside_the_band() {
        // The bug the old code had: prev=0.001 (effectively zero), current=-0.06,
        // threshold=0.05 — old logic fired a pulse. Symmetric logic must not.
        let mut e = make_emitter(0.05, 2);
        e.prev_m = 0.001;
        assert!(!e.should_pulse(-0.06, 100));
    }

    #[test]
    fn rejects_crossing_when_current_is_inside_the_band() {
        // Symmetric counterpart of the above.
        let mut e = make_emitter(0.05, 2);
        e.prev_m = -0.4;
        assert!(!e.should_pulse(0.001, 100));
    }

    #[test]
    fn rejects_oscillation_entirely_inside_the_band() {
        // The thermal-phase failure mode: <M> jitters around zero.
        let mut e = make_emitter(0.05, 2);
        e.prev_m = -0.03;
        assert!(!e.should_pulse(0.04, 100));
    }

    #[test]
    fn respects_debounce_window() {
        let mut e = make_emitter(0.05, 4);
        e.prev_m = -0.4;
        e.last_pulse_tick = 100;
        // Inside debounce window (tick 102 - 100 = 2, not > 4).
        assert!(!e.should_pulse(0.4, 102));
        // Just outside (tick 105 - 100 = 5, > 4).
        assert!(e.should_pulse(0.4, 105));
    }

    #[test]
    fn no_pulse_when_signs_match() {
        let mut e = make_emitter(0.05, 2);
        e.prev_m = 0.3;
        // Same sign — not a crossing, even though both are above threshold.
        assert!(!e.should_pulse(0.5, 100));
    }
}