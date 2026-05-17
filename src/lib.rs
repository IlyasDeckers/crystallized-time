//! Library crate for crystallized_time. The realtime program (src/main.rs)
//! and the sweep tool (src/bin/sweep.rs) both link against this; modules
//! that need to be visible to either binary are re-exported here.

pub mod chain;
pub mod chain_id;
pub mod config;
pub mod quantizer;
