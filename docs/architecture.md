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

## Exactness

The original's float behavior matters only where it differs from IEEE single and double precision: fused multiply-adds, `frsqrte` and `fres` estimates, and paired singles. Those go through `gekko-fp`; plain arithmetic is native. The game's own `sqrtf`, sin, cos and atan replace platform math, which also makes the simulation agree across platforms. ssbm-rs's two float modes, Hardware (the console) and Slippi (Ishiiruka's x86-64 JIT, which recorded Slippi's replays), both remain available.

## Rulesets

Vanilla is NTSC 1.02 as the console runs it. Slippi adds the codes Slippi tags as affecting gameplay: UCF 0.84, fixed-limit off-screen damage, the camera-independent Pokémon Stadium transition, wobbling prevention, and the online determinism fixes. Lag-reduction codes are not gameplay, and the player's own loop replaces them.

## Netplay

Netplay is designed to play against Slippi Dolphin users, behind two traits: a `Matchmaker` (Slippi, own server, direct IP, LAN) and a wire protocol (Slippi-compatible or native). Two melee-hd clients negotiate the native protocol; Slippi mode is used only against Slippi Dolphin. The Slippi ruleset stays the online default either way.
