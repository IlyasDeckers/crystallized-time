# MIDI Routing — TOML Configuration

*Companion specification to `stage_1_2_spec.md`, `stage_2-5_spec.md`, `domain_walls_spec.md`, and `OSC_implementation_spec.md`. Implementation spec.*

`Status: Finished.`

---

## Purpose

Replace the current CLI flag routing with a TOML configuration file that gives complete per-voice control over MIDI channel assignments. The CLI flags for routing are removed once this is implemented; the config file is the single source of truth for all MIDI assignments.

The design needs to accommodate two chains worth of signals without requiring changes when chain B arrives. It also needs to handle wall voices, whose population is dynamic, in a way that's consistent with the per-voice philosophy. Two options for wall voice routing are specified below; they are mutually exclusive and a choice between them is required before implementation begins.

---

## What needs to be routable

For each chain, the routable signals are:

- **Gate voices.** One per output site (four by default). Each fires gate events independently.
- **Wall voices.** A pool of voices assigned to domain walls as they are created. Wall population is dynamic; the number of active walls at any moment is not fixed.
- **Clock.** One channel per chain.

With two chains this is a maximum of 16 channels in use simultaneously, though in practice far fewer will be active at any given time.

---

## Config file structure

A single TOML file is passed at startup via `--config <path>`. The file contains all routing, physics, tempo, and OSC configuration. CLI flags for any of these are removed once the file is in use.

One flag remains: `--config <path>`. A default path (`crystallized_time.toml` in the working directory) is tried if the flag is absent, so the program can be run without any flags in a directory that contains a config file.

### Tempo, seed, OSC

```toml
[tempo]
bpm = 120

[osc]
listen_port = 9000
send_address = "127.0.0.1:9001"
state_rate_hz = 50
enable_state = true
```

Seeds are per-chain:

```toml
[chain_a]
seed = 47

[chain_b]
seed = 83
```

### Physics config -- two options

**Option A: shared physics.** One `[physics]` block applies to both chains.

```toml
[physics]
kt = 0.1
eps = 0.01
j = 1.2
w = 2.0
n_sites = 8
ticks_per_period = 25
```

**Option B: per-chain physics.** Each chain has its own block. This is the configuration that enables two chains to run in different phases simultaneously, which is the precondition for polyrhythmic content (chain A locked at f/2, chain B at f/3).

```toml
[chain_a.physics]
kt = 0.1
eps = 0.01
j = 1.2
w = 2.0
n_sites = 8
ticks_per_period = 25

[chain_b.physics]
kt = 0.1
eps = 0.01
j = 1.4
w = 2.5
n_sites = 8
ticks_per_period = 25
```

Both options are valid TOML and both are straightforward to deserialize. Option B is strictly more expressive; Option A is a convenience for sessions where both chains run identical physics. The implementation can support both by making the per-chain blocks optional and falling back to a shared `[physics]` block when they are absent.

OSC parameter control via `/physics/kt` etc. would need a target chain when per-chain physics is active. The OSC address scheme for that is outside the scope of this spec and can be resolved when chain B is implemented.

---

## Wall voice routing -- two options

This is the decision point. Both options are specified fully below.

---

### Option 1: Fixed named wall voices

Wall voices are assigned explicit channel numbers in the config, the same way gate voices are. The allocator treats them as a fixed pool: voice_0 through voice_N, each bound to a specific channel. When a wall is created it takes the next free voice from the pool. Voice stealing (oldest-active) applies when the pool is exhausted.

```toml
[chain_a.walls]
voice_0 = 5
voice_1 = 6
voice_2 = 7
voice_3 = 8
```

**What this gives you.** Complete determinism about which MIDI channels walls can ever appear on. Your DAW or hardware routing is fully predictable: channel 5 is always a wall voice, never anything else. Patches built around specific channels are stable across sessions.

**What this costs.** The pool size is fixed at config time. If you want five simultaneous wall voices you add a fifth entry; if you want two you remove entries. A wall born when all four voices are active steals the oldest -- the same behavior as today, just with explicitly named channels instead of a range.

**Full example, two chains:**

```toml
[chain_a.gates]
voice_0 = 1
voice_1 = 2
voice_2 = 3
voice_3 = 4

[chain_a.walls]
voice_0 = 5
voice_1 = 6
voice_2 = 7
voice_3 = 8

[chain_a.clock]
channel = 16

[chain_b.gates]
voice_0 = 9
voice_1 = 10
voice_2 = 11
voice_3 = 12

[chain_b.walls]
voice_0 = 13
voice_1 = 14
voice_2 = 15
voice_3 = 16

[chain_b.clock]
channel = 15
```

Note that chain A clock and chain B wall voice_3 both landing on channel 16 is a config error; the validator catches this (see Validation below).

---

### Option 2: Named wall pool

Wall voices are assigned as a named list of channels rather than individually numbered voices. The allocator draws from the pool in round-robin order; individual voices within the pool are not named or distinguished.

```toml
[chain_a.walls]
channels = [5, 6, 7, 8]
```

**What this gives you.** Flexibility to size the pool without worrying about voice numbering. Adding a channel is one number appended to the list. The pool can be any size, including a single channel (one wall voice at a time) or all available channels.

**What this costs.** Less predictability about which specific channel a given wall will sound on at any moment -- the allocator picks the next free one from the list. This matters if your patch treats individual channels differently (different filters, different effects chains). For patches where all wall channels feed the same destination, it makes no difference.

**Full example, two chains:**

```toml
[chain_a.gates]
voice_0 = 1
voice_1 = 2
voice_2 = 3
voice_3 = 4

[chain_a.walls]
channels = [5, 6, 7, 8]

[chain_a.clock]
channel = 16

[chain_b.gates]
voice_0 = 9
voice_1 = 10
voice_2 = 11
voice_3 = 12

[chain_b.walls]
channels = [13, 14, 15]

[chain_b.clock]
channel = 15
```

---

### Comparison

| | Option 1: Fixed named voices | Option 2: Named pool |
|---|---|---|
| Per-channel patch routing | Full control | Round-robin, less predictable |
| Pool resizing | Add/remove numbered entries | Append/remove from list |
| Config verbosity | Higher | Lower |
| Consistency with gate routing | Same pattern | Different pattern |
| Voice stealing behavior | Oldest named voice | Oldest in pool |

Option 1 is more consistent with the gate voice routing pattern and gives the most control for hardware rigs where individual channels feed distinct CV destinations. Option 2 is simpler to configure and more appropriate when wall channels all feed the same destination.

---

## Gate voice pitch assignment

Pitch per gate voice is also routable in the config, keeping it alongside the channel assignment:

```toml
[chain_a.gates]
voice_0 = { channel = 1, pitch = 48 }   # C3
voice_1 = { channel = 2, pitch = 52 }   # E3
voice_2 = { channel = 3, pitch = 55 }   # G3
voice_3 = { channel = 4, pitch = 59 }   # B3
```

When pitch is omitted the default (C3, MIDI 48) is used, matching the current behavior.

---

## Wall voice pitch range

Wall pitch mapping (position-to-pitch) is per-chain, not per-voice:

```toml
[chain_a.walls]
# Option 1 style
voice_0 = 5
voice_1 = 6
voice_2 = 7
voice_3 = 8
pitch_low = 36
pitch_high = 84
motion_cc = 1
repitch_on_move = false

# Option 2 style
channels = [5, 6, 7, 8]
pitch_low = 36
pitch_high = 84
motion_cc = 1
repitch_on_move = false
```

---

## Validation

The config loader validates the file before the program starts and exits with a clear error message on any of the following:

- A MIDI channel number outside 1-16.
- The same channel assigned to more than one signal (across gates, walls, and clock, across both chains).
- A gate voice index that doesn't correspond to an output site (e.g. voice_4 when n_sites = 8 gives only four output sites).
- A missing required field (clock channel, at least one gate voice).

Duplicate channel assignments are the most likely config error in practice. The error message names both signals that claim the channel:

```
Error: channel 16 is assigned to both chain_a.clock and chain_b.walls.voice_3
```

---

## Default config file

A default `crystallized_time.toml` is included in the repository. It reproduces the current program defaults exactly, so existing users can adopt the file with no change in behavior and then edit from there.

---

## Migration from CLI flags

The CLI flags being replaced:

```
--bpm, --seed, --mode, --clock-channel, --no-clock, --no-walls,
--wall-channels, --wall-pitch-range, --wall-motion-cc,
--wall-repitch-on-move, --osc-listen, --osc-send,
--osc-state-rate, --no-osc-state
```

These are removed from `cli.rs` once the config file is in use. The flags that remain:

```
--config <path>    Path to the TOML config file.
--list-ports       Print available MIDI output ports and exit.
--port <N>         Which MIDI output port to open.
--periods <N>      Number of drive periods to run.
```

`--port` stays on the CLI because it's session-specific (which physical device to open) rather than compositional config, and varies between machines. It could move to the config file in a later pass if that turns out to be more convenient.

---

## Module layout

A new module `src/config_file.rs` owns:

- The TOML-deserializable structs (`TomlConfig`, `ChainRouting`, `GateVoiceConfig`, `WallRoutingConfig`, `ClockConfig`, etc.).
- A `load(path: &Path) -> Result<TomlConfig, ConfigError>` function that reads, parses, and validates the file.
- A `TomlConfig::into_config() -> Config` conversion that produces the existing `Config` struct the rest of the program uses.

The existing `Config` struct and its sub-types are unchanged. The TOML layer is a new deserialization front-end that produces the same types the program already knows how to use. This keeps the change contained: `main.rs` calls `config_file::load()` instead of `Config::from(&cli)`, and everything downstream is unaffected.

---

## Definition of done

1. A TOML file at the path given by `--config` (or `crystallized_time.toml` by default) is loaded at startup and fully determines MIDI routing, physics, tempo, and OSC config.
2. All routing CLI flags are removed.
3. Per-voice channel assignment for gate voices works: each of the four output sites can be independently assigned to any MIDI channel 1-16.
4. Wall voice routing works under whichever option is chosen (fixed named voices or named pool).
5. The config validator catches duplicate channel assignments and missing required fields, and exits with a message that names the conflicting entries.
6. A default `crystallized_time.toml` is included that reproduces current behavior exactly.
7. The program starts and behaves identically to the pre-config-file version when given the default config file.

---

*Companion spec for Crystallized Time | MIDI Routing | TOML configuration*