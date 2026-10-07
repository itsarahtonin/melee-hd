# The oracle

melee-hd's simulation is checked against ssbm-rs at the `melee-hd-base` tag, whose ports are verified function by function against the original machine code. The oracle comes before any porting: every ported system is checked by it from its first commit.

## Levels

| Level | Reference | Compares | Status |
| --- | --- | --- | --- |
| Replay events | Slippi replays, which ssbm-rs reproduces | Slippi's pre-frame, post-frame, item and frame-start events, field by field, through the timeline rollback settled on | `melee-oracle compare` |
| Full state | ssbm-rs running the same inputs | Fighters, items, stage, RNG and camera every tick, read from ssbm-rs's memory into the same structs melee-hd's state produces | Traces read and checked (`melee-oracle trace-check`); the diff comes with the first ported state |
| Leaf functions | ssbm-rs's ports | Math, collision geometry and other code whose inputs and outputs map directly | `leaf-check`: the RNG, the math routines, the SDK's vectors and matrices, HAL's matrix helpers and quaternions, the game's vector helpers, and its hit collision geometry |

**Replay events.** ssbm-rs plays the 40-replay smoke set to the same recordings as the original code, with documented entry-frame and off-screen-flag differences in 12. A melee-hd recording is compared with the original replay through ssbm-rs's own comparison (`ssbm_slippi::compare`). This covers VS matches only, and only what Slippi records.

**Full state.** Replays leave out most of the game: menus, single-player modes, stage objects, the camera and most fighter fields. ssbm-rs's runner, with `STATE_TRACE=FILE` (branch `feat/state-trace`), writes the game's raw state at the end of every engine tick: the RNG seed, the player blocks, the stage, the camera, and every fighter and item. `melee-oracle trace-check TRACE REPLAY.slp` checks a trace against the replay it ran: each frame's post-frame fighter positions and percents must equal the trace's, bit for bit, one tick per frame. Two full smoke replays (Dream Land, 13,779 frames; Pokémon Stadium, 13,671) pass with no difference; a full match's trace is about 14 MB. Slippi's pre-frame data doesn't line up as exactly, because it is sampled partway through a tick. melee-hd will decode the same regions into its own structs, and a diff will report the first tick and field that differ. Function-level lockstep doesn't carry over, because melee-hd's data no longer has the original's layout.

**Leaf functions.** `leaf-check` loads the disc's `main.dol` into ssbm-rs's runtime, registers every port, and calls them one at a time, so melee-sim's functions are compared with ssbm-rs's input by input, in both float modes. Inputs are special values, random bit patterns and values spread over each function's useful range. Changing one bit of a constant makes the checks fail.

## Rules

- Floats compare by bit pattern. One ULP is a divergence.
- External events (a disc read completing, a camera snapshot) are inputs, recorded and replayed like controllers, never compared as state.
- A known difference must be explained and listed per replay, as ssbm-rs's known-divergence files are; a blanket exception is never accepted.
