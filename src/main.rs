mod cli;
mod clock;
mod events;
mod midi;
mod modulation;
mod osc;
mod osc_io;
mod runtime;
mod scheduler;
mod wall_midi;
mod walls;
mod input;
mod perturbation;

use crate::cli::Cli;
// These `use` imports bring `chain` and `config` into the binary crate root so
// that in-crate modules (clock.rs, events.rs, walls.rs, etc.) can reference
// `crate::chain::*` and `crate::config::*` without an explicit dependency on
// the library crate name. Keep this line unless every sub-module is migrated
// to `crystallized_time::chain` / `crystallized_time::config`.
use crystallized_time::{chain, config};
use crate::config::{config_file, Config, PhysicsTargets};
use clap::Parser;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use crate::runtime::{CouplingTargets, Runtime};

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

    let midi_sender = match midi::MidiSender::open(cli.port, config.chain_a.midi.clone()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to open MIDI port: {}", e);
            std::process::exit(1);
        }
    };

    // let targets = Arc::new(RwLock::new(PhysicsTargets::from_physics(&config.chain_a.physics)));

    let (input_listener, perturbation_router) = open_input(&cli, &config);
    let targets_a = Arc::new(RwLock::new(PhysicsTargets::from_physics(&config.chain_a.physics)));
    let targets_b = config.chain_b.as_ref().map(|b| {
        Arc::new(RwLock::new(PhysicsTargets::from_physics(&b.physics)))
    });

    let coupling_targets = match (&config.coupling, &config.chain_b) {
        (Some(c), Some(_)) => Some(Arc::new(RwLock::new(CouplingTargets::from_config(c)))),
        _ => None,
    };

    let osc_targets = osc_io::OscTargets::new(
        Arc::clone(&targets_a),
        targets_b.as_ref().map(Arc::clone),
        coupling_targets.as_ref().map(Arc::clone),
    );
    let osc_sink = start_osc(&config, osc_targets);

    let mut runtime = Runtime::build(
        &config,
        midi_sender,
        osc_sink,
        targets_a,
        targets_b,
        coupling_targets,
        input_listener,
        perturbation_router,
    );

    let total_ticks =
        cli.periods.unwrap_or(20_000) * config.chain_a.physics.ticks_per_period as u64;
    println!(
        "Running for {} drive periods...",
        total_ticks / config.chain_a.physics.ticks_per_period as u64
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
    targets: osc_io::OscTargets,
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