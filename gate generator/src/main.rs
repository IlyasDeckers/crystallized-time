//! crystallized_time — time-crystal-driven MIDI gate generator.
//!

mod chain;
mod config;
mod events;
mod midi;
mod scheduler;
mod clock;
mod wall_midi;
mod walls;

use crate::wall_midi::WallVoiceAllocator;
use clap::Parser;
use config::{Config, TempoConfig};
use rand::SeedableRng;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Parser, Debug)]
#[command(name = "crystallized_time", version, about)]
struct Cli {
    #[arg(long)]
    list_ports: bool,

    #[arg(short, long, default_value_t = 0)]
    port: usize,

    #[arg(long, default_value_t = 120.0)]
    bpm: f64,

    #[arg(long, default_value_t = 47)]
    seed: u64,

    #[arg(long, value_enum, default_value_t = config::OutputMode::OneChannelPerChain)]
    mode: config::OutputMode,

    #[arg(long, default_value_t = 16)]
    clock_channel: u8,

    #[arg(long)]
    no_clock: bool,

    /// Disable domain-wall detection and output entirely.
    #[arg(long)]
    no_walls: bool,

    /// MIDI channel range for wall voices, 1-indexed inclusive (e.g. "5:8").
    #[arg(long, value_parser = parse_channel_range)]
    wall_channels: Option<(u8, u8)>,

    /// MIDI pitch range walls span, 0-127 inclusive (e.g. "36:84").
    #[arg(long, value_parser = parse_pitch_range)]
    wall_pitch_range: Option<(u8, u8)>,

    /// CC number for wall motion, 0-127. Set to 0 to disable.
    #[arg(long)]
    wall_motion_cc: Option<u8>,

    /// Use discrete repitching on wall motion instead of held-pitch + CC.
    #[arg(long)]
    wall_repitch_on_move: bool,
}

fn main() {
    let cli = Cli::parse();

    let walls_cfg = config::WallConfig {
        enabled: !cli.no_walls,
        ..Default::default()
    };

    let mut wall_midi_cfg = config::WallMidiConfig::default();

    if let Some((lo, hi)) = cli.wall_channels {
        // CLI is 1-based, internal is 0-based.
        wall_midi_cfg.channel_low  = lo - 1;
        wall_midi_cfg.channel_high = hi - 1;
    }
    if let Some((lo, hi)) = cli.wall_pitch_range {
        wall_midi_cfg.pitch_low  = lo;
        wall_midi_cfg.pitch_high = hi;
    }
    if let Some(cc) = cli.wall_motion_cc {
        wall_midi_cfg.motion_cc = if cc == 0 { None } else { Some(cc) };
    }
    wall_midi_cfg.repitch_on_move = cli.wall_repitch_on_move;

    let config = Config {
        tempo: TempoConfig::from_bpm(cli.bpm),
        seed: cli.seed,
        midi: config::MidiConfig {
            mode: cli.mode,
            ..Default::default()
        },
        clock: config::ClockConfig {
            enabled: !cli.no_clock,
            channel: cli.clock_channel.saturating_sub(1).min(15),
            ..Default::default()
        },
        walls: walls_cfg,
        wall_midi: wall_midi_cfg,
        ..Default::default()
    };

    if cli.list_ports {
        match midi::MidiSender::list_ports() {
            Ok(ports) => {
                println!("Available MIDI output ports:");
                if ports.is_empty() {
                    println!("  (none)");
                } else {
                    for (i, name) in ports.iter().enumerate() {
                        println!("  [{}] {}", i, name);
                    }
                }
            }
            Err(e) => {
                eprintln!("Error listing ports: {}", e);
                std::process::exit(1);
            }
        }
        return;
    }

    let running = Arc::new(AtomicBool::new(true));
    let running_handler = Arc::clone(&running);
    ctrlc::set_handler(move || {
        running_handler.store(false, Ordering::Release);
    }).expect("failed to install Ctrl-C handler");

    let midi_sender = match midi::MidiSender::open(cli.port, config.midi.clone()) {
        Ok(sender) => sender,
        Err(e) => {
            eprintln!("Failed to open MIDI port: {}", e);
            std::process::exit(1);
        }
    };

    let mut rng = rand::rngs::StdRng::seed_from_u64(config.seed);
    let mut chain = chain::SpinChain::new(config.physics.clone(), &mut rng);
    let mut detector = events::EventDetector::new(config.events.clone(), &chain);
    let mut clock_emitter = clock::ClockEmitter::new(config.clock.clone(), &chain);
    let mut wall_detector = walls::WallDetector::new(config.walls.clone());
    let mut wall_voicer = WallVoiceAllocator::new(
        config.wall_midi.clone(),
        &config.physics,
    );

    println!();
    println!("Running for 20000 drive periods, sending MIDI gates...");
    // println!("tick   site  channel  intensity");

    let tick_duration = std::time::Duration::from_secs_f64(
        config.tempo.drive_period_secs / config.physics.ticks_per_period as f64
    );
    let start = std::time::Instant::now();

    let total_ticks = 20000 * config.physics.ticks_per_period as u64;
    for tick in 1..=total_ticks {
        if !running.load(Ordering::Acquire) {
            break;
        }

        chain.step(&mut rng);
        let events = detector.check(&chain);
        for event in events {
            midi_sender.send_gate(event);
        }

        clock_emitter.tick(&chain, &midi_sender);

        let wall_events = wall_detector.check(&chain);
        for event in &wall_events {
            // Print for visibility; safe to remove later.
            match event {
                walls::WallEvent::Created { id, position, tick } => {
                    println!("tick {:5}  wall {:3} CREATED  at {:.2}", tick, id, position);
                }
                walls::WallEvent::Destroyed { id, last_position, tick, lifetime_ticks } => {
                    println!("tick {:5}  wall {:3} DESTROYED at {:.2} (lived {} ticks)",
                             tick, id, last_position, lifetime_ticks);
                }
                walls::WallEvent::Moved { id, from, to, velocity, tick } => {
                    println!("tick {:5}  wall {:3} MOVED  {:.2} -> {:.2} (v={:+.2})",
                             tick, id, from, to, velocity);
                }
            }
            wall_voicer.handle(event, &midi_sender);
        }

        // Pace to wall-clock.
        let target = start + tick_duration * tick as u32;
        let now = std::time::Instant::now();
        if target > now {
            std::thread::sleep(target - now);
        }
    }

    println!("\nShutting down cleanly...");
    midi_sender.shutdown();
    // Give the scheduler's worker thread a moment to drain any pending
    // note-offs before MidiSender drops (which will join the worker anyway,
    // but the worker fires remaining messages on the way out — this small
    // pause lets normal-deadline note-offs fire at their proper times).
    std::thread::sleep(std::time::Duration::from_millis(
        config.midi.gate_length_ms + 50
    ));
}

fn parse_channel_range(s: &str) -> Result<(u8, u8), String> {
    let (lo, hi) = parse_u8_pair(s)?;
    if lo < 1 || hi > 16 {
        return Err(format!("channels must be in 1..=16 (got {}:{})", lo, hi));
    }
    if lo > hi {
        return Err(format!("low channel must be <= high channel (got {}:{})", lo, hi));
    }
    Ok((lo, hi))
}

fn parse_pitch_range(s: &str) -> Result<(u8, u8), String> {
    let (lo, hi) = parse_u8_pair(s)?;
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
    let lo: u8 = parts[0].trim().parse()
        .map_err(|_| format!("invalid number: '{}'", parts[0]))?;
    let hi: u8 = parts[1].trim().parse()
        .map_err(|_| format!("invalid number: '{}'", parts[1]))?;
    Ok((lo, hi))
}
