//! crystallized_time — time-crystal-driven MIDI gate generator.
//!

mod chain;
mod config;
mod events;
mod midi;
mod scheduler;

use clap::Parser;
use config::{Config, TempoConfig};
use rand::SeedableRng;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Parser, Debug)]
#[command(name = "crystallized_time", version, about)]
struct Cli {
    /// List available MIDI output ports and exit.
    #[arg(long)]
    list_ports: bool,

    /// MIDI output port index (run with --list-ports to see options).
    #[arg(short, long, default_value_t = 0)]
    port: usize,

    /// Tempo in beats per minute. One drive period = one beat.
    #[arg(long, default_value_t = 120.0)]
    bpm: f64,

    /// RNG seed for the substrate.
    #[arg(long, default_value_t = 47)]
    seed: u64,
}

fn main() {
    let cli = Cli::parse();

    let config = Config {
        tempo: TempoConfig::from_bpm(cli.bpm),
        seed: cli.seed,
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
    let _ = midi_sender; // suppress unused warning until we wire up send_gate

    let mut rng = rand::rngs::StdRng::seed_from_u64(config.seed);
    let mut chain = chain::SpinChain::new(config.physics.clone(), &mut rng);

    let mut detector = events::EventDetector::new(config.events.clone(), &chain);

    println!();
    println!("Running for 200 drive periods, sending MIDI gates...");
    println!("tick   site  channel  intensity");

    let tick_duration = std::time::Duration::from_secs_f64(
        config.tempo.drive_period_secs / config.physics.ticks_per_period as f64
    );
    let start = std::time::Instant::now();

    let total_ticks = 200 * config.physics.ticks_per_period as u64;
    for tick in 1..=total_ticks {
        if !running.load(Ordering::Acquire) {
            break;
        }

        chain.step(&mut rng);
        let events = detector.check(&chain);
        for event in events {
            let channel = config.midi.base_channel + event.voice;
            println!("{:5}  {:4}  {:7}  {:.2}",
                     event.tick, event.site, channel + 1, event.intensity);
            midi_sender.send_gate(event);
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
