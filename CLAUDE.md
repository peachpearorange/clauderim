# Skyrim II
A Bevy 0.19.1 game meant to pass, for five minutes, as "Skyrim 2": a Nordic wilderness valley, a horned-helmet Dragonborn with sword and shield, wolves, draugr, bandits, caves/barrows, a word wall, a dragon. Everything is procedural: no mesh or texture assets. Only fonts live in `assets/fonts`.

# Modules
- `terrain.rs` — heightfield world (±768 m, 2 m grid), `Ground` resource (`height`, `normal`, `surface`), `forest(at)` density, lake, far mountains. `height_at` is the pure function.
- `place.rs` — named places (`Place` enum with `name/spot/marker/flat/sunk`), `START`, lake, road polylines, `road_distance`.
- `sky.rs` — physical atmosphere, sun/moon cycle, `Daylight { level, shelter }`. Set `shelter` toward 1 while the player is underground; exposure and ambient follow it.
- `model.rs` — mesh toolkit: `Piece::new(mesh, color)` then `.at/.sized/.pitched/.yawed/.rolled/.span/.mirrored/.shaded`, `merge(pieces)`, shapes `lathe`, `tube` + `curve` + `taper`, `lump` (noisy flat-shaded rock), `blade`, `fan`, `ball`, `block`, `rod`, `cone`, `limb`.
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
- `hud.rs` — compass, bars, notices, prompts, intro. `audio.rs` + `audio/` — all sound synthesised at startup.
- `signal.rs` — shared messages/resources between modules: `Notice`, `Discovered`, `Prompt`, `Engaged`, `Sound { cue: Cue, at }`, `FoeSpawn { kind, dormant }` (spawn an entity with this + `Transform` and the creature module turns it into a creature), `WordWall`, `Shouts`.

Physics: avian3d. Exact vertical rays can miss the heightfield collider; tilt them slightly or use shape casts.
Lighting is physical: sun is `lux::RAW_SUNLIGHT`, exposure ev100 ≈ 13 outdoors, ≈ 9 underground. Emissive is NOT exposure-scaled: ~1–10 glows (see `Stuff::Frost`/`Ember`/`Cinder`), 50+ blows out to white. Point lights need ~100k+ lumens to matter outdoors, far less underground.

# Opts
Env var `SKYRIM` holds JSON5 `opts::Opts`: `hour` (start hour, 9.5), `day` (secs per day), `shot: <secs>` (screenshot to `screenshots/shot-$SHOT_NAME.png` then exit), `at: '<place name fragment>'` (spawn near that place), `yaw`, `pitch`, `turn` (camera yaw offset, deg), `zoom` (camera distance), `pose: 'swing'|'guard'|'dead'|'run'`, `intro`, `first` (first person), `inside: [x,y,z]` (spawn at a spot), `foe: 'wolf'|'draugr'|'overlord'|'bandit'|'chief'` (inert specimen in front of the player), `foe: 'dragon'|'dragonaloft'|'dragonslain'` (dragon posed in front), `dragon: <secs>` (dragon arrival time, default 75), `press: [[secs, 'Z'|'E'|'F'|'W'|'Space'|'Shift'|'LMB'|'RMB'], …]` (inject input).
e.g. `SKYRIM='{shot: 5, at: "hollow"}' SHOT_NAME=barrow cargo run`. Screenshots are the way to check visuals — always look at them.

# Style
Follow ~/CLAUDE.md Rust style. rustfmt config is in ~/.rustfmt.toml (2-space). Avoid comments; name things well.
