# Crystallized Time

*An audiovisual installation in progress. This repository contains its first working component: a program that turns the dynamics of a simulated physical system into MIDI gates that drive a eurorack synthesizer.*

---

## What this project is

The eventual goal is an installation: a room where music and visuals are generated in real time by a simulated physical system, and where the people in the room are themselves part of that system. Their presence shapes what they hear and see.

The system being simulated is something physicists call a **discrete time crystal**. The full story is in [`crystallized_time.md`](crystallized_time.md), but the short version is this: it's a kind of matter that, when you push on it rhythmically, settles into a beat of its own, slower than the beat you're applying, and stable enough to keep going even when conditions change a little. It produces rhythm naturally, the way a vibrating string produces pitch naturally. That's the substrate. The music comes out of watching it move.

This repository is **not the installation**. It's the first piece of working software the installation will eventually be built on.

---

## How the music actually gets made

Three layers, top to bottom.

**The simulation.** The program runs a model of a chain of eight tiny magnets — "spins" — sitting in a row, each one influenced by its neighbors. Every half-second or so, the program gives the whole chain a synchronized kick. If the conditions are right (the right amount of disorder, the right strength of interaction, not too much heat), the chain falls into a rhythm: each spin flips up-down-up-down at half the rate of the kick. That's the time-crystal behavior. The rhythm is **emergent**. Nothing in the code says "flip every two kicks."

**The reading.** The simulation runs continuously, and the program watches four of the eight spins. Every time one of those spins crosses zero (going from up to down or down to up), that's an event, a moment in time where something musical could happen.

**The output.** Each event becomes a MIDI message: "play this note now, stop playing it 50 milliseconds later." Those messages go out a MIDI port to whatever's listening.

There's also a **clock**. On top of the four voices, the program watches the *average* of all the spins together. When that average crosses zero, the program sends out a clock pulse on its own MIDI channel. That pulse can drive the rest of the rig — sequencers, dividers, anything that wants a beat to follow. The point of this is that the clock comes from the simulation itself, not from a fixed metronome. When the simulated system is locked into its rhythm, the clock is steady. When the system is near a phase boundary or breaking down, the clock breathes or stops.

---

## Running the gate generator

Full instructions are in [`gate generator/README.md`](gate%20generator/README.md). The short version:

```sh
cd "gate generator"
cargo build --release
cargo run --release -- --list-ports     # see available MIDI outputs
cargo run --release -- --port 1         # run, sending to port 1
```

You need a MIDI destination. On Windows, install [loopMIDI](https://www.tobias-erichsen.de/software/loopmidi.html) and create a virtual port to route into a DAW. On macOS, the IAC driver does the same thing. For hardware, a USB MIDI-to-CV interface plugged in will show up directly.

---

## Configuration

All parameters have defaults that work. The CLI exposes the ones you'll most often want to change.

### Command-line flags

| Flag | Default | What it does |
|---|---|---|
| `--list-ports` | — | Print available MIDI output ports and exit. |
| `--port <N>` | `0` | Which output port to send to. |
| `--bpm <N>` | `120` | Tempo. One drive period = one beat. |
| `--seed <N>` | `47` | RNG seed for the simulation. Same seed → same run. |
| `--mode <name>` | `one-channel-per-chain` | Output topology. See below. |
| `--clock-channel <N>` | `16` | MIDI channel for the substrate clock (1–16). |
| `--no-clock` | — | Disable the substrate clock output. |

### Output modes

**`one-channel-per-chain`** (default). The whole chain plays as one monophonic voice on one MIDI channel. The four output sites each have their own pitch (C3, E3, G3, B3 by default — a Cmaj7 voicing).

**`channel-per-site`**. Each of the four output sites gets its own MIDI channel (1, 2, 3, 4).

### Substrate clock

Independent of the output mode, the program watches the *average magnetization* of the chain — the mean of all eight spin values — and emits a short gate pulse on the configured clock channel every time that average crosses zero. In the time-crystal phase this fires twice per crystal period; outside the phase the pulse rate degrades or stops. Feed this into a clock divider in your rig and it becomes the master clock for everything downstream.

### Physics parameters

Not exposed on the CLI yet — they live in `src/config.rs` as defaults on `PhysicsConfig`. The ones that matter:

| Parameter | Default | Effect |
|---|---|---|
| `n_sites` | `8` | Length of the chain. |
| `eps` | `0.01` | Drive imperfection. Closer to 0 → tighter lock to the time-crystal phase. |
| `j` | `1.2` | Coupling strength between neighbors. |
| `w` | `2.0` | Disorder width. Stabilizes the time-crystal phase against thermal breakup. |
| `kt` | `0.1` | Effective temperature. Higher → more thermal noise → eventually breaks the phase. |
| `ticks_per_period` | `25` | Integration steps per drive period. |

Editing these and rebuilding gives you a different substrate. Pushing `kt` up is the easiest way to hear the phase break down.

---

## License

MIT. See [`LICENSE`](LICENSE).
