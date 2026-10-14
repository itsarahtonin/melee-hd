# melee-hd

melee-hd is a native Rust rewrite of Super Smash Bros. Melee NTSC 1.02 (GALE01 revision 2), with a game simulation checked bit for bit, tick by tick, against ssbm-rs and real Slippi replays.

It's a personal project, and the next stage of [ssbm-rs](https://github.com/itsarahtonin/ssbm-rs). ssbm-rs is a faithful port: it keeps the GameCube's memory layout and checks its Rust functions call by call against the original machine code. melee-hd starts over from the [Melee decompilation](https://github.com/doldecomp/melee) and ssbm-rs, rewrites the game into plain Rust data structures, and uses ssbm-rs (at its [`melee-hd-base`](https://github.com/itsarahtonin/ssbm-rs/releases/tag/melee-hd-base) tag) as the reference it is checked against. No ssbm-rs code runs in the player.

melee-hd is early: VS matches between a handful of characters on the six tournament stages, on Windows, from your own disc.

## How it differs

Several native Melee ports exist, each taking its own approach, and you may well want one of them instead: [melee-pc](https://github.com/999sian/melee-pc), [YAMPP](https://github.com/sonsegajp/YAMPP) and [melee-unlocked](https://github.com/Hero88go/melee-unlocked) build on the decompilation's C source, and [nikhilunni/melee](https://github.com/nikhilunni/melee) is another Rust port. melee-hd takes a different approach.

What it's built around:

- **Checked against a reference.** After every engine tick of whole matches, the simulation's state is compared with ssbm-rs's, floats by bit pattern. A difference is a bug, or a known divergence listed and explained per replay.
- **Plain-data state.** The original C is rewritten rather than compiled, into state made of arenas and index handles with no references, closures or trait objects. A rollback snapshot is a `clone`, and a restore and the ticks replayed after it allocate nothing.
- **A separate simulation crate.** `melee-sim` steps the game from its state and the controllers alone, with no rendering, audio output, clocks or disc timing inside a tick. The player and the checking tools are frontends to it.
- **A low-latency player.** It reads the controllers just before each tick and presents the frame at once.

The trade-off is that it's a slow work in progress, all the slower for being helmed by a solo developer working within hobbyist LLM usage limits.

Like ssbm-rs, it contains no game data: the game's models, textures, sounds and music come from your own disc image when you play.

## Status

| | |
| --- | --- |
| Characters | Fox, Falco, Marth and Roy, Sheik and Zelda, Captain Falcon, Ganondorf, Ness, Jigglypuff, Samus, Luigi, Mario, Dr. Mario, Peach, Link, Pikachu and Pichu, each checked over whole matches |
| Stages | Final Destination, Battlefield, Pokémon Stadium (transformations included), Yoshi's Story, Dream Land, Fountain of Dreams |
| Whole matches compared | 33 matches, 433,437 ticks, every checked field equal after every tick, the original allocator's heap included: eleven CPU-played benchmark matches (Fox ditto; Fox against Marth on three stages; Ganondorf against Captain Falcon; three each for Ness and Samus) and twenty-two real Slippi replays (Slippi 3.19's, and ranked 3.16 and 3.17 replays) |
| Player | A VS match in a window, with a launcher for picking characters, stage, stocks and time. Frames are identical to ssbm-rs's on every checked frame except Slippi's name plates; the audio mix is sample-exact with ssbm-rs's on the matches checked |
| Functions | 6,429 of the 19,827 in ssbm-rs's symbol table ported and checked, 197 replaced by native subsystems, 11 dead ([port ledger](docs/port-ledger.md)) |
| Speed | The simulation runs a match about 16.7 times as fast as headless Dolphin on one core; a snapshot is about 0.8 MB, cloned in about 0.1 ms ([performance](docs/performance.md)) |

Not yet: the other characters and stages, menus and every mode other than VS, items spawning during a match (matches play with items off), testing on macOS and Linux, and release builds. [The oracle](docs/oracle.md) lists each comparison, and [the player](docs/player.md) its open issues.

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

The disc-backed checks need `SSBM_DISC`. Many also need traces and captures made with ssbm-rs, which contain game memory and stay out of the repository; without them those checks skip. The six benchmark matches' replays, inputs the CPU AI played in ssbm-rs, are in the repository (`data/bench`), so with ssbm-rs and your own disc you can make their traces and run the benchmark diffs yourself.

## Roadmap

The core game comes first: a complete and exact simulation with a player around it, solid enough to build on.

1. **The rest of the cast and stages,** each checked over whole matches the way the current ones are.
2. **The whole game:** items, the boot, menus, settings and the single-player modes, with the Vanilla and Slippi rulesets.
3. **The player on Windows, macOS and Linux,** with release builds. Minimum latency is the default; an added delay for those who want the feel of playing on a CRT will be an option, never the default.
4. **Speed targets:** at least 20 times Dolphin's speed per core, and a rollback (restore and 8 ticks) under 4 ms on the worst match ([performance](docs/performance.md)).
5. **Rollback-ready, proven.** A sync test restores an earlier state and replays the ticks since, every tick, and fails on any difference from the straight run, so the simulation stays correct for rollback, and its cost measured, without netplay code here.

melee-hd aims to be the base other projects build on: `melee-sim` as a documented library to depend on rather than fork, so they get its accuracy fixes for free. Netplay, training tools, mods and other features belong in a planned follow-on repository built on it, and what that repository needs decides the library's extension points ([the architecture](docs/architecture.md#netplay-and-other-features)). melee-hd won't connect to Slippi's matchmaking servers.

## Contributing

[The porting guide](docs/porting.md) describes how a function gets from ssbm-rs into melee-hd and how it is checked; [the architecture](docs/architecture.md) and [the engine notes](docs/engine.md) explain the design, and [AGENTS.md](AGENTS.md) holds the repository's rules. Two rules matter most: never commit disc images, extracted files, captures or replays (the benchmark matches' replays are the one exception), and never port from nikhilunni/melee's source, which has no license.

## License and legal

melee-hd's code is offered under GPL-3.0-or-later; see [LICENSE](LICENSE). Parts are ported from or follow Dolphin (GPL-2.0-or-later), as their file headers say.

melee-hd is ported from ssbm-rs and the Melee decompilation. The decompilation has no stated license for the original game code, and porting it does not establish ownership of that code. Super Smash Bros. Melee and its assets belong to Nintendo and HAL Laboratory. This is an unofficial project, not affiliated with or endorsed by them. The repository contains no disc image, extracted assets or captured memory, and no replays beyond the benchmark matches' CPU-played inputs; you need your own copy of the game. The project's license grants no rights to the game's assets and does not resolve the original code's licensing.
