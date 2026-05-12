mod chain;
mod cli;
mod clock;
mod config;
mod events;
mod midi;
mod osc;
mod osc_io;
mod runtime;
mod scheduler;
mod wall_midi;
mod walls;

use crate::cli::Cli;
use crate::config::{Config, PhysicsTargets};
use crate::runtime::Runtime;
use clap::Parser;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

fn main() {
    let cli = Cli::parse();

    if cli.list_ports {
        return list_ports();
    }

    let config = Config::from(&cli);

    let running = install_shutdown_handler();

    let midi_sender = match midi::MidiSender::open(cli.port, config.midi.clone()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to open MIDI port: {}", e);
            std::process::exit(1);
        }
    };

    let targets = Arc::new(RwLock::new(PhysicsTargets::from_physics(&config.physics)));
    let osc_sink = start_osc(&cli, &config, Arc::clone(&targets));

    let mut runtime = Runtime::build(&config, midi_sender, osc_sink, targets);

    let total_ticks =
        cli.periods.unwrap_or(20_000) * config.physics.ticks_per_period as u64;
    println!("Running for {} drive periods...", total_ticks / config.physics.ticks_per_period as u64);

    runtime.run_until(total_ticks, &running);

    println!("\nShutting down cleanly...");
    runtime.shutdown();
}

fn install_shutdown_handler() -> Arc<AtomicBool> {
    let running = Arc::new(AtomicBool::new(true));
    let handler = Arc::clone(&running);
    ctrlc::set_handler(move || handler.store(false, Ordering::Release))
        .expect("failed to install Ctrl-C handler");
    running
}

fn list_ports() {
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
}

fn start_osc(
    cli: &Cli,
    config: &Config,
    targets: Arc<RwLock<PhysicsTargets>>,
) -> Option<osc_io::OscSink> {
    if let Some(port) = cli.osc_listen {
        match osc_io::spawn_receiver(port, targets) {
            Ok(bound) => println!("OSC: listening on port {}", bound),
            Err(e) => {
                eprintln!("Failed to bind OSC listener on port {}: {}", port, e);
                std::process::exit(1);
            }
        }
    }

    let addr = cli.osc_send.as_deref()?;
    match osc_io::spawn_sender(addr) {
        Ok(tx) => {
            println!("OSC: sending to {}", addr);
            Some(osc_io::OscSink::new(tx, &config.osc))
        }
        Err(e) => {
            eprintln!("Failed to start OSC sender for {}: {}", addr, e);
            std::process::exit(1);
        }
    }
}