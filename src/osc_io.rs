//! OSC I/O — the receiver thread.
//!
//! Owns a UDP socket bound to the configured port. Loops on recv_from,
//! decodes each packet via the `osc` module, and writes resolved values
//! into the shared `PhysicsTargets` under its RwLock.
//!
//! Threading model per spec: a regular spawned thread, never joined.
//! The thread blocks on `recv_from` indefinitely and is terminated by
//! process exit. No graceful shutdown signal yet — adding one would
//! require non-blocking sockets and a poll loop, which the spec defers.
//!
//! Outbound (sender) thread comes in Steps 4–5; this file will gain a
//! `spawn_sender` function at that point.

use crate::config::PhysicsTargets;
use crate::osc::{extract_messages, InboundMessage};
use rosc::decoder::MTU;
use std::net::UdpSocket;
use std::sync::{Arc, RwLock};
use std::thread;
use crate::osc::{serialize_bundle, OutboundEvent};
use std::net::SocketAddr;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use crystallized_time::chain_id::ChainId;

/// One tick's worth of outbound events, bundled together for the sender
/// thread. Aliased for readability at the call sites that move these
/// across the channel.
pub type OutboundBundle = Vec<OutboundEvent>;

/// Bind a UDP socket to `port` on all interfaces and spawn a thread that
/// drains it forever, applying each recognized message to `targets`.
///
/// Returns the port the socket actually bound to (always the requested
/// port — we don't fall back), or an error if the bind failed. Callers
/// typically print this on startup as the spec's one-time
/// "OSC: listening on port N" line and never interact with the thread
/// again.
pub fn spawn_receiver(
    port: u16,
    targets: Arc<RwLock<PhysicsTargets>>,
) -> std::io::Result<u16> {
    let socket = UdpSocket::bind(("0.0.0.0", port))?;
    let bound_port = socket.local_addr()?.port();

    thread::spawn(move || {
        receiver_loop(socket, targets);
    });

    Ok(bound_port)
}

fn receiver_loop(socket: UdpSocket, targets: Arc<RwLock<PhysicsTargets>>) {
    let mut buf = [0u8; MTU];

    loop {
        let (size, _from) = match socket.recv_from(&mut buf) {
            Ok(pair) => pair,
            Err(_) => {
                // Transient recv error. Don't log (would spam on a
                // persistent fault); just loop and try again. If the
                // socket is permanently broken the thread will spin
                // here, which is acceptable for a localhost dev tool.
                continue;
            }
        };

        let packet = match rosc::decoder::decode_udp(&buf[..size]) {
            Ok((_remaining, pkt)) => pkt,
            Err(_) => continue,
        };

        let messages = extract_messages(packet);
        if messages.is_empty() {
            continue;
        }

        // Acquire the write lock once per packet, not once per message.
        // Bundles may contain several writes; we want them visible to
        // the sim thread atomically as a group.
        let mut t = match targets.write() {
            Ok(g) => g,
            Err(_) => {
                eprintln!("warning: physics targets lock poisoned; dropping OSC writes");
                continue;
            }
        };

        for msg in messages {
            apply(&mut t, msg);
        }
    }
}

/// Apply one parsed message to the targets, clamping to per-parameter
/// bounds. Clamping is silent — TouchDesigner sliders can easily
/// produce out-of-range values during normal use, and logging each
/// clamp would flood stderr.
fn apply(targets: &mut PhysicsTargets, msg: InboundMessage) {
    match msg {
        InboundMessage::SetKt(v)  => targets.kt  = PhysicsTargets::clamp_kt(v),
        InboundMessage::SetEps(v) => targets.eps = PhysicsTargets::clamp_eps(v),
        InboundMessage::SetJ(v)   => targets.j   = PhysicsTargets::clamp_j(v),
        InboundMessage::SetW(v)   => targets.w   = PhysicsTargets::clamp_w(v),
    }
}

/// Bind an outbound UDP socket, parse the destination, and spawn the
/// sender thread. Returns the channel the simulation thread uses to
/// push bundles.
///
/// The channel is bounded (16 bundles, ~320ms of buffering per spec).
/// If the sender thread can't keep up, the sim thread drops bundles
/// rather than blocking or growing memory without bound.
pub fn spawn_sender(send_addr: &str) -> std::io::Result<SyncSender<OutboundBundle>> {
    let dest: SocketAddr = send_addr.parse().map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("invalid OSC send address '{}': {}", send_addr, e),
        )
    })?;
    
    let socket = UdpSocket::bind("0.0.0.0:0")?;

    let (tx, rx) = sync_channel::<OutboundBundle>(16);

    thread::spawn(move || {
        sender_loop(socket, dest, rx);
    });

    Ok(tx)
}

fn sender_loop(socket: UdpSocket, dest: SocketAddr, rx: Receiver<OutboundBundle>) {
    while let Ok(events) = rx.recv() {
        let bytes = serialize_bundle(&events);
        if bytes.is_empty() {
            continue;
        }

        let _ = socket.send_to(&bytes, dest);
    }
}

/// The simulation thread's handle for pushing events out via OSC.
///
/// Owns a reusable staging buffer so per-tick allocation is zero when
/// no events fire (the common case in the locked phase). `push` adds
/// to the buffer; `flush_tick` ships it as one bundle and clears it.
pub struct OscSink {
    tx: SyncSender<OutboundBundle>,
    staging: Vec<OutboundEvent>,
    /// Throttle state for /state/* messages. None means state pushing
    /// is disabled (caller passed enable_state=false).
    state_throttle: Option<StateThrottle>,
}

struct StateThrottle {
    /// Minimum wall-clock interval between state pushes.
    min_interval: std::time::Duration,
    /// Time of the last state push. Initialized to Instant::now() at
    /// construction so the first tick after startup is honored — the
    /// first push will be one min_interval after the sink is built,
    /// which is the natural debouncing behavior.
    last_send: std::time::Instant,
}

impl OscSink {
    pub fn new(tx: SyncSender<OutboundBundle>, config: &crate::config::OscConfig) -> Self {
        let state_throttle = if config.enable_state {
            if config.state_rate_hz > 0.0 {
                Some(StateThrottle {
                    min_interval: std::time::Duration::from_secs_f64(
                        1.0 / config.state_rate_hz,
                    ),
                    last_send: std::time::Instant::now(),
                })
            } else {
                None
            }
        } else {
            None
        };

        Self {
            tx,
            staging: Vec::with_capacity(16),
            state_throttle,
        }
    }

    /// Add one event to the current tick's staging buffer. Cheap; no
    /// allocation unless the buffer needs to grow.
    pub fn push(&mut self, event: OutboundEvent) {
        self.staging.push(event);
    }

    /// Push the three state messages if the wall-clock throttle says
    /// one is due. No-op if state pushing is disabled or the throttle
    /// hasn't elapsed yet.
    ///
    /// `spins` is the chain's current per-site sigma_z values, cast to
    /// f32 internally. `magnetization` is the mean. `wall_count` is the
    /// number of currently-active walls.
    pub fn push_state_if_due(
        &mut self,
        chain: ChainId,
        spins: &[f64],
        magnetization: f64,
        wall_count: usize,
    ) {
        let throttle = match &mut self.state_throttle {
            Some(t) => t,
            None => return,
        };

        let now = std::time::Instant::now();
        if now.duration_since(throttle.last_send) < throttle.min_interval {
            return;
        }

        let values: Vec<f32> = spins.iter().map(|v| *v as f32).collect();
        self.staging.push(OutboundEvent::StateSpins { chain, values });
        self.staging.push(OutboundEvent::StateMagnetization {
            chain,
            magnetization: magnetization as f32,
        });
        self.staging.push(OutboundEvent::StateWallCount {
            chain,
            count: wall_count.min(i32::MAX as usize) as i32,
        });

        throttle.last_send = now;
    }

    /// Ship the current tick's events as one bundle and clear the
    /// buffer for the next tick. No-op if nothing was pushed.
    ///
    /// On channel-full, the bundle is dropped (and the staging buffer
    /// still cleared, so we don't accumulate). Drops are silent per
    /// spec — they're rare and per-tick logging would spam.
    pub fn flush_tick(&mut self) {
        if self.staging.is_empty() {
            return;
        }

        let bundle = std::mem::take(&mut self.staging);
        match self.tx.try_send(bundle) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                // Sender thread is behind. Drop this bundle; the next
                // tick gets a fresh empty staging buffer either way.
            }
            Err(TrySendError::Disconnected(_)) => {
                // Sender thread is gone. Nothing to do; future flushes
                // will fail the same way. The sim continues normally.
            }
        }
    }
}