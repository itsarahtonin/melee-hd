# melee-hd

Super Smash Bros. Melee NTSC 1.02 (GALE01 revision 2) rebuilt in idiomatic Rust, with a simulation that stays bit-exact with the console.

melee-hd is the follow-on to [ssbm-rs](https://github.com/itsarahtonin/ssbm-rs), a faithful port that keeps the GameCube's memory layout and checks every function against the original machine code. melee-hd rewrites the game from scratch into native structures and checks its simulation against ssbm-rs at the [`melee-hd-base`](https://github.com/itsarahtonin/ssbm-rs/releases/tag/melee-hd-base) tag, tick by tick, and against Slippi replays.

**Status:** one character on one stage. Fox on Final Destination simulates bit-exact with ssbm-rs over a whole match: the Fox–Fox benchmark match (4:20, a four-stock match whose every move both players' level-9 CPUs chose) compares equal in every checked field of the game's state after each of its 15,660 ticks, in the console's floating point, and plays natively in sync with its replay from the first frame to the last. That covers Fox's moveset and Illusion, blaster and reflector, grabs, throws and the held fighter's states, clanks, powershields, the rapid jab, taunts, ledges, KOs past the side and bottom blast zones and as stars off the top, their stats, and respawning on the platform. Sheik is in progress (the Fox–Sheik replay compares clean through its first 292 ticks), and Marth's creation is ported. Not yet playable: there are no menus, no renderer or audio output for the display pass's commands, and most characters and stages aren't ported.

Ported and checked bit for bit against ssbm-rs: the RNG and math routines; the SDK's and HAL's vectors, matrices and quaternions; the game's vector and hit-collision geometry; the stage collision library; controllers and rumble; and HAL's engine: the GObj scheduler, animation (AObj, FObj, expression bytecode, splines), joints with inverse kinematics and constraints, display objects with their materials, textures and shapes, cameras, lights, fog, and the particle system (the display pass's drawing aside). Of the game, the stage system, the game camera, the effects (efLib, efSync, efAsync and efAlt, every effect's case and the effect files), the fighter core, fighter creation (Fox's and Marth's), and the VS match around them: the player blocks, the match's clock, pausing, outcomes, respawns and setup, the button maps and the scene loop; and Slippi replay playback, with the Slippi ruleset's codes the ported code reaches (UCF 0.84's pad buffer and SDI, neutral spawns, stage-limit off-screen damage). Each port's tests drive it through real disc data and random operations, compare every field after every step, and are mutation-checked. Game states captured from ssbm-rs can be imported and compared, which is how game code gets checked as it lands, and `melee-oracle diff` compares the whole state with ssbm-rs's trace of a replay tick by tick. `docs/port-ledger.md` counts what's accounted for.

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
