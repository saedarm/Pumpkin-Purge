# Pumpkin Purge

A Halloween festival FPS in Rust + Bevy 0.20. Pumpkin-headed scarecrows pour out of the corn maze in waves. Fight them off with a candy-themed arsenal, using hay bales and booths as cover.


**What's in it:** 4 weapons plus a pitchfork and candy grenades, 5 enemy types (including a dive-bombing bat), 3 rotating bosses, a between-wave shop with upgrades, weapon unlocks, and 23 attachments, aim down sights with scopes, power-ups, flow-field pathfinding, Easy/Normal/Hard, saved settings and best scores. There are no asset files: every model is built from primitive shapes and every sound is synthesized in code.

## How it was made

The game was built with AI assistance: the code was written by Claude (Anthropic) from feature requests, and each feature was checked with unit tests and scripted playtests before moving on. Rough numbers:

- About 7,500 lines of Rust across 18 files in `src/`
- 56 tests (54 run by default, 2 are on-demand export tools)
- Built feature by feature: the base shooter first, then enemy bodies and weapons, sound, collision, reloading, bosses, the shop, the pitchfork and power-ups, settings and saves, pathfinding, aim down sights and attachments, then health bars, damage numbers, the bat, goo, difficulty, and grenades

Art is placeholder on purpose. The plan was to get every mechanic working and tested first, then swap in 3D models (see [Swapping in real assets](#swapping-in-real-assets)).

## Requirements

- **Rust 1.97.1 or newer** (Bevy 0.20 needs it). Install from [rustup.rs](https://rustup.rs), or run `rustup update stable` if you already have Rust.
- **Windows:** Visual Studio Build Tools with the "Desktop development with C++" workload (Rust needs its linker).
- **Linux (Debian/Ubuntu):** `sudo apt install build-essential pkg-config libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev`
- **macOS:** `xcode-select --install`
- A GPU with Vulkan, DirectX 12, or Metal support (any card from the last ten years or so).
- About 15 GB of free disk space for the build folder. Bevy's compiled files are large; `cargo clean` deletes them.

## Build and run

```
git clone <this repo>
cd pumpkin-purge
cargo run --release
```

The first build compiles Bevy and takes 5-15 minutes. Later builds take seconds. Use `--release` for playing; a plain `cargo run` builds faster but runs choppier.

**Keep `Cargo.lock`.** It pins the exact versions the game was tested with. In particular, the game uses **rand 0.9**. rand 0.10 moved `random_range` off the `Rng` trait, so if an IDE or `cargo upgrade` bumps rand to 0.10 you'll get dozens of `no method named random_range` errors. Set it back to `rand = "0.9"`.

**RustRover / IntelliJ:** if a run configuration fails with `package ID specification ... did not match any packages`, it's passing the folder name as the package name. Edit the configuration's command to just `run --release` (the package is named `pumpkin-purge`).

## Tests

`cargo test` checks bat flight and dive-bombs, goo splats, grenade limits, difficulty settings, per-difficulty best scores, hit detection, collision, reloading, attachment and upgrade stat math, buying and swapping attachments, weapon unlocks, aiming, the shop, boss rotation, enemy looks, power-ups, settings ranges, save-file parsing, best-score tracking, damage direction math, pathfinding (including walking from every spawn point to the player on the real map), and the generated audio. `cargo test export_wavs -- --ignored` writes every sound effect to `target/sounds/` so you can listen to them on their own.

### Practice mode

Launch with `PUMPKIN_WAVE` set to start at the shop right before that wave, with all four weapons unlocked, 2,000 candy to spend, and one of each power-up waiting just in front of you. Practice runs don't count toward your best score. To fight each boss:

```
PUMPKIN_WAVE=5 cargo run    # The Pumpkin King
PUMPKIN_WAVE=10 cargo run   # The Scarecrow Witch
PUMPKIN_WAVE=15 cargo run   # The Gourd Golem
```

On Windows PowerShell: `$env:PUMPKIN_WAVE=5; cargo run`

### God mode

`PUMPKIN_GOD=1 cargo run` (PowerShell: `$env:PUMPKIN_GOD=1; cargo run`) makes you take no damage, so you can watch mechanics without dying. Hit direction markers still show; the red flash and shake don't. Combine it with `PUMPKIN_WAVE` to test bosses. God-mode runs don't count toward your best score.

## Controls

| Input | Action |
|---|---|
| Click | Menu buttons; during play, re-locks the mouse if it got freed |
| WASD | Move |
| Shift | Sprint |
| Space | Jump (you can climb onto hay bales and bale stacks) |
| Left mouse | Fire (hold for the SMG) |
| Right mouse (hold) | Aim down sights |
| F | Pitchfork jab |
| G | Throw a candy grenade |
| R | Reload |
| 1-4 or mouse wheel | Switch weapons (wheel skips empty ones) |
| B | Open / close the shop (between waves) |
| Enter | Start the next wave early (between waves) |
| Esc | Pause menu and settings (Esc again to resume) |
| F3 | Pathfinding overlay |
| R | Restart after dying |

## Menu and settings

The title screen and the pause menu (Esc) share one panel. The title screen has **Play** and shows your best run; the pause menu has **Resume**, **Restart**, and **Quit to Title**. Both have **Quit Game** and the settings, which you change with the `<` and `>` buttons:

| Setting | Range | Default |
|---|---|---|
| Mouse sensitivity | 0.1x - 3.0x | 1.0x |
| Invert mouse Y | On / Off | Off |
| Field of view | 60 - 110 | 80 |
| Master volume | 0 - 100% | 80% |
| Music volume | 0 - 100% | 60% |
| Screen shake | Off / Low / Full | Full |
| Fullscreen | On / Off | Off |
| Difficulty | Easy / Normal / Hard | Normal |
| Damage numbers | On / Off | On |

**Difficulty** can only be changed on the title screen (it's greyed out in the pause menu).

| | Easy | Normal | Hard |
|---|---|---|---|
| Enemy health | 70% | 100% | 140% |
| Damage you take | 60% | 100% | 140% |

Best scores are tracked separately for each difficulty, and the title screen shows the best for the one you have selected.

Settings save the moment you change them. Your best wave, score, and kills save when a run ends; each is tracked separately (per difficulty), and the game-over screen says **NEW BEST SCORE!** when you beat your old score.

Both are small text files you can open or delete:

- Windows: `%APPDATA%\PumpkinPurge\settings.txt` and `best.txt`
- Linux / macOS: `~/.config/pumpkin-purge/`

Set `PUMPKIN_SAVE_DIR` to keep them somewhere else.

## Combat feedback

- **Health bars:** once you hurt a regular scarecrow, a small bar appears over its head (green, then yellow, then red). It hides 5 seconds after the last hit. Bosses use the big bar at the top of the screen instead.
- **Damage numbers:** each hit floats a number up from the target, shown at 10x (a pistol body shot reads "10"). Headshots show in red with a "!". Turn them off in settings.
- **Goo:** every kill leaves a splat of pumpkin goo on the ground, the same color as the pumpkin (pale ghost pumpkins leave pale goo), with droplets and bits of straw. Splats shrink away after 30 seconds; at most 90 stay on the ground at once.
- **Damage direction:** each hit puts a red bar on a ring around the crosshair, pointing toward whatever hit you. It stays pointed at that spot as you turn and fades after about a second and a half.
- **Screen shake:** getting hit, explosions (including your own gourds), the King's slam, the Golem crashing into a prop, boss deaths, and firing the scattergun or launcher all shake the view. Bigger hits shake harder, and distant explosions shake less. Turn it down or off in settings.

## Weapons

You start each run with only the pistol. The other three unlock in the shop.

| # | Weapon | Unlock | Magazine | Reload | Notes |
|---|---|---|---|---|---|
| 1 | Candy Corn Pistol | free | 12 | 1.0s | Unlimited spare ammo. Semi-auto. |
| 2 | Hayseed Scattergun | 200 | 6 | 1.8s | 9 pellets, wide spread. Devastating up close. |
| 3 | Kettle Corn SMG | 350 | 40 | 1.5s | Full auto, low damage per shot. |
| 4 | Gourd Launcher | 600 | 1 | 1.2s | Lobs an exploding gourd. Splash damage. |

**Aim down sights:** hold right mouse. The gun comes up to the center of the screen and the view zooms in (1.25x with iron sights, more with a sight attachment). While aiming, spread is about a third of hip fire, recoil is halved, you move at 60% speed, you can't sprint, and mouse look slows down to match the zoom. With a scope you look through a round scope view instead of at the gun.

**Candy grenades:** press G to lob one. It bounces off the ground and props and pops after 1.8 seconds, hitting everything within 4.5m (it can't hurt you). You start with 2 and can carry 5. Get more from the shop (80 candy each) or from ammo crates (+1 each).

**Pitchfork:** press F to jab. It's always available and needs no ammo. It hits up to three scarecrows in front of you within about 2.4m and knocks them back, so it's your answer to getting swarmed or running dry. The scattergun also shoves enemies back a little. Bosses barely budge from either.

The weapon list shows `magazine / spare rounds`. Firing with an empty magazine reloads automatically. Switching weapons cancels a reload. A weapon with nothing left falls back to the pistol. Headshots do double damage (the crosshair flashes red).

## Cover

Hay bales, bale stacks, booth counters, booth posts, lamp posts, and gravestones are solid. They block you, the scarecrows, your bullets, thrown seeds, and gourds. Standing on top of a bale stack keeps you out of reach of Walkers, Runners, and Lobbers' fists, but Brutes are tall enough to hit you up there, and Lobbers can still hit you with seeds.

## Pathfinding

Scarecrows path around props instead of walking into them. The arena is split into a grid of half-meter cells, and cells under props are marked blocked, padded by an enemy's width so they don't clip corners. A few times a second the game works out, for every open cell, the walking distance to you around the props (Dijkstra's algorithm, run outward from your cell). Each scarecrow then follows those distances downhill. One calculation serves every enemy, so big waves stay cheap; a full rebuild takes a couple of milliseconds.

- If a scarecrow has a clear straight line to you, it walks straight at you.
- Otherwise it aims for the farthest point along its path it can see, so it rounds corners smoothly instead of zig-zagging cell to cell.
- Brutes and bosses use a second grid with wider padding, since they need more room.
- If you're standing on a bale stack, they gather around its edge.
- The Golem's charge still goes in a straight line on purpose, so you can still bait it into props.

**Press F3** in game to see it: blocked cells show as red squares on the ground, and each scarecrow has a green line to where it's currently heading.

`cargo test export_path_map -- --ignored` draws top-down pictures of the real map with walked paths to `target/pathmap-*.ppm`.

## Sound

Every sound is synthesized in code when the game starts, so there are no audio files to download or license: four gunshots, reload clicks, hit and headshot dings, pumpkin splats, explosions, a hurt buzz, a pickup chime, thrown-seed whooshes, scarecrow groans, a bell at the start of each wave, and a looping music-box theme over a low drone. Far-away sounds play quieter. To swap in real recordings, see the comment at the top of `sound.rs`.

## Enemies

| Type | Look | Behavior |
|---|---|---|
| Walker | Red shirt | Shambles toward you, claws when close. Shows up from wave 1. |
| Runner | Small, green shirt | Fast and fragile. Wave 2+. |
| Lobber | Purple shirt, witch hat | Keeps its distance and throws flaming seeds in an arc. Wave 3+. |
| Brute | Huge, dark shirt | Slow bullet sponge that hits hard. Always drops a pickup. Wave 4+. |
| Bat | Small glowing pumpkin with purple bat wings | Flies. Circles overhead, then hovers and screeches (your warning) and dive-bombs where you're standing. If it hits you it climbs back up and tries again. If it misses and hits the ground or a prop, it's stunned for 1.6s and takes 50% extra damage, and the pitchfork can reach it. Only its head can be hit. Wave 3+. |

Every regular scarecrow rolls its own look: pumpkin color (orange shades, plus the odd pale ghost pumpkin or unripe green one), head shape, eyes (diamond, angry, round, mismatched), mouth (toothy grin, zigzag, shocked O, wide smile), and hat (none, straw, top hat, bucket). Shirt color still marks the type, so you can tell a Walker from a Runner at a glance. Lobbers always wear witch hats.

Each wave is bigger than the last and spawns faster. Killed enemies sometimes drop **wrapped candy** (+25 health) or **crates** (ammo for every weapon).

## Power-ups

Glowing orbs that drop from 6% of regular kills and from every boss. Each lasts 12 seconds (grabbing another of the same kind resets the timer) and shows a countdown in the top right.

| Orb | Power-up | Effect |
|---|---|---|
| Red | Double Damage | Every weapon and the pitchfork hit twice as hard |
| Blue | Sugar Rush | Move 50% faster |
| Gold | Candy Magnet | Pickups within 18m fly to you |

## Bosses

Every 5th wave is a boss wave with a small escort. The boss has a health bar at the top of the screen, and the three bosses rotate: King on wave 5, Witch on 10, Golem on 15, then King again on 20 with 60% more health, and so on.

### The Pumpkin King (wave 5)

Giant with a gold crown and a green glowing face.

- **Ground slam:** when you're close, he raises his arms, then slams down and sends out an expanding ring. **Jump over the ring** or stand on a hay bale to dodge it.
- **Seed volley:** when you're far away, he throws five flaming seeds in a spread.
- **Summon:** every 14 seconds he calls two Runners to his side.

### The Scarecrow Witch (wave 10)

Floats above the ground. Pale ghost-pumpkin head, witch hat, purple glow.

- **Homing wisps:** every few seconds she releases three purple wisps that curve toward you. You can outrun them by sprinting, or break line of sight behind a prop; they pop when they hit one.
- **Teleport:** every 9 seconds, or whenever you knock off another 20% of her health, she vanishes in a purple flash and reappears somewhere else on the field.
- **Summon:** every 16 seconds she calls two Lobbers.

### The Gourd Golem (wave 15)

The biggest of the three. Green warty gourd head, mossy shirt, angry red glow.

- **Charge:** it stops, raises its arms (the bar says CHARGING), then barrels straight at where you were standing. Sidestep it.
- **Stun:** after a charge it's dazed for 2 seconds and takes **50% extra damage** (the bar says STUNNED). If you lure it into a hay bale or booth, it crashes and stays stunned for 3.5 seconds.
- **Split:** when it dies it breaks into two smaller, faster Gourdlings that also charge. The health bar tracks both halves.

Every boss drops five health/ammo pickups and one power-up when it dies.

## Shop

Between waves, press **B** to open the shop. The break timer stops while it's open. Every kill earns **candy** equal to its points. Press **Enter** (or the button) to start the next wave early, or **B** / **Esc** to go back to the field.

### Upgrades tab

Player upgrades that last for the rest of the run:

| Upgrade | Effect | Max level |
|---|---|---|
| Big Bag of Candy | +25 max health and a full heal | 5 |
| Sharper Kernels | +15% damage, every weapon | 5 |
| Butter Fingers | -15% reload time | 3 |
| Deep Pockets | +25% magazine size | 3 |
| Scarecrow Armor | -10% damage taken | 4 |
| Ammo Refill | fills every weapon you own | unlimited |
| Candy Grenade | +1 grenade | carry up to 5 |

Upgrades get more expensive each level; the refill is always 100.

### Weapon tabs (the customizer)

One tab per weapon. A locked weapon shows its stats and an **Unlock** button. An unlocked one shows its live stats on the left and five attachment slots on the right: **Sight, Barrel, Magazine, Grip, Special**.

- **Hover** any attachment to preview it: the stats panel shows exactly what changes (`Aim zoom  1.25x  ->  2.30x`) along with its description and price.
- **Click** to buy and equip it. You only pay once per attachment; after that, swapping between owned attachments (or back to None) is free.
- Equipped attachments show up on the gun model.

| | Pistol | Scattergun | SMG | Launcher |
|---|---|---|---|---|
| **Sight** | Red Dot (120), Corn Cob Scope 2.6x (220) | Red Dot (120) | Red Dot (120), Popcorn Scope 2.3x (220) | Red Dot (120), Gourd Scope 2x (220) |
| **Barrel** | Long Barrel: +25% damage, tighter (180) | Full Choke: 40% tighter spread (180) | Compensator: less recoil, tighter (180) | Rifled Tube: faster gourds, 30% wider blast (200) |
| **Magazine** | Extended Mag: +50% (150) | Speed Loader: 40% faster reload (160) | Drum Mag: 2x mag, slower reload (170) | Quad Tube: 4 gourds (250) |
| **Grip** | Grip Tape: -45% recoil (120) | Pump Grip: -45% recoil (140) | Foregrip: -40% recoil (140) | Stabilizer: -50% recoil (120) |
| **Special** | Candy Corn Piercers: rounds go through 3 scarecrows (350) | Hot Sauce Shells: sets scarecrows on fire for 3s (350) | Caramel Rounds: hit scarecrows move at half speed for 2.5s (350) | Cluster Gourds: each blast throws out 4 bomblets (400) |

Scopes give the most zoom and the tightest aimed spread but are slower to raise. A bigger magazine doesn't fill itself; reload to top it up.

## Code layout

- `main.rs` — app setup, game states (title / playing / game over), restart
- `bat.rs` — the bat pumpkin: flight, dive-bombs, stuns, wings
- `boss.rs` — the three bosses: King (slam, volley, summons), Witch (wisps, teleport, summons), Golem (charge, stun, split)
- `collision.rs` — box collision for props: push-out, standing on top, bullet blocking
- `festival.rs` — the map: ground, corn wall, hay bales, bale stacks, booths, string lights, lamp posts, graveyard
- `player.rs` — camera, movement, jump, health, screen shake, mouse lock and pause
- `save.rs` — save folder, reading/writing the text files, best scores
- `settings.rs` — settings values and the title/pause menu
- `weapons.rs` — base weapon stats, switching, magazines and reloading, aiming down sights, firing and specials, pitchfork, weapon and attachment models, gourds, pickups
- `enemies.rs` — enemy types, scarecrow bodies, walk animation, attacks, waves, deaths, hit detection
- `effects.rs` — chunks, tracers, light flashes, goo splats, shared meshes/materials
- `hud.rs` — crosshair, score, health bar, weapon list, grenade count, reload hint, boss bar, enemy health bars, damage numbers, damage flash, damage direction markers, center text
- `loadout.rs` — the attachment catalog, weapon unlock prices, and the stat math (base + attachments + upgrades)
- `looks.rs` — randomized pumpkin heads, carved faces, and hats
- `pathfinding.rs` — the flow-field grid, enemy steering, and the F3 overlay
- `powerups.rs` — Double Damage, Sugar Rush, Candy Magnet
- `shop.rs` — the shop screen: upgrades, weapon unlocks, the attachment customizer with stat previews
- `sound.rs` — synthesized sound effects and music

## Swapping in real assets

Everything is built from primitive shapes. To use a model pack, put `.glb` files in an `assets/` folder at the project root and replace a mesh with a scene:

```rust
commands.spawn((
    SceneRoot(asset_server.load("models/hay_bale.glb#Scene0")),
    Transform::from_xyz(6.0, 0.0, 4.0),
));
```

For enemies, keep the `Enemy` root entity and the hit spheres in `enemies.rs` (`PARTS`), and swap the child meshes for the model. Good CC0 sources: KayKit (Halloween set), Kenney (graveyard and blaster kits), Quaternius (rigged characters).

## Status and known issues

This is a mechanics-complete prototype, not a finished game.

- **Placeholder art.** Everything is primitive shapes. Real models are the next step.
- **One map.** The festival grounds.
- **Keyboard and mouse only.** No gamepad support.
- **Balance is untuned.** Wave sizes, prices, and boss health are first guesses.
- **Checked by unit tests but not yet in a full play session:** the King's ground slam, the Witch's teleport, the Golem's split, the bat's stun after a miss, the burn / caramel / cluster specials in live combat, sound output, and fullscreen. Bug reports on any of these are welcome.

## License

MIT. See `LICENSE`.
