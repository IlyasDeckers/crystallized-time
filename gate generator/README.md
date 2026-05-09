# crystallized_time

Time-crystal-driven MIDI gate generator. Companion to *Crystallized Time*.

Stage 1–2: classical disordered spin chain, four output voices, real-time MIDI gates.
See `stage_1_2_spec.md` for the full specification.

## Build

```
cargo build --release
```

## Run

List available MIDI output ports:

```
cargo run --release -- --list-ports
```

Run with default parameters, sending to the first available output port:

```
cargo run --release
```

Specify a port by index, set tempo, set seed:

```
cargo run --release -- --port 1 --bpm 120 --seed 42
```

## Windows MIDI routing

`midir` on Windows uses WinMM. To route the output to a DAW for testing, install
[loopMIDI](https://www.tobias-erichsen.de/software/loopmidi.html) and create a
virtual port — it will appear in `--list-ports` on both ends.

For hardware MIDI-to-CV interfaces connected via USB, the device shows up in the
port list directly. No additional setup needed.

## Project layout

```
src/
  main.rs       CLI, top-level loop
  config.rs     parameters and defaults
  chain.rs      spin chain physics
  events.rs     zero-crossing detection, GateEvent type
  midi.rs       midir wrapper, note-on/off scheduling
```

`chain.rs` is the only module that knows about physics. When the substrate eventually
swaps from classical to quantum (quantrs2), only that module changes.
