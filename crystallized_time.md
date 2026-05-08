# Crystallized Time

*An Audiovisual Installation Grounded in Driven Many-Body Dynamics*

`Draft 0.3 — Research in Progress`

---

## Abstract

This document describes the conceptual and technical foundations of an audiovisual installation in which a simulated driven many-body system — exhibiting the dynamics of a discrete time crystal — generates music and visual structure in real time, and in which the presence of visitors directly perturbs the system's trajectory. The installation makes a specific structural claim about music perceptible as direct experience: that rhythm, polyrhythm, and macro-form can emerge from the same physics that governs how stable patterns appear in driven matter, and that an observer is not separate from the work but is part of the dynamical system that produces it.

The framework draws on two distinct registers, and this document is careful to keep them distinct. The **technical substrate** is described in classical physical terms: a periodically driven spin chain whose period-multiplication, rigidity, and phase-transition behavior are well-characterized phenomena, simulated faithfully and sonified deliberately. The **artistic premise** is described in the language of observation and emergence, because that is the language that most economically describes what the visitor experiences. The two registers are not in conflict — the artistic experience is a true description of the technical situation, viewed from inside it.

These are early-stage research notes for a work in progress.

---

## 1. The Installation

A visitor enters a room. At its center stands a pillar. Sound fills the space; visuals are projected in 360 degrees. Both are being generated in real time by a physical simulation running in the substrate of the installation. Nothing is pre-recorded, nothing is looped, and nothing repeats — though patterns lock and hold for as long as the conditions that produced them persist.

The visitor's presence is registered: their position in the room, their movement, and — depending on the version of the installation — biometric signals such as heart rate or galvanic response. These measurements feed into the parameters of the simulation. The system the visitor is hearing and seeing is not a system that exists independently of them. It is a system whose trajectory is being shaped, in real time, by the fact that they are in the room.

The visitor is not told this. The piece offers no instructions, no interface, no labels. The relationship between presence and output is something to be discovered — through stillness, through movement, through attention — and the discovery is the experience the work is designed to produce.

When a second visitor enters, the dynamics change again. Not by averaging two inputs, and not by switching modes, but because the substrate is a coupled many-body system whose response to multiple simultaneous perturbations is itself determined by its physics. Two visitors produce a piece neither of them would have produced alone, in a way that is genuinely a property of the system rather than a designed feature.

---

## 2. Why Quantum Mechanics

The installation is grounded in a particular reading of quantum mechanics, and the relevance of that reading to the artistic premise should be stated plainly before any technical content.

In popular culture, quantum mechanics is associated with the idea that observation collapses possibilities into outcomes — that consciousness, somehow, brings reality into focus. This is a misreading. What actually fixes a quantum state is **interaction**: any sufficient coupling between the system and its environment. A stray photon will do it. Consciousness has nothing to do with it.

This correction is, for the installation, a feature rather than a problem. The visitor is not metaphorically observing the work; they are physically interacting with it. Sensors register their presence, their data perturbs the simulation, and the simulation's trajectory changes accordingly. The visitor is, in the literal sense the physics requires, *part of the measurement*. The poetic framing — that the music exists in superposition until the visitor enters — is a true description of the system from the visitor's perspective, even though it is not a description of quantum mechanics in the technical sense.

The deeper structural reason for reaching toward physics is that music, at its base, is already a wave phenomenon. A note is a wavelength. A chord is a superposition of wavelengths. Constructive and destructive interference, resonance, beating, harmonic series — these are not analogies between music and physics; they are the same phenomena, named differently in different fields. The question this installation began with was whether the physics could be pushed further: not just the wave physics of sound itself, but the dynamics of a *system* that produces those waves.

The specific entry point — time crystals — emerged from a compositional concern rather than a theoretical one. The author writes music without strict downbeats and without uniform time signatures, in polymetric layers whose interaction produces complex but musical rhythm. Discovering that driven many-body systems naturally produce sub-harmonic responses (f/2, f/3, …), with periods determined by the system rather than imposed externally, was a recognition rather than a hypothesis: this is how the music already works. The substrate was found, not chosen.

---

## 3. The Technical Substrate

This section describes the simulation in classical physical terms. The artistic claims of §2 do not depend on the simulation being a faithful reproduction of a real quantum time crystal; they depend on the simulation having the *dynamical properties* that the time-crystal phase exhibits. Those properties — period-multiplication, rigidity, and phase-discreteness — can be reproduced classically, and the existing prototype does so.

### 3.1 Driven Systems and Period-Multiplication

A driven system is one acted on by a periodic external force at frequency *F*. Most driven systems respond at *F*. A discrete time crystal is a driven many-body system that, despite being kicked at *F*, settles into a steady state oscillating at *F/n* for some integer *n* > 1. The drive period is *T*; the system's period is *nT*.

Three properties of this behavior are central to the installation:

The integer *n* is **determined by interactions**, not by the drive. Different couplings, disorder profiles, and initial conditions land the system in *n = 2*, *n = 3*, or higher. The composer does not choose *n*; the substrate does.

Once the system locks into period *nT*, that period is **rigid**. Small changes to the drive — imperfect pulses, parameter drift, thermal noise — do not break the lock. The system absorbs the perturbation and continues at *nT*. This is what physicists mean by saying a time crystal is a *phase* of matter: it has the stability of ice rather than of a balanced pencil.

Outside the time-crystal phase, the system either responds at the drive frequency (trivial) or thermalizes into noise. The transition between regimes is **sharp**. There is no smooth gradient between "locked at 2T" and "locked at 3T" — the system is in one phase or the other.

### 3.2 Why These Properties Matter for the Installation

Each of the three properties above corresponds to something the installation needs:

A groove that **self-corrects**. The visitor's presence is a perturbation. If the substrate were not rigid, every movement would knock the music off its rhythm and the experience would be chaotic. Because the substrate is rigid, the rhythm holds — the visitor influences the music without destroying it. This is the property that makes interaction musically tolerable.

A period that is **discovered, not assigned**. The simulation is told only the drive period and the interaction structure. The musical period emerges. This means the rhythmic content of the piece is genuinely produced by the system rather than scripted, and the visitor's perturbations can shift the system between different emergent periods rather than between authored states.

**Phase transitions** as macro-form. The installation has a vocabulary of structural shifts — locking, melting, re-crystallizing — corresponding to recognizable musical gestures (a groove establishing, dissolving, re-forming). These are not scripted events. They are the simulation crossing a phase boundary, which it does in response to its parameters, which respond in turn to the visitor.

### 3.3 Polyrhythm as Coupled Sub-Harmonics

Certain musical traditions — Meshuggah, Tigran Hamasyan, Carnatic konnakol, Hindustani tala — construct rhythm as independent periodic layers sharing a common pulse rather than as nested subdivisions of a uniform meter. A composition might run a layer of period 13 against a layer of period 9 over a shared underlying tick. The full system returns to its starting configuration after lcm(13, 9) = 117 pulses; because gcd(13, 9) = 1, that recurrence is maximally delayed. This is why these traditions reach for prime or coprime groupings, and why the moment of recurrence registers to the ear as inevitable rather than designed.

Two metronomes set to periods 13 and 9 will produce the same arithmetic recurrence, but they will drift if perturbed and they cannot interact: each is indifferent to the other. The distinctive musical contribution of a time-crystal substrate is that **multiple coupled chains can lock at different *n* and influence each other through their shared substrate without losing their individual periods**. This is the polyrhythmic architecture made physical: layers that interact while keeping their own time. It is the technical reason the substrate fits the music, beyond the bare arithmetic of lcm-recurrence.

### 3.4 The Minimum Viable Substrate

The simplest system that exhibits the relevant dynamics is a disordered, periodically driven, classical spin chain — the system already running in the prototype. Each site carries a spin; sites interact with their neighbors; an external drive applies a near-π pulse at fixed intervals; disorder in the local fields stabilizes the period-doubled phase against thermalization.

For a single rhythmic layer, one chain suffices. For polyrhythmic structure, multiple chains weakly coupled through a shared field or shared pulse give independent layers whose periods are determined separately but whose interactions are mediated. This is the architecture §3.3 describes, instantiated.

The author retains the right to take liberties with the physics where the music or the installation demands it, provided the substrate retains the three properties of §3.1. Faithfulness to the published time-crystal literature is not the goal. Faithfulness to the dynamics that make the substrate musically right is.

---

## 4. The Measurement-to-Music Mapping

A simulated spin chain is not music until something is read out of it and turned into sound. **This mapping is the actual compositional act of the framework**, and it deserves more attention than physics-based generative systems typically give it.

A non-exhaustive list of observables and the musical decisions they encode:

The **z-component of each spin** is a continuous signal between −1 and +1 that flips sign with the period of the crystal. Reading zero-crossings as note events (the choice in the prototype) gives a sparse, percussive sonification where each site is a voice firing at its own sub-harmonic. This is rhythm-first.

**Total magnetization** ⟨M(t)⟩ is a single global signal oscillating at the dominant period of the system. Mapping it to pitch or amplitude gives a single melodic or dynamic line representing the macro-state of the chain. This is form-first.

**Domain walls** — boundaries between regions of opposite spin — are localized, mobile structures whose motion can be tracked. Each domain wall is a point-like object with a position and a velocity; sonifying their trajectories gives a class of voices that exist only when the chain is partially ordered. This is texture-first.

**Spin-spin correlations** between distant sites encode the long-range structure of the phase. In the time-crystal regime these correlations are large; in the thermal regime they collapse to zero. Mapping correlation strength to a global parameter (filter cutoff, reverb mix, harmonic density) gives the phase transition itself an audible signature. This is environment-first.

The framework should not commit to a single mapping. The mapping is a *choice*, and different choices yield different music from the same physics. Treating the mapping space as a first-class compositional variable — alongside the substrate parameters and the visitor-interaction model — is the part of the project that most resembles traditional composition.

---

## 5. The Visitor as Part of the System

The visitor's role in the installation is described in §1 in experiential terms and in §2 in conceptual terms. This section describes it in technical terms, to make explicit that the experiential and conceptual claims rest on actual implementation choices.

The visitor's measured state — position, movement, biometrics — is mapped onto the parameters of the substrate in real time. The four parameters exposed by the existing prototype are the relevant ones: the drive imperfection ε, the interaction strength *J*, the disorder width *W*, and the effective temperature *kT*. Each of these has a known effect on the system's dynamics. Increasing *kT* pushes the system toward the thermal phase. Decreasing ε strengthens the period-2T lock. Changing *J* and *W* shifts the boundary between phases.

The visitor, in other words, is operating four knobs without knowing they are doing so. They cannot turn the system into anything they want — the physics permits a finite set of behaviors and forbids the rest — but within that set they have real influence. The question of *how much* influence, and *how legibly*, is the central design question of the installation. Too tight a coupling and the piece becomes a theremin: every movement registers, the substrate's autonomy disappears, the visitor learns the rules in seconds and the piece collapses into demonstration. Too loose and the visitor never discovers that they matter at all. The right setting is somewhere visitors find the relationship through curiosity rather than instruction, and where the substrate retains enough of its own life that discovery never reduces it to a controller.

This is also where multiple visitors become structurally interesting. Each contributes a perturbation; the perturbations sum into the parameter space; the substrate's response is determined by its physics. Two visitors can push the system into phase regions that neither could reach alone — not because the installation contains an authored "two-person mode" but because the parameter sums land in a different region of the phase diagram. A piece that one visitor experiences as locked and rhythmic might melt into something else when a second visitor enters. This is a property of coupled many-body systems, not a feature added to the installation.

---

## 6. What This Framework Is and Is Not

It **is** an audiovisual installation in which a simulated driven many-body system generates music and image in real time, in which visitors perturb the system through their physical presence and measured state, and in which the dynamics of period-multiplication, rigidity, and phase transitions correspond to musical behaviors the author has been pursuing through composition independently of physics.

It **is** a use of quantum-mechanical vocabulary — observer, superposition, collapse — in the registers where it earns its place: as the most economical description of what the visitor experiences, and as honest acknowledgement that the artistic premise grew from engagement with the conceptual structure of the field.

It **is not** a claim that music is quantum mechanical, that the simulation is a faithful reproduction of a real time crystal, or that the visitor's consciousness has any special role in the system's dynamics. The visitor's *presence* matters, in a literal physical sense; their consciousness, in the technical sense the physics requires, does not.

Where classical descriptions suffice for the technical layer, this draft uses them. Where quantum-mechanical language is the right description of the experiential layer, this draft uses it. The two layers are kept distinct, and the relationship between them is one of correspondence, not identity.

---

## 7. Next Steps

Three concrete priorities, in order:

**Characterize the existing substrate.** The classical spin chain in the prototype already exhibits period-doubling, rigidity, and a thermal phase. Map its parameter space: which (ε, *J*, *W*, *kT*) combinations produce stable 2T, which produce higher-order *nT*, which thermalize. This is a few hours of parameter sweeps and gives the framework an empirical floor for the visitor-interaction design of §5.

**Build a second chain and couple them.** The minimum interesting polyrhythmic system is two chains with different disorder profiles, weakly coupled through a shared pulse or shared field. Verify that the chains can lock at different *n* and that the lcm of their periods governs joint recurrence. If this works, §3.3 is no longer hypothetical.

**Pick one measurement-to-music mapping and commit to it for an extended piece.** Zero-crossings on the existing single chain is the obvious candidate. Compose with it — meaning, sweep parameters intentionally over five to ten minutes — and listen. The point of this step is to discover what the substrate actually wants to do musically, before making decisions about what we want it to do. Everything about the installation downstream — the pillar, the visual language, the sensor mapping, the room itself — is easier to design once the substrate's musical character is known by ear rather than by argument.

After these three, the architecture of the installation can be designed against an empirically grounded substrate rather than against a hypothetical one.

---

*— Draft 0.3 | Research in Progress*
