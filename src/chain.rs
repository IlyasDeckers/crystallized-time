//! Classical disordered spin chain with periodic Floquet drive.
//!
//! This module is the only place that knows about the physics. Other modules
//! see only the public interface: construct a chain, step it, read sigma_z values.
//! When the substrate eventually swaps to quantum (quantrs2), only this file changes.

use crate::config::PhysicsConfig;
use arc_swap::ArcSwap;
use rand::Rng;
use rand_distr::StandardNormal;
use std::sync::Arc;

/// A single 3D spin, unit length.
pub type Spin = [f64; 3];

/// A 3D vector (used for fields, couplings as scaled vectors, etc.).
pub type Vec3 = [f64; 3];

/// Rotation axis for localized perturbations.
///
/// An enum rather than a char so the compiler enforces exhaustiveness —
/// adding a new variant later will produce build errors at every match site
/// that hasn't handled it. Cheap insurance.
#[derive(Clone, Copy, Debug)]
pub enum Axis {
    X,
    Y,
    Z,
}

/// A single localized perturbation applied to one site at one tick.
///
/// `Flip` and `Rotate` modify the spin vector directly. `FieldSpike` modifies
/// the effective field for the *next* integration step, then clears itself —
/// so a spike is a one-tick forced perturbation, not a state change.
#[derive(Clone, Copy, Debug)]
pub enum PerturbationKind {
    /// Negate the z-component of the spin. The x and y components are
    /// untouched; renormalization happens regardless (cheap, robust against
    /// floating-point drift).
    Flip,
    /// Rotate the spin by `angle` radians around `axis`.
    Rotate { axis: Axis, angle: f64 },
    /// Add `delta` to this site's effective field for exactly the next
    /// integration step, then clear.
    FieldSpike { delta: Vec3 },
}

/// State of the disordered Floquet spin chain.
pub struct SpinChain {
    /// Live configuration. Read once at the top of each step so the whole
    /// step sees a consistent snapshot, even if another thread swaps the
    /// pointer mid-step.
    pub config: Arc<ArcSwap<PhysicsConfig>>,
    /// Spin vectors, one per site.
    pub spins: Vec<Spin>,
    /// Local field at each site (xy components small noise, z drawn from disorder).
    pub fields: Vec<Vec3>,
    /// Nearest-neighbor coupling strengths J_{i,i+1}, length n_sites - 1.
    pub couplings: Vec<f64>,
    /// One-tick field perturbations, one slot per site. Most slots are `None`
    /// at any given time. `step` consumes any `Some` value into the effective
    /// field for that tick, then sets the slot back to `None`.
    pending_field_deltas: Vec<Option<Vec3>>,
    /// Tick counter (advances on every step).
    pub tick: u64,
}

impl SpinChain {
    /// Build a new chain with random initial spins, fields, and couplings.
    /// `rng` is taken so the caller controls seeding.
    pub fn new(config: Arc<ArcSwap<PhysicsConfig>>, rng: &mut impl Rng) -> Self {
        let snapshot = config.load();
        let n = snapshot.n_sites;

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
            let hx: f64 = rng.sample::<f64, _>(StandardNormal) * 0.3;
            let hy: f64 = rng.sample::<f64, _>(StandardNormal) * 0.3;
            let hz: f64 = (rng.gen::<f64>() * 2.0 - 1.0) * snapshot.w;
            fields.push([hx, hy, hz]);
        }

        let mut couplings = Vec::with_capacity(n.saturating_sub(1));
        for _ in 0..n.saturating_sub(1) {
            let j = snapshot.j * (0.7 + rng.gen::<f64>() * 0.6);
            couplings.push(j);
        }

        drop(snapshot);

        Self {
            config,
            spins,
            fields,
            couplings,
            pending_field_deltas: vec![None; n],
            tick: 0,
        }
    }

    /// Apply a localized perturbation to one site. Out-of-range site indices
    /// are silently dropped — perturbation requests come from external input
    /// (MIDI, OSC) and a bad request shouldn't kill the realtime loop.
    pub fn perturb(&mut self, site: usize, kind: PerturbationKind) {
        if site >= self.spins.len() {
            return;
        }
        match kind {
            PerturbationKind::Flip => {
                self.spins[site][2] = -self.spins[site][2];
                self.renormalize_site(site);
            }
            PerturbationKind::Rotate { axis, angle } => {
                self.rotate_site(site, axis, angle);
                self.renormalize_site(site);
            }
            PerturbationKind::FieldSpike { delta } => {
                self.add_pending_field_delta(site, delta);
            }
        }
    }

    /// Rotate one site's spin by `angle` radians around `axis`. Same rotation
    /// math as `apply_drive_pulse`, generalized to any axis and one site.
    fn rotate_site(&mut self, i: usize, axis: Axis, angle: f64) {
        let c = angle.cos();
        let s = angle.sin();
        let spin = &mut self.spins[i];
        match axis {
            Axis::X => {
                // Rotation in the y-z plane; x is the axis of rotation.
                let y_new = spin[1] * c - spin[2] * s;
                let z_new = spin[1] * s + spin[2] * c;
                spin[1] = y_new;
                spin[2] = z_new;
            }
            Axis::Y => {
                // Rotation in the z-x plane; y is the axis.
                let z_new = spin[2] * c - spin[0] * s;
                let x_new = spin[2] * s + spin[0] * c;
                spin[2] = z_new;
                spin[0] = x_new;
            }
            Axis::Z => {
                // Rotation in the x-y plane; z is the axis.
                let x_new = spin[0] * c - spin[1] * s;
                let y_new = spin[0] * s + spin[1] * c;
                spin[0] = x_new;
                spin[1] = y_new;
            }
        }
    }

    /// Renormalize one site's spin to unit length. Belt-and-braces against
    /// floating-point drift after perturbations. No-op if the spin is the
    /// zero vector (shouldn't happen, but defended against).
    fn renormalize_site(&mut self, i: usize) {
        let s = &mut self.spins[i];
        let norm = (s[0] * s[0] + s[1] * s[1] + s[2] * s[2]).sqrt();
        if norm > 0.0 {
            s[0] /= norm;
            s[1] /= norm;
            s[2] /= norm;
        }
    }

    /// Advance the chain by one integration step.
    /// Applies the drive pulse if `tick` lands on a drive boundary.
    /// `rng` is used for thermal noise.
    pub fn step(&mut self, rng: &mut impl Rng) {
        let snapshot = self.config.load();
        let n = snapshot.n_sites;
        let dt = snapshot.dt;
        let kt = snapshot.kt;

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

            // Consume any pending field spike for this site. `.take()` reads
            // the Option and replaces it with None in one move, so the spike
            // lasts exactly one step.
            if let Some(delta) = self.pending_field_deltas[i].take() {
                h[0] += delta[0];
                h[1] += delta[1];
                h[2] += delta[2];
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

        self.tick += 1;
        if self.tick > 0 && self.tick % snapshot.ticks_per_period as u64 == 0 {
            self.apply_drive_pulse(snapshot.eps, snapshot.kick_angle);
        }
    }

    /// Apply the (1 - eps) * pi rotation around the x-axis to every spin.
    /// This is the periodic Floquet kick that produces period-doubling.
    fn apply_drive_pulse(&mut self, eps: f64, base_angle: f64) {
        let angle = (1.0 - eps) * base_angle;
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

    /// Add a one-tick field perturbation to a site, composing with any
    /// existing pending delta on that site. Coupling injection uses this so
    /// it doesn't clobber MIDI-triggered field spikes (and vice versa).
    ///
    /// Out-of-range site indices are silently dropped, matching `perturb`.
    pub fn add_pending_field_delta(&mut self, site: usize, delta: Vec3) {
        if site >= self.spins.len() {
            return;
        }
        match &mut self.pending_field_deltas[site] {
            Some(existing) => {
                existing[0] += delta[0];
                existing[1] += delta[1];
                existing[2] += delta[2];
            }
            None => {
                self.pending_field_deltas[site] = Some(delta);
            }
        }
    }

    /// Read the z-component of spin `i`.
    pub fn sz(&self, i: usize) -> f64 {
        self.spins[i][2]
    }

    /// Mean sigma_z across all sites. Range [-1, 1].
    /// In the time-crystal phase this flips sign every drive period.
    pub fn global_magnetization(&self) -> f64 {
        if self.spins.is_empty() {
            return 0.0;
        }
        let sum: f64 = self.spins.iter().map(|s| s[2]).sum();
        sum / self.spins.len() as f64
    }
}