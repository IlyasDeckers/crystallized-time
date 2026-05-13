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
mod input;
mod perturbation;

use crate::cli::Cli;
use crate::config::{config_file, Config, PhysicsTargets};
use crate::runtime::Runtime;
use clap::Parser;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

fn main() {
    let cli = Cli::parse();

    if cli.list_ports {
        return list_ports();
    }

    if cli.list_input_ports {
        return list_input_ports();
    }

    let config = match config_file::load(&cli.config) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };

    let running = install_shutdown_handler();

    let midi_sender = match midi::MidiSender::open(cli.port, config.midi.clone()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to open MIDI port: {}", e);
            std::process::exit(1);
        }
    };

    let targets = Arc::new(RwLock::new(PhysicsTargets::from_physics(&config.physics)));
    let osc_sink = start_osc(&config, Arc::clone(&targets));
    let (input_listener, perturbation_router) = open_input(&cli, &config);
    
    let mut runtime = Runtime::build(
        &config,
        midi_sender,
        osc_sink,
        targets,
        input_listener,
        perturbation_router,
    );

    let total_ticks =
        cli.periods.unwrap_or(20_000) * config.physics.ticks_per_period as u64;
    println!(
        "Running for {} drive periods...",
        total_ticks / config.physics.ticks_per_period as u64
    );

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

fn list_input_ports() {
    match input::MidiInputListener::list_ports() {
        Ok(ports) => {
            println!("Available MIDI input ports:");
            if ports.is_empty() {
                println!("  (none)");
            } else {
                for (i, name) in ports.iter().enumerate() {
                    println!("  [{}] {}", i, name);
                }
            }
        }
        Err(e) => {
            eprintln!("Error listing input ports: {}", e);
            std::process::exit(1);
        }
    }
}

fn open_input(
    cli: &Cli,
    config: &Config,
) -> (
    Option<input::MidiInputListener>,
    Option<perturbation::PerturbationRouter>,
) {
    // No [input] section in the config: input is fully disabled, regardless
    // of whether --input-port was given. Telling the user is friendlier than
    // silently ignoring the flag.
    let Some(input_cfg) = config.input.as_ref() else {
        if cli.input_port.is_some() {
            eprintln!(
                "warning: --input-port was given but the config file has no [input] section; \
                 input will not be opened"
            );
        }
        return (None, None);
    };

    let Some(port_index) = cli.input_port else {
        // Config opted in but no port chosen on the CLI. Equally valid —
        // user might want to enumerate first.
        return (None, None);
    };

    let listener = match input::MidiInputListener::open(port_index) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Failed to open MIDI input port {}: {}", port_index, e);
            std::process::exit(1);
        }
    };

    let router = perturbation::PerturbationRouter::new(input_cfg.perturbation.clone());
    (Some(listener), Some(router))
}

fn start_osc(
    config: &Config,
    targets: Arc<RwLock<PhysicsTargets>>,
) -> Option<osc_io::OscSink> {
    if let Some(port) = config.osc.listen_port {
        match osc_io::spawn_receiver(port, targets) {
            Ok(bound) => println!("OSC: listening on port {}", bound),
            Err(e) => {
                eprintln!("Failed to bind OSC listener on port {}: {}", port, e);
                std::process::exit(1);
            }
        }
    }

    let addr = config.osc.send_address.as_deref()?;
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