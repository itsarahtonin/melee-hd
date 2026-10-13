# melee-hd

melee-hd is a native, idiomatic Rust rewrite of Super Smash Bros. Melee NTSC 1.02 (GALE01 revision 2), whose game simulation is checked bit for bit, tick by tick, against a port verified against the original and against real Slippi replays.

It is the second stage of a two-part project. The first, [ssbm-rs](https://github.com/itsarahtonin/ssbm-rs), is a faithful port: it keeps the GameCube's memory layout and checks its Rust functions call by call against the original machine code. melee-hd starts over from the [Melee decompilation](https://github.com/doldecomp/melee) and ssbm-rs, rewrites the game into plain Rust data structures, and uses ssbm-rs (at its [`melee-hd-base`](https://github.com/itsarahtonin/ssbm-rs/releases/tag/melee-hd-base) tag) as the oracle it is checked against.

melee-hd is early. It plays VS matches between a handful of characters on the six tournament stages, on Windows, from your own disc. It is not yet the whole game.

## Why another Melee port

There are already good ways to play Melee natively. [melee-pc](https://github.com/999sian/melee-pc) builds the decompilation's C source on the Aurora compatibility layer with a WebGPU renderer; its README describes every mode booting and playing, builds for Linux, Windows and Android with more platforms in progress, and online play with rollback, which it marks as partial. [YAMPP](https://github.com/sonsegajp/YAMPP) and [melee-unlocked](https://github.com/Hero88go/melee-unlocked) are also native ports built from the decompilation, and [nikhilunni/melee](https://github.com/nikhilunni/melee) is a Rust port checked against the original running in Dolphin. If you want to play the whole game natively today, start there.

melee-hd aims at something narrower, as a foundation:

- **Bit-exact, and checked.** The simulation is compared with ssbm-rs's after every engine tick over whole matches: every field the comparison covers, floats by bit pattern. It plays real Slippi replays in Slippi Dolphin's floating point, and CPU-played benchmark matches in the console's. A difference is a bug, or a known divergence listed and explained per replay; there are no blanket exceptions.
- **Idiomatic Rust, built for rollback.** The original C isn't compiled or wrapped. Each function is rewritten into Rust whose state is plain data: arenas with index handles, and no references, closures or trait objects. A rollback snapshot is a `clone`, a restore is `clone_from`, and a restore and the ticks replayed after it allocate nothing.
- **A reusable simulation.** `melee-sim` steps the game as a function of its state and the controllers, with no rendering, audio output, clocks or disc timing inside a tick. The player, the checking tools and the benchmark are frontends to it, and netplay and bots can be too.
- **A low-latency player.** The player reads the controllers just before each tick and presents the frame it produces at once, with one frame of latency. Its renderer draws at any resolution and aspect ratio while the gameplay camera stays 4:3, so off-screen rules don't change.
- **No game data in the repository.** The game's data comes from your own disc image when you play; there are no models, textures, sounds or music here.

| | What it is | Relation to melee-hd |
| --- | --- | --- |
| [doldecomp/melee](https://github.com/doldecomp/melee) | A matching C decompilation of the game | Names, structure and intent for every port |
| [ssbm-rs](https://github.com/itsarahtonin/ssbm-rs) | A faithful Rust port in the original memory layout, verified against the machine code | What each function computes, and the oracle melee-hd is checked against; no ssbm-rs code runs in the player |
| melee-hd | A native rewrite in idiomatic Rust | This repository |

## Status

| | |
| --- | --- |
| Characters | Fox, Falco, Marth, Sheik and Zelda, Captain Falcon, each checked over whole matches; Ganondorf, checked function by function until a replay with him can be compared |
| Stages | Final Destination, Battlefield, Pokémon Stadium (transformations included), Yoshi's Story, Dream Land, Fountain of Dreams |
| Whole matches compared | 11 matches, 138,383 ticks, every checked field equal after every tick: four CPU-played benchmark matches (Fox ditto; Fox against Marth on three stages) and seven real Slippi replays |
| Player | A VS match in a window, with a launcher for picking characters, stage, stocks and time. Frames are identical to ssbm-rs's on every checked frame except Slippi's name plates; the audio mix is sample-exact with ssbm-rs's on the matches checked |
| Functions | 5,143 of the 19,827 in ssbm-rs's symbol table ported and checked, 197 replaced by native subsystems, 2 dead ([port ledger](docs/port-ledger.md)) |
| Speed | The simulation runs a match about 16.7 times as fast as headless Dolphin on one core; a snapshot is about 0.8 MB, cloned in about 0.1 ms ([performance](docs/performance.md)) |

What's missing: the other characters and stages, menus and every mode other than VS, items spawning during a match (matches play with items off), netplay, and testing on macOS and Linux. There are no release builds yet. [The oracle](docs/oracle.md) lists each comparison, and [the player](docs/player.md) its open issues.

## Build and play

You need:

- Rust through [rustup](https://rustup.rs); [rust-toolchain.toml](rust-toolchain.toml) pins the toolchain. On Windows, the Visual Studio C++ build tools. The first build fetches ssbm-rs's crates from GitHub, which the checking tools use.
- Your own disc image of Melee NTSC 1.02 (game ID GALE01, revision 2): ISO, RVZ, CISO or GCZ. Other revisions are refused.

```sh
cargo build --locked --release -p melee-player
target/release/melee-hd --disc path/to/melee.iso
```

(`melee-hd.exe` on Windows.) `SSBM_DISC` can name the disc instead of `--disc`. With no match on the command line the player opens on its launcher; Escape returns to it from a match, and quits from the launcher. A match can also be named directly:

```sh
melee-hd --p1 fox --p2 marth --stage bf --stocks 4 --minutes 8
melee-hd --replay game.slp                 # play back a Slippi replay's match
melee-hd --widescreen --scale 3 --msaa 4   # or --aspect 21:9; F11 or Alt+Enter for fullscreen
```

`melee-hd --help` lists the options and the character and stage names. By default the internal resolution follows the window's height.

**Controllers.** GameCube controllers on a Wii U adapter (Nintendo's, or a Mayflash in Wii U mode) take their adapter ports; on Windows the adapter needs the WinUSB driver, which [Zadig](https://zadig.akeo.ie) installs. Standard gamepads and then the keyboard take the first free ports. Keyboard: arrows for the stick, IJKL for the C-stick, X, Z, C and S for A, B, X and Y, D for Z, Q and W for L and R, Enter for Start, left Shift to halve the stick. `--no-adapter` leaves the adapter alone.

## How it's checked

Four levels, from the whole game down ([the oracle](docs/oracle.md)):

- **Full state.** `melee-oracle diff` plays a replay in melee-hd and compares the whole game state with ssbm-rs's state trace of the same replay after every engine tick, stopping at the first difference or at the first function not yet ported.
- **Replay events.** `melee-oracle compare` checks a recording against a Slippi replay, event by event.
- **Captured states.** Game code runs from real mid-match states captured in ssbm-rs, on both sides, and the whole state is compared.
- **Leaf functions.** `leaf-check` calls each port and ssbm-rs's on special, random and in-range inputs in both float modes.

Presentation is checked the same way: `melee-render-check` compares frames with ssbm-rs's draw by draw, and `melee-audio-check` compares the mix sample by sample. Each check is mutation-tested as it is written: a port broken on purpose must fail it.

The disc-backed checks need `SSBM_DISC`. Many also need traces, captures and replays made with ssbm-rs, which contain game memory and stay out of the repository; without them those checks skip.

## Roadmap

The core game comes first: a complete, exact and fast simulation with a player around it, solid enough to build on.

1. **The rest of the cast and stages,** each checked over whole matches the way the current ones are.
2. **The whole game:** items, the boot, menus, settings and the single-player modes, with the Vanilla and Slippi rulesets.
3. **The player on Windows, macOS and Linux,** with release builds. Minimum latency is the default; an added delay for those who want the feel of playing on a CRT will be an option, never the default.
4. **Speed targets:** at least 20 times Dolphin's speed per core, and a rollback (restore and 8 ticks) under 4 ms on the worst match ([performance](docs/performance.md)).
5. **Rollback netplay.** Either integrated with Slippi, with the Slippi team's permission, or a bespoke implementation if not. Until then, melee-hd won't connect to Slippi's matchmaking servers.

Training tools, mods and other add-ons aren't planned yet; they come after the core game is an excellent foundation.

## Contributing

[The porting guide](docs/porting.md) describes how a function gets from ssbm-rs into melee-hd and how it is checked; [the architecture](docs/architecture.md) and [the engine notes](docs/engine.md) explain the design, and [AGENTS.md](AGENTS.md) holds the repository's rules. Two rules matter most: never commit disc images, extracted files, captures or replays, and never port from nikhilunni/melee's source, which has no license.

## License and legal

melee-hd's code is offered under GPL-3.0-or-later; see [LICENSE](LICENSE). Parts are ported from or follow Dolphin (GPL-2.0-or-later), as their file headers say.

melee-hd is ported from ssbm-rs and the Melee decompilation. The decompilation has no stated license for the original game code, and porting it does not establish ownership of that code. Super Smash Bros. Melee and its assets belong to Nintendo and HAL Laboratory. This is an unofficial project, not affiliated with or endorsed by them. The repository contains no disc image, extracted assets, captured memory or replays; you need your own copy of the game. The project's license grants no rights to the game's assets and does not resolve the original code's licensing.
