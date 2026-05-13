//! MIDI output. Wraps midir to send note-on/note-off pairs for each GateEvent.

use crate::config::MidiConfig;
use crate::events::GateEvent;
use crate::scheduler::NoteOffScheduler;
use midir::MidiOutput;
use std::collections::HashSet;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// MIDI sender. Owns a connection to a single output port.
pub struct MidiSender {
    config: MidiConfig,
    scheduler: NoteOffScheduler,
    used_channels: Mutex<HashSet<u8>>,
    /// For mono priority: currently-sounding pitch on each channel, if any.
    /// Indexed by channel (0..16). None if nothing is sounding.
    sounding: Mutex<[Option<u8>; 16]>,
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

        let port = ports.get(port_index).ok_or_else(|| {
            format!(
                "port index {} out of range (found {} ports)",
                port_index,
                ports.len()
            )
        })?;

        let port_name = midi_out.port_name(port)?;
        println!("Opening MIDI port [{}]: {}", port_index, port_name);

        let conn = midi_out.connect(port, "crystallized_time-out")?;

        Ok(Self {
            config,
            scheduler: NoteOffScheduler::new(conn),
            used_channels: Mutex::new(HashSet::new()),
            sounding: Mutex::new([None; 16]),
        })
    }

    /// Send a gate: note-on now, note-off scheduled after gate_length_ms.
    ///
    /// Per-voice routing: channel and pitch come from the config vecs
    /// indexed by `event.voice`. Mono priority applies per-channel —
    /// if another voice is currently sounding on the same channel, its
    /// note-off fires immediately. Voices on distinct channels don't
    /// interact, which reproduces the historic ChannelPerSite behavior
    /// without a mode flag.
    pub fn send_gate(&self, event: GateEvent) {
        let voice = event.voice as usize;

        // Look up channel and pitch from the parallel vecs. If the event
        // refers to a voice the config doesn't define (shouldn't happen
        // post-loader, but guard anyway), drop silently — better than
        // panicking in the realtime path.
        let channel = match self.config.voice_channels.get(voice) {
            Some(&c) if c <= 15 => c,
            _ => {
                eprintln!(
                    "warning: voice {} has no valid channel (config has {} entries)",
                    event.voice,
                    self.config.voice_channels.len()
                );
                return;
            }
        };
        let pitch = match self.config.voice_pitches.get(voice) {
            Some(&p) => p,
            None => {
                eprintln!(
                    "warning: voice {} has no pitch defined (config has {} entries)",
                    event.voice,
                    self.config.voice_pitches.len()
                );
                return;
            }
        };

        if let Ok(mut used) = self.used_channels.lock() {
            used.insert(channel);
        }

        let velocity = (event.intensity * 127.0).clamp(1.0, 127.0) as u8;

        // Mono priority per channel: if something's already sounding on
        // this channel, retire it before sending the new note-on. Voices
        // on distinct channels never trip this branch.
        if let Ok(mut sounding) = self.sounding.lock() {
            if let Some(prior_pitch) = sounding[channel as usize].take() {
                let prior_off = [0x80 | channel, prior_pitch, 0];
                self.scheduler.send_now(prior_off);
            }
            sounding[channel as usize] = Some(pitch);
        }

        let note_on = [0x90 | channel, pitch, velocity];
        let note_off = [0x80 | channel, pitch, 0];

        self.scheduler.send_now(note_on);
        let fire_at = Instant::now() + Duration::from_millis(self.config.gate_length_ms);
        self.scheduler.schedule(fire_at, note_off);
    }

    /// Send a clock-style gate pulse on a specific channel. Used by the
    /// substrate clock; bypasses voice routing and mono-priority logic.
    pub fn send_clock_pulse(&self, channel: u8, pitch: u8, gate_length_ms: u64) {
        if channel > 15 {
            return;
        }

        if let Ok(mut used) = self.used_channels.lock() {
            used.insert(channel);
        }

        let note_on = [0x90 | channel, pitch & 0x7F, 100];
        let note_off = [0x80 | channel, pitch & 0x7F, 0];

        self.scheduler.send_now(note_on);
        let fire_at = Instant::now() + Duration::from_millis(gate_length_ms);
        self.scheduler.schedule(fire_at, note_off);
    }

    /// Send a Control Change immediately on the given channel.
    pub fn send_cc(&self, channel: u8, cc_number: u8, value: u8) {
        if channel > 15 {
            return;
        }
        if let Ok(mut used) = self.used_channels.lock() {
            used.insert(channel);
        }
        let bytes = [0xB0 | channel, cc_number & 0x7F, value & 0x7F];
        self.scheduler.send_now(bytes);
    }

    /// Send a note-on immediately. The caller is responsible for sending the
    /// matching note-off later. Used for walls, where note-off timing is
    /// determined by physics (wall destruction), not by a fixed gate length.
    pub fn send_note_on(&self, channel: u8, pitch: u8, velocity: u8) {
        if channel > 15 {
            return;
        }
        if let Ok(mut used) = self.used_channels.lock() {
            used.insert(channel);
        }
        let bytes = [0x90 | channel, pitch & 0x7F, velocity & 0x7F];
        self.scheduler.send_now(bytes);
    }

    /// Send a note-off immediately.
    pub fn send_note_off(&self, channel: u8, pitch: u8) {
        if channel > 15 {
            return;
        }
        let bytes = [0x80 | channel, pitch & 0x7F, 0];
        self.scheduler.send_now(bytes);
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
            let all_notes_off = [0xB0 | channel, 123, 0];
            let all_sound_off = [0xB0 | channel, 120, 0];
            self.scheduler.send_now(all_notes_off);
            self.scheduler.send_now(all_sound_off);
        }
    }
}