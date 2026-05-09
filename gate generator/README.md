# Time-Crystal Gate Generator

*Reference document for the first working version of Crystallized Time's substrate-driven event generator.*

---

## Purpose

Produce a stream of MIDI gate events from a simulated classical disordered spin chain exhibiting period-doubled Floquet dynamics. Four output voices, each driven by one site of the chain, emitting MIDI note-on / note-off pairs in real time when that site's σ^z crosses zero. Output routes to a hardware MIDI-to-CV interface and on to eurorack.

This is the smallest version of the system that validates the substrate's musical behavior end-to-end.

---

## Why each piece is here

**Classical disordered spin chain.** Cheapest substrate that exhibits the three properties the project needs — period-multiplication, rigidity, phase transitions. Already validated to work in the JS prototype.

**Floquet drive.** The thing that produces period-multiplication. Without periodic kicking, the chain just relaxes — there's no clock for events to happen against.

**Disorder.** Without it, the chain thermalizes within tens of periods and the time-crystal phase doesn't survive. Site-to-site variation in the local Z-field stabilizes the sub-harmonic locking.

**Zero-crossing detection on σ^z.** Simplest possible event criterion. Each site flips between "up" and "down" once per crystal period (twice the drive period in the f/2 phase), so each crossing is one event. Other readouts are possible later; this one gets us to MIDI fastest.

**Four voices.** Not one, because one voice doesn't validate that the chain is doing something coupled and interesting — it could be any oscillator.

**Real-time MIDI output.** Triggers eurorack live.

---

## System parameters

### Physics parameters (substrate behavior)

| Parameter | Symbol | Default | Rationale |
|---|---|---|---|
| Number of sites | N | 8 | Big enough for disorder to matter, small enough to be instant |
| Integration step (sim units) | dt | 0.04 | 25 steps per drive period — Euler-stable, matches JS prototype |
| Drive imperfection | ε | 0.10 | Inside the time-crystal phase per the prototype |
| Coupling strength | J | 1.2 | Strong enough to stabilize period-doubling |
| Disorder width | W | 2.0 | Wide enough to localize, narrow enough to keep coupling effective |
| Effective temperature | kT | 0.05 | Low; higher values push toward thermal phase |
| Output sites | — | [0, 2, 4, 6] | Spaced out along the chain, not adjacent |

### Tempo / timing parameters (musical behavior)

| Parameter | Default | Rationale |
|---|---|---|
| Drive period (real time) | 0.5 s | One drive period = half a second; in f/2 phase each site flips every ~1 s |
| Effective BPM | 120 | Drive period of 0.5 s ≈ quarter note at 120 BPM |
| Per-site debounce | 4 ticks | Prevents jitter near the zero crossing emitting multiple events |
| MIDI gate length | 50 ms | Long enough for a CV interface to register, short enough not to overlap |

The drive period is exposed as a runtime parameter so tempo can be adjusted without touching the physics. Internally this scales the wall-clock duration of one tick: `tick_duration = drive_period / 25`. The simulation's logical time and event-emission criteria are unchanged; only the rate at which the loop yields to the MIDI clock changes.

### Tempo control

Single tempo parameter, expressed as **drive period in seconds** or equivalently **BPM** (BPM = 60 / drive_period when one drive period maps to one beat). Changeable at runtime; the simulation re-paces but keeps its state. Default: 120 BPM (0.5 s drive period).

---

## State

### Per site `i`

- Spin vector `[sx, sy, sz]`, unit length
- Local field vector `[hx, hy, hz]` — random, fixed at initialization
- `prev_sz` — for zero-crossing detection
- `last_event_tick` — for debouncing

### Per nearest-neighbor pair `(i, i+1)`

- Coupling strength `J_ij` — random in `[0.7·J, 1.3·J]`, fixed at init

### Global

- Tick counter (monotonic, u64)
- Simulation time (sim units, f64)
- Wall-clock start time
- RNG (seeded — reproducibility matters for debugging the substrate)

---

## Loop

```text
init chain:
    for each site:
        random spin near +z or -z pole
        random local field, random_z component scaled by W
    for each pair:
        random coupling near J

main loop:
    for each tick:
        integrate one dt step (Landau-Lifshitz-like + thermal noise)
        if tick is at drive boundary (every 25 ticks):
            apply (1-ε)π rotation around x to every site
        for each output site:
            if sz changed sign with |Δsz| above threshold
               and (tick - last_event_tick) > debounce:
                emit gate event (site, tick, intensity = min(1, |Δsz|))
                last_event_tick = tick
        sleep until wall-clock time matches expected tick time
```

### Integration step (per site)

For each site `i`:

```
h_eff = h_i + J_{i-1,i} * s_{i-1}.z * z_hat + J_{i,i+1} * s_{i+1}.z * z_hat
torque = s_i × h_eff
noise = sqrt(2 * kT * dt) * gaussian_random_3vec
s_i_new = s_i + torque * dt + noise
s_i_new = normalize(s_i_new)
```

Boundary sites have only one neighbor.

### Drive pulse

Every 25 ticks, every spin is rotated by angle `(1 - ε) * π` around the x-axis:

```
s_y_new = s_y * cos(angle) - s_z * sin(angle)
s_z_new = s_y * sin(angle) + s_z * cos(angle)
s_x unchanged
```

This is the kick that produces period-doubling.

### Zero-crossing detection

A crossing is registered when `prev_sz < -threshold` and `current_sz > +threshold` (or vice versa) — i.e. a *real* sign change with margin, not just noise wobbling around zero. Threshold default: `0.15`. Intensity is `min(1.0, |sz - prev_sz|)`.

---

## Event model

```rust
struct GateEvent {
    site: usize,        // index into chain (0..N)
    tick: u64,          // monotonic tick counter
    intensity: f32,     // 0.0 to 1.0
}
```

Stage 1: emit via `println!` (tick, site, intensity).
Stage 2: route to MIDI.

---

## MIDI mapping

Four output sites → four MIDI channels (1, 2, 3, 4).

Each `GateEvent`:
1. Send note-on on channel `site_index + 1`, pitch C3 (MIDI 48), velocity from `intensity * 127`.
2. Schedule note-off after `MIDI_GATE_LENGTH` (50 ms) on the same channel and pitch.

Pitch is irrelevant for triggering eurorack — the gate is the signal. Velocity carries intensity, which can be used as an accent CV downstream if desired.

### Library

`midir` (the standard Rust real-time MIDI crate, cross-platform). Backend uses the OS-native MIDI: CoreMIDI on macOS, ALSA on Linux, WinMM on Windows. Output to a virtual port the user can route in their DAW or directly to a hardware interface.

---

## Tempo behavior in detail

Wall-clock pacing is implemented as a sleep at the end of each tick, targeting the expected wall-clock time:

```
expected_time = start_time + tick_count * (drive_period / 25)
sleep until expected_time
```

This couples simulation time to wall-clock time. If the simulation step takes longer than the tick budget (unlikely for N=8 but worth handling), the loop falls behind without crashing — events still emit, just late. We can add drift detection later if it matters.

Tempo can be changed at runtime by mutating `drive_period`. The next tick will re-target based on the new value. State is preserved across tempo changes — it's just the loop pacing that changes.

---

## What's out of scope for Stage 1–2

- Visitor perturbation (parameters fixed at start)
- Other observables (magnetization, domain walls, correlations)
- Phase detection / regime classification
- Multiple chains or polyrhythmic coupling (stage 3+)
- Quantum substrate (stage 4+)
- GUI / parameter live-tuning
- Multi-run analysis or parameter sweeping
- Audio synthesis directly from the chain (eurorack handles sound)

---

## Definition of done

- Process runs from the command line, opens a MIDI output port (named, visible to the OS).
- Four voices emit gates in real time corresponding to the four output sites of the simulated chain.
- Tempo (drive period / BPM) is configurable at startup via CLI flag or constant.
- In the default-parameter regime, the four voices produce a recognizable pattern that holds for at least a minute of running — period-doubled, rhythmically coherent, not collapsing to silence and not thermalizing into noise.
- Hooked up to eurorack via MIDI-to-CV, the four voices trigger four gates that fire at the substrate's emergent rhythm.

---

## Open questions, resolved or deferred

| Question | Resolution |
|---|---|
| Real-time vs. file output | Real-time, via `midir`. |
| Integration scheme | Euler, matching the JS prototype. RK4 if we see drift. |
| Tempo control | Drive period in seconds, exposed as runtime parameter. Default 0.5 s ≈ 120 BPM. |
| Note pitch | Fixed C3. Pitch is irrelevant for gate signals. |
| Note length | 50 ms. Long enough for CV interfaces, short enough not to overlap. |
| RNG seeding | Seeded by default for reproducibility. CLI override possible later. |

---

## Migration path forward

This spec deliberately avoids commitments that would lock us into the classical-only substrate. The event-emission interface (`GateEvent` plus the MIDI router) is independent of how the events are produced. Stage 3+ replaces the inner physics loop with a quantrs2 Floquet circuit; the outer scaffold and the MIDI plumbing remain unchanged.

Polyrhythmic two-chain (the framework doc's §3.3) is a straightforward extension: a second chain runs in parallel, with a different disorder profile, weakly coupled through a shared drive pulse or a shared field. The event emitter sees more sites and routes them to more MIDI channels. No structural changes to the loop.

---

*Reference spec for Crystallized Time | Stage 1–2 | Companion to crystallized_time.md*
