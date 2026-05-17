//! OSC I/O — the receiver thread.

use crate::config::PhysicsTargets;
use crate::osc::{extract_messages, InboundMessage, InboundTarget};
use crate::tui::{LogSource, TuiState};
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

/// Change-filter thresholds for state messages. Hardcoded for now since
/// they're internal tuning, not user-facing knobs. Promote to TOML if
/// experience says they need to vary by patch.
///
/// `MAG_EPSILON` is the change in mean magnetization required to emit a
/// new `/state/magnetization`. 0.005 corresponds to less than 1 unit on
/// a 7-bit MIDI CC scale, which is below the resolution most receivers
/// can act on, so smaller changes carry no information.
///
/// `SPINS_EPSILON` is the max per-component change required to emit a
/// new `/state/spins`. Same idea; chosen slightly larger because individual
/// spins are noisier than the chain mean.
const MAG_EPSILON: f32 = 0.005;
const SPINS_EPSILON: f32 = 0.01;


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
    tui_state: Option<Arc<TuiState>>,
) -> std::io::Result<u16> {
    let socket = UdpSocket::bind(("0.0.0.0", port))?;
    let bound_port = socket.local_addr()?.port();

    thread::spawn(move || {
        receiver_loop(socket, targets, tui_state);
    });

    Ok(bound_port)
}

fn receiver_loop(socket: UdpSocket, targets: OscTargets, tui_state: Option<Arc<TuiState>>) {
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
            if let Some(ref tui) = tui_state {
                let content = format_inbound_message(&msg);
                tui.push_log(LogSource::Osc, content);
            }
            apply(&targets, msg);
        }
    }
}

fn format_inbound_message(msg: &InboundMessage) -> String {
    match msg {
        InboundMessage::SetKt(InboundTarget::Chain(c), v) => {
            format!("{}/kt {:.3}", c.osc_prefix(), v)
        }
        InboundMessage::SetEps(InboundTarget::Chain(c), v) => {
            format!("{}/eps {:.3}", c.osc_prefix(), v)
        }
        InboundMessage::SetJ(InboundTarget::Chain(c), v) => {
            format!("{}/j {:.3}", c.osc_prefix(), v)
        }
        InboundMessage::SetW(InboundTarget::Chain(c), v) => {
            format!("{}/w {:.3}", c.osc_prefix(), v)
        }
        InboundMessage::SetKt(InboundTarget::Both, v) => {
            format!("/both/kt {:.3}", v)
        }
        InboundMessage::SetEps(InboundTarget::Both, v) => {
            format!("/both/eps {:.3}", v)
        }
        InboundMessage::SetJ(InboundTarget::Both, v) => {
            format!("/both/j {:.3}", v)
        }
        InboundMessage::SetW(InboundTarget::Both, v) => {
            format!("/both/w {:.3}", v)
        }
        InboundMessage::SetCouplingStrengthAB(v) => {
            format!("/coupling/strength_ab {:.3}", v)
        }
        InboundMessage::SetCouplingStrengthBA(v) => {
            format!("/coupling/strength_ba {:.3}", v)
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

/// Per-chain state-message gating.
///
/// Each scalar gets a wall-clock rate cap *and* a change filter; nothing
/// is sent unless both gates open. `state_rate_hz` controls magnetization
/// and wall_count; `state_spins_rate_hz` controls the heavier spin vector.
///
/// `wall_count` has no rate cap of its own: it's a small integer that
/// changes rarely and is interesting precisely when it changes, so we
/// just send-on-change.
struct StateThrottle {
    /// Min wall-clock interval between magnetization sends. None disables
    /// the rate gate (change filter still applies).
    scalar_min_interval: Option<std::time::Duration>,
    /// Min wall-clock interval between spin-vector sends. None disables
    /// the rate gate.
    spins_min_interval: Option<std::time::Duration>,

    /// Per-chain last-send timestamps and last-sent values for each
    /// state stream.
    last_mag_send: std::collections::HashMap<ChainId, std::time::Instant>,
    last_spins_send: std::collections::HashMap<ChainId, std::time::Instant>,

    last_mag: std::collections::HashMap<ChainId, f32>,
    last_wall_count: std::collections::HashMap<ChainId, i32>,
    last_spins: std::collections::HashMap<ChainId, Vec<f32>>,
}

impl StateThrottle {
    fn from_config(config: &crate::config::OscConfig) -> Self {
        let interval_from_hz = |hz: f64| {
            if hz > 0.0 {
                Some(std::time::Duration::from_secs_f64(1.0 / hz))
            } else {
                None
            }
        };
        Self {
            scalar_min_interval: interval_from_hz(config.state_rate_hz),
            spins_min_interval: interval_from_hz(config.state_spins_rate_hz),
            last_mag_send: std::collections::HashMap::new(),
            last_spins_send: std::collections::HashMap::new(),
            last_mag: std::collections::HashMap::new(),
            last_wall_count: std::collections::HashMap::new(),
            last_spins: std::collections::HashMap::new(),
        }
    }

    /// Has the min interval passed since this chain's last send for the
    /// given stream? `None` interval means "no rate gate, always due".
    fn rate_gate_open(
        last_send: &std::collections::HashMap<ChainId, std::time::Instant>,
        interval: Option<std::time::Duration>,
        chain: ChainId,
        now: std::time::Instant,
    ) -> bool {
        let Some(interval) = interval else { return true };
        match last_send.get(&chain) {
            Some(last) => now.duration_since(*last) >= interval,
            None => true,
        }
    }

    /// Has any spin component changed by at least `SPINS_EPSILON` since
    /// the last sent vector? Different lengths count as a change.
    fn spins_changed(&self, chain: ChainId, current: &[f32]) -> bool {
        match self.last_spins.get(&chain) {
            None => true,
            Some(prev) if prev.len() != current.len() => true,
            Some(prev) => {
                prev.iter()
                    .zip(current.iter())
                    .any(|(a, b)| (a - b).abs() >= SPINS_EPSILON)
            }
        }
    }
}

impl OscSink {
    pub fn new(tx: SyncSender<OutboundBundle>, config: &crate::config::OscConfig) -> Self {
        let state_throttle = if config.enable_state {
            Some(StateThrottle::from_config(config))
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

    /// Push whichever state streams are due AND have changed enough since
    /// the last send. Each stream is gated independently: a chain whose
    /// magnetization is steady but whose wall_count just changed will send
    /// only the wall_count message.
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
        let mag_f32 = magnetization as f32;
        let wall_count_i32 = wall_count.min(i32::MAX as usize) as i32;

        // Magnetization: rate-limited and change-filtered.
        let mag_rate_open = StateThrottle::rate_gate_open(
            &throttle.last_mag_send,
            throttle.scalar_min_interval,
            chain,
            now,
        );
        let mag_changed = match throttle.last_mag.get(&chain) {
            None => true,
            Some(last) => (last - mag_f32).abs() >= MAG_EPSILON,
        };
        if mag_rate_open && mag_changed {
            self.staging.push(OutboundEvent::StateMagnetization {
                chain,
                magnetization: mag_f32,
            });
            throttle.last_mag_send.insert(chain, now);
            throttle.last_mag.insert(chain, mag_f32);
        }

        // wall_count: change-only, no rate gate. wall_count is small and
        // discrete; redundant sends are pure waste and changes are always
        // worth surfacing immediately.
        let wall_changed = throttle
            .last_wall_count
            .get(&chain)
            .is_none_or(|last| *last != wall_count_i32);
        if wall_changed {
            self.staging.push(OutboundEvent::StateWallCount {
                chain,
                count: wall_count_i32,
            });
            throttle.last_wall_count.insert(chain, wall_count_i32);
        }

        // Spins: rate-limited (with its own slower rate by default) and
        // change-filtered. We allocate the f32 vector unconditionally
        // since we need it to compare; if neither gate opens we just drop
        // it before pushing.
        let values: Vec<f32> = spins.iter().map(|v| *v as f32).collect();
        let spins_rate_open = StateThrottle::rate_gate_open(
            &throttle.last_spins_send,
            throttle.spins_min_interval,
            chain,
            now,
        );
        let spins_changed = throttle.spins_changed(chain, &values);
        if spins_rate_open && spins_changed {
            throttle.last_spins.insert(chain, values.clone());
            self.staging.push(OutboundEvent::StateSpins { chain, values });
            throttle.last_spins_send.insert(chain, now);
        }
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