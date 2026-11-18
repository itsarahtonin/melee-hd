# The refactor and the fast renderer

## Status (2026-11-17, `main` at `f0fa4c81`): surveyed, tasks proposed, design calls open

Sarah and the coordinator agreed the goals below on 2026-11-14. Milestone 6's full verify came back green on `0b835117`, and the survey ran on 2026-11-17 from `main` at `f0fa4c81` ([what it found](#what-the-survey-found)). Its numbers set the [proposed tasks](#proposed-tasks)' targets; the [design calls](#design-calls) are Sarah's before the tasks start.

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
3. **A lean engine under the Slippi ruleset.** A `.slp` must still play back exactly in Slippi Dolphin, which runs the original game with codes, so a quirk that reaches gameplay and that no Slippi code fixes stays under the ruleset too: the Super Scope's one-shot charge reads the scope's object address ([the Slippi ruleset's corpus run](oracle.md), 2026-11-14), which the ruleset keeps as a fixed stand-in address, every heap address taking the same branches, rather than running the allocator model for it. Nana's CPU DI, which read a leftover heap address, is one Slippi 3.19 does fix. The rule is to reproduce the quirks gameplay sees, not the machinery behind them: model the one value, not the whole memory layout. The ruleset's first step already measured snapshots 16 to 31% smaller ([the Slippi ruleset](slippi-ruleset.md#what-the-ruleset-makes-moot)).
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

Measured by the survey (2026-11-17, [quirk modelling by ruleset](#quirk-modelling-by-ruleset) has the whole list):

- **The allocator model's addresses and the pools' per-address slots** (a computer player's DI reads its `Fighter*`): snapshots 16 to 31% larger and growing through the match (Sarah's 3.19 replay at tick 10,000: 2.22 MB with the model, 1.78 MB without; the materials' pool alone 501 against 251 KB), and the per-address lookups' 3.5 to 5.8% of match speed carried in from `perf/pool-trim`. The speed difference of the model alone stayed within this run's noise.
- **The heap's image and the menus' layouts** (`game::leftovers`, `vanilla_vs`, `from_menus`): 138 to 227 KB of a session's snapshot, 40 on/off tests in 28 files and 121 calls into the model in 59 files.
- **r5 through the computer player's frame** (`cpu_ai::Leftover` and the floor checks): bookkeeping in 7 files that runs in every match.

None yet blocks a refactor task; [task 7](#proposed-tasks) asks whether the slots are needed at all.

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

Done on 2026-11-17 ([below](#what-the-survey-found)).

## What the survey found

Surveyed on 2026-11-17 from `main` at `f0fa4c81` (milestone 6 done), release builds with line tables (`CARGO_PROFILE_RELEASE_DEBUG=line-tables-only`), on [this machine](performance.md#machine) (Windows). **The machine wasn't quiet**: other agents' builds and oracle runs shared it (the processor 11% busy before the profiles, 37 to 62% around the timings), so the profiles' shares are what to read, and absolute speeds are low and noisy (the Fox ditto ran 4,373 frames/s alone before the timings and 3,454 to 3,831 during them). The scripts are an agent's scratch files, not in the repository; each is described where its numbers are.

**How the profiles were taken.** samply records through ETW on Windows, which needs administrator rights the agents don't have, so a small sampler stands in: a Python script that suspends the simulation's thread 2,000 times a second, walks its stack with dbghelp (`StackWalk64` over the modules' unwind tables) and resumes it, then symbolizes every address from the PDB (the leaf's source line names inlined code's own file). It slows the run about three times (suspending is costly) but samples the code evenly; 5,500 to 12,300 samples a profile on the simulation's thread, the match's setup and disc reads left out. The tick and display pass timings are a separate, unprofiled scratch build that plays the replay through `Match::step_replay_tick` and `Match::render` and times each with `Instant`.

The matches: `fox-fox-fd`, `fox-marth-ps` and `ics-peach-ps` (Nana runs the computer player's code; it leaves its replay at frame 10,139 as always, so it plays to frame 10,986, 11,223 ticks) under Vanilla (`melee-bench native`: the replay's rules, the console's floating point, the allocator model on); the same three under the Slippi ruleset (`melee-bench corpus --rules slippi --heap off --fp slippi`: `Rules::slippi_online`, Slippi's floating point, the model off, the recorded inputs driving a 3.19 match that leaves the recording early, so `fox-fox-fd` runs 9,066 ticks and `fox-marth-ps` fewer); and a real 3.19 match, Sarah's `Game_20260129T130802` (Fox against Sheik on Dream Land, `MELEE_BENCH_FP=slippi MELEE_BENCH_HEAP=off melee-bench native`), in sync throughout.

### Where the simulation's time goes

**The display pass is 21 to 26% of a tick plus its display** (unprofiled, `Instant` around each part): Vanilla `fox-fox-fd` 222 µs a tick and 68 µs a pass (23.5%), `fox-marth-ps` 222 and 78 µs (26.0%), `ics-peach-ps` 260 and 75 µs (22.4%); under the Slippi ruleset `fox-fox-fd` 216 and 67 µs (23.6%) and Sarah's replay 189 to 238 and 53 to 65 µs (21.4 to 21.9%).

**Self time by what the hot paths have in common** (each sample's leaf function, bucketed; ticks and display pass apart; share of the match's samples):

| Self time in | Vanilla `fox-fox-fd` | Vanilla `fox-marth-ps` | Vanilla `ics-peach-ps` | Slippi `fox-fox-fd` | Slippi `ics-peach-ps` | Sarah's 3.19 replay |
| --- | --- | --- | --- | --- | --- | --- |
| **Address dispatch chains** (`code::run_proc`, `run_fighter_cb`, `call`, every module's `run`, `run_event`, `*_run`) | **25.8%** | **19.3%** | **17.8%** | **24.4%** | **22.1%** | **25.1%** |
| Joint and matrix math (`Mtx34::srt`, `concat`, `make_matrix`, world positions) | 13.9% | 12.7% | 15.8% | 10.8% | 12.5% | 10.5% |
| Bit-exact float ops and the game's math (`gekko_fp`, `sin`, `cos`, `atan`, range reduction) | 11.2% | 11.8% | 16.4% | 10.4% | 16.8% | 12.5% |
| Animation interpretation (FObjs, AObjs, splines, texture and shape animation) | 8.1% | 5.9% | 4.7% | 8.9% | 6.8% | 12.0% |
| The display pass's walk and draws (`hsd::display`, the render callbacks) | 6.9% | 8.4% | 6.0% | 10.4% | 5.7% | 8.3% |
| Collision (`mp`) | 3.4% | 3.5% | 5.9% | 3.1% | 3.9% | 1.8% |
| Particles and effects | 2.1% | 3.2% | 2.0% | 1.2% | 1.7% | 0.8% |
| Pools and the allocator model | 1.8% | 2.3% | 1.5% | 1.1% | 0.7% | 0.9% |
| Bone dynamics (`swing_chain`) | 1.5% | 1.5% | 2.0% | 1.0% | 1.5% | 0.9% |
| Archive lookups | 1.2% | 1.4% | 1.1% | 1.2% | 1.3% | 1.2% |
| Everything else (game logic spread over about 2,000 functions) | 23.8% | 28.4% | 25.8% | 25.6% | 25.6% | 24.8% |

**The headline: a fifth to a quarter of the simulation is finding the function a callback names.** A callback in the state is the original's address (`CodeRef(u32)`, 12,034 uses), and calling one walks a chain: `code::run_fighter_cb` asks 37 modules in turn (`fox::run`, `seak::run`, ... `masterhand::run`, ... and the common action states' `ft::co::dispatch::run` 36th), `code::run_proc` 46, `code::run_event` 41 stages, `code::call` 33, and `it::kinds::run_event` about 160 item kinds joined by `||`. Each link is a `match` on the address (or a binary search), so every common action state's callback, every HUD and stage process, runs dozens of them. The samples' leaf lines confirm it is the chain itself: the hottest in this bucket are the tournament's `tou_run`, Target Test's and Kongo Jungle's `run` and the item chain's links, in a Final Destination match with items off. `hud::run_proc` alone is 8% inclusive of the Fox ditto, much of it the chain. No profile since 2026-10-09 ([performance](performance.md#simulation-speed)) saw this, when far fewer modules were ported; the chains grew with every port, which fits the Fox ditto falling from 6,637 frames/s then to about 4,400 now. A table from address to function (one lookup) should win most of it back: [task 1](#proposed-tasks).

The rest is spread thin. The bit-exact float ops are fused and inlined already ([speed work](performance.md#speed-work)); joint matrices, animation and collision are the original's own work. Pools and the allocator model are 0.7 to 2.3% of self time, so the model's own cost is small; what `perf/pool-trim` cost is the lookups' layout ([performance](performance.md#the-exactness-rounds-cost-2026-11-13-the-ubuntu-worker)), which a profile doesn't separate. Bounds checks and copies don't stand out: system copies are 0.8 to 2.1%.

**Speed, the allocator model on and off** (`melee-bench native --runs 5`, two alternating rounds, median frames/s; on a busy machine, so the differences are noise): `fox-fox-fd` 3,726 and 3,551 on, 3,454 and 3,489 off; `fox-marth-ps` 3,310 and 2,837 on, 3,177 and 3,251 off; `ganon-captain-fd` 3,664 and 3,388 on, 3,436 and 3,345 off; Sarah's replay (Slippi float) 3,334 and 3,525 on, 3,337 and 3,512 off. Every run in sync. As on 2026-11-14 the model's speed cost is within noise; the targets below should be measured on the worker or a quiet machine.

### Snapshots

A match's snapshot is `Game` cloned. By part (scratch tool: each of `Game`'s fields and `hsd::Objects`' pools cloned alone, bytes held and median clone time; the busy machine's times, so compare them with each other):

| State | Vanilla `fox-fox-fd`, ticks 5,000 / 10,000 / 15,000 | Sarah's replay, model on, 10,000 | Same, model off |
| --- | --- | --- | --- |
| Whole snapshot, clone | 1.81 / 2.00 / 2.26 MB, 254 / 270 / 335 µs | 2.22 MB, 303 µs | 1.78 MB, 253 µs |
| Materials (`objs.mobjs`) | 371 / 477 KB (21 to 24%), 74 / 101 µs | 501 KB, 84 µs | 251 KB, 51 µs |
| Shapes (`objs.pobjs`) | 190 / 224 KB (11%), 57 / 60 µs | 254 KB, 85 µs | 178 KB, 78 µs |
| Joints, textures, FObjs, display objects | 166, 140, 84, 55 KB at 10,000; 2 to 7 µs each | 221, 172, 97, 58 KB | 214, 164, 89, 57 KB |
| Audio (synth, AX voices, driver) | 113 KB, 4 µs | 122 KB | 122 KB |
| Every scene's and mode's own state carried in a match (tournament 52 KB, debug menu 28, VS Melee 28, 1P 22, endings 17, ...) | 223 KB (11%), 6 µs | | |
| All of HSD's pools | 1.31 MB, 128 µs | | |

**Materials and shapes are half to 60% of a snapshot's clone time** (131 to 169 µs of 250 to 335): each material owns its compiled TEV and each shape its envelope lists as vectors, so a clone allocates and copies each one apart. With the model on the materials' pool also grows by address (501 against 251 KB). The scenes' and modes' state, though 11% of the bytes, copies in 6 µs. Flattening the materials' and shapes' storage is [task 8](#proposed-tasks), done (2026-11-18, `perf/flat-materials`): the compiled TEVs held in the materials and each shape's envelopes in one vector, a match snapshot's clone 41 to 46% faster with 79 to 85% fewer allocations and 6 to 10% more bytes ([performance](performance.md#flat-materials-and-shapes-2026-11-18)).

### The state-only display pass

The display pass is 21 to 26% of a tick plus its display. Inside it (share of the pass's samples, inclusive): the joints' walk and its cache misses lead (`jobj_disp_all`'s flag test and `pobj_disp`'s cull test are the hottest lines), light setup 11 to 17%, shadows' render 7 to 17%, envelope and skin matrices 9 to 14%, the screen visibility marks 8 to 12%, the HUD 8 to 11%, material and texture setup 4 to 6%, Pokémon Stadium's TEV compiles 14 to 15% there, text layout 1 to 8%, particles 1 to 3%.

**A state-only walk as defined (particles spawned, RNG drawn, matrices set, no draws built) saves 0.6 to 2.7% of a tick plus its display**: the `draws.push` statements are 0.2 to 1.0% of the pass, and the functions that only snapshot state for the presentation (`push_light_set`, the text layout for its draws) 1.8 to 8.5% (Pokémon Stadium's text the most). Everything else the pass does writes state the oracle compares: the joint, envelope and texture matrices, the lights' slots, the compiled TEVs, the off-screen marks, the shadows' cameras. Skipping more (light, material, envelope and shadow setup, about 30 to 45% of the pass, 7 to 12% of a tick plus display) would need each written field shown to be presentation-only and rewritten before anything reads it, which holds for most of them in a rollback's re-simulated ticks (the last tick's pass rewrites them) but changes the state those ticks compare. That is [design call 5](#design-calls).

### Duplicated code

A near-copy scan over `crates/melee-sim/src/game` (scratch script: comments, strings and attributes stripped, identifiers and numbers anonymized, every window of N tokens hashed, a line counted when it lies in a window found twice or more; the seven generated files, 71,578 lines, left out). Of 303,254 code lines, 79,483 (26%) lie in a 100-token near-copy and 40,446 (13%) in a 200-token one. By area at 100 tokens: fighters 32% (30,480 of 94,450), items 33% (19,399 of 57,915), stages 23%, the HUD 42% (data tables), modes 15%, menus 16%. Part of that is boilerplate the registries remove (each module's address constants and `run` arms: 7,499 address constants and 3,246 arms) rather than copied logic.

The original's clone characters are already one copy each: Falco, Dr. Mario, Young Link, Roy and Pichu are 70 to 231 lines of their own, their moves their originals' with branches on the kind. The biggest families left (lines of the first also found in the second, 100-token windows):

| Family | Near-copied lines | Notes |
| --- | --- | --- |
| Kirby's copy abilities and their characters' neutral specials | 2,581 of 4,833 lines in 13 `ft/kirby/special_*` files | the largest: Jigglypuff's Rollout 1,150 of 1,291 (`special_purin` against `purin/special_n`), Mewtwo's Shadow Ball 211, Donkey Kong's punch 232, Samus's Charge Shot 200 |
| Master Hand and Crazy Hand | 915 and 862 | grabs, waits, their `mod.rs` |
| Zelda, Sheik and Mewtwo's up specials (the teleports) | 390 to 397 a pair, three ways; Zelda's and Sheik's down specials 153 | |
| The enemy items (ReDead, Like Like, Octorok, Goomba, Old Ottosea, White Bear) | 1,895 lines covered in 6 items | `it/kinds` |
| Link's hookshot and Samus's grapple beam | 927 | |
| Shadow Ball and Charge Shot (items), the green and red shells, Ho-Oh and Lugia | 528, 512, 385 | |
| Corneria and Venom, Kongo Jungle and its past version, Mushroom Kingdom II's routes | 837, 708, 631 | stages |
| The tournament's two brackets | 497 | `gm/tournament` |
| The debug menu's glyph tables, the name entry's and `db`'s | 844 | data tables |
| Luigi's and Pikachu's side specials | 288 | |

**The pilot should be Jigglypuff's Rollout and Kirby's copy of it**: one move, 1,150 near-copied lines in a pair, both characters in the replay corpus, and the pattern (a copy ability that is the original's code reading Kirby's attributes) repeats twelve times more.

### Hardcoded lists

Characters, stages and items have no type of their own: a kind is an integer, and each list of them is written where it's used.

- **Characters.** 162 definitions of 49 `KIND_*` constants (`KIND_KIRBY` defined 10 times, `KIND_MARS` 8), 1,680 uses of `ckind`; 33 per-kind tables (`[_; 33]`) in 12 files (`ft::kind_tables`' files, `ft::co::kind_callbacks`' 20-odd callback tables generated from the executable, `lb::audio`'s `CHARACTER_SLOT`, the sound banks, the names, the counts, the endings' trophies); the per-character attribute sizes and item lists in `ft::kinds`; the dispatch chains above (`run_fighter_cb` 37 modules, `code::call` 33); `game::roster`'s presentation list.
- **Stages.** 70 definitions of 55 stage-kind constants in 37 files, 419 uses; `code::run_event` (41 stages' setup, load and start hooks) and `run_demo_init` (40 arms), `run_proc`'s stage arms, `gr::data`'s tables, the roster's audits.
- **Items.** About 160 item modules joined in `it::kinds::run_event` and `run_pred`, item kinds named three ways (`IT_KIND_*`, `It_Kind_*` in comments, bare numbers), the generated `it::tables_gen`.
- **Callbacks in general.** 709 dispatch functions, 3,246 address arms and 7,499 address constants (`const NAME: u32 = 0x80...`), the ledger's link being each function's `#[doc(alias)]` (17,775 aliases).

One registry each would replace them: a `Character` entry per fighter kind (its module's callbacks, files, attributes' size, sound slot, names, roster line), a `Stage` entry per stage kind (its hooks, data, audit), an `ItemKind` entry per item (its callbacks and tables), all built on one callback table from the original's address to the Rust function, so a new port registers its functions where they're written and nothing else lists it.

### Quirk modelling by ruleset

What models the original's memory rather than its rules, who needs it, and its cost. "Gameplay reaches it" means a value read by gameplay depends on it in that ruleset's matches; the [Slippi ruleset's moot table](slippi-ruleset.md#what-the-ruleset-makes-moot) and its [corpus run](oracle.md) (5,462 replays under 3.19's rules with the model off: no stop but the Super Scope's) are the evidence.

| Modelling | Vanilla | Slippi ruleset | Cost measured |
| --- | --- | --- | --- |
| The allocator model's addresses (`hsd::heap`: the OS heap, HSD's pieces, `HSD_ObjAlloc`'s pools) | reached: a computer player's DI reads its `Fighter*` | not reached: NanaDeterminism clears it; the Super Scope's one-shot charge takes a fixed stand-in address (`sscope::ADDRESS_STAND_IN`) | 0.7 to 2.3% of self time (pools and the model together); speed with it off within noise here, 0 to +4% on 2026-11-14; 121 `heap::with` calls in 59 files, 40 on/off tests in 28 |
| The pools' per-address slots (`Pool::alloc_at`, `retire`, the place lookup) | only through the addresses above; whether a class object's address reaches gameplay is the carried-in question ([task 7](#proposed-tasks)) | not needed (the model is off) | 3.5 to 5.8% of match speed (`perf/pool-trim`); snapshots growing with the match (materials 501 against 251 KB) |
| The heap's image (`game::leftovers`: Brinstar's timer, Fountain of Dreams' star, Nana's ring, the collision stamp, Rainbow Cruise, the spawner, links, the fan) and the layouts `vanilla_vs`, `from_menus` | reached from the menus | not in a live match (playback's fresh heap); still in the ruleset's menus, which run Vanilla's | 138 to 227 KB of a session's snapshot, about 3 µs of its clone |
| The stack descriptors' keys (`hsd::id::stack_key`) | only the oracle's comparison: nothing looks them up | the same | 5 sites, no measurable time |
| r5 through the computer player's frame (`cpu_ai::Leftover`, `check_floor_r5`, `excluded_floor_r5`) | reached (Nana's DI) | not consulted under 3.19 (a stop by name if read); 3.16 replays still need it | still runs in every match: 46 lines in 7 files |
| Leftovers within a match (an item's variables over the block's last item, `alloc_stale`, `it::Var`; a fighter's handle words and stale bytes; the stale-read counters) | reached where read | reached alike: Melee's own behaviour on every machine, which no Slippi code clears | part of the items' and fighters' state; 21 `alloc_stale` sites |
| Registers and stack words a function never set (about 75 cases in [the engine notes](engine.md#uninitialized-values): `mpLib_80056C54`, `resolveIKJoint1`, Yoshi's egg, a far Arwing's laser...) | reached | reached alike, but the stack words of Slippi's playback sound code (`it::Globals::stale_last_pos`) | each a few words |
| The display pass's phase (the doubled round every 1,001 ticks, writes the logic reads: off-screen marks, Stadium's monitor, the KO'd fighter's placement) | reached | fixed by the codes but for homing and nearest-opponent items and Onett's cars, held by the display-phase tripwire, both outcomes computed | the tripwire's second computation, unmeasured (rare) |
| The audio driver, synth and DSP walk, and the timeline's clock | reached (voice handles, the doubled rounds) | reached alike | audio 0.4 to 1.0% of self time; 113 to 122 KB of a snapshot |
| Disc timing in a match (Stadium's asynchronous transformations) | reached | not reached (3.19 loads synchronously) | none measurable |

So the Slippi ruleset's matches can drop the allocator model, the per-address slots, the heap's image and the r5 bookkeeping, and the menus can't yet; what remains in both is Melee's own in-match behaviour.

### Ruleset hook points

Outside the rules' own modules (`game::slippi`, `game::ucf`, `game::player_codes`, `game::state`), the rules branch at 69 sites in 35 files (`Rules::slippi` 20, UCF 13, the session's `Ruleset` 13, players' codes 10, `CodeSet` 4, neutral spawns, wobbling, off-screen limits and the 1P C-stick 2 each, the freeze glitch 1), playback at 21 sites in 8 files, and the allocator model's on/off at 40 in 28. Grouped into the hook points one interface needs:

| Hook point | Where it branches today | Rules |
| --- | --- | --- |
| Session boot: card, unlock getters, messages, float mode | `game::session`, `gm::memcard`, `gm::unlocks`, `state::fp_mode` | Ruleset |
| Match setup: spawns, the fighter and stage blocks zeroed, the heap layout and the model | `gm::vs::place_players`, `ft::create::alloc_fighter_block`, `gr::setup::new_ground`, `vs_match::Settings` | neutral spawn, Init Player and Stage Data, the model |
| Controller input and the action states' input checks | `ft::input::read_inputs`, `ft::co::{escape, squat, damage, ground, guard, phys}`, `game::ucf` | UCF 0.8, 0.84 |
| Off-screen damage | `ft::fighter::proc_anim` | off-screen limits |
| A KO toward the screen | `ft::co::dead` (4 sites), `render::render_game_camera` | FreezeDeadUpFall |
| Targeting through fighters (opponents' side, nearest opponent) | `it::owner` | WhispyBlowDirFix, the display-phase tripwire |
| Stage determinism | `gr::stadium` (5: loads, monitor), `gr::last` (the background's seed), `gr::onett`, `live::Haze` | Slippi, CodeSet |
| Grabs and hits: wobbling | `ft::co::grab`, `ft::co::hit` | wobbling prevention |
| The Ice Climbers' freeze glitch | `ft::popo::nana_special_s` | freeze glitch fix |
| The computer player's leftover registers | `ft::cpu_ai`, `ft::cpu_move` | NanaDeterminism by CodeSet |
| Addresses gameplay reads | `it::kinds::sscope` | the stand-in |
| The 1P modes' C-stick | `gm::is_1p_mode_for_c_stick`, `gm::vs` | `c_stick_1p` |
| Recording and playback | `game::record`, `game::playback`, `lb::audio::sfx_start_param`, `gm::scene::finish_tick`, `ft::dynamics` | Ruleset, playback, live |
| Players' own codes | 10 sites in `ft`, `gr::izumi`, `gr::stadium`, `gm::scene`, `lb::audio` | `rules.player` |

Fourteen hook points; the interface is one module with a function per hook point taking the rules (plain data, no trait objects, as [state](porting.md#state) requires), so Vanilla, Slippi 3.19, 3.16, 3.14 and the UCF versions are tables of choices rather than branches in 35 files.

### The draw records

**What the display pass emits today** (`hsd::display::Draw`, 52 kinds, pushed at 49 sites in 20 files): commands in the order the original issues GX commands, naming objects by handle (`Shape { jobj, dobj, pobj, trsp, rendermode, vmtx }`, `FighterJoint`, `SpecularInit`, cameras, light sets, fog, shadows, particles, text, copies, quads), with snapshots of the state that changes between draws (`CameraView`, `LightSet`, the shadow lists, fighter and item flags, highlight and toon state). melee-render reads the objects' state as the pass left it and builds the GX stream; `melee_render::blend` already keys a frame's joints, shapes, cameras and particles by these handles.

**What the fast renderer's records need**, and where each already is:

| Record field | Today |
| --- | --- |
| The model: a shape's vertices and display list, decoded once into GPU buffers | `PObj::verts` and `PObj::display` are `DataRef`s into immutable archives: a stable key. Exceptions: display lists the game rewrites in place (refraction's patches, `PObjs::refract_patches`; the 86 rewritten lists melee-gx tests for) need a key that changes with them |
| Joint matrices | each joint's world matrix in the state after the pass; envelope matrices for enveloped shapes computed by the pass (`setup_envelopes`); the blend's `Capture` holds the previous tick's |
| Material | the `MObj` (its compiled TEV, colours, pixel-engine block, textures and their matrices), read live, plus the per-draw snapshots above |
| Order | the `Draw` list's order, with its transparency pass |

**What changes in melee-sim: nothing, for the first milestones.** The records can be built on the presentation side from today's `Draw` list and state, as melee-render builds GX from them, so the fast renderer's mesh cache, GPU transforms, materials and blending need no simulation change and no oracle run. If the records later move into melee-sim (compact records in place of `Draw::Shape`, [design call 6](#design-calls)), the check is the oracle's: the state identical with the old and the new pass on the corpus and the sessions (the display list is taken out of the state after each pass, so only the records' own bookkeeping changes), and the exact renderer's frames tests unchanged. The state-only mode is the same seam: one flag that skips the pushes and snapshots, checked by playing matches with it on and off and finding every tick's state equal, as `cosmetic_tests` does.

### Naming

Function names are already the port's own: 47 of 21,390 functions carry an address (`skip_800711dc`, `pl_800411c4`). What's left of the decomp's naming (scratch scan over identifiers outside comments, strings and attributes):

| Module | Items named by address (statics, tables, consts, variants) | Fields and items named by offset (`x2223_b2`, `xDD8`, `unk_8`) | Uses of offset names |
| --- | --- | --- | --- |
| `game::structs` (generated from the decomp's layouts: `Fighter`, `Item`, `StageInfo`...) | 18 | 3,977 | 4,440 |
| `game::mirror_gen` (generated, the comparer's mirror) | 2 | 1,937 | 7,814 |
| `game::ft` | 256 (`skip_*`, `ft_8008*`, motion tables) | 1,397 | 7,459 |
| `game::gr` | 0 | 1,423 | 6,123 |
| `game::it` | 172 (`STATES_IT_803F5468`...) | 191 | 7,980 |
| `game::gm` | 5 | 373 | 3,887 |
| `game::hud`, `game::pl`, `game::ty`, `game::mn`, `game::vi` | 34, 17, 0, 0, 0 | 21, 19, 40, 13, 1 | 368, 583, 258, 177, 8 |
| `hsd`, `lb`, `mp`, `audio` | 0 | 52, 57, 33, 7 | 208, 399, 394, 28 |
| Whole crate | 504 | 9,567 | 40,868 |

Most offset names are fields of the structs `tools/structgen` generates from the decomp's type layouts, which the code reads everywhere (`fp.x2223_b2`); renaming them means a name map in `tools/structgen/config.py` (or taking the structs over, [design call 3](#design-calls)), and each wave renames the uses in one module. A wave order, smallest conflict first: (1) `hsd`, `lb`, `mp`, `audio` (149 definitions, the engine); (2) `game::gr` (1,423); (3) `game::it` (363); (4) `game::ft` (1,653, after the character registry so the two don't collide); (5) `game::gm`, `mn`, `hud`, `ty`, `vi`, `pl` (with `vs_speed_scale`, [carried in](#carried-in)); (6) the generated structs' fields, struct by struct (`Fighter`, `Item`, `StageInfo`, then the rest), each with its uses. The ports' `#[doc(alias)]` stay as they are.

**Subsystem boundaries, for scale.** `Running` (the game's state lent to the code) has 8,437 methods in 506 `impl` blocks over 447 files, 39% of the crate's 21,390 functions: every subsystem reaches every other's state through it. The boundaries goal is to split that up; one pilot first ([task 13](#proposed-tasks)).

## Proposed tasks

Sizes: S a few days of one agent's work, M a subsystem, L more. Every refactor task keeps the oracle identical: the golden diffs and sessions it reaches (`melee-oracle` with `--write-golden` where states change shape), `sync-test --rollback 7` on the bench, `melee-bench native` before and after on the worker or a quiet machine, the ledger's `--check`, and the frames tests where the display pass is touched. A new check is mutation-tested; a defect found goes in [Melee's bugs](melee-bugs.md). The fast renderer's tasks are checked against the exact renderer (geometry within a tolerance and perceptual image comparisons, [above](#the-fast-renderer)), not the oracle.

**The Slippi ruleset's state** (2026-11-17): its task 4 is done; task 5's second part (`feat/m6s-moot-2`: the model off in the player's Slippi matches, `Settings::vs_with`) and task 6 (`feat/m6s-player`) have branches at `f0fa4c81`, and the live match's audio (`feat/m6s-live-audio`) is queued. Tasks below that touch `vs_match::Settings`, `game::live`, `game::session` or `game::record` wait for those to merge; they're marked.

### The refactor

| # | Task | Size | Depends on | Shared code it touches | Target and check |
| --- | --- | --- | --- | --- | --- |
| 1 | **One callback table.** A table from the original's address to the Rust function for every callback kind (fighter callbacks, GObj processes, stage events, item events and predicates, `code::call`), built once (a sorted array or a perfect hash generated with the ports), in place of the chains; `CodeRef` stays the address, so the state and the oracle don't change. Each module's `run` becomes its table entries | M | nothing | `game::code`, every module's `run`/`call`/`run_event` (mechanical) | dispatch below 3% of self time (from 17 to 26%); match speed +15% or more on `fox-fox-fd`, `fox-marth-ps` and Sarah's 3.19 replay; oracle and corpus unchanged |
| 2 | **The character registry.** A `Character` entry per fighter kind (task 1's callbacks, `kind_tables`' files, `kind_callbacks`, the attributes' size, the sound slot, names, roster line) and one fighter-kind type in place of the 162 `KIND_*` constants ([design call 2](#design-calls)) | M | 1 | `game::ft`, `game::roster`, `lb::audio`, the generators in `tools/tables` | adding a character's port touches its own module and one entry; no `KIND_*` constant outside the registry; oracle |
| 3 | **The stage registry.** A `Stage` entry per stage kind (setup, load, start and demo hooks, data, audit) and one stage-kind type for the 70 constants | M | 1 | `game::gr`, `game::code`, `game::roster` | the same for stages; oracle and sessions |
| 4 | **The item registry.** An `ItemKind` entry per item (its callbacks and tables), one item-kind type | M | 1 | `game::it` | the same for items; oracle with items on |
| 5 | **The ruleset interface.** The 14 [hook points](#ruleset-hook-points) as one module of functions over `Rules`; Vanilla, Slippi 3.19, 3.16, 3.14 and the UCF versions as data | M | 1; Slippi tasks 5b and 6 merged | `game::slippi`, `game::ucf`, `game::state`, the 35 files that branch | no `rules.*` test outside the module; oracle and the ruleset's sessions and live matches |
| 6 | **Vanilla's modelling behind the ruleset.** The allocator model, heap image, layouts and r5 bookkeeping not run under the Slippi ruleset's matches (a tripwire stops any `heap::with` reached there), the Super Scope's stand-in kept | M | 5; Slippi task 5b | `hsd::heap`, `game::leftovers`, `ft::cpu_ai` | under the Slippi ruleset: no model call in a match, snapshots without slots; the live-equals-playback test and the corpus run clean |
| 7 | **Do the pools need per-address slots?** (carried in) Audit whether any gameplay reads a class object's address (a new object reading its address's leftovers, an ordering of addresses); if none, freely reused slots with the comparer mapping addresses (oracle-side, not state), a tripwire, the leak fix kept ([design call 4](#design-calls)) | S to M | nothing to start; its change after 6 | `hsd::pool`, the comparer | win back `perf/pool-trim`'s 3.5 to 5.8%; Vanilla's match snapshots stop growing (the Fox ditto under 1.6 MB at tick 15,000) |
| 8 | **Flat materials and shapes.** The compiled TEV and envelope lists stored inline or in one arena per pool, so a clone is copies, not allocations. **Done (2026-11-18, `perf/flat-materials`):** the compiled stages and constants inline in the material (4 and 8, longer lists on the heap), a shape's envelopes one vector with their ends inline; clone −41 to −46%, allocations −79 to −85%, snapshots 6 to 10% larger (indicative, this PC; [performance](performance.md#flat-materials-and-shapes-2026-11-18)); the state's hashes unchanged tick for tick on 327 of the verify list's replays, sync-test on the bench, the frames tests | M | nothing | `hsd::mobj`, `hsd::pobj`, `hsd::texp`, melee-render's readers | snapshot clone time −40% (materials and shapes are 52 to 60% of it today), allocations per snapshot halved; `snapshot-match`, sync-test, oracle |
| 9 | **The state-only display pass** (if [design call 5](#design-calls) takes it): a flag that skips the draws and their snapshots, used for rollback's re-simulated ticks, headless bots and `melee-bench` | S | nothing | `hsd::display`, `game::render` | state equal every tick with the flag on and off (a test like `cosmetic_tests`); saves 0.6 to 2.7% of a tick plus display as defined |
| 10 | **The dedup pilot: Rollout.** Jigglypuff's Rollout and Kirby's copy of it as one implementation over the user's attributes, both aliases on it | S | nothing | `ft::purin`, `ft::kirby` | about 1,100 lines fewer; speed unchanged within noise on corpus replays with either; oracle on every Jigglypuff and Kirby replay of the corpus |
| 11 | **The other families**, one task each after the pilot's numbers: Kirby's 12 other copies (about 1,400 lines), Master and Crazy Hand (900), the teleports (1,200 over three), the enemy items (1,900), hookshot and grapple (930), the stage pairs (2,200 over three), the shells, Shadow Ball and Charge Shot, the brackets, the glyph tables | S each | 10 (and 2 to 4 for their families' entries) | the family's modules | lines removed per family; oracle on the replays and sessions that reach it |
| 12 | **Rename waves** 1 to 6 ([naming](#naming)), one module a wave, while few branches are open; the generated structs through `tools/structgen`'s map | S to M each | wave 4 after 2 | each wave's module; everything that reads the structs for wave 6 | no offset or address name left in the wave's module; ledger and oracle unchanged |
| 13 | **A boundary pilot.** One subsystem (the camera: `game::camera`, 3,351 lines, read by fighters, stages, the HUD and the pass) behind its own state and functions rather than `Running`'s methods, measured for what crossed the line | M | 1 | `game::camera` and its callers | the camera's state reached only through its module; oracle; a written rule for the next subsystems |

**Start at once** (six agents): 1, 7's audit, 8, 10, and the fast renderer's F1 and F2 below. Then 2, 3 and 4 after 1 (in parallel: they touch different modules); 13 after 1; 5 and 6 after the Slippi ruleset's tasks 5b and 6 merge; 11 after 10; the rename waves between merges, wave 4 after 2. Task 9 only if design call 5 takes it.

### The fast renderer

Its own crates (melee-render, melee-gpu, melee-player); the `feat/linux-011` branch works in melee-gpu and melee-player now, so F2 onward should start from its merge or keep to new files.

| # | Task | Size | Depends on | Target and check |
| --- | --- | --- | --- | --- |
| F1 | **The comparison harness**: the fast renderer's frames against the exact renderer's, geometry within a tolerance and a perceptual image metric, over the frames tests' replays and stages; a low-end measurement setup (the worker's integrated or throttled run, the MacBook) | S | nothing | a check that fails on a deliberately wrong frame; baseline numbers on each test machine |
| F2 | **The mesh cache**: each shape's vertices and display list decoded once into GPU buffers keyed by `PObj::verts` and `display`, a rewritten list's key changing with its patch | M | F1 | the CPU's frame building on Fountain of Dreams from 4.5 ms to under 2.5; frames within F1's tolerance |
| F3 | **GPU transforms, skinning and lighting** from the joints' world and envelope matrices | M | F2 | CPU build under 1 ms a frame on every stage; frames within tolerance |
| F4 | **Materials**: shaders specialized per material from the compiled TEV, as `perf/tev-specialize` does per draw | M | F3 | no uber-shader draws after warm-up; frames within tolerance |
| F5 | **The whole scene**: HUD, text, particles, copies, shadows, refraction, the magnifier, every `Draw` kind | L | F4 | every frames test's frame within tolerance; the exact renderer still selectable |
| F6 | **High refresh by matrix blending** on the fast path (the blend's `Capture` reused) | S | F5 | Extrapolate at any refresh rate with the CPU's frame cost below 1 ms |
| F7 | Records from melee-sim (only if [design call 6](#design-calls) wants them) | M | F5, refactor 1 | oracle identical; the records' bookkeeping only |

## Design calls

Sarah's to answer; each with a recommendation.

| # | Call | Recommendation | What it changes |
| --- | --- | --- | --- |
| 1 | **How callbacks are named in the state** (task 1) | Keep the original's address in the state (`CodeRef`) and look it up in one table: the state, the oracle's comparisons and saved states stay as they are, and the win is the same. Typed tokens (a Rust enum of every callback) would read better in a debugger but change the state's type everywhere and the comparer with it; leave that for later, if ever | how modders write a callback: a table entry next to the function |
| 2 | **Kinds as types** (tasks 2 to 4) | One enum each for fighter, stage and item kinds, with the game's own numbers (`#[repr(u8)]` and the like), and the registry indexed by it. The game's data and the replays use those numbers, so they can't change | what modders see: `FighterKind::Fox` rather than `1` or `KIND_FOX` |
| 3 | **The generated structs' names** (`Fighter`, `Item`, `StageInfo`: 5,914 offset-named fields) | Rename through `tools/structgen`'s map, struct by struct, keeping the generator for the comparer's mirror; take the structs over by hand only once every field has a name | 9,567 names modders read |
| 4 | **The per-address pool slots in Vanilla** (carried in, task 7) | If the audit finds no gameplay reading a class object's address, drop the slots (a tripwire where an address would be read), keep the addresses only in the comparer, and win the 3.5 to 5.8% back; if it finds one, keep them for Vanilla alone and add the finding to [what Vanilla's exactness cost](#what-vanillas-exactness-cost) | Vanilla's speed and snapshot size; nothing players see |
| 5 | **The state-only display pass** | Strict as agreed (no draws built) saves only 0.6 to 2.7%: build it (task 9, S) for headless runs and the netplay follow-on's re-simulated ticks, and leave skipping setup work (lights, materials, envelopes, shadows: 7 to 12% more, but state the oracle compares changes in the skipped ticks) to the follow-on, which can prove it per field | the speed of rollback's re-simulated ticks; nothing players see |
| 6 | **Where the fast renderer's records are built** | On the presentation side from today's `Draw` list and state, so the fast renderer needs no simulation change and no verify; move records into melee-sim only if profiling the fast renderer shows building them is its limit | whether the fast renderer's work touches the oracle |
| 7 | **How a dedup keeps both originals** (tasks 10, 11) | One Rust item carrying both originals' `#[doc(alias)]`, its differences passed in (Kirby's attributes or Jigglypuff's), each original's entry kept as a one-line call only where other code names it | the ledger's link stays; readers find one copy |
| 8 | **Vanilla's future** | Keep Vanilla in the tree: its measured costs so far (the model within noise, snapshots 16 to 31% larger, the slots' 3.5 to 5.8% pending task 7) don't hamstring the refactor, and tasks 5 to 7 put them behind one ruleset test. Revisit after task 7 | whether Vanilla is frozen or moved out |
| 9 | **Measuring the targets** | Take every speed target on the worker in its exclusive runner, as the exactness round did, since this machine is rarely quiet with agents running; the low-end target as a throttled worker run until a low-end machine is in hand | which numbers count |
