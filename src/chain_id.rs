//! Stable identifier for a chain in a multi-chain system.
//!
//! Used for OSC address prefixing, config section selection, and any
//! routing that needs to distinguish chains. Two variants for now;
//! could extend to three or more without code restructuring.

use serde::Deserialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize)]
pub enum ChainId {
    A,
    B,
}

impl ChainId {
    /// The OSC address prefix for this chain. Used to namespace
    /// per-chain events ('/a/site/event', '/b/clock/pulse', etc).
    pub fn osc_prefix(self) -> &'static str {
        match self {
            ChainId::A => "/a",
            ChainId::B => "/b",
        }
    }

    /// Short human-readable label for logging.
    pub fn label(self) -> &'static str {
        match self {
            ChainId::A => "chain_a",
            ChainId::B => "chain_b",
        }
    }
}