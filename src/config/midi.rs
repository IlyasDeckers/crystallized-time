//! MIDI output configuration — per-voice channel and pitch assignments,
//! gate length.
//!
//! After the TOML-routing refactor there is no `OutputMode` enum: every
//! gate voice is named individually in the config file, with its own
//! channel and pitch. The old `OneChannelPerChain` mode is what you get
//! by assigning all four voices to the same channel with distinct
//! pitches; the old `ChannelPerSite` mode is what you get by assigning
//! distinct channels and uniform pitch.

/// MIDI output parameters.
#[derive(Clone, Debug)]
pub struct MidiConfig {
    /// Per-voice MIDI channels (0-15). Length must match `voice_pitches`
    /// and the number of output sites.
    pub voice_channels: Vec<u8>,
    /// Per-voice MIDI pitches.
    /// Default: Cmaj7 voicing (C3, E3, G3, B3) on a single channel.
    pub voice_pitches: Vec<u8>,
    /// Gate length in milliseconds (note-on to note-off delay).
    pub gate_length_ms: u64,
}

impl Default for MidiConfig {
    fn default() -> Self {
        Self {
            // All four voices on channel 1 (0-indexed 0), distinct pitches.
            // Reproduces the previous OneChannelPerChain default behavior.
            voice_channels: vec![0, 0, 0, 0],
            voice_pitches: vec![48, 52, 55, 59], // C3, E3, G3, B3 — Cmaj7
            gate_length_ms: 50,
        }
    }
}