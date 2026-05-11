# Domain Walls — Sonifying the Boundaries Between Ordered Regions

*Companion specification to `stage_1_2_spec.md`, `stage_2-5_spec.md`, and `stage_3_plus_reference.md`. Implementation spec — next on the list after Stage 2.5.*

`Status: finished`

---

## Purpose

The current substrate produces music through zero-crossing detection on individual sites. Each of four output sites is a fixed voice, firing whenever its σ_z flips sign. The voice is *anchored* to its site — voice 1 is always site 0, voice 2 is always site 2, and so on. The chain's spatial structure is invisible to the sonification; the four voices could equivalently come from four independent oscillators with no coupling between them.

This specification describes a parallel sonification layer that reads the chain's *spatial* structure: domain walls — the boundaries between regions of opposite spin orientation. Walls are point-like objects with positions and velocities. They drift, get created in pairs, annihilate in pairs. They exist only when the chain is partially ordered — neither fully locked nor fully thermal — which is exactly the regime the project is trying to live in.

Sonifying walls produces music with a fundamentally different temporal structure than the existing site-based voices. Voices are no longer fixed in number or in space. A wall born somewhere in the chain becomes a voice that exists for as long as the wall persists, and that travels in some sound parameter (pitch, pan, filter cutoff) as the wall moves. When the wall annihilates, the voice ends.

This is the *texture-first* sonification described in the framework document's §4. It coexists with the existing rhythm-first sonification — the two together give the substrate two simultaneous musical layers reading different aspects of the same physics.

---

## Why each piece is here

**Walls as point objects, not as a per-site classification.** A wall is an object that *exists* — it has identity, it persists across ticks, it can move. Treating walls this way (rather than as "site i and site i+1 have opposite signs right now") makes wall-tracking the natural operation. Voice continuity comes from object continuity.

**Identity preserved across ticks via nearest-neighbor matching.** A wall between sites 2 and 3 last tick and a wall between sites 3 and 4 this tick is *the same wall, moved right*. Without matching, every tick would see a fresh wall list, and walls wouldn't have identity — every tick would be either a complete birth-death cycle or an opaque rearrangement. The matching step is what gives walls musical continuity.

**Greedy nearest-neighbor matching is good enough.** The chain has 8 sites, so at most 7 walls. Walls move slowly relative to the integration timestep (a wall typically moves at most one site per tick under normal dynamics). Greedy matching against a small list of candidates is correct in practice and trivial to implement. Hungarian-algorithm-style optimal matching would be overkill.

**Walls are coexistent with site-based voices, not a replacement.** The existing rhythm-first sonification works and produces music the project wants. Walls add a second layer reading different information. Both running simultaneously gives the chain two parallel musical interpretations of itself, on independent MIDI channels.

**Sub-tick wall position via interpolation.** A wall lives between two sites, but it's natural to assign it a continuous position (e.g. 2.5 for "between site 2 and site 3"). Interpolating gives walls finer spatial resolution than the integer site grid — useful when mapping position to pitch or pan, which both prefer continuous values.

**Wall events as note-on / note-off, not as gates.** The existing sites use 50 ms gates because each event is a discrete trigger — the gate is the signal. Walls are persistent voices whose lifetime is *the wall's lifetime*. So a wall is born → note-on, the wall lives for some number of ticks → note held with optional CC modulation tracking position, the wall annihilates → note-off. This is closer to a normal MIDI voice than to a gate trigger.

---

## What walls actually are, in this substrate

Concretely: a wall lives between sites `i` and `i+1` where `sign(spins[i].z) != sign(spins[i+1].z)`. The chain's wall list at any tick is the set of all such `i`.

In the time-crystal phase, both halves of the chain (the sites with positive sz and the sites with negative sz) flip together every drive period. A wall between sites 2 and 3 *stays between sites 2 and 3* — both sides flip, the boundary doesn't move, but the *direction* of the boundary inverts (was "+ → -", now "- → +"). The wall is rigid in position but has an orientation that flips.

When a single site flips against its neighbors (because of disorder, noise, or perturbation), two walls are created — one on each side of the flipped site. Conversely, when an isolated single-site domain disappears (it flips back, or its neighbors flip), the two flanking walls annihilate.

In the thermal phase, walls are everywhere and constantly being created and destroyed. The wall list changes radically every tick. Tracking individual walls breaks down — there's no meaningful identity to preserve.

The musically interesting regime is **locked-but-perturbed**: the chain is mostly in the time-crystal phase, but disorder, noise, or perturbation produces a small population of walls that persist for many drive periods before annihilating. This is the regime where wall sonification produces sparse, mobile, identifiable voices rather than either silence (fully locked) or noise (fully thermal).

---

## State

### Per wall

A `Wall` struct with the information the sonification needs:

```rust
pub struct Wall {
    /// Persistent identity. Assigned at creation, not reused.
    pub id: u64,
    /// Sub-tick position. 2.5 means "between sites 2 and 3".
    pub position: f64,
    /// Position-per-tick. Computed from previous and current positions on
    /// each match. Zero on the tick of creation.
    pub velocity: f64,
    /// Tick when this wall was created. Used for age and for note-on
    /// timestamps.
    pub birth_tick: u64,
    /// Sign of the left side. +1 if sites left of the wall are positive,
    /// -1 if negative. Flips every drive period in the time-crystal phase
    /// (walls invert orientation when both halves flip together).
    pub left_sign: i8,
}
```

### Per detector

```rust
pub struct WallDetector {
    config: WallConfig,
    /// Walls present at the previous tick.
    walls: Vec<Wall>,
    /// Monotonic counter for assigning new wall IDs.
    next_id: u64,
}
```

### Per voice allocator

The detector emits events; a separate `WallVoiceAllocator` (analogous to the existing per-channel sounding tracker in `MidiSender`) maintains the wall-id-to-MIDI-pitch mapping and the active note list. Discussed in the MIDI section below.

---

## Loop

### Per-tick wall detection

```text
1. Build current wall list:
   for i in 0..n_sites - 1:
       if sign(spins[i].z) != sign(spins[i+1].z):
           position = i + 0.5 + epsilon-correction (see Position section)
           candidate_walls.push((position, sign(spins[i].z)))

2. Match candidate walls to previous walls:
   - Sort previous walls and candidates by position.
   - For each candidate, find the previous wall with the smallest position
     difference within `match_radius`. If found, the candidate inherits the
     previous wall's id; the previous wall is marked as matched.
   - Candidates without a match are new walls (creation events).
   - Previous walls without a match are gone (annihilation events).

3. Emit events:
   - WallCreated(id, position) for each unmatched candidate.
   - WallDestroyed(id, last_position) for each unmatched previous wall.
   - WallMoved(id, old_position, new_position) for each matched pair where
     position changed by more than `move_threshold`.

4. Update detector state:
   - Replace walls with the matched + newly-created list.
   - Update each wall's velocity from old/new positions.
   - Update each wall's left_sign from current chain state.
```

The matching is greedy and proximity-based. With at most 7 walls in an 8-site chain and walls moving slowly, this is correct in practice. If a future longer chain produces matching errors, switch to Hungarian-style optimal matching — but verify the problem before adding complexity.

### Position interpolation

A wall lives between sites `i` and `i+1`. Its base position is `i + 0.5`. For finer resolution, interpolate using the magnitudes of the two adjacent sz values:

```text
interpolated = i + |spins[i].z| / (|spins[i].z| + |spins[i+1].z|)
```

If `spins[i].z` is much larger in magnitude than `spins[i+1].z`, the wall sits closer to `i+1` (the small-magnitude side is closer to crossing zero, so the boundary is geographically closer to it). If the magnitudes are equal, the wall is at `i + 0.5`. The interpolation gives walls a continuous position parameter even at fixed integer site occupancy, smoothing the wall's apparent motion.

This refinement is optional. Integer-plus-half positions work for a first version; the interpolation only matters if continuous wall position drives a sensitive parameter like pitch.

### Match radius

`match_radius` (in position units) bounds how far a wall can move between ticks and still be considered the same wall. Default: 1.0 — a wall can move at most one site per tick. At default `ticks_per_period = 25`, this is a generous bound; walls in the time-crystal phase typically move 0 sites per tick, with rare jumps of 1.

Setting this too tight produces spurious destruction-creation pairs every time a wall jumps. Setting it too loose means walls in the thermal phase get matched across long distances, which is meaningless.

### Move threshold

Small fluctuations in interpolated position (< 0.05 units, say) shouldn't fire move events. The `move_threshold` parameter filters these. Default: 0.1.

---

## Event model

Three event kinds, distinct from the existing `GateEvent`:

```rust
pub enum WallEvent {
    Created {
        id: u64,
        position: f64,
        tick: u64,
    },
    Destroyed {
        id: u64,
        last_position: f64,
        tick: u64,
        lifetime_ticks: u64,
    },
    Moved {
        id: u64,
        from: f64,
        to: f64,
        velocity: f64,
        tick: u64,
    },
}
```

The detector returns `Vec<WallEvent>` per tick. Most ticks in the locked phase will return zero or one event (walls don't move, so most ticks have no events at all). Ticks with annihilation events will typically have two destruction events (walls annihilate in pairs). Perturbations produce paired creation events.

`Moved` events fire frequently in the thermal phase and rarely in the locked phase. They are the main channel for continuous wall-position information; the MIDI router translates them to CC messages, not new note-ons.

---

## MIDI mapping

### Voice allocation

Each `Created` event allocates a MIDI voice. Each `Destroyed` event frees one. The allocator maintains a wall-id-to-(channel, pitch) mapping.

```rust
pub struct WallVoiceAllocator {
    config: WallMidiConfig,
    /// Currently active voices: wall_id -> (channel, pitch).
    active: HashMap<u64, (u8, u8)>,
    /// Per-channel state for round-robin: which channel to try next.
    next_channel: u8,
}
```

Voice allocation strategy:

- Walls are polyphonic, unlike the per-chain mono in Stage 2.5's Mode A. A wall is its own object; cutting it off when another wall is born would lose the wall's identity.
- Voices are distributed across a configurable channel range (default: channels 5–8, leaving channels 1–4 for the existing site-based voices in Mode A or Mode B).
- Channel allocation is round-robin among available channels. If all channels are occupied, the oldest active wall is voice-stolen (its note-off fires, the new wall takes its channel). This is a corner case — at 8 sites the chain has at most 7 walls, and 4 channels is enough for typical wall populations in the locked-but-perturbed regime.

### Pitch mapping

Wall position maps to pitch. Default mapping: position 0.5 (left edge of chain) = MIDI 36 (C2), position 7.5 (right edge of chain) = MIDI 84 (C6). Linear interpolation between, quantized to the nearest semitone or to a configured scale.

For an 8-site chain this gives 4 octaves of range. Operators who want narrower or wider range adjust the endpoints in config.

Pitch is set at creation and held for the wall's lifetime. Wall *motion* updates a CC, not the pitch itself — so a moving wall produces glissando-like behavior through the receiving synth's portamento or through CC-driven detuning, not through repeated note-ons.

(Alternative: re-emit note-on/note-off pairs as the wall crosses semitone boundaries. This produces audible discrete pitch changes rather than smooth motion. Worth a flag — both behaviors are musically valid for different intents.)

### CC mapping for motion

`Moved` events become CC messages on the wall's channel. The CC number is configurable; default is CC 1 (mod wheel). The CC value is derived from wall position, mapped to 0–127 across the position range:

```text
cc_value = clamp(round(position / (n_sites - 1) * 127), 0, 127)
```

This sends a continuous trace of wall position alongside the held note. Receiving synths can use the CC for filter cutoff, pulse-width modulation, panning, or anything else.

Velocity (the wall's motion-per-tick, not MIDI velocity) is available but not mapped to CC by default. Worth a config option: a fast-moving wall could drive an intensity parameter. Not in scope for the first version.

### Note velocity

MIDI velocity for the note-on at wall creation: derived from the rate of creation. A wall that emerges out of a stable region (the chain was uniform, then a single site flipped, then walls appeared) creates with high velocity. A wall that emerges in a thrashing thermal region creates with low velocity.

Concretely: `velocity = clamp(round(local_order * 127), 1, 127)`, where `local_order` is the average alignment of sites within some window around the wall's birth position. Sites mostly aligned → high local order → high velocity. Sites already disordered → low local order → low velocity.

This makes wall-births in clean regions sound like clear sharp attacks; wall-births in thermal regions sound soft and indistinct, blending into the texture rather than punching through it.

### Note-off on destruction

Straightforward. `Destroyed` event → note-off for the wall's allocated (channel, pitch). The voice allocator frees the channel for reuse.

A subtlety: the existing scheduler infrastructure handles delayed note-offs for fixed-gate notes. Wall note-offs are *immediate* — they fire at the moment of destruction, not at a fixed delay after creation. The scheduler supports immediate sends via `send_now`; route wall note-offs through that path rather than scheduling them.

---

## Configuration

A new struct alongside the existing physics / events / MIDI / clock configs:

```rust
pub struct WallConfig {
    /// Whether wall detection is active.
    pub enabled: bool,
    /// Greedy match radius in position units.
    pub match_radius: f64,
    /// Minimum position change to fire a Moved event.
    pub move_threshold: f64,
    /// Interpolate wall position from sz magnitudes (true) or use integer+0.5 (false).
    pub interpolate_position: bool,
}

pub struct WallMidiConfig {
    /// Channel range for wall voices (inclusive).
    pub channel_low: u8,
    pub channel_high: u8,
    /// Position-to-pitch endpoints.
    pub pitch_low: u8,    // pitch at position 0.5
    pub pitch_high: u8,   // pitch at position (n_sites - 1) - 0.5
    /// CC number for wall motion. None disables motion CC.
    pub motion_cc: Option<u8>,
    /// Whether moving walls re-emit note-ons across semitone boundaries
    /// (false = held pitch + CC, true = discrete repitching).
    pub repitch_on_move: bool,
    /// Voice-stealing strategy when no channels are free.
    pub voice_steal: VoiceStealStrategy,
}

pub enum VoiceStealStrategy {
    OldestActive,
    NewestActive,
    None,  // drop new wall events if no voices free
}
```

Defaults:

```rust
impl Default for WallConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            match_radius: 1.0,
            move_threshold: 0.1,
            interpolate_position: true,
        }
    }
}

impl Default for WallMidiConfig {
    fn default() -> Self {
        Self {
            channel_low: 4,   // MIDI channel 5 (0-indexed 4)
            channel_high: 7,  // MIDI channel 8 (0-indexed 7)
            pitch_low: 36,    // C2
            pitch_high: 84,   // C6
            motion_cc: Some(1),
            repitch_on_move: false,
            voice_steal: VoiceStealStrategy::OldestActive,
        }
    }
}
```

### CLI additions

```
--no-walls
    Disable wall detection and output. Walls are on by default.

--wall-channels <low>:<high>
    Channel range for wall voices. Default: 5:8 (1-indexed).

--wall-pitch-range <low>:<high>
    MIDI pitch range that walls span. Default: 36:84 (C2 to C6).

--wall-motion-cc <0..127>
    CC number for wall motion. 0 disables motion CC. Default: 1.

--wall-repitch-on-move
    Discrete repitching as walls cross semitones, instead of held pitch + CC.
```

---

## Module layout

A new module `src/walls.rs`, alongside `events.rs` and `clock.rs`. It owns:

- `Wall`, `WallEvent` types.
- `WallDetector` with the per-tick `check(&chain)` method, mirroring `EventDetector`'s shape.

A new module `src/wall_midi.rs` (or extension of `midi.rs`, decision point) for:

- `WallVoiceAllocator`.
- `MidiSender::send_wall_event(...)` for routing wall events to MIDI.

Recommend keeping voice allocation in its own module — `midi.rs` is already doing two output topologies and the substrate clock; adding wall voicing inline would obscure the structure. A separate module that *uses* `MidiSender` for byte-level output keeps responsibilities clear.

The main loop's per-tick body adds:

```rust
let wall_events = wall_detector.check(&chain);
for event in wall_events {
    wall_voicer.handle(event, &midi_sender);
}
```

Inserted alongside the existing `detector.check(&chain)` block. Walls and sites detect independently; both contribute to the MIDI output stream.

---

## What walls actually sound like — predicted behavior in each phase

**Deeply locked phase (low ε, low kT, tight disorder).** Few walls — typically 0 to 2, born from disorder fluctuations and slowly drifting along the chain. Wall lifetimes are long (many drive periods). Musically: sparse held notes that drift slowly in pitch (or in CC), long silences between events. The wall layer is *quiet* in this phase; the site-based voices carry most of the rhythm.

**Locked-but-perturbed phase** (the target regime). Walls are present, persistent, and mobile. Creation events are rare but distinct. Wall populations vary slowly. Musically: held notes with slow movement, occasional new entries and graceful exits, alongside the rhythmic content from the site-based voices. This is where wall sonification earns its place.

**Near phase boundary.** Walls are abundant and dynamic. Creation and destruction events are frequent. Wall positions move rapidly. Musically: the wall layer thickens, becomes busy, and starts to compete with the site-based voices for foreground. Useful as a phase indicator audible in the music itself.

**Thermal phase.** Walls are everywhere, constantly being created and destroyed, lifetimes short. The matching breaks down — walls don't have meaningful identities to track. Musically: rapid voice-on / voice-off cycling, no voice persists long enough to register as a held note. The wall layer becomes noise. This is consistent with the other observables (the substrate clock dies, the site-based voices thermalize) — all signals point to the chain having left its musically useful regime.

---

## Open questions

**Whether held notes with CC modulation, or repitching at semitone boundaries, sounds better.** Held notes are smoother and reflect the physics (a wall moving from position 3.4 to 3.6 *is the same wall*, not two different walls). Repitching is more rhythmically articulated and fits better with sequenced or arpeggiated patches. Both are valid; the spec keeps the choice as a flag and defers committing.

**Whether 4 channels of wall voices is enough.** In the locked-but-perturbed regime, the chain typically has 1–3 walls. In thermalizing transitions it may briefly spike to 5+. Voice-stealing handles overflow but loses information. Worth checking by ear after the first implementation whether overflow happens often enough to matter.

**Velocity from local order — whether the formula is right.** "Local order around the wall's birth position" is the right *direction* but the specific window size and weighting is unmotivated. Worth tuning by ear.

**Wall orientation as a separate signal.** Walls have a `left_sign` that flips every drive period in the locked phase. This is a fast, regular signal — could be sonified independently (e.g., as a CC sweep or a separate channel) but is omitted from the spec to keep the first version focused. Note for future iteration.

**Interaction with site-based voices.** The two sonification layers run on different channels and don't directly conflict, but musically they're competing for attention. In Mode A (Stage 2.5 default), site voices are on channel 1, walls on channels 5–8 — independent synth voices. In Mode B (channel-per-site), site voices occupy channels 1–4, walls 5–8 — same separation. No structural conflict. Whether they complement or muddy each other musically is empirical.

**Whether wall sonification needs its own clock.** The substrate clock derived from ⟨M⟩ is the project's master timing reference. Walls don't need a separate clock — their events are unambiguously timestamped by tick. The existing clock continues to drive downstream timing.

---

## What's intentionally not in scope

- **Multi-chain wall sonification.** When Stage 3 lands, each chain has its own walls. The wall detector and voice allocator generalize trivially to multiple chains (one detector per chain, walls from each chain on disjoint channel ranges). Not in this spec — single-chain first.
- **Wall-wall interaction tracking.** Two walls colliding produces two destruction events on the same tick; the spec emits them but doesn't *recognize* the collision as a single musical event. Recognizing collisions and emitting a special "annihilation" event with combined properties is interesting but adds complexity; defer.
- **Wall sub-types.** Walls between specific sign patterns (e.g., a wall preceded by a "+ +" region vs by a "+ - +" region) could be sonified differently. The chain's geometry isn't rich enough at 8 sites for this distinction to matter much; defer.
- **Quantum-substrate walls.** When the substrate becomes quantum (Stage 5), "wall" means something different — a domain wall in a quantum spin chain is a coherent superposition of wall states, not a sharp object. The classical wall detector doesn't transfer directly. The framework's eigenmode sonification (Stage 6) is closer to the right approach for the quantum case. Not this spec's problem.
- **Wall-derived modulation of site voices.** A nearby wall could modulate the pitch of a site-based voice. Possible cross-layer interaction; out of scope for the first version. Worth considering once both layers exist and have been listened to independently.

---

## Definition of done

1. `WallDetector::check(&chain)` correctly identifies walls each tick, matches them to the previous tick's walls within `match_radius`, and emits `Created` / `Destroyed` / `Moved` events with stable wall IDs.
2. `WallVoiceAllocator` allocates MIDI voices on creation, releases them on destruction, and handles voice-stealing when overflow occurs.
3. Wall position maps to MIDI pitch at note-on. Wall motion produces a CC stream (or repitch events, depending on flag).
4. In the default-parameter regime, listening to the wall channel(s) for several minutes reveals walls being born, drifting, and annihilating, with audible distinction between the locked phase (sparse, slow) and the near-boundary regime (active, mobile).
5. The wall layer coexists with the existing site-based voices on independent MIDI channels, both contributing to the eurorack patch without channel conflicts.
6. CLI flags allow toggling wall output, configuring channel range and pitch range, and choosing between the held-pitch-with-CC and repitch-on-semitone-cross modes.
7. Pushing `kT` upward audibly thickens the wall layer and eventually transitions it to noise, mirroring the substrate clock's behavior. The wall layer becoming noise is a real signature of the chain leaving the time-crystal phase, not a bug.

---

## Migration path

The wall infrastructure carries forward straightforwardly:

- **Stage 3 (second chain)** — each chain has its own `WallDetector` and `WallVoiceAllocator`. Chain B's walls go to a different channel range. Polyrhythmic content emerges naturally if the two chains' walls have different lifetime distributions.
- **Localized perturbations** (parking spec) — perturbations create walls deliberately. Wall sonification is the sound of the chain's response to perturbation, in real time. The two specs combine into a much richer instrument than either alone.
- **Layered drives** (parking spec) — under layered or quasi-periodic drives, wall dynamics may have richer structure (drift on one timescale, oscillate on another). Wall sonification gives a clean audible signal of whether that's happening.
- **Stage 4 (visitor perturbation)** — once walls exist as voices, perturbations from visitors directly shape the wall population. Walls are the natural audible signature of "the chain is responding to me."
- **Stage 5 (quantum substrate)** — wall detection in the classical sense doesn't transfer. The quantum analog is closer to the eigenmode sonification (Stage 6). This spec's wall code is *not* intended to survive the quantum substrate transition; that's expected. The classical wall layer is a feature of the classical substrate, and when the substrate changes, the sonification changes with it.

---

*Companion spec for Crystallized Time | Domain Walls | Implementation spec, next on the list*
