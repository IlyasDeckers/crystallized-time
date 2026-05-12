# Generative Visual Layer — Weight Drift and Model Merging

*A companion specification to `crystallized_time.md` and `stage_3_plus_reference.md`. Conceptual spec — research direction, not a commitment.*

`Status: parked. Explore after visual pipeline is established in TouchDesigner.`

---

## Abstract

This document captures a conceptual direction for the installation's visual layer: a small generative neural network whose weights drift in real time, driven by the spin chain's phase state, and whose latent space can be traversed continuously. The network is not a static model producing fixed outputs — it is a system that accumulates visitor history in its weights, becoming something over time that it was not at the start.

The core ideas: weight drift as a real-time process governed by phase rigidity, model merging to define a navigable space between trained poles, and the uncanny zone between poles as the primary aesthetic target.

---

## 1. The Core Idea

A neural network trained on a visual vocabulary produces outputs by running a forward pass through its weights. Those weights encode everything the network learned — the implicit grammar of the material it was trained on.

If you slowly modify the weights during operation, the grammar degrades. Not uniformly — some rules break before others. The network continues producing outputs confidently, but the outputs violate rules it previously held. The result is images that are internally consistent but wrong in ways that resist articulation. This is the aesthetic target: **confidently alien**.

The spin chain is the governor of this drift. Its phase properties — rigidity, phase transitions, thermalization — map directly onto how aggressively weights change:

- **Locked phase** → weights drift slowly or not at all. Output is coherent, almost real.
- **Near phase boundary** → weights drift faster. Grammar begins to slip unevenly.
- **Thermal phase** → drift stops, or the system oscillates between states. Output is in the fully uncanny register.

The visitor doesn't control this directly. They perturb the chain, the chain's phase state changes, the visual grammar shifts as a consequence. The relationship is discovered, not instructed.

---

## 2. The Aesthetic Target

The visual vocabulary is **brutalist architecture and related structures**: raw concrete, inhuman scale, aggressive geometry, compressed mass. Brutalism is already at the edge of hospitable. The installation pushes it further.

The target quality is the space between real and almost-real — structures that seem like they could exist, that carry the weight and logic of real buildings, but that violate implicit rules the viewer cannot immediately name. Load-bearing logic that doesn't hold. Proportions scaled for nothing that breathes. Material honesty that is almost, but not quite, honest.

This is distinct from abstract noise or obvious distortion. The wrongness should be subtle enough that the viewer's brain registers it before they can articulate it. The uncanny valley applied to architecture rather than faces.

**Candidate training poles** (each trained separately, merged to navigate between):

- Brutalist architecture — the primary vocabulary. Real structures, documentary photography and architectural renders.
- Geological formations — cave systems, rock faces, compressed strata. Shares brutalism's formal vocabulary (raw material, scale ambiguity, mass) but is organic rather than designed. The merge between these two produces structures that feel grown rather than built.
- Industrial infrastructure — cooling towers, refineries, bunkers. Already inhospitable to human presence. Merging with brutalism produces things that look functional but whose function is unreadable.
- Deep sea structures / organisms — alien scale, pressure-logic rather than gravity-logic, surfaces evolved for a world without light. The furthest pole from architecture. Merging here produces the most unsettling outputs — structures that obey rules, but not any rules the visitor knows.

The poles are **compositional decisions**. What sits at the extremes determines what the uncanny middle becomes. This list is a starting point, not a commitment.

---

## 3. Model Merging

Two networks trained on different datasets but with **identical architectures** can be linearly interpolated in weight space:

```
merged_weights = (1 - t) * weights_A + t * weights_B
```

Where `t = 0` is pure model A, `t = 1` is pure model B, and the middle is a blend of both learned grammars. This is known as model souping and works because both networks learned to represent their respective domains using the same mathematical operations — the weights are not random relative to each other.

More interesting is **layer-wise merging**, which exploits the layer structure of the network:

```
Early layers (structure, spatial relationships):  blend ratio α
Late layers (texture, surface detail):            blend ratio β
```

With `α ≠ β`, you can produce outputs with the mass and geometry of one pole and the surface quality of another. A structure with the volumetric logic of concrete and the surface of deep-sea organism. The grammar of brutalism applied to material that has never been above the waterline.

For the installation, the spin chain's observables map onto the merge parameters:

- **Global magnetization** → coarse blend ratio (overall structural grammar)
- **Domain wall density** → fine blend ratio (surface texture grammar)
- **Phase boundary proximity** → rate of change of blend ratios

The merge is not a one-time operation. It is continuous. The network the installation runs at midnight on the third day is not the network it ran at opening. It has drifted through the merge space shaped by every visitor who has been in the room.

---

## 4. Latent Space Traversal

Beyond weight merging, the visual layer needs smooth traversal of the generative model's latent space — the ability to slide continuously between images without discrete jumps.

Two candidate architectures have been explored:

**StyleGAN3** is well-suited for smooth traversal. The W-space latent representation is specifically designed for coherent interpolation — walking between two points produces meaningful intermediate states rather than artifacts. Layer-wise style control (coarse layers for structure, fine layers for texture) maps cleanly onto the domain-wall / magnetization split described in §3. The main limitation: StyleGAN3 is a large model; weight drift at the scale required is not feasible at runtime. It is better used as an **exploration and validation tool** — finding what the uncanny zone looks and feels like — than as the installation's final substrate.

**A small custom network** (GAN or VAE, own architecture, own training data) is the realistic installation substrate. Lower parameter count means weight drift is actually feasible at runtime. The latent space is smaller but fully owned — instrumentable, modifiable, driftable. Output fidelity is lower, but for projection in a dark room, lower fidelity serves the aesthetic. Slightly soft, slightly wrong, ambiguous in detail is closer to the dream-logic target than a hyperreal render.

The recommended workflow: **use StyleGAN3 to find the aesthetic, then train the small network to inhabit it**. StyleGAN3 and SD as design tools; the custom network as the installation component.

---

## 5. Relationship to the Existing Architecture

The generative visual layer connects to the existing substrate through the OSC layer already implemented in `osc_io.rs`. The spin chain already emits:

- `/state/magnetization` — global phase indicator
- `/state/spins` — per-site sigma_z values
- `/state/wall_count` — domain wall population
- `/wall/created`, `/wall/destroyed`, `/wall/moved` — wall lifecycle events
- `/clock/pulse` — phase-locked clock signal

These signals already flow into TouchDesigner. The generative layer sits between TouchDesigner and the visual output, receiving these signals as control inputs and producing image material in response.

No changes to the substrate are required. The OSC layer is the interface.

---

## 6. Weight Drift — Technical Considerations

### Catastrophic forgetting

Unconstrained weight drift overwrites previously learned structure. The network doesn't gradually become something new — it rapidly becomes incoherent noise. This is the primary technical risk.

Mitigation: **slow learning rate, bounded drift**. Weights move by a small epsilon per tick, clamped to a maximum distance from their initialization. The phase rigidity of the spin chain is the natural bound — in the locked phase, weights drift so slowly that the cumulative effect over a session is subtle. Only sustained phase-boundary states produce noticeable drift.

### Irreversibility

Weight drift accumulates across the installation's lifetime. Each visitor cohort leaves a permanent mark on future visitors' experience. This is a feature, not a bug — the installation has a history — but it should be a deliberate design decision rather than an accident. Consider: does the network reset between sessions? Between days? Never? Each answer produces a different piece.

### Drift direction

Random drift collapses to noise. Meaningful drift follows a **path** — toward another trained model (the merging case), or along a direction in weight space that was found during development to produce the target aesthetic. The spin chain governs speed of travel along the path, not direction. Direction is a compositional decision made during development.

---

## 7. What This Framework Is and Is Not

It **is** a generative visual layer whose outputs evolve in real time based on the spin chain's phase dynamics, whose weight space is navigable between trained poles, and whose accumulated drift encodes the history of the installation's visitors.

It **is not** a claim that the network "understands" the structures it produces, that the drift produces arbitrary outputs on demand, or that model merging between any two poles will produce aesthetically interesting midpoints. The interesting midpoints have to be found empirically.

It **is** consistent with the existing installation's premise: the visitor is part of the system, their presence shapes what they experience, and the system they interact with is not independent of them.

---

## 8. Next Steps

Three concrete priorities when this becomes active, in order:

**Validate the aesthetic with StyleGAN3 or SD.** Train or fine-tune on brutalist material. Find the uncanny zone manually — what prompt or latent position produces the target quality. Document it in visual terms, not just in parameter terms. This is a design exercise, not an engineering one.

**Choose the poles.** Decide what the merge space spans. Train simple small networks on each pole dataset using the same architecture. Verify that linear interpolation between them produces coherent intermediate outputs rather than noise. The pole choice is the most important creative decision in this layer.

**Instrument the drift.** Build the small network with weight drift as a first-class feature, not an afterthought. Define: what is the drift rate per tick at each phase state? What is the maximum cumulative drift before reset? What is the reset policy? Answer these questions before the network is built, not after.

---

*Companion spec for Crystallized Time | Generative Visual Layer | Parking spec, future work*
