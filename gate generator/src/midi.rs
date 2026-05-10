//! MIDI output. Wraps midir to send note-on/note-off pairs for each GateEvent.
//!
//! Note-off is scheduled on a background thread with a sleep so the main loop
//! doesn't block waiting for gate length. This is the simplest correct approach
//! for short fixed gate lengths; for variable or long gates we'd switch to a
//! priority queue of pending note-offs serviced by the loop.

use crate::config::MidiConfig;
use crate::events::GateEvent;
use midir::MidiOutput;
use std::time::{Duration, Instant};
use crate::scheduler::NoteOffScheduler;
use std::collections::HashSet;
use std::sync::Mutex;

/// MIDI sender. Owns a connection to a single output port.
pub struct MidiSender {
    config: MidiConfig,
    scheduler: NoteOffScheduler,
    used_channels: Mutex<HashSet<u8>>,
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
            scheduler: NoteOffScheduler::new(conn),
            used_channels: Mutex::new(HashSet::new()),
        })
    }

    /// Send a gate: note-on now, note-off scheduled after gate_length_ms.
    pub fn send_gate(&self, event: GateEvent) {
        let channel = self.config.base_channel + event.voice;
        if channel > 15 {
            eprintln!("warning: voice {} maps to MIDI channel {} (> 15), skipping",
                      event.voice, channel + 1);
            return;
        }

        if let Ok(mut used) = self.used_channels.lock() {
            used.insert(channel);
        }

        let pitch = self.config.pitch;
        let velocity = (event.intensity * 127.0).clamp(1.0, 127.0) as u8;

        let note_on  = [0x90 | channel, pitch, velocity];
        let note_off = [0x80 | channel, pitch, 0];

        self.scheduler.send_now(note_on);
        let fire_at = Instant::now() + Duration::from_millis(self.config.gate_length_ms);
        self.scheduler.schedule(fire_at, note_off);
    }

    /// Send "All Notes Off" and "All Sound Off" on every channel used during
    /// the run. Belt-and-braces: catches any notes that downstream gear thinks
    /// are still on, regardless of whether we sent the note-off ourselves.
    pub fn shutdown(&self) {
        let channels: Vec<u8> = match self.used_channels.lock() {
            Ok(used) => used.iter().copied().collect(),
            Err(_) => return,
        };

        for channel in channels {
            // CC 123 = All Notes Off, CC 120 = All Sound Off.
            // Status byte 0xB0 is Control Change; OR with channel for the channel.
            let all_notes_off = [0xB0 | channel, 123, 0];
            let all_sound_off = [0xB0 | channel, 120, 0];
            self.scheduler.send_now(all_notes_off);
            self.scheduler.send_now(all_sound_off);
        }
    }
}
