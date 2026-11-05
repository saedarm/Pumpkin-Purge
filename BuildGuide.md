
Pumpkin Purge Build Guide
Nov 1, 2026 · @Sam Armentrout
Building Pumpkin Purge from scratch takes about 30 minutes, most of it Bevy's first compile. The game only compiles once all 18 files are in, because the modules all depend on each other. Until then, a one-line placeholder main.rs keeps every build green, so you can copy files at your own pace.
Before you start
You need Rust 1.97.1 or newer. Bevy 0.20 refuses to build on anything older.
What
Why
How to get it
Rust 1.97.1+
Bevy 0.20's minimum
Install from rustup.rs, or run rustup update stable if you already have it. Check with rustc --version.
Visual Studio Build Tools (Windows)
Rust on Windows uses Microsoft's linker
In the Visual Studio Installer, pick the Desktop development with C++ workload.
Up-to-date GPU drivers
Bevy draws with DirectX 12 or Vulkan
Your graphics card maker's latest driver.
10-15 GB free disk
Bevy's build folder (target) gets big
It reached 14 GB on my test machine with tests built too.
On Linux you also need the audio, input and window libraries: sudo apt install libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev pkg-config. On macOS, the Xcode command line tools are enough.
Get the source files from pumpkin-purge.zip in our chat. Unzip it somewhere you can open the files while you work.
Step 1: Create the project and start Bevy compiling
Create the project, give it the right dependencies, and let Bevy compile in the background while you copy files.
1. Create an empty project:
   cargo new pumpkin-purge
   cd pumpkin-purge
2. Replace Cargo.toml with this:
   [package]
   name = "pumpkin-purge"
   version = "0.1.0"
   edition = "2024"
   
   [dependencies]
   bevy = { version = "0.20", features = ["wav"] }
   rand = "0.9"
   
   # Bevy is slow in debug builds; optimize dependencies but keep our own code fast to compile.
   [profile.dev]
   opt-level = 1
   
   [profile.dev.package."*"]
   opt-level = 3
   The wav feature is what lets the game play its generated sounds. Leave it out and the game panics when it tries to play audio.
3. Copy Cargo.lock from the zip into the project folder. This pins every library to the exact version I built and tested with. Without it, Cargo picks the newest compatible versions, which usually works but isn't guaranteed.
4. Leave src/main.rs as the one-liner Cargo made (fn main() { println!(...) }). Run:
   cargo build
   This compiles Bevy and takes 5-15 minutes the first time. It only happens once. Copy files while it runs.
Step 2: Copy the 18 source files
Copy these into src/ in this order. The order goes from foundations up to the game loop, so if you read each file as you copy it, most of what it uses will already be familiar. Cargo ignores every file until main.rs declares it, so builds stay green the whole time.
#
File
Lines
What it does
1
collision.rs
164
Box collision for props: pushing you out, standing on top, blocking bullets
2
effects.rs
288
Flying chunks, tracers, light flashes, goo splats, shared meshes and colors
3
sound.rs
365
Every sound effect and the music, generated in code at startup
4
festival.rs
271
The map: ground, corn wall, hay bales, booths, lamps, graveyard
5
pathfinding.rs
541
The flow-field grid scarecrows follow, and the F3 overlay
6
player.rs
237
Camera, movement, jumping, health, screen shake, pausing
7
loadout.rs
467
The attachment catalog and the stat math (base + attachments + upgrades)
8
weapons.rs
1081
Guns, aiming down sights, reloading, pitchfork, grenades, pickups, gun models
9
powerups.rs
107
Double Damage, Sugar Rush, Candy Magnet
10
looks.rs
301
Random pumpkin heads, carved faces, and hats
11
enemies.rs
804
Enemy types, bodies, waves, walking, attacks, deaths
12
bat.rs
280
The bat pumpkin's flight and dive-bombs
13
boss.rs
443
The Pumpkin King, Scarecrow Witch, and Gourd Golem
14
save.rs
173
The save folder, best scores, practice and god mode flags
15
settings.rs
487
Settings and the title / pause menu
16
shop.rs
645
The shop screen: upgrades, unlocks, the attachment customizer
17
hud.rs
633
Everything on screen during play: health, ammo, boss bar, damage numbers
18
main.rs
273
Wires everything together. Copy this one last (Step 3)
Check each copy against the Lines column. A paste cut short is the most likely mistake, especially with weapons.rs. In PowerShell, this lists every file with its line count:
Get-ChildItem src\*.rs | ForEach-Object { "{0,-16} {1}" -f $_.Name, (Get-Content $_).Count }
Optionally copy README.md into the project root too. It documents every control, weapon, enemy and setting.
Step 3: Turn it on and run it
Copying main.rs over the placeholder is what switches the game on. Its mod lines pull in all 17 other files.
1. Replace src/main.rs with the one from the zip.
2. Build and start the game:
   cargo run
   With Bevy already compiled from Step 1, this takes under a minute.
3. To play at full speed, use a release build:
   cargo run --release
   The first release build compiles Bevy again (another 5-15 minutes), then it's quick. Plain cargo run is fine for testing changes; release is better for judging how the game feels.
You should see the title screen with PLAY, QUIT GAME, and nine settings. Click Play to start.
Step 4: Run the tests and check every mechanic
Tests
cargo test
Expected result: 54 passed, 2 ignored. The two ignored tests are tools you run on purpose:
• cargo test export_wavs -- --ignored writes every synthesized sound to WAV files so you can listen to them outside the game.
• cargo test export_path_map -- --ignored prints the pathfinding grid for the map.
Practice and god mode
Set these in PowerShell before cargo run (they last for that terminal window):
$env:PUMPKIN_GOD=1; $env:PUMPKIN_WAVE=5; cargo run --release
• PUMPKIN_WAVE=N starts at the shop right before wave N with 2,000 candy, every weapon unlocked, and one of each power-up on the ground.
• PUMPKIN_GOD=1 makes you take no damage.
• Practice runs don't count toward saved best scores.
• To turn them off: Remove-Item Env:PUMPKIN_GOD, Env:PUMPKIN_WAVE
Play checklist
These were checked in scripted playtests: menus, settings saving, best scores, damage markers, scope and red dot, shop preview and purchases, bats, damage numbers, health bars, grenades, and goo. The items marked (untested) have only been checked by unit tests or not at all, so look at those first.
Basics
[ ] Move, sprint, jump. You can't walk through stalls, hay bales, or fences.
[ ] Fire, reload with R, switch with 1-4 and the mouse wheel.
[ ] Hold right mouse to aim down sights. The scope overlay shows on scoped weapons.
[ ] F swings the pitchfork. G throws a candy grenade.
[ ] Pick up each power-up: double damage, speed, candy magnet.
Enemies
[ ] Walkers, Runners, Brutes, and Lobbers path around props instead of getting stuck on them.
[ ] Bats fly in, dive-bomb, and get stunned for a moment after a miss (untested).
[ ] Health bars and damage numbers show over hurt enemies. Goo stays on the ground after kills.
Bosses (use PUMPKIN_WAVE=5, 10, 15)
[ ] Pumpkin King's ground slam sends out a shockwave you can jump over (untested).
[ ] Witch fires homing wisps and teleports away when you hurt her (untested).
[ ] Golem charges, gets stunned when it crashes into a prop, and splits into two Gourdlings when it dies (untested).
Shop and attachments
[ ] Press B between waves. Buy a weapon, an upgrade, and an attachment, and check that it shows on the gun.
[ ] Ammo specials work in a real fight: burn, caramel slow, cluster (untested).
Feel and settings
[ ] Sound plays: shots, hits, music (untested; the test container has no sound card).
[ ] Screen shake and the red damage direction marker point the right way.
[ ] Fullscreen on/off works (untested).
[ ] Easy, Normal, and Hard change enemy health and damage. Each keeps its own best score.
[ ] Balance: play a normal run without god mode and note where it gets too easy or too hard (untested).
If something goes wrong
Symptom
Fix
linker 'link.exe' not found
Install Visual Studio Build Tools with the Desktop development with C++ workload, then open a new terminal.
requires rustc 1.97.1 or newer
Run rustup update stable.
package ID specification 'Purgarator' did not match any packages
Your IDE run configuration is using the folder name. Change its command to run --release (the package is pumpkin-purge).
Dozens of no method named random_range found errors
rand got bumped to 0.10. Set rand = "0.9" in Cargo.toml and keep the Cargo.lock from the zip. Don't accept IDE prompts to upgrade it.
file not found for module 'xyz'
main.rs names a file that's missing or misspelled in src/. Check the file list in Step 2.
Lots of errors inside one file, or unexpected end of file
That file was cut off when you pasted it. Compare its line count with the table in Step 2 and copy it again.
cannot find ... in module errors across many files
One of the 18 files is an older version. Copy all of them again from the same zip.
Window opens black, or a GPU / adapter error
Update your graphics driver.
No sound
Check Master volume and Music volume in settings, and your Windows output device.
Choppy or low frame rate
Use cargo run --release.
target folder is huge
Normal for Bevy (10+ GB). cargo clean deletes it; the next build starts from scratch.
Settings or scores look wrong
Delete the files in %APPDATA%\PumpkinPurge and the game goes back to defaults.
Game starts at a later wave or you can't die
PUMPKIN_WAVE or PUMPKIN_GOD is still set. Remove them as shown in Step 4.
