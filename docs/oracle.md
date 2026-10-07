# The oracle

melee-hd's simulation is checked against ssbm-rs at the `melee-hd-base` tag, whose ports are verified function by function against the original machine code. The oracle comes before any porting: every ported system is checked by it from its first commit.

## Levels

| Level | Reference | Compares | Status |
| --- | --- | --- | --- |
| Replay events | Slippi replays, which ssbm-rs reproduces | Slippi's pre-frame, post-frame, item and frame-start events, field by field, through the timeline rollback settled on | `melee-oracle compare` |
| Full state | ssbm-rs running the same inputs | Fighters, items, stage, RNG and camera every tick, read from ssbm-rs's memory into the same structs melee-hd's state produces | Traces read and checked (`melee-oracle trace-check`); the diff comes with the first ported state |
| Captured states | ssbm-rs's lockstep captures (memory at a real call) | A game function run from a real game state on both sides, then the whole state | `melee-capture`: imports a captured state into melee-sim (scheduler, joint trees and everything on them, fighters, the stage) and compares melee-sim's state with memory; round trips on real captures come back clean. leaf-check's `stage` tests run Final Destination's and Battlefield's setup and processes from captures |
| Leaf functions | ssbm-rs's ports | Math, collision geometry and other code whose inputs and outputs map directly | `leaf-check`: the RNG, math, the SDK's and HAL's vectors, matrices and quaternions, collision geometry, stage and environment collision, controllers and rumble, and HSD's scheduler, animation, joints, references, display lists and shapes |

**Replay events.** ssbm-rs plays the 40-replay smoke set to the same recordings as the original code, with documented entry-frame and off-screen-flag differences in 12. A melee-hd recording is compared with the original replay through ssbm-rs's own comparison (`ssbm_slippi::compare`). This covers VS matches only, and only what Slippi records.

**Full state.** Replays leave out most of the game: menus, single-player modes, stage objects, the camera and most fighter fields. ssbm-rs's runner, with `STATE_TRACE=FILE` (branch `feat/state-trace`), writes the game's raw state at the end of every engine tick: the RNG seed, the player blocks, the stage, the camera, and every fighter and item. `melee-oracle trace-check TRACE REPLAY.slp` checks a trace against the replay it ran: each frame's post-frame fighter positions and percents must equal the trace's, bit for bit, one tick per frame. Two full smoke replays (Dream Land, 13,779 frames; Pokémon Stadium, 13,671) pass with no difference; a full match's trace is about 14 MB. Slippi's pre-frame data doesn't line up as exactly, because it is sampled partway through a tick. melee-hd will decode the same regions into its own structs, and a diff will report the first tick and field that differ. Function-level lockstep doesn't carry over, because melee-hd's data no longer has the original's layout.

**Captured states.** Game code needs real states to run from: a fighter mid-action, with its joints, animations and stage. ssbm-rs's lockstep captures (`CAPTURE=DIR CAPTURE_FUNCS=NAME,...`) save main memory and registers at real calls. `melee-capture` finds which disc archives are loaded where (by header and contents; the game writes into some, such as fighters' action tables), then imports the state into a `Game`: the GObj scheduler with its processes, every joint tree with its animations, references, display lists and shapes, the fighters through their generated mirrors, and the stage (`stage_info`, the stage objects' user data with their variables typed for the stage, the stage parameters the game writes into, the collision geometry and islands, fog, lights, the game camera, the random seed and HSD's ID table). Afterwards it compares a `Game` with memory, pairing objects by where they're reached from, so objects either side created since pair too. A port runs from the imported state while ssbm-rs runs the call from the same memory, and the two states are compared. Pointers the importer doesn't model yet (static data in the executable, item user data) are listed rather than guessed.

Besides single calls, a captured state can drive longer runs: leaf-check's `last_stage_ticks` runs Final Destination's stage processes alone for 6,400 ticks on both sides from one capture (phases 1 through 8 of its background cycle), `battle_stage_ticks` runs Battlefield's for 9,000, and the `on_state` tests call functions no capture reached on a real stage. Where objects are freed and new ones made, both sides are paired afresh by where objects are reached from, since the heap reuses blocks by address and pools by last freed.

The fighter mirrors come from `tools/structgen`, which generates each game struct with an importer and a field-by-field comparer. `melee-oracle mirror-check TRACE` round-trips every fighter of a state trace through them.

**Leaf functions.** `leaf-check` loads the disc's `main.dol` into ssbm-rs's runtime, registers every port, and calls them one at a time, so melee-sim's functions are compared with ssbm-rs's input by input, in both float modes. Inputs are special values, random bit patterns and values spread over each function's useful range. Changing one bit of a constant makes the checks fail.

## Rules

- Floats compare by bit pattern. One ULP is a divergence.
- External events (a disc read completing, a camera snapshot) are inputs, recorded and replayed like controllers, never compared as state.
- A known difference must be explained and listed per replay, as ssbm-rs's known-divergence files are; a blanket exception is never accepted.
