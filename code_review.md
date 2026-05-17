# Crystallized Time — Code Review

A walk through the codebase against `specifications.md`, `README.md`, and the prose docs. Organized as: spec status (does what the spec claims is "done" actually match the code), then bugs and risks, then smaller cleanup items.

## Summary

The substrate, MIDI output, walls, OSC, TOML config, perturbations, and chain coupling are all present and structurally sound. The build looks like it should compile (I couldn't run `cargo` in this environment, so this is a reading of source plus a manual TOML parse of the shipped `config.toml`).

The main gaps relative to the spec are:

1. **The summed-σ_z modulation CC from Stage 2.5 is not implemented**, despite Stage 2.5 being marked `done`.
2. **SIGTERM is not actually handled** — the spec promises clean shutdown on SIGTERM but the `ctrlc` dependency is on its default features, which only catches SIGINT.
3. **Chain-B inter-chain coupling exists in code but the spec still marks it "planning"** — the spec's status markers are stale in both directions.
4. **The `--debug` CLI flag does nothing** but its own doc comment says it should fire a flip on site 4 at tick 2000.
5. **The bundled `config.toml` is set up with `coupling.strength = 0`** so the two configured chains run independently — fine as a starting point but worth noting if the intent was to exercise coupling.

The rest of the review is a more detailed list. Severity tags: **[bug]** for correctness issues, **[gap]** for missing-feature-vs-spec, **[doc]** for documentation drift, **[risk]** for fragile-but-working, **[cleanup]** for stylistic noise.

---

## 1. Spec-vs-code status

### Stage 1–2 — single-chain MIDI gates (`done`)

Matches the spec. `SpinChain` does Landau–Lifshitz + thermal noise + periodic Floquet kick. `EventDetector` produces gates on signed sz-crossings with debounce. Routing via `midir`. All the defaults the spec lists are present in `PhysicsConfig::default()` and `EventConfig::default()` — though note that the shipped `config.toml` overrides most of them.

### Stage 2.5 — clock, per-chain voice, modulation streams (`done` — but partially wrong)

What is implemented:

- Substrate clock from mean-magnetization zero-crossings (`clock.rs`). ✓
- Mono priority via last-note-wins per channel in `MidiSender::send_gate`. ✓
- Cmaj7 default pitches (C3/E3/G3/B3). ✓ (with a footgun, see §3)
- Clean shutdown via `ctrlc` + `AtomicBool` polling at the top of each tick. ✓ (with a SIGTERM caveat, see §2)
- `MidiSender::shutdown()` sends `All Notes Off` (CC 123) and `All Sound Off` (CC 120) on every used channel. ✓

What is **missing**: **[gap]**

The spec's "Modulation CC" section under Stage 2.5 specifies:

> Signal: sum of sigma_z over output sites, range [-4,4]. Map linearly to [0,127] centered on CC 64. Sample once per tick (50 Hz at defaults). Filter: emit only if changed by ≥1 since last emission.

There is no code that does this. Grep across all source files for any per-tick CC stream comes up only with `wall_midi.rs` sending the wall-position CC. The chain-wide modulation CC simply doesn't exist. The README also doesn't mention it (the "Modulation CC" section was dropped during the TOML-routing refactor), so this is either a regression that nobody noticed because the README was updated in lockstep, or it was never implemented and the spec's `done` marker is overstated. Either way, the spec and code disagree.

It's also not surfaced in the TOML schema — there is no `[chain_a.modulation]` block or equivalent, so even if you implemented this today you'd have to decide where the channel/CC-number/range go in the config file.

### Walls (`done`)

The detector, the wall tracker, the round-robin voice allocator with oldest-active stealing, the `repitch_on_move` vs held-CC modes, the OSC events — all present.

**[gap]** Wall velocity is hardcoded to `96` in `wall_midi.rs`. The spec describes a "local-order" velocity derivation ("high order → high velocity (sharp attack); thermal region → low velocity (soft)"). The walls spec lists this in "Open items" so the omission is acknowledged, but it should probably be reflected in the velocity field or at least noted in a code comment rather than buried in a magic constant.

**[doc]** `Wall.left_sign` is computed on every tick but never read by anything that emits a signal — also acknowledged in the spec's "Open items" as deferred.

### OSC (`done`)

Inbound receiver, outbound sender, parameter clamping, exponential smoothing of kt/eps/j/w, per-chain `/a/...` and `/b/...` prefixes, `/physics/...` writes that fan out to both chains, throttled state messages, per-tick bundling. All present.

**[cleanup]** `osc_io.rs` defines a `PhysicsTargetsMap` struct (with a `new()` and a `get()`) that is never used — superseded by `OscTargets`. Dead code that will produce a `dead_code` warning.

**[risk]** Initial values of `kt`/`eps`/`j`/`w` loaded from TOML are not run through the bounds checks. `PhysicsTargets::clamp_kt` etc. enforce `[0..=2]` / `[0..=0.5]` / etc. only on OSC writes. If `config.toml` has `kt = 100`, the chain will start at that value and only get clamped if an OSC write later arrives. The validator at startup should run the file's physics values through the same clamps (or refuse to load).

### MIDI routing via TOML (`done`)

Matches the spec — channels 1-based, all the validators (channel range, duplicate detection, voice-site-exists, pitch/CC ranges), error messages name both claimants. The unit tests in `config_file.rs` cover the cases the spec calls out. Good.

**[doc]** The spec says "Option 1 (fixed named voices) chosen over Option 2 (named pool)". The README and the code describe the same thing as a "named pool of channels" with round-robin allocation. After re-reading the spec carefully, Option 1 in the spec context means "named `voice_N` keys, allocated round-robin, stolen oldest-first", which is exactly what's implemented — the disagreement is only terminological. Worth tightening the language in one of the two docs so a future reader doesn't think they're meaningfully different.

**[risk]** The shorthand `voice_N = <channel>` falls back to a default pitch indexed by the voice's **position in the sorted list of defined voices**, not by `N`. So if you define only `voice_4 = 1`, you get pitch 48 (C3) — not pitch 55 (G3, the "fourth" entry in the Cmaj7 voicing table). The unit test `gate_voice_full_form_carries_pitch` documents this as "index 1 (E3, MIDI 52)" — confirming it's intentional, but the README's bare phrase "default pitch" hides the surprise. Either document the positional default explicitly or change the default to a function of `N`.

### Localized perturbations (`done`)

`PerturbationRouter` + `MidiInputListener` + per-pipeline `apply_input_perturbations()` all match the spec. The router unit tests in `perturbation.rs` are thorough.

**[bug — minor]** The `--debug` CLI flag is declared in `cli.rs` with a comment saying it should fire a single Flip on site 4 at tick 2000, "Remove once MIDI input lands." MIDI input has landed, and the flag's handler was never wired up — `main.rs` never reads `cli.debug`. The flag exists but does nothing. Either implement it (low value now) or delete the field.

**[doc]** `config.toml` has `[input.perturbation]` with `kind = "flip"` and `axis = "x"` and `magnitude = 0.6`. With `kind = "flip"` the axis and magnitude fields are ignored. Loads fine, but the file is mildly misleading.

**[risk]** Timing asymmetry: coupling injects pending field deltas in *phase 1.5* of the runtime step, before `chain.step()`, so coupling fields are consumed on the same tick. MIDI-driven `FieldSpike` perturbations are applied in *phase 3*, after `chain.step()`, so they wait a tick. Both are correct per the spec wording ("FieldSpike modifies the effective field for the *next* integration step"), but the one-tick latency is invisible from outside and worth a code comment for whoever wonders why a hard `FieldSpike` doesn't fire an immediate zero-crossing event.

### Stage 3 — polyrhythmic two-chain (`planning` — but partly built)

The spec marks this as "planning". The code has:

- `chain_b` support in `Config` and the loader, with per-chain physics overrides
- `ChainPipeline` abstraction so the runtime holds a `Vec<ChainPipeline>`
- `[coupling]` section in the TOML schema with `mean_field_z` implemented and `site_paired` / `shared_drive` as stubs that log a one-time warning
- OSC namespacing (`/a/...`, `/b/...`, plus broadcast `/physics/...`)
- Per-chain physics in `chain_b` of `config.toml` set up to chase period-3 (`kick_angle = 2.0944 ≈ 2π/3`)

So Stage 3 is materially done in the single-machine, single-process sense. The "planning" tag is stale.

What is genuinely **not done** of Stage 3:

- The sweep tool (`sweep.rs`) covers one chain at a time, not joint two-chain analysis. The spec's "sweep tool to find f/3 parameters" exists in single-chain form; verifying the lcm-recurrence story between two chains is still manual.
- `site_paired` and `shared_drive` coupling shapes are stubs with no implementation.

---

## 2. Bugs and correctness risks

### 2.1 [bug] SIGTERM is not actually handled

`Cargo.toml`:

```toml
ctrlc = "3.4"
```

With no feature flags. The `ctrlc` crate by default catches only `SIGINT` (the Ctrl-C interrupt); `SIGTERM` requires the `termination` feature. The spec explicitly lists `SIGTERM` among the clean-shutdown triggers:

> Triggers: normal end, SIGINT, SIGTERM.

Fix:

```toml
ctrlc = { version = "3.4", features = ["termination"] }
```

This is a real divergence — if the process gets killed by `systemd`, a launchd job, or `kill` (not `kill -2`), the cleanup path won't run and hanging notes can stick on the eurorack. The scheduler's `Drop` impl mitigates this for in-flight note-offs but the `All Notes Off` / `All Sound Off` belt-and-braces sweep in `MidiSender::shutdown()` is skipped entirely.

### 2.2 [bug, latent] Per-chain `ticks_per_period` is allowed but only `chain_a`'s drives the wall clock

`Runtime::build`:

```rust
let dt_real_secs =
    config.tempo.drive_period_secs / config.chain_a.physics.ticks_per_period as f64;
```

And `main.rs`:

```rust
let total_ticks =
    cli.periods.unwrap_or(20_000) * config.chain_a.physics.ticks_per_period as u64;
```

The TOML loader accepts an independent `ticks_per_period` on `chain_b` (and `chain_b` in the shipped config does override most physics fields, though it happens to keep `ticks_per_period = 25`). If a future config sets `chain_b.physics.ticks_per_period = 50`, the wall-clock pacing remains tied to chain A's value, and chain B's drive period — measured in seconds — becomes half of chain A's, *not* the same. The two chains would no longer share a common drive period.

Either:

- forbid per-chain `ticks_per_period` overrides at validation time, or
- compute `dt_real_secs` from the GCD/LCM of the two chains' tick rates, or
- document that `ticks_per_period` is shared-only and let the validator reject mismatches.

In the shipped config the values match, so it's latent.

### 2.3 [bug] Clock detector applies threshold to current value only

`clock.rs`:

```rust
let sign_changed = self.prev_m.signum() != current_m.signum()
    && self.prev_m != 0.0;
let above_floor = current_m.abs() > threshold;
let crossed = sign_changed && above_floor;
```

The per-site `EventDetector` requires `prev < -threshold && current > threshold` (or vice versa) — both endpoints must clear the threshold. The clock checks only `current.abs() > threshold`. So if `prev_m = 0.001` and `current_m = -0.06`, with `threshold = 0.05`, this fires a clock pulse — even though the previous value was essentially zero. In thermal-but-noisy regimes this could produce false clock ticks at near-zero magnetization, contradicting the spec's "stops when thermalized" claim.

Probably wants the symmetric version:

```rust
let crossed = (prev_m < -threshold && current_m > threshold)
           || (prev_m > threshold && current_m < -threshold);
```

### 2.4 [risk] Initial physics values are not range-checked

Already mentioned in §1. Worth repeating because it's the single most likely way to get a config that "loads OK but produces NaN-flavored audio". Validation should run `PhysicsTargets::clamp_*` against the file values and either reject out-of-range or warn-and-clamp.

### 2.5 [risk] `[coupling]` is silently ignored when `chain_b` is absent

`Runtime::build`:

```rust
let coupling = match (&config.coupling, &config.chain_b, coupling_targets) {
    (Some(c), Some(_), Some(targets)) => Some(CouplingState::new_with_targets(c, targets)),
    _ => None,
};
```

If a user keeps `[coupling]` while deleting `[chain_b]`, the program runs but ignores the coupling section. A startup warning (or a validation error) would be friendlier.

### 2.6 [risk] `MidiSender::send_gate` warns on invalid channels, but every other send method silently returns

All paths are unreachable thanks to validator coverage, but the inconsistency makes the warning either useful (and missing elsewhere) or pointless (and noisy). Pick one style and apply it everywhere.

---

## 3. Smaller issues

### 3.1 [doc] `Cargo.toml` description is stale

```toml
description = "Time-crystal-driven MIDI gate generator (Stage 1-2)"
```

The codebase implements through Stage 2.5 plus walls, OSC, perturbations, and most of Stage 3. Replace with something accurate, e.g. "single- or two-chain time-crystal MIDI generator with OSC parameter control and MIDI input perturbation".

### 3.2 [doc] License declaration vs `LICENSE` file mismatch

```toml
license = "MIT OR Apache-2.0"
```

The `LICENSE` file contains MIT only. Either drop the Apache half from `Cargo.toml` (use `license = "MIT"`) or add an `LICENSE-APACHE` file with the Apache 2.0 text.

### 3.3 [doc] `tempo.bpm = 17` in the shipped config

17 BPM = drive period of ~3.5 seconds. That's the drive period, not the perceived beat — in the locked period-2 phase the perceived tempo would be ~8.5 BPM, which is glacial. This is presumably intentional for the author's piece, but worth a comment in `config.toml` saying so. A reader who knows the README's default of 120 BPM will be startled.

### 3.4 [cleanup] `PhysicsTargetsMap` in `osc_io.rs` is dead

Defined but unused. Delete.

### 3.5 [cleanup] Commented imports in `src/config/mod.rs`

```rust
// use coupling::CouplingState;
// use pipeline::ChainPipeline;
```

Both refer to symbols that now live in `runtime/`, not `config/`. Delete.

### 3.6 [cleanup] Duplicate imports in `src/osc.rs`

```rust
use rosc::{OscMessage, OscPacket, OscType};
use rosc::{encoder, OscBundle, OscPacket as RoscPacket, OscTime, OscType as RoscOscType};
```

`OscPacket` and `OscType` are pulled twice (the second pair under aliases). Merge into one `use` and pick one name.

### 3.7 [cleanup] `crate::chain` resolution depends on a `use` in `main.rs`

`main.rs` has `use crystallized_time::{chain, config};`, which makes `crate::chain` and `crate::config` resolve from sibling modules like `clock.rs`. That works (verified with a minimal rustc check) but it's a fragile pattern — removing or moving that `use` line breaks compilation in unrelated files. Either:

- promote it to `pub use` and make the dependence explicit, or
- have each module import `crystallized_time::chain` etc. directly (as `clock.rs` already does for `chain_id`).

### 3.8 [cleanup] `--debug` CLI flag

Either wire it up or delete it. Its own doc comment says "Remove once MIDI input lands"; MIDI input has landed.

### 3.9 [cleanup] `Cargo.lock` in `.gitignore`

For binary crates the usual convention is to commit `Cargo.lock`. Optional — choice of policy.

---

## 4. What I couldn't verify

- I had `rustc` but no `cargo`, so I couldn't run `cargo check` / `cargo test` / `cargo clippy`. The compilation reasoning above is purely from reading source.
- I didn't run the program against a virtual MIDI port, so all claims about runtime behavior are inferences from the code.
- The physics correctness — whether the chain actually period-doubles in the default regime, whether chain B with `kick_angle = 2π/3` reaches period-3 — is a question for the sweep tool, not a code review. The sweep tool exists and looks correctly implemented.

---

## 5. Suggested next steps, prioritised

1. **Fix SIGTERM** — one-line `Cargo.toml` change. Real bug, low cost.
2. **Decide on the modulation CC** — either implement it (small per-tick CC emitter alongside the clock), update the spec to mark the feature dropped, or fold it into a deliberate "Stage 2.6" revision. The spec currently lies about it.
3. **Tighten the clock crossing test** — make it symmetric like the per-site detector, or document the asymmetry.
4. **Range-check initial physics values at load time** — easy, lifts a foot-gun.
5. **Update the spec's stage markers** — Stage 3 is largely done; the description in `Cargo.toml` is also out of date. Drift here is cheap to fix and high-value for future you.
6. **Drop `--debug` and the unused `PhysicsTargetsMap`/commented imports** — small, free wins.
7. **Decide on wall velocity** — if it's going to stay 96, write `const WALL_BIRTH_VELOCITY: u8 = 96;` with a comment pointing at the "Open items" decision. If it's going to compute from local order, that's a couple of evenings.
8. **Optional but tasteful** — pick one of the two "where does the inter-module path come from" patterns (§3.7) and use it consistently.

Nothing in the list is a blocker for the next listening session. The first two are the ones I'd actually fix before the next eurorack run.