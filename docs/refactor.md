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

- **The allocator model's addresses** (a computer player's DI reads its `Fighter*`): snapshots 16 to 31% larger and growing through the match (Sarah's 3.19 replay at tick 10,000: 2.22 MB with the model, 1.78 MB without; the materials' pool alone 501 against 251 KB), and the per-address lookups' 3.5 to 5.8% of match speed carried in from `perf/pool-trim`. The speed difference of the model alone stayed within this run's noise. **Since task 7 (2026-11-18) the pools' slots no longer follow the model's addresses** ([below](#task-7-the-pools-slots)): the growth and the lookups are gone, and with the model on a match's snapshot is the model-off one's plus the model's own bytes (the Fox ditto 1.90 MB at tick 15,000 against 1.86 MB off, 2.59 MB before).
- **The heap's image and the menus' layouts** (`game::leftovers`, `vanilla_vs`, `from_menus`): 138 to 227 KB of a session's snapshot, 40 on/off tests in 28 files and 121 calls into the model in 59 files.
- **r5 through the computer player's frame** (`cpu_ai::Leftover` and the floor checks): bookkeeping in 7 files that runs in every match.

None yet blocks a refactor task; [task 7](#task-7-the-pools-slots) found the slots weren't needed and dropped them.

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

- **`perf/pool-trim`'s cost** (Sarah, 2026-11-13: "merge it and win the speed back later"): 3.5 to 5.8% of match speed. First question: does anything gameplay sees depend on the class-object pools' addresses (a new object reading its address's leftovers)? If nothing does, go back to freely reused slots, compare pointers to these objects by identity, and add a tripwire. The leak fix (finished scenes' objects freed) stays either way. **Done by [task 7](#task-7-the-pools-slots)** (2026-11-18).
- **The per-address slots** of the exactness round (`fix/exact-alloc`, 2026-11-12), which Sarah questioned as never shown to be needed for gameplay. Dropped by task 7: no gameplay needs them.
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

**Measured on the worker (2026-11-19).** The survey's timings and profiles were taken again on [the worker](performance.md#machine), quiet, in its runner's exclusive reservations, at the survey's own commit and at main ([below](#measured-on-the-worker-2026-11-19)). The worker's numbers replace the busy PC's in this section; the busy PC's are kept, marked superseded. The worker's profile changes one conclusion: **the dispatch chains are 8 to 11% of the simulation's self time there, not 18 to 26%**, and the float ops and animation take more of it.

**How the profiles were taken.** samply records through ETW on Windows, which needs administrator rights the agents don't have, so a small sampler stands in: a Python script that suspends the simulation's thread 2,000 times a second, walks its stack with dbghelp (`StackWalk64` over the modules' unwind tables) and resumes it, then symbolizes every address from the PDB (the leaf's source line names inlined code's own file). It slows the run about three times (suspending is costly) but samples the code evenly; 5,500 to 12,300 samples a profile on the simulation's thread, the match's setup and disc reads left out. The tick and display pass timings are a separate, unprofiled scratch build that plays the replay through `Match::step_replay_tick` and `Match::render` and times each with `Instant`.

The matches: `fox-fox-fd`, `fox-marth-ps` and `ics-peach-ps` (Nana runs the computer player's code; it leaves its replay at frame 10,139 as always, so it plays to frame 10,986, 11,223 ticks) under Vanilla (`melee-bench native`: the replay's rules, the console's floating point, the allocator model on); the same three under the Slippi ruleset (`melee-bench corpus --rules slippi --heap off --fp slippi`: `Rules::slippi_online`, Slippi's floating point, the model off, the recorded inputs driving a 3.19 match that leaves the recording early, so `fox-fox-fd` runs 9,066 ticks and `fox-marth-ps` fewer); and a real 3.19 match, Sarah's `Game_20260129T130802` (Fox against Sheik on Dream Land, `MELEE_BENCH_FP=slippi MELEE_BENCH_HEAP=off melee-bench native`), in sync throughout.

### Where the simulation's time goes

**The display pass is 21 to 26% of a tick plus its display** (unprofiled, `Instant` around each part; the worker at `f0fa4c81`, median of three runs): Vanilla `fox-fox-fd` 356 µs a tick and 111 µs a pass (23.7%), `fox-marth-ps` 402 and 138 µs (25.6%), `ics-peach-ps` 514 and 147 µs (22.2%); under the Slippi ruleset `fox-fox-fd` 338 and 103 µs (23.4%), `fox-marth-ps` 355 and 126 µs (26.1%), `ics-peach-ps` 483 and 134 µs (21.6%); Sarah's replay 346 and 98 µs (22.0%). The busy PC (superseded) had the same shares at about 0.6 of the times: Vanilla `fox-fox-fd` 222 and 68 µs (23.5%), `fox-marth-ps` 222 and 78 µs (26.0%), `ics-peach-ps` 260 and 75 µs (22.4%); Slippi `fox-fox-fd` 216 and 67 µs (23.6%), Sarah's replay 189 to 238 and 53 to 65 µs (21.4 to 21.9%).

**Self time by what the hot paths have in common** (each sample's leaf function, bucketed by the survey's own patterns; share of the timed loop's samples on the simulation's thread). On the worker, `perf record` at 2 kHz, `f0fa4c81`, 4,400 to 16,300 samples a profile:

| Self time in | Vanilla `fox-fox-fd` | Vanilla `fox-marth-ps` | Vanilla `ics-peach-ps` | Slippi `fox-fox-fd` | Slippi `fox-marth-ps` | Slippi `ics-peach-ps` | Sarah's 3.19 replay |
| --- | --- | --- | --- | --- | --- | --- | --- |
| **Address dispatch chains** (`code::run_proc`, `run_fighter_cb`, `call`, every module's `run`, `run_event`, `*_run`) | **9.5%** | **8.5%** | **9.1%** | **9.9%** | **9.2%** | **8.3%** | **10.8%** |
| Joint and matrix math (`Mtx34::srt`, `concat`, `make_matrix`, world positions) | 15.2% | 13.9% | 13.5% | 13.2% | 13.2% | 13.4% | 11.8% |
| Bit-exact float ops and the game's math (`gekko_fp`, `sin`, `cos`, `atan`, range reduction) | 17.7% | 17.9% | 19.9% | 17.5% | 19.1% | 21.3% | 15.0% |
| Animation interpretation (FObjs, AObjs, splines, texture and shape animation) | 13.8% | 11.1% | 10.8% | 15.6% | 11.8% | 11.0% | 22.2% |
| The display pass's walk and draws (`hsd::display`, the render callbacks) | 8.4% | 8.8% | 6.9% | 9.0% | 9.1% | 6.9% | 8.2% |
| Collision (`mp`) | 5.7% | 6.0% | 5.8% | 5.7% | 4.5% | 6.8% | 2.6% |
| Particles and effects | 1.3% | 2.0% | 1.6% | 1.0% | 1.1% | 1.8% | 0.8% |
| Pools and the allocator model | 1.7% | 1.8% | 1.5% | 0.7% | 0.7% | 0.5% | 0.8% |
| Bone dynamics (`swing_chain`) | 0.7% | 0.6% | 0.7% | 0.5% | 0.9% | 0.7% | 0.5% |
| Archive lookups | 1.0% | 1.0% | 1.0% | 0.9% | 1.0% | 1.1% | 1.3% |
| Everything else (game logic, audio, system copies) | 25.2% | 28.4% | 29.3% | 25.8% | 29.4% | 28.2% | 26.1% |

Superseded: the busy PC's sampler (below), 5,500 to 12,300 samples a profile:

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

**The headline: a large share of the simulation is finding the function a callback names.** The busy PC's sampler put it at a fifth to a quarter of the self time; the worker's `perf` puts the chains themselves at 8 to 11%, spread over about 365 functions (`run_proc` 0.7%, the render chain's `run` 0.6%, then `run_fighter_cb`, Target Test's `run`, the item chain, `tou_run`, ...). The sampler suspended the thread to sample it, which may well land unevenly, and the Windows build may lay the chains out worse; the worker's figure is the one to read. It is a lower bound on what the chains cost: [task 1](#proposed-tasks)'s table, measured on the worker, gained 13 to 15% of match speed (reported with task 1), more than the chains' own self time, so they also slow the code around them. A callback in the state is the original's address (`CodeRef(u32)`, 12,034 uses), and calling one walks a chain: `code::run_fighter_cb` asks 37 modules in turn (`fox::run`, `seak::run`, ... `masterhand::run`, ... and the common action states' `ft::co::dispatch::run` 36th), `code::run_proc` 46, `code::run_event` 41 stages, `code::call` 33, and `it::kinds::run_event` about 160 item kinds joined by `||`. Each link is a `match` on the address (or a binary search), so every common action state's callback, every HUD and stage process, runs dozens of them. The samples' leaf lines confirm it is the chain itself: the hottest in this bucket are the tournament's `tou_run`, Target Test's and Kongo Jungle's `run` and the item chain's links, in a Final Destination match with items off. `hud::run_proc` alone is 8% inclusive of the Fox ditto, much of it the chain. No profile since 2026-10-09 ([performance](performance.md#simulation-speed)) saw this, when far fewer modules were ported; the chains grew with every port, which fits the Fox ditto falling from 6,637 frames/s then to about 4,400 now. A table from address to function (one lookup) should win most of it back: [task 1](#proposed-tasks).

The rest is spread thin. The bit-exact float ops (14 to 21% on the worker, `ps_madds0` alone 6.7% of the Fox ditto) are fused and inlined already ([speed work](performance.md#speed-work)); joint matrices, animation and collision are the original's own work. Pools and the allocator model are 0.5 to 1.8% of self time on the worker (0.7 to 2.3% on the busy PC), so the model's own cost is small; what `perf/pool-trim` cost is the lookups' layout ([performance](performance.md#the-exactness-rounds-cost-2026-11-13-the-ubuntu-worker)), which a profile doesn't separate. Bounds checks and copies don't stand out: system copies are 0.8 to 2.1%.

**Speed, the allocator model on and off** (the worker at `f0fa4c81`: `melee-bench native --runs 5`, three rounds alternating with main's build, median of the 15 runs and their range, frames/s): `fox-fox-fd` 2,202 (2,119 to 2,218) on, 2,234 (2,159 to 2,256) off; `fox-marth-ps` 1,882 (1,830 to 1,894) on, 1,938 (1,865 to 1,947) off; `ganon-captain-fd` 2,045 (2,008 to 2,085) on, 2,130 (2,045 to 2,135) off; Sarah's replay (Slippi float) 2,281 (2,212 to 2,293) on, 2,265 (2,241 to 2,310) off. Every run in sync. The model costs 0 to 4%, within the runs' spread but for `ganon-captain-fd`. Superseded, the busy PC (two alternating rounds, median frames/s; the differences noise): `fox-fox-fd` 3,726 and 3,551 on, 3,454 and 3,489 off; `fox-marth-ps` 3,310 and 2,837 on, 3,177 and 3,251 off; `ganon-captain-fd` 3,664 and 3,388 on, 3,436 and 3,345 off; Sarah's replay (Slippi float) 3,334 and 3,525 on, 3,337 and 3,512 off. Every run in sync.

### Measured on the worker (2026-11-19)

**How.** [The worker](performance.md#machine) (Intel Core i9-7900X, Ubuntu, the Performance power profile), every measurement in an exclusive reservation of its runner after the quietness check (processor and GPU at most 5% busy, no I/O wait; the processor 0 to 2% busy before and after each round). Release builds with line tables (`CARGO_PROFILE_RELEASE_DEBUG=line-tables-only cargo build --locked --release -p melee-bench`) of three commits: (a) `f0fa4c81`, the survey's; (b) main at `c6e274d5` (batch 3: the Slippi follow-ups, flat materials, the pools' slots, F2), the refactor's new baseline; and `ea441044`, main before batch 3, as an extra column. The tick and display pass timings are a scratch example (`perfsplit`, the survey's timing tool: the match set up as `melee-bench native` sets it up for Vanilla, as `corpus --rules slippi --heap off --fp slippi` does for the Slippi ruleset, `Match::step_replay_tick` and `Match::render` timed apart with `Instant`, to the match's end). The profiles are `perf record -F 2000 -D -1 --control fifo:...` around the same tool, enabled over the timed loop alone (the setup and disc reads left out), self time by symbol on the simulation's thread (`perf report --no-children --sort pid,sym`), bucketed by the survey's own patterns. In each job the commits alternated, one run each, three rounds; across jobs the same build moved by up to 2%.

**Speed** (ticks a second to the match's end, median of three rounds and their range; the tick and the display pass in µs, medians):

| Match | `f0fa4c81` | `c6e274d5` (main) | `ea441044` (other job; `f0fa4c81` there) |
| --- | --- | --- | --- |
| Vanilla `fox-fox-fd` | 2,134 (2,127 to 2,137); 356 + 111 | 2,235 (2,213 to 2,253); 342 + 104 | 2,163; 352 + 109 (2,171) |
| Vanilla `fox-marth-ps` | 1,842 (1,828 to 1,883); 402 + 138 | 1,912 (1,912 to 1,913); 390 + 131 | 1,861; 399 + 137 (1,877) |
| Vanilla `ics-peach-ps` | 1,509 (1,509 to 1,513); 514 + 147 | 1,563 (1,559 to 1,569); 499 + 139 | 1,526; 509 + 145 (1,529) |
| Slippi `fox-fox-fd` | 2,256 (2,254 to 2,261); 338 + 103 | 2,351 (2,346 to 2,410); 326 + 97 | 2,328; 328 + 100 (2,340) |
| Slippi `fox-marth-ps` | 2,072 (2,059 to 2,123); 355 + 126 | 2,150 (2,127 to 2,160); 344 + 119 | 2,112; 350 + 122 (2,165) |
| Slippi `ics-peach-ps` | 1,616 (1,579 to 1,620); 483 + 134 | 1,625 (1,624 to 1,670); 483 + 130 | 1,615; 485 + 132 (1,599) |
| Sarah's 3.19 replay | 2,245 (2,237 to 2,255); 346 + 98 | 2,346 (2,344 to 2,349); 334 + 91 | 2,285; 341 + 95 (2,318) |

Main runs 1 to 5% faster than the survey's commit, `ea441044` the same as it. `melee-bench native` (three rounds of five runs, [performance](performance.md#baselines-on-the-worker-2026-11-19)) puts main within −1 to +5% of `f0fa4c81`.

**Main's profile** (`c6e274d5`, the refactor's baseline; shares as above):

| Self time in | Vanilla `fox-fox-fd` | Vanilla `fox-marth-ps` | Vanilla `ics-peach-ps` | Slippi `fox-fox-fd` | Slippi `fox-marth-ps` | Slippi `ics-peach-ps` | Sarah's 3.19 replay |
| --- | --- | --- | --- | --- | --- | --- | --- |
| **Address dispatch chains** | **9.9%** | **9.1%** | **8.9%** | **10.2%** | **9.1%** | **8.7%** | **10.6%** |
| Joint and matrix math | 14.5% | 13.8% | 13.9% | 13.1% | 13.7% | 13.0% | 11.3% |
| Bit-exact float ops and the game's math | 17.7% | 19.0% | 21.1% | 18.6% | 20.2% | 21.9% | 16.3% |
| Animation interpretation | 14.3% | 11.1% | 11.2% | 14.7% | 11.0% | 10.7% | 22.0% |
| The display pass's walk and draws | 8.5% | 7.9% | 6.6% | 7.8% | 7.7% | 6.3% | 8.0% |
| Collision | 5.6% | 6.1% | 5.9% | 5.6% | 4.6% | 6.9% | 2.3% |
| Particles and effects | 1.1% | 1.8% | 1.5% | 1.2% | 1.0% | 1.5% | 0.7% |
| Pools and the allocator model | 1.0% | 1.3% | 0.9% | 0.6% | 0.5% | 0.4% | 0.7% |
| Bone dynamics | 0.5% | 0.7% | 0.8% | 0.5% | 0.9% | 0.8% | 0.4% |
| Archive lookups | 1.0% | 1.0% | 1.1% | 1.0% | 1.0% | 1.1% | 1.3% |
| Everything else | 25.9% | 28.1% | 28.2% | 26.8% | 30.3% | 28.8% | 26.5% |

`ea441044`'s shares are `f0fa4c81`'s within 0.8 points everywhere.

**The busy PC against the worker.** The busy PC ran the simulation 1.6 to 1.7 times as fast as the quiet worker (the Fox ditto 3,454 to 3,831 frames/s against 2,202; about 2 times alone, 4,373), and its tick and pass times were about 0.6 of the worker's; the shares of a tick and its display pass are the same on both (21 to 26%). Its speeds were noisy (two runs of one build 3,310 and 2,837 frames/s), the worker's within 2 to 5% across 15 runs. The profiles' shares differ: on the worker the dispatch chains take 8 to 11% (busy PC 18 to 26%), the float ops 14 to 21% (11 to 17%), animation 11 to 22% (5 to 12%), joint and matrix math 12 to 15% (11 to 16%), the display pass's walk 7 to 9% (6 to 10%), collision 3 to 7% (2 to 6%), pools 0.5 to 1.8% (0.7 to 2.3%), the rest about the same. The proposed tasks, their order and the design calls are unchanged; task 1's target is the worker's to judge, and it measures the whole cost, which a self-time share understates (above).

### Snapshots

A match's snapshot is `Game` cloned. By part (scratch tool: each of `Game`'s fields and `hsd::Objects`' pools cloned alone, bytes held and median clone time, under a counting allocator, so compare the times with each other). The times are the worker's at `f0fa4c81` (2026-11-19); the busy PC's, superseded, in brackets:

| State | Vanilla `fox-fox-fd`, ticks 5,000 / 10,000 / 15,000 | Sarah's replay, model on, 10,000 | Same, model off |
| --- | --- | --- | --- |
| Whole snapshot, clone | 1.81 / 2.00 / 2.26 MB, 384 / 416 / 463 µs [254 / 270 / 335] | 2.22 MB, 439 µs [303] | 1.78 MB, 360 µs [253] |
| Materials (`objs.mobjs`) | 371 / 477 / 656 KB (21 to 29%), 73 / 96 / 134 µs [74 / 101 at 5,000 and 10,000] | 501 KB, 102 µs [84] | 251 KB, 52 µs [51] |
| Shapes (`objs.pobjs`) | 190 / 224 / 279 KB (11 to 12%), 73 / 75 / 79 µs [57 / 60] | 254 KB, 99 µs [85] | 178 KB, 92 µs [78] |
| Joints, textures, FObjs, display objects | 166, 140, 84, 55 KB at 10,000; 2 to 12 µs each [2 to 7] | 221, 172, 97, 58 KB | 214, 164, 89, 57 KB |
| Audio (synth, AX voices, driver) | 113 KB, 6 µs [4] | 122 KB | 122 KB |
| Every scene's and mode's own state carried in a match (tournament 52 KB, debug menu 28, VS Melee 28, 1P 22, endings 17, ...) | 223 KB (11%), [6 µs] | | |
| All of HSD's pools | 1.31 MB, 202 µs [128] | | |

**Materials and shapes are 38 to 46% of a snapshot's clone time on the worker** (145 to 212 µs of 360 to 463; on the busy PC, superseded, half to 60%: 131 to 169 µs of 250 to 335): each material owns its compiled TEV and each shape its envelope lists as vectors, so a clone allocates and copies each one apart. With the model on the materials' pool also grows by address (501 against 251 KB). The scenes' and modes' state, though 11% of the bytes, copies in 6 µs. Flattening the materials' and shapes' storage is [task 8](#proposed-tasks), done (2026-11-18, `perf/flat-materials`): the compiled TEVs held in the materials and each shape's envelopes in one vector, a match snapshot's clone 41 to 46% faster with 79 to 85% fewer allocations and 6 to 10% more bytes ([performance](performance.md#flat-materials-and-shapes-2026-11-18)).

### The state-only display pass

The display pass is 21 to 26% of a tick plus its display. Inside it (share of the pass's samples, inclusive): the joints' walk and its cache misses lead (`jobj_disp_all`'s flag test and `pobj_disp`'s cull test are the hottest lines), light setup 11 to 17%, shadows' render 7 to 17%, envelope and skin matrices 9 to 14%, the screen visibility marks 8 to 12%, the HUD 8 to 11%, material and texture setup 4 to 6%, Pokémon Stadium's TEV compiles 14 to 15% there, text layout 1 to 8%, particles 1 to 3%.

**A state-only walk as defined (particles spawned, RNG drawn, matrices set, no draws built) saves 0.6 to 2.7% of a tick plus its display**: the `draws.push` statements are 0.2 to 1.0% of the pass, and the functions that only snapshot state for the presentation (`push_light_set`, the text layout for its draws) 1.8 to 8.5% (Pokémon Stadium's text the most). Everything else the pass does writes state the oracle compares: the joint, envelope and texture matrices, the lights' slots, the compiled TEVs, the off-screen marks, the shadows' cameras. Skipping more (light, material, envelope and shadow setup, about 30 to 45% of the pass, 7 to 12% of a tick plus display) would need each written field shown to be presentation-only and rewritten before anything reads it, which holds for most of them in a rollback's re-simulated ticks (the last tick's pass rewrites them) but changes the state those ticks compare. That is [design call 5](#design-calls).

**Built (task 9, `perf/state-only-display`, 2026-11-18).** `Display::state_only` runs the pass state-only ([the engine notes](engine.md#state-the-render-pass-writes)): every draw goes through `Display::push`, which drops it, and the snapshots the draws name (the cameras' views, the light sets, the shadow lists and the bookkeeping that names them, `Display::OUTPUT`) aren't built. The flag isn't state (`#[clone_from(keep)]`), so the state hash is unchanged: goldens written by main's oracle (clean full diffs, `--heap`) match the branch on every tick of the Fox ditto, Fox against Marth on Pokémon Stadium and Ganondorf against Captain Falcon. One correction to the estimate above: the text layout isn't a snapshot. `SisLib::layout_text` runs the text's typing and writes its line width and height, spacing and state stack, so the state-only pass lays texts out too and drops only their quads; a build that skipped it fails the equality test in Pokémon Stadium's match at tick 0 (`sis.texts[40].sis_buffer`) and in `pause-fd` at tick 293. The saving is therefore the draws, the light and camera snapshots and the boxed draws' allocations, nearer the bottom of the 0.6 to 2.7% range on Stadium. Used by `melee-bench native` (`MELEE_BENCH_DRAWS=on` builds the draws) and `seed-search`, and by `sync-test --state-only` for a rollback's ticks but its last. Checked by `crates/melee-player/src/state_only_tests.rs` (the Stadium and Final Destination benchmark matches and `pause-fd`, every tick's state equal outside `Display::OUTPUT`, no draw built), mutation-tested by skipping the particles' app-SRT setup (fails at tick 17 on Stadium) and the text layout (above); `sync-test --rollback 7 --state-only` passes on the six benchmark matches. Rollback's replays with it, on this machine and indicative: Pokémon Stadium's restore and 7 ticks 3.06 ms median against 3.29 ms with full passes. `melee-bench native` on this machine with other agents running moved 5 to 10% between runs, more than the pass's share, so it shows nothing either way here; the speed target is the worker's to take ([design call 9](#design-calls)).

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
| The pools' per-address slots (`Pool::alloc_at`, `retire`, the place lookup) | not needed: no gameplay reads a class object's address through its slot ([task 7](#task-7-the-pools-slots)); dropped 2026-11-18 | not needed (the model is off) | was 3.5 to 5.8% of match speed (`perf/pool-trim`) and snapshots growing with the match (materials 501 against 251 KB); gone |
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
| 1 | **Done (2026-11-18, `perf/callback-table`, [below](#task-1-the-callback-table)).** **One callback table.** A table from the original's address to the Rust function for every callback kind (fighter callbacks, GObj processes, stage events, item events and predicates, `code::call`), built once (a sorted array or a perfect hash generated with the ports), in place of the chains; `CodeRef` stays the address, so the state and the oracle don't change. Each module's `run` becomes its table entries | M | nothing | `game::code`, every module's `run`/`call`/`run_event` (mechanical) | dispatch below 3% of self time (from 17 to 26%); match speed +15% or more on `fox-fox-fd`, `fox-marth-ps` and Sarah's 3.19 replay; oracle and corpus unchanged |
| 2 | **The character registry.** A `Character` entry per fighter kind (task 1's callbacks, `kind_tables`' files, `kind_callbacks`, the attributes' size, the sound slot, names, roster line) and one fighter-kind type in place of the 162 `KIND_*` constants ([design call 2](#design-calls)) | M | 1 | `game::ft`, `game::roster`, `lb::audio`, the generators in `tools/tables` | adding a character's port touches its own module and one entry; no `KIND_*` constant outside the registry; oracle |
| 3 | **The stage registry.** A `Stage` entry per stage kind (setup, load, start and demo hooks, data, audit) and one stage-kind type for the 70 constants | M | 1 | `game::gr`, `game::code`, `game::roster` | the same for stages; oracle and sessions |
| 4 | **The item registry.** An `ItemKind` entry per item (its callbacks and tables), one item-kind type | M | 1 | `game::it` | the same for items; oracle with items on |
| 5 | **The ruleset interface.** The 14 [hook points](#ruleset-hook-points) as one module of functions over `Rules`; Vanilla, Slippi 3.19, 3.16, 3.14 and the UCF versions as data | M | 1; Slippi tasks 5b and 6 merged | `game::slippi`, `game::ucf`, `game::state`, the 35 files that branch | no `rules.*` test outside the module; oracle and the ruleset's sessions and live matches |
| 6 | **Vanilla's modelling behind the ruleset.** The allocator model, heap image, layouts and r5 bookkeeping not run under the Slippi ruleset's matches (a tripwire stops any `heap::with` reached there), the Super Scope's stand-in kept | M | 5; Slippi task 5b | `hsd::heap`, `game::leftovers`, `ft::cpu_ai` | under the Slippi ruleset: no model call in a match, snapshots without slots; the live-equals-playback test and the corpus run clean |
| 7 | **Done (2026-11-18, `perf/pool-slots`, [below](#task-7-the-pools-slots)).** **Do the pools need per-address slots?** (carried in) Audit whether any gameplay reads a class object's address (a new object reading its address's leftovers, an ordering of addresses); if none, freely reused slots with the comparer mapping addresses (oracle-side, not state), a tripwire, the leak fix kept ([design call 4](#design-calls)) | S to M | nothing to start; its change after 6 | `hsd::pool`, the comparer | win back `perf/pool-trim`'s 3.5 to 5.8%; Vanilla's match snapshots stop growing (the Fox ditto under 1.6 MB at tick 15,000) |
| 8 | **Flat materials and shapes.** The compiled TEV and envelope lists stored inline or in one arena per pool, so a clone is copies, not allocations. **Done (2026-11-18, `perf/flat-materials`):** the compiled stages and constants inline in the material (4 and 8, longer lists on the heap), a shape's envelopes one vector with their ends inline; clone −41 to −46%, allocations −79 to −85%, snapshots 6 to 10% larger (indicative, this PC; [performance](performance.md#flat-materials-and-shapes-2026-11-18)); the state's hashes unchanged tick for tick on 327 of the verify list's replays, sync-test on the bench, the frames tests | M | nothing | `hsd::mobj`, `hsd::pobj`, `hsd::texp`, melee-render's readers | snapshot clone time −40% (materials and shapes are 52 to 60% of it today), allocations per snapshot halved; `snapshot-match`, sync-test, oracle |
| 9 | **The state-only display pass** (if [design call 5](#design-calls) takes it): a flag that skips the draws and their snapshots, used for rollback's re-simulated ticks, headless bots and `melee-bench` | S | nothing | `hsd::display`, `game::render` | state equal every tick with the flag on and off (a test like `cosmetic_tests`); saves 0.6 to 2.7% of a tick plus display as defined |
| 9 status | Built on `perf/state-only-display` (2026-11-18): [the state-only display pass](#the-state-only-display-pass). The text layout writes the text's typing state, so it stays in the pass | | | | |
| 10 | **The dedup pilot: Rollout.** Jigglypuff's Rollout and Kirby's copy of it as one implementation over the user's attributes, both aliases on it | S | nothing | `ft::purin`, `ft::kirby` | about 1,100 lines fewer; speed unchanged within noise on corpus replays with either; oracle on every Jigglypuff and Kirby replay of the corpus |
| 11 | **The other families**, one task each after the pilot's numbers: Kirby's 12 other copies (about 1,400 lines), Master and Crazy Hand (900), the teleports (1,200 over three), the enemy items (1,900), hookshot and grapple (930), the stage pairs (2,200 over three), the shells, Shadow Ball and Charge Shot, the brackets, the glyph tables | S each | 10 (and 2 to 4 for their families' entries) | the family's modules | lines removed per family; oracle on the replays and sessions that reach it |
| 12 | **Rename waves** 1 to 6 ([naming](#naming)), one module a wave, while few branches are open; the generated structs through `tools/structgen`'s map | S to M each | wave 4 after 2 | each wave's module; everything that reads the structs for wave 6 | no offset or address name left in the wave's module; ledger and oracle unchanged |
| 13 | **A boundary pilot.** One subsystem (the camera: `game::camera`, 3,351 lines, read by fighters, stages, the HUD and the pass) behind its own state and functions rather than `Running`'s methods, measured for what crossed the line | M | 1 | `game::camera` and its callers | the camera's state reached only through its module; oracle; a written rule for the next subsystems |

**Start at once** (six agents): 1, 7's audit, 8, 10, and the fast renderer's F1 and F2 below. Then 2, 3 and 4 after 1 (in parallel: they touch different modules); 13 after 1; 5 and 6 after the Slippi ruleset's tasks 5b and 6 merge; 11 after 10; the rename waves between merges, wave 4 after 2. Task 9 only if design call 5 takes it.

### Task 1, the callback table

**Done (2026-11-18, `perf/callback-table` from `53429b6c`).** Every callback the state holds as an address (`CodeRef`) runs through one table from the address to its port: fighter callbacks, GObj processes (the stages', items' events, the HUD's, the scenes'), functions without arguments (stages' hooks, scenes' frames), item predicates, render callbacks and `code::call`. `CodeRef` is still the address, so the state, the oracle's comparisons and saved states are unchanged.

**How.** Each module's dispatch function keeps the `match` on the address its port wrote, marked `#[melee_callbacks::callbacks(KIND)]` (`Fighter`, `Proc`, `Event`, `Pred`, `Call`, `Render`). The attribute (`crates/melee-callbacks`, a proc-macro crate of its own so the oracle's coverage hash, which covers `melee-derive`, doesn't change) turns the function into one that adds an entry per arm: the arm's body as a function of the callback's parameters. A delegation arm (`a if fox::run(g, gobj, a) => {}`, or a body `init::run(..) || special_n::run(..)`, `a(..).or_else(|| b(..))`) adds the other function's entries. `game::code::table` walks the roots in `game::code` once, on the first dispatch, into one open-addressing hash table per kind (at most half full; a multiply and one or two probes). The conversion was mechanical: a script read rustc's diagnostics for each delegation called with the table's builder and marked the function it named, round by round, 718 functions in all, the roots in `game::code` included; the attribute refuses, at compile time, any arm that tests more than the address. Hand edits where the shapes differed: the target stages' role tables and hooks (a registration written out in `gr::target`), the legendary beasts' shared dispatch (each beast's own match now), the crowds' join callbacks (a constant list), the All-Star intro's frame and render, the album's renders, Camera Mode's calls, the characters' `call` fallbacks to their `run` (one generic fallback in `code::call` now: a fighter callback called with its GObj alone), and five direct callers of a module's `run` (Kirby's mushroom, the hands' dispatch, the hookshot's and grapple's frame hooks), which now go through the table.

**Order.** The chains ran the first module that knew an address. The table refuses an address registered twice by name with both places (`the_callback_table_builds`), and the build found 53: the common states' arms inlined in `code::run_fighter_cb` that `ft::co::dispatch` (earlier in the chain) shadowed, gone, and two of `dispatch`'s that earlier modules shadowed (`0x800CEE00`, Link's forward smash's interrupt; `0x800E8864`, Fox's down special's physics), gone from `dispatch`. No call is also a fighter callback except efLib's pause and resume, which the chain's first link took as the table does (`no_call_is_also_a_fighter_callback`).

**Kirby** is converted last, in a commit of its own (`sim: Put Kirby's callbacks in the table`) on top of a merge of `refactor/dedup-kirby`'s first round (seven copies sharing their originals' specials), so the two land together; until that commit his chain ran after the table. A later round of the dedup marks its new dispatch functions the same way: the build names each delegation that needs the attribute.

**Dispatch's share of self time: 23.7 to 29.7% before, 0.9 to 1.4% after** (the survey's sampler and buckets, below; the fn-pointer shims and the lookups counted as dispatch, an entry's own body as the callback's work). The survey's 17 to 26% counted the old chains' inlined callee bodies as dispatch too, which is why the speed-up is smaller than removing a quarter of self time would give.

| Profile (sampler, 2 kHz, `melee-bench native --runs 1`) | Base: the survey's dispatch bucket | Branch: the same bucket | Branch: with the shims and lookups |
| --- | --- | --- | --- |
| `fox-fox-fd` | 25.7% | 0.2% | 1.35% |
| `fox-marth-ps` | 23.7% | 0.0% | 1.38% |
| Sarah's `Game_20260129T130802` (`MELEE_BENCH_FP=slippi MELEE_BENCH_HEAP=off`) | 29.7% | 0.1% | 0.88% |

**Match speed: about +8 to +13%, indicative only** ([design call 9](#design-calls): the worker's exclusive runner decides, and the +15% target stands until it has measured). Median frames/s, base and branch alternating in rounds of five runs, on this PC; every run in sync. First, 15 runs each in three rounds with other agents' builds and oracle runs keeping the PC busy (the branch before the dedup merge and Kirby's commit):

| Match | Base | Branch | Change | Runs' spread, base / branch |
| --- | --- | --- | --- | --- |
| `fox-fox-fd` (Vanilla) | 2,921 | 3,254 | +11.4% | 2,809–3,021 / 3,111–3,435 |
| `fox-marth-ps` (Vanilla) | 2,511 | 2,702 | +7.6% | 2,428–2,604 / 2,624–3,228 |
| Sarah's `Game_20260129T130802` (`MELEE_BENCH_FP=slippi`, the model on) | 2,882 | 3,249 | +12.7% | 2,742–3,066 / 3,174–3,338 |

Then 10 runs each in two rounds with the PC quieter (63% load), the final branch (with `refactor/dedup-kirby`'s merge):

| Match | Base | Branch | Change | Runs' spread, base / branch |
| --- | --- | --- | --- | --- |
| `fox-fox-fd` | 4,729 | 5,176 | +9.5% | 4,467–4,964 / 5,071–5,382 |
| `fox-marth-ps` | 3,929 | 4,324 | +10.0% | 3,874–4,130 / 4,165–4,469 |
| Sarah's replay | 4,780 | 5,303 | +10.9% | 4,573–4,925 / 5,214–5,488 |

The commands, to rerun on the worker (`SSBM_DISC` and `SSBM_RS_LOCAL` set; the base built at `53429b6c`, the branch at its tip, both `cargo build --locked --release -p melee-bench`):

```sh
melee-bench native --runs 5 fox-fox-fd fox-marth-ps
# Sarah's replay copied into DIR as sarah-130802.slp
MELEE_BENCH_FP=slippi melee-bench native --dir DIR --runs 5 sarah-130802
```

Profiles: the same two builds with `CARGO_PROFILE_RELEASE_DEBUG=line-tables-only`, `python -I sampler.py OUT.json 2000 -- melee-bench native --runs 1 MATCH` (Sarah's with `MELEE_BENCH_FP=slippi MELEE_BENCH_HEAP=off`), bucketed by the survey's `cats.py`.

**Checks.**

- **The whole state, tick by tick.** Two scratch oracles, the base's (`53429b6c`) and the branch's, each patched to write the state's hash after every tick of a golden check rather than compare it (the golden's staleness ignored, so it only sets how many ticks run), gave the same hash every tick on all 329 of the verify list's replay diffs, 3,534,697 ticks, with no stop (the branch before Kirby's commit, his chain on the compatibility path). The final branch (the dedup merge and Kirby's commit on top) against the same base dumps: the same, all 329, 3,534,697 ticks.
- **Mutation:** `ftCo_Wait_Anim`'s and `ftCo_Wait_IASA`'s entries swapped in `ft::co::dispatch` (each address run by the other's port) makes `fox-fox-fd`'s hashes differ from the base's in tick 124, its first, and the full diff names the fighters' joints there (`p_link 8 #1 jobj/ aobj: ours None, theirs 0x80eeac40`). An address added twice fails `the_callback_table_builds`, naming both places (how the 53 shadowed ones were found).
- `sync-test --rollback 7 --fp hardware`: the six bench replays (`fox-fox-fd` 15,569 ticks checked, `fox-marth-fd` 15,235, `fox-marth-bf` 15,104, `fox-marth-ps` 13,579, `ganon-captain-fd` 12,061, `ics-peach-ps` 11,217) and Sarah's replay with `--fp slippi` (13,773), no difference.
- `melee-oracle --test ruleset`: all 9 pass. `--test session`, 31 named: `the_vanilla_runs_from_power_on`, `a_session_plays_its_match_from_the_selects`, `a_second_match_sets_its_spawner_up_again`, `a_session_match_rolls_back`, `the_approach_match_rolls_back`, `training_unloads_its_computer_players`, `the_debug_menus_go_back_laid_out`, `a_tournament_match_on_peachs_castle_goes_back_laid_out`, `a_tournament_tie_runs_over_the_stage_selects_preview`, `the_selects_left_by_l_r_and_start`, `matches_from_the_menus_on_three_stages`, `the_attract_match_on_brinstar_reads_a_leftover`, `rainbow_cruises_blocks_hold_the_heaps_leftovers`, `the_modes_exits`, `single_player_setups`, `a_brick_blocks_item_pops_out_at_random`, `the_trophy_lottery_draws_and_saves`, `the_trophy_gallery_toured`, `the_album_browsed`, `camera_mode_takes_and_saves_a_snapshot`, `target_test_cleared`, `the_home_run_contest`, `multi_man_melee`, `all_star`, `single_player_intros`, `the_debug_endings`, `master_hand_in_debug_vs`, `crazy_hand_in_debug_vs`, `giga_bowser_in_debug_vs`, `adventure_game_over`, `the_debug_cutscenes`: all pass (the whole suite and single-player's are the merge check's).
- The ledger's `--check` (19,827 of 19,827; only `docs/port-ledger.csv`'s line numbers moved, an attribute line above each dispatch function), melee-sim's unit tests, `fmt` and `clippy -D warnings`.
- `ticks_and_restores_dont_allocate` fails, `fox-marth-bf: tick 1029 made 1 allocations`, on the base `53429b6c` as on the branch: not this change's.

### Task 10, the Rollout pilot: done (2026-11-18, `refactor/dedup-rollout` from `aa79a9b8`)

Jigglypuff's Rollout and Kirby's copy of it are one implementation in `ft::purin::special_n`: each shared function takes a `Rollout` (Jigglypuff's `PURIN` beside it, Kirby's `KIRBY` in `ft::kirby::special_purin`) and carries both originals' `#[doc(alias)]`. Kirby's file keeps his 59 addresses, his `Rollout` and his callback table; his init's copy dispatch and leaf-check's two by-name calls (`*_8013DA24`/`*_801010D4`, `*_8013DD54`/`*_8010140C`) pass the descriptor, so no one-line wrappers were needed.

**What differed.** Once the names were normalized (`kb_pr_` to `purin_`, `kb_attr_` to `purin_attr_`), the two files differed only in: the attributes' place (Kirby's `specialn_pr_*` are Jigglypuff's `ftPurinAttributes` offsets plus 0x280, every one of the 38), the `u` word that keeps the model's scale (x4, x8C), the 17 states, five hook addresses (Kirby's death and damage hooks are his generic cleanups, a defect already in [Melee's bugs](melee-bugs.md)), and two branches of Kirby's own: off the ground or landing mid-charge he sets the roll angle before turning `TopN` side-on (`roll_before_top`), and his bounce's landing dust takes the roll's way and angle (`dust_from_roll`, also in Melee's bugs). The squash tables, the ECB, the flags and every operation were the same.

**The numbers.**

| | |
| --- | --- |
| Lines | the two files 2,974 to 1,740; with `purin/mod.rs`' attribute reader folded into `char_attr_*` and the callers, 1,244 fewer (639 added, 1,883 removed) |
| Aliases | the same 18,076 `#[doc(alias)]` attributes before and after (a sorted list diffed); `ledger --check` 19,827 of 19,827, only the CSV's locations moved |
| Time | about an hour of reading and diffing, 40 minutes of editing, then builds and checks: 5 hours of wall time in all, 4 of them the corpus's oracle runs (16.8 hours of job time, four at a time) |
| Oracle | every corpus replay and session with Jigglypuff or Kirby (below) compares with no difference; `sync-test --rollback 7` clean on a Jigglypuff replay (`Game_20260524T180613`, 17,460 ticks) and Kirby's (`oracle-kirby-jigglypuff-fd`, 12,483 ticks) |
| Mutation | the shared `rollout_init` writing -2 for `mv` x4's -1 fails Jigglypuff's `Game_20260524T180613` at tick 3020 and Kirby's `oracle-kirby-jigglypuff-fd` at tick 333, both on `fighter.mv[7]` |
| Speed (indicative, this PC at about 94% busy with other agents) | `melee-bench native`, three alternating rounds of five runs, median of the 15: `Game_20260524T180613` (`MELEE_BENCH_FP=slippi`) 2,623 frames/s before, 2,625 after; `oracle-kirby-jigglypuff-fd` 3,588 before, 3,539 after (−1.4%, inside the runs' spread of 3,455 to 3,715) |

**The corpus, and how it was found.** The verify's replay diffs (`verify-par.sh`'s job list, 329 replays) by their Slippi Game Start's characters (external ids 4 and 15): 51 have Kirby or Jigglypuff (34 with Kirby: his 22 copy CPU matches, six Slippi replays, two catch and four CPU matches; 18 with Jigglypuff, one of them the Kirby–Jigglypuff copy match). The verify's sessions (535 jobs on 533 traces) by the fighter kinds in their traces' fighter regions: 75 traces, 76 session jobs. The trace scan over-counts (some sessions' fighter regions hold other data at that offset), which only adds sessions to run. Each replay ran as a full diff (`main`'s goldens were stale at tick 1 for these), writing a scratch golden; the build from before the change then checked all 52 of those goldens, so the whole state hashes the same before and after at every tick, not only what the comparer reads.

**What was tricky.**

- Finding what reaches the move. Of the corpus's Jigglypuff replays, the two the mutation was first tried on (`roy-puff-fod`, the CPU `oracle-cpu-puff-fod`) pass with it: the mutated word is never seen there; the replay that does, `Game_20260524T180613` (the ledger's check for `ftPr_SpecialS_8013D658`), is only a sync job in the verify, though its trace is in `v5-mixed`. Its diff was added here; the verify should run it as a diff.
- Every `Running` method shares one namespace, so the copies had prefixed private helpers (`kb_pr_mvf` beside `mvf`); the shared code keeps one set (`ro_*`).
- The ledger records each alias's line, so moving aliases rewrites `docs/port-ledger.csv` (123 rows) with no change in status.

**The recipe for the next family.**

1. Diff the pair with the copy's prefixes normalized (`sed` the names), and list each difference by kind: an attribute base (often one offset for all), `u` and `mv` offsets, states, hook addresses, and real branches.
2. Put a descriptor struct in the original's module with a `const` per user, the shared functions taking `(gobj, &Descriptor)`; give each real branch a named field with its reason, and point any Melee's-bugs entry at it.
3. Move every alias of both originals onto the shared items; keep each user's own `run` table (the addresses differ) and pass the descriptor from callers that name a function, rather than keeping wrappers. Diff the sorted alias list before and after, and run `ledger --check`.
4. Find the corpus from the replays' Game Start characters and the sessions' trace fighter kinds, then prove with one mutation in the shared code (a word every use writes) that a replay of each user actually reaches it.
5. Run the corpus with the new build writing scratch goldens (`--write-golden --golden FILE`), check them with the old build, run the sessions in full, sync-test one replay of each user, and bench before and after in alternating rounds.

### Task 11, Kirby's other copies: 7 done, 11 left (2026-11-18, `refactor/dedup-kirby` from `909b868c`)

The recipe above, one character at a time, closest copy first: Luigi, Mario (with Dr. Mario), Pikachu (with Pichu), Captain Falcon (with Ganondorf), Donkey Kong, Samus, Zelda. Each original's `special_n.rs` holds the move over a descriptor (`Fireball` twice, `ThunderJolt`, `FalconPunch`, `GiantPunch`, `ChargeShot`, `NayrusLove`) with the character's `const` beside it; Kirby's `ft/kirby/special_<char>.rs` keeps only his addresses, his `KIRBY` and his `run` table. Where the original asks the fighter's kind (Mario or Dr. Mario, Pikachu or Pichu, Captain Falcon or Ganondorf) and Kirby's copy asks his copy's (`u` `HAT_KIND`), the descriptor says which (`ft::kirby::KindFrom`, `Running::special_kind`).

**What differed**, besides the states, the item and effect kinds, the joints and the hooks' addresses, which every pair has:

| Copy | Kirby's attributes | Named branches |
| --- | --- | --- |
| Luigi | none read | none |
| Mario, Dr. Mario | none read; the pill's colours in his `u` x68, x6C | his copy-less kind throws nothing (Mario's own kind never is Kirby's, so the test is shared) |
| Pikachu, Pichu | a block per copy, Pikachu's layout (x1C0, x1DC) | none (the aerial throw's unset landing lag for another copy, in Melee's bugs, stays a stop) |
| Captain Falcon, Ganondorf | Captain Falcon's layout at x1F8 | `wind`: Kirby's Warlock Punch blows no wind |
| Donkey Kong | Donkey Kong's layout 0x164 on; the swings in `u` xBC | `on_any_damage`: his copy sets (and clears) no any-damage hook |
| Samus | Samus's layout 0x150 on; `u` 0xA4 on | `fire_offset` (his fire's position taken with a zero offset), `effects_off_always` (his charge's effects destroyed unguarded, a new entry in Melee's bugs) |
| Zelda | Zelda's layout 0x2A4 on, the reflection 0x37C on | `hooks_on_change`: changing between the ground and the air sets the hitlag hooks at once where Zelda's arms the effect's accessory again |

**The numbers.**

| | |
| --- | --- |
| Lines (the copy's file and the original's `special_n.rs`) | Luigi 285 to 220 (−65), Mario 407 to 324 (−83), Pikachu 422 to 308 (−114), Captain Falcon 447 to 344 (−103), Donkey Kong 995 to 759 (−236), Samus 932 to 667 (−265), Zelda 440 to 335 (−105); with callers and the attribute constants folded, 960 fewer in all (1,420 added, 2,380 removed in `crates` and `tools`) |
| Aliases | the same 18,078 `#[doc(alias)]` attributes after every commit (a sorted list diffed); `ledger --check` 19,827 of 19,827 each time, only the CSV's locations moved |
| Oracle | the base build (`909b868c`) ran the full diff of every verify replay with Kirby or a touched character (external ids 0, 1, 4, 7, 8, 13, 16, 18, 19, 22, 24, 25: 205 replays, and 30 others) and wrote per-tick whole-state goldens; every commit's build then checked the replays with Kirby or its character (52 to 80 each, 450 checks) and the final build all 205: every tick's state hash equal. Kirby's 87 session jobs (the pilot's list): the 52 diffs compare with no difference and 34 of the 35 session-syncs roll back clean; `rec-kirby-throw`'s shortcut stops at tick 508 on the heap's free lists, as it does with the base build (a known difference, not this change's). The trace scan can't tell Mario's kind (0) from zeros, so sessions weren't picked for the other characters: their code is covered by the replays' per-tick hashes. `sync-test --rollback 7` clean on the seven Kirby copy matches (`oracle-kirby-{luigi-bf,mario-fd,pikachu-bf,captain-fd,donkey-bf,zelda-bf}`, 11,565 to 16,272 ticks, and `hf-kirby-samus-fod`, 19,865) |
| Mutation | one write in each shared function, switched on for one user at a time: both users of every pair fail, Kirby's in his copy match and the original's in its own replay (Luigi `dk-luigi-ys` tick 200 and `oracle-kirby-luigi-bf` 2760; Mario `pc-7452c0d4` (Mario against Bowser) 150 and `kirby-mario-fd` 714; Pikachu `mewtwo-pikachu-fod` 3154 and `kirby-pikachu-bf` 833; Captain Falcon `ganon-captain-fd` 519 and `kirby-captain-fd` 704; Donkey Kong `dk-falco-ys` 3649 and `kirby-donkey-bf` 369; Samus `dk-samus-bf` 97 and `hf-kirby-samus-fod` 8771; Zelda `gnw-zelda-ys` 10875 and `kirby-zelda-bf` 4345), so the corpus reaches every copy |
| Leaf-check | Kirby, Luigi, Mario, Dr. Mario, Pikachu, Pichu, Captain Falcon, Ganondorf, Donkey Kong, Samus, Zelda: all pass (Luigi's needs `LUIGI_CALLS`: its harness reads a relative path and otherwise checks none) |
| Speed (indicative, this PC busy with other agents) | `melee-bench native`, three alternating rounds of five runs, median of the 15: `oracle-kirby-mario-fd` 4,776 frames/s before, 4,535 after; `oracle-kirby-zelda-bf` 3,917 before, 4,281 after; the runs spread 3,342 to 5,819, so both differences are noise (`kirby-zelda-bf`'s bench reports "5 of 5 runs differ" before and after alike) |

**Left, and why** (Kirby has 18 copy files besides Rollout's; the survey's 12 are those with 100-token near-copies): Mewtwo, Ness, Peach, Bowser, Marth and Roy, Sheik, Link and Young Link, the Ice Climbers, Mr. Game & Watch, Yoshi, Fox and Falco: not skipped for differing, only not reached in this run (the corpus's base goldens took most of the day on this machine). Bowser's differs in more than settings (the mouth's offset, Giga Bowser's states, two known Melee bugs of his own); Yoshi's, Mr. Game & Watch's and Fox's look furthest from their originals and may not dedup cleanly.

### The fast renderer

Its own crates (melee-render, melee-gpu, melee-player); the `feat/linux-011` branch works in melee-gpu and melee-player now, so F2 onward should start from its merge or keep to new files.

| # | Task | Size | Depends on | Target and check |
| --- | --- | --- | --- | --- |
| F1 | **The comparison harness**: the fast renderer's frames against the exact renderer's, geometry within a tolerance and a perceptual image metric, over the frames tests' replays and stages; a low-end measurement setup (the worker's integrated or throttled run, the MacBook) | S | nothing | a check that fails on a deliberately wrong frame; baseline numbers on each test machine |
| F1 status | Done on `feat/fast-f1` (2026-11-18): [the fast renderer](fast-renderer.md). Found on the way: the shadow cameras erase on the near plane exactly, so one ulp of depth there drops the erase; F3 must put what lies on the depth range's edge exactly there, or clamp depth | | | |
| F2 | **The mesh cache**: each shape's vertices and display list decoded once into GPU buffers keyed by `PObj::verts` and `display`, a rewritten list's key changing with its patch | M | F1 | the CPU's frame building on Fountain of Dreams from 4.5 ms to under 2.5; frames within F1's tolerance |
| F2 status | Done on `feat/fast-f2` (2026-11-18): [the mesh cache](fast-renderer.md#f2-the-mesh-cache), every display list drawn from a mesh decoded once and keyed by what the GPU reads (the list and its rewrite, the vertex format, the arrays) by place in the archives; the 106 frames of the fast test identical to the exact renderer's. Target not met: Fountain of Dreams' CPU frame a third shorter (6.5–7.1 ms to 4.3–4.7 on a fully loaded machine, indicative), the CPU transforms and the render thread's per-draw state and uploads left, which F3 and F4 take | | | |
| F3 | **GPU transforms, skinning and lighting** from the joints' world and envelope matrices | M | F2 | CPU build under 1 ms a frame on every stage; frames within tolerance |
| F3 status | Done on `feat/fast-f3` (2026-11-18): [transforms, skinning and lighting on the GPU](fast-renderer.md#f3-transforms-skinning-and-lighting-on-the-gpu), the meshes and their indices kept on the GPU, each draw a record and its XF state (matrices, lights, registers) run through the transform unit by a compute pass, clip positions the CPU's bit for bit (the depth range's edge kept, with a harness check), back faces culled by the GPU; the 106 frames of the fast test within tolerance, geometry identical on all. The target is the worker's runs' to judge (commands in the doc); on this PC (indicative, loaded) the CPU frame was 1.6 to 5.2 ms (a half or less of F2's on one or two threads, about F2's with the default threads), what's left the stream's building and run, each draw's state and wgpu's encoding, which F4 and F5 take; the Ice Climbers' match slower than F2 threaded, to measure on the worker | | | |
| F4 | **Materials**: shaders specialized per material from the compiled TEV, as `perf/tev-specialize` does per draw | M | F3 | no uber-shader draws after warm-up; frames within tolerance |
| F5 | **The whole scene**: HUD, text, particles, copies, shadows, refraction, the magnifier, every `Draw` kind | L | F4 | every frames test's frame within tolerance; the exact renderer still selectable |
| F6 | **High refresh by matrix blending** on the fast path (the blend's `Capture` reused) | S | F5 | Extrapolate at any refresh rate with the CPU's frame cost below 1 ms |
| F7 | Records from melee-sim (only if [design call 6](#design-calls) wants them) | M | F5, refactor 1 | oracle identical; the records' bookkeeping only |

## Task 7: the pools' slots

**Done (2026-11-18, `perf/pool-slots`).** [Design call 4](#design-calls) asked whether any gameplay reads a class object's address in a way that needs the exactness round's per-address slots (`fix/exact-alloc`, `perf/pool-trim`). Nothing does: slots are reused freely now, the comparer knows the addresses a reused slot had from a history kept outside the state, and the leak fix stays (a scene's objects go with its heap).

### The audit

Two different things were in question. **A live object's address** is the allocator model's, handed out when the object is made (`heap::class_alloc`), and its slot keeps it whatever the slot policy, so its readers don't depend on slots. **Which slot an object takes** is what the per-address slots modelled. It reaches the game only through a handle kept past its object's free (a dangling pointer) that the game then follows, compares or lays out, through an object reading what its slot's last object left, or through an ordering of slots. The audit read every `Pool::addr` call in `melee-sim` (about 60 outside the heap image's layouts), every dangling handle [the engine notes](engine.md#uninitialized-values) document, and the class allocation itself; the oracle runs below then looked for what reading misses.

| Pool | Reads of a live object's address (the model's, whatever the slot) | Handles kept past the free, and who reads them | Leftovers, order | Verdict |
| --- | --- | --- | --- | --- |
| Joints (`JObj`) | Big Blue's Falcon Flyer road search reads two live joints' addresses as floats (`grBigBlue_801E8D64`, `gr::bigblue::flyer_stale_pos`; a stand-in with the model off); Samus's grapple swing takes bit 19 of the links' joint's address as its grabbed-victim-only bit (`it_802B7160`, `samusgrapple::SwingSlot::SavedR31`); stages' spline joints key the ID table by their own address (`grMaterial_801C8B68`, `id::address_key`, never looked up); the ID table's entries for a freed joint (`jobj::DANGLING`) and a stage's freed collision joints (`gr::Globals::freed_coll_jobjs`) take the address at the free; the heap image's layouts | the HUD timer's digits (written before they're read, so compared only), Zebes's bubble (a read counts a stale read), Samus's `parts[0x8B]` tip (a read counts a stale read; her match runs clean with a handle that stops), an item's hitbox joints (`HitJObj`: a read before the item writes them stops; the word its block keeps is an address now, below), the HUD's joints table and the stage select's icons across scenes (retired: following stops) | none: `hsdNew` clears every class object (`memset` at `0x803823A0`), and each port starts from its default; no ordering by address | no reader of the slot |
| Display objects (`DObj`) | the heap image's layouts, `class_free` | none documented (Kirby's DObj lists are the fighters' `HSD_ObjAlloc` pool) | as above | none |
| Materials (`MObj`) | layouts, `class_free` | none | as above; a reused slot hands on its compiled TEV's storage, emptied: storage only | none |
| Textures (`TObj`) | layouts, `class_free` | Kirby's costume textures after the costume goes (written before they're read) | as above; the palette copies' storage, emptied | none |
| Shapes (`PObj`) | layouts, `class_free` | none | as above | none |
| Cameras (`CObj`) | the current camera freed (`CObjs::freed_current`, taken at the free, compared); layouts | a GObj's object word naming a camera of a heap made anew since, which the heap image lays out (below) | as above | none |
| Lights (`LObj`) | the active table at the heap's end (`LObjs::active_left`, taken from the live lights); layouts | none | as above | none |
| World objects, fog and its adjustments | layouts, `class_free` | none | as above | none |

The pools that stand for the original's `HSD_ObjAlloc` (the scheduler's, the items', the effects', the fighters' and the others) were never in question within a scene: the model's free list and the pool's go back and come out alike, so last in, first out gives each slot one address. Where the two would part (a fighter's animation buffers after a joiner's unload), the pool still takes the slot freed at the model's address (`Pool::reused_at`).

Two places need an address the per-address slots kept and free reuse loses, and both are the heap image's, not gameplay's:

- **A retired slot's address.** The image lays out a scene's GObjs as its heap goes, and one can name an object of a heap made anew before: in `m6s-player/player-fd` (the player's own session) a GObj's object word names a camera retired with an earlier heap (tick 1766). So a retired slot keeps its last address (`Pool`'s `gone`, 4 bytes a slot and nothing else): the oracle's ruleset tests stopped there on a tripwire that asked a retired slot's address, until it did.
- **An item's hitbox joint words.** A new item's hitbox keeps its block's last item's joint word until its script writes it, and the image lays out the item as it goes. The joint's slot can have been reused at another address by then: `tournament-sessions/bracket64`'s image differed from memory in tick 9115 (a word at `0x80E24DF8`, ours `0x80E16180`, memory `0x80E047C0`). The word now keeps the joint's address as the item wrote it (`HitJObj::set_at`, from `it_802790C0`, `it::anim::create_hitbox`), as the block already kept the item's variables, dynamic bones and hurtboxes by address. A first version took the address as the item went and was wrong in the ruleset's `matches-a` (tick 1843), where the joint's slot had been taken by another joint before the item went (the word laid and compared as that joint's address).

**Under the Slippi ruleset** the model is off in the player's matches, and Samus's swing reads address 0 there without stopping (`rebuilt_slot`'s `SwingSlot::SavedR31` in `crates/melee-sim/src/game/it/kinds/samusgrapple.rs`: bit 19 of the links' joint's address, her `ThrowN` bone's, which Slippi Dolphin has). That is a gap in design call 8's tripwire, not the slots'. It went to the Slippi ruleset's follow-up.

### The change

- **`hsd::pool::Pool`.** Slots are reused last in, first out: a class object's (`Pool::alloc_at`) whatever its address, a model pool's at the model's address where the lists part. There's no address lookup (`by_addr`) and no place table (`cell`): a handle indexes its slot's contents directly, past `base`. A pool made anew (`Pool::retire`) starts its slots past the last scene's (`Pool::base`), each keeping only its last address; following a handle into them stops by name, as before.
- **The comparer** (`melee_capture::compare`). A handle whose slot has been reused at another address since, or a freed slot's handle naming an earlier occupant, compares by the addresses the slot has had (`Pool::had_addr`). `hsd::pool::history` keeps them on the thread while a tool asks (`keep_history`: `melee-oracle` and the capture importer); the simulation never reads it. That is the class objects' reuse order moved into the comparer, as design call 4 put it. It can't tell a dangling handle from a live one to the slot's new object, so it accepts a handle that names any address its slot has had; the audit shows nothing in the game follows such a handle.
- **The leak fix stays.** A scene's heap made anew still retires its class objects, `HSD_ObjInit`'s pools and the stage objects' user data.
- **The tripwire.** Following a retired handle stops by name (`followed a handle to … whose object went with its scene's heap`). A handle into a class slot reused within the scene can't be told from a live one, so it has no tripwire of its own: the audit and the oracle's comparisons are what cover it.

### Checks

**Match speed: about 3 to 15% faster on this machine, indicative only** ([design call 9](#design-calls): the targets are the worker's). `melee-bench native --runs 5`, the base (`aa79a9b8`) and the branch alternating in two rounds, median frames per second, with other agents' work keeping the processor busy throughout (Windows reported it 100% loaded); every run in sync:

| Match | Base, round 1 / 2 | Branch, round 1 / 2 | Change |
| --- | --- | --- | --- |
| `fox-fox-fd` (Vanilla, the model on) | 2,829 / 2,723 | 2,973 / 3,141 | +5.1% / +15.4% |
| `fox-marth-ps` (Vanilla) | 2,488 / 2,569 | 2,736 / 2,672 | +10.0% / +4.0% |
| Sarah's 3.19 replay `Game_20260129T130802` (`MELEE_BENCH_FP=slippi`, the model on) | 2,797 / 2,967 | 2,878 / 3,122 | +2.9% / +5.2% |

`perf/pool-trim` cost 3.5 to 5.8% on the worker; whether all of it came back is the worker's measurement to make.

**The Fox ditto's snapshot** (`melee-bench snapshot-match --every 5000`) no longer grows: 1.85, 1.90 and 1.90 MB at ticks 5,000, 10,000 and 15,000, against 2.13, 2.33 and 2.59 MB before; 2,508 allocations at 15,000 against 3,928. With the model off it is 1.86 MB, so all that's left of the model is its own 40 KB. The target of 1.6 MB isn't met by the slots alone: the rest is the state grown since `fix/exact-alloc`'s base (1.48 to 1.53 MB then), the materials' and shapes' storage first ([task 8](#proposed-tasks)).

**The oracle.** Every run compared clean, with no stale read:

- Replay diffs: `oracle-cpu-bowser-ps --heap` (11,033 ticks: the HUD timer's freed digit joints), `roy-fox-ys` (11,512: the Shy Guys' hitbox joints), `fox-fox-fd --heap` (15,660), `oracle-cpu-l1-luigi-l5-samus-ys --heap` (11,030: Samus's freed beam tip), `oracle-items-shoot-fd --heap` (7,428) and Sarah's `Game_20260129T130802` (13,851). Golden hashes all change (slots are numbered differently), so these ran in full.
- `sync-test --rollback 7`: `fox-fox-fd` (15,569 ticks checked), `fox-marth-ps` (13,579) and Sarah's replay (13,773, `--fp slippi`); `melee-oracle`'s `--test sync` (18 tests); `ticks_and_restores_dont_allocate`.
- Sessions (`melee-oracle --test session`), the ones that cross the most scene changes, retires and heap images: `the_vanilla_runs_from_power_on` (Vanilla's menus to its matches), `a_session_plays_its_match_from_the_selects` and `a_second_match_sets_its_spawner_up_again` (menus, a match and the next), `a_session_match_rolls_back` and `the_approach_match_rolls_back` (`session-sync` over the pools), `training_unloads_its_computer_players` (the fighter pools freed mid-scene, the animation buffers by address), `the_debug_menus_go_back_laid_out`, `a_tournament_match_on_peachs_castle_goes_back_laid_out` and `a_tournament_tie_runs_over_the_stage_selects_preview` (the heap image's layouts against memory), `the_selects_left_by_l_r_and_start` (the stage select's joints across scenes), `matches_from_the_menus_on_three_stages`, `the_attract_match_on_brinstar_reads_a_leftover` and `rainbow_cruises_blocks_hold_the_heaps_leftovers` (leftovers from the image), `the_modes_exits` and `single_player_setups` (many scenes); `a_brick_blocks_item_pops_out_at_random` again on the final build with the tournament one and the Vanilla runs. All of `--test ruleset` (8). The whole session and single-player suites are the merge check's.

**Mutations**, each on a scratch build of the oracle:

- Without the slots' history (`set_addr` recording nothing), `oracle-cpu-bowser-ps` differs in tick 10,680 at `hud time.digits[4]` (ours dangling at `0x810A0640`, theirs `0x810A0820`), the tick and field the exactness round's mutation found: the comparer's mapping is what makes the class objects' dangling handles compare.
- The scene's end walking every GObj handle from 0, the scenes gone's included, stops `2026-10-10_13-05-00` in tick 150 by name (`followed a handle to … GObj … 0, whose object went with its scene's heap`): the tripwire on a retired handle.
- A first version without a retired slot's address stopped `m6s-player/player-fd` in tick 1766 by name on the address it asked (the GObj naming a gone camera above), and one without the hitbox joint's address made `bracket64`'s image differ from memory in tick 9115 and then, taking it as the item went, `matches-a` differ in tick 1843: both kept now.

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
