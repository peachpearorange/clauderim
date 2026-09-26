# Skyrim II
A Bevy 0.19.1 game meant to pass, for five minutes, as "Skyrim 2": a Nordic wilderness valley, a horned-helmet Dragonborn with sword and shield, wolves, draugr, bandits, caves/barrows, a word wall, a dragon. Everything is procedural: no mesh or texture assets. Only fonts live in `assets/fonts`.

# Modules
- `terrain.rs` — heightfield world (±768 m, 2 m grid), `Ground` resource (`height`, `normal`, `surface`), `forest(at)` density, lake, far mountains. `height_at` is the pure function.
- `place.rs` — named places (`Place` enum with `name/spot/marker/flat/sunk`), `START`, lake, road polylines, `road_distance`.
- `sky.rs` — physical atmosphere, sun/moon cycle, `Daylight { level, shelter }`. Set `shelter` toward 1 while the player is underground; exposure and ambient follow it.
- `cloud.rs` + `cloud.wgsl` — procedural cloud layer (embedded shader on a plane at 1.9 km), lit from `sky::toward_sun/toward_moon` with sunset reddening.
- `model.rs` — mesh toolkit: `Piece::new(mesh, color)` then `.at/.sized/.pitched/.yawed/.rolled/.span/.mirrored/.shaded`, `merge(pieces)`, shapes `lathe`, `tube` + `curve` + `taper`, `lump` (noisy flat-shaded rock), `blade`, `fan`, `ball`, `block`, `rod`, `cone`; organic: `loft` (stacked `Hoop` rings), `sweep` (along a curved spine), `spline`, `sculpt` (spline + sweep), `ruffled` (fur/scale noise), `sheet` (membranes).
- `stuff.rs` — material palette `Stuff` (Skin, Fur, Leather, Cloth, Iron, Steel, Gold, Bone, Wood, Bark, Needles, Stone, Frost, Ember, Gloss, Membrane, Cinder); `Stuffs::of(stuff)` gives the shared material. Vertex colour tints; the material adds grain texture.
- `texture.rs` — tiling procedural textures.
- `humanoid.rs` — jointed rig (11 joints), `Motion` drives procedural poses, outfits `dragonborn/draugr/bandit` built as `Kit`s.
- `walker.rs` — kinematic character movement for anything with `Walker` + `Collider` (set `wish`, `facing`, `leap`, `shove`).
- `player.rs` — player, camera `View` resource, controls.
- `combat.rs` — `Vitals`, `Side`, `Fighter`, `Dead`, `Struck` message, `Shake`.
- `creature.rs` — turns `FoeSpawn` into wolves/draugr/bandits, `Mind` AI, looting, world encounters. `wolf.rs` — quadruped rig.
- `dragon.rs` — the dragon: 18-bone rig, `Flight` state machine (arrive, circle, strafe with fire breath, land, fight, rise, fall, slain → soul absorbed).
- `shout.rs` — Unrelenting Force (Z), word walls, soul wisps. `fx.rs` — bevy_hanabi particle `Effects`.
- `flora.rs` — trees, rocks, grass and scatter. `cave.rs` + `sdf.rs` — SDF-meshed Hollowcrag Barrow and Fellhound Den. `landmark.rs` — watchtower, camp, standing stones.
- `hud.rs` — compass, bars, notices, prompts, intro. `inventory.rs` — `Item` (name/kind/value/weight/power), `Loot`, player `Inventory` (`take(loot)`), Tab/I menu. `audio.rs` + `audio/` — all sound synthesised at startup.
- `signal.rs` — shared messages/resources between modules: `Notice`, `Discovered`, `Prompt`, `Engaged`, `Sound { cue: Cue, at }`, `FoeSpawn { kind, dormant }` (spawn an entity with this + `Transform` and the creature module turns it into a creature), `WordWall`, `Shouts`.

Physics: avian3d. Exact vertical rays can miss the heightfield collider; tilt them slightly or use shape casts.
Lighting is physical: sun is `lux::RAW_SUNLIGHT`, exposure ev100 ≈ 13 outdoors by day, ≈ 6.2 at night, ≈ 7.6 underground; the eye adapts toward that smoothly (fast to bright, slow to dark). Emissive is NOT exposure-scaled: ~1–10 glows (see `Stuff::Frost`/`Ember`/`Cinder`), 50+ blows out to white. Point lights need ~100k+ lumens to matter outdoors, far less underground.

# Opts
Env var `SKYRIM` holds JSON5 `opts::Opts`: `hour` (start hour, 9.5), `day` (secs per day), `shot: <secs>` (screenshot to `screenshots/shot-$SHOT_NAME.png` then exit), `at: '<place name fragment>'` (spawn near that place), `yaw`, `pitch`, `turn` (camera yaw offset, deg), `zoom` (camera distance), `pose: 'swing'|'guard'|'dead'|'run'`, `intro`, `first` (first person), `inside: [x,y,z]` (spawn at a spot), `foe: 'wolf'|'draugr'|'overlord'|'bandit'|'chief'` (inert specimen in front of the player), `foe: 'dragon'|'dragonaloft'|'dragonslain'` (dragon posed in front), `dragon: <secs>` (dragon arrival time, default 75), `press: [[secs, 'Z'|'E'|'F'|'W'|'A'|'S'|'D'|'I'|'Tab'|'Enter'|'Escape'|'Up'|'Down'|'Space'|'Shift'|'LMB'|'RMB'], …]` (inject input).
e.g. `SKYRIM='{shot: 5, at: "hollow"}' SHOT_NAME=barrow cargo run`. Screenshots are the way to check visuals — always look at them.

# Build
Priority is fast incremental builds over runtime speed. Debug builds only; release only when asked.
- `cargo check` to verify compilation; `cargo run --features dev` to test (`dev` = Bevy dynamic linking, incremental rebuild ~13 s vs ~21 s). `tools/shot <name> '<opts>'` builds and screenshots.
- On a panic or startup failure (including Bevy system param conflicts), rerun with `RUST_BACKTRACE=1`.
- Cloud sessions have no GPU: wrap runs in `xvfb-run -a -s "-screen 0 1920x1080x24"` (lavapipe, software Vulkan). Frames are slow and the game clock is capped per frame, so it runs far behind real time: `shot: 3` takes ~1.5 min. Keep `shot` small and pair it with `intro: false`.
- Docs: https://docs.rs/bevy/0.19.1/bevy/

# Style
Code structure should mirror the thinking behind it: translate the user's conceptual framework fairly directly into code. Functional-ish.
- Format with `cargo +nightly fmt` (repo `rustfmt.toml` uses nightly-only options; stable rustfmt ignores them and reflows everything). Avoid comments; name things well.
- Order within a file: if A refers to B, A comes after B. Keep related code together.
- Semicolons are a verbosity signal: prefer expressions over statements.
- No `return`, `continue`, `let … else`, or early exits.
- Prefer let chains over nested `if let`/`if` or `.and_then()`. Chains can mix fallible and plain (irrefutable) lets:
  ```rust
  if let Some(weapon) = equipped
    && weapon.is_ranged()
    && let reach = weapon.range() * skill
    && let &Transform { translation, .. } = weapon.transform()
    && let Some(target) = find_target(translation, reach)
  {
    attack(target)
  } else if let Some(weapon) = equipped {
    melee(weapon)
  } else {
    flee()
  }
  ```
- Prefer `Option` methods over matching on `Option` (not so for most other enums).
- Prefer `find` (match a predicate), `find_map` (match and transform), `any`, `fold` over loops with break.
- Eagerly destructure: `let &Thing { field } = thing()` binds `field` by value, no `*field` later.
- Domain model with named instances, newtypes and associated consts.
- No more function-calls-function indirection than needed: inline, or use blocks or small local closures, when a helper would wrap a single call site.
- ECS: merge components that always co-occur. Name components by behaviour (`WalkAroundRandomly`), not entity (`SheepWalk`).

# Collaboration
- Don't implement features that weren't asked for. Ask when unclear.
- If a requested approach can't be done, say so; never change direction without asking.
