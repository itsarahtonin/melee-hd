# melee-hd

Super Smash Bros. Melee NTSC 1.02 (GALE01 revision 2) rebuilt in idiomatic Rust, with a simulation that stays bit-exact with the console.

melee-hd is the follow-on to [ssbm-rs](https://github.com/itsarahtonin/ssbm-rs), a faithful port that keeps the GameCube's memory layout and checks every function against the original machine code. melee-hd rewrites the game from scratch into native structures and checks its simulation against ssbm-rs at the [`melee-hd-base`](https://github.com/itsarahtonin/ssbm-rs/releases/tag/melee-hd-base) tag, tick by tick, and against Slippi replays.

**Status:** early. The game's random numbers and math routines (sin, cos, tan, sqrt, atan, atan2, acos, asin) are ported and match ssbm-rs bit for bit; the game itself is not ported yet.

## Goals

- **A simulation crate.** A tick is a pure function of the state and the controllers, with no rendering, audio, clocks or disc timing inside it, and its state is plain data that clones cheaply for rollback.
- **Bit-exact with the console.** Float math follows the original instruction sequences where it fused, estimated or used paired singles. Slippi replays play back exactly.
- **Rulesets.** Vanilla (the console as it is) and Slippi (Vanilla plus the gameplay codes Slippi's online play runs), recorded in replays.
- **A modern player.** Low input latency, resolution scaling, widescreen with a 4:3 gameplay camera, and precompiled pipelines.
- **Later:** rollback netplay designed to be compatible with Slippi Dolphin, training tools, mods.

[Architecture](docs/architecture.md) explains the design and [the oracle](docs/oracle.md) how it is checked.

## Game data

Players supply their own disc image. The repository contains no disc image, extracted assets, captured memory or replays, and never will.

## License and provenance

Project code is offered under GPL-3.0-or-later; see [LICENSE](LICENSE). It is ported from ssbm-rs and the [Melee decompilation](https://github.com/doldecomp/melee). The decompilation has no stated license for the original game code, and porting it does not establish ownership of that code. Super Smash Bros. Melee and its assets belong to Nintendo and HAL Laboratory. The project's license does not grant rights to those assets or resolve the original code's licensing.
