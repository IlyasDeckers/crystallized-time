//! MIDI output. Wraps midir to send note-on/note-off pairs for each GateEvent.
//!
//! Note-off is scheduled on a background thread with a sleep so the main loop
//! doesn't block waiting for gate length. This is the simplest correct approach
//! for short fixed gate lengths; for variable or long gates we'd switch to a
//! priority queue of pending note-offs serviced by the loop.

use crate::config::MidiConfig;
use crate::events::GateEvent;
use midir::{MidiOutput, MidiOutputConnection};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// MIDI sender. Owns a connection to a single output port.
pub struct MidiSender {
    config: MidiConfig,
    /// Wrapped in Arc<Mutex<_>> so background threads can write note-offs safely.
    conn: Arc<Mutex<MidiOutputConnection>>,
}

impl MidiSender {
    /// List available output port names.
    pub fn list_ports() -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let midi_out = MidiOutput::new("crystallized_time")?;
        let ports = midi_out.ports();
        let mut names = Vec::with_capacity(ports.len());
        for port in &ports {
            names.push(midi_out.port_name(port)?);
        }
        Ok(names)
    }

    /// Open a connection to the port at `port_index`.
    pub fn open(port_index: usize, config: MidiConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let midi_out = MidiOutput::new("crystallized_time")?;
        let ports = midi_out.ports();

        let port = ports
            .get(port_index)
            .ok_or_else(|| format!("port index {} out of range (found {} ports)", port_index, ports.len()))?;

        let port_name = midi_out.port_name(port)?;
        println!("Opening MIDI port [{}]: {}", port_index, port_name);

        let conn = midi_out.connect(port, "crystallized_time-out")?;

        Ok(Self {
            config,
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Send a gate: note-on now, note-off scheduled after gate_length_ms.
    pub fn send_gate(&self, event: GateEvent) {
        let channel = self.config.base_channel + event.site as u8;
        if channel > 15 {
            eprintln!("warning: site {} maps to MIDI channel {} (> 15), skipping",
                      event.site, channel + 1);
            return;
        }

        let pitch = self.config.pitch;
        let velocity = (event.intensity * 127.0).clamp(1.0, 127.0) as u8;

        let note_on  = [0x90 | channel, pitch, velocity];
        let note_off = [0x80 | channel, pitch, 0];

        if let Ok(mut conn) = self.conn.lock() {
            if let Err(e) = conn.send(&note_on) {
                eprintln!("MIDI send (note-on) failed: {}", e);
                return;
            }
        } else {
            eprintln!("MIDI mutex poisoned, dropping event");
            return;
        }

        let conn_for_off = Arc::clone(&self.conn);
        let gate_length = Duration::from_millis(self.config.gate_length_ms);

        thread::spawn(move || {
            thread::sleep(gate_length);
            if let Ok(mut conn) = conn_for_off.lock() {
                let _ = conn.send(&note_off);
            }
        });
    }
}
