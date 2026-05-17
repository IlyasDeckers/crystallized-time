//! OSC message types and parsing for the inbound control surface.
//!
//! This module is pure data — no I/O, no threads, no sockets. It maps
//! raw `rosc::OscPacket` values onto our internal `InboundMessage` enum,
//! discarding anything malformed or unrecognized.

use rosc::{encoder, OscBundle, OscMessage, OscPacket, OscTime, OscType};
use crystallized_time::chain_id::ChainId;

const TIME_IMMEDIATE: OscTime = OscTime { seconds: 0, fractional: 1 };

/// Which chain(s) an inbound write targets.
///
/// `Both` is what unprefixed `/physics/...` writes produce. Useful for
/// TouchDesigner patches that don't know about chain B yet, and for
/// "move both chains together" gestures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InboundTarget {
    Chain(ChainId),
    Both,
}

#[derive(Debug, Clone, Copy)]
pub enum InboundMessage {
    SetKt(InboundTarget, f64),
    SetEps(InboundTarget, f64),
    SetJ(InboundTarget, f64),
    SetW(InboundTarget, f64),
    SetCouplingStrengthAB(f64),
    SetCouplingStrengthBA(f64),
}

#[derive(Debug, Clone)]
pub enum OutboundEvent {
    WallCreated {
        chain: ChainId,
        id: u64,
        position: f64,
        channel: u8,
    },
    WallDestroyed {
        chain: ChainId,
        id: u64,
        last_position: f64,
        lifetime_ticks: u64,
    },
    WallMoved {
        chain: ChainId,
        id: u64,
        from: f64,
        to: f64,
        velocity: f64,
    },
    SiteEvent {
        chain: ChainId,
        site: u8,
        voice: u8,
        intensity: f32,
    },
    ClockPulse {
        chain: ChainId,
        magnetization: f64,
    },
    StateSpins {
        chain: ChainId,
        values: Vec<f32>,
    },
    StateMagnetization {
        chain: ChainId,
        magnetization: f32,
    },
    StateWallCount {
        chain: ChainId,
        count: i32,
    },
}

pub fn extract_messages(packet: OscPacket) -> Vec<InboundMessage> {
    let mut out = Vec::new();
    walk(packet, &mut out);
    out
}

fn walk(packet: OscPacket, out: &mut Vec<InboundMessage>) {
    match packet {
        OscPacket::Message(msg) => {
            // Coupling addresses can fan out to multiple messages;
            // physics addresses always produce zero or one.
            if let Some(messages) = parse_coupling(&msg) {
                out.extend(messages);
            } else if let Some(inbound) = parse_message(&msg) {
                out.push(inbound);
            }
        }
        OscPacket::Bundle(bundle) => {
            for inner in bundle.content {
                walk(inner, out);
            }
        }
    }
}

/// Parse coupling addresses. Returns Some(messages) if the address
/// is in the coupling namespace (including malformed messages, which
/// return an empty Vec — that way the caller knows not to fall through
/// to the physics parser for /coupling/wrong_param). Returns None
/// only when the address isn't ours at all.
fn parse_coupling(msg: &OscMessage) -> Option<Vec<InboundMessage>> {
    let suffix = msg.addr.strip_prefix("/coupling/")?;
    let value = match extract_float(&msg.args) {
        Some(v) => v,
        None => return Some(Vec::new()),
    };
    let messages = match suffix {
        "strength_ab" => vec![InboundMessage::SetCouplingStrengthAB(value)],
        "strength_ba" => vec![InboundMessage::SetCouplingStrengthBA(value)],
        "strength" => vec![
            InboundMessage::SetCouplingStrengthAB(value),
            InboundMessage::SetCouplingStrengthBA(value),
        ],
        _ => Vec::new(),
    };
    Some(messages)
}

/// Map an OSC address to a `(target, parameter_suffix)` pair, if it's
/// one of ours. Returns None if the address doesn't match the
/// `[<prefix>]/physics/<param>` shape.
///
/// Recognized addresses:
///   /physics/<param>     -> (Both, <param>)
///   /a/physics/<param>   -> (Chain(A), <param>)
///   /b/physics/<param>   -> (Chain(B), <param>)
fn parse_address(addr: &str) -> Option<(InboundTarget, &str)> {
    if let Some(rest) = addr.strip_prefix("/a/physics/") {
        Some((InboundTarget::Chain(ChainId::A), rest))
    } else if let Some(rest) = addr.strip_prefix("/b/physics/") {
        Some((InboundTarget::Chain(ChainId::B), rest))
    } else if let Some(rest) = addr.strip_prefix("/physics/") {
        Some((InboundTarget::Both, rest))
    } else {
        None
    }
}

fn parse_message(msg: &OscMessage) -> Option<InboundMessage> {
    let (target, param) = parse_address(&msg.addr)?;
    let value = extract_float(&msg.args)?;
    match param {
        "kt"  => Some(InboundMessage::SetKt(target, value)),
        "eps" => Some(InboundMessage::SetEps(target, value)),
        "j"   => Some(InboundMessage::SetJ(target, value)),
        "w"   => Some(InboundMessage::SetW(target, value)),
        _ => None,
    }
}

pub fn serialize_bundle(events: &[OutboundEvent]) -> Vec<u8> {
    if events.is_empty() {
        return Vec::new();
    }

    let content: Vec<OscPacket> = events
        .iter()
        .map(|e| OscPacket::Message(to_message(e)))
        .collect();

    let bundle = OscBundle {
        timetag: TIME_IMMEDIATE,
        content,
    };

    encoder::encode(&OscPacket::Bundle(bundle)).unwrap_or_default()
}

fn to_message(event: &OutboundEvent) -> rosc::OscMessage {
    match event {
        OutboundEvent::WallCreated { chain, id, position, channel } => rosc::OscMessage {
            addr: format!("{}/wall/created", chain.osc_prefix()),
            args: vec![
                OscType::Int(*id as i32),
                OscType::Float(*position as f32),
                OscType::Int(*channel as i32),
            ],
        },
        OutboundEvent::WallDestroyed { chain, id, last_position, lifetime_ticks } => rosc::OscMessage {
            addr: format!("{}/wall/destroyed", chain.osc_prefix()),
            args: vec![
                OscType::Int(*id as i32),
                OscType::Float(*last_position as f32),
                OscType::Int(*lifetime_ticks as i32),
            ],
        },
        OutboundEvent::WallMoved { chain, id, from, to, velocity } => rosc::OscMessage {
            addr: format!("{}/wall/moved", chain.osc_prefix()),
            args: vec![
                OscType::Int(*id as i32),
                OscType::Float(*from as f32),
                OscType::Float(*to as f32),
                OscType::Float(*velocity as f32),
            ],
        },
        OutboundEvent::SiteEvent { chain, site, voice, intensity } => rosc::OscMessage {
            addr: format!("{}/site/event", chain.osc_prefix()),
            args: vec![
                OscType::Int(*site as i32),
                OscType::Int(*voice as i32),
                OscType::Float(*intensity),
            ],
        },
        OutboundEvent::ClockPulse { chain, magnetization } => rosc::OscMessage {
            addr: format!("{}/clock/pulse", chain.osc_prefix()),
            args: vec![
                OscType::Float(*magnetization as f32),
            ],
        },
        OutboundEvent::StateSpins { chain, values } => rosc::OscMessage {
            addr: format!("{}/state/spins", chain.osc_prefix()),
            args: values.iter().map(|v| OscType::Float(*v)).collect(),
        },
        OutboundEvent::StateMagnetization { chain, magnetization } => rosc::OscMessage {
            addr: format!("{}/state/magnetization", chain.osc_prefix()),
            args: vec![OscType::Float(*magnetization)],
        },
        OutboundEvent::StateWallCount { chain, count } => rosc::OscMessage {
            addr: format!("{}/state/wall_count", chain.osc_prefix()),
            args: vec![OscType::Int(*count)],
        },
    }
}

fn extract_float(args: &[OscType]) -> Option<f64> {
    if args.len() != 1 {
        return None;
    }
    match &args[0] {
        OscType::Float(f) => Some(*f as f64),
        OscType::Double(d) => Some(*d),
        OscType::Int(i) => Some(*i as f64),
        OscType::Long(l) => Some(*l as f64),
        _ => None,
    }
}