# Stage 2.5 — Substrate clock, per-chain voice, modulation streams

*Companion specification to `stage_1_2_spec.md`. Refines the output topology so the substrate generates a complete musical performance — rhythm, pitch, modulation, and clock — from its own dynamics, before the second chain (Stage 3) is added.*

---

## Purpose

Stage 1–2 produces four independent gate streams from four sites of one chain. Stage 2.5 reorganizes the output so that:

- The chain itself, not the wall clock, drives the master clock used by the rest of the rig.
- Each chain occupies one MIDI channel, with all four sites contributing to a single monophonic voice on that channel (with the original four-channel mode preserved as an option).
- Each chain emits a continuous modulation stream derived from its site dynamics, intended for analog modular CV destinations.
- The program shuts down cleanly without leaving hanging notes.

Stage 2.5 changes no physics. It changes only how the substrate's existing observables are mapped to MIDI output. The Stage 1–2 substrate (classical disordered Floquet spin chain, 8 sites, default parameters) is unchanged.

---

## Why each piece is here

**One channel per chain, monophonic.** The target rig is analog eurorack, fed via MIDI-to-CV. The used eurorack MIDI-to-CV interfaces are monophonic per channel, and the project's compositional unit is the chain rather than the site. Treating each chain as one voice on one channel matches both constraints. The four sites become the chain's *content* (which note plays at any moment) rather than four parallel voices.

**Last-note-wins mono priority.** Of the standard monophonic priority schemes, last-note-wins produces the most rhythmically alive output for this substrate, where multiple sites can fire in close succession near phase boundaries. It also matches the default behavior of nearly all analog mono synths and most MIDI-to-CV interfaces, so the software's behavior matches what the hardware does internally.

**The original four-channel mode kept as a flag.** The Stage 1–2 mapping is useful for debugging the substrate (each site visible on its own channel, easy to inspect on a MIDI monitor) and for setups where four MIDI-to-CV interfaces are available. Flag-switchable, no code path is removed.

**Substrate-derived MIDI clock.** The framework's premise requires that the substrate's autonomy be audible to the rest of the rig. A fixed clock breaks that — the rhythm becomes something the sequencer imposes on the chain. Deriving the clock from chain A's global magnetization ⟨M⟩ makes the substrate the timing authority for everything downstream.

**Gate-on-channel for clock, not MIDI realtime clock.** The rig is gate-driven and analog. A gate channel feeding a clock divider gives the same result as MIDI clock for analog gear, without committing to the 24-PPQN-with-interpolation model that digital gear expects. Clock irregularity becomes a feature: when the chain is locked, the clock is steady; when the chain is near a phase boundary, the clock breathes.

**Per-chain CC modulation stream.** The site σ^z values, summed, form a continuous CV-like signal that responds to the chain's internal dynamics. Sent as a CC stream on the chain's MIDI channel, it can drive filter cutoff, pulse width, or any other modulation destination on the synth that's also receiving the chain's gates and pitches. The result is one chain producing a self-modulated voice — pitch and modulation both substrate-derived.

**Clean shutdown.** A program that leaves hanging gates on eurorack until the rig is power-cycled is broken in a way that matters. Ctrl-C and normal exit must both flush all outstanding notes.

---

## Output topology

### Mode A — `one-channel-per-chain` (new default)

| Output | Channel | Content |
|---|---|---|
| Chain A voice | 1 | Gates with per-site pitches, monophonic (last-note-wins) |
| Chain A modulation | 1 (same channel, CC) | CC from summed σ^z, sampled at tick rate |
| Master clock | 16 | Gate on every ⟨M_A⟩ zero-crossing |

### Mode B — `channel-per-site` (Stage 1–2 mapping, preserved)

| Output | Channel | Content |
|---|---|---|
| Chain A site 0 | 1 | Gate, pitch C3 |
| Chain A site 2 | 2 | Gate, pitch C3 |
| Chain A site 4 | 3 | Gate, pitch C3 |
| Chain A site 6 | 4 | Gate, pitch C3 |
| Chain A modulation | 1 (lowest channel of chain) | CC from summed σ^z |
| Master clock | 16 | Gate on every ⟨M_A⟩ zero-crossing |

The fix from Stage 1–2: site → channel mapping uses the position in `output_sites`, not the raw site index. Channels are 1, 2, 3, 4 — contiguous, as the original spec promised.

In both modes, the master clock and the modulation stream are present. Only the gate routing differs.

---

## Per-site pitch assignment

In Mode A, each of the four sites needs a distinct pitch so the chain's voice has musical content rather than four sites hitting the same note. Default mapping:

| Site | Pitch | MIDI note | Rationale |
|---|---|---|---|
| 0 | C3 | 48 | Root |
| 2 | E3 | 52 | Major third |
| 4 | G3 | 55 | Perfect fifth |
| 6 | B3 | 59 | Major seventh |

This is a Cmaj7 voicing — chosen as a neutral, unambiguous starting point. Quantization to a different scale happens in external gear (your stated workflow). The point of the default is to produce a recognizable harmonic field rather than four arbitrary pitches; users of the program can override the mapping via config.

In Mode B, all sites share pitch C3 (Stage 1–2 default), since each site is on its own channel and downstream pitch comes from CV.

---

## Monophonic priority (Mode A)

When a new site fires while another site's gate is still active on the same channel:

1. Send note-off for the currently-sounding pitch.
2. Send note-on for the new site's pitch.
3. Update "currently-sounding pitch" tracker for this channel.

When a site's gate length expires:

1. If the tracker still shows this site's pitch as currently-sounding, send note-off and clear the tracker.
2. If the tracker shows a different pitch (because another site has since taken over), do nothing — that earlier note-off has already been sent.

This is the standard last-note-wins implementation. The tracker is per-channel state, not per-site, so it generalizes cleanly to Stage 3 (chain B's tracker is independent of chain A's).

---

## Master clock — chain A magnetization

### Signal

`M_A(t) = (1/N) * Σ_i σ^z_i(t)` for chain A.

### Detection

The same zero-crossing detector used for sites, applied to ⟨M⟩ instead of individual σ^z:

- Tighter threshold than per-site (recommended default `0.05`) — magnetization is averaged over N=8 sites and has a smaller noise floor than individual sites.
- Same debounce structure (recommended default `2 ticks` — ⟨M⟩ moves more slowly than single sites, shorter debounce is safe).
- Crossing in either direction emits a clock pulse.

### Output

Gate on dedicated channel (default 16, configurable). Fixed pitch (default C3, irrelevant for clock use). Fixed gate length (default 25 ms — short enough to not bleed into the next clock pulse at typical tempos, long enough for a clock divider to register).

### Behavior across phases

- **Time-crystal phase (f/2 lock):** ⟨M⟩ flips sign every drive period (every ~0.5 s at default tempo). Two clock pulses per crystal period, fed to a `÷2` divider gives quarter notes.
- **Higher-order locking (f/3, f/4):** Pulse rate scales with the lock period.
- **Near phase boundary:** Period jitters; clock breathes.
- **Thermalized:** ⟨M⟩ noise around zero; debounce suppresses most spurious crossings, but the clock degrades and may stop. This is the desired behavior — the clock dying is a real signature of the chain leaving the crystal phase.

The clock has no fallback. If chain A is in the thermal phase, no clock pulses are emitted. The composer's job is to keep chain A in (or near) the time-crystal phase by choosing parameters appropriately.

---

## Modulation CC stream — per-chain σ^z sum

### Signal

`mod_signal(t) = Σ_{i in output_sites} σ^z_i(t)`

For the default config (4 output sites, σ^z ∈ [-1, 1]), this is in [-4, 4].

### Mapping to CC

Linear scale to [0, 127]:

```
cc_value = clamp(round((mod_signal + 4.0) / 8.0 * 127.0), 0, 127)
```

Centered: `mod_signal = 0` maps to CC 64 (the eurorack-standard "no modulation" point for a bipolar CV).

### Sample rate

One CC message per integration tick. At default `ticks_per_period = 25` and 120 BPM, that's 50 Hz. To smooth the stepping (each tick gives a discrete jump), `ticks_per_period` can be raised to 50 or 100 for 100–200 Hz CC update rate without changing the physics.

### CC number and channel

- CC number: configurable, default `1` (mod wheel — universal compatibility).
- Channel: chain's MIDI channel in Mode A, chain's lowest assigned channel in Mode B.
- Filter: only emit a CC message if the value has changed by ≥ 1 since the last emission. Avoids flooding the bus with redundant identical-value messages when the chain is locked at extremes.

---

## Clean shutdown

### Triggers

1. Normal end-of-run (the existing 200-period loop completing, or a future `--periods` flag expiring).
2. SIGINT (Ctrl-C).
3. SIGTERM (graceful kill, e.g. systemd stop).

### Sequence

On any shutdown trigger:

1. Stop accepting new events from the substrate (drop further substrate output silently).
2. Wait for any pending note-offs scheduled by the gate-length mechanism to fire (≤ `gate_length_ms`).
3. Send "All Notes Off" CC (CC 123, value 0) on every channel that has been used during the run (chain channels and clock channel).
4. As a belt-and-braces guarantee, also send "All Sound Off" CC (CC 120, value 0) on the same channels.
5. Disconnect the MIDI port.
6. Exit.

### Implementation

Use the `ctrlc` crate (or equivalent) to install a signal handler that sets an atomic flag. The main loop checks the flag at the top of each tick; when set, it breaks out of the loop and runs the shutdown sequence inline.

The note-off-scheduling refactor (replacing thread-per-note with a single scheduler) noted in the Stage 1–2 review is a prerequisite for clean shutdown to work reliably — otherwise scheduled note-offs in detached threads can fire after `MidiSender` has been dropped, producing errors. Either land the refactor first, or keep an `Arc` to the connection that survives until all scheduled note-offs have completed.

---

## CLI additions

```
--mode <one-channel-per-chain | channel-per-site>
    Output mapping. Default: one-channel-per-chain.

--clock-channel <1..16>
    MIDI channel for the substrate clock. Default: 16.

--mod-cc <0..127>
    CC number for the modulation stream. Default: 1.

--no-clock
    Disable substrate clock output.

--no-mod
    Disable modulation CC output.
```

Existing flags (`--port`, `--bpm`, `--seed`, `--list-ports`) unchanged.

---

## State additions

### Per chain

- Currently-sounding pitch on the chain's channel (Mode A only) — `Option<u8>`.
- Last-emitted CC value — `Option<u8>` (for change-only filter).

### Master clock detector

- Identical to a per-site `EventDetector`, but the input signal is `chain.global_magnetization()` and there's only one output, not a vector.

### Shutdown

- `Arc<AtomicBool>` flag, set by signal handler, polled by main loop.
- `HashSet<u8>` of channels that have been used during the run, populated lazily as gates are sent. Used during shutdown to determine which channels need All Notes Off.

---

## What's intentionally not in Stage 2.5

- **Second chain.** Stage 3.
- **Per-site direction-sensitive clock variants** (only positive-going crossings, etc.). Easy to add later if the symmetric version proves musically thin.
- **MIDI realtime clock messages.** The gate-on-channel approach matches the analog rig. If a digital device that wants 24-PPQN clock enters the patch later, this can be added as an additional output without disturbing the gate clock.
- **Adaptive parameter tuning** to keep chain A in the crystal phase automatically. The composer chooses parameters; if the chain thermalizes, that's information, not a bug.
- **Pitch derived from chain frequency** (the "tachometer" idea from earlier discussion). Defer to Stage 3 — it's most musically interesting when there are two chains in different phases producing pitches in a fixed integer ratio.

---

## Definition of done

1. `--mode` flag switches between Mode A and Mode B output topologies. Mode B reproduces Stage 1–2 behavior with the channel mapping fixed (channels 1–4, contiguous).
2. In Mode A, chain A produces a single monophonic voice on channel 1, with last-note-wins behavior verified by sending overlapping site events and confirming that the second note replaces the first cleanly (no stuck notes, no missed note-offs).
3. The modulation CC stream is present in both modes, on the chain's channel (Mode A) or the chain's lowest channel (Mode B), with values centered on CC 64 when the chain is balanced.
4. The substrate clock emits a gate on channel 16 (or the configured clock channel) on every ⟨M⟩ zero-crossing. In the default-parameter regime, the clock pulses at a recognizable, roughly steady rate corresponding to f/2 on the drive period. Pushing `kT` upward visibly degrades the clock; pushing toward thermalization stops it.
5. Ctrl-C produces a clean exit with no hanging notes on any channel of the rig. A `kill -TERM` of the process produces the same.
6. Patched into eurorack: the clock-gate channel feeds a clock divider, the chain's voice channel feeds a MIDI-to-CV interface, the chain's mod CC drives a filter or VCA. The result is a self-clocked, self-modulated, single-voice performance from one chain that holds for at least five minutes of continuous running without user intervention.

---

## Migration path forward (revisited)

Stage 3 (second chain) plugs into this architecture without structural changes:

- Chain B occupies channel 2 (Mode A) or channels 5–8 (Mode B).
- Chain B emits its own modulation CC on channel 2 / channel 5.
- Chain B does *not* contribute to the master clock — chain A is the timing authority.
- Chain B's parameters target the f/3 phase (per the Stage 3 phase-sweep work in the next-steps doc).

Stage 4 (visitor perturbation) feeds into the existing `PhysicsConfig` mutability path. The Mode A / Mode B split is orthogonal to perturbation — both modes respond identically to parameter changes.

Stage 5 (quantum substrate) is unchanged from the previous plan. The Stage 2.5 output topology is substrate-agnostic; it reads `sz(i)` and `global_magnetization()` from whatever implements the chain interface.

---

*Reference spec for Crystallized Time | Stage 2.5 | Companion to crystallized_time.md and stage_1_2_spec.md*