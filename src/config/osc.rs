//! OSC-specific configuration. Listen port and send address now live in
//! the config file alongside the other knobs that have non-trivial
//! defaults worth keeping out of `main.rs`.

#[derive(Clone, Debug)]
pub struct OscConfig {
    /// UDP port to listen for inbound parameter messages. None disables
    /// the receiver thread entirely.
    pub listen_port: Option<u16>,
    /// Destination "host:port" for outbound events and state. None
    /// disables the sender thread entirely.
    pub send_address: Option<String>,
    /// Target rate for scalar state messages (`/state/magnetization`,
    /// `/state/wall_count`), in Hz. Wall-clock-based, so the rate is
    /// honest regardless of BPM. Set to 0 to disable rate-gating
    /// (change-filtering still applies).
    pub state_rate_hz: f64,
    /// Target rate for the heavier `/state/spins` vector messages, in Hz.
    /// Defaults to half of `state_rate_hz` since spin vectors are the
    /// biggest payload and change slowly in the locked phase. Set to 0
    /// to disable rate-gating.
    pub state_spins_rate_hz: f64,
    /// When false, state messages are not pushed even if send_address is set.
    /// Events still flow. Useful for bandwidth-sensitive setups where the
    /// receiver only needs event triggers.
    pub enable_state: bool,
}

impl Default for OscConfig {
    fn default() -> Self {
        Self {
            listen_port: None,
            send_address: None,
            state_rate_hz: 50.0,
            state_spins_rate_hz: 25.0,
            enable_state: true,
        }
    }
}