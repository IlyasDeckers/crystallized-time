# Localized Perturbations — Playing the Chain

*Companion specification to `stage_1_2_spec.md`, `stage_2-5_spec.md`, and `stage_3_plus_reference.md`. Parking spec — research direction, not a commitment.*

`Status: parked. Substrate-as-instrument extension. Implement after domain walls.`

---

## Purpose

Currently the chain is something the system *listens to*. The substrate runs autonomously — driven globally, perturbed only by thermal noise — and the program reads its dynamics out as music. There is no input channel. The visitor (in the eventual installation) and the operator (right now, at the desk) can change parameters but cannot directly disturb the chain's state.

This specification describes adding a **localized perturbation** mechanism: a way to deliberately disturb a single site at a single time, and a routing layer that translates external signals (MIDI input first, sensor data later) into those disturbances. The chain becomes a thing that can be *played into*, with its own physics-determined response.

The motivation is that this is the smallest extension that turns the substrate from a generator into an instrument. The project's framework document describes the visitor as "part of the dynamical system that produces the work" — this specification is the operational implementation of that claim. It also opens a much richer interaction surface for the operator, who can probe the chain's response in different phases, develop intuition for what the substrate "wants" to do, and use that intuition compositionally.

This is also the structurally simplest path to Stage 4 (visitor perturbation) of the existing roadmap. Once a localized perturbation mechanism exists, the question of what perturbs the chain becomes a routing question, and visitor sensors are one routing among several.

---

## Why each piece is here

**Localized, not global.** The existing drive is global — every site gets the same kick at the same time. Localized perturbations break that symmetry: one site is disturbed, the rest aren't, and the chain has to redistribute. The asymmetry is what makes the chain's response *informative* — under a global perturbation the chain's symmetric response gives you no spatial information; under a localized one, the response tells you about how disturbances propagate, which depends on the phase.

**Single-site, not multi-site patterns.** A perturbation that hits sites 0, 4, 8 simultaneously is a multi-site pattern — interesting, but it composes from single-site perturbations applied at the same tick. The atomic operation is one site, one time. Multi-site patterns are sequences of atomic operations.

**Multiple perturbation kinds.** A flip (negate sz), a rotation (small angle around some axis), and a field spike (transient addition to the local field) produce qualitatively different responses. Flip is a violent perturbation — large amplitude, takes a long time to absorb. Rotation is gentle — small amplitude, decays fast. Field spike is a *forced* perturbation that lasts for one tick rather than just changing the state. Different musical characters; all worth supporting.

**Decoupled from input source.** The mechanism that perturbs the chain knows nothing about where the perturbation request came from. MIDI input, file watch, sensor data, scheduled scripts — these are all routing layers that produce perturbation requests. Keeping the chain side and the input side separate means each can evolve independently, and the perturbation infrastructure earns its place even when the eventual visitor sensors arrive.

**MIDI input first.** It's the lowest-cost input channel — every piece of music gear in the project's environment produces MIDI, and `midir` already supports input as well as output. A MIDI keyboard, a controller pad, a sequencer, a DAW — any of these become a way to play the chain. Sensor input (Stage 4 of the roadmap) reuses the same perturbation mechanism with a different routing layer.

---

## What changes in the code

Three additions: a perturbation method on the chain, a perturbation source (MIDI input listener), and a routing layer that translates input events into perturbation calls.

### Chain method

`SpinChain` gains a method that applies a perturbation between integration steps:

```rust
pub enum PerturbationKind {
    /// Negate the z-component of the spin (and renormalize).
    Flip,
    /// Rotate by `angle` around the named axis.
    Rotate { axis: char, angle: f64 },
    /// Add a transient field that lasts one integration step.
    FieldSpike { delta: Vec3 },
}

impl SpinChain {
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
            }
            PerturbationKind::FieldSpike { delta } => {
                // Recorded as a one-tick field addition; consumed and cleared
                // by the next step.
                self.pending_field_deltas[site] = Some(delta);
            }
        }
    }
}
```

`pending_field_deltas: Vec<Option<Vec3>>` is new chain state — it's read inside `step` when computing the effective field at each site, then cleared. This makes field spikes single-tick, which matches the user mental model of "kick this site once."

### Perturbation source

A new module, `input.rs`, owning a MIDI input connection. Pattern matches the existing `midi.rs` for output:

```rust
pub struct MidiInputListener {
    /// Receiver end of a channel; events are pushed by the midir input thread.
    rx: std::sync::mpsc::Receiver<RawMidiMessage>,
    /// Connection held to keep the input thread alive.
    _conn: midir::MidiInputConnection<()>,
}

impl MidiInputListener {
    pub fn open(port_index: usize) -> Result<Self, ...> { ... }
    pub fn list_ports() -> Result<Vec<String>, ...> { ... }
    /// Drain any pending messages without blocking. Called once per tick.
    pub fn poll(&self) -> Vec<RawMidiMessage> { ... }
}
```

The `midir` input callback is already non-blocking and runs on its own thread; pushing into an `mpsc::Sender` from there is the standard pattern. The main loop polls the receiver each tick and drains whatever has accumulated. At realistic input rates (a human playing notes), the drain is usually 0–2 messages per tick.

### Routing layer

A new module, `perturbation.rs`, that translates `RawMidiMessage`s into chain perturbations:

```rust
pub struct PerturbationRouter {
    config: PerturbationConfig,
}

pub struct PerturbationConfig {
    /// How a MIDI note number maps to a site index.
    /// Default: site = (note - base_note) mod n_sites.
    pub base_note: u8,
    /// What kind of perturbation a note-on produces.
    pub kind: PerturbationKindConfig,
    /// Velocity-to-magnitude scaling (multiplier applied to angle or field
    /// magnitude). 1.0 means velocity 127 = full magnitude, velocity 0 = 0.
    pub velocity_scale: f64,
}

pub enum PerturbationKindConfig {
    Flip,
    Rotate { axis: char, base_angle: f64 },
    FieldSpike { axis: char, base_magnitude: f64 },
}

impl PerturbationRouter {
    pub fn route(&self, msg: RawMidiMessage, n_sites: usize) -> Option<(usize, PerturbationKind)> { ... }
}
```

The main loop calls `router.route()` on each polled MIDI message and applies the resulting perturbation to the chain via `chain.perturb()`. Routing is per-message; no buffering, no scheduling — perturbations land on whichever tick they arrive on.

### Loop integration

The main loop's per-tick body grows by one block, sandwiched between the chain step and the event detection:

```rust
for tick in 1..=total_ticks {
    // ... existing shutdown check ...

    chain.step(&mut rng);

    // NEW: drain MIDI input and perturb the chain.
    if let Some(input) = &midi_input {
        for msg in input.poll() {
            if let Some((site, kind)) = router.route(msg, config.physics.n_sites) {
                chain.perturb(site, kind);
            }
        }
    }

    let events = detector.check(&chain);
    // ... existing event handling ...

    clock_emitter.tick(&chain, &midi_sender);

    // ... existing pacing ...
}
```

Position matters: perturbations apply *after* the integration step but *before* event detection. So a perturbation arriving on tick T is visible to the chain's state on tick T and can produce events from that same tick if it crosses thresholds immediately. In practice, large flips will produce immediate events; small rotations will produce events later, after the chain has redistributed.

### CLI additions

```
--list-input-ports
    Print available MIDI input ports and exit.

--input-port <N>
    Open MIDI input port N. Without this flag, no input is read and the
    chain runs in its current autonomous mode.

--perturbation-kind <flip|rotate|spike>
    What kind of perturbation incoming notes produce. Default: rotate.

--perturbation-magnitude <float>
    Base magnitude (angle in radians for rotate, field strength for spike).
    Default: 0.3 (small rotation, intentionally subtle).

--base-note <0..127>
    MIDI note that maps to site 0. Default: 60 (middle C).
```

Absence of `--input-port` keeps the existing autonomous behavior. Presence enables the perturbation channel.

---

## State additions

### Per chain

- `pending_field_deltas: Vec<Option<Vec3>>` — one slot per site, set by `perturb` with a `FieldSpike`, consumed and cleared by the next `step`.

### Globally

- An `Option<MidiInputListener>` and an `Option<PerturbationRouter>` on the main loop, both `None` if `--input-port` isn't set.

### Per perturbation router

- The config struct above. No mutable state; routing is a pure function of message + config.

---

## Sonification implications

The existing event detector and substrate clock are unaffected by the *mechanism* of perturbation — they read `sz(i)` and `global_magnetization()` and produce events on zero-crossings. What changes is the content of those signals.

A flip perturbation on site 4 will, at the moment it happens, produce an immediate sign change on site 4 — the detector sees a zero-crossing and emits an event. Subsequent ticks reveal how the chain absorbs that disturbance: the perturbation propagates into the neighbors, which may also flip, producing a small cascade of events from sites 3 and 5 over the next several drive periods. In the locked phase the cascade is short and contained; in the thermal phase the perturbation just gets absorbed into noise; near the phase boundary the cascade rings.

A rotation perturbation produces no immediate event (the small angle doesn't push sz across the threshold), but it changes the chain's trajectory subtly — the next drive cycle's lock is slightly modulated, and events from the perturbed site shift in their timing relative to where they'd otherwise have fired. Repeated small rotations build up. Musically: rotation perturbations bend the chain rather than striking it.

A field spike is somewhere between the two: the spike biases the integration step that follows, producing a transient drift on the affected site that can but doesn't always cross thresholds.

The substrate clock under perturbation is itself an interesting signal. In the deeply locked phase, a single perturbation barely affects ⟨M⟩ and the clock continues unphased. Near the phase boundary, perturbations transiently disrupt the global magnetization and the clock briefly stutters before recovering. In the thermal phase the clock isn't running anyway. So the clock's response to perturbation is a real-time indicator of where in phase space the chain currently sits — useful for the operator and audible to the listener as a tightening or loosening of the rhythm.

---

## Open questions

**The right perturbation magnitude.** Too large and every input flip-kicks the chain hard, the music becomes a series of cascades, the substrate's autonomy disappears under the input rate. Too small and the operator can't hear that they're affecting anything. The setting that produces "I can feel myself influencing the music without controlling it" is the target, and finding it requires playing.

**Whether the chain's response is legible enough to play.** The framework's §5 describes the central design question: how *legibly* should perturbation translate to response. Tighter coupling makes the chain a regular instrument; looser coupling makes it a thing that responds without obeying. The right setting is probably looser than feels intuitive at first, but how much looser is empirical.

**Velocity sensitivity.** Linear velocity-to-magnitude works as a starting point. Whether the chain's response is itself linear in input magnitude (a velocity-127 flip is twice as audible as a velocity-64 flip) or non-linear (small inputs get absorbed, large inputs cause cascades) is a property of the substrate that the operator will discover by playing.

**Site-mapping from notes.** The default of `site = (note - 60) mod 8` is the obvious starting point, but it makes the chain feel like a small piano. Alternatives: chromatic notes within an octave map to specific perturbation *kinds* on a fixed site; black keys vs white keys map to different axes; controller knobs map to continuous parameters of the perturbation rather than discrete events. Each gives a different instrument character.

**Whether perturbations should be quantized to drive boundaries.** Currently the spec applies them on whichever tick they arrive. An alternative is to queue them and apply them only on the next drive boundary, so all perturbations land "in time" with the chain's clock. The unquantized version is more responsive; the quantized version sounds more rhythmically coherent. Both have arguments; possibly worth a flag.

**Polyphony.** Multiple notes held simultaneously produce multiple perturbations on different sites. The chain handles this naturally — each perturbation independently disturbs its site, and the chain's evolution from then on sees the combined disturbed state. So polyphony is "free" from the chain's perspective. But the *musical* result of holding a chord on the input depends on whether the chain treats the multi-site disturbance as a single coherent event or as several uncorrelated events. Worth exploring.

---

## What's intentionally not in scope

- **Continuous perturbation streams.** Every-tick perturbations from continuous-CC inputs (modulation wheel, expression pedal) are a different kind of input than discrete events. They're worth supporting eventually, but they probe a different dynamical question (forced response to continuous driving) than discrete kicks. Separate spec.
- **Perturbations that target couplings or fields persistently.** The spec covers transient single-tick disturbances. A perturbation that *changes a coupling J_ij persistently* is a parameter change rather than a perturbation, and lives in the existing parameter-mutation infrastructure that Stage 4 (visitor perturbation) will use.
- **Output routed back to input.** Self-perturbing chains are a feedback loop with their own stability questions and their own musical character. Not for this spec.
- **Multi-channel input.** A single input port is enough for the prototype. Multiple sensors per visitor, multiple visitors with separate sensors — these are routing-layer extensions, not chain-side ones.
- **Recording and playback of perturbation sequences.** Useful for reproducible experiments. Not core to the substrate-as-instrument question. Add later if needed.

---

## Definition of done

This spec is parked, so "done" is provisional. If implemented:

1. `chain.perturb(site, kind)` works for all three perturbation kinds, applies the disturbance correctly, and integrates with the existing `step` cleanly (field spikes consumed within one tick, no leaks of perturbation state across runs).
2. MIDI input flag connects to a port, routes incoming note-ons to chain perturbations, and the chain produces audible response within a few drive periods of input arrival.
3. Held by ear: the operator can play a MIDI keyboard into the chain, hear the substrate respond, and develop a sense for what kinds of input produce what kinds of response in different phases.
4. The chain's autonomous behavior is preserved when no input is connected — Stage 1–2 / Stage 2.5 behavior is the default and is not affected by the addition.
5. A 10-minute exploratory session, recorded as MIDI input + audio output, demonstrates that the chain has a recognizable "playing character" — that two operators given the same chain configuration but different inputs would produce audibly different but coherent musical results.

---

## Migration path

If localized perturbations prove valuable:

- **Stage 4 (visitor perturbation)** uses the same `chain.perturb()` interface with a sensor-driven routing layer in place of the MIDI router. The chain side doesn't need to change. The sensor module (camera, ultrasonic, microphone) translates physical-world signals into perturbation requests using whatever mapping the installation requires.
- **The roadmap's parameter-mutation perturbations** (Stage 4 in its existing form — perturbing eps, J, W, kT) coexist with localized perturbations. They affect the chain on different timescales and through different channels: parameter perturbations slowly reshape the phase the chain is in, localized perturbations are momentary kicks within the current phase. Both are valid; both are useful; they don't conflict.
- **Stage 3 (second chain)** extends naturally — the input router can target either chain, and inputs that hit specific MIDI channels can be routed to specific chains. Chain A's keyboard is octave 4; chain B's is octave 5.
- **Layered drives** (the parking spec) interact interestingly with localized perturbations: a single-site perturbation under layered drives explores the chain's response in a richer dynamical regime, where the response itself might be quasi-periodic or multi-period rather than just decay-or-cascade.
- **Stage 5 (quantum substrate)** gets a slightly different perturbation operator. Classical: directly modify the spin vector. Quantum: apply a single-qubit gate on the affected site between Floquet evolutions. The gate is a more natural primitive than the classical "set the state" operation, and produces cleaner dynamics. The router and input layer are unchanged.

---

*Companion spec for Crystallized Time | Localized Perturbations | Parking spec, future work*