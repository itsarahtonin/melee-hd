# The refactor and the fast renderer

## Status (2026-11-14, `main` at `f8492689`): goals agreed, survey next

Sarah and the coordinator agreed the goals below on 2026-11-14. The next step is a survey, after milestone 6's full verify is green: profile the simulation, measure duplicated code, list the quirk modelling only Vanilla needs, and sketch the draw boundary ([below](#the-survey)). Its numbers set the targets; this page then gains tasks the way [the Slippi ruleset](slippi-ruleset.md) did.

## The goal

melee-hd should be a good base for other people's projects: fast, and easy to read and change.

- Sarah: "by moddability i just mean in terms of this being a really good base repo for anyone else to use as a starting point for whatever projects they want to undergo".
- The means: "ample code cleanup + removing as much 'red tape' as we can, e.g. tech debt from having to perfectly reproduce an old gamecube game", as the netplay-ready simulation and the Slippi compatibility already did. "of course you'd want your basis repo to be very high performance and easy to work with".
- Performance means low-end PCs first.

Exactness doesn't change: every step keeps the oracle identical, done in small slices, with a full verify at milestones ([what must be exact](porting.md#what-must-be-exact)). The oracle is what makes bold restructuring safe here.

## What the refactor does

The simulation is about 520,000 lines of Rust (455,000 of them in `game/`), ported one for one from the decomp's roughly 18,000 functions so each could be checked alone. That shape was right for porting and isn't how a game engine would be designed. The refactor reshapes it without predicting what anyone builds on it: it makes change cheap, and adds no extension points nobody asked for.

1. **Structure.**
   - Registries in place of hardcoded lists: one table each for characters, stages and items, so adding one is an entry and not a hunt through dozens of files.
   - Clear boundaries between subsystems (fighters, items, stages, camera, HUD, menus).
   - All game state in one place a snapshot takes, as rollback already needs.
   - One copy of logic the original duplicated per character, piloted on one character family first and measured for size and speed.
2. **Names.** Rename freely: Sarah, "rename freely. the decomp is wip and renaming things regularly themselves ... we should focus on our own project, not maintaining diffs or easy lookups against the decomp. in fact renames are encouraged for the sake of clarity and clean code". The tools' link to ssbm-rs stays where it is today, in each port's `#[doc(alias = "NAME")]` (the ledger and the oracle's call traces use the decomp's names), so nobody reading the code has to care. Renames conflict with every branch in flight, so they go in waves, one module at a time, while few branches are open.
3. **A lean engine under the Slippi ruleset.** A `.slp` must still play back exactly in Slippi Dolphin, which runs the original game with codes, so a quirk that reaches gameplay and that no Slippi code fixes stays under the ruleset too: the Super Scope's one-shot charge reads the scope's object address ([the Slippi ruleset's corpus run](oracle.md), 2026-11-14). Nana's CPU DI, which read a leftover heap address, is one Slippi 3.19 does fix. The rule is to reproduce the quirks gameplay sees, not the machinery behind them: model the one value, not the whole memory layout. The ruleset's first step already measured snapshots 16 to 31% smaller ([the Slippi ruleset](slippi-ruleset.md#what-the-ruleset-makes-moot)).
4. **Simulation speed,** found by profiling, not guessed. Targets come from the survey: ticks per second on the bench and snapshot bytes. It starts by winning back `perf/pool-trim`'s 3.5 to 5.8% ([below](#carried-in)).
5. **The display pass without drawing.** Melee's display pass changes game state (a joint's particles spawn the first time it's displayed and draw from the game's RNG), so melee-sim runs it as simulation ([architecture](architecture.md#simulation-and-presentation)). A state-only mode runs the same walk in the same order, spawning and drawing random numbers, but builds nothing to draw: for rollback's re-simulated ticks, headless bots and benchmarks, where nobody sees the frame.
6. **Gameplay changes as rulesets.** Vanilla, Slippi and the UCF versions become implementations of one set of named hook points, so a new ruleset (or a mod, which is a ruleset recorded in replays) is one module. That also keeps the oracle useful to modders: the base game stays checked against ssbm-rs, and a mod can't silently change Vanilla or Slippi.

### Not in this round

- Speculative mod features: plugin loading, scripting, hook points checked at every step, any number of players, support for new characters. A clean structure is what makes those cheap for whoever wants them later.
- Players' own Gecko codes (Sarah, 2026-10-16; [the engine notes](engine.md)).
- Netplay (the follow-on repository).

## Vanilla, the legacy mode

Vanilla stays as melee-hd's accuracy and compatibility mode, exact to the console, and the Slippi ruleset is the one the lean engine is built for. Sarah: "vanilla stays as our legacy/accuracy/compatibility mode because it's worth preserving a console-original degree of accuracy ... if having to maintain a legacy starts to feel like dragging along a zombie that is hamstringing us i am kinda open to cutting it or spinning it off into its own repo entirely".

So the refactor keeps a list, below, of every change Vanilla's exactness blocked or slowed. If it grows long, that list is the case for freezing Vanilla at a tagged version or moving it to its own repository.

### What Vanilla's exactness cost

(None yet.)

## The fast renderer

A track of its own beside the refactor, for low-end PCs. The design (2026-10-19, deferred then until more of the game was ported):

- **Two renderers.** The exact renderer (it builds the console's GX stream and consumes it as the GameCube's GPU would) stays as the reference, the frames tests' oracle and an "accurate" option. The fast one becomes the default.
- **What it draws from:** the display pass's draws as compact records (a model, its joint matrices, its material) in place of the GX stream. Models are decoded once, at load, into GPU buffers; the GPU does the transforms, skinning and lighting; shaders are specialized to each material; the CPU's work per frame is matrices and draw records. High refresh blends matrices.
- **What may differ:** vertex float math, the 1/16 snap, depth precision, later enhancements. **What may not:** what's drawn, the order, materials, blending, effects. It is checked against the exact renderer by geometry within a tolerance and perceptual image comparisons.
- **Why:** today the CPU's frame building is the limit (about 3 ms of the game thread on Fountain of Dreams, [performance](performance.md#rendering-speed)), and the fast renderer removes it.
- **Milestones:** a mesh cache, GPU transforms, the whole scene, high refresh by matrix blending, optional enhancements.

The draw records are the one seam with the refactor. Changing what the display pass hands out must leave game state identical, which the oracle checks, so that part waits for a green baseline. The renderer's own work is checked against the exact renderer, not ssbm-rs, and needs no verify.

**Test machines:** a MacBook (macOS through Metal, and integrated graphics) and the worker PC (a mid-range point, Windows and Linux). The low-end target sits between them; a deliberately throttled run can stand in for a true low-end machine.

## Carried in

- **`perf/pool-trim`'s cost** (Sarah, 2026-11-13: "merge it and win the speed back later"): 3.5 to 5.8% of match speed. First question: does anything gameplay sees depend on the class-object pools' addresses (a new object reading its address's leftovers)? If nothing does, go back to freely reused slots, compare pointers to these objects by identity, and add a tripwire. The leak fix (finished scenes' objects freed) stays either way.
- **The per-address slots** of the exactness round (`fix/exact-alloc`, 2026-11-12), which Sarah questioned as never shown to be needed for gameplay.
- **Relaxing Vanilla's leftover modelling** ([the Slippi ruleset](slippi-ruleset.md#not-in-this-plan)): measured there; Vanilla's list above decides it.
- **`vs_speed_scale`** (`game/gm/vs.rs`) is misnamed: `gm_8016AE94`'s value only weights the random-item table (`it::spawn::setup`), 1.1 and 1.2 with 3 and 4 players in VS.

## The survey

After the full verify, one agent, read-only apart from its report:

1. Profile the simulation on the bench's matches (Vanilla and the Slippi ruleset): where ticks and snapshots spend their time and bytes.
2. Measure duplicated code: per-character near-copies, and which families would pilot the dedup.
3. List the quirk modelling the Slippi ruleset doesn't need and Vanilla does, with each one's cost.
4. Sketch the registries, the subsystem boundaries, the ruleset hook points and the draw records, with what each touches.
5. Propose tasks with sizes, dependencies and measurable targets, and the design calls for Sarah.
