//! Scheduler for delayed MIDI messages (note-offs, primarily).

use midir::MidiOutputConnection;
use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Instant;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::thread;
use std::time::Duration;

/// A MIDI message scheduled to fire at a specific wall-clock time.
struct PendingMessage {
    fire_at: Instant,
    /// Raw 3-byte MIDI message (status, data1, data2).
    bytes: [u8; 3],
}

impl Ord for PendingMessage {
    fn cmp(&self, other: &Self) -> Ordering {
        self.fire_at.cmp(&other.fire_at)
    }
}

impl PartialOrd for PendingMessage {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for PendingMessage {}

impl PartialEq for PendingMessage {
    fn eq(&self, other: &Self) -> bool {
        self.fire_at == other.fire_at
    }
}

pub struct NoteOffScheduler {
    /// The actual MIDI connection. Wrapped in Arc<Mutex<_>> so the scheduler
    /// thread can hold a clone and the foreground (send_now) can also use it.
    conn: Arc<Mutex<MidiOutputConnection>>,
    /// Pending messages, ordered by fire_at (earliest first via Reverse).
    heap: Arc<Mutex<BinaryHeap<Reverse<PendingMessage>>>>,
    /// Cleared on shutdown; the worker thread checks it each poll cycle.
    running: Arc<std::sync::atomic::AtomicBool>,
    /// Handle to the worker thread, taken on drop so we can join.
    handle: Option<JoinHandle<()>>,
}

impl NoteOffScheduler {
    /// Wraps an open MIDI connection and starts a background worker thread
    /// that drains the heap in deadline order. Poll interval is 1ms — fine for
    /// MIDI's ~1ms timing resolution and a 50ms gate length.
    pub fn new(conn: MidiOutputConnection) -> Self {
        let conn = Arc::new(Mutex::new(conn));
        let heap = Arc::new(Mutex::new(BinaryHeap::new()));
        let running = Arc::new(AtomicBool::new(true));

        // Clones for the worker thread. The originals stay on `self`.
        let conn_w = Arc::clone(&conn);
        let heap_w = Arc::clone(&heap);
        let running_w = Arc::clone(&running);

        let handle = thread::spawn(move || {
            Self::worker_loop(conn_w, heap_w, running_w);
        });

        Self {
            conn,
            heap,
            running,
            handle: Some(handle),
        }
    }

    /// Worker thread body. Polls the heap every 1ms; sends any messages whose
    /// deadlines have arrived. Exits when `running` is cleared.
    fn worker_loop(
        conn: Arc<Mutex<MidiOutputConnection>>,
        heap: Arc<Mutex<BinaryHeap<Reverse<PendingMessage>>>>,
        running: Arc<AtomicBool>,
    ) {
        loop {
            if !running.load(AtomicOrdering::Acquire) {
                // Shutdown: drain remaining messages immediately so we don't
                // leave hanging note-offs on the rig.
                let remaining: Vec<PendingMessage> = {
                    let mut h = heap.lock().unwrap();
                    h.drain().map(|Reverse(m)| m).collect()
                };
                if let Ok(mut c) = conn.lock() {
                    for msg in remaining {
                        let _ = c.send(&msg.bytes);
                    }
                }
                break;
            }
            
            let now = Instant::now();
            let due: Vec<PendingMessage> = {
                let mut h = heap.lock().unwrap();
                let mut due = Vec::new();
                while let Some(Reverse(top)) = h.peek() {
                    if top.fire_at <= now {
                        // Safe: we just peeked and saw Some.
                        due.push(h.pop().unwrap().0);
                    } else {
                        break;
                    }
                }
                due
            };

            if !due.is_empty() {
                if let Ok(mut c) = conn.lock() {
                    for msg in due {
                        let _ = c.send(&msg.bytes);
                    }
                }
            }

            thread::sleep(Duration::from_millis(1));
        }
    }

    /// Send a MIDI message immediately. Used for note-ons and any other
    /// message that doesn't need to wait.
    pub fn send_now(&self, bytes: [u8; 3]) {
        if let Ok(mut c) = self.conn.lock() {
            let _ = c.send(&bytes);
        }
    }

    /// Queue a MIDI message to fire at `fire_at`. Used for note-offs.
    pub fn schedule(&self, fire_at: Instant, bytes: [u8; 3]) {
        if let Ok(mut h) = self.heap.lock() {
            h.push(Reverse(PendingMessage { fire_at, bytes }));
        }
    }
}

impl Drop for NoteOffScheduler {
    fn drop(&mut self) {
        // Signal the worker to stop. It will drain any remaining messages
        // before exiting (see worker_loop).
        self.running.store(false, AtomicOrdering::Release);
        // Take the handle out of the Option so we can join it (join takes
        // ownership of the JoinHandle).
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}