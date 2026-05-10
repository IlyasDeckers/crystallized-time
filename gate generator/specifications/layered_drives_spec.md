# Layered Drives — Multiple Drive Frequencies on a Single Chain

*Companion specification to `stage_1_2_spec.md`, `stage_2-5_spec.md`, and `stage_3_plus_reference.md`. Parking spec — research direction, not a commitment.*

`Status: parked. Speculative substrate extension. Implement after domain walls.`

---

## Purpose

The current substrate is driven by a single periodic kick: every 25 ticks, every spin gets rotated by `(1 - ε)π` around the x-axis. This is the minimal driving condition for the period-doubled time-crystal phase, and it works.

This specification describes adding a **second simultaneous drive** at a different period, alongside the existing one. The chain is then driven by the superposition of two periodic schedules. Depending on the ratio of the two periods, the combined drive is either commensurate (a longer but still finite period equal to the lcm) or incommensurate (no period at all — the drive is quasi-periodic).

The motivation is two-fold:

**A single chain might produce polyrhythmic content under layered drives.** The framework's §3.3 polyrhythm argument is currently realized through *two chains* locked at different sub-harmonics of *one* drive (Stage 3). Layered drives offer an alternative architecture: *one chain* potentially locking at sub-harmonics of *two drives*, producing rhythm at F/2, G/3, and the beat between them. If this works, the polyrhythmic two-chain might not be the only path to polyrhythm — it might not even be the most economical one.

**Quasi-periodic time crystals are a real and active research area.** Recent theoretical and experimental work shows that driven systems with multiple incommensurate drive frequencies can host stable sub-harmonic phases of matter that are richer than the periodic case. Whether the project's specific substrate (small disordered classical chain) lands in that regime is unknown. Finding out is interesting on its own terms and produces musical material if it works.

This is the most physics-novel of the parked extensions. The hopeful outcome is a single chain with a wider repertoire of stable behaviors than it currently has. The cautious outcome is that the parameter window for layered locking is too narrow to be musically reliable for this substrate. Either result is informative.

---

## Why each piece is here

**Two drives, not three or more.** The smallest extension that produces the qualitative new behavior. Three drives multiply the parameter space and make the regime-finding work intractable on a small chain. Two is enough to see whether quasi-periodic locking is reachable at all.

**Same kick mechanism, different schedule.** The drives are structurally identical to the existing one — angle, axis, applied uniformly to every site. Only the period and (optionally) the rotation axis differ. This minimizes the number of new things being introduced at once. If the chain doesn't sustain a quasi-periodic phase even with structurally simple second drives, more elaborate second-drive shapes won't save it.

**Drives are global.** Site-dependent or partially-applied drives are interesting and live in their own space (see the localized-perturbations spec for the closest cousin), but they break a different symmetry than what this spec is exploring. Layered drives keep the spatial structure of the existing drive intact and only change the temporal structure.

**Optional axis variation.** Two drives around the same axis (both x) commute with each other — their order doesn't matter, and the combined effect is equivalent to applying their angles' sum. Two drives around different axes (x and y) do *not* commute, and the relative timing between them matters in a way that produces qualitatively different dynamics. Worth supporting both — the same-axis case is a simpler starting point, the different-axis case is where the more interesting dynamics live.

**No commitment to specific period ratios.** The space of interesting ratios is wide. Simple ratios (3:2, 4:3, 5:4) probe commensurate behavior at lcm = 6, 12, 20 drive ticks. Irrational ratios (golden ratio, √2, etc.) probe true quasi-periodicity. Both should be supported by the same machinery; the choice is a config decision.

---

## What changes in the code

The drive logic currently lives in two places: `SpinChain::apply_drive_pulse` (the kick itself) and `SpinChain::step` (the trigger that fires the kick every `ticks_per_period` ticks). Both need to generalize to a list of drives.

### State additions

```rust
pub struct DriveSchedule {
    /// Period in integration ticks.
    pub ticks_per_period: u32,
    /// Rotation axis: 'x', 'y', or 'z'.
    pub axis: char,
    /// Pulse angle scale. Final angle is (1 - eps) * pi for the primary
    /// drive; for the secondary, configurable independently.
    pub angle: f64,
    /// Tick offset — when the schedule's first kick lands. Allows two drives
    /// to be locked in phase, anti-phase, or arbitrary relative phase.
    pub phase_offset: u32,
}

pub struct PhysicsConfig {
    // existing fields ...
    /// List of drive schedules. The Stage 1-2 default is a single drive
    /// matching the existing behavior.
    pub drives: Vec<DriveSchedule>,
}
```

The existing `eps`, `ticks_per_period` fields collapse into the first entry of `drives`. A migration that preserves Stage 1–2 default behavior:

```rust
impl Default for PhysicsConfig {
    fn default() -> Self {
        Self {
            // ... existing fields ...
            drives: vec![DriveSchedule {
                ticks_per_period: 25,
                axis: 'x',
                angle: (1.0 - 0.01) * std::f64::consts::PI,
                phase_offset: 0,
            }],
        }
    }
}
```

### Step logic

`SpinChain::step` checks each drive at the end of every tick:

```rust
self.tick += 1;
for drive in &self.config.drives {
    let effective_tick = self.tick.saturating_sub(drive.phase_offset as u64);
    if effective_tick > 0 && effective_tick % drive.ticks_per_period as u64 == 0 {
        self.apply_drive_pulse(drive);
    }
}
```

Order matters when two drives fire on the same tick. The convention: drives are applied in the order they appear in `drives`. For non-commuting axes (x then y vs y then x) the result will differ; for the same axis it doesn't.

### Pulse application

`apply_drive_pulse` takes a `&DriveSchedule` instead of using the chain's stored eps:

```rust
fn apply_drive_pulse(&mut self, drive: &DriveSchedule) {
    let (c, s) = (drive.angle.cos(), drive.angle.sin());
    for spin in self.spins.iter_mut() {
        match drive.axis {
            'x' => {
                let sy_new = spin[1] * c - spin[2] * s;
                let sz_new = spin[1] * s + spin[2] * c;
                spin[1] = sy_new;
                spin[2] = sz_new;
            }
            'y' => {
                let sx_new =  spin[0] * c + spin[2] * s;
                let sz_new = -spin[0] * s + spin[2] * c;
                spin[0] = sx_new;
                spin[2] = sz_new;
            }
            'z' => {
                let sx_new = spin[0] * c - spin[1] * s;
                let sy_new = spin[0] * s + spin[1] * c;
                spin[0] = sx_new;
                spin[1] = sy_new;
            }
            _ => {} // ignore unknown axis silently
        }
    }
}
```

The integration code (Landau-Lifshitz step + thermal noise) is unchanged.

### CLI additions

A second drive should be configurable from the command line for quick exploration. Suggested flags:

```
--drive2-period <ticks>      Period of the second drive in integration ticks.
--drive2-axis <x|y|z>        Rotation axis for the second drive. Default: y.
--drive2-angle <radians>     Rotation angle for the second drive. Default: pi/2.
--drive2-offset <ticks>      Phase offset of the second drive. Default: 0.
```

Absence of `--drive2-period` keeps the Stage 1–2 single-drive behavior. Presence enables the second drive with the configured parameters.

For ratio exploration without thinking in tick counts:

```
--drive2-ratio <num:den>     Second drive period as a ratio of the first.
                             E.g. 3:2 means second period = (2/3) * first.
```

(Period scales inversely with frequency — a 3:2 frequency ratio means the second period is 2/3 of the first. The ratio is interpreted as frequency ratio, which is the more intuitive musical parameter.)

---

## Sonification implications

The existing zero-crossing detector (`events.rs`) and substrate clock (`clock.rs`) work unchanged. Their input is `sz(i)` and `global_magnetization()` — pure observables of the chain state, indifferent to how the chain got there.

What changes is the *content* of those observables. Under layered drives, possible behaviors include:

**The chain locks at the primary drive's sub-harmonic and ignores the secondary.** The secondary drive contributes a continuous low-amplitude perturbation that doesn't break the lock. Musically: roughly the same output as the single-drive case, but with subtle phase-modulation-like coloring on each event. Possibly the *most likely* outcome at small secondary-drive amplitudes.

**The chain locks at a combined sub-harmonic.** The two drives both contribute to determining the lock period; the chain settles at some F/n and G/m simultaneously, with the joint period being lcm-related. Musically: polyrhythmic content from one chain. Probably needs commensurate ratios and carefully chosen secondary-drive parameters.

**The chain develops quasi-periodic stable behavior.** Under incommensurate drives, the chain enters a regime that's neither periodic nor thermal — events fire in patterns that don't repeat but are statistically stable. Musically: rhythm that almost-repeats without ever exactly repeating, audibly different from both periodic locking and noise.

**The chain thermalizes.** The two drives interfere destructively, drive the chain harder than disorder can stabilize, and the lock collapses. Musically: noise. Probably the outcome at large secondary-drive amplitudes or unfortunate ratios.

The substrate clock's behavior under layered drives is itself an interesting signal. In the locked-at-combined-sub-harmonic case it should fire at the joint period and degrade smoothly with secondary-drive amplitude. In the quasi-periodic case it should fire at quasi-stable rates that wobble around an average. In the thermal case it stops, exactly as in the single-drive thermal phase.

---

## Parameter regime exploration

This is the hardest part. The parameter space is too large to characterize by ear alone:

- Primary drive: existing parameters (eps, J, W, kT, ticks_per_period).
- Secondary drive: period (or ratio), axis, angle, phase offset.
- Combined system: how all of the above interact.

The recommended approach mirrors the Stage 3 phase-sweep:

1. **Fix the primary drive** at its working time-crystal parameters (the Stage 1–2 defaults).
2. **Sweep the secondary drive's period and angle** over a grid. For each combination:
    - Run the chain for, say, 200 drive periods.
    - Compute the global magnetization autocorrelation `⟨M(t) M(t+τ)⟩` over a window after the first 50 periods (skipping the transient).
    - Identify peaks in the autocorrelation — they correspond to the dominant locked periods.
    - Classify the run: single-period locked, multi-period locked, quasi-periodic, thermal.
3. **Map the phase diagram.** Record which (period, angle) regions land in which class. The regions where multi-period or quasi-periodic locking are stable are the musically interesting ones.
4. **Pick representative points** from each interesting region for extended listening.

This is several days of work. It produces a phase diagram, not a parameter setting — the actual musical decisions come from listening to chosen points, not from the diagram alone.

The sweep tool is a separate binary (or a `--sweep` mode of the main binary), not part of the realtime path. Same pattern as the Stage 3 phase-sweep tool.

---

## What's intentionally not in scope

- **More than two drives.** The combinatorics of three-drive phase diagrams are intractable for a project this size. If two drives prove fruitful, three becomes a focused future question; until then, no.
- **Site-dependent drives.** A drive that hits only some sites is a different kind of generalization (spatial rather than temporal) and lives in a separate spec. The localized-perturbations spec covers the closely-adjacent case of single-site, single-time kicks.
- **Drive amplitude modulation.** A drive whose angle changes over time (slowly) is a third axis of generalization. It's interesting but multiplies the parameter space again. Defer.
- **Adaptive drive parameters.** The drives are fixed at startup. A chain whose drives respond to its own state is a feedback loop with its own stability questions; not for this spec.
- **Quantum substrate.** This spec describes the classical substrate. The quantum version (Stage 5) of layered drives is a richer problem — Floquet circuits with multiple drives have their own quasi-energy structure — but the classical case is the right place to start.

---

## Open questions

**Whether the chain can sustain quasi-periodic locking at all** at this size and disorder strength. Published results on quasi-periodic time crystals are mostly for specific lattice models in specific regimes; whether the project's small disordered classical chain has a viable quasi-periodic regime is genuinely unknown.

**Whether quasi-periodic dynamics are distinguishable by ear** from layered single-frequency drives. If a 3:2 frequency ratio combined drive sounds essentially the same as a single drive at the average frequency with a slightly modulated angle, the quasi-periodic part isn't earning its keep.

**Whether the regime is stable enough to be musically reliable.** Even if quasi-periodic locking exists, if the phase boundary is so narrow that small parameter drift kicks the chain in and out of the regime randomly, it won't work as a substrate for composition.

**Whether two drives on one chain produce the same musical effect as one drive on two coupled chains** (the Stage 3 architecture). If they do, this spec offers an architectural alternative to Stage 3. If they don't — if the coupled-chain case has properties this spec can't reach — then both architectures are independently worth pursuing for different music.

**The right starting ratio.** Simple integer ratios (3:2, 4:3) are the safest first targets. Irrational ratios (golden, √2) are the most genuinely novel. Picking which to prototype first is itself a compositional decision.

---

## Definition of done

This spec is parked, so "done" is provisional. If implemented:

1. The `drives` list in `PhysicsConfig` works for one drive (existing behavior preserved exactly) and for two drives (new behavior available).
2. The CLI flags configure a second drive without code changes.
3. A parameter-sweep tool exists, characterizes the (period, angle) phase diagram for at least one fixed set of primary-drive parameters, and produces a classification (single-period / multi-period / quasi-periodic / thermal) for each grid point.
4. At least one parameter setting is found where the chain demonstrably locks at a non-trivial combined sub-harmonic, audibly producing rhythmic content the single-drive substrate doesn't produce.
5. Listening sessions at representative points from each phase-diagram region inform a written assessment of whether layered drives are musically worth pursuing further. The assessment may conclude *no*, and that's a valid result.

---

## Migration path

If layered drives prove valuable, they integrate cleanly with the rest of the roadmap:

- **Stage 3 (second chain)** can adopt layered drives independently per chain. Chain A might run a single drive for clean f/2; chain B might run layered drives for quasi-periodic content. The two chains' outputs combine the way Stage 3 already plans for.
- **Stage 4 (visitor perturbation)** can target the secondary drive's parameters specifically. The primary drive holds the chain's anchor; the secondary becomes the perturbation channel. This is a much more controllable interaction than perturbing a single drive's parameters directly.
- **Stage 5 (quantum substrate)** the layered-drive Floquet circuit is straightforward to construct from the classical version — the kick operations are unitary gates either way, and the gate sequence per drive period is just longer.

If layered drives don't prove valuable, the work of building the parameter-sweep infrastructure is still useful — the same machinery applies to Stage 3's phase-sweep for finding f/3.

---

*Companion spec for Crystallized Time | Layered Drives | Parking spec, future work*