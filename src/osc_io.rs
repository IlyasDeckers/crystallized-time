//! OSC I/O — the receiver thread.

use crate::config::PhysicsTargets;
use crate::osc::{extract_messages, InboundMessage, InboundTarget};
use rosc::decoder::MTU;
use std::net::UdpSocket;
use std::sync::{Arc, RwLock};
use std::thread;
use crate::osc::{serialize_bundle, OutboundEvent};
use std::net::SocketAddr;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use crystallized_time::chain_id::ChainId;
use crate::runtime::CouplingTargets;

pub type OutboundBundle = Vec<OutboundEvent>;

/// Bundle of OSC-writable targets, shared between the receiver thread
/// and the simulation. Holds per-chain physics targets plus global
/// coupling targets.
#[derive(Clone)]
pub struct OscTargets {
    pub physics_a: Arc<RwLock<PhysicsTargets>>,
    pub physics_b: Option<Arc<RwLock<PhysicsTargets>>>,
    pub coupling: Option<Arc<RwLock<CouplingTargets>>>,
}

impl OscTargets {
    pub fn new(
        physics_a: Arc<RwLock<PhysicsTargets>>,
        physics_b: Option<Arc<RwLock<PhysicsTargets>>>,
        coupling: Option<Arc<RwLock<CouplingTargets>>>,
    ) -> Self {
        Self {
            physics_a,
            physics_b,
            coupling,
        }
    }

    fn get_physics(&self, chain: ChainId) -> Option<&Arc<RwLock<PhysicsTargets>>> {
        match chain {
            ChainId::A => Some(&self.physics_a),
            ChainId::B => self.physics_b.as_ref(),
        }
    }
}

pub fn spawn_receiver(
    port: u16,
    targets: OscTargets,
) -> std::io::Result<u16> {
    let socket = UdpSocket::bind(("0.0.0.0", port))?;
    let bound_port = socket.local_addr()?.port();

    thread::spawn(move || {
        receiver_loop(socket, targets);
    });

    Ok(bound_port)
}

fn receiver_loop(socket: UdpSocket, targets: OscTargets) {
    let mut buf = [0u8; MTU];

    loop {
        let (size, _from) = match socket.recv_from(&mut buf) {
            Ok(pair) => pair,
            Err(_) => continue,
        };

        let packet = match rosc::decoder::decode_udp(&buf[..size]) {
            Ok((_remaining, pkt)) => pkt,
            Err(_) => continue,
        };

        let messages = extract_messages(packet);
        if messages.is_empty() {
            continue;
        }

        for msg in messages {
            apply(&targets, msg);
        }
    }
}

/// Apply one parsed message to whichever chain(s) it targets.
fn apply(targets: &OscTargets, msg: InboundMessage) {
    match msg {
        InboundMessage::SetKt(t, v)  => apply_physics(targets, t, ParamKind::Kt,  v),
        InboundMessage::SetEps(t, v) => apply_physics(targets, t, ParamKind::Eps, v),
        InboundMessage::SetJ(t, v)   => apply_physics(targets, t, ParamKind::J,   v),
        InboundMessage::SetW(t, v)   => apply_physics(targets, t, ParamKind::W,   v),
        InboundMessage::SetCouplingStrengthAB(v) => {
            apply_coupling(targets, CouplingField::AB, v);
        }
        InboundMessage::SetCouplingStrengthBA(v) => {
            apply_coupling(targets, CouplingField::BA, v);
        }
    }
}

fn apply_physics(targets: &OscTargets, target: InboundTarget, kind: ParamKind, raw: f64) {
    let chains: &[ChainId] = match target {
        InboundTarget::Chain(c) => match c {
            ChainId::A => &[ChainId::A],
            ChainId::B => &[ChainId::B],
        },
        InboundTarget::Both => &[ChainId::A, ChainId::B],
    };

    for chain in chains {
        let Some(lock) = targets.get_physics(*chain) else { continue };
        let mut t = match lock.write() {
            Ok(g) => g,
            Err(_) => {
                eprintln!("warning: {:?} physics targets lock poisoned; dropping OSC write", chain);
                continue;
            }
        };
        write_physics(&mut t, kind, raw);
    }
}

fn apply_coupling(targets: &OscTargets, field: CouplingField, raw: f64) {
    let Some(lock) = targets.coupling.as_ref() else {
        return;
    };
    let mut t = match lock.write() {
        Ok(g) => g,
        Err(_) => {
            eprintln!("warning: coupling targets lock poisoned; dropping OSC write");
            return;
        }
    };
    let clamped = CouplingTargets::clamp_strength(raw);
    match field {
        CouplingField::AB => t.strength_ab = clamped,
        CouplingField::BA => t.strength_ba = clamped,
    }
}

#[derive(Clone, Copy)]
enum ParamKind { Kt, Eps, J, W }

#[derive(Clone, Copy)]
enum CouplingField { AB, BA }

fn write_physics(targets: &mut PhysicsTargets, kind: ParamKind, raw: f64) {
    match kind {
        ParamKind::Kt  => targets.kt  = PhysicsTargets::clamp_kt(raw),
        ParamKind::Eps => targets.eps = PhysicsTargets::clamp_eps(raw),
        ParamKind::J   => targets.j   = PhysicsTargets::clamp_j(raw),
        ParamKind::W   => targets.w   = PhysicsTargets::clamp_w(raw),
    }
}

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

pub struct OscSink {
    tx: SyncSender<OutboundBundle>,
    staging: Vec<OutboundEvent>,
    state_throttle: Option<StateThrottle>,
}

struct StateThrottle {
    min_interval: std::time::Duration,
    last_send: std::collections::HashMap<ChainId, std::time::Instant>,
}

impl OscSink {
    pub fn new(tx: SyncSender<OutboundBundle>, config: &crate::config::OscConfig) -> Self {
        let state_throttle = if config.enable_state {
            if config.state_rate_hz > 0.0 {
                Some(StateThrottle {
                    min_interval: std::time::Duration::from_secs_f64(1.0 / config.state_rate_hz),
                    last_send: std::collections::HashMap::new(),
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

    pub fn push(&mut self, event: OutboundEvent) {
        self.staging.push(event);
    }

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
        if let Some(last) = throttle.last_send.get(&chain) {
            if now.duration_since(*last) < throttle.min_interval {
                return;
            }
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

        throttle.last_send.insert(chain, now);
    }

    pub fn flush_tick(&mut self) {
        if self.staging.is_empty() {
            return;
        }

        let bundle = std::mem::take(&mut self.staging);
        match self.tx.try_send(bundle) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {}
            Err(TrySendError::Disconnected(_)) => {}
        }
    }
}