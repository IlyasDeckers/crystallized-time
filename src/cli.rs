//! Command-line interface — argument parsing and translation to Config.

use crate::config::{
    ClockConfig, Config, MidiConfig, OscConfig, OutputMode, TempoConfig,
    WallConfig, WallMidiConfig,
};
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "crystallized_time", version, about)]
pub struct Cli {
    /// Print available MIDI output ports and exit.
    #[arg(long)]
    pub list_ports: bool,

    /// Which MIDI output port to open.
    #[arg(short, long, default_value_t = 0)]
    pub port: usize,

    /// Tempo. One drive period = one beat.
    #[arg(long, default_value_t = 120.0)]
    pub bpm: f64,

    /// RNG seed for the simulation. Same seed → same run.
    #[arg(long, default_value_t = 47)]
    pub seed: u64,

    /// Output topology for site-based voices.
    #[arg(long, value_enum, default_value_t = OutputMode::OneChannelPerChain)]
    pub mode: OutputMode,

    /// MIDI channel (1-16) for the substrate clock.
    #[arg(long, default_value_t = 16)]
    pub clock_channel: u8,

    /// Disable the substrate clock output.
    #[arg(long)]
    pub no_clock: bool,

    /// Disable domain-wall detection and output entirely.
    #[arg(long)]
    pub no_walls: bool,

    /// MIDI channel range for wall voices, 1-indexed inclusive (e.g. "5:8").
    #[arg(long, value_parser = parse_channel_range)]
    pub wall_channels: Option<(u8, u8)>,

    /// MIDI pitch range walls span, 0-127 inclusive (e.g. "36:84").
    #[arg(long, value_parser = parse_pitch_range)]
    pub wall_pitch_range: Option<(u8, u8)>,

    /// CC number for wall motion, 0-127. Set to 0 to disable.
    #[arg(long)]
    pub wall_motion_cc: Option<u8>,

    /// Use discrete repitching on wall motion instead of held-pitch + CC.
    #[arg(long)]
    pub wall_repitch_on_move: bool,

    /// UDP port to listen for inbound OSC parameter messages.
    #[arg(long)]
    pub osc_listen: Option<u16>,

    /// Destination "host:port" for outbound OSC events and state.
    #[arg(long)]
    pub osc_send: Option<String>,

    /// Target rate for OSC state messages, in Hz. Default 50.
    #[arg(long)]
    pub osc_state_rate: Option<f64>,

    /// Disable OSC state messages entirely. Events still flow.
    #[arg(long)]
    pub no_osc_state: bool,

    /// Number of drive periods to run before shutting down. Default 20000.
    #[arg(long)]
    pub periods: Option<u64>,
}

impl From<&Cli> for Config {
    fn from(cli: &Cli) -> Self {
        Config {
            tempo: TempoConfig::from_bpm(cli.bpm),
            seed: cli.seed,
            midi: MidiConfig {
                mode: cli.mode,
                ..Default::default()
            },
            clock: ClockConfig {
                enabled: !cli.no_clock,
                // CLI is 1-based; internal is 0-based. saturating_sub guards
                // against a nonsense `--clock-channel 0`, and `min(15)` clamps
                // values above 16 to the last channel rather than panicking.
                channel: cli.clock_channel.saturating_sub(1).min(15),
                ..Default::default()
            },
            walls: WallConfig {
                enabled: !cli.no_walls,
                ..Default::default()
            },
            wall_midi: build_wall_midi(cli),
            osc: OscConfig {
                state_rate_hz: cli
                    .osc_state_rate
                    .unwrap_or_else(|| OscConfig::default().state_rate_hz),
                enable_state: !cli.no_osc_state,
            },
            ..Default::default()
        }
    }
}

/// Build the wall MIDI config from CLI overrides. Each field falls back to
/// `WallMidiConfig::default()` when its flag wasn't provided.
fn build_wall_midi(cli: &Cli) -> WallMidiConfig {
    let mut cfg = WallMidiConfig::default();

    if let Some((lo, hi)) = cli.wall_channels {
        // 1-based on the CLI, 0-based internally.
        cfg.channel_low = lo - 1;
        cfg.channel_high = hi - 1;
    }
    if let Some((lo, hi)) = cli.wall_pitch_range {
        cfg.pitch_low = lo;
        cfg.pitch_high = hi;
    }
    if let Some(cc) = cli.wall_motion_cc {
        cfg.motion_cc = if cc == 0 { None } else { Some(cc) };
    }
    cfg.repitch_on_move = cli.wall_repitch_on_move;

    cfg
}

// --- Value parsers for the "lo:hi" range flags ----------------------------
//
// clap calls these on each candidate string and uses the returned `Result`
// to either accept the parsed pair or print a usage error. Splitting them
// out from `parse_u8_pair` keeps the bound-checking (channels 1..=16 vs
// pitches 0..=127) close to the flag definitions that need it.
fn parse_channel_range(s: &str) -> Result<(u8, u8), String> {
    let (lo, hi) = parse_u8_pair(s)?;
    if lo < 1 || hi > 16 {
        return Err(format!("channels must be in 1..=16 (got {}:{})", lo, hi));
    }
    if lo > hi {
        return Err(format!(
            "low channel must be <= high channel (got {}:{})",
            lo, hi
        ));
    }
    Ok((lo, hi))
}

fn parse_pitch_range(s: &str) -> Result<(u8, u8), String> {
    let (lo, hi) = parse_u8_pair(s)?;
    // u8 max is 255, so the upper bound is the only check we need.
    if lo > 127 || hi > 127 {
        return Err(format!("pitches must be in 0..=127 (got {}:{})", lo, hi));
    }
    if lo > hi {
        return Err(format!("low pitch must be <= high pitch (got {}:{})", lo, hi));
    }
    Ok((lo, hi))
}

fn parse_u8_pair(s: &str) -> Result<(u8, u8), String> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 2 {
        return Err(format!("expected 'low:high', got '{}'", s));
    }
    let lo: u8 = parts[0]
        .trim()
        .parse()
        .map_err(|_| format!("invalid number: '{}'", parts[0]))?;
    let hi: u8 = parts[1]
        .trim()
        .parse()
        .map_err(|_| format!("invalid number: '{}'", parts[1]))?;
    Ok((lo, hi))
}