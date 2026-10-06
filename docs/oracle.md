# The oracle

melee-hd's simulation is checked against ssbm-rs at the `melee-hd-base` tag, whose ports are verified function by function against the original machine code. The oracle comes before any porting: every ported system is checked by it from its first commit.

## Levels

| Level | Reference | Compares | Status |
| --- | --- | --- | --- |
| Replay events | Slippi replays, which ssbm-rs reproduces | Slippi's pre-frame, post-frame, item and frame-start events, field by field, through the timeline rollback settled on | `melee-oracle compare` |
| Full state | ssbm-rs running the same inputs | Fighters, items, stage, RNG and camera every tick, read from ssbm-rs's memory into the same structs melee-hd's state produces | Next |
| Leaf functions | ssbm-rs's ports | Math, collision geometry and other code whose inputs and outputs map directly | Later |

**Replay events.** ssbm-rs plays the 40-replay smoke set to the same recordings as the original code, with documented entry-frame and off-screen-flag differences in 12. A melee-hd recording is compared with the original replay through ssbm-rs's own comparison (`ssbm_slippi::compare`). This covers VS matches only, and only what Slippi records.

**Full state.** Replays leave out most of the game: menus, single-player modes, stage objects, the camera and most fighter fields. ssbm-rs will run a replay or an input recording headless and write the chosen state each tick; melee-hd writes the same records, and a diff reports the first tick and field that differ. Function-level lockstep doesn't carry over, because melee-hd's data no longer has the original's layout.

## Rules

- Floats compare by bit pattern. One ULP is a divergence.
- External events (a disc read completing, a camera snapshot) are inputs, recorded and replayed like controllers, never compared as state.
- A known difference must be explained and listed per replay, as ssbm-rs's known-divergence files are; a blanket exception is never accepted.
