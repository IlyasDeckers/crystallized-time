# OSC and Live Parameter Mutability

*Companion specification to `stage_1_2_spec.md`, `stage_2-5_spec.md`, `domain_walls_spec.md`, and `stage_3_plus_reference.md`. Implementation spec — replaces the framework's lighter Stage 4 sketch with a TouchDesigner-targeted version using OSC as the sole external control protocol.*

`Status: Done.`

---

## Purpose

The substrate runs with fixed parameters set at startup. This spec adds two intertwined capabilities:

1. **Live parameter mutability.** The physics parameters `kt`, `eps`, `j`, `w` become mutable while the simulation is running, with per-parameter smoothing so changes feel like influence rather than control.
2. **Bidirectional OSC.** TouchDesigner sends parameter targets to the substrate; the substrate sends events and state back to TouchDesigner. OSC is the sole external control protocol for parameter mutation.

Together these turn the substrate from a deterministic generator into a system that can be played in real time, while preserving its determinism for any session where the OSC layer is disabled. They are also the precondition for everything downstream: the framework document's §5 visitor-perturbation work, the eventual sensor input that drives the installation, and the TouchDesigner-based visual layer.

---

## Why each piece is here

**Live mutability of physics parameters, not chain topology or RNG state.** The four mutable parameters are the ones that change *how the chain behaves* without changing what the chain *is*. Changing `n_sites` or `ticks_per_period` mid-run would require rebuilding the chain. Changing the seed or the initial spin configuration would discard the chain's accumulated state. The four physics parameters can be mutated freely; the chain absorbs the change as a perturbation, exactly as the framework's §3.1 describes the time-crystal phase doing for any perturbation it survives.

**Smoothing on every parameter, on the simulation thread.** Without smoothing, dragging `kt` from 0.1 to 0.5 produces an audible glitch as the chain instantly thermalizes — not musical, not interesting. With smoothing over seconds, the chain *transitions* into the thermal phase, audibly. The transition is the interesting part; smoothing is what makes it audible. Smoothing belongs on the simulation thread because the time domain that matters is the simulation's, not the network's or TouchDesigner's frame rate.

**`ArcSwap<PhysicsConfig>` for the config handoff.** The simulation thread reads the config every tick (50Hz at defaults, higher under closer-to-realtime configurations). The OSC receiver thread writes targets at TouchDesigner's update rate (typically 30–60Hz, occasionally higher). A `Mutex<PhysicsConfig>` works in principle but introduces a lock on the tick loop's hot path; the lock is uncontested almost always, but occasional contention with OSC writes produces tick-timing jitter. `ArcSwap` gives lock-free reads on the simulation thread and a cheap atomic pointer swap on writes. Per-scalar atomics are overkill for four floats and make extending the parameter list verbose.

**OSC over UDP localhost.** TouchDesigner has first-class OSC support — no plugins, no custom Python. UDP's lower overhead and loss-tolerance both suit parameter-control traffic where a missed packet is replaced by the next one ~16ms later. OSC's structured message format with named addresses makes the protocol self-documenting in a way raw UDP doesn't.

**Inbound and outbound use separate ports.** Bidirectional OSC over a single port is technically possible but fragile — the two flows have different sender behaviors (TD sends from many sources simultaneously; the substrate sends from one) and conflating them complicates routing on both ends. Two ports keeps the model clean.

**Outbound events vs state on the same socket but different addresses.** Events (`/wall/created`, `/site/event`, `/clock/pulse`) and state (`/state/spins`, `/state/magnetization`) have different cadences but the same destination. One UDP socket handles both; TouchDesigner's OSC In routing splits them by address.

**Outbound events bundled per tick.** OSC bundles carry multiple messages with a shared timestamp. Per simulation tick the substrate may emit several events (a wall created, a site fired, the clock pulsed). Sending these as separate UDP packets multiplies socket overhead and complicates TouchDesigner's processing. A single bundle per tick has constant per-tick overhead, regardless of how many events fired.

**Default off.** OSC support is additive. The CLI without OSC flags behaves exactly as today. No regression, no surprise change in behavior for existing usage.

---

## The mutability layer

### State

`PhysicsConfig` becomes a *snapshot* of the chain's current physics parameters, not a fixed initial setting. The chain reads the latest snapshot each tick.

Two new concepts alongside it:

**`PhysicsTargets`** — the values the OSC layer (or any future input source) wants the chain to reach. Each parameter has a target value. Targets are written by the OSC receiver thread, read by the simulation thread.

**`SmoothingConfig`** — per-parameter time constants. Held by the simulation thread, applied when computing the snapshot from the targets.

The simulation thread's per-tick parameter update:

```text
for each smoothed parameter p:
    snapshot.p = snapshot.p + (target.p - snapshot.p) * alpha_p
```

where `alpha_p = 1 - exp(-dt_real / tau_p)`, with `dt_real` being the wall-clock duration of one tick (drive period / ticks_per_period) and `tau_p` being the time constant for parameter p in seconds.

This gives an exponential approach to the target value, with `tau_p` as the time it takes to cover ~63% of the remaining gap. After `3 * tau_p` seconds the value is essentially at target.

### Smoothing defaults

The four mutable parameters have distinct musical character and warrant distinct defaults:

| Parameter | Time constant | Rationale |
|---|---|---|
| `kt` | 1.5 s | Thermalization is structural; should feel like a regime shift, not a click. |
| `eps` | 1.0 s | Lock tightness; faster than kt, slower than instant. |
| `j` | 2.0 s | Structural; coupling changes shift phase boundaries, want them slow. |
| `w` | 2.0 s | Structural; same logic. |

These are starting points and are exposed in `SmoothingConfig` so they can be tuned. They are not exposed on the CLI — they are not the kind of parameter one tunes per-run; they are the kind one finds by listening and then commits to.

### Concurrency model

```text
OSC receiver thread:
    on each /physics/<param> message:
        update PhysicsTargets via atomic write

Simulation thread (existing tick loop):
    each tick:
        read PhysicsTargets snapshot
        compute new PhysicsConfig snapshot via per-parameter smoothing
        ArcSwap::store(Arc::new(new_snapshot))
        chain.step() — reads ArcSwap::load() once
        ...
```

The `ArcSwap` serves two purposes:
1. The chain reads the latest config snapshot cheaply (one atomic pointer load).
2. Future readers (event detector, wall detector) read the same snapshot consistently — they can call `ArcSwap::load()` and see the same view the chain saw on that tick.

`PhysicsTargets` is a plain struct behind `Arc<[AtomicU64; 4]>` (four parameters as bit-pattern-cast f64s using `AtomicU64`), or alternatively `RwLock<PhysicsTargets>`. The AtomicU64 approach is lock-free but requires bit-cast helpers; the RwLock is simpler and sufficient since target reads happen once per tick. **Use the RwLock.** Simpler, no measurable cost at this scale, fewer places where a future change can introduce a bug.

---

## The OSC layer

### Ports and CLI

Two CLI flags:

```
--osc-listen <port>
    UDP port to listen for inbound parameter messages.
    Default: not set; OSC inbound disabled.

--osc-send <host:port>
    UDP address to send outbound events and state.
    Default: not set; OSC outbound disabled.
```

Either can be set independently. Sending state without listening for parameters is a valid configuration (monitoring without control). Listening without sending is also valid (control without visualization data).

When `--osc-listen` is set, an OSC receiver thread starts. When `--osc-send` is set, an OSC sender thread starts. When both are absent, the substrate behaves exactly as it does today.

### Inbound message schema

Five message addresses; all carry one float argument unless noted.

```
/physics/kt    float    target value for kt
/physics/eps   float    target value for eps
/physics/j     float    target value for j
/physics/w     float    target value for w
```

A future extension may add `/perturb/flip_site <int>` for direct chain manipulation, but that is not in scope for this spec.

Values outside reasonable ranges (negative `kt`, `eps` outside `[0, 1]`, etc.) are clamped on receipt to per-parameter min/max bounds defined in `PhysicsTargets`. The bounds are not user-configurable; they reflect physically meaningful ranges:

| Parameter | Min | Max |
|---|---|---|
| `kt` | 0.0 | 2.0 |
| `eps` | 0.0 | 0.5 |
| `j` | 0.0 | 3.0 |
| `w` | 0.0 | 5.0 |

Clamping is silent — no error message, no logged warning. TouchDesigner controllers may produce values outside these ranges as a normal part of operation, and logging every clamp would flood stderr.

Malformed messages (wrong types, wrong argument counts) are dropped silently. The receiver thread does not propagate parse errors; it logs them at startup as a one-time "OSC bound to port N" message and is otherwise silent.

### Outbound message schema

Three categories: events, state, and clock.

**Events** — fired once per occurrence, on the tick the event happens:

```
/wall/created    int(id)  float(position)  int(channel)
/wall/destroyed  int(id)  float(last_position)  int(lifetime_ticks)
/wall/moved      int(id)  float(from)  float(to)  float(velocity)
/site/event      int(site_index)  int(voice_index)  float(intensity)
/clock/pulse     float(magnetization)
```

The `channel` argument on `/wall/created` is the MIDI channel the wall is sounding on (0–15) — useful for visualizations that want to color-code walls by channel. If voice stealing occurs, no extra message is sent; the receiver sees a `/wall/destroyed` for the old wall and a `/wall/created` for the new one with the same channel.

**State** — sent at a configurable rate (default 50Hz), on the simulation thread's pacing:

```
/state/spins           [float; n_sites]    z-components of all spins
/state/magnetization   float                mean magnetization
/state/wall_count      int                  number of active walls
```

State messages are sent every tick by default, throttled to a maximum rate of 50Hz. At default `ticks_per_period = 25` and 120 BPM, 25 ticks per 0.5 second = 50 ticks per second, so every tick goes out. At higher tick rates the sender throttles to 50Hz by skipping ticks; at lower tick rates every tick is sent.

The throttle rate is a config field (`OscConfig::state_rate_hz`), not a CLI flag. Most users will not need to tune it.

**Bundling.** Per tick, all events emitted on that tick plus any state messages due on that tick are packed into a single OSC bundle and sent as one UDP packet. The bundle's timestamp is the OSC "immediate" sentinel — TouchDesigner does not need timestamp scheduling for this use case, and computing accurate NTP timestamps adds complexity without benefit.

### Concurrency model (outbound)

```text
Simulation thread:
    each tick:
        collect events and (optionally) state into a per-tick bundle staging buffer
        send the buffer via an MPSC channel to the OSC sender thread
        (channel is bounded; if full, drop the bundle — visualization data is non-critical)

OSC sender thread:
    receive bundles from channel
    serialize each bundle to OSC wire format
    send via UDP socket
```

The sender thread exists to keep socket-send latency off the simulation thread. UDP localhost sends are fast (single-digit microseconds typically), but pathological cases (kernel buffer full, network stack hiccup) can occasionally take milliseconds. The simulation thread should never block on the network.

The MPSC channel is bounded with a small capacity (say 16 bundles, ~320ms of buffering). If the sender thread can't keep up — extremely unlikely under normal conditions — bundles are dropped at the simulation-thread side rather than memory growing without bound. A dropped state message is harmless; a dropped event message is a minor visual glitch that the visualization layer should tolerate. Both are far less harmful than the simulation stalling.

### Concurrency model (inbound)

```text
OSC receiver thread:
    bind UDP socket to --osc-listen port
    loop:
        recv_from socket
        parse OSC packet (bundle or message)
        for each /physics/<param> message:
            clamp to bounds
            write to PhysicsTargets via RwLock::write()
        ignore everything else
```

The receiver thread blocks on `recv_from`. It has no shutdown signal yet — when the program exits, the thread is terminated by process exit. A clean shutdown sequence for OSC would require a non-blocking socket and a poll loop, which is more complex than warranted for this version. The thread is a daemon thread (spawned with `thread::spawn`, not joined on exit).

---

## Module layout

Two new modules:

**`src/osc.rs`** — OSC types, message schema, serialization helpers. Pure data and conversion code; no I/O.

**`src/osc_io.rs`** (or split into `osc_recv.rs` and `osc_send.rs`) — the receiver and sender threads. Owns the UDP sockets. Uses `osc.rs` for message handling.

A new struct in `config.rs`:

```rust
pub struct OscConfig {
    pub listen_port: Option<u16>,
    pub send_addr: Option<String>,
    pub state_rate_hz: f64,
    pub enable_state: bool,
}
```

The `enable_state` flag exists because some users may want only events (for visualization that's purely event-driven) and not state streams (which are bandwidth-heavy at high tick rates). Defaults to `true`.

A new struct (`SmoothingConfig`) holding the per-parameter time constants. Held by the simulation; not exposed on the CLI; tunable by editing defaults.

The existing `PhysicsConfig` does not change shape — only how it is owned. The chain takes an `Arc<ArcSwap<PhysicsConfig>>` instead of a `PhysicsConfig`. The chain's `step` method calls `self.config.load()` once at the top to get a snapshot for the entire step.

`main.rs` orchestrates: creates the `ArcSwap`, creates `PhysicsTargets` and `SmoothingConfig`, wires the OSC threads if configured, runs the main loop with per-tick smoothing.

---

## CLI additions

```
--osc-listen <port>
    Port for inbound OSC. Disabled if not set.

--osc-send <host:port>
    Destination for outbound OSC. Disabled if not set.

--osc-state-rate <hz>
    Rate for state messages. Default: 50.

--no-osc-state
    Disable state messages (events only). Useful for bandwidth-sensitive setups.
```

All four are independent. Typical TouchDesigner use case:

```
--osc-listen 9000 --osc-send 127.0.0.1:9001
```

OSC events are disabled in the inbound direction; OSC inbound only listens for physics parameter messages. Future per-event inbound messages (e.g., explicit "flip this site" perturbations) can be added without breaking the schema.

---

## Worked example: parameter ramp from outside

A user in TouchDesigner has a slider for `kt`. They drag it from 0.1 to 0.5 over 200ms (a few frames of TD).

Sequence of events:

1. TouchDesigner sends, over the 200ms, perhaps 10 OSC messages: `/physics/kt 0.12`, `/physics/kt 0.18`, `/physics/kt 0.25`, ..., `/physics/kt 0.5`. Each is a UDP packet to port 9000.
2. The substrate's OSC receiver thread parses each message, clamps to `[0.0, 2.0]` (no clamping needed here, all in range), and writes to `PhysicsTargets.kt` via the RwLock. After 200ms the target is 0.5.
3. The simulation thread, running at 50 ticks/second, reads the target each tick. At tick T0 (start of the drag), target = 0.1, snapshot.kt = 0.1, no change. At tick T0 + 10 (200ms in), target = 0.5, snapshot.kt has moved partway, perhaps 0.15. At tick T0 + 75 (1.5 seconds in, which is `tau_kt`), snapshot.kt has covered ~63% of the gap → ~0.35. At tick T0 + 225 (4.5 seconds, 3 * tau_kt), snapshot.kt is essentially at 0.5.
4. Over those 4.5 seconds, the chain's behavior shifts continuously. The user dragged the slider in 200ms but *hears* the change unfold over several seconds — a regime transition, not a click.

The same principle applies in reverse if the user pulls `kt` back down. The substrate moves toward the new target with the same time constant.

If the user oscillates the slider rapidly, `PhysicsTargets.kt` changes rapidly but `snapshot.kt` only moves at the smoothing rate — fast inputs are filtered to the smoothing's bandwidth. This is desired behavior: noisy or jittery inputs (e.g. sensor data with fluctuations) become smooth substrate responses.

---

## Definition of done

1. Two new threads (OSC receiver, OSC sender) start when the corresponding CLI flags are set, and remain dormant when not set.
2. With `--osc-listen` set, sending `/physics/kt 0.5` from TouchDesigner causes the substrate's effective `kt` to ramp toward 0.5 over the configured time constant. Audible: pushing `kt` up over time produces audible thermalization; pulling it down restores the time-crystal phase.
3. With `--osc-send` set, TouchDesigner's OSC In CHOP receives `/state/magnetization`, `/state/spins`, and `/state/wall_count` at ~50Hz. The OSC In DAT receives event messages (`/wall/created`, `/site/event`, `/clock/pulse`, etc.) immediately when they fire in the simulation.
4. Per-tick events are bundled into single OSC packets; TouchDesigner receives the bundle and processes individual messages from it.
5. The substrate's MIDI output and overall musical behavior with no OSC flags set is byte-identical to the pre-OSC version. The OSC layer is genuinely additive.
6. Out-of-range parameter values from OSC are silently clamped to bounds; malformed OSC packets are silently dropped. The substrate does not crash or log spam under any OSC input.
7. Disconnecting TouchDesigner mid-run does not affect the substrate. The substrate continues running with whatever parameter targets it last received, smoothing toward them indefinitely (and reaching them in finite time).
8. CtrlC / shutdown behavior is unchanged. OSC threads are terminated by process exit.

---

## Migration path

This spec is the canonical Stage 4 — the framework's "visitor perturbation" stage in its first concrete form. After this lands:

- **TouchDesigner-driven LFOs and patches.** TouchDesigner can run any LFO or modulation source it likes, generating OSC streams that drive the substrate's parameters. No further substrate work is needed for this; it's a pure TouchDesigner project.
- **Visualization on the substrate's state.** The `/state/spins` stream and event messages give TouchDesigner everything it needs to drive a visual layer. This is also pure TouchDesigner work after this spec.
- **Sensor input.** Webcams, ultrasonic sensors, biometric devices, all flow into TouchDesigner first and then to the substrate as OSC. The substrate doesn't know whether `/physics/kt 0.5` came from a slider, an LFO, or a person walking through a room. This is the framework's §5 made real.
- **Stage 5 (quantum substrate).** Unchanged. The OSC layer is substrate-agnostic; whatever provides the `n_sites` sigma_z values per tick feeds the same OSC sender.
- **Stage 6 (eigenmode sonification).** Unchanged. A new sonification layer produces new event types; the OSC sender already knows how to emit named messages from internal events.

The clean separation between "what the substrate is" and "what controls it" is the architectural payoff of this stage. After this lands, every future input modality is a TouchDesigner-side problem, not a substrate-side problem.

---

## What's intentionally not in scope

- **OSC inbound for perturbations beyond the four physics parameters.** Direct chain manipulation (`/perturb/flip_site/3`, etc.) is conceivable but deferred. The four parameters are sufficient for the framework's §5 work; perturbations are a refinement.
- **OSC inbound for output configuration.** Changing `--mode`, `--wall-channels`, etc. at runtime is not supported. These are session-level settings; restarting the program is fine.
- **Authentication or encryption.** This is localhost UDP. Adding TLS or token auth is unwarranted complexity for a development tool.
- **OSC over WebSocket or TCP.** UDP is the right transport for this use case. If TouchDesigner ever changes its OSC support such that UDP is no longer the default, revisit.
- **Discovery (Bonjour/Zeroconf).** The substrate doesn't advertise itself. The TouchDesigner side knows the substrate's port because the user typed it there.
- **Recording OSC sessions for replay.** Useful for debugging and for reproducing musical moments, but not needed for the first cut. Add later if the workflow demands it.
- **Throttling /state messages adaptively.** The fixed 50Hz default is enough. If TouchDesigner ever struggles with the load, drop to 30Hz via `--osc-state-rate`.
- **GUI for the substrate.** Explicitly out of scope. TouchDesigner is the GUI.

---

*Companion spec for Crystallized Time | OSC and Live Parameter Mutability | Implementation spec, next on the list*
