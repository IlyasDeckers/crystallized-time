# Crystallized Time — Next Steps

*A roadmap for what comes after Stage 1–2.*

`Status: planning, post-first-working-version`

---

## Where we are

Stage 1–2 is working. A classical disordered Floquet spin chain runs in real time, four sites are sonified through zero-crossing detection, and the resulting gate events stream out as MIDI to a eurorack rig. The substrate produces emergent period-doubled rhythm; the texture is heterophonic (four voices sharing the 2T meter at different phases and lock strengths); the system is paced to wall-clock so it triggers downstream gear at a controllable tempo.

Roughly 200 lines of Rust across `chain.rs`, `events.rs`, `midi.rs`, plus configuration and CLI scaffolding. No quantum machinery yet, no visitor perturbation, no second chain, no GUI. By design — the goal of Stage 1–2 was to validate that the substrate's emergent dynamics are musically meaningful end-to-end, before adding complexity.

This document is the plan for what to do next.

---

## Stage 3 — Polyrhythmic two-chain

**The question this answers.** The current setup has heterophony (multiple voices, same meter) but not polyrhythm (multiple voices, different meters). Stage 3 produces genuine cross-rhythm: one chain locked at f/2, another locked at f/3, with the lcm-of-periods governing structural recurrence.

This is the framework document's §3.3 — the smallest extension that produces something Stage 1–2 cannot.

**What it requires.**

Two `SpinChain` instances with independent seeds, running in lockstep. Their drive pulses can fire on the same tick (synchronized drive) or stagger (independent drives) — the synchronized version is the structurally cleanest because the lcm recurrence becomes a real audible event rather than getting smeared.

The second chain needs to actually land in the f/3 phase, which is parameter-dependent. The default parameters of the first chain favor f/2. Finding parameters that favor f/3 is an empirical exercise, ideally done first as an extension of the existing single-chain code: a parameter sweep that runs a chain for, say, 100 periods at each of many `(eps, J, W)` combinations and detects the dominant period from the magnetization autocorrelation. Output is a phase diagram; the f/3 region (if it exists for our chain length) is where the second chain's parameters come from.

Optionally: weak coupling between the two chains. The framework document is explicit that the polyrhythmic-via-coupling case is the structurally interesting one — two chains that share a substrate, influencing each other through their physics rather than just running in parallel. The simplest coupling is a small additional term in each chain's effective field that depends on the *other* chain's average magnetization. Strong enough to be felt, weak enough not to forcibly synchronize the two.

**The implementation shape.** A new module, `dual_chain.rs`, that owns two `SpinChain`s and exposes a unified `step` that advances both. Two `EventDetector`s, one per chain, watching different MIDI channels. Eight output voices total: four on channels 1–4 from chain A, four on channels 5–8 from chain B. The MIDI module needs no changes — it already supports up to 16 channels.

Estimated effort: a day, plus however long the parameter sweep takes to identify a good f/3 configuration. The sweep is a standalone tool, not part of the realtime path.

**The musical question Stage 3 raises.** Whether the lcm recurrence is actually *audible*. f/2 against f/3 has a 6T common period — six drive periods between alignments. At BPM 120 (drive period 0.5s), that's three seconds. Long enough that the listener's sense of meter has time to settle, short enough that the recurrence registers when it happens. Whether this manifests as a perceptible "click into place" or just as continuous musical density is something we can't predict from theory. Has to be heard.

---

## Stage 4 — Visitor perturbation

**The question this answers.** The framework document's central premise is that the substrate's dynamics respond to the visitor's presence — that the system the visitor hears is not the system that exists independently of them, but a system whose trajectory is being shaped by the fact that they are in the room. Stage 4 makes that real.

**What it requires.**

A way to mutate the chain's parameters at runtime, in response to some external signal. Three input modalities, in order of "easiest to plumb today":

A MIDI CC input. Receive control-change messages from a controller — say, a knob on a MIDI controller — and map their values to one of `(eps, J, W, kT)`. This is the smallest possible loop: turn the knob, hear the substrate respond. Useful for parameter exploration even without an installation context.

A file watch. Re-read a small config file every few hundred ms; if its values changed, update the chain's parameters. Lets you tweak parameters from a text editor while the substrate runs. Useful for sessions where you want to record parameter sweeps with reproducible inputs.

Sensor input from an actual visitor proxy — webcam motion detection, ultrasonic distance sensor, microphone level. Real-world input, the closest to the installation's eventual interaction model. Higher engineering cost; needs a separate sensor module that polls hardware and translates to a parameter delta.

**The implementation shape.** A new module, `perturbation.rs`, that owns the input source and produces a stream of `ParameterDelta` values. The main loop reads from this stream and applies changes to the chain's `PhysicsConfig` between steps. The chain itself doesn't know it's being perturbed — its config just changes underneath.

The structural design question is *how aggressively* perturbations should map to parameter changes. The framework document's §5 frames this as the central design question of the installation — too tight and it becomes a theremin (every movement is a parameter change, the substrate's autonomy disappears); too loose and the visitor never notices they matter. The right setting is somewhere visitors discover the relationship through curiosity rather than instruction.

Initial implementation: a smoothing filter on the input, so visitor signals contribute gradually rather than instantly. Makes the relationship feel more like "influence" than "control."

Estimated effort: a day for MIDI CC input. Two days for file watch. A week or more for proper sensor integration depending on what hardware is available.

**The musical question Stage 4 raises.** Whether parameter-space movement is *legible* through sound. Pushing `kT` upward should audibly degrade the lock; pushing `eps` toward 0 should sharpen it. Whether the listener experiences these changes as "the music is responding" rather than "the music is randomly changing" depends on the smoothness, the mapping curve, and the time constants. Empirical question; iterate.

---

## Stage 5 — Quantum substrate

**The question this answers.** Whether the genuine quantum properties of a Floquet spin system — entanglement growth, coherent interference, true superposition — produce musically distinguishable output compared to the classical version. The framework document is appropriately humble about this: the *artistic* claims of the project don't require a quantum substrate, only the dynamical properties that the classical chain already exhibits. Quantum is not necessary; it's an additional axis of exploration.

**What it requires.**

Replace the inner loop of `SpinChain::step` with a Floquet circuit applied to a quantum state, simulated via quantrs2's state-vector backend. The circuit per drive period is roughly:

For each interaction step (one between drive pulses): apply RZZ gates between nearest neighbors with angle `J * dt`, apply RZ gates on each site with angle `h_i * dt`, optionally add weak depolarizing noise.

For each drive boundary: apply RX gates on each site with angle `(1 - eps) * π`.

Between Floquet periods: compute expectation values `⟨ψ|σ^z_i|ψ⟩` for the output sites, feed those values to the same `EventDetector` we already have. The detector doesn't know whether the values came from classical spins or quantum expectations — it just detects zero crossings.

This is the migration path the spec promised: same outer scaffold, same MIDI plumbing, only `chain.rs` changes.

**Practical constraints.** A 16-site state vector is 65,536 complex numbers — fast to manipulate, easy. The polyrhythmic two-chain (Stage 3) at 12+12 sites would be 16 million complex numbers — still tractable, but the per-step cost goes up significantly. Beyond that, MPS or tensor-network backends become necessary, and the framework already supports them — but switching backends is a non-trivial engineering exercise.

For a first quantum implementation, start with one chain at 8 sites — same scale as the classical version. Validate that the dynamics match the classical case in the regime where they should match (low entanglement, well inside the time-crystal phase). Then push parameters to where quantum effects start to differ — probably the thermal phase or near phase boundaries, where entanglement spreads quickly and the classical mean-field-like description breaks down.

**The implementation shape.** A new chain module, `quantum_chain.rs`, with the same interface as the classical `SpinChain` (constructor, `step`, `sz` for expectation values). Both modules implement a common trait, `Substrate`, that the rest of the system uses without caring which is underneath. CLI flag picks which substrate to instantiate.

Estimated effort: one to two weeks, including time spent learning quantrs2's API and validating that the quantum dynamics actually reproduce the classical behavior in the appropriate regime. Most of that time is debugging — gate ordering, expectation-value bookkeeping, sign conventions on RX rotations versus σ_x flips. Quantum simulation is unforgiving of small errors.

**The musical question Stage 5 raises.** Whether the quantum and classical substrates *sound* different through the same sonification mapping. The honest expectation is that for the first round, they will not — the time-crystal phase is well-described classically, and the zero-crossing readout is too coarse to distinguish quantum interference effects. The interesting cases are the regimes where quantum behavior diverges: strong entanglement spreading, coherent Bell-pair-like initial states between distant sites, or the late-time behavior near a phase transition where mean-field breaks down. These require composing *for* the quantum substrate rather than just running it as a classical-replacement.

---

## Stage 6 — Eigenmode sonification

**The question this answers.** The current sonification reads single-site σ_z values. The harmonics-and-eigenmodes companion note proposes a richer alternative: project the chain's state onto its eigenmodes and sonify the modes themselves, producing a real-time harmonic decomposition of the substrate. Different sonification, possibly very different musical character.

**What it requires.**

A way to compute the chain's instantaneous spatial Fourier modes (the discrete-chain analog of harmonics) or, more ambitiously, the eigenvectors of the chain's effective Hamiltonian. For an 8-site chain, this is a small linear algebra operation: form a vector of σ_z values, project onto a basis of plane waves with wavenumbers k = π/N, 2π/N, ..., readout the amplitudes.

Each mode becomes a voice. Mode index maps to pitch (low modes → low pitches, high modes → high pitches, with some chosen scaling). Mode amplitude maps to volume. The chord one hears at any moment is the chain's eigendecomposition.

In the time-crystal phase, certain modes will lock at f/2; others remain at f; some thermalize. The harmonic content separates naturally into "drive-aligned" partials and "subharmonic" partials. The phase boundary is audible as a reorganization of this partial structure.

**The implementation shape.** A new event detector, `EigenmodeEmitter`, replacing or coexisting with the zero-crossing `EventDetector`. Output is no longer discrete gate events but continuous control values per mode. This means a different MIDI mapping: control change (CC) messages for amplitudes, sent at a fixed rate (say, 50 Hz), plus the existing gate events for the dominant modes.

The receiving side in eurorack changes: instead of gate-triggered envelopes, you have CC-controlled amplitude envelopes on continuously-running oscillators tuned to specific frequencies. The patch becomes additive synthesis driven by the substrate's spectral decomposition.

This is a significantly different musical paradigm than Stage 1–4. It moves the project from rhythm-first to harmony-first. The framework's harmonics note explicitly parks this for later, after rhythm is established. We're now well into "later."

Estimated effort: two to three days for the analysis and CC plumbing, plus an unknown amount of time figuring out a sonification mapping that doesn't sound like academic linear-algebra noise. The analytical machinery is straightforward; the *musical* work is figuring out which modes to foreground, which to suppress, what pitch mapping makes the spectrum legible.

**The musical question Stage 6 raises.** What the substrate "wants" to do harmonically. The framework's harmonics note suggests this is a fundamentally different aesthetic dimension than rhythm — and that the substrate's eigenmode structure is musically meaningful in its own right. Whether that turns out to be true is, again, an empirical listening question.

---

## A note on order

The stages above are listed roughly in dependency order — Stage 3 doesn't need Stage 4, but Stage 5 benefits from having Stage 3 done first because the polyrhythmic two-chain is exactly the case where quantum dynamics start to *matter* (more sites, more entanglement, more divergence from classical behavior). Stage 6 (eigenmode sonification) is largely independent of the substrate question and could be done with the classical chain.

But order is not obligation. The right next stage is the one that answers a question you actually have, not the one that comes next in the list. If listening to Stage 1–2 raises a "what would two chains sound like" question, do Stage 3. If it raises a "I want to push and pull on this in real time" question, skip to Stage 4. If it raises a "what does the spectrum look like" question, jump to Stage 6.

The substrate is now real and audible. The next step is whatever the substrate asks for.

---

## What's intentionally not on this list

**A GUI.** Tempting, looks productive, eats huge amounts of time, doesn't add anything musical. The CLI plus a config file is enough. If real-time parameter exploration becomes a bottleneck, MIDI CC input (Stage 4 in lighter form) solves the actual need without committing to a UI framework.

**Multi-machine distribution.** The substrate scales to thousands of sites on one machine. There is no reason to run it across multiple machines until there is, and there isn't.

**A composition-language layer.** The temptation to abstract the substrate behind a higher-level "score" representation will appear. Resist. The substrate *is* the composition language; abstracting it loses precisely what makes the project distinctive.

**Performance optimization.** At 8–16 sites, the loop runs in microseconds. There is no performance problem. Profile only when something demonstrably stutters.

**ML on substrate output.** The earlier conversation parked this for good reasons. Revisit only when there's a specific musical role for a learned model that the substrate itself can't fill.

---

## Working principle

Every stage above is a candidate for "the next thing." None of them is *the* next thing. The principle that has worked so far — listen, then build the smallest thing that answers a real question — is the principle to keep working with.

The substrate exists. It produces sound. The work now is composition, with code as one of the materials.

---

*Companion to crystallized_time.md and stage_1_2_spec.md | Roadmap from working substrate forward*
