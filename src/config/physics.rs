//! Physics parameters governing the spin chain, plus the mutability
//! infrastructure that lets four of them be live-tuned via OSC.

/// Physics parameters governing the spin chain.
#[derive(Clone, Debug)]
pub struct PhysicsConfig {
    /// Number of sites in the chain.
    pub n_sites: usize,
    /// Integration step in simulation units.
    pub dt: f64,
    /// Drive imperfection (epsilon). Pulse angle is (1 - eps) * pi.
    pub eps: f64,
    /// Coupling strength J.
    pub j: f64,
    /// Disorder width W (range of random local Z-fields).
    pub w: f64,
    /// Effective temperature kT (thermal noise strength).
    pub kt: f64,
    /// Number of integration ticks per drive period.
    /// 25 by default — chosen so dt * ticks_per_period = 1.0 sim time unit.
    pub ticks_per_period: u32,
    /// Base angle for the periodic kick, in radians. The actual rotation
    /// applied is (1 - eps) * kick_angle. Default pi (period-2 dynamics).
    /// Set to 2*pi/3 to target period-3, pi/2 for period-4, etc.
    pub kick_angle: f64,
}

impl Default for PhysicsConfig {
    fn default() -> Self {
        Self {
            n_sites: 8,
            dt: 0.04,
            eps: 0.01,
            j: 1.2,
            w: 2.0,
            kt: 0.1,
            ticks_per_period: 25,
            kick_angle: std::f64::consts::PI,
        }
    }
}

/// Mutable target values for the four live-tunable physics parameters.
/// Writers (the OSC receiver thread) update these via `RwLock::write()`;
/// the simulation thread reads them once per tick via `RwLock::read()`.
///
/// Values are clamped to per-parameter bounds on write (see `clamp_kt`
/// etc.). Reads always return in-bounds values.
#[derive(Clone, Debug)]
pub struct PhysicsTargets {
    pub kt: f64,
    pub eps: f64,
    pub j: f64,
    pub w: f64,
}

impl PhysicsTargets {
    /// Build targets that match an initial `PhysicsConfig` exactly, so the
    /// chain starts at the configured values and doesn't smooth toward
    /// anything until an external writer changes a target.
    pub fn from_physics(config: &PhysicsConfig) -> Self {
        Self {
            kt: config.kt,
            eps: config.eps,
            j: config.j,
            w: config.w,
        }
    }

    pub fn clamp_kt(v: f64) -> f64  { v.clamp(0.0, 2.0) }
    pub fn clamp_eps(v: f64) -> f64 { v.clamp(0.0, 0.5) }
    pub fn clamp_j(v: f64) -> f64   { v.clamp(0.0, 3.0) }
    pub fn clamp_w(v: f64) -> f64   { v.clamp(0.0, 5.0) }
}

/// Per-parameter smoothing time constants, in seconds. `tau` is the
/// time it takes to cover ~63% of the remaining gap to the target.
/// After `3 * tau` seconds the value is essentially at target.
#[derive(Clone, Debug)]
pub struct SmoothingConfig {
    pub kt_tau_secs: f64,
    pub eps_tau_secs: f64,
    pub j_tau_secs: f64,
    pub w_tau_secs: f64,
}

impl Default for SmoothingConfig {
    fn default() -> Self {
        Self {
            kt_tau_secs: 1.5,
            eps_tau_secs: 1.0,
            j_tau_secs: 2.0,
            w_tau_secs: 2.0,
        }
    }
}

/// Pre-computed per-tick smoothing coefficients. Each `alpha` is
/// `1 - exp(-dt_real / tau)` for the corresponding parameter.
///
/// Computed once at startup from `SmoothingConfig` and the nominal tick
/// duration; doesn't change for the run. Per spec, smoothing uses the
/// nominal tick duration (drive_period / ticks_per_period), so the
/// smoothing rate is coupled to BPM — a higher BPM means parameters
/// reach their targets in fewer wall-clock seconds.
#[derive(Clone, Debug)]
pub struct SmoothingAlphas {
    pub kt: f64,
    pub eps: f64,
    pub j: f64,
    pub w: f64,
}

impl SmoothingAlphas {
    pub fn from_config(smoothing: &SmoothingConfig, dt_real_secs: f64) -> Self {
        // alpha = 1 - exp(-dt / tau). If tau is zero or negative, treat as
        // "no smoothing" (alpha = 1.0) so targets land instantly.
        let alpha = |tau: f64| -> f64 {
            if tau <= 0.0 { 1.0 } else { 1.0 - (-dt_real_secs / tau).exp() }
        };
        Self {
            kt: alpha(smoothing.kt_tau_secs),
            eps: alpha(smoothing.eps_tau_secs),
            j: alpha(smoothing.j_tau_secs),
            w: alpha(smoothing.w_tau_secs),
        }
    }
}

/// Compute the next physics snapshot by exponentially approaching the
/// targets. Returns `Some(new_config)` if any parameter moved by more
/// than `EPSILON`, `None` if all four are effectively at their targets.
///
/// Returning `None` lets the caller skip the ArcSwap store (and the
/// `Arc::new` allocation) on steady-state ticks — at rest with no OSC
/// traffic, this function returns `None` every tick and the loop does
/// zero work for parameter management.
pub fn apply_smoothing(
    current: &PhysicsConfig,
    targets: &PhysicsTargets,
    alphas: &SmoothingAlphas,
) -> Option<PhysicsConfig> {
    const EPSILON: f64 = 1e-9;

    let new_kt  = current.kt  + (targets.kt  - current.kt)  * alphas.kt;
    let new_eps = current.eps + (targets.eps - current.eps) * alphas.eps;
    let new_j   = current.j   + (targets.j   - current.j)   * alphas.j;
    let new_w   = current.w   + (targets.w   - current.w)   * alphas.w;

    let changed =
        (new_kt  - current.kt ).abs() > EPSILON ||
            (new_eps - current.eps).abs() > EPSILON ||
            (new_j   - current.j  ).abs() > EPSILON ||
            (new_w   - current.w  ).abs() > EPSILON;

    if !changed {
        return None;
    }

    let mut next = current.clone();
    next.kt  = new_kt;
    next.eps = new_eps;
    next.j   = new_j;
    next.w   = new_w;
    Some(next)
}