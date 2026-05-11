//! MIDI voice allocation for domain walls.
//!
//! A wall is a voice with a physics-determined lifetime: born at a position
//! (becoming a held note), possibly moving (Step 6 adds CC tracking), and
//! ending only when the wall annihilates. This module maps WallEvents to
//! note-on / note-off byte streams via MidiSender.
//!
//! Voice allocation is round-robin across a configured channel range. Each
//! wall is monophonic on its own channel — at most one wall at a time per
//! channel. With up to 7 walls in an 8-site chain and 4 default channels,
//! voice stealing is occasionally required; Step 7 adds that. For Step 5,
//! a wall born when no channel is free is silently dropped.

use crate::config::{PhysicsConfig, WallMidiConfig};
use crate::midi::MidiSender;
use crate::walls::WallEvent;
use std::collections::HashMap;

struct ActiveVoice {
    channel: u8,
    pitch: u8,
    born_at_tick: u64,
}

pub struct WallVoiceAllocator {
    config: WallMidiConfig,
    /// Number of sites in the chain — needed to map position to pitch.
    /// Cached at construction; doesn't change at runtime.
    n_sites: usize,
    /// Active voices: wall_id -> (channel, pitch).
    active: HashMap<u64, ActiveVoice>,
    /// Round-robin pointer for the next channel to try.
    next_channel: u8,
}

impl WallVoiceAllocator {
    pub fn new(config: WallMidiConfig, physics: &PhysicsConfig) -> Self {
        let next_channel = config.channel_low;
        Self {
            config,
            n_sites: physics.n_sites,
            active: HashMap::new(),
            next_channel,
        }
    }

    /// Handle one wall event by sending the corresponding MIDI bytes,
    /// and (if `osc_sink` is provided) pushing a matching OSC event.
    pub fn handle(
        &mut self,
        event: &WallEvent,
        sender: &MidiSender,
        osc_sink: Option<&mut crate::osc_io::OscSink>,
    ) {
        match event {
            WallEvent::Created { id, position, tick } => {
                self.handle_created(*id, *position, *tick, sender, osc_sink);
            }
            WallEvent::Destroyed { id, last_position, lifetime_ticks, .. } => {
                self.handle_destroyed(*id, *last_position, *lifetime_ticks, sender, osc_sink);
            }
            WallEvent::Moved { id, from, to, velocity, .. } => {
                self.handle_moved(*id, *from, *to, *velocity, sender, osc_sink);
            }
        }
    }

    fn handle_moved(
        &mut self,
        id: u64,
        from: f64,
        to: f64,
        velocity: f64,
        sender: &MidiSender,
        osc_sink: Option<&mut crate::osc_io::OscSink>,
    ) {
        if self.config.repitch_on_move {
            self.handle_moved_repitch(id, to, sender);
        } else {
            self.handle_moved_cc(id, to, sender);
        }
        if let Some(sink) = osc_sink {
            sink.push(crate::osc::OutboundEvent::WallMoved { id, from, to, velocity });
        }
    }

    fn handle_moved_cc(&self, id: u64, position: f64, sender: &MidiSender) {
        let voice = match self.active.get(&id) {
            Some(v) => v,
            None => return,
        };
        if let Some(cc) = self.config.motion_cc {
            sender.send_cc(voice.channel, cc, self.position_to_cc(position));
        }
    }

    fn handle_moved_repitch(&mut self, id: u64, position: f64, sender: &MidiSender) {
        let new_pitch = self.position_to_pitch(position);

        let voice = match self.active.get(&id) {
            Some(v) => v,
            None => return,
        };

        if voice.channel > 15 {
            return;
        }

        if new_pitch == voice.pitch {
            return;
        }

        let channel = voice.channel;
        let old_pitch = voice.pitch;
        let velocity = 96;

        sender.send_note_off(channel, old_pitch);
        sender.send_note_on(channel, new_pitch, velocity);

        if let Some(v) = self.active.get_mut(&id) {
            v.pitch = new_pitch;
        }
    }

    fn handle_created(
        &mut self,
        id: u64,
        position: f64,
        tick: u64,
        sender: &MidiSender,
        osc_sink: Option<&mut crate::osc_io::OscSink>,
    ) {
        let channel = match self.allocate_channel() {
            Some(c) => c,
            None => match self.steal_oldest(sender) {
                Some(c) => c,
                None => {
                    eprintln!("wall {}: voice stealing failed, dropping note-on", id);
                    return;
                }
            },
        };

        let pitch = self.position_to_pitch(position);
        let velocity = 96;

        if let Some(cc) = self.config.motion_cc {
            if !self.config.repitch_on_move {
                sender.send_cc(channel, cc, self.position_to_cc(position));
            }
        }

        sender.send_note_on(channel, pitch, velocity);
        self.active.insert(id, ActiveVoice { channel, pitch, born_at_tick: tick });

        if let Some(sink) = osc_sink {
            sink.push(crate::osc::OutboundEvent::WallCreated { id, position, channel });
        }
    }

    fn steal_oldest(&mut self, sender: &MidiSender) -> Option<u8> {
        let victim_id = self
            .active
            .iter()
            .min_by_key(|(_, v)| v.born_at_tick)
            .map(|(id, _)| *id)?;

        let victim = self.active.remove(&victim_id)?;
        sender.send_note_off(victim.channel, victim.pitch);
        eprintln!("wall {}: stolen for new voice on channel {}",
                  victim_id, victim.channel + 1);
        Some(victim.channel)
    }

    fn handle_destroyed(
        &mut self,
        id: u64,
        last_position: f64,
        lifetime_ticks: u64,
        sender: &MidiSender,
        osc_sink: Option<&mut crate::osc_io::OscSink>,
    ) {
        if let Some(voice) = self.active.remove(&id) {
            sender.send_note_off(voice.channel, voice.pitch);
        }
        if let Some(sink) = osc_sink {
            sink.push(crate::osc::OutboundEvent::WallDestroyed {
                id,
                last_position,
                lifetime_ticks,
            });
        }
    }

    /// Find a free channel in the configured range. Round-robin, starts from
    /// `next_channel` and walks the range. Returns None if all are occupied.
    fn allocate_channel(&mut self) -> Option<u8> {
        let used: std::collections::HashSet<u8> =
            self.active.values().map(|v| v.channel).collect();

        let range_size = (self.config.channel_high - self.config.channel_low + 1) as usize;
        let mut tries = 0;
        let mut ch = self.next_channel;

        while tries < range_size {
            if !used.contains(&ch) {
                // Found one. Advance the pointer for next time.
                self.next_channel = self.advance(ch);
                return Some(ch);
            }
            ch = self.advance(ch);
            tries += 1;
        }

        None
    }

    fn advance(&self, ch: u8) -> u8 {
        if ch >= self.config.channel_high {
            self.config.channel_low
        } else {
            ch + 1
        }
    }

    /// Map a wall position in [0.5, n_sites - 1.5] to a MIDI pitch in
    /// [pitch_low, pitch_high]. Linear, then clamped.
    fn position_to_pitch(&self, position: f64) -> u8 {
        let pos_min = 0.5;
        let pos_max = (self.n_sites - 1) as f64 - 0.5;
        let pos_range = (pos_max - pos_min).max(0.001);

        let normalized = ((position - pos_min) / pos_range).clamp(0.0, 1.0);

        let pitch_range = self.config.pitch_high as f64 - self.config.pitch_low as f64;
        let pitch_f = self.config.pitch_low as f64 + normalized * pitch_range;

        pitch_f.round().clamp(0.0, 127.0) as u8
    }

    /// Map a wall position to a 7-bit CC value (0..=127). Linear across the chain.
    fn position_to_cc(&self, position: f64) -> u8 {
        let span = (self.n_sites - 1) as f64;
        let normalized = (position / span).clamp(0.0, 1.0);
        (normalized * 127.0).round() as u8
    }
}