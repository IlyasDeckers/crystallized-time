//! TOML configuration front-end.
//!
//! Reads a config file from disk, validates it, and produces a `Config`
//! struct the rest of the program already knows how to consume. The
//! existing `Config` types are not changed by this module — it is purely
//! a deserialization layer.
//!
//! # File layout (schema summary)
//!
//! ```toml
//! [tempo]
//! bpm = 120
//!
//! [osc]
//! listen_port = 9000              # optional
//! send_address = "127.0.0.1:9001" # optional
//! state_rate_hz = 50
//! enable_state = true
//!
//! [physics]   # optional shared block
//! kt = 0.1
//! eps = 0.01
//! j = 1.2
//! w = 2.0
//! n_sites = 8
//! ticks_per_period = 25
//!
//! [chain_a]
//! seed = 47
//!
//! [chain_a.physics]   # optional; overrides [physics] for chain A
//! # same fields as [physics]
//!
//! [chain_a.gates]
//! voice_0 = 1                                # shorthand: channel-only
//! voice_1 = { channel = 2, pitch = 52 }      # full form
//! gate_length_ms = 50                        # optional
//!
//! [chain_a.walls]
//! voice_0 = 5                                # named voices, Option 1 style
//! voice_1 = 6
//! pitch_low = 36
//! pitch_high = 60
//! motion_cc = 1                              # 0 disables
//! repitch_on_move = false
//!
//! [chain_a.clock]
//! channel = 16
//! enabled = true                             # optional, default true
//! ```
//!
//! Channel and pitch numbers in the file are 1-based for channels
//! (1..=16) and 0..=127 for MIDI pitches. Internal representations
//! convert channels to 0-based.

use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use super::{
    ClockConfig, Config, EventConfig, MidiConfig, OscConfig, PhysicsConfig, TempoConfig,
    WallConfig, WallMidiConfig,
};

// --- Public API ------------------------------------------------------------

/// Top-level error for config-file problems. Variants distinguish IO
/// failures (file doesn't exist, can't read) from parse failures
/// (TOML syntax) from validation failures (semantically invalid).
#[derive(Debug)]
pub enum ConfigError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
    Validation(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Io { path, source } => {
                write!(f, "could not read '{}': {}", path.display(), source)
            }
            ConfigError::Parse { path, source } => {
                write!(f, "could not parse '{}': {}", path.display(), source)
            }
            ConfigError::Validation(msg) => write!(f, "config error: {}", msg),
        }
    }
}

impl std::error::Error for ConfigError {}

/// Read the TOML file at `path`, parse it, validate it, and return the
/// runtime `Config`. The single entry point this module exposes to
/// the rest of the program.
pub fn load(path: &Path) -> Result<Config, ConfigError> {
    let raw = std::fs::read_to_string(path).map_err(|e| ConfigError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;

    let toml_config: TomlConfig = toml::from_str(&raw).map_err(|e| ConfigError::Parse {
        path: path.to_path_buf(),
        source: e,
    })?;

    toml_config.into_config()
}

// --- TOML schema -----------------------------------------------------------
//
// One struct per TOML section. `Option<T>` fields are genuinely optional
// in the file; non-Option fields are required and serde will emit a
// parse error if they're missing. Defaults are applied at conversion
// time in `into_config`, not here, so that missing fields produce
// clearer errors than serde's default-substitution would.

#[derive(Debug, Deserialize)]
struct TomlConfig {
    tempo: Option<TempoSection>,
    osc: Option<OscSection>,
    /// Optional shared physics block. When `chain_a.physics` is absent,
    /// this is what the single chain uses. When both are absent, the
    /// program defaults apply.
    physics: Option<PhysicsSection>,
    chain_a: ChainSection,
}

#[derive(Debug, Deserialize)]
struct TempoSection {
    bpm: f64,
}

#[derive(Debug, Deserialize)]
struct OscSection {
    listen_port: Option<u16>,
    send_address: Option<String>,
    state_rate_hz: Option<f64>,
    enable_state: Option<bool>,
}

#[derive(Debug, Deserialize, Clone)]
struct PhysicsSection {
    kt: Option<f64>,
    eps: Option<f64>,
    j: Option<f64>,
    w: Option<f64>,
    n_sites: Option<usize>,
    ticks_per_period: Option<u32>,
    dt: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct ChainSection {
    seed: Option<u64>,
    physics: Option<PhysicsSection>,
    gates: GatesSection,
    walls: Option<WallsSection>,
    clock: ClockSection,
}

#[derive(Debug, Deserialize)]
struct GatesSection {
    /// Optional gate length override for this chain.
    gate_length_ms: Option<u64>,
    /// All other fields — the `voice_N` entries — are captured as a
    /// flat map so we can accept any number of them and validate them
    /// uniformly.
    #[serde(flatten)]
    voices: BTreeMap<String, GateVoiceEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum GateVoiceEntry {
    /// Shorthand: just the channel number. Pitch falls back to default.
    ChannelOnly(u8),
    /// Full form: channel and pitch named.
    Full { channel: u8, pitch: Option<u8> },
}

#[derive(Debug, Deserialize)]
struct WallsSection {
    pitch_low: Option<u8>,
    pitch_high: Option<u8>,
    /// 0 disables the motion CC. Any other value sets the CC number.
    motion_cc: Option<u8>,
    repitch_on_move: Option<bool>,
    /// `voice_N` entries — same flatten pattern as gates.
    #[serde(flatten)]
    voices: BTreeMap<String, WallVoiceEntry>,
}

/// Wall voice entries are always a bare channel number — wall pitch is
/// derived from position, not assigned per voice. Unlike gates there's
/// no struct-form variant.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum WallVoiceEntry {
    Channel(u8),
}

#[derive(Debug, Deserialize)]
struct ClockSection {
    channel: u8,
    enabled: Option<bool>,
    pitch: Option<u8>,
    gate_length_ms: Option<u64>,
}

// --- Conversion ------------------------------------------------------------

impl TomlConfig {
    fn into_config(self) -> Result<Config, ConfigError> {
        // Tempo. Default to TempoConfig::default() if the section is missing.
        let tempo = match self.tempo {
            Some(t) => TempoConfig::from_bpm(t.bpm),
            None => TempoConfig::default(),
        };

        // OSC. Each field falls back to OscConfig::default().
        let osc = build_osc(self.osc);

        // Physics. Per-chain block wins; falls back to the shared block;
        // falls back to PhysicsConfig::default().
        let physics_section = self.chain_a.physics.clone().or(self.physics.clone());
        let physics = build_physics(physics_section);

        // Seed.
        let seed = self.chain_a.seed.unwrap_or(47);

        // Gates: sort voices by their `voice_N` index, then build parallel
        // channel/pitch vecs.
        let (midi, gate_channels_used) = build_midi(&self.chain_a.gates)?;

        // Walls: same approach. Returns the channels actually claimed for
        // duplicate-detection.
        let (walls, wall_midi, wall_channels_used) =
            build_walls(self.chain_a.walls.as_ref())?;

        // Clock.
        let clock = build_clock(&self.chain_a.clock)?;

        // Event detection follows the gate voices: one output site per
        // gate voice. Indices come from the voice_N names.
        let events = build_events(&self.chain_a.gates)?;

        // Duplicate-channel validation across every claimed channel.
        validate_no_duplicates(
            &gate_channels_used,
            &wall_channels_used,
            clock.channel,
        )?;

        // Sanity: every gate voice's site index must be < n_sites.
        for &site in &events.output_sites {
            if site >= physics.n_sites {
                return Err(ConfigError::Validation(format!(
                    "gate voice_{} refers to a site index that doesn't exist (n_sites = {})",
                    site, physics.n_sites
                )));
            }
        }

        Ok(Config {
            physics,
            events,
            midi,
            tempo,
            clock,
            walls,
            wall_midi,
            osc,
            seed,
        })
    }
}

fn build_osc(section: Option<OscSection>) -> OscConfig {
    let defaults = OscConfig::default();
    match section {
        Some(s) => OscConfig {
            listen_port: s.listen_port,
            send_address: s.send_address,
            state_rate_hz: s.state_rate_hz.unwrap_or(defaults.state_rate_hz),
            enable_state: s.enable_state.unwrap_or(defaults.enable_state),
        },
        None => defaults,
    }
}

fn build_physics(section: Option<PhysicsSection>) -> PhysicsConfig {
    let defaults = PhysicsConfig::default();
    let Some(s) = section else { return defaults };

    PhysicsConfig {
        kt: s.kt.unwrap_or(defaults.kt),
        eps: s.eps.unwrap_or(defaults.eps),
        j: s.j.unwrap_or(defaults.j),
        w: s.w.unwrap_or(defaults.w),
        n_sites: s.n_sites.unwrap_or(defaults.n_sites),
        ticks_per_period: s.ticks_per_period.unwrap_or(defaults.ticks_per_period),
        dt: s.dt.unwrap_or(defaults.dt),
    }
}

/// Build `MidiConfig` from `[chain_a.gates]`. Returns the config plus
/// the list of (voice_index, channel) pairs claimed, for the duplicate
/// validator.
fn build_midi(section: &GatesSection) -> Result<(MidiConfig, Vec<ChannelClaim>), ConfigError> {
    let defaults = MidiConfig::default();
    let sorted = parse_voice_indices(&section.voices, "gates")?;

    if sorted.is_empty() {
        return Err(ConfigError::Validation(
            "chain_a.gates must define at least one voice_N".to_string(),
        ));
    }

    let mut voice_channels = Vec::with_capacity(sorted.len());
    let mut voice_pitches = Vec::with_capacity(sorted.len());
    let mut claims = Vec::with_capacity(sorted.len());

    // Default pitches fall back to the Cmaj7 voicing for voice_0..voice_3;
    // beyond that we have no opinion, and using C3 for all extras is a
    // boring but well-defined choice.
    let default_pitches: &[u8] = &defaults.voice_pitches;

    for (idx, name, entry) in sorted {
        let (channel_1b, pitch) = match entry {
            GateVoiceEntry::ChannelOnly(c) => (*c, None),
            GateVoiceEntry::Full { channel, pitch } => (*channel, *pitch),
        };

        let channel_0b = validate_channel_1based(channel_1b, &format!("chain_a.gates.{}", name))?;
        let pitch = pitch
            .or_else(|| default_pitches.get(idx).copied())
            .unwrap_or(48);

        voice_channels.push(channel_0b);
        voice_pitches.push(pitch);
        claims.push(ChannelClaim {
            channel: channel_0b,
            label: format!("chain_a.gates.{}", name),
        });
    }

    let midi = MidiConfig {
        voice_channels,
        voice_pitches,
        gate_length_ms: section.gate_length_ms.unwrap_or(defaults.gate_length_ms),
    };

    Ok((midi, claims))
}

fn build_walls(
    section: Option<&WallsSection>,
) -> Result<(WallConfig, WallMidiConfig, Vec<ChannelClaim>), ConfigError> {
    let wall_defaults = WallConfig::default();
    let midi_defaults = WallMidiConfig::default();

    // No [chain_a.walls] table at all → walls disabled. Detection still
    // runs (cheap), but the channels list is empty so the allocator
    // silently drops every wall event. Effectively the same as
    // `walls.enabled = false`, which we set here for clarity.
    let Some(s) = section else {
        return Ok((
            WallConfig { enabled: false, ..wall_defaults },
            WallMidiConfig { channels: Vec::new(), ..midi_defaults },
            Vec::new(),
        ));
    };

    let sorted = parse_voice_indices(&s.voices, "walls")?;

    let mut channels = Vec::with_capacity(sorted.len());
    let mut claims = Vec::with_capacity(sorted.len());
    for (_idx, name, entry) in sorted {
        let WallVoiceEntry::Channel(c) = entry;
        let channel_0b = validate_channel_1based(*c, &format!("chain_a.walls.{}", name))?;
        channels.push(channel_0b);
        claims.push(ChannelClaim {
            channel: channel_0b,
            label: format!("chain_a.walls.{}", name),
        });
    }

    let motion_cc = match s.motion_cc {
        Some(0) => None,
        Some(n) if n <= 127 => Some(n),
        Some(n) => {
            return Err(ConfigError::Validation(format!(
                "chain_a.walls.motion_cc must be in 0..=127 (got {})",
                n
            )));
        }
        None => midi_defaults.motion_cc,
    };

    let pitch_low = s.pitch_low.unwrap_or(midi_defaults.pitch_low);
    let pitch_high = s.pitch_high.unwrap_or(midi_defaults.pitch_high);
    if pitch_low > 127 || pitch_high > 127 {
        return Err(ConfigError::Validation(format!(
            "chain_a.walls pitch range must be in 0..=127 (got {}..{})",
            pitch_low, pitch_high
        )));
    }
    if pitch_low > pitch_high {
        return Err(ConfigError::Validation(format!(
            "chain_a.walls pitch_low must be <= pitch_high (got {} > {})",
            pitch_low, pitch_high
        )));
    }

    // No walls voices listed → equivalent to walls disabled.
    let enabled = !channels.is_empty();

    let wall_midi = WallMidiConfig {
        channels,
        pitch_low,
        pitch_high,
        motion_cc,
        repitch_on_move: s.repitch_on_move.unwrap_or(midi_defaults.repitch_on_move),
    };

    let walls = WallConfig {
        enabled,
        ..wall_defaults
    };

    Ok((walls, wall_midi, claims))
}

fn build_clock(section: &ClockSection) -> Result<ClockConfig, ConfigError> {
    let defaults = ClockConfig::default();
    let channel_0b = validate_channel_1based(section.channel, "chain_a.clock.channel")?;
    Ok(ClockConfig {
        enabled: section.enabled.unwrap_or(defaults.enabled),
        channel: channel_0b,
        pitch: section.pitch.unwrap_or(defaults.pitch),
        crossing_threshold: defaults.crossing_threshold,
        debounce_ticks: defaults.debounce_ticks,
        gate_length_ms: section.gate_length_ms.unwrap_or(defaults.gate_length_ms),
    })
}

fn build_events(section: &GatesSection) -> Result<EventConfig, ConfigError> {
    let defaults = EventConfig::default();
    let sorted = parse_voice_indices(&section.voices, "gates")?;
    // The voice index N in `voice_N` is interpreted as the chain site
    // index the voice listens to. So `voice_0`, `voice_2`, `voice_4`,
    // `voice_6` produces output_sites = [0, 2, 4, 6] — matching the
    // historical default.
    let output_sites: Vec<usize> = sorted.iter().map(|(idx, _, _)| *idx).collect();
    Ok(EventConfig {
        output_sites,
        crossing_threshold: defaults.crossing_threshold,
        debounce_ticks: defaults.debounce_ticks,
    })
}

// --- Helpers ---------------------------------------------------------------

/// Record of "this channel is claimed by this signal in the config."
/// Used to assemble a flat list across gates, walls, and clock for the
/// duplicate-channel validator.
struct ChannelClaim {
    channel: u8,
    label: String,
}

/// Parse `voice_N` entries from a flatten map into `(index, name, entry)`
/// tuples sorted by index. Names that don't match `voice_<digits>` are
/// a validation error — we won't silently ignore typos.
fn parse_voice_indices<'a, V>(
    map: &'a BTreeMap<String, V>,
    section_label: &str,
) -> Result<Vec<(usize, &'a String, &'a V)>, ConfigError> {
    let mut out: Vec<(usize, &String, &V)> = Vec::with_capacity(map.len());
    for (name, value) in map.iter() {
        let Some(suffix) = name.strip_prefix("voice_") else {
            return Err(ConfigError::Validation(format!(
                "chain_a.{}: unrecognized key '{}' (expected 'voice_N')",
                section_label, name
            )));
        };
        let idx: usize = suffix.parse().map_err(|_| {
            ConfigError::Validation(format!(
                "chain_a.{}: voice key '{}' has non-numeric suffix",
                section_label, name
            ))
        })?;
        out.push((idx, name, value));
    }
    out.sort_by_key(|(idx, _, _)| *idx);
    Ok(out)
}

/// Translate a 1-based channel number from the file (1..=16) into
/// a 0-based channel for internal use. Out-of-range values are a
/// validation error.
fn validate_channel_1based(channel_1b: u8, label: &str) -> Result<u8, ConfigError> {
    if !(1..=16).contains(&channel_1b) {
        return Err(ConfigError::Validation(format!(
            "{}: channel must be in 1..=16 (got {})",
            label, channel_1b
        )));
    }
    Ok(channel_1b - 1)
}

/// Final pass: assert that no two signals across the whole config want
/// the same MIDI channel. Names both claimants in the error message,
/// matching the spec's example exactly.
fn validate_no_duplicates(
    gate_claims: &[ChannelClaim],
    wall_claims: &[ChannelClaim],
    clock_channel: u8,
) -> Result<(), ConfigError> {
    let mut owners: HashMap<u8, String> = HashMap::new();

    let mut register = |claim: &ChannelClaim| -> Result<(), ConfigError> {
        if let Some(prior) = owners.get(&claim.channel) {
            return Err(ConfigError::Validation(format!(
                "channel {} is assigned to both {} and {}",
                claim.channel + 1,
                prior,
                claim.label
            )));
        }
        owners.insert(claim.channel, claim.label.clone());
        Ok(())
    };

    for c in gate_claims {
        register(c)?;
    }
    for c in wall_claims {
        register(c)?;
    }
    register(&ChannelClaim {
        channel: clock_channel,
        label: "chain_a.clock".to_string(),
    })?;

    Ok(())
}

// --- Tests -----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn load_str(s: &str) -> Result<Config, ConfigError> {
        let raw: TomlConfig = toml::from_str(s).map_err(|e| ConfigError::Parse {
            path: PathBuf::from("<test>"),
            source: e,
        })?;
        raw.into_config()
    }

    #[test]
    fn minimal_config_loads() {
        let toml = r#"
            [chain_a]
            seed = 7

            [chain_a.gates]
            voice_0 = 1
            voice_2 = 2
            voice_4 = 3
            voice_6 = 4

            [chain_a.clock]
            channel = 16
        "#;
        let config = load_str(toml).expect("should parse");
        assert_eq!(config.seed, 7);
        assert_eq!(config.midi.voice_channels, vec![0, 1, 2, 3]);
        assert_eq!(config.events.output_sites, vec![0, 2, 4, 6]);
        // Clock channel was 16 (1-based) → 15 (0-based)
        assert_eq!(config.clock.channel, 15);
        // No walls section → walls disabled, empty channel list
        assert!(!config.walls.enabled);
        assert!(config.wall_midi.channels.is_empty());
    }

    #[test]
    fn gate_voice_full_form_carries_pitch() {
        let toml = r#"
            [chain_a]
            [chain_a.gates]
            voice_0 = { channel = 1, pitch = 60 }
            voice_1 = 2
            [chain_a.clock]
            channel = 16
        "#;
        let config = load_str(toml).expect("should parse");
        assert_eq!(config.midi.voice_pitches[0], 60);
        // voice_1 fell back to the default-pitch table at index 1 (E3, MIDI 52)
        assert_eq!(config.midi.voice_pitches[1], 52);
    }

    #[test]
    fn wall_voices_become_channel_list() {
        let toml = r#"
            [chain_a]
            [chain_a.gates]
            voice_0 = 1
            [chain_a.walls]
            voice_0 = 5
            voice_1 = 6
            voice_2 = 7
            voice_3 = 8
            [chain_a.clock]
            channel = 16
        "#;
        let config = load_str(toml).expect("should parse");
        assert!(config.walls.enabled);
        assert_eq!(config.wall_midi.channels, vec![4, 5, 6, 7]);
    }

    #[test]
    fn duplicate_channel_is_rejected_naming_both() {
        let toml = r#"
            [chain_a]
            [chain_a.gates]
            voice_0 = 1
            [chain_a.walls]
            voice_0 = 1
            [chain_a.clock]
            channel = 16
        "#;
        let err = load_str(toml).expect_err("duplicate channel must fail");
        let msg = format!("{}", err);
        assert!(msg.contains("channel 1"));
        assert!(msg.contains("chain_a.gates.voice_0"));
        assert!(msg.contains("chain_a.walls.voice_0"));
    }

    #[test]
    fn out_of_range_channel_is_rejected() {
        let toml = r#"
            [chain_a]
            [chain_a.gates]
            voice_0 = 17
            [chain_a.clock]
            channel = 16
        "#;
        let err = load_str(toml).expect_err("channel 17 must fail");
        let msg = format!("{}", err);
        assert!(msg.contains("1..=16"));
        assert!(msg.contains("17"));
    }

    #[test]
    fn unknown_voice_key_is_rejected() {
        let toml = r#"
            [chain_a]
            [chain_a.gates]
            voixe_0 = 1
            [chain_a.clock]
            channel = 16
        "#;
        let err = load_str(toml).expect_err("typo must fail");
        let msg = format!("{}", err);
        assert!(msg.contains("voixe_0"));
    }

    #[test]
    fn no_gate_voices_is_rejected() {
        let toml = r#"
            [chain_a]
            [chain_a.gates]
            [chain_a.clock]
            channel = 16
        "#;
        let err = load_str(toml).expect_err("empty gates must fail");
        assert!(format!("{}", err).contains("at least one"));
    }

    #[test]
    fn per_chain_physics_overrides_shared() {
        let toml = r#"
            [physics]
            kt = 0.05

            [chain_a]
            [chain_a.physics]
            kt = 0.5

            [chain_a.gates]
            voice_0 = 1
            [chain_a.clock]
            channel = 16
        "#;
        let config = load_str(toml).expect("should parse");
        assert!((config.physics.kt - 0.5).abs() < 1e-9);
    }

    #[test]
    fn shared_physics_is_used_when_per_chain_absent() {
        let toml = r#"
            [physics]
            kt = 0.42

            [chain_a]
            [chain_a.gates]
            voice_0 = 1
            [chain_a.clock]
            channel = 16
        "#;
        let config = load_str(toml).expect("should parse");
        assert!((config.physics.kt - 0.42).abs() < 1e-9);
    }

    #[test]
    fn motion_cc_zero_disables() {
        let toml = r#"
            [chain_a]
            [chain_a.gates]
            voice_0 = 1
            [chain_a.walls]
            voice_0 = 5
            motion_cc = 0
            [chain_a.clock]
            channel = 16
        "#;
        let config = load_str(toml).expect("should parse");
        assert!(config.wall_midi.motion_cc.is_none());
    }

    #[test]
    fn site_index_must_exist() {
        // n_sites = 4 but voice_6 references site 6
        let toml = r#"
            [physics]
            n_sites = 4

            [chain_a]
            [chain_a.gates]
            voice_0 = 1
            voice_6 = 2
            [chain_a.clock]
            channel = 16
        "#;
        let err = load_str(toml).expect_err("nonexistent site must fail");
        assert!(format!("{}", err).contains("doesn't exist"));
    }

    #[test]
    fn tempo_bpm_translates_to_drive_period() {
        let toml = r#"
            [tempo]
            bpm = 60

            [chain_a]
            [chain_a.gates]
            voice_0 = 1
            [chain_a.clock]
            channel = 16
        "#;
        let config = load_str(toml).expect("should parse");
        // 60 BPM → 1.0 sec drive period
        assert!((config.tempo.drive_period_secs - 1.0).abs() < 1e-9);
    }
}