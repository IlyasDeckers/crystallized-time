# Crystallized Time — Specifications

Consolidated reference for all spec documents. Stage marker conventions: implementation specs describe what was built; parking specs describe research directions not yet committed.

---

## Stage 1–2 — MIDI gate generator from a single chain

`Status: done.`

Generate a stream of MIDI gate events from a simulated classical disordered spin chain under Floquet drive. Four output voices, each driven by one site, emitting note-on/note-off pairs when sigma_z crosses zero. Output routes to a MIDI-to-CV interface.

### Substrate

- Classical disordered spin chain (cheapest substrate with period-multiplication, rigidity, phase transitions).
- Periodic Floquet kick produces period-doubling.
- Disorder in local Z-fields stabilizes the time-crystal phase against thermalization.
- Zero-crossing on sigma_z as the simplest event criterion.

### Defaults

Physics: N=8 sites, dt=0.04, eps=0.10, J=1.2, W=2.0, kT=0.05, output sites [0,2,4,6].
Tempo: drive period 0.5s (= 120 BPM), 25 integration ticks per drive period.
Events: crossing threshold 0.15, debounce 4 ticks.
MIDI: 50ms gate length, fixed pitch C3, velocity from crossing intensity.

### State

Per site: spin vector `[sx,sy,sz]` (unit), local field, prev_sz, last_event_tick.
Per pair: coupling J_ij randomized in [0.7J, 1.3J].
Global: tick counter, simulation time, wall-clock start, seeded RNG.

### Loop

Per tick: Landau-Lifshitz integration step plus thermal noise; if at drive boundary, apply (1-eps)*pi rotation around x to every site; for each output site, check signed zero-crossing with margin and debounce, emit GateEvent if triggered; sleep until wall-clock time matches expected tick time.

### MIDI

`midir` (CoreMIDI / ALSA / WinMM). Four sites map to channels 1–4. Each event: note-on, scheduled note-off after gate length. Velocity = `intensity * 127`.

### Tempo control

`tick_duration = drive_period / 25`. Tempo changeable at runtime; chain state preserved across changes.

### Definition of done

CLI process opens a MIDI output port; four voices emit gates in the default-parameter regime for at least a minute without collapsing or thermalizing; works through a MIDI-to-CV interface into eurorack.

---

## Stage 2.5 — Substrate clock, per-chain voice, modulation streams

`Status: done.` Modulation CC configured via `[chain_a.modulation]` TOML section (channel, cc_number, enabled).

Reorganize the output of one chain so it produces a complete musical performance (rhythm, pitch, modulation, clock) before chain B arrives. No physics changes.

### Goals

- Chain drives the master clock, not wall clock.
- Each chain occupies one MIDI channel; four sites contribute to a single monophonic voice (Mode A).
- Each chain emits a continuous CC modulation stream.
- Clean shutdown with no hanging notes.
- Original four-channel mapping preserved as Mode B.

### Output topologies

Mode A (default): chain A voice on channel 1 with last-note-wins mono, summed-sz CC on the same channel, master clock on channel 16.
Mode B: site 0–3 on channels 1–4 with pitch C3; modulation CC on lowest channel; master clock on channel 16.

### Per-site pitch (Mode A)

Default Cmaj7 voicing: site 0 → C3 (48), site 2 → E3 (52), site 4 → G3 (55), site 6 → B3 (59). Overridable via config.

### Mono priority

Last-note-wins per channel. When a new site fires while another is sounding on the same channel: note-off the prior pitch, note-on the new one, update the per-channel tracker. When a gate length expires: only send note-off if the tracker still shows this site's pitch.

### Master clock

Signal: mean magnetization `M_A(t)`. Detection: zero-crossing with threshold 0.05, debounce 2 ticks. Output: gate on dedicated channel (default 16), pitch C3, gate length 25ms. Behavior: stable in locked phase, jittery near phase boundaries, stops when thermalized. No fallback.

### Modulation CC

Signal: sum of sigma_z over output sites, range [-4,4]. Map linearly to [0,127] centered on CC 64. Sample once per tick (50 Hz at defaults). Filter: emit only if changed by ≥1 since last emission.

### Clean shutdown

Triggers: normal end, SIGINT, SIGTERM. Sequence: stop accepting events; wait for pending note-offs (≤ gate length); send All Notes Off (CC 123) and All Sound Off (CC 120) on every used channel; disconnect; exit. Implementation via `ctrlc` crate plus an `AtomicBool` polled at the top of each tick. Requires the scheduler refactor below.

### Definition of done

`--mode` switches topologies; Mode A mono priority verified with overlapping events; CC stream centered correctly; clock pulses at the right rate, degrades with kT, stops on thermalization; Ctrl-C and SIGTERM leave no hanging notes; five-minute eurorack run holds.

---

## Domain walls

`Status: done.`

A second sonification layer reading spatial structure. A wall lives between sites `i` and `i+1` where `sign(sz[i]) != sign(sz[i+1])`. Walls have identity, persist across ticks, and can move; coexists with site-based voices on different channels.

### Wall mechanics

In the time-crystal phase, walls sit at fixed positions but their orientation flips every drive period (both halves flip together). Single-site flips create wall pairs; isolated single-site domains disappearing annihilate the flanking wall pair. In the thermal phase walls are everywhere and identity tracking breaks down. The musically useful regime is locked-but-perturbed: a small population of walls that persist for many drive periods.

### Detection per tick

1. Build current wall list from adjacent sign-differences.
2. Greedy nearest-neighbor match against previous tick's walls within `match_radius` (default 1.0). With ≤7 walls in an 8-site chain and slow motion, this is correct in practice.
3. Emit `Created` for unmatched candidates, `Destroyed` for unmatched previous walls, `Moved` for matched pairs whose position changed by more than `move_threshold` (default 0.1).
4. Replace internal wall list.

### Position interpolation

`position = i + |sz[i]| / (|sz[i]| + |sz[i+1]|)`. Optional (default on). Gives continuous position even when sites stay integer-occupied.

### Events

```
Created   { id, position, tick }
Destroyed { id, last_position, tick, lifetime_ticks }
Moved     { id, from, to, velocity, tick }
```

### MIDI mapping

Each wall is a held note: note-on at birth, note-off at destruction. Polyphonic across a configurable channel range. Round-robin allocation; voice-stealing on overflow (default OldestActive).

Pitch: linear from `pitch_low` (position 0.5) to `pitch_high` (position n_sites-1.5). Held for the wall's lifetime when `repitch_on_move = false`; reissued as new note-on/note-off pairs across semitone boundaries when `true`.

Motion CC: position normalized to [0,127] on a configurable CC (default CC 1). Disabled with `motion_cc = 0`.

Velocity at birth: derived from local order around the wall's birth position. High order → high velocity (sharp attack); thermal region → low velocity (soft).

Note-off on destruction: immediate, via `send_now`, not the gate scheduler.

### Behavior by phase

- Locked: 0–2 walls, long lifetimes, sparse held notes.
- Locked-but-perturbed (target): persistent mobile walls, the regime where wall sonification earns its place.
- Near boundary: dense, mobile, competes with site voices.
- Thermal: rapid creation/destruction, becomes noise. Real signal that the chain has left the time-crystal phase.

### Open items

Whether held-CC vs repitch-on-move sounds better in practice. Whether 4 channels is enough. Whether the local-order velocity formula is right. Wall orientation (left_sign flipping each drive period) as a separate signal — deferred.

---

## OSC and live parameter mutability

`Status: done.`

Two capabilities: physics parameters (`kt`, `eps`, `j`, `w`) become mutable at runtime with per-parameter smoothing; bidirectional OSC connects to TouchDesigner.

### Why

The four parameters change how the chain behaves without changing what it is — `n_sites`, `ticks_per_period`, and seed are not mutable. Smoothing makes parameter sweeps audible as transitions rather than clicks. `ArcSwap<PhysicsConfig>` for lock-free reads on the simulation thread, with `RwLock<PhysicsTargets>` for OSC writes (simpler than per-scalar atomics, fast enough). OSC over UDP localhost (TouchDesigner-native). Separate ports for in/out. Bundles per tick. Default off (additive).

### Smoothing

Each tick: `snapshot.p += (target.p - snapshot.p) * alpha_p` where `alpha_p = 1 - exp(-dt_real / tau_p)`. Defaults: kt=1.5s, eps=1.0s, j=2.0s, w=2.0s. Not CLI-exposed.

### Inbound

Addresses: `/physics/kt`, `/physics/eps`, `/physics/j`, `/physics/w`. Single float arg. Bounds (clamped silently): kt [0,2], eps [0,0.5], j [0,3], w [0,5]. Malformed packets dropped silently. Per-chain variants `/a/physics/...` and `/b/physics/...` (and `/physics/...` writes both).

### Outbound

Events (per tick, bundled into one UDP packet):
```
/wall/created   id, position, channel
/wall/destroyed id, last_position, lifetime_ticks
/wall/moved     id, from, to, velocity
/site/event     site, voice, intensity
/clock/pulse    magnetization
```

State (throttled to `state_rate_hz`, default 50):
```
/state/spins         [n_sites floats]
/state/magnetization float
/state/wall_count    int
```

OSC bundle timestamp: `immediate` sentinel.

### Concurrency

Receiver thread: blocking `recv_from`, parses, clamps, writes targets via `RwLock::write`. Daemon thread (no shutdown signal).
Sender thread: bounded MPSC (16 bundles, ~320ms). On overflow, drop bundles at the sim side rather than block.

### Config (no longer CLI)

```
[osc]
listen_port    = 9000        # absent disables receiver
send_address   = "127.0.0.1:9001"  # absent disables sender
state_rate_hz  = 50
enable_state   = true
```

### Not in scope

Inbound chain-state perturbations (deferred to localized perturbations spec). Runtime output reconfiguration. Auth/encryption. TCP/WebSocket. Discovery. Session recording. GUI.

---

## MIDI routing via TOML

`Status: done.`

Single config file replaces routing CLI flags. Validated at startup; the config file is the single source of truth for routing, physics, tempo, and OSC.

### Remaining CLI

`--config <path>` (default `crystallized_time.toml`), `--list-ports`, `--port <N>`, `--periods <N>`. All routing flags removed.

### Per-chain layout

```toml
[tempo]
bpm = 120

[osc]
listen_port = 9000
send_address = "127.0.0.1:9001"

[physics]            # shared, applies to chains without per-chain physics
kt = 0.1; eps = 0.01; j = 1.2; w = 2.0; n_sites = 8; ticks_per_period = 25

[chain_a]
seed = 47

[chain_a.physics]    # optional override

[chain_a.gates]
voice_0 = { channel = 1, pitch = 48 }   # full form
voice_2 = 2                              # shorthand: channel only, default pitch
gate_length_ms = 50

[chain_a.walls]      # delete to disable wall output
voice_0 = 5
voice_1 = 6
voice_2 = 7
voice_3 = 8
pitch_low = 36
pitch_high = 84
motion_cc = 1        # 0 disables
repitch_on_move = false

[chain_a.clock]
channel = 16

[chain_a.modulation]  # optional; omit to disable
enabled = true
channel = 1           # defaults to first gate voice channel
cc_number = 1
```

### Wall voice routing — design choice taken

Option 1 (fixed named voices) chosen over Option 2 (named pool). Each wall voice is named `voice_N` with an explicit channel, matching the gate-voice pattern. Round-robin allocation across these named entries; oldest-active stealing on overflow.

### Validation

Channels must be 1..=16. No channel may be claimed by more than one signal across gates, walls, clock, and both chains. `voice_N` indices must correspond to real sites (`N < n_sites`). Pitches and CC numbers in range. Errors name the offending entry: `channel 16 is assigned to both chain_a.clock and chain_b.walls.voice_3`.

### Defaults

A default `config.toml` reproduces pre-config behavior exactly (chain A only, four gate voices on a single channel with Cmaj7 pitches, four wall voices on channels 5–8, clock on 16).

### Module

`src/config/config_file.rs` owns TOML structs, `load(path) -> Result<Config, ConfigError>`, and conversion to the existing `Config`. The runtime `Config` is unchanged; this is a new deserialization front-end.

---

## Localized perturbations

`Status: done.`

Single-site, single-time disturbances. Chain becomes a thing that can be played into. MIDI input first; same mechanism used later for sensors.

### Three kinds

```rust
Flip                          // negate sz, then renormalize
Rotate { axis, angle }        // rotation around X/Y/Z by angle
FieldSpike { delta: Vec3 }    // one-tick effective field addition
```

`pending_field_deltas: Vec<Option<Vec3>>` on the chain; consumed and cleared in `step`. Flip is violent; rotate is gentle; spike is forced for one tick. Site index bounds-checked; out-of-range requests silently dropped.

### Routing

`PerturbationRouter` (pure, immutable config) translates `RawMidiMessage` into `(site, kind)`. Note-on with non-zero velocity → perturbation. Note-off and CC dropped.

Site mapping: `site = (note - base_note).rem_euclid(n_sites)`.
Magnitude scaling: `scale = (velocity / 127) * velocity_scale`. Flip ignores scale. Rotate scales the angle. Spike scales the delta on the chosen axis.

### Input layer

`MidiInputListener` mirrors `MidiSender`. `midir` callback pushes `RawMidiMessage` into an `mpsc::Sender`; main loop drains via `poll()` once per tick. Callback stays cheap (atomic ops only).

### Loop integration

```
step()
poll input → perturb()       // after step, before event detection
detect events
emit
```

Perturbations land on the tick they arrive. Large flips produce immediate zero-crossings; small rotations show up later as the chain redistributes.

### Config

```toml
[input.perturbation]
base_note = 60
kind = "rotate"      # flip | rotate | field_spike
axis = "x"
magnitude = 0.3
velocity_scale = 1.0
```

CLI: `--list-input-ports`, `--input-port <N>`. Absence of `--input-port` keeps autonomous behavior. Warning if `--input-port` given without `[input]` in config.

### Open items

Right magnitude (find by playing). Legibility vs autonomy tradeoff (framework §5). Velocity curve. Note-to-site mapping alternatives (chromatic chord = different kinds, etc.). Quantize to drive boundaries vs apply on arrival. Polyphony semantics.

### Not in scope

Continuous CC-driven perturbation streams. Persistent coupling changes (parameter mutation already covers this). Output-to-input feedback. Multi-port input. Session recording.

---

## Layered drives

`Status: parked. Speculative substrate extension.`

A second simultaneous Floquet kick at a different period on the same chain. Tests whether a single chain can host polyrhythmic content under quasi-periodic drive.

### Why

Two motivations. Polyrhythm via one chain with two drives, as an alternative to two coupled chains (Stage 3). Quasi-periodic time crystals are a real research direction; whether the small disordered classical chain has a viable QP regime is unknown.

### Changes

`DriveSchedule { ticks_per_period, axis, angle, phase_offset }`. `PhysicsConfig.drives: Vec<DriveSchedule>`. Default `drives` = one entry matching current behavior. `step()` checks each drive; pulses applied in `drives`-order on collision ticks (matters for non-commuting axes).

### Two drives only

Three+ multiplies the parameter space intractably. Same-axis drives commute (sum of angles); different-axis drives don't (order and timing matter).

### CLI

```
--drive2-period <ticks>
--drive2-axis <x|y|z>          default y
--drive2-angle <radians>       default pi/2
--drive2-offset <ticks>        default 0
--drive2-ratio <num:den>       frequency-ratio shorthand (period = first / ratio)
```

### Sonification

No change to event detector or clock. They see the chain's observables and don't care how it got there. Four possible regimes: ignored (locks at primary sub-harmonic), combined locking (joint sub-harmonic, polyrhythm from one chain), quasi-periodic (almost-repeating, not noise), thermalized.

### Parameter exploration

Sweep tool. Fix primary drive at working defaults, sweep (period, angle) of the secondary. Run 200 periods per point, compute global-M autocorrelation after warmup, find peaks, classify. Same shape as the Stage 3 phase-sweep tool. Days of work.

### Open

Whether the chain supports QP locking at this size. Whether it's audible as distinct from a modulated single drive. Whether the regime is stable enough to compose with. Whether two drives on one chain match two chains coupled.

---

## Generative visual layer

`Status: parked. Explore after TouchDesigner visual pipeline.`

Small generative network whose weights drift in real time, governed by chain phase state. Latent space navigable between trained poles.

### Core idea

Drifting weights produce internally-consistent but rule-violating outputs. Phase rigidity governs drift rate: locked → little drift, near-boundary → faster, thermal → halts or oscillates. Visitor doesn't control directly; they perturb the chain, the chain shifts the grammar.

### Aesthetic

Brutalist architecture as the primary vocabulary. Target: confidently alien. Candidate poles: brutalism, geological formations, industrial infrastructure, deep-sea structures. Merging between poles is the compositional decision. Layer-wise merging splits structure (coarse layers) from texture (fine layers) and maps to magnetization vs domain wall density.

### Architecture choice

StyleGAN3 as exploration/validation tool — smooth latent traversal, layer-wise style control, too big to drift at runtime. Small custom network (GAN or VAE) as the installation substrate — fewer parameters, drift feasible, lower fidelity that serves the aesthetic. Workflow: StyleGAN3 finds the aesthetic; small network inhabits it.

### Connection to substrate

Uses existing OSC outputs (`/state/magnetization`, `/state/spins`, `/state/wall_count`, wall lifecycle events, `/clock/pulse`). No substrate changes required.

### Drift constraints

Slow learning rate, bounded distance from initialization. Phase rigidity is the natural bound. Reset policy is a deliberate decision (between sessions? days? never?). Direction is a development-time choice; phase governs speed, not direction.

### Next steps when active

Validate aesthetic with StyleGAN3 or SD on brutalist material. Choose poles and verify linear interpolation produces coherent midpoints. Instrument drift as a first-class feature (rate per phase state, max cumulative drift, reset policy).

---

## Stage 3+ roadmap

`Status: ~80% implemented. Two-chain, coupling, per-chain routing, and OSC namespacing work. sweep tool is single-chain only; site_paired and shared_drive coupling shapes are stubs.`

### Stage 3 — Polyrhythmic two-chain

Two `SpinChain` instances, independent seeds, optionally weakly coupled through a shared field term tied to the other chain's mean magnetization. Targets f/2 against f/3 (6T common period at default tempo ≈ 3s). Two `EventDetector`s, eight voices total. Sweep tool to find f/3 parameters. ~1 day plus sweep time. Musical question: is the lcm recurrence audible.

### Stage 4 — Visitor perturbation

Three input modalities, increasing engineering cost: MIDI CC input (knob → parameter), file watch (edit config while running), sensor input (webcam / ultrasonic / mic). `perturbation.rs` owns input and produces `ParameterDelta` stream. Smoothing filter on input. Central design question (framework §5): tightness of coupling. ~1 day for MIDI CC; ~1 week for sensors.

### Stage 5 — Quantum substrate

Replace inner loop of `SpinChain::step` with a Floquet circuit via quantrs2. Per drive period: RZZ between neighbors with angle J*dt, RZ on each site with angle h*dt, optional depolarizing noise; at drive boundary, RX on each site with angle (1-eps)*pi. Read sigma_z expectations to feed the same `EventDetector`. Common `Substrate` trait so the rest of the system is substrate-agnostic. Start at 8 sites. Validate classical match in low-entanglement regime, then push to where they diverge (thermal phase, near boundaries, strong entanglement). 1–2 weeks; debugging dominates.

### Stage 6 — Eigenmode sonification

Project chain state onto spatial Fourier modes (or eigenvectors of effective Hamiltonian). Each mode is a voice. Mode index → pitch, amplitude → volume. Output is CC streams plus gate events for dominant modes. Receiving side: CC-controlled amplitudes on continuously-running oscillators (additive synthesis). Different paradigm: rhythm-first becomes harmony-first. Independent of substrate question. 2–3 days plus open-ended musical-mapping work.

### Order

Roughly dependency-ordered, not obligatory. Stage 5 benefits from Stage 3 (more sites, more entanglement, more quantum/classical divergence). Stage 6 is substrate-independent. The right next stage is whichever answers a question raised by listening.

### Not on the list

GUI. Multi-machine distribution. Composition-language abstraction layer. Premature performance optimization. ML on substrate output.

---

*End of consolidated specification reference.*