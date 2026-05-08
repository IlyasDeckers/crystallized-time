# Harmonics and Eigenmodes

*A companion note to Crystallized Time — on the structural relationship between natural harmonics, eigenstates, and the substrate*

`Note for later — rhythm comes first, harmony comes after`

---

## Purpose of this document

This is a parking document. It captures a structural observation about the relationship between natural harmonics (as already used compositionally) and the eigenmode structure of the simulation substrate, so that the observation isn't lost while the project's primary focus stays on rhythm. Harmony is downstream of rhythm in the staged plan; this note is here for when that stage arrives.

The observation is genuine and worth pursuing, but it should not pull focus from the rhythm-first work. Read this when ready, not before.

---

## The core observation

Natural harmonics on a string are eigenmodes of the wave equation on that string with specific boundary conditions. Eigenstates of a quantum system are eigenvectors of the Hamiltonian operator. These are both instances of the same general mathematical structure — eigenvalue problems for linear operators — and they share deep properties because of that shared structure: discrete spectra, orthogonality of modes, decomposition of arbitrary states into superpositions of modes, the role of boundary conditions in selecting which modes are allowed.

They are not literally the same physical phenomenon. A guitar string is a classical continuous medium described by a real-valued displacement field. A quantum spin chain is a quantum system described by a complex-valued state vector. They obey different equations of motion (the classical wave equation vs. the Schrödinger equation), and they differ crucially in that the quantum system's modes can be in superposition with complex amplitudes that interfere, while the classical string's modes superpose with real amplitudes that just add. They are different beasts living in the same mathematical genus.

The honest statement: **natural harmonics and quantum eigenstates are both eigenmodes of linear systems, share the same mathematical structure, and the intuition transfers cleanly between them — but they are not identical, and any claim that one "is" the other needs to be unpacked into a specific operational meaning to be true.**

---

## What a natural harmonic actually is

Lightly touching a guitar string at the 12th fret and plucking does not mute the string — it forces the string to have a *node* (a point of zero displacement) at that location. The string already has fixed nodes at the nut and bridge; adding a third node restricts which vibration patterns the string can support. The fundamental mode, which has its maximum displacement at the middle, is forbidden. What remains is the second harmonic, which has a natural node at the middle and is therefore compatible with the imposed constraint.

Touching at the 7th fret (1/3 of the way along) forbids any mode without a node at 1/3. The fundamental and second harmonic are eliminated; the third harmonic, with nodes at 1/3 and 2/3, survives. The fifth fret (1/4 along) selects the fourth harmonic. Each light touch is a *boundary condition* selecting which vibration modes the system is permitted to occupy.

The vibrating string supports a discrete set of standing-wave modes — sinusoids with an integer number of half-wavelengths fitting between the endpoints — each with a specific spatial pattern and a specific frequency. These modes are the *eigenfunctions* of the wave equation under the boundary conditions. The general motion of the string is a superposition of these modes; a natural harmonic is the technique of selecting a single mode (or a small subset) by imposing additional boundary conditions through finger placement.

When the touch is imperfect — not exactly at a node — the suppression of forbidden modes is incomplete, and a small amount of those modes leaks through. The resulting sound is the target harmonic plus a quiet fringe of forbidden modes beating against each other slightly out of phase. This leakage is not a flaw; it is what gives natural harmonics their characteristic shimmer on a real instrument. A pure sinusoid from an oscillator does not sound like a guitar harmonic precisely because it has no leakage.

---

## Why this is structurally analogous to quantum mechanics

A quantum system has a Hamiltonian operator, which has eigenstates — states with definite energies. The general state of the system is a superposition of energy eigenstates, with complex amplitudes. A measurement of energy returns one of the eigenvalues with probability given by the squared magnitude of the corresponding amplitude.

This is the same eigendecomposition structure. Both the classical string and the quantum system are linear systems whose general behavior decomposes into a sum of fundamental modes, weighted by amplitudes. In both cases, the modes are determined by the structure of the system (string properties and boundary conditions; Hamiltonian and boundary conditions) and the amplitudes are determined by how the system was prepared.

The mathematical machinery — eigenvalues, eigenvectors, completeness relations, orthogonality, mode decomposition — is identical between the two cases. Schrödinger originally wrote his equation by direct analogy with the wave equation for a vibrating medium, and the parallel runs all the way down to the operational level: when you play a natural harmonic, you are doing physically, with your hand, the same operation that a quantum projector does to a quantum state.

The differences that matter:

The classical string's amplitudes are real numbers; the quantum system's amplitudes are complex numbers. Real amplitudes can add or cancel only through sign; complex amplitudes can interfere through phase, which produces phenomena (interference patterns, entanglement) that have no classical-string analog.

The classical string's modes are spatial patterns of displacement; the quantum system's modes are abstract vectors in a Hilbert space whose dimension depends on the system, not on physical space.

The classical string's measurement is direct observation of the displacement field; the quantum system's measurement collapses the state probabilistically, with statistics determined by squared amplitudes.

These differences are real and matter for the physics. They do not invalidate the structural correspondence; they qualify the sense in which it holds.

---

## Where this connects to the substrate

Several specific connections, ordered from most concrete to most speculative.

### Spin chains have eigenmodes

The driven spin chain in the prototype has, underneath its time-domain dynamics, a spectrum of collective modes — spin waves, magnons, domain wall excitations. Each mode has a wavelength along the chain and a frequency at which it oscillates. A 16-site chain has on the order of 16 spatial modes, analogous to the harmonics of a 16-segment string. These modes are well-defined, orthogonal, and complete: any state of the chain can be written as a superposition of them, and the time-evolution of the chain can be understood as the time-evolution of each mode's amplitude.

The dispersion relation — the relationship between mode wavelength and mode frequency — is determined by the Hamiltonian. For a typical Heisenberg or Ising spin chain, the dispersion is something like ω(k) ∝ |sin(k/2)|, which is *not* the integer harmonic series of a guitar string. The modes are real and well-defined, but their frequencies do not match a stretched-string harmonic series. They produce a different "natural scale" — denser at the high end than the integer series, with its own characteristic intervallic structure.

This is compositionally interesting rather than a problem. Different Hamiltonians produce different dispersion relations and therefore different mode spectra. Each Hamiltonian is, in this sense, a different instrument with its own natural scale baked into its physics.

### Sonification through eigenmode projection

The current sonification reads zero-crossings of individual sites' σ^z values. An alternative — and the one that connects directly to the harmonics intuition — is to decompose the chain's instantaneous state into its spatial Fourier modes (the discrete-chain analog of harmonics) and sonify the modes rather than the sites.

The mechanics: at each timestep, take the vector of spin values along the chain, project it onto the eigenvectors of the chain's Hamiltonian (or, as a simpler first pass, onto the discrete Fourier basis), and read out the amplitudes. Each mode becomes a voice; its amplitude is its loudness; its mode index determines its pitch through some chosen mapping. The chord one hears at any moment is a real-time decomposition of the chain into its eigenmodes.

In the time-crystal phase, this approach has a particularly clean signature. Certain modes lock at f/2 against the drive; others remain at f; some thermalize. The harmonic content separates naturally into "drive-aligned" partials and "subharmonic" partials, with the phase boundary between regimes producing audible reorganization of the harmonic series. A composer could write *for* this — the substrate produces a harmonic series whose internal structure depends on the dynamical phase of the system, and the visitor's perturbations move the system through different harmonic regimes.

### The polyrhythm-as-harmonics duality

The framework's §3.3 argument concerns coupled chains locking at different sub-harmonic periods (f/2 vs f/3, etc.) and the lcm of those periods governing recurrence. This is the time-domain version of harmonic interval relationships in pitch. A frequency ratio of 3:2 in the time domain — two layers locked at f/2 and f/3 — is, transposed by many octaves into the audible range, a perfect fifth.

The same arithmetic that produces consonance in pitch produces locking and recurrence in rhythm. This is not a coincidence; it is the same wave physics, sampled at different timescales. Pythagoras noticed the integer ratios in the audible range; the framework's polyrhythmic argument notices them in the rhythmic range; they are the same ratios, and the substrate's behavior at one timescale informs its behavior at the other.

This suggests that the relationship between the substrate's rhythmic content (Stage 5–6 of the implementation plan) and its harmonic content (whatever stage harmony enters) is not two independent compositional layers but two facets of the same eigenmode structure, viewed through different time-windows.

---

## On a quantum computer specifically

Hamiltonian simulation — preparing and evolving eigenstates of a target Hamiltonian — is one of the canonical applications of quantum computers, and it is one of the things they are genuinely good at. This is directly relevant to the staged plan's later quantum-substrate work.

The operationally meaningful claims:

A spin-chain Hamiltonian can be encoded on a quantum computer and its eigenstates prepared (approximately) using known algorithms. The evolution of an eigenstate under its own Hamiltonian is particularly clean: the spatial pattern stays fixed; only the global phase rotates. This is exactly analogous to a guitar string vibrating in a single harmonic mode — the shape does not change, only the time-oscillation.

The same imperfect-harmonic picture from the classical case translates directly. An imperfectly prepared quantum eigenstate is the target eigenstate plus small amplitudes of other eigenstates, which beat against each other at frequencies given by the energy differences. The "shimmer" of an imperfect natural harmonic is, in quantum language, the interference between nearly-degenerate eigenstates with small admixtures. The physics is the same; only the substrate differs.

This is a clean place where the quantum simulation could do something the classical chain cannot do as cleanly: prepare specific eigenstates with controlled admixtures, and listen to the beating between them as the controlled phenomenon rather than as an artifact. The musicality of imperfect-harmonic preparation is something the artist already knows from guitar; doing it deliberately on a designed Hamiltonian is something only the quantum substrate enables in full generality.

---

## The eurorack natural-harmonics study, with eigenmodes in mind

The planned study to model natural-harmonic waveforms in eurorack is the right next move and connects directly to this document. One specific suggestion: model the harmonic not as a single sinusoid but as a primary partial plus small amplitudes of the suppressed modes, all beating slightly out of phase. The math is simple — additive synthesis of a few sinusoids with carefully chosen relative amplitudes and phases — and the result will sound substantially more like a real natural harmonic than a pure tone does.

The same code structure will reuse for the spin-chain eigenmode sonification. In both cases, the operation is: identify the modes, weight them by amplitudes, sum the resulting sinusoids. The substrate differs (a string vs. a spin chain) but the synthesis is identical.

This is a small piece of leverage worth noting: the eurorack study, framed this way, is not preparation for harmony work that's separate from the quantum substrate work — it's a prototype of exactly the synthesis stage that the eigenmode sonification will eventually need.

---

## What this changes about the project

Concretely: nothing immediate. Rhythm comes first. The staged plan does not need to be revised. The current focus on the time-crystal phase, on period-multiplication, on coupling chains for polyrhythm — all of that proceeds.

What this document marks is that when harmony enters the project (post Stage 5, plausibly during or after Stage 6), the sonification mapping for harmonic content already has a candidate form: eigenmode decomposition of the chain's state, with amplitudes mapped to partial loudnesses and mode indices mapped to pitches. The infrastructure for this — Fourier or eigenbasis projection of the chain state — is straightforward to add to the existing prototype when the time comes.

It also suggests, more speculatively, that *Hamiltonian design* — choosing or constructing Hamiltonians whose mode spectra produce musically meaningful scales — is a compositional move available to this project that is not available to most. This sits in the gap between physics and composition where the project lives, and it is a real research question that nobody is currently working on. It is far too early to pursue this directly; it is parked here for later consideration.

---

## A larger observation, kept brief

The reason these patterns recur across the artist's existing work — natural harmonics, polymetric composition, time crystals — is that *eigenmode decomposition is a structural feature of any linear system with conservation laws and boundary conditions*, and an enormous amount of physics, music, and signal processing falls into that category. The mathematical sensibility developed through music for how modes combine and decompose is the same sensibility used in quantum mechanics, on a different substrate.

This is why the project's instinct to find a physical substrate that "naturally does what the music already does" is structurally sound rather than metaphorical. The substrate and the music share an eigendecomposition skeleton, and the framework document is correct that the relationship is one of correspondence, not metaphor. This document extends that correspondence one layer deeper: not only do the dynamics correspond (period-doubling, rigidity, phase transitions), but so does the spectral structure (modes, harmonics, partials). Both are facets of the same underlying linear-operator mathematics.

---

## Status

`Parked for later. Rhythm first.`

This document exists so the observation is preserved and can be picked up when the project's focus moves from rhythm to harmony. It does not need action now. It does not need refinement now. It is here to be re-read with fresh eyes when the time is right.

---

*Companion note to Crystallized Time | Draft 0.1 | Research in progress*
