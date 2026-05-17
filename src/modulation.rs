/// Per-tick modulation CC emitter.
///
/// Sums sigma_z over the chain's output sites, maps the sum linearly to
/// [0, 127] centered at CC 64 (sum = 0), and sends a CC when the value
/// changes by at least 1 unit. Stage 2.5 spec feature.

use crate::chain::SpinChain;
use crate::config::ModulationConfig;
use crate::midi::MidiSender;

/// Map a summed sz value (range [-n_sites, +n_sites]) to a 7-bit CC value
/// centred at 64. Factored out so the mapping is independently testable.
pub fn sum_to_cc(sum: f64, n_output_sites: usize) -> u8 {
    if n_output_sites == 0 {
        return 64;
    }
    let n = n_output_sites as f64;
    let normalized = (sum + n) / (2.0 * n);
    (normalized * 127.0).round().clamp(0.0, 127.0) as u8
}

pub struct ModulationEmitter {
    config: ModulationConfig,
    output_sites: Vec<usize>,
    last_value: Option<u8>,
}

impl ModulationEmitter {
    pub fn new(config: ModulationConfig, output_sites: Vec<usize>) -> Self {
        Self {
            config,
            output_sites,
            last_value: None,
        }
    }

    pub fn tick(&mut self, chain: &SpinChain, sender: &MidiSender) {
        if !self.config.enabled || self.output_sites.is_empty() {
            return;
        }

        let sum: f64 = self.output_sites.iter().map(|&i| chain.sz(i)).sum();
        let cc_value = sum_to_cc(sum, self.output_sites.len());

        if self.last_value != Some(cc_value) {
            sender.send_cc(self.config.channel, self.config.cc_number, cc_value);
            self.last_value = Some(cc_value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_positive_range_maps_to_127() {
        assert_eq!(sum_to_cc(4.0, 4), 127);
    }

    #[test]
    fn full_negative_range_maps_to_0() {
        assert_eq!(sum_to_cc(-4.0, 4), 0);
    }

    #[test]
    fn zero_sum_maps_to_64() {
        assert_eq!(sum_to_cc(0.0, 4), 64);
    }

    #[test]
    fn half_positive_maps_halfway() {
        // sum=2, n=4 → normalized = (2+4)/8 = 0.75 → CC 95
        assert_eq!(sum_to_cc(2.0, 4), 95);
    }

    #[test]
    fn half_negative_maps_halfway() {
        // sum=-2, n=4 → normalized = (-2+4)/8 = 0.25 → CC 32
        assert_eq!(sum_to_cc(-2.0, 4), 32);
    }

    #[test]
    fn empty_sites_defaults_to_64() {
        assert_eq!(sum_to_cc(0.0, 0), 64);
    }

    #[test]
    fn odd_num_sites_still_centers_at_64() {
        // 3 sites, all +1 → sum=3 → normalized = (3+3)/6 = 1.0 → 127
        assert_eq!(sum_to_cc(3.0, 3), 127);
        // 3 sites, zero sum → normalized = (0+3)/6 = 0.5 → 64
        assert_eq!(sum_to_cc(0.0, 3), 64);
    }
}
