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

All routing, tempo, physics, and OSC settings live in [`config.toml`](config.toml) in the project root. The included default file reproduces the previous program defaults exactly — edit it to change anything.

---

## Command-line flags

| Flag | Default | Description |
|---|---|---|
| `--list-ports` | | Print available MIDI output ports and exit. |
| `--port <N>` | `0` | Output port index. |
| `--config <path>` | `config.toml` | Path to the TOML config file. |
| `--periods <N>` | `20000` | Number of drive periods to run. |

Everything else — tempo, RNG seed, physics parameters, MIDI routing, OSC ports — is set in the config file.

---

## The config file

`config.toml` is loaded from the project root by default. The shape:

```toml
[tempo]
bpm = 120

[osc]                                # optional
listen_port = 9000
send_address = "127.0.0.1:9001"

[physics]                            # optional shared block
kt = 0.1
eps = 0.01
j = 1.2
w = 2.0
n_sites = 8
ticks_per_period = 25

[chain_a]
seed = 47

[chain_a.gates]
voice_0 = { channel = 1, pitch = 48 }
voice_2 = { channel = 1, pitch = 52 }
voice_4 = { channel = 1, pitch = 55 }
voice_6 = { channel = 1, pitch = 59 }
gate_length_ms = 50

[chain_a.walls]
voice_0 = 5
voice_1 = 6
voice_2 = 7
voice_3 = 8
pitch_low = 36
pitch_high = 84
motion_cc = 1
repitch_on_move = false

[chain_a.clock]
channel = 16
```

Channel numbers in the file are 1-based (`1..=16`); MIDI pitches are `0..=127`.

**Gate voices.** Each `voice_N` entry means "this voice listens to chain site `N`". The shorthand `voice_0 = 1` sets channel 1 with the default pitch; the full form `voice_0 = { channel = 1, pitch = 48 }` sets both. Voices that share a channel are mono — a new note retires the previous one. Voices on distinct channels are polyphonic.

**Wall voices.** A named pool of channels. The allocator picks the next free channel round-robin when a wall is created; when the pool is full, the oldest active wall yields its channel. Delete `[chain_a.walls]` to disable wall output.

**Clock.** A gate pulse on the configured channel every time the chain's mean magnetization crosses zero. In the locked phase this fires at a stable rate; near a phase boundary it becomes irregular; in the thermal phase it stops.

**Physics.** A shared `[physics]` block applies to every chain that doesn't define its own `[chain_a.physics]` (or `[chain_b.physics]` once that exists). Per-chain blocks win when both are present.

The loader validates the file before the program starts:

- Channel numbers must be in 1..=16.
- No channel can be claimed by more than one signal across gates, walls, and clock.
- `voice_N` indices must correspond to real chain sites (`N < n_sites`).
- Pitch and CC numbers must be in range.

Errors name the offending entry exactly — e.g.

```
config error: channel 16 is assigned to both chain_a.gates.voice_3 and chain_a.clock
```

---

## Chain clock

The program watches the mean magnetization of the chain and emits a gate pulse every time it crosses zero. In the locked phase this fires at a stable rate; near a phase boundary the pulse rate becomes irregular; in the thermal phase it stops. Feed this into a clock divider and it becomes the master clock for downstream sequencers and oscillators.

---

## Domain walls

A domain wall is the boundary between adjacent sites with opposite spin direction. Walls exist as objects: they are born in pairs, drift along the chain, and annihilate in pairs. Their lifetime ranges from a few ticks (transient in the locked phase) to many drive periods (persistent walls in the partially-disordered regime).

Each wall sounds as a held note on one of the wall channels, with note-on at birth and note-off at annihilation. Wall motion is reported in one of two ways, controlled by `repitch_on_move` in the config:

- **Held pitch + CC** (default, `repitch_on_move = false`). The note holds a single pitch; a CC stream tracks the wall's current position. Patch this CC to filter cutoff, pan, or any other modulation destination on the receiving synth.
- **Repitch on move** (`repitch_on_move = true`). Wall motion produces new note-on/note-off pairs as the position-derived pitch crosses semitone boundaries. Useful for hardware MIDI-to-CV interfaces where CC-to-modulation routing is inconvenient.

When the number of active walls exceeds the available channels, the oldest active wall yields its channel to the new one.

---

## Live control via OSC

The OSC layer turns the program into a system that can be shaped in real time. An external program (TouchDesigner, a Python script, a custom controller) can write to the four mutable physics parameters while the simulation runs; the program publishes its internal state and events back over a second port.

Both directions are opt-in. Without `listen_port` and `send_address` in the `[osc]` section, no OSC threads start and the behavior is identical to running without them.

### Inbound parameter control

When `listen_port` is set, the program accepts messages on four addresses:

```
/physics/kt   float    effective temperature,  0.0 - 2.0
/physics/eps  float    drive imperfection,     0.0 - 0.5
/physics/j    float    coupling strength,      0.0 - 3.0
/physics/w    float    disorder width,         0.0 - 5.0
```

Out-of-range values are clamped silently. Parameter changes are smoothed over several seconds so that sudden input produces a gradual transition rather than an audible click.

### Outbound events and state

When `send_address` is set, the program publishes two kinds of traffic.

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

```toml
[osc]
listen_port = 9000
send_address = "127.0.0.1:9001"
```

```sh
cargo run --release -- --port 1
```

The program listens for parameter writes on port 9000 and publishes to port 9001.

---

## OSC message reference

**Inbound** (`listen_port`):

| Address | Arguments |
|---|---|
| `/physics/kt` | float |
| `/physics/eps` | float |
| `/physics/j` | float |
| `/physics/w` | float |

**Outbound events** (`send_address`):

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