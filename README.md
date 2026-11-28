# melee-hd

melee-hd is a native Rust rewrite of Super Smash Bros. Melee NTSC 1.02 (GALE01 revision 2), with a game simulation checked bit for bit, tick by tick, against ssbm-rs, real Slippi replays and recorded play sessions.

It's a personal project, and the next stage of [ssbm-rs](https://github.com/itsarahtonin/ssbm-rs). ssbm-rs is a faithful port: it keeps the GameCube's memory layout and checks its Rust functions call by call against the original machine code. melee-hd starts over from the [Melee decompilation](https://github.com/doldecomp/melee) and ssbm-rs, rewrites the game into plain Rust data structures, and uses ssbm-rs (at its [`melee-hd-base`](https://github.com/itsarahtonin/ssbm-rs/releases/tag/melee-hd-base) tag) as the reference it is checked against. No ssbm-rs code runs in the player.

melee-hd plays the whole game on Windows and Linux, from your own disc: it boots straight into Melee as a console does, with your memory card's save, and every menu and mode works, VS with the whole cast on all 29 stages, items and computer players included, and the single-player modes (Classic, Adventure, All-Star, Event Match, Target Test, the Home-Run Contest, Multi-Man Melee and Training). Esc opens melee-hd's own settings over the game.

## How it differs

Several native Melee ports exist, each taking its own approach, and you may well want one of them instead: [melee-pc](https://github.com/999sian/melee-pc), [YAMPP](https://github.com/sonsegajp/YAMPP) and [melee-unlocked](https://github.com/Hero88go/melee-unlocked) build on the decompilation's C source, and [nikhilunni/melee](https://github.com/nikhilunni/melee) is another Rust port. melee-hd takes a different approach.

What it's built around:

- **Checked against a reference.** After every engine tick of whole matches and whole play sessions from power-on, the simulation's state is compared with ssbm-rs's, floats by bit pattern. A difference is a bug, or a known divergence listed and explained.
- **Plain-data state.** The original C is rewritten rather than compiled, into state made of arenas and index handles with no references, closures or trait objects. A rollback snapshot is a `clone`, and a restore and the ticks replayed after it allocate nothing.
- **A separate simulation crate.** `melee-sim` steps the game from its state and the controllers alone, with no rendering, audio output, clocks or disc timing inside a tick. The player and the checking tools are frontends to it.
- **A low-latency player.** It reads the controllers just before each tick, starts each tick just in time for the refresh that shows it, and draws frames at your display's own rate, whatever it is, by predicting the next tick, which adds no latency.

The trade-off is that it's a slow work in progress, all the slower for being helmed by a solo developer working within hobbyist LLM usage limits.

Like ssbm-rs, it contains no game data: the game's models, textures, sounds and music come from your own disc image when you play.

## Status

| | |
| --- | --- |
| The game | From power-on: the memory card scene, the opening movie, the title and every menu; VS (Melee, Special Melee's modes, Tournament, Camera Mode and the rest); the single-player modes, Classic, Adventure and All-Star with their intros, bosses, bonus stages, endings and staff roll, Event Match's 51 events, Target Test, the Home-Run Contest, Multi-Man Melee's six modes and Training; the trophies, records and unlocks, saved to your memory card as Melee saves them |
| Characters | The whole cast, Kirby with every copy ability, each checked over whole matches |
| Stages | All 29 VS stages and the single-player modes' stages |
| Checked | Every function in ssbm-rs's symbol table accounted for: 17,995 ported and checked, 1,007 replaced by native subsystems and 825 proven dead, of 19,827 ([port ledger](docs/port-ledger.md)). Whole matches from CPU-played benchmark replays and real Slippi replays, and hundreds of recorded sessions from power-on through the menus and modes, compare clean with ssbm-rs every tick ([the oracle](docs/oracle.md)) |
| Player | Melee in a window or fullscreen at any resolution and aspect ratio, widescreen included; frames identical to ssbm-rs's on every checked frame (but for Slippi's replay name plates, which melee-hd doesn't draw), and the audio mix sample-exact with ssbm-rs's on the matches checked. Your save is Melee's own file, which Dolphin uses too, to import and export. Each VS match is saved as a Slippi replay (`.slp`), and each session can be recorded to report a bug ([the player](docs/player.md)) |
| Speed | The simulation runs a match about 13 to 15 times as fast as headless Dolphin on one core; a snapshot is about 1.45 MB, cloned in about 0.14 ms and restored in about 0.04 ms ([performance](docs/performance.md)) |

Not yet: the Slippi ruleset (Slippi's gameplay codes in live play, [in progress](docs/slippi-ruleset.md)), and testing and builds on macOS. [The player](docs/player.md) lists its open issues.

## Build and play

Each [release](https://github.com/itsarahtonin/melee-hd/releases) has a Windows build, `melee-hd.exe`: download it and double-click it. For Linux, download `melee-hd-linux-x86_64.tar.gz`, unpack it and run `melee-hd` in its folder; it needs a 64-bit PC distribution from 2022 or later (Ubuntu 22.04 or newer, or one of its age) with Vulkan graphics drivers, and its dialogs use zenity, which most desktops include. To build it yourself, you need:

- Rust through [rustup](https://rustup.rs); [rust-toolchain.toml](rust-toolchain.toml) pins the toolchain. On Windows, the Visual Studio C++ build tools; on Linux, `pkg-config` and the ALSA and udev development files (`libasound2-dev` and `libudev-dev` on Debian and Ubuntu). The first build fetches ssbm-rs's crates from GitHub, which the checking tools use.
- Your own disc image of Melee NTSC 1.02 (game ID GALE01, revision 2): ISO, RVZ, CISO or GCZ. Other revisions are refused.

```sh
cargo run --locked -p dist                  # builds dist/melee-hd.exe; add -- --shortcut for a desktop shortcut
```

Double-click `dist/melee-hd.exe` (or the shortcut) to play. The first time, it asks for your disc image and remembers it. Then it boots straight into Melee, as a console with a memory card in slot A, and matches start from the game's own menus. Escape opens the settings over the game, paused: the display (resolution, antialiasing, aspect ratio, fullscreen, high refresh), the latency (present mode, added delay, tick timing), your save (import, export, unlock everything), recording, a developer's debug level and the disc. Escape again goes back to the game.

melee-hd keeps everything it writes in its settings folder, `%APPDATA%\melee-hd` on Windows and `~/.config/melee-hd` on Linux: the settings, your save (`saves`), each VS match's replay under the Slippi ruleset (`replays`, unless the settings name another folder; Vanilla saves none, its session recording keeps every match exactly), the session recordings (`sessions`), F12's bug captures (`captures`) and, on Windows, the log (`logs`); its shader caches go to `%LOCALAPPDATA%\melee-hd` (`~/.cache/melee-hd` on Linux). Nothing is written beside the program, so it runs from anywhere, a read-only folder included. Replays and captures older versions saved beside the program stay there, untouched. On Windows it opens no console window. [The player](docs/player.md#playing-it) has the details.

From a terminal, `cargo build --locked --release -p melee-player` builds `target/release/melee-hd` (`melee-hd.exe` on Windows), which prints to the terminal. `--disc path/to/melee.iso` or `SSBM_DISC` names the disc for that run instead of the remembered one. For developers, a VS match or a replay can also be started directly, without the menus:

```sh
melee-hd --p1 fox --p2 marth --stage bf --stocks 4 --minutes 8
melee-hd --replay game.slp                 # play back a Slippi replay's match
melee-hd --widescreen --scale 3 --msaa 4   # or --aspect 21:9; F11 or Alt+Enter for fullscreen
```

`melee-hd --help` lists the options and the character and stage names.

**Controllers.** GameCube controllers on a Wii U adapter (Nintendo's, or a Mayflash in Wii U mode) take their adapter ports; on Windows the adapter needs the WinUSB driver, which [Zadig](https://zadig.akeo.ie) installs. On Linux it needs a udev rule that lets you open it: copy `51-melee-hd-gcadapter.rules` (in the Linux download, and in `crates/melee-player/linux`) into `/etc/udev/rules.d/` (`sudo cp 51-melee-hd-gcadapter.rules /etc/udev/rules.d/`), run `sudo udevadm control --reload-rules`, then unplug the adapter and plug it in again. Standard gamepads and then the keyboard take the first free ports. Keyboard: arrows for the stick, IJKL for the C-stick, X, Z, C and S for A, B, X and Y, D for Z, Q and W for L and R, Enter for Start, left Shift to halve the stick. `--no-adapter` leaves the adapter alone.

## How it's checked

Five levels, from the whole game down ([the oracle](docs/oracle.md)):

- **Sessions.** `melee-oracle session` boots melee-hd from power-on with a play session's recorded controller reads and compares the whole game state with ssbm-rs's trace of the same session after every engine tick: the boot, the menus, the modes and the memory card, stopping at the first difference.
- **Full state.** `melee-oracle diff` plays a replay in melee-hd and compares the whole game state with ssbm-rs's state trace of the same replay after every engine tick.
- **Replay events.** `melee-oracle record` plays a Slippi replay with melee-hd's recorder on and checks its recording against the replay, event by event (`melee-oracle compare` checks any two).
- **Captured states.** Game code runs from real mid-match states captured in ssbm-rs, on both sides, and the whole state is compared.
- **Leaf functions.** `leaf-check` calls each port and ssbm-rs's on special, random and in-range inputs in both float modes.

Presentation is checked the same way: `melee-render-check` compares frames with ssbm-rs's draw by draw, and `melee-audio-check` compares the mix sample by sample. Each check is mutation-tested as it is written: a port broken on purpose must fail it.

The disc-backed checks need `SSBM_DISC`. Many also need traces, captures and recorded sessions made with ssbm-rs, which contain game memory and stay out of the repository; without them those checks skip. The six benchmark matches' replays, inputs the CPU AI played in ssbm-rs, are in the repository (`data/bench`), so with ssbm-rs and your own disc you can make their traces and run the benchmark diffs yourself.

## Roadmap

The core game came first: a complete and exact simulation with a player around it, solid enough to build on. The whole cast, the stages, items, the boot, the menus and the single-player modes are in. Next:

1. **The Slippi ruleset:** Slippi's gameplay codes in live play, without a memory card and with Slippi's unlocks, as an option beside Vanilla, its replays playing back exactly in Slippi Dolphin ([the plan](docs/slippi-ruleset.md)).
2. **Speed and structure:** a round of performance and code-structure work at the same exactness, toward at least 20 times Dolphin's speed per core and a rollback (restore and 8 ticks) under 4 ms on the worst match ([performance](docs/performance.md)).
3. **The player on Windows, macOS and Linux,** with release builds. Minimum latency is the default; an added delay for those who want the feel of playing on a CRT is an option, never the default.
4. **Rollback-ready, proven.** A sync test restores an earlier state and replays the ticks since, every tick, and fails on any difference from the straight run, so the simulation stays correct for rollback, and its cost measured, without netplay code here. It exists (`melee-oracle sync-test`, [the oracle](docs/oracle.md#sync-test)), and every checked match passes it with 7-tick rollbacks; it has to keep passing as the game changes.
5. **The player's polish:** controller rebinding and port assignment, box controllers and PhobGCC's processing for stock controllers, settings reachable from a controller, macOS builds, replay viewer controls and more presentation options ([the plan](docs/milestone-7.md)).

melee-hd aims to be the base other projects build on: `melee-sim` as a documented library to depend on rather than fork, so they get its accuracy fixes for free. Netplay, training tools, mods and other features belong in a planned follow-on repository built on it, and what that repository needs decides the library's extension points ([the architecture](docs/architecture.md#netplay-and-other-features)). melee-hd won't connect to Slippi's matchmaking servers.

## Contributing

[The porting guide](docs/porting.md) describes how a function gets from ssbm-rs into melee-hd and how it is checked; [the architecture](docs/architecture.md) and [the engine notes](docs/engine.md) explain the design, [Melee's own bugs](docs/melee-bugs.md) catalogs the original game's defects melee-hd keeps for exactness, [Slippi's own defects](docs/slippi-bugs.md) those of Slippi it reproduces for exact `.slp` recordings, and [AGENTS.md](AGENTS.md) holds the repository's rules. Two rules matter most: never commit disc images, extracted files, captures or replays (the benchmark matches' replays are the one exception), and never port from nikhilunni/melee's source, which has no license.

## License and legal

melee-hd's code is offered under GPL-3.0-or-later; see [LICENSE](LICENSE). Parts are ported from or follow Dolphin (GPL-2.0-or-later), as their file headers say.

Builds that include PhobGCC's processing (`crates/melee-phob`, ported from PhobGCC-SW, GPL-3.0-only) are distributed under GPL-3.0.

melee-hd is ported from ssbm-rs and the Melee decompilation. The decompilation has no stated license for the original game code, and porting it does not establish ownership of that code. Super Smash Bros. Melee and its assets belong to Nintendo and HAL Laboratory. This is an unofficial project, not affiliated with or endorsed by them. The repository contains no disc image, extracted assets or captured memory, and no replays beyond the benchmark matches' CPU-played inputs; you need your own copy of the game. The project's license grants no rights to the game's assets and does not resolve the original code's licensing.
