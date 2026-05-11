//! OSC message types and parsing for the inbound control surface.
//!
//! This module is pure data — no I/O, no threads, no sockets. It maps
//! raw `rosc::OscPacket` values onto our internal `InboundMessage` enum,
//! discarding anything malformed or unrecognized.
//!
//! Outbound message types and serialization will be added here in
//! Steps 4–5 (events and state). For Step 3 the module only covers the
//! four `/physics/<param>` addresses.

use rosc::{OscMessage, OscPacket, OscType};

use rosc::{encoder, OscBundle, OscPacket as RoscPacket, OscTime, OscType as RoscOscType};

/// OSC's "immediate" time tag per the 1.0 spec: seconds=0, fractional=1.
/// Receivers should process the bundle without scheduling delay. We use
/// this for every outbound bundle — no NTP timing needed for our use case.
const TIME_IMMEDIATE: OscTime = OscTime { seconds: 0, fractional: 1 };

/// Recognized inbound OSC messages.
///
/// The four physics parameter targets are the only things callers can
/// send today. Future extensions (e.g. `/perturb/flip_site`) would add
/// variants here; the receiver thread doesn't need to change beyond
/// that match-arm.
#[derive(Debug, Clone, Copy)]
pub enum InboundMessage {
    SetKt(f64),
    SetEps(f64),
    SetJ(f64),
    SetW(f64),
}

/// One event the simulation thread wants to publish externally.
///
/// These mirror the things that already happen in the loop: a site
/// fires, a wall is created / destroyed / moves, the clock pulses.
/// Serialized into per-tick OSC bundles by `serialize_bundle`.
#[derive(Debug, Clone)]
pub enum OutboundEvent {
    WallCreated {
        id: u64,
        position: f64,
        /// The MIDI channel (0–15) the wall is sounding on. Useful for
        /// visualizations that want to color-code walls by channel.
        channel: u8,
    },
    WallDestroyed {
        id: u64,
        last_position: f64,
        lifetime_ticks: u64,
    },
    WallMoved {
        id: u64,
        from: f64,
        to: f64,
        velocity: f64,
    },
    SiteEvent {
        site: u8,
        voice: u8,
        intensity: f32,
    },
    ClockPulse {
        magnetization: f64,
    },
    StateSpins {
        /// Per-site sigma_z values. Length equals n_sites.
        values: Vec<f32>,
    },
    StateMagnetization {
        magnetization: f32,
    },
    StateWallCount {
        count: i32,
    },
}

/// Flatten an OSC packet into a sequence of recognized messages.
///
/// Bundles are unwrapped recursively — TouchDesigner may batch multiple
/// parameter writes per frame into a single packet, and they're all
/// equivalent to individually-delivered messages as far as we care.
///
/// Anything we don't recognize (wrong address, wrong argument types,
/// wrong argument count) is dropped silently. Per spec: "Malformed
/// messages are dropped silently. The receiver thread does not
/// propagate parse errors."
pub fn extract_messages(packet: OscPacket) -> Vec<InboundMessage> {
    let mut out = Vec::new();
    walk(packet, &mut out);
    out
}

fn walk(packet: OscPacket, out: &mut Vec<InboundMessage>) {
    match packet {
        OscPacket::Message(msg) => {
            if let Some(inbound) = parse_message(&msg) {
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

/// Try to interpret a single OSC message as one of our inbound types.
/// Returns None if the address isn't ours or the arguments don't match.
fn parse_message(msg: &OscMessage) -> Option<InboundMessage> {
    let value = extract_float(&msg.args)?;
    match msg.addr.as_str() {
        "/physics/kt"  => Some(InboundMessage::SetKt(value)),
        "/physics/eps" => Some(InboundMessage::SetEps(value)),
        "/physics/j"   => Some(InboundMessage::SetJ(value)),
        "/physics/w"   => Some(InboundMessage::SetW(value)),
        _ => None,
    }
}

/// Serialize a tick's worth of events into a single OSC bundle ready
/// for `UdpSocket::send_to`. Empty input returns an empty Vec; callers
/// should skip sending in that case.
pub fn serialize_bundle(events: &[OutboundEvent]) -> Vec<u8> {
    if events.is_empty() {
        return Vec::new();
    }

    let content: Vec<RoscPacket> = events
        .iter()
        .map(|e| RoscPacket::Message(to_message(e)))
        .collect();

    let bundle = OscBundle {
        timetag: TIME_IMMEDIATE,
        content,
    };

    encoder::encode(&RoscPacket::Bundle(bundle)).unwrap_or_default()
}

/// Map one event to its OSC message. Addresses and argument shapes
/// follow the spec's outbound schema exactly.
fn to_message(event: &OutboundEvent) -> rosc::OscMessage {
    match event {
        OutboundEvent::WallCreated { id, position, channel } => rosc::OscMessage {
            addr: "/wall/created".to_string(),
            args: vec![
                RoscOscType::Int(*id as i32),
                RoscOscType::Float(*position as f32),
                RoscOscType::Int(*channel as i32),
            ],
        },
        OutboundEvent::WallDestroyed { id, last_position, lifetime_ticks } => rosc::OscMessage {
            addr: "/wall/destroyed".to_string(),
            args: vec![
                RoscOscType::Int(*id as i32),
                RoscOscType::Float(*last_position as f32),
                RoscOscType::Int(*lifetime_ticks as i32),
            ],
        },
        OutboundEvent::WallMoved { id, from, to, velocity } => rosc::OscMessage {
            addr: "/wall/moved".to_string(),
            args: vec![
                RoscOscType::Int(*id as i32),
                RoscOscType::Float(*from as f32),
                RoscOscType::Float(*to as f32),
                RoscOscType::Float(*velocity as f32),
            ],
        },
        OutboundEvent::SiteEvent { site, voice, intensity } => rosc::OscMessage {
            addr: "/site/event".to_string(),
            args: vec![
                RoscOscType::Int(*site as i32),
                RoscOscType::Int(*voice as i32),
                RoscOscType::Float(*intensity),
            ],
        },
        OutboundEvent::ClockPulse { magnetization } => rosc::OscMessage {
            addr: "/clock/pulse".to_string(),
            args: vec![
                RoscOscType::Float(*magnetization as f32),
            ],
        },
        OutboundEvent::StateSpins { values } => rosc::OscMessage {
            addr: "/state/spins".to_string(),
            args: values.iter().map(|v| RoscOscType::Float(*v)).collect(),
        },
        OutboundEvent::StateMagnetization { magnetization } => rosc::OscMessage {
            addr: "/state/magnetization".to_string(),
            args: vec![RoscOscType::Float(*magnetization)],
        },
        OutboundEvent::StateWallCount { count } => rosc::OscMessage {
            addr: "/state/wall_count".to_string(),
            args: vec![RoscOscType::Int(*count)],
        },
    }
}

/// Pull a single numeric argument from a message's argument list, widened
/// to f64. Accepts both Float (TouchDesigner's default) and Int (a small
/// kindness for callers who forget to cast). Returns None if the message
/// has anything other than exactly one numeric argument.
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