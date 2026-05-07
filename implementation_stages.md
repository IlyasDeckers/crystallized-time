# Implementation Stages

*A working document for the Crystallized Time installation*

`Living document — expand as work progresses`

---

## Purpose

This document tracks the staged implementation of the quantum simulation substrate for the *Crystallized Time* installation. Each stage has a clear technical objective, a definition of done, and space for notes accumulated during the work itself. Stages are ordered such that each one builds on the last; a stage is not begun until the previous one has been validated.

The early stages prototype in Python (NumPy) for speed of iteration. The implementation will be ported to Rust once the physics is settled and the architecture is stable, before any installation deployment. The boundary between "prototype" and "production" is drawn explicitly in Stage 7.

---

## Stage 0 — Foundations

**Objective.** Establish the development environment, the conventions, and the testing scaffolding that every subsequent stage will depend on.

**Scope.** Project layout. Choice of numerical library (NumPy for prototype). Conventions for qubit ordering, basis state representation, and complex amplitudes. A minimal test harness — a single passing test is enough; the point is to have somewhere to put the next one.

**Definition of done.** A repository structure exists. A trivial test (e.g. that |0⟩ has amplitude 1 in the zero basis state and 0 elsewhere) passes. Conventions are written down somewhere, even if briefly, so they don't have to be re-derived later.

**Notes.**
*(to be filled in)*

---

## Stage 1 — Bare-Metal Qubit Simulator

**Objective.** Build a state vector simulator from scratch, with no quantum-specific libraries, that supports single-qubit and two-qubit gates and projective measurement.

**Scope.**
- State vector representation for *n* qubits (a complex array of length 2^n)
- Single-qubit gates: X, Y, Z, H, S, T, and parameterized rotations Rx(θ), Ry(θ), Rz(θ)
- Two-qubit gates: CNOT, CZ, and ideally a parameterized two-qubit gate (e.g. controlled rotation)
- Projective measurement of a single qubit, with proper state collapse and renormalization
- Helper utilities: state preparation, expectation values for Pauli observables, sampling from the measurement distribution

**Definition of done.**
- Bell state preparation (apply H to qubit 0, then CNOT from 0 to 1) produces the expected entangled state
- Repeated measurements of a Bell state show the correct correlation statistics
- Single-qubit rotations match analytic expectations on test cases (e.g. Rx(π) on |0⟩ gives |1⟩ up to a global phase)
- Norm preservation holds across long sequences of gates to within numerical precision

**Notes.**
*(to be filled in)*

---

## Stage 2 — Hamiltonian Evolution

**Objective.** Move from discrete gates to continuous-time evolution under a Hamiltonian, which is the natural language for many-body physics.

**Scope.**
- Construction of a Hamiltonian as a sum of Pauli terms (e.g. ∑ J_ij σ_i^z σ_j^z + ∑ h_i σ_i^x)
- Time evolution by Trotterization: decomposing exp(−iHΔt) into a product of single- and two-qubit operations
- Validation against exact diagonalization for small systems (≤ 6 qubits) where the full Hamiltonian matrix can be exponentiated directly

**Definition of done.**
- Trotterized evolution of a small spin chain agrees with exact diagonalization to acceptable error
- Trotter error scales as expected with step size Δt
- Conservation laws of the test Hamiltonian (e.g. total magnetization for an Ising chain) are respected

**Notes.**
*(to be filled in)*

---

## Stage 3 — Driven Floquet Dynamics

**Objective.** Combine continuous Hamiltonian evolution with periodic kicks, observe period-doubling, and confirm that the simulator can reproduce the basic time-crystal phenomenology.

**Scope.**
- A driven protocol: evolve under H for time T, then apply a global near-π pulse, repeat
- Measurement of stroboscopic observables (the system's state sampled once per drive period)
- Detection of period-doubling: the magnetization at stroboscopic times alternates between two values rather than returning to the same value each period

**Definition of done.**
- For a clean driven Ising chain at small *n* (4–8 qubits), period-doubling is visible in stroboscopic magnetization
- Period-doubling persists for at least tens of drive cycles before finite-size effects destroy it
- The behavior matches the qualitative shape of the classical prototype's results, but is now genuinely quantum (visible entanglement entropy growth, etc.)

**Notes.**
*(to be filled in)*

---

## Stage 4 — Disorder and the Time-Crystal Phase

**Objective.** Add disorder to the system and find the parameter regime in which period-doubling persists indefinitely (within simulation horizons), as a genuine phase rather than a transient.

**Scope.**
- Random local fields, drawn from a tunable distribution
- Parameter sweeps over disorder strength, interaction strength, and drive imperfection
- Identification of the time-crystal phase boundary: where does the period-doubled response survive long-time evolution, and where does it thermalize?
- Measurement of phase-distinguishing observables (long-time magnetization autocorrelation, subharmonic peak in the Fourier spectrum)

**Definition of done.**
- A phase diagram is produced for the available parameter range, even if coarse
- At least one parameter set is identified where period-doubling clearly persists, and one where it clearly does not
- The author has a working intuition for which knobs move the system across the phase boundary, validated against simulation rather than memory

**Notes.**
*(to be filled in)*

---

## Stage 5 — Observable Extraction and Sonification

**Objective.** Define and implement the measurement-to-music mapping. This is the first stage where the project becomes audible.

**Scope.**
- Stream of observables extracted from the simulation in real time: per-site ⟨σ^z⟩, total magnetization ⟨M⟩, two-point correlations ⟨σ_i^z σ_j^z⟩, entanglement entropy of subsystems
- One mapping selected and implemented (default candidate: zero-crossings of per-site ⟨σ^z⟩ as note events, following the prototype)
- A working audio output path (offline rendering is acceptable here; real-time can wait)
- A small piece — a few minutes of generated audio — produced by sweeping parameters intentionally, recorded, and listened to

**Definition of done.**
- Audio renders without numerical artifacts (NaNs, clicks from discontinuous parameter changes, etc.)
- The author can listen to a generated piece and articulate what the substrate is doing musically and where its strengths and weaknesses lie
- At least two alternative mappings have been sketched, even if not implemented, so the choice is informed

**Notes.**
*(to be filled in)*

---

## Stage 6 — Coupled Systems

**Objective.** Move from a single chain to multiple coupled chains, instantiating the polyrhythmic architecture described in the framework document.

**Scope.**
- Two or more chains in a single state vector, with a controllable coupling between them
- Verification that the chains can lock at different sub-harmonic periods
- Verification that the joint state recurs at the lcm of the individual periods
- Exploration of the regime where the chains are entangled across the coupling — this is where the project does something the classical substrate cannot

**Definition of done.**
- Two chains running with distinct periods (e.g. 2T and 3T) confirmed by stroboscopic observation
- Joint recurrence at lcm of periods is observable in correlations between the chains
- Entanglement between chains is non-zero in the coupled regime and zero (or near-zero) in the uncoupled regime
- Compute cost is understood: the qubit count for two coupled chains is large enough to be felt, and the practical limit on chain size is now known empirically

**Notes.**
*(to be filled in)*

---

## Stage 7 — Port to Rust

**Objective.** Reimplement the working Python prototype in Rust, with performance and deployment characteristics suitable for the installation runtime.

**Scope.**
- A Rust crate mirroring the prototype's structure
- Equivalent functionality, validated by running the same test cases in both languages and comparing outputs
- Identified performance hot paths optimized using Rust idioms (slices, iterators, possibly SIMD)
- Decision deferred to this stage: whether to keep state vector simulation on the CPU or move it to GPU via CUDA / wgpu / similar. Decision is informed by whether CPU performance meets the latency budget defined in Stage 8.

**Definition of done.**
- The Rust simulator passes the same physics tests as the Python prototype, to acceptable numerical agreement
- The Rust simulator is meaningfully faster than the Python one on the chain sizes used in Stages 5–6
- The author has written enough Rust during this stage to be comfortable extending it independently

**Notes.**
*(to be filled in)*

---

## Stage 8 — Real-Time Pipeline

**Objective.** Embed the simulator in a real-time pipeline suitable for installation use: sensors in, parameters perturbed, simulation stepped, observables extracted, audio and visuals out, all within a perceptually acceptable latency budget.

**Scope.**
- Latency budget defined and measured: target on the order of 50–100 ms from sensor input to audio output
- Sensor abstraction layer: a clean interface that can take input from real sensors or from synthetic test patterns
- Audio output via a real-time-capable backend
- Visual output, at minimum, of the substrate state (the pillar visualization design lives downstream of this)
- Graceful behavior under parameter changes: no clicks, no instabilities, no thermal runaway

**Definition of done.**
- A test harness can simulate a visitor (synthetic sensor input following a scripted pattern) and the output responds within budget
- The simulator runs continuously for at least an hour without drift, leaks, or numerical degradation
- The relationship between sensor input and audible/visible output is legible enough that a curious test listener discovers the coupling without instruction

**Notes.**
*(to be filled in)*

---

## Stage 9 — Installation Architecture

**Objective.** Beyond the scope of pure simulator work — the room, the pillar, the sensor hardware, the speaker layout, the visual rendering, the failure modes when no one is in the room or when ten people are. This stage exists in this document as a placeholder; its scope will be elaborated when Stage 8 is complete.

**Notes.**
*(to be filled in when the upstream stages stabilize)*

---

## Cross-Stage Concerns

Several questions span multiple stages and are worth tracking outside the linear sequence:

**Numerical precision.** The prototype uses double-precision complex arithmetic; the Rust port may want to consider single precision for performance, and the question of whether single precision is sufficient for the physics depends on how long the simulation runs without resetting. To be revisited in Stage 7.

**Compute scaling.** Each added qubit doubles the state vector. Decisions about chain size in Stages 3–6 set the ceiling for the installation. The current expectation is that 16–20 qubits is the working range; this should be tested rather than assumed.

**Visualization of the substrate during development.** The classical prototype's visualizations (lattice, magnetization, raster) were essential for understanding what the system was doing. Equivalent debug visualizations for the quantum simulator will save enormous time and should be built early — probably during Stage 3 — even though they are not part of the deliverable.

**Reproducibility.** Random seeds for disorder, initial states, and noise need to be tracked from Stage 4 onward. Two runs with the same seed should be bit-identical at the prototype stage; whether this holds across the Python-to-Rust port is a Stage 7 question.

---

*Living document — last updated with v0.1 of the stages plan*
