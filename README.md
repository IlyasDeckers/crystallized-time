# Crystallized Time

A time-crystal-driven MIDI gate generator. Simulates a disordered classical spin chain under periodic Floquet drive and converts its dynamics into MIDI output for a eurorack synthesizer. The rhythm, pitch, and modulation signals all emerge from the simulation rather than being programmed directly.

This repository contains the first working component of a larger audiovisual installation. See [`crystallized_time.md`](crystallized_time.md) for the full project description.

---

## How it works

**Simulation.** The program runs a model of eight coupled spins. A periodic kick is applied to the whole chain at regular intervals. Under the right conditions (appropriate disorder, coupling strength, and temperature), the chain enters a period-doubled phase: each spin flips at half the drive rate. Nothing in the code specifies this rhythm; it emerges from the physics.

**Readout.** Two observables are tracked simultaneously. Four of the eight spins are watched for zero-crossings (sign flips from up to down or back). Domain walls -- boundaries between adjacent spins pointing in opposite directions -- are tracked as mobile objects that appear, drift along the chain, and annihilate.

**MIDI output.** Each zero-crossing produces a 50ms gate pulse, useful for triggering envelopes and rhythmic events. Each domain wall produces a held note: note-on when the wall is born, note-off when it annihilates. Wall motion is reported as a CC stream (position mapped to a configurable CC number) or as discrete repitching when the wall crosses semitone boundaries. A chain clock on its own channel derives from the chain's mean magnetization crossing zero, providing a beat signal that degrades naturally when the chain leaves the locked phase.

---

## Running

```sh
cargo build --release
cargo run --release -- --list-ports     # see available MIDI outputs
cargo run --release -- --port 1         # run, sending to port 1
```

A MIDI destination is required. On Windows, install [loopMIDI](https://www.tobias-erichsen.de/software/loopmidi.html) and create a virtual port. On macOS, use the IAC driver. For hardware, a USB MIDI-to-CV interface will appear directly in the port list.

---

## Command-line flags

| Flag | Default | Description |
|---|---|---|
| `--list-ports` | | Print available MIDI output ports and exit. |
| `--port <N>` | `0` | Output port index. |
| `--bpm <N>` | `120` | Tempo. One drive period = one beat. |
| `--seed <N>` | `47` | RNG seed. Same seed produces the same run. |
| `--mode <name>` | `one-channel-per-chain` | Output topology. See below. |
| `--clock-channel <N>` | `16` | MIDI channel for the chain clock (1-16). |
| `--no-clock` | | Disable the chain clock. |
| `--no-walls` | | Disable domain-wall detection. |
| `--wall-channels <lo>:<hi>` | `5:8` | Channel range for wall voices (1-16). |
| `--wall-pitch-range <lo>:<hi>` | `36:84` | Pitch range walls span. |
| `--wall-motion-cc <N>` | `1` | CC number for wall position. `0` disables. |
| `--wall-repitch-on-move` | | Discrete repitching on wall motion instead of held pitch + CC. |
| `--osc-listen <port>` | | UDP port for inbound OSC parameter control. |
| `--osc-send <host:port>` | | UDP destination for outbound OSC events and state. |
| `--osc-state-rate <hz>` | `50` | Rate cap for the OSC state stream. |
| `--no-osc-state` | | Disable the OSC state stream (events still flow). |

### Output modes

**`one-channel-per-chain`** (default). The whole chain is a single monophonic voice on channel 1. The four output sites each have their own pitch (C3, E3, G3, B3 by default).

**`channel-per-site`**. Each output site gets its own MIDI channel (1, 2, 3, 4).

In both modes, wall voices occupy their configured channel range (default 5-8) and the clock occupies its configured channel (default 16).

---

## Chain clock

The program watches the mean magnetization of the chain and emits a gate pulse every time it crosses zero. In the locked phase this fires at a stable rate; near a phase boundary the pulse rate becomes irregular; in the thermal phase it stops. Feed this into a clock divider and it becomes the master clock for downstream sequencers and oscillators.

---

## Domain walls

A domain wall is the boundary between adjacent sites with opposite spin direction. Walls exist as objects: they are born in pairs, drift along the chain, and annihilate in pairs. Their lifetime ranges from a few ticks (transient in the locked phase) to many drive periods (persistent walls in the partially-disordered regime).

Each wall sounds as a held note on one of the wall channels, with note-on at birth and note-off at annihilation. Wall motion is reported in one of two ways:

- **Held pitch + CC** (default). The note holds a single pitch; a CC stream tracks the wall's current position. Patch this CC to filter cutoff, pan, or any other modulation destination on the receiving synth.
- **Repitch on move** (`--wall-repitch-on-move`). Wall motion produces new note-on/note-off pairs as the position-derived pitch crosses semitone boundaries. Useful for hardware MIDI-to-CV interfaces where CC-to-modulation routing is inconvenient.

When the number of active walls exceeds the available channels, the oldest active wall yields its channel to the new one.

---

## Physics parameters

These are not exposed on the CLI and live in `src/config.rs` as defaults on `PhysicsConfig`.

| Parameter | Default | Effect |
|---|---|---|
| `n_sites` | `8` | Chain length. |
| `eps` | `0.01` | Drive imperfection. Closer to 0 = tighter lock. |
| `j` | `1.2` | Coupling strength between neighbors. |
| `w` | `2.0` | Disorder width. Stabilizes the locked phase against thermal noise. |
| `kt` | `0.1` | Effective temperature. Higher values eventually break the phase. |
| `ticks_per_period` | `25` | Integration steps per drive period. |

---

## Live control via OSC

The OSC layer turns the program into a system that can be shaped in real time. An external program (TouchDesigner, a Python script, a custom controller) can write to the four mutable physics parameters while the simulation runs; the program publishes its internal state and events back over a second port.

Both directions are opt-in. Without `--osc-listen` and `--osc-send`, no OSC threads start and the behavior is identical to running without the flags.

### Inbound parameter control

When `--osc-listen <port>` is set, the program accepts messages on four addresses:

```
/physics/kt   float    effective temperature,  0.0 - 2.0
/physics/eps  float    drive imperfection,     0.0 - 0.5
/physics/j    float    coupling strength,      0.0 - 3.0
/physics/w    float    disorder width,         0.0 - 5.0
```

Out-of-range values are clamped silently. Parameter changes are smoothed over several seconds so that sudden input produces a gradual transition rather than an audible click.

### Outbound events and state

When `--osc-send <host:port>` is set, the program publishes two kinds of traffic.

**Events** fire once per occurrence:

```
/site/event      int site, int voice, float intensity
/clock/pulse     float magnetization
/wall/created    int id, float position, int channel
/wall/destroyed  int id, float last_position, int lifetime_ticks
/wall/moved      int id, float from, float to, float velocity
```

**State** is sampled at up to the configured rate (default 50 Hz):

```
/state/spins           n_sites floats    per-site sigma_z values
/state/magnetization   float             mean sigma_z
/state/wall_count      int               number of active walls
```

Events and state that fire on the same tick are packed into a single OSC bundle and sent as one UDP packet.

### Typical invocation

```sh
cargo run --release -- --port 1 \
    --osc-listen 9000 \
    --osc-send 127.0.0.1:9001
```

The program listens for parameter writes on port 9000 and publishes to port 9001.

---

## OSC message reference

**Inbound** (`--osc-listen` port):

| Address | Arguments |
|---|---|
| `/physics/kt` | float |
| `/physics/eps` | float |
| `/physics/j` | float |
| `/physics/w` | float |

**Outbound events** (`--osc-send` destination):

| Address | Arguments |
|---|---|
| `/site/event` | int site, int voice, float intensity |
| `/clock/pulse` | float magnetization |
| `/wall/created` | int id, float position, int channel |
| `/wall/destroyed` | int id, float last_position, int lifetime_ticks |
| `/wall/moved` | int id, float from, float to, float velocity |

**Outbound state** (throttled):

| Address | Arguments |
|---|---|
| `/state/spins` | n_sites x float |
| `/state/magnetization` | float |
| `/state/wall_count` | int |

---

## License

MIT. See [`LICENSE`](LICENSE).
