//! Classical disordered spin chain with periodic Floquet drive.
//!
//! This module is the only place that knows about the physics. Other modules
//! see only the public interface: construct a chain, step it, read sigma_z values.
//! When the substrate eventually swaps to quantum (quantrs2), only this file changes.

use crate::config::PhysicsConfig;
use rand::Rng;
use rand_distr::StandardNormal;

/// A single 3D spin, unit length.
pub type Spin = [f64; 3];

/// A 3D vector (used for fields, couplings as scaled vectors, etc.).
pub type Vec3 = [f64; 3];

/// State of the disordered Floquet spin chain.
pub struct SpinChain {
    /// Configuration (held for parameter access during steps).
    pub config: PhysicsConfig,
    /// Spin vectors, one per site.
    pub spins: Vec<Spin>,
    /// Local field at each site (xy components small noise, z drawn from disorder).
    pub fields: Vec<Vec3>,
    /// Nearest-neighbor coupling strengths J_{i,i+1}, length n_sites - 1.
    pub couplings: Vec<f64>,
    /// Tick counter (advances on every step).
    pub tick: u64,
}

impl SpinChain {
    /// Build a new chain with random initial spins, fields, and couplings.
    /// `rng` is taken so the caller controls seeding.
    pub fn new(config: PhysicsConfig, rng: &mut impl Rng) -> Self {
        let n = config.n_sites;

        let mut spins = Vec::with_capacity(n);
        for _ in 0..n {
            // Choose a pole (+z or -z), then add a small random tilt away from it.
            // theta is the polar angle from the chosen pole.
            let near_north = rng.gen::<bool>();
            let theta = rng.gen::<f64>() * 0.4; // up to ~23 degrees off-pole
            let phi = rng.gen::<f64>() * std::f64::consts::TAU;

            let sx = theta.sin() * phi.cos();
            let sy = theta.sin() * phi.sin();
            let sz = if near_north { theta.cos() } else { -theta.cos() };

            spins.push([sx, sy, sz]);
        }

        let mut fields = Vec::with_capacity(n);
        for _ in 0..n {
            // Small random xy components, large random z component scaled by W.
            let hx: f64 = rng.sample::<f64, _>(StandardNormal) * 0.3;
            let hy: f64 = rng.sample::<f64, _>(StandardNormal) * 0.3;
            let hz: f64 = (rng.gen::<f64>() * 2.0 - 1.0) * config.w;
            fields.push([hx, hy, hz]);
        }

        let mut couplings = Vec::with_capacity(n.saturating_sub(1));
        for _ in 0..n.saturating_sub(1) {
            // J scaled by uniform random in [0.7, 1.3].
            let j = config.j * (0.7 + rng.gen::<f64>() * 0.6);
            couplings.push(j);
        }

        Self {
            config,
            spins,
            fields,
            couplings,
            tick: 0,
        }
    }

    /// Advance the chain by one integration step.
    /// Applies the drive pulse if `tick` lands on a drive boundary.
    /// `rng` is used for thermal noise.
    pub fn step(&mut self, rng: &mut impl Rng) {
        let n = self.config.n_sites;
        let dt = self.config.dt;
        let kt = self.config.kt;

        // Compute new spins from old spins. We can't update in place because
        // each spin's update depends on its neighbors' *current* values —
        // updating site 0 first would leave site 1 reading the new value of 0.
        let mut new_spins: Vec<Spin> = Vec::with_capacity(n);

        let noise_scale = (2.0 * kt * dt).sqrt();

        for i in 0..n {
            let s = self.spins[i];

            // Effective field at site i: local field + neighbor coupling on z.
            let mut h = self.fields[i];
            if i > 0 {
                h[2] += self.couplings[i - 1] * self.spins[i - 1][2];
            }
            if i < n - 1 {
                h[2] += self.couplings[i] * self.spins[i + 1][2];
            }

            // Torque: s × h. This is the precession term.
            let cross = [
                s[1] * h[2] - s[2] * h[1],
                s[2] * h[0] - s[0] * h[2],
                s[0] * h[1] - s[1] * h[0],
            ];

            // Thermal noise: small Gaussian kicks on each component.
            let nx: f64 = rng.sample::<f64, _>(StandardNormal) * noise_scale;
            let ny: f64 = rng.sample::<f64, _>(StandardNormal) * noise_scale;
            let nz: f64 = rng.sample::<f64, _>(StandardNormal) * noise_scale;

            // Euler step: new = old + torque*dt + noise.
            let mut next = [
                s[0] + cross[0] * dt + nx,
                s[1] + cross[1] * dt + ny,
                s[2] + cross[2] * dt + nz,
            ];

            // Renormalize to unit length. Spins on the Bloch sphere stay on it.
            let norm = (next[0] * next[0] + next[1] * next[1] + next[2] * next[2]).sqrt();
            if norm > 0.0 {
                next[0] /= norm;
                next[1] /= norm;
                next[2] /= norm;
            }

            new_spins.push(next);
        }

        self.spins = new_spins;

        // If we just landed on a drive boundary, apply the kick.
        // tick > 0 so we don't kick on the very first step.
        self.tick += 1;
        if self.tick > 0 && self.tick % self.config.ticks_per_period as u64 == 0 {
            self.apply_drive_pulse();
        }
    }

    /// Apply the (1 - eps) * pi rotation around the x-axis to every spin.
    /// This is the periodic Floquet kick that produces period-doubling.
    fn apply_drive_pulse(&mut self) {
        let angle = (1.0 - self.config.eps) * std::f64::consts::PI;
        let c = angle.cos();
        let si = angle.sin();

        for s in self.spins.iter_mut() {
            let sy_new = s[1] * c - s[2] * si;
            let sz_new = s[1] * si + s[2] * c;
            // s[0] (x) unchanged — rotation is around x-axis.
            s[1] = sy_new;
            s[2] = sz_new;
        }
    }

    /// Read the z-component of spin `i`.
    pub fn sz(&self, i: usize) -> f64 {
        self.spins[i][2]
    }
}
