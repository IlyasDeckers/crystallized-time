/// Per-chain modulation CC config (Stage 2.5 spec).
///
/// Emits a continuous CC stream of summed sigma_z over the chain's output
/// sites, mapped to [0, 127], change-filtered by >= 1 unit.
#[derive(Clone, Debug)]
pub struct ModulationConfig {
    pub enabled: bool,
    /// MIDI channel (0-based internal representation).
    pub channel: u8,
    pub cc_number: u8,
}

impl Default for ModulationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            channel: 0,
            cc_number: 1,
        }
    }
}
