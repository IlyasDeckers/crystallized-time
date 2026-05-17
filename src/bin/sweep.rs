use clap::Parser;
use crystallized_time::chain::SpinChain;
use crystallized_time::config::PhysicsConfig;
use arc_swap::ArcSwap;
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Parser, Debug)]
#[command(name = "sweep", about = "Phase diagram sweep for a single chain")]
struct SweepCli {
    /// eps range as "start,stop,count". Inclusive on both ends.
    #[arg(long, default_value = "0.0,0.3,21")]
    eps_range: String,

    /// J range as "start,stop,count".
    #[arg(long, default_value = "0.5,2.5,21")]
    j_range: String,

    #[arg(long, default_value_t = 2.0)]
    w: f64,
    #[arg(long, default_value_t = 0.1)]
    kt: f64,
    #[arg(long, default_value_t = 8)]
    n_sites: usize,
    #[arg(long, default_value_t = 25)]
    ticks_per_period: u32,
    #[arg(long, default_value_t = 0.04)]
    dt: f64,
    #[arg(long, default_value_t = 47)]
    seed: u64,

    #[arg(long, default_value_t = 100)]
    warmup_periods: u64,
    #[arg(long, default_value_t = 200)]
    analysis_periods: u64,

    #[arg(long, default_value_t = 10)]
    max_period: usize,
    #[arg(long, default_value_t = 0.7)]
    lock_threshold: f64,
    #[arg(long, default_value_t = 0.2)]
    thermal_threshold: f64,

    #[arg(long, default_value = "sweep.csv")]
    output: PathBuf,

    #[arg(long, default_value_t = std::f64::consts::PI)]
    kick_angle: f64,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = SweepCli::parse();

    let (eps_start, eps_stop, eps_n) = parse_range(&cli.eps_range)?;
    let (j_start, j_stop, j_n) = parse_range(&cli.j_range)?;
    let eps_values = linspace(eps_start, eps_stop, eps_n);
    let j_values = linspace(j_start, j_stop, j_n);

    let total = eps_values.len() * j_values.len();
    println!("Running {} grid points ({}x{} eps x J)...", total, eps_n, j_n);
    println!("  w={}, kT={}, n_sites={}, seed={}", cli.w, cli.kt, cli.n_sites, cli.seed);
    println!("  lock_threshold={}, thermal_threshold={}", cli.lock_threshold, cli.thermal_threshold);
    println!("  warmup={} periods, analysis={} periods", cli.warmup_periods, cli.analysis_periods);

    let file = File::create(&cli.output)?;
    let mut writer = BufWriter::new(file);
    write_csv_header(&mut writer, cli.max_period)?;

    let mut rows: Vec<(f64, f64, Classification, f64, usize)> = Vec::with_capacity(total);
    let start = std::time::Instant::now();
    let mut done = 0usize;

    for &eps in &eps_values {
        for &j in &j_values {
            let physics = PhysicsConfig {
                eps, j, w: cli.w, kt: cli.kt,
                n_sites: cli.n_sites,
                ticks_per_period: cli.ticks_per_period,
                dt: cli.dt,
                kick_angle: cli.kick_angle,
            };

            let samples = run_point(physics.clone(), cli.seed, cli.warmup_periods, cli.analysis_periods);
            let corr = autocorrelation(&samples, cli.max_period);
            let (class, best_corr, best_tau) = classify(&corr, cli.lock_threshold, cli.thermal_threshold);
            let dominant_period = match &class {
                Classification::Locked(n) => *n,
                _ => best_tau,
            };

            write_csv_row(&mut writer, eps, j, cli.w, cli.kt, cli.seed,
                          &class, dominant_period, best_corr, &corr)?;
            rows.push((eps, j, class, best_corr, best_tau));

            done += 1;
            if done.is_multiple_of(50) || done == total {
                let elapsed = start.elapsed().as_secs_f64();
                println!("  {}/{} ({:.1}s, ~{:.1}s remaining)",
                         done, total, elapsed,
                         elapsed * (total - done) as f64 / done as f64);
            }
        }
    }

    writer.flush()?;
    drop(writer);

    print_summary(&rows);
    println!("\nCSV written to {}", cli.output.display());

    Ok(())
}

#[derive(Clone, Debug)]
enum Classification {
    Locked(usize), // tau
    QuasiPeriodic,
    Thermal,
}

fn classify(corr: &[f64], lock_threshold: f64, thermal_threshold: f64) -> (Classification, f64, usize) {
    // Look for the smallest tau where C(tau) is large and positive.
    // A period-n lock means the stroboscopic signal returns to itself
    // after n steps: C(n) close to +1. Negative C values at smaller tau
    // are the within-period oscillation, not a separate lock.
    let positive_lock = corr.iter()
        .enumerate()
        .find(|(_, c)| **c >= lock_threshold);

    if let Some((idx, c)) = positive_lock {
        return (Classification::Locked(idx + 1), *c, idx + 1);
    }

    // No positive peak above threshold. Find max absolute value for the
    // thermal-vs-quasi decision.
    let (best_idx, best_val) = corr.iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.abs().partial_cmp(&b.abs()).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(i, v)| (i, *v))
        .unwrap_or((0, 0.0));

    let class = if best_val.abs() < thermal_threshold {
        Classification::Thermal
    } else {
        Classification::QuasiPeriodic
    };

    (class, best_val, best_idx + 1)
}

fn autocorrelation(samples: &[f64], max_tau: usize) -> Vec<f64> {
    let n = samples.len();
    let mean: f64 = samples.iter().sum::<f64>() / n as f64;
    let centered: Vec<f64> = samples.iter().map(|m| m - mean).collect();
    let denom: f64 = centered.iter().map(|m| m * m).sum();

    if denom < 1e-12 {
        // Chain is essentially constant (probably stuck at <M> = 0 in the
        // thermal phase, or fully aligned with no fluctuation). Return
        // zeros so it classifies as thermal.
        return vec![0.0; max_tau];
    }

    (1..=max_tau)
        .map(|tau| {
            let sum: f64 = (0..(n - tau))
                .map(|k| centered[k] * centered[k + tau])
                .sum();
            sum / denom
        })
        .collect()
}

fn parse_range(s: &str) -> Result<(f64, f64, usize), String> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 3 {
        return Err(format!("range must be 'start,stop,count', got '{}'", s));
    }
    let start: f64 = parts[0].trim().parse().map_err(|e| format!("start: {}", e))?;
    let stop: f64 = parts[1].trim().parse().map_err(|e| format!("stop: {}", e))?;
    let count: usize = parts[2].trim().parse().map_err(|e| format!("count: {}", e))?;
    if count < 2 {
        return Err("count must be >= 2".to_string());
    }
    Ok((start, stop, count))
}

fn linspace(start: f64, stop: f64, count: usize) -> Vec<f64> {
    (0..count)
        .map(|i| start + (stop - start) * (i as f64) / ((count - 1) as f64))
        .collect()
}

fn run_point(physics: PhysicsConfig, seed: u64, warmup_periods: u64, analysis_periods: u64) -> Vec<f64> {
    let mut rng = StdRng::seed_from_u64(seed);
    let physics_arc = Arc::new(ArcSwap::from_pointee(physics.clone()));
    let mut chain = SpinChain::new(Arc::clone(&physics_arc), &mut rng);

    let tpp = physics.ticks_per_period as u64;

    // Warmup: step and throw away.
    for _ in 0..(warmup_periods * tpp) {
        chain.step(&mut rng);
    }

    // Analysis: step and sample stroboscopically.
    // The drive pulse fires on ticks where (tick % ticks_per_period == 0).
    // We sample <M> immediately after each kick, i.e. right after step()
    // increments the tick to a multiple of tpp.
    let mut samples = Vec::with_capacity(analysis_periods as usize);
    for _ in 0..analysis_periods {
        for _ in 0..tpp {
            chain.step(&mut rng);
        }
        samples.push(chain.global_magnetization());
    }

    samples
}

fn write_csv_header(w: &mut impl Write, max_tau: usize) -> std::io::Result<()> {
    write!(w, "eps,j,w,kt,seed,classification,dominant_period,best_corr")?;
    for tau in 1..=max_tau {
        write!(w, ",c{}", tau)?;
    }
    writeln!(w)
}

#[allow(clippy::too_many_arguments)]
fn write_csv_row(
    w: &mut impl Write,
    eps: f64, j: f64, w_param: f64, kt: f64, seed: u64,
    class: &Classification, period: usize, best_corr: f64,
    corr: &[f64],
) -> std::io::Result<()> {
    let class_str = match class {
        Classification::Locked(_) => "locked",
        Classification::QuasiPeriodic => "quasi",
        Classification::Thermal => "thermal",
    };
    write!(w, "{:.4},{:.4},{:.4},{:.4},{},{},{},{:.4}",
           eps, j, w_param, kt, seed, class_str, period, best_corr)?;
    for c in corr {
        write!(w, ",{:.4}", c)?;
    }
    writeln!(w)
}

fn print_summary(rows: &[(f64, f64, Classification, f64, usize)]) {
    use std::collections::BTreeMap;

    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_period: BTreeMap<usize, Vec<(f64, f64, f64)>> = BTreeMap::new();
    let mut quasi: Vec<(f64, f64, f64)> = Vec::new();

    for (eps, j, class, corr, _best_tau) in rows {
        let key = match class {
            Classification::Locked(n) => {
                by_period.entry(*n).or_default().push((*eps, *j, corr.abs()));
                format!("period-{}", n)
            }
            Classification::QuasiPeriodic => {
                quasi.push((*eps, *j, corr.abs()));
                "quasi-periodic".to_string()
            }
            Classification::Thermal => "thermal".to_string(),
        };
        *counts.entry(key).or_default() += 1;
    }

    println!("\nSweep complete: {} grid points", rows.len());
    for (k, v) in &counts {
        println!("  {:<16} {}", k, v);
    }

    for (period, mut points) in by_period {
        if period == 2 { continue; }  // not interesting, that's what we already have
        println!("\nPeriod-{} candidates (top 5 by |C(tau)|):", period);
        points.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        for (eps, j, c) in points.iter().take(5) {
            println!("  eps={:.3} J={:.3}  |C({})|={:.3}", eps, j, period, c);
        }
    }
}

