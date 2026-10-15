# Architecture

melee-hd separates the game's simulation from everything that presents it. The simulation is a library; the player, netplay, tools and bots are frontends to it.

## Simulation and presentation

| Layer | Rule |
| --- | --- |
| Simulation: game state each tick, RNG, physics, action states | Bit-exact with ssbm-rs, checked by [the oracle](oracle.md) |
| Presentation: rendering, audio mixing, HUD drawing, the rendered camera | Free: modern pipelines, any aspect ratio, upscaling |
| Gameplay changes: UCF, Slippi's codes | Rulesets, recorded in replays |

`melee-sim` steps the game: roughly `step(&mut State, Inputs) -> Events`. Sounds, rumble and anything else that leaves the simulation are events it returns, never calls it makes. Its state is plain data in arenas with indexed handles, so a rollback snapshot is a clone.

In the original, rendering feeds some game state, and the simulation computes those values itself instead:

- **Off-screen damage.** The original tests visibility in the fighter's draw callback against the camera's scissor; the next tick deals 1% per 60 off-screen frames, up to 150%. melee-hd tests it against a fixed 4:3 gameplay camera, as Slippi's widescreen code does, so the rendered view can take any shape. The Slippi ruleset uses fixed stage limits, as Slippi's online play does.
- **Camera snapshots.** The game reads back a camera-mode snapshot's EFB copy to save it. The frontend renders that one image on request and hands it back as an input.
- **Disc timing.** Loads finish outside the simulation; where the original polls a read (Pokémon Stadium's transformations), its completion is an input.

The audio driver's bookkeeping is simulation too. Playing a sound returns a voice handle that game code stores (a fighter keeps its sounds' handles to stop them later): HAL's AX driver builds it from a running counter and the virtual voice it assigns, and which voices are free depends on the driver ending them as their command streams finish, on the audio interrupt's schedule. So melee-sim runs the driver's voice allocation, its counter and its per-audio-frame command processing on the schedule the original's audio interrupts keep relative to engine ticks, and hands what plays to the presentation layer as events; only mixing and output are presentation. That keeps handles bit-exact without treating audio hardware as an input, which rollback could not replay.

Which voices are free depends on more than the driver: an effect plays on one or two of the AX library's 64 voices, which the synth acquires by priority, steals from lower priorities when none is free, and loses when the DSP's cycle budget for a frame runs out, and each loss ends the driver's voice. So the synth's nodes and the AX library's voice allocation are simulation too (`melee_sim::audio`). The schedule is ssbm-rs's: it keeps virtual time that moves at wait points and by a few ticks per time base or register read, and takes interrupts there. In play every audio frame lands at a wait point between ticks, and which ticks it falls between follows from a few clocks (video fields, the 1/60 s pad sampling alarm, the 5 ms audio DMA, the DSP stand-in's answer) and the scene loop's waits, which melee-sim runs as plain data (`audio::clock`). That reproduces ssbm-rs's tick start times and audio frames exactly over whole matches. Three decisions settle what that schedule is:

1. **The clock is ssbm-rs's virtual hardware clock**, not the console's (whose audio interrupts land wherever real CPU time puts them): the same wait steps, register read costs, handler costs and disc latency, so the oracle can check it frame for frame. How fast the disc reads is a match setting a recording carries too: Slippi's fast disc speed, which ssbm-rs's traces of replays use, or a drive's pace, which ssbm-rs's window and the player use and the music needs ([the engine notes](engine.md)).
2. **Both of ssbm-rs's DSP models are a match setting a recording carries**, as ssbm-rs's recordings carry their sound flag: silent (a voice ends only when keyed off, stolen or dropped; ssbm-rs's `AX=0`) and sample-accurate (the DSP also walks each voice through its samples and ends it with them, as the console does; ssbm-rs's default). They end voices at different times, so they hand out different handles. Sample-accurate is the model, for play and for a match from a Slippi replay alike: Slippi Dolphin's DSP played the recording's samples, and ssbm-rs's runs mix by default. Silent stays available as an explicit setting. Its cost falls on the oracle: with voices advancing, audio memory changes every tick, so a state trace with memory is about twice the size of a silent one (about 525 MB for a whole match). Sample-accurate needs only the walk through each voice's samples (positions, loops, ends), not mixing.
3. **Without ssbm-rs's old cancelled-alarm bug.** On the console a cancelled or re-set alarm leaves the OS's queue and a stopped audio DMA raises no more interrupts. ssbm-rs used to leave their events queued, where a disc read's 10 s timeout cut a wait short and moved later audio frames. ssbm-rs fixed that in `ab3cd01`, and the clock takes such events back the same way.

A tick's fighter animation loads out of ARAM take time on it too: an audio frame that comes due during one runs between its 4 KB chunks or as its last one ends, where ssbm-rs's interrupt handlers return ([the engine notes](engine.md#audio), "When an event runs").

The original's display traversal (`HSD_GObjRender` over the GX links, down through joints, display objects and shapes) changes game state as it draws. A joint's particle list spawns its particles the first time it is displayed, the particle system draws from the game's RNG, and setup computes joint and envelope matrices. So the display pass is simulation. melee-sim runs it where the original does, after a frame's engine ticks, and where the original issues GX commands it appends draw commands to the frame's output. The presentation layer turns those commands into GPU work, at any resolution or aspect ratio, but never decides what the pass runs. [The player](player.md) describes that layer.

## Exactness

The original's float behavior matters only where it differs from IEEE single and double precision: fused multiply-adds, `frsqrte` and `fres` estimates, and paired singles. Those go through `gekko-fp` (melee-hd builds a faster copy of ssbm-rs's, checked against the unchanged one; see [performance](performance.md)); plain arithmetic is native. The game's own `sqrtf`, sin, cos and atan replace platform math, which also makes the simulation agree across platforms. ssbm-rs's two float modes, Hardware (the console) and Slippi (Ishiiruka's x86-64 JIT, which recorded Slippi's replays), both remain available.

## Rulesets

Vanilla is NTSC 1.02 as the console runs it. Slippi adds the codes Slippi tags as affecting gameplay: UCF (0.84, or 0.8 in older replays), neutral spawns, fixed-limit off-screen damage, the camera-independent Pokémon Stadium transition, wobbling prevention, the Ice Climbers' freeze glitch fix, and the online determinism fixes. Lag-reduction codes are not gameplay, and the player's own loop replaces them.

A Slippi replay records the codes it ran, and its rules are the ones those codes make: melee-sim recognizes each code by where it injects and the instructions it runs, and refuses a replay with a gameplay code it doesn't know. Playing a replay back is not a ruleset: it is what Slippi's playback codes do to a match (`game::playback`), with the replay's settings and frames as inputs.

## Netplay and other features

**Decision (2026-10-14): netplay belongs to a follow-on repository.** Sarah's call: melee-hd is the base, exact, fast and complete, and features built on the game (netplay, training tools, mods) belong in a planned follow-on repository that depends on `melee-sim`. A real consumer decides the library's extension points; hooks designed here for imagined users would mostly go unused. Depending on the library rather than forking it keeps a project's accuracy fixes free.

melee-hd stays rollback-ready all the same, because a base can't be made so from outside: a snapshot is a `clone`, a restore and the ticks replayed after it reproduce the straight run bit for bit and allocate nothing, inputs come from outside the simulation, and state only the display pass sets is recomputed when ticks are replayed ([the engine notes](engine.md)). A sync test, on the roadmap, will check this every tick: restore an earlier state, replay the ticks since, and fail on any difference.

The follow-on's netplay is planned as rollback over these snapshots behind two traits, a `Matchmaker` (own server, direct IP, LAN, and Slippi's if the Slippi team agrees) and a wire protocol, with the Slippi ruleset as the online default. Neither repository connects to Slippi's matchmaking servers until the Slippi team is asked.
