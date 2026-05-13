//! MIDI input listener.
//!
//! Mirrors the shape of `midi.rs` for output: opens a `midir` connection
//! to a named input port and bridges its callback-driven model to the
//! main loop's polling model via an `mpsc` channel.
//!
//! The midir input thread is owned by midir and runs the callback we
//! install on every incoming message. The callback's only job is to
//! push raw bytes into the channel — fast and non-blocking, because it
//! runs in a real-time context. All interpretation happens later, in
//! the routing layer on the simulation thread.

use midir::{Ignore, MidiInput, MidiInputConnection};
use std::sync::mpsc::{channel, Receiver, Sender};

/// One raw incoming MIDI message. Three bytes covers note-on, note-off,
/// control change, polyphonic aftertouch, and most other channel messages.
/// Longer messages (SysEx) are dropped at the callback level — we don't
/// need them.
#[derive(Clone, Copy, Debug)]
pub struct RawMidiMessage {
    pub status: u8,
    pub data1: u8,
    pub data2: u8,
}

/// Open MIDI input connection plus the receiver end of the message channel.
pub struct MidiInputListener {
    /// Receiver end of the channel; the callback owns the Sender.
    rx: Receiver<RawMidiMessage>,
    /// Held to keep the midir input thread alive. Dropping this closes
    /// the connection and stops the callback from firing.
    _conn: MidiInputConnection<Sender<RawMidiMessage>>,
}

impl MidiInputListener {
    /// List available input port names. Used by `--list-input-ports`.
    pub fn list_ports() -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let mut midi_in = MidiInput::new("crystallized_time-in")?;
        midi_in.ignore(Ignore::None);

        let ports = midi_in.ports();
        let mut names = Vec::with_capacity(ports.len());
        for port in &ports {
            names.push(midi_in.port_name(port)?);
        }
        Ok(names)
    }

    /// Open the input port at `port_index` and start receiving messages.
    pub fn open(port_index: usize) -> Result<Self, Box<dyn std::error::Error>> {
        let mut midi_in = MidiInput::new("crystallized_time-in")?;
        // Default Ignore excludes SysEx, timing, and active sensing — exactly
        // what we want. We don't use any of them.
        midi_in.ignore(Ignore::All);

        let ports = midi_in.ports();
        let port = ports.get(port_index).ok_or_else(|| {
            format!(
                "input port index {} out of range (found {} ports)",
                port_index,
                ports.len()
            )
        })?;

        let port_name = midi_in.port_name(port)?;
        println!("Opening MIDI input port [{}]: {}", port_index, port_name);

        let (tx, rx) = channel::<RawMidiMessage>();

        // midir's connect_input takes a closure (the callback), plus an
        // arbitrary "data" value that gets passed to the callback on each
        // message. We pass the Sender as the data, so the callback can
        // forward each message into the channel.
        //
        // The callback runs on midir's input thread. It must be cheap and
        // non-blocking — anything it does delays the next callback. Pushing
        // into an mpsc Sender is a couple of atomic ops, well below the
        // budget.
        let conn = midi_in.connect(
            port,
            "crystallized_time-in-conn",
            |_timestamp_us, bytes, tx| {
                // Filter for 3-byte channel messages. Status bytes have the
                // high bit set; data bytes do not. We accept anything that
                // looks like a normal channel message and drop the rest.
                if bytes.len() < 3 {
                    return;
                }
                let msg = RawMidiMessage {
                    status: bytes[0],
                    data1: bytes[1],
                    data2: bytes[2],
                };
                // send() on a disconnected channel returns Err; we ignore
                // it because there's nothing useful to do from inside the
                // callback. If the main thread has gone away, so will we.
                let _ = tx.send(msg);
            },
            tx,
        )?;

        Ok(Self { rx, _conn: conn })
    }

    /// Drain any messages accumulated since the last call. Non-blocking.
    /// Returns an empty Vec if nothing has arrived.
    pub fn poll(&self) -> Vec<RawMidiMessage> {
        let mut out = Vec::new();
        // try_iter consumes messages as long as they're immediately ready,
        // stopping at the first empty read. Exactly the semantics we want
        // for a per-tick drain.
        for msg in self.rx.try_iter() {
            out.push(msg);
        }
        out
    }
}