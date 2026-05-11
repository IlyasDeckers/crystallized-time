//! MIDI output configuration — pitch assignments, channel routing,
//! gate length, output topology.

use super::OutputMode;

/// MIDI output parameters.
#[derive(Clone, Debug)]
pub struct MidiConfig {
    /// MIDI note pitch for ChannelPerSite mode (where pitch is irrelevant —
    /// gate signals).
    pub pitch: u8,
    /// Per-voice MIDI pitches for OneChannelPerChain mode.
    /// Length must match the number of output sites.
    /// Default: Cmaj7 voicing (C3, E3, G3, B3).
    pub voice_pitches: Vec<u8>,
    /// Gate length in milliseconds (note-on to note-off delay).
    pub gate_length_ms: u64,
    /// Base MIDI channel (0-15).
    /// In OneChannelPerChain: chain's channel.
    /// In ChannelPerSite: voice 0's channel; voice k goes to base_channel + k.
    pub base_channel: u8,
    /// Output topology.
    pub mode: OutputMode,
}

impl Default for MidiConfig {
    fn default() -> Self {
        Self {
            pitch: 48,
            voice_pitches: vec![48, 52, 55, 59], // C3, E3, G3, B3 — Cmaj7
            gate_length_ms: 50,
            base_channel: 0,
            mode: OutputMode::default(),
        }
    }
}