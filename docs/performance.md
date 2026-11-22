# Performance

Measured with `tools/bench` (`melee-bench`), release builds, other applications closed, median of 5 runs. Every number names the machine it came from. The speed targets are measured on `release` (Sarah, 2026-11-21, [the refactor's design call 10](refactor.md#design-calls)): it is the quick build to develop against, and the player's build (`dist`) only runs faster ([which build](#which-build-2026-11-21)). `melee-bench`'s tables name the profile they were built with.

## Targets

| What | Target |
| --- | --- |
| Headless simulation, one core | ≥ 20× Dolphin per core (aim 30×), same match and start (Fox–Marth on Battlefield, Final Destination and Pokémon Stadium; Ice Climbers–Peach on Pokémon Stadium); Dolphin headless, Null video, no audio, unlimited speed |
| Snapshot | < 8 MB |
| Snapshot clone, restore | < 0.5 ms each |
| Restore and 8 ticks | < 4 ms on the worst match |
| Memory per match | < 64 MB |
| Match create or reset | < 5 ms, files cached |
| Ticks | no allocations (counting-allocator test); lock-free scaling over cores |
| Player | p99 simulation and rendering < 8 ms at 1440p |

**Which machine (Sarah, 2026-11-19).** The targets in milliseconds are met on [the worker](#machine), the slower reference (a 2017 processor running the simulation at about half this machine's speed), since the aim is a game that runs on low-end PCs; the ratios (to Dolphin, a branch against its base) hold on any machine but are taken there too. The worker's numbers, 2026-11-19: the snapshot, its clone and restore are well inside their targets; restore and 8 ticks, 4.07 to 4.57 ms on the worst states at `c6e274d5`, is 2.1 to 3.9 ms at main `160ae506` (after task 1's callback table) on every state but one, Fox–Marth on Pokémon Stadium at tick 12,500, which is at the line (3.96 to 4.09 ms, one run of three over; [below](#baselines-on-the-worker-2026-11-19)). Since the refactor's task 20 (2026-11-21) every state of the benchmark matches and Sarah's replay is under it, at most 3.76 ms, and no tick, display pass or restore allocates ([below](#pokémon-stadiums-allocations-and-its-worst-state-2026-11-21)). The player's p99 is to be measured there in the fast renderer's F5 round.

## Machine

AMD Ryzen 7 9800X3D (8 cores), 62 GB RAM, Windows 11 Pro, Rust 1.98.1.

Some comparisons are taken on the Ubuntu worker instead, a smaller machine kept quiet for them: Intel Core i9-7900X (10 cores, 20 threads, 3.3 GHz, 4.5 GHz boost), 14 GB RAM, NVIDIA GeForce GTX 1080 Ti (11 GB; driver 580.178.04, Vulkan), Ubuntu 26.04.1 LTS (Linux 7.0), Rust 1.98.1. Its numbers compare builds with each other, not with the Windows figures: a snapshot's clone takes it about 2.2 times as long (300 against 136 µs on the Fox ditto before the exactness round) and the simulation runs at about 45 to 50% of the Windows speed (the Fox ditto 2,202 frames/s against 4,373 on this machine alone, 2026-11-17/19).

**Since 2026-11-18 the worker is the reference for speed** (Sarah: every speed number that decides anything runs there; design call 9 of [the refactor](refactor.md#design-calls)). It runs the Performance power profile and `kernel.perf_event_paranoid = 1`, so `perf record` works for its user; neither is to be changed. Measurements run in its runner's exclusive reservations: each waits for earlier jobs, then for the quietness check (the processor and GPU at most 5% busy and no I/O wait over three one-second samples) before it starts; each job's script logs the profile, `perf_event_paranoid`, the load and the GPU's state before and after its rounds. Builds there use `-j 3` at most (14 GB of RAM). The numbers on this machine are indicative only.

## Snapshots (2026-10-07)

`melee-bench snapshot CALL...` imports a real mid-match state from ssbm-rs's saved calls and measures a rollback snapshot of it: the `Game` value plus everything it owns on the heap, a snapshot (`clone`) and a restore (`clone_from`), and their allocations. The state was what melee-sim modeled then (scheduler, HSD objects, fighters, stage, camera, collision), before items and effects were ported; the whole-match figures below include them.

| Capture (stage) | GObjs | Joints | Snapshot | Allocations | Clone | Restore | Restore allocations |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Final Destination | 72 | 731 | 0.64 MB | 1,895 | 138.5 µs | 110.0 µs | 1,895 |
| Pokémon Stadium | 77 | 651 | 0.58 MB | 1,501 | 125.0 µs | 87.9 µs | 1,501 |
| Battlefield | 65 | 780 | 0.64 MB | 1,865 | 129.0 µs | 134.0 µs | 1,865 |

`melee-bench snapshot-match [--dir DIR] [--every TICKS] [MATCH]` measures a real match's states the same way, as the match plays natively (as `melee-bench native` plays it, `fox-fox-fd` by default) as far as it runs: every `TICKS` ticks (default 1000) the state is kept, and afterwards each is measured, together with a restore followed by 8 ticks (rollback's window) and the allocations each of those ticks makes, which the "no allocations" target counts.

`melee-bench rollback-split [--dir DIR] [--at TICK] [--runs N] [MATCH]` takes one state's window apart: it plays the match (`fox-marth-ps` by default) to tick `TICK` (default 12,500), keeps the state, and `N` times (default 200, after two warm-ups) restores it and plays 8 ticks as `snapshot-match` does, the display passes building their draws, timing the restore and each tick's engine part and display pass apart; it prints their medians and the whole window's. Its many runs make it steadier than `snapshot-match`'s median of 5. The window is a function of its own (`rollback_window`), so a sampled profile can keep its samples alone, and with `PERF_CTL_FIFO` and `PERF_ACK_FIFO` set (`perf record -D -1 --control fifo:CTL,ACK`) the profile holds the windows only.

On the whole Fox–Fox match (2026-10-13, melee-hd `edabfcb`, every 2,500 ticks; the match runs 15,575 ticks, to its last frame, 15,451, frames counting from -123). Provisional: other agents were building on the machine, so the times are high; the sizes and allocations don't depend on it:

| Tick (frame) | GObjs | Joints | Snapshot | Allocations | Clone | Restore | Restore allocations | Restore and 8 ticks | Allocations per frame |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 2500 (2376) | 66 | 524 | 0.85 MB | 1,578 | 128.7 µs | 34.3 µs | 0 | 2.15 ms | 1 |
| 5000 (4876) | 73 | 559 | 0.85 MB | 1,584 | 126.3 µs | 32.1 µs | 0 | 2.89 ms | 1 |
| 7500 (7376) | 65 | 521 | 0.86 MB | 1,578 | 143.1 µs | 40.7 µs | 0 | 2.94 ms | 1 |
| 10000 (9876) | 69 | 534 | 0.88 MB | 1,579 | 147.6 µs | 36.4 µs | 0 | 2.13 ms | 1 |
| 12500 (12376) | 63 | 502 | 0.88 MB | 1,593 | 141.0 µs | 39.4 µs | 0 | 2.05 ms | 1 |
| 15000 (14876) | 66 | 523 | 0.89 MB | 1,580 | 134.8 µs | 40.4 µs | 0 | 3.00 ms | 1 |

Before, while the match ran only to frame 4,572 (tick 4,695, where it stopped at `Player_GetMoreFlagsBit5`; 2026-10-09), every 750 ticks:

| Tick (frame) | GObjs | Joints | Snapshot | Allocations | Clone | Restore | Restore allocations | Restore and 8 ticks | Allocations per frame |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 750 (626) | 66 | 523 | 0.76 MB | 1,536 | 99.0 µs | 19.8 µs | 0 | 3.68 ms | 1 |
| 1500 (1376) | 70 | 533 | 0.77 MB | 1,537 | 100.2 µs | 29.5 µs | 0 | 4.12 ms | 1 |
| 2250 (2126) | 66 | 529 | 0.78 MB | 1,536 | 100.5 µs | 28.8 µs | 0 | 3.46 ms | 1 |
| 3000 (2876) | 69 | 540 | 0.77 MB | 1,540 | 104.6 µs | 29.4 µs | 0 | 4.31 ms | 1 (max 4) |
| 3750 (3626) | 65 | 521 | 0.78 MB | 1,537 | 106.0 µs | 29.3 µs | 0 | 3.65 ms | 1 |
| 4500 (4376) | 67 | 528 | 0.78 MB | 1,536 | 96.1 µs | 24.6 µs | 0 | 3.23 ms | 1 |

On the four benchmark matches that play to their ends (2026-10-15, melee-hd `2b3e4fb` with the sync test's changes, every 2,500 ticks). Provisional: other agents were building and testing on the machine; the sizes and allocations don't depend on it. The snapshot is larger than on 2026-10-13 because the state has grown (items, audio, more of the HUD and stages):

| Match | Snapshot | Allocations | Clone | Restore | Restore allocations | Restore and 8 ticks | Allocations per tick |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `fox-fox-fd` | 1.14 to 1.19 MB | 2,489 to 2,525 | 116 to 164 µs | 28 to 41 µs | 0 | 1.08 to 1.58 ms | 1 (max 19) |
| `fox-marth-bf` | 1.14 to 1.17 MB | 2,497 to 2,508 | 117 to 177 µs | 29 to 42 µs | 0 | 0.82 to 2.41 ms | 1 (max 19) |
| `fox-marth-ps` | 1.24 to 1.36 MB | 2,580 to 2,657 | 124 to 189 µs | 31 to 51 µs | 1 | 1.36 to 1.63 ms | 10 to 75 (max 112) |
| `ganon-captain-fd` | 1.20 to 1.22 MB | 2,465 to 2,471 | 121 to 162 µs | 29 to 43 µs | 0 | 1.85 to 2.43 ms | 1 |

Pokémon Stadium allocated in every tick, straight or replayed: its screens' text (`gr::pokemon_stadium::text_names`, `hsd::sis::printf`) and the TEV compiles its display changes make (`hsd::mobj::compile_tev` from `gr::pokemon_stadium::apply_display`), about 18 allocations an engine tick and 59 a display pass between ticks 5,000 and 5,020 (`melee-bench alloc-sites`). Since 2026-10-16 its engine ticks allocate only when a transformation loads (the material marks' map, `gr::setup::mark_materials`, and new shapes' envelope lists when the spares run out), and a restore allocates when the text system's maps changed (they're copied whole); its display passes still lay their text out into new vectors (`hsd::sis::layout_text`), about 7 a pass besides the text draws' boxes. That was open against the "no allocations" target until 2026-11-21, when the last of it went ([below](#pokémon-stadiums-allocations-and-its-worst-state-2026-11-21)). The sync test ([the oracle](oracle.md#sync-test)) times the same operations as it runs, the ring's snapshot taken with `clone_from` into kept storage: on the Fox ditto with rollbacks of 1, a snapshot about 60 µs and a restore about 55 µs (median); with rollbacks of 7, run two at a time beside other agents' builds, a restore and 7 ticks 2.0 to 3.0 ms (median) on the benchmark matches.

Size, clone and restore are well inside their targets. A restore reuses the target's storage: the state's types derive `melee_derive::CloneFrom`, a `Clone` whose `clone_from` copies field by field (each field's own `clone_from`, so vectors, boxes and pools keep their capacity), and the few containers that would build anew (maps, file lists of strings) are copied only when they differ. A snapshot (`clone`) still allocates, since it makes a new copy. Restore and 8 ticks was at the 4 ms target then, over it on some states: the ticks are the cost (see the profile below). After the speed work below it is 2 to 3 ms on the whole match.

**After milestone 5 (2026-11-01, melee-hd `77ff15aa`, quiet machine).** `fox-fox-fd`, every 5,000 ticks:
- **Size:** 1.40, 1.45 and 1.45 MB, with 2,505 to 2,534 allocations.
- **Clone:** 134 to 138 µs.
- **Restore:** 37 to 38 µs, with no allocations.
- **Restore and 8 ticks:** 1.25 to 1.80 ms.
- **Allocations per tick:** 1, the text draw's box.

The state has grown about a quarter since 2026-10-15 (items, audio, the HUD), and the clone and restore with it. Both are still well inside the targets.

**A session's snapshots (2026-10-24).** A session from power-on also carries the heap's image ([the engine notes](engine.md#uninitialized-values), a session's heap leftovers): its runs are kept by 64 KiB page, each shared between snapshots until something writes it, so a snapshot copies the lists of pages and a few vectors, not the runs. `melee-oracle session --snapshot TICK,...` measures a session's state as `snapshot-match` measures a match's (melee-hd `feat/m5-heap`, while two sessions ran beside it):

| Session, tick | Snapshot | Clone | Restore | The image's part | Its clone | Its restore |
| --- | --- | --- | --- | --- | --- | --- |
| idle-title, 797 (the attract match's first tick) | 0.42 MB | 50 µs | 15 µs | 18 KB | 1.8 µs | 1.3 µs |
| idle-title, 1514 (the trace's end) | 2.03 MB | 195 µs | 58 µs | 205 KB | 3.0 µs | 2.7 µs |
| 13-05-00, 941 (the match's first tick) | 1.36 MB | 116 µs | 39 µs | 49 KB | 2.2 µs | 1.9 µs |
| 13-05-00, 2516 (the match's last) | 2.31 MB | 197 µs | 67 µs | 120 KB | 3.2 µs | 2.7 µs |
| attract, 898 | 1.89 MB | 236 µs | 56 µs | 181 KB | 3.2 µs | 2.9 µs |
| menu-tour, 5588 | 2.43 MB | 228 µs | 69 µs | 196 KB | 3.3 µs | 3.1 µs |
| rules-match, 4000 | 2.50 MB | 223 µs | 72 µs | 149 KB | 3.4 µs | 3.1 µs |
| options-set, 3000 | 2.40 MB | 221 µs | 70 µs | 138 KB | 3.2 µs | 2.9 µs |
| idle-movie, 7000 | 2.37 MB | 238 µs | 68 µs | 227 KB | 3.2 µs | 2.9 µs |

Kept as plain maps the image took 1.5 to 3.4 MB of a snapshot and 0.3 to 0.8 ms of its clone, past the targets; paged, every session is well inside them. A replay keeps no image, and its match is as before: its snapshot (`fox-fox-fd` every 5,000 ticks: 1.23, 1.27 and 1.28 MB on the base, 1.29 MB at the last on the branch, which keeps a TEV input's padding byte; 2,493 to 2,522 allocations and none in a restore on both) and its speed (`melee-bench native`, three alternating rounds of five runs on an otherwise idle machine: `fox-fox-fd` 5,350 frames/s to the base's 5,369, `ganon-captain-fd` 5,075 to 5,089, within the runs' spread).

**Allocations.** An engine tick allocated about 1,550 times on this match (2026-10-08), almost all in the display pass and joint walks that collected vectors to walk over. Now walks read each next link as they go (none of their bodies changes the links they walk), envelopes are lent out instead of copied, small fixed-size lists are held inline, symbol names and voice ids are read in place, a freed shape's envelope lists are kept for the next shapes to reuse (spare storage a restore keeps and a snapshot leaves out), and `Match::recycle` takes a frame's event and draw buffers back. A material's compiled TEV is written over the storage its last compile, or a freed material's, held (`hsd::mobj::compile_tev`, in the room the compiles keep; a restore moves the compiled TEVs it would drop to the spares), and a reused texture slot keeps its palette list's storage. A frame's last allocation, the text draw's box (`Draw::SisText`), went on 2026-11-21: a text is laid out into the storage of a draw the frontend gave back with its frame (`Match::recycle`) or a state-only pass dropped (`Display::spare_texts`). An engine tick still allocates the first time something grows past what it has held (a pool's first new slot, a fighter's first handle word): `melee-bench`'s check `ticks_and_restores_dont_allocate` plays each benchmark match that plays to its end (Pokémon Stadium's too since 2026-11-21) to tick 4,600, restores its state from tick 1,000 and plays ticks 1,000 to 4,600 again, as a rollback does, each with its display pass, its draws built and the frame given back as the player gives it, and asserts that the restore, every one of those ticks and every display pass make no allocation. The same stretch replayed from one Slippi replay of each ported character (2026-10-16, `alloc-sites --dir DIR --rollback --fp slippi`, ticks 1,000 to 4,000) makes no allocation off Pokémon Stadium either. `melee-bench alloc-sites [--from TICK] [--ticks N] [--rollback] [--fp slippi] [MATCH]` tallies where a stretch of ticks allocates, by backtrace (with a profiling build: line tables on, `CARGO_PROFILE_RELEASE_DEBUG=line-tables-only`).

## Simulation speed

`melee-bench native [--dir DIR] [--runs N] [MATCH...]` (`tools/bench/src/native.rs`) plays each benchmark match (`DIR` defaults to `data/bench`) natively and headless on one thread: from the replay's start through `vs_match`'s replay path (Slippi's playback codes, `Match::step_replay`), with the display pass, in the console's floating point (`FpMode::Hardware`, as the matches were made). Since 2026-11-18 the pass is the state-only one (every state write, no draws built: [the engine notes](engine.md#state-the-render-pass-writes)), as nothing looks at the frames; `MELEE_BENCH_DRAWS=on` builds the draws, as the runs before did and the player does, and the table's header says which. The match's files are read from the disc before the clock starts, and the fighters' addresses come from the match's start trace (`NAME.start.trace`, in `DIR` or `$SSBM_RS_LOCAL/bench`), as the oracle takes them. A run is timed from the match's first tick to its last frame, and afterwards every frame's fighters (action state, position, facing, percent and stocks), sampled before the frame's display pass as Slippi's post-frame updates record them, are checked against the replay's, the fields the Dolphin harness checks. (Sampled after it, a fighter KO'd toward the screen, which the pass places, reads as out of sync: the Battlefield and Ganondorf–Captain Falcon matches did.) It prints `melee-bench dolphin`'s table (median of 5 runs, with the machine), so the two divide. A run that stops before the match's last frame is no result: its row says where it stopped and why, and gives no speed.

The Fox–Fox match plays natively from its first frame to its last, in sync with the replay at every one of its 15,575 frames. The first official benchmark (2026-10-09, melee-hd `37e5d8d`, release, AMD Ryzen 7 9800X3D, 62 GB, Windows 11 Pro 10.0.26200, the machine quiet and melee-hd's agents paused; median of 5 runs each; every run in sync, Dolphin's with its final stocks and percents equal):

| match | melee-hd native, median frames/s (runs) | Dolphin, median frames/s (runs) | ratio |
| --- | --- | --- | --- |
| `fox-fox-fd` | 3,311 (3,365, 3,335, 3,302, 3,311, 3,292) | 397 (388, 390, 397, 397, 398) | 8.3× |

The target is at least 20× (aim 30×). Earlier provisional figures (melee-hd 2,231, Dolphin 150 to 190 frames/s) were taken with other work loading the machine and understate both sides.

After the speed work below (2026-10-09, melee-hd `86dc9ab`, the same machine, quiet, median of 5 runs, every run in sync; Dolphin's figure is the one above, its build and configuration unchanged):

| match | build | melee-hd native, median frames/s (runs) | ratio to Dolphin's 397 |
| --- | --- | --- | --- |
| `fox-fox-fd` | release | 6,637 (6,697, 6,637, 6,768, 6,304, 6,356) | 16.7× |
| `fox-fox-fd` | `dist` | 7,174 (7,194, 7,174, 7,235, 6,794, 6,851) | 18.1× |

Speed work is parked here until more of the game is ported, since new content changes the profile; profile-guided optimization (an estimated 10 to 15%) is the next step when it resumes.

**After milestone 5 (2026-11-01, melee-hd `a8615a75`, which only made `melee-bench` run its commands on a 512 MB stack: they overflowed the main thread's as they started).** This is the same machine, with no builds, agents or verify running; the CPU was 7 to 11% busy before each step. Each figure is the median of 5 runs, every run in sync. Dolphin was measured again the same day on the same build and calibrations, with `ganon-captain-fd` newly calibrated. `ics-peach-ps` (its recording before 2026-11-19, [below](#ice-climbers-and-the-heap-layout)) left its replay at frame 10,139 natively, as it always did, so it gives no melee-hd figure:

| match | melee-hd release, median frames/s (runs) | melee-hd `dist` | Dolphin | release ratio | `dist` ratio |
| --- | --- | --- | --- | --- | --- |
| `fox-fox-fd` | 5,251 (5,213, 5,280, 5,251, 5,262, 5,197) | 5,620 | 388 | 13.5× | 14.5× |
| `fox-marth-bf` | 5,107 (5,015, 5,107, 5,062, 5,257, 5,147) | 5,476 | 365 | 14.0× | 15.0× |
| `fox-marth-fd` | 4,936 (4,936, 4,738, 4,964, 4,732, 5,003) | 5,234 | 389 | 12.7× | 13.5× |
| `fox-marth-ps` | 4,525 (4,462, 4,522, 4,571, 4,567, 4,525) | 4,800 | 337 | 13.4× | 14.2× |
| `ganon-captain-fd` | 5,017 (5,111, 4,977, 5,018, 5,017, 4,977) | 5,327 | 391 | 12.8× | 13.6× |

The `dist` runs came first, on the machine as quiet as the release runs (CPU 8% before them). An earlier pass of the release runs was taken while another agent was finishing its work (CPU 20% busy): it was 4,412 to 5,167 frames/s and is left out. Dolphin's figures are within 2% of 2026-10-09's (397 on `fox-fox-fd`).

The Fox ditto has gone from 6,637 frames/s (2026-10-09) through 5,350 (2026-10-24, on an otherwise idle machine, [the snapshots](#snapshots-2026-10-07)) to 5,251. The simulation now does more each tick: the items, the audio's timeline and mix, and more of the HUD's and stages' display, all of which the match runs whether or not it shows them. No profile has been taken since 2026-10-09, so where the time went is unmeasured. The same holds for whether the 20× target is still in reach with profile-guided optimization alone.

This round's speed work (below) is measured provisionally, on a loaded machine, against main built the same way: on the whole match about 4,340 frames per second in release and 4,680 in the `dist` profile (`cargo build --profile dist -p melee-bench`), median of five alternating runs each, against 2,340 for main before it (a28bb77), so about 1.85× and 2.0×. Dolphin's side, the baseline, is below.

**Profile.** A sampled profile of that run (samply at 4 kHz, about 10,000 samples on the simulation's thread; `melee-bench symbolize RVA...` names the functions at a profile's offsets in its own executable, when the profiler can't find the PDB), by share of samples including what each calls: the engine ticks 69%, the display pass 19%, and the match's setup and disc reads, outside the timed stretch, the rest. In the ticks, joint matrices (`HSD_JObjSetupMatrix`: `Mtx34::srt` 21%, `Mtx34::concat` 13%) take 34%, about half of it from the playback codes' pass over every dirty matrix of each fighter's model (16%); the fighters' bone dynamics (`swing_chains`) 24%; animation interpretation (AObjs and FObjs) about 13%; collision about 5%. By function alone, the bit-exact fused float operations (`gekko_fp::fmadds`, `fmsub`, `fnmsubs`, the paired `ps_madds0` and `ps_madds1`, and the C library's `fma` under them) take about 33%, and `sin` and `cos` with their range reduction about 10%. That was before the speed work below.

### Speed work

The rule is the goal's: no change may alter a checked bit. Each step was checked by the oracle's diff on the Fox ditto (clean through tick 4,695 while the first KO stopped it, then through all 15,660 ticks once main played the match to its end), the leaf-checks it touches, and `melee-bench native` before and after. The timings are provisional: one core, below-normal priority, builds alternating run after run on the machine above while another agent built and tested. The steps' figures are the median of five rounds to frame 4,572 (4,695 ticks), measured together; the whole match's are above.

| Build (2026-10-09) | Frames/s to frame 4,572 | Against main |
| --- | --- | --- |
| Before (main) | 2,240 | |
| Step 1: gekko-fp's ops (`21e16ef`) | 2,810 | 1.25× |
| And the inlined FMA3 instruction and `HSD_MtxSRT`'s memo (`49b54e7`, `03905db`) | 3,690 | 1.65× |
| And `sinf`'s and `cosf`'s memo (`520b8e3`) | 3,760 | 1.68× |
| And the float mode read without a thread-local (`852c66d`) | 4,230 | 1.89× |
| And the `dist` profile (`2a97a99`) | 4,590 | 2.05× |

On the whole match, against the final release build, an archive's pointer words looked up in a bitmap instead of a binary search of its relocations (`1989185`) gained about 3% (4,180 against 4,060 frames per second, median of seven alternating runs).

**gekko-fp.** melee-hd builds a copy of ssbm-rs's `gekko-fp` (at `melee-hd-base`), the package `melee-gekko-fp` in `crates/gekko-fp`, since ssbm-rs itself isn't changed for melee-hd. Only melee-sim depends on it (and re-exports it as `melee_sim::gekko_fp`, which the oracle and `melee-bench` set the mode through); ssbm-rs's crates, the leaf-checks' reference among them, keep the upstream crate, and leaf-check's `fp::set_fp_mode` sets the mode in both. Its changes are for speed only:

- The NaN results, `frsp` of a NaN and the single fused ops' halfway correction (a double result exactly between two singles) are computed in out-of-line, cold functions. Before, the compiler computed them on every operation and picked the result afterwards, which put them in the critical path of every chain of float operations. A microbenchmark of chained `fmadds` and `fnmsubs` (`cargo run --release -p gekko-fp --example fused_speed`) went from 7.5 to 3.9 ns per operation (a bare `mul_add` and rounding takes 2.5).
- The fused multiply-add takes x86-64's FMA3 instruction where the processor has it (detected at run time), written as `vfmadd213sd` in inline assembly so it inlines into code built without the feature (the intrinsic needs the feature, so every op called out to it): 3.1 ns. Elsewhere it is `f64::mul_add`, the hardware `fmadd` on aarch64 and the C library's `fma` on an x86-64 without FMA3. On Windows the C library's `fma` already uses the instruction, so the instruction alone gains little; the profile's 33% in the fused operations was mostly the code around the multiply-add.
- frC's rounding to 25 bits is skipped when nothing lies below the rounding bit, which is always so for a single.
- `lfs` widens a finite single with the processor's own conversion (the same exact value); infinities and NaNs keep the bitwise path, which moves a NaN's payload over unquieted. This alone took the run from about 2,590 to 2,990 frames per second.
- The float mode and the multiply-add path stay per thread, but are read without a thread-local while no thread has set Slippi's mode or the portable path (global counts of the threads that have; each counts only itself, so its own setting is always seen). Windows doesn't inline a thread-local read across crates, and every store and fused op read one: about 5% of the samples.
- The operations are marked `#[inline]` so they inline into melee-sim. Without the cold paths, inlining them made the run about 9% slower.

`tests/against_upstream.rs` checks the copy against the unchanged upstream crate (`tests/upstream`, its source with only its module paths changed): every operation, scalar and paired, in both float modes and on both multiply-add paths, on 74 special values (zeros, infinities, quiet and signaling NaNs with payloads, double and single subnormals, halfway and near-halfway mantissas) in every combination of three, on a million random operand triples per mode (any double, any single, singles in the game's ranges, sums that cancel), on 400,000 constructed halfway results, and `lfs` on every single whose exponent is all zeros or all ones. `tests/modes.rs`, in a process of its own, checks that the mode and the path stay per thread. Each change made wrong on purpose fails one of them (the paths' counts only where a wrong count would change bits; one that never comes down only slows the reads). Beyond these tests, every leaf-check compares the copy (through melee-sim's ports) with the original crate (through ssbm-rs's reference), and the oracle's diff compares it with traces of ssbm-rs's unchanged code.

**Memo tables.** `HSD_MtxSRT` (`Mtx34::srt`), `sinf` and `cosf` depend only on their arguments' bits and the float mode, so a per-thread direct-mapped table keyed by those bits (`melee-sim`'s `memo`) returns the result a call would compute: a joint whose parent moved but whose own scale, rotation and translation didn't gets its local matrix without six sines and cosines, and one with a single changed angle reuses the other two. The tables hold no game state, nothing else reads them, and a snapshot or restore doesn't touch them. Tests compare each with its uncached routine on hits, on slots replaced by colliding inputs, and in both float modes, with inputs whose results differ between the modes and with and without a parent scale; making the key drop the mode, the parent scale's presence or the key comparison fails them. (No single gives `sinf` or `cosf` different bits in the two modes: all 2^32 were tried once. Their tables are keyed by the mode all the same.)

**Tried and left out.** A memo of `PSMTXConcat` (25-word keys): no measurable gain, as a concatenation's inputs rarely come again. Paired fused ops and multiplies in one pass of packed instructions (passing the upstream comparison): 5.9 against 8.1 ns per paired op in the microbenchmark, but no gain on the whole match (seven alternating runs each, twice). Keyframe numbers read from the archive's bytes in one slice: under 1%, within the noise. A `dist` build for `x86-64-v3`: no measurable gain. The fused multiply-add's path as a global instead of a thread-local alone: about 1%.

**Where the time goes now.** A profile of the whole stretch after these steps (samply, about 6,300 samples on the simulation's thread, setup and disc reads about 13% of them): joint matrices 24% (`Mtx34::srt` 10%, still mostly animated joints the memo can't help, and `Mtx34::concat` 14%), the fighters' bone dynamics 20% (the playback codes' pass over dirty matrices 11% of it), the display pass 20%, animation interpretation 14% (the stock icons' joint animations alone 5%), collision about 5%.

## The exactness round's cost (2026-11-13, the Ubuntu worker)

On 2026-11-12 the exactness round ([what must be exact](porting.md#what-must-be-exact), [milestone 6](milestone-6.md)) kept and compared every leftover word, and `fix/exact-alloc` made the pools take slots by the allocator model's address ([the engine notes](engine.md#objects-and-identity)): a freed object's slot stays at its address, and a new object at another address takes a new slot. Three builds were measured on [the worker](#machine), release, with the benchmark harness's own commands: A, main just before the round (`f41e96ee`); M, the allocator change alone (`a3949e93`); B, the whole round (`100381ae`). Every timing ran in an exclusive reservation of the worker's runner after its quietness check (CPU and I/O idle), the builds alternating. Every run was in sync and every session compared clean to where it was stopped.

**Rollback in a match: within the targets, about 15 to 40% larger.** `melee-bench snapshot-match --every 2500` on the five benchmark matches that play to their ends (A, then M, then B, on the same states):

| Match | Snapshot, A / B | Allocations, A / B | Clone, A / B | Restore, A / B | Restore and 8 ticks, A / B |
| --- | --- | --- | --- | --- | --- |
| `fox-fox-fd` | 1.48 to 1.53 / 1.72 to 2.19 MB | 2,504 to 2,540 / 2,923 to 3,926 | 293 to 301 / 325 to 411 µs | 135 to 139 / 147 to 189 µs | 3.05 to 4.09 / 3.21 to 4.24 ms |
| `fox-marth-bf` | 1.48 to 1.51 / 1.69 to 1.84 MB | 2,512 to 2,523 / 2,891 to 3,199 | 296 to 303 / 323 to 355 µs | 137 to 139 / 146 to 159 µs | 2.41 to 3.84 / 2.45 to 3.88 ms |
| `fox-marth-fd` | 1.51 to 1.56 / 1.71 to 2.03 MB | 2,586 to 2,617 / 2,925 to 3,576 | 301 to 310 / 327 to 387 µs | 138 to 144 / 148 to 175 µs | 3.13 to 3.77 / 3.18 to 3.80 ms |
| `fox-marth-ps` | 1.58 to 1.70 / 1.79 to 2.16 MB | 2,595 to 2,673 / 2,957 to 3,582 | 312 to 329 / 335 to 400 µs | 144 to 156 / 154 to 186 µs | 3.62 to 4.35 / 3.65 to 4.43 ms |
| `ganon-captain-fd` | 1.54 to 1.56 / 1.75 to 1.87 MB | 2,480 to 2,486 / 2,886 to 3,145 | 297 to 300 / 333 to 360 µs | 140 to 142 / 157 to 167 µs | 3.37 to 3.66 / 3.52 to 3.79 ms |

M's figures are B's within 0.02 MB and a few µs: the allocator change is the whole difference. On A a match's snapshot stays the same size from start to end; on B it grows as the match goes on (the Fox ditto 1.72 MB at tick 2,500, 2.19 MB at 15,000), as the class objects' pools gain a slot for each address their pieces haven't had before. Restores still make no allocation (Pokémon Stadium's one, the text system's maps, as before), the ticks after a restore allocate as before (1 a tick, 8 to 11 on Pokémon Stadium), and `ticks_and_restores_dont_allocate` passes on B. At the worker's 2.2 times the Windows clone time, B's match snapshots would clone in about 0.19 ms and restore in 0.09 ms there, inside the 0.5 ms targets; restore and 8 ticks moved by −0.01 to +0.17 ms, the ticks being most of it.

**Speed: no cost in the simulation's work; release builds 2 to 6% slower by code layout.** `melee-bench native`, one run a match per invocation, five rounds with the builds alternating, median frames per second:

| Match | A | M | B | B against A | Same, melee-sim in one codegen unit (A / M / B) |
| --- | --- | --- | --- | --- | --- |
| `fox-fox-fd` | 2,347 | 2,280 | 2,230 | −5.0% | 2,328 / 2,388 / 2,337 (+0.4%) |
| `fox-marth-bf` | 2,317 | 2,234 | 2,192 | −5.4% | 2,266 / 2,341 / 2,306 (+1.8%) |
| `fox-marth-fd` | 2,172 | 2,130 | 2,119 | −2.4% | 2,152 / 2,204 / 2,193 (+1.9%) |
| `fox-marth-ps` | 2,003 | 1,957 | 1,918 | −4.2% | 1,980 / 2,025 / 2,004 (+1.2%) |
| `ganon-captain-fd` | 2,222 | 2,156 | 2,144 | −3.5% | 2,171 / 2,205 / 2,228 (+2.6%) |

An earlier round of A against B alone gave −5.0 to −6.1%. A sampled profile of A and B (gdb stopping the program every 10 ms, about 1,000 samples on the simulation's thread each; `perf` isn't open to users on the worker) shows the same shares everywhere within the samples' noise, and the pools' own work (`Pool::alloc_at`, `set_addr`, `take_free`) under 1% of B's samples. With the allocator model off (`MELEE_BENCH_HEAP=off`, every pool last in, first out) M was still 2.8 to 3.1% slower than A, so the cost isn't the slots' order. Built with melee-sim in one codegen unit (`--config 'profile.release.package.melee-sim.codegen-units=1'`), where how the compiler splits the crate doesn't move its inlining, B is 0.4 to 2.6% faster than A and M 1.6 to 3.3% faster: the release builds' gap is where the default sixteen codegen units fell, not work the round added. It may come back or go with the next change to melee-sim. The `dist` profile (one codegen unit, fat LTO) wasn't measured.

**A session's snapshots: over the targets in a long session, on A already, twice that on B.** `melee-oracle session --snapshot` from power-on (sizes are the state with everything it owns; times on the worker):

| Session, tick | Snapshot, A / M / B | Clone, A / B | Restore, A / B |
| --- | --- | --- | --- |
| `menu-tour`, 5588 | 2.69 / 3.86 / 3.89 MB | 657 / 904 µs | 302 / 446 µs |
| `13-05-00`, 941 (the match's first tick) | 1.60 / 1.82 / 1.82 MB | 349 / 406 µs | 169 / 196 µs |
| `13-05-00`, 2516 (the match's last) | 2.55 / 3.00 / 3.02 MB | 557 / 638 µs | 275 / 323 µs |
| `rules-match`, 4000 | 2.75 / 3.36 / 3.36 MB | 613 / 701 µs | 296 / 352 µs |
| `rules-match`, 8000 | 4.07 / 4.93 / 4.96 MB | 1,038 / 1,234 µs | 489 / 611 µs |
| `2026-10-06_12-04-48` (Adventure), 10000 | 5.12 / — / 9.61 MB | 1.47 / 3.16 ms | 0.69 / 1.61 ms |
| `2026-10-06_12-04-48`, 20000 | 6.83 / — / 14.11 MB | 2.44 / 5.54 ms | 1.11 / 2.88 ms |
| `2026-10-06_12-04-48`, 30000 | 16.79 / — / 29.89 MB | 9.17 / 14.47 ms | 4.31 / 7.68 ms |

The heap's image is a small part (150 to 300 KB). A breakdown of the Adventure session's state by part (a scratch diagnostic) shows two causes:

- **The class objects of scenes gone (A and B).** Joints, display objects, materials, textures and shapes live in pools no scene change empties: when a scene makes its heap anew, the objects its heap held are never freed, so they stay live. At tick 30,000 the materials' pool holds 9,800 live objects, the textures' 10,103, the shapes' 11,869 and the joints' 11,377, the same counts on A and B, against 2,768 materials at tick 10,000. A's snapshot is 16.8 MB there (4.4 MB of materials, 3.6 MB of shapes, 2.8 MB of textures, 2.7 MB of joints).
- **The slots the allocator change keeps (B).** A freed or retired slot stays at its address, so each pool holds as many slots as addresses its objects have had: at tick 30,000 the materials' pool has 17,739 slots for 9,800 live objects, the shapes' 22,865 for 11,869, the joints' 14,574 for 11,377; the items' pool 313 slots of 7,928 bytes (2.4 MB) for 2 live items, its slots grown from 5,272 bytes by the round's first part (the item variables kept over the union's bytes); the scheduler's processes 4,964 slots for 140 and its GObjs 1,594 for 91.

In a replay's match neither shows much (one scene, a heap no image follows), which is why the matches stay inside the targets. A session from power-on is what the player runs once it boots into Melee, so a rollback there would snapshot all of it. The options are for the coordinator.

**The pools trimmed (`perf/pool-trim`, 2026-11-13).** Sarah took both of the options ("yes i agree do a+b"): (a) the objects whose memory goes with a scene's heap are retired with it, as `HSD_CreateMainHeap` leaves them (the class objects, `HSD_ObjInit`'s pools, the stage objects' user data; the fighters' pools where `Fighter_800679B0` inits them again), and (b) a retired slot keeps only its address, its contents dropped, the contents of the others living apart from their slots ([the engine notes](engine.md#objects-and-identity)). Measured on the worker as above, the base (`f17b920b`, `pool-base`) against the branch (`pool-trim`), the builds alternating in the runner's exclusive reservations; each with `session-sync --time` added (the oracle's tool, for the restore and 8 ticks below). Every session compared clean, or played in sync, to where it was stopped.

A session's snapshot, from power-on (`session --snapshot` for `menu-tour`, whose recording ends before its trace; `session-sync --rollback 0 --time` for the rest, which also restores the state into a second session and plays 8 ticks, the median of 21):

| Session, tick | Snapshot, base / branch | Clone, base / branch | Restore, base / branch | Restore and 8 ticks, base / branch |
| --- | --- | --- | --- | --- |
| `menu-tour`, 5588 (5500 for the ticks) | 3.89 / 2.70 MB | 874 / 616 µs | 419 / 290 µs | 34.8 / 36.0 ms |
| `rules-match`, 4000 | 3.36 / 2.06 MB | 744 / 482 µs | 359 / 223 µs | 35.3 / 35.4 ms |
| `rules-match`, 8000 | 4.96 / 1.77 MB | 1,378 / 450 µs | 645 / 203 µs | 20.5 / 19.8 ms |
| `2026-10-06_12-04-48`, 10000 | 9.61 / 2.47 MB | 3.49 / 0.62 ms | 1.72 / 0.27 ms | 40.8 / 40.7 ms |
| `2026-10-06_12-04-48`, 20000 | 14.11 / 2.96 MB | 5.93 / 0.75 ms | 3.04 / 0.33 ms | 7.14 / 3.78 ms |
| `2026-10-06_12-04-48`, 30000 | 29.89 / 4.74 MB | 14.52 / 1.09 ms | 7.73 / 0.55 ms | 50.8 / 44.2 ms |
| `2026-10-06_12-04-48`, 40000 | 38.11 / 5.37 MB | 18.26 / 1.13 ms | 10.08 / 0.60 ms | 47.4 / 36.4 ms |

On Windows (busy with other agents' checks) the Adventure session's snapshot peaks at 5.37 MB (tick 40,000 of 55,022), its clone 0.26 to 0.58 ms and its restore 0.06 to 0.19 ms at every 10,000 ticks: the size inside the target, the clone at it. What's left of a long session's snapshot is mostly the slots' own bookkeeping, 16 bytes for each address a pool's objects have had (an address, a place, an entry in the sorted list of addresses: the Adventure session's shapes have 32,594 slots at tick 50,000, 378 of them live), and the live scene's objects. A restore and 8 ticks is the ticks: a session's ticks outside a match cost 2 to 6 ms each on the worker (the menus' and the movies' display passes and loads), on both builds alike, which is a separate question from the snapshot (open against the 4 ms target, which was set for matches).

A match's rollback (`melee-bench snapshot-match --every 2500`): snapshots 0.03 to 0.07 MB larger (each stored slot keeps its place, 16 bytes more than before: `fox-fox-fd` 1.77 to 2.26 MB against 1.72 to 2.19, `fox-marth-bf` 1.74 to 1.90 against 1.69 to 1.84, `fox-marth-ps` 1.84 to 2.24 against 1.79 to 2.16, `ganon-captain-fd` 1.81 to 1.93 against 1.75 to 1.87), the same allocations, clone within 5% either way (337 to 434 µs against 341 to 454), restore up to 8% slower (157 to 204 µs against 146 to 201), restore and 8 ticks 0.1 to 0.3 ms slower; restores still allocate nothing (Pokémon Stadium's one as before), and `ticks_and_restores_dont_allocate` passes.

The simulation is 3 to 6% slower in a match. `melee-bench native`, one run a match per invocation, eight rounds with the builds alternating, median frames per second:

| Match | Base | Branch | Change | Melee-sim in one codegen unit, base / branch (4 rounds) |
| --- | --- | --- | --- | --- |
| `fox-fox-fd` | 2,281 | 2,181 | −4.4% | 2,355 / 2,219 (−5.8%) |
| `fox-marth-ps` | 1,963 | 1,895 | −3.5% | 2,017 / 1,921 (−4.8%) |

A pool's lookup goes through the slot's place (`Pool`'s `cell`) once the pool has retired a slot; a pool that never has, as a match's class objects' pools, indexes its contents directly (`Pool::direct`). Experiments, each built and timed against the base the same way: the lookup always through the place cost 5 to 8%; the place's path out of line, 5%; a build that indexed the contents directly with no check (unsound outside a match, but in sync there) was 2% slower than the base, so about 2 to 3 points are the lookup's check and the rest the build's layout (moving the pool's fields so the lookup reads one cache line made it 7% slower, not faster). The session's own speed doesn't move with the place's path (`session-sync --rollback 0` to tick 20,000: 172 to 176 s for both layouts). Not taken further here.

## The allocator model in the Slippi ruleset's matches (2026-11-17)

Under the Slippi ruleset a live match runs without the allocator model ([the Slippi ruleset](slippi-ruleset.md#what-the-ruleset-makes-moot), design call 8). Measured on three of the live matches' own recordings (`$SSBM_RS_LOCAL/lockstep/m6s-stall/traced`: `matches-a-5`, Captain Falcon against Ganondorf on Dream Land to the time's end; `matches-a-4`, the Ice Climbers against a computer Peach on Final Destination; `matches-b-5`, three players with a computer Pichu), played through `vs_match`'s replay path in Slippi's float mode (`MELEE_BENCH_FP=slippi`), the model on as playback lays it out and off (`MELEE_BENCH_HEAP=off`). A live match's state is its playback's but for playback's bookkeeping (`a_live_match_equals_its_playback`), so these are a live match's costs. This machine, with other agents' work running (about half its CPU busy), `melee-bench` from `feat/m6s-moot-2`; every run stayed in sync.

`native`, two alternations of five runs each, median frames per second:

| Match | Model on | Model off |
| --- | --- | --- |
| `matches-a-5` (7,324 ticks) | 3,184, 3,249 | 3,115, 3,229 |
| `matches-a-4` (1,828) | 2,017, 2,352 | 2,085, 2,093 |
| `matches-b-5` (1,538) | 2,626, 2,896 | 2,728, 2,993 |

The speed change is inside this machine's noise, as on the worker for task 5's first part.

`snapshot-match`:

| Match, ticks | Model on | Model off |
| --- | --- | --- |
| `matches-a-5`, 2,000, 4,000, 6,000 | 1.91, 1.94, 1.94 MB (2,529 to 2,579 allocations); clone 221 to 225 µs, restore 59 to 61 µs | 1.82, 1.83, 1.84 MB (2,421 to 2,427); clone 183 to 196 µs, restore 53 to 56 µs |
| `matches-a-4`, 500, 1,000, 1,500 | 2.07, 2.11, 2.16 MB (2,612 to 2,685); clone 217 to 232 µs, restore 62 to 68 µs | 1.95, 1.95, 1.98 MB (2,423 to 2,468); clone 203 to 210 µs, restore 59 to 60 µs |

The snapshot is 5 to 8% smaller and 6 to 17% faster to clone, and doesn't grow with the match without the model's per-address slots (`matches-a-4`'s allocations fall as its objects come and go). The win is smaller than the Vanilla benchmark matches' (16 to 31%, [the Slippi ruleset](slippi-ruleset.md#what-the-ruleset-makes-moot)): these matches are shorter, and the slots grow with the objects a match has made.

## Flat materials and shapes (2026-11-18)

[The refactor's](refactor.md#proposed-tasks) task 8. A material's compiled TEV (`hsd::texp::TevDesc`: its stages and the constants its expression list keeps) and an enveloped shape's envelopes (`hsd::pobj::Binding::Envelopes`) were vectors, a list of lists for the envelopes, so a snapshot allocated each apart: on `fox-fox-fd` at tick 5,000, 1,275 of the snapshot's 3,001 allocations were the materials' and 1,375 the shapes', and the two pools took 148 of its 267 µs to clone (a scratch count in `snapshot-match`: each pool cloned alone, its allocations counted).

**The lists' sizes.** Every stored slot, freed ones too (a snapshot copies them), in the six benchmark matches' snapshots every 5,000 ticks: a compile makes 1 to 6 stages (1, 2 and 4 the most; 5 and 6 in up to 3% of the materials) and keeps 0 to 7 constants; a shape has 1 to 10 envelopes (10 is the GX position matrices a display list can index) of 1 to 3 joints, 1 to 30 joints in all; 240 to 300 of a match's 740 to 1,250 stored shapes are enveloped, the rest skinned. A stage is 112 bytes, a constant 12, an envelope's joint 20.

**The layout.** A compiled TEV's stages and constants are held in the material (`InlineVec`, `crates/melee-sim/src/inline_vec.rs`): room for 4 stages and 8 constants, a longer list on the heap. That takes `MObj` from 248 to 776 bytes; room for 6 stages would have held every compile measured, saving the 6 to about 30 allocations a snapshot makes for the long lists, for 224 bytes more a material. A list is on the heap only while it holds more than fits inline: a restore keeps as spares the heap storage of every list that won't need it (dropped, or fitting inline) and gives it to the lists it would otherwise allocate (`MObjs::clone_from`), and a compile takes a spare's for a long list and gives its own up for a short one, so restores and the ticks after them don't allocate. (Storage left in a slot whose list fits, as a first version did, was out of the compiles' reach until the slot was reused, and once the pools reused freed slots last in, first out, `fox-marth-bf`'s rollback in `ticks_and_restores_dont_allocate` allocated for a compile of more than 4 stages in tick 1029.) A shape's envelopes are one vector of joints with each envelope's end held inline for up to 10 envelopes (`pobj::EnvelopeList`): one allocation a shape instead of one and one more an envelope. Holding the joints inline too would grow every shape, since the binding is an enum and most shapes are skinned. Nothing is bounded: longer lists go to the heap (a shape with more than 65,535 joints stops by name). One arena per pool was the other choice: the slots would hold ranges into it, so the state's comparison and hash, slot by slot, would see offsets rather than the lists, and the goldens and the comparer would change with them. Both lists read as slices (an envelope list as a list of slices) and compare and hash as the vectors they replace, so the state's hashes and melee-render's readers (`hsd::draw`'s `setup_tev` and envelope matrices, `blend.rs`) are unchanged.

**Results.** `melee-bench snapshot-match --every 2500` on the five benchmark matches that play to their ends, base (`aa79a9b8`) and branch alternating, two rounds each; this PC with other agents' work running, so the times are indicative (the sizes and allocations don't depend on it):

| Match | Snapshot, base / branch | Allocations | Clone, median (range) | Restore, median |
| --- | --- | --- | --- | --- |
| `fox-fox-fd` | 2.10 to 2.60 / 2.23 to 2.85 MB | 2,925 to 3,928 / 584 to 589 | 289 (254 to 335) / 156 (143 to 178) µs | 76 / 72 µs |
| `fox-marth-bf` | 2.07 to 2.23 / 2.21 to 2.41 MB | 2,893 to 3,201 / 604 to 613 | 274 (242 to 283) / 152 (118 to 164) µs | 72 / 69 µs |
| `fox-marth-ps` | 2.17 to 2.57 / 2.34 to 2.84 MB | 2,959 to 3,584 / 619 to 639 | 284 (181 to 318) / 167 (152 to 186) µs | 80 / 81 µs |
| `ganon-captain-fd` | 2.14 to 2.26 / 2.27 to 2.43 MB | 2,888 to 3,147 / 606 to 607 | 276 (264 to 313) / 152 (146 to 154) µs | 74 / 71 µs |
| `fox-marth-fd` | 2.09 to 2.42 / 2.22 to 2.63 MB | 2,927 to 3,578 / 613 to 623 | 277 (233 to 314) / 158 (114 to 170) µs | 74 / 72 µs |

A snapshot's clone takes 41 to 46% less time and makes 79 to 85% fewer allocations, and no longer grows in allocations as a match goes on; it is 6 to 10% larger, the materials' room. A restore is as before (it copied into kept storage already), with no allocation (Pokémon Stadium's one, the text system's maps, as before); restore and 8 ticks and the ticks' allocations are unchanged; `ticks_and_restores_dont_allocate` passes. In the scratch count the materials now clone with 6 to 10 allocations (the long lists) in 31 to 48 µs, the shapes with 225 in 16 to 21 µs.

A session's snapshot (`session-sync --rollback 0 --time` on the Adventure session `2026-10-06_12-04-48`, two runs each, same PC): 5 to 10% larger (2.47 / 2.59 MB at tick 10,000, 5.38 / 5.94 at 40,000, 4.59 / 4.72 at 50,000), its clone faster at ticks 10,000 to 30,000 and 50,000 (30,000: 533 and 587 / 357 and 360 µs; 50,000: 315 / 255 and 279 µs) but slower at 40,000 (523 and 529 / 676 and 747 µs), where the session's scene holds the most materials; restores as before. A session holds many more material slots than a match, so the inline room costs it more; the worker's exclusive runner should settle whether the 40,000 tick's clone is the room's bytes.

**Checks.** The goldens on disk were written before the base (`aa79a9b8`'s own state differs from them from tick 1), so the base's and the branch's states were compared by hash instead: two scratch oracles built from each (the goldens' coverage kept), each printing the state's hash after every tick of the golden check rather than stopping, gave the same hash every tick on 327 of the verify list's 329 replay diffs (3.5 million ticks; the other two's goldens predate their re-recorded replays and neither ran). `sync-test --rollback 7` passes on the six benchmark replays; `classic`, `stage-ps` and `menus-ics-fox-fod` compare clean from power-on with `--all --heap` and `session-sync --rollback 7 --shortcut` on `classic`, `stage-ps` and `rules-match` as `session-expect.txt` has them; the frames tests pass (`melee-render-check` 535, `melee-audio-check` 4). Unit tests check that both lists compare and hash as the vectors they replace and that a restore gives long lists spare storage (`materials_restore_long_tevs_into_spares`); each fails with its piece broken.

## Following playback's camera (2026-11-18)

Under the Slippi ruleset the oracle and the tests follow playback's camera beside a live match's from the first frame until the two meet, and from a pause to the match's end or until they meet ([the engine notes](engine.md#the-cameras-readers-after-a-pause), "Playback's camera, followed"): one more camera update a tick (without the quakes and the width average), the framed boxes' eased fields swapped in and out, a bit-for-bit comparison, and on the stages that keep the display pass's views one more view at each tick's end. The player doesn't (`Game::follow_playback_camera` off; Sarah: "yeah do it that way"), since that cost, about 1% for a match followed to its end, isn't negligible. Measured with melee-bench's ignored `measure_following_playbacks_camera` (`feat/m6s-settling-both-ways`, this PC with other agents and a merge check running, 60 to 97% CPU load; five alternations each, the medians): the benchmark replay played as a live match plays it (`Playback::no_settling`, Slippi 3.19's rules, the allocator model off) with the switch on, with it off (the player's), and with playback's camera kept followed to the match's end as a pause keeps it. Three runs, the last with the switch off as the player's:

| Match | Ticks | Followed | Those ticks, following / not | The window's extra, of the match | Followed to the end |
| --- | --- | --- | --- | --- | --- |
| `fox-marth-ps` | 4,549 | 337 | 127.0 / 122.8 ms; 118.5 / 122.0 ms; 90.4 / 89.4 ms | +0.26%; -0.21%; +0.09% | +1.25%; +2.64% |
| Corneria (`oracle-m4/stage-corneria`) | 14,524 | 290 | 95.9 / 89.5 ms; 93.8 / 88.9 ms; 70.6 / 67.1 ms | +0.11%; +0.09%; +0.08% | -0.87%; +1.29% |

The window is within the machine's noise, well under 1% of a match's ticks; a match followed to its end costs about 1 to 3%. The snapshot while following is 1,680 bytes larger (1,931,316 against 1,929,636 bytes at `fox-marth-ps`'s tick 30, 2,122,552 against 2,120,872 on Corneria; the 70 boxes' followed extents and timers), one allocation more (472 against 471, 568 against 567), its clone within noise. With the switch off nothing is added (an empty vector and a `None`). `session-sync --rollback 7` holds through the follower (`--follow-playback-camera`): `pause-fd` (its live match's window and pause) 1,357 ticks in 2 matches, `matches-a` 18,505 ticks in 11 matches, no difference.

**The player's path against `main`** (`c6e274d5`; indicative, this PC under load, the worker to confirm): `melee-bench native --runs 5`, the two builds alternated three times, median frames per second. These replays are Vanilla's, so the camera's update runs its new shape (the follower's flag checks) without a follower:

| Match | `main` | `feat/m6s-settling-both-ways` |
| --- | --- | --- |
| `fox-marth-ps` | 3,822; 3,922; 4,101 | 3,842; 4,028; 4,072 |
| Corneria (`oracle-m4/stage-corneria`) | 4,069; 3,998; 4,183 | 4,018; 4,050; 4,137 |

Within noise both ways.

## Baselines on the worker (2026-11-19)

The speed targets' baselines at main `c6e274d5` (after batch 3), on [the worker](#machine) in exclusive reservations, release (`cargo build --locked --release -p melee-bench`, with `CARGO_PROFILE_RELEASE_DEBUG=line-tables-only`). Every run in sync. The survey's numbers redone are in [the refactor](refactor.md#measured-on-the-worker-2026-11-19).

**Simulation speed.** `melee-bench native --runs 5`, the allocator model on (the default) and off (`MELEE_BENCH_HEAP=off`), three rounds alternating with the survey's commit `f0fa4c81` (three jobs); median frames per second of the 15 runs, their range, and the three rounds' medians:

| Match | `c6e274d5`, model on | `c6e274d5`, model off | `f0fa4c81`, model on | `f0fa4c81`, model off |
| --- | --- | --- | --- | --- |
| `fox-fox-fd` | 2,222 (2,210 to 2,274; 2,222, 2,260, 2,216) | 2,226 (2,214 to 2,297) | 2,202 (2,119 to 2,218) | 2,234 (2,159 to 2,256) |
| `fox-marth-ps` | 1,895 (1,889 to 1,904; 1,898, 1,891, 1,896) | 1,955 (1,903 to 1,969) | 1,882 (1,830 to 1,894) | 1,938 (1,865 to 1,947) |
| `ganon-captain-fd` | 2,092 (2,076 to 2,128) | 2,106 (2,094 to 2,166) | 2,045 (2,008 to 2,085) | 2,130 (2,045 to 2,135) |
| Sarah's 3.19 replay (`MELEE_BENCH_FP=slippi`) | 2,310 (2,289 to 2,356; 2,290, 2,339, 2,345) | 2,368 (2,307 to 2,382) | 2,281 (2,212 to 2,293) | 2,265 (2,241 to 2,310) |

A fourth job (three more rounds against task 1's branch, the model on) gave main `fox-fox-fd` 2,193 (2,167 to 2,242), `fox-marth-ps` 1,877 (1,864 to 1,929) and Sarah's replay 2,321 (2,264 to 2,340): the same build moves by 1 to 2% from one job to another, so compare builds within a job. Sarah's replay is `$SLIPPI_REPLAYS/2026-01/Game_20260129T130802.slp` (`--dir` its folder). Dolphin's figures on the worker, and the target's ratio, are in [Dolphin on the worker](#dolphin-on-the-worker-2026-11-19).

```sh
for heap in on off; do  # MELEE_BENCH_HEAP=off for off
  melee-bench native --runs 5 fox-fox-fd fox-marth-ps ganon-captain-fd
  MELEE_BENCH_FP=slippi melee-bench native --runs 5 --dir "$SLIPPI_REPLAYS/2026-01" Game_20260129T130802
done
```

**Rollback.** `melee-bench snapshot-match --every 2500 MATCH` (Sarah's replay with `MELEE_BENCH_FP=slippi`, and `MELEE_BENCH_HEAP=off` for the model off), one run each:

| Match | Snapshot | Allocations | Clone | Restore | Restore allocations | Restore and 8 ticks | Allocations per tick |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `fox-fox-fd` | 1.94 to 2.01 MB | 495 to 498 | 241 to 247 µs | 161 to 165 µs | 0 | 3.33 to 4.42 ms | 1 |
| `fox-marth-bf` | 1.95 to 1.98 MB | 504 to 507 | 227 to 234 µs | 155 to 159 µs | 0 | 2.58 to 4.07 ms | 1 |
| `fox-marth-fd` | 1.98 to 2.03 MB | 509 to 517 | 227 to 238 µs | 159 to 165 µs | 0 | 3.36 to 3.96 ms | 1 |
| `fox-marth-ps` | 2.08 to 2.24 MB | 522 to 528 | 242 to 262 µs | 169 to 186 µs | 1 | 3.84 to 4.57 ms | 8 to 11 |
| `ganon-captain-fd` | 2.01 to 2.02 MB | 520 to 521 | 231 to 237 µs | 160 to 163 µs | 0 | 3.57 to 3.82 ms | 1 |
| Sarah's replay, model on | 2.16 to 2.23 MB | 614 to 623 | 244 to 268 µs | 164 to 183 µs | 0 | 3.01 to 4.48 ms | 1 |
| Sarah's replay, model off | 2.11 to 2.17 MB | 587 to 595 | 239 to 247 µs | 161 to 168 µs | 0 | 2.92 to 4.35 ms | 1 |

Against [the targets](#targets): the snapshot (under 2.3 MB) and its clone and restore (under 0.27 and 0.19 ms) are well inside them; restore and 8 ticks is over 4 ms on some states (`fox-marth-ps` at 2,500, 5,000, 7,500 and 12,500, `fox-fox-fd` at 5,000, `fox-marth-bf` at 15,000, Sarah's replay at 7,500, the model on and off), on a machine that runs the simulation at about half this one's speed; Pokémon Stadium's ticks still allocate (its text, as before).

**Rollback at main `160ae506` (2026-11-19, after the callback table).** The same command on [the worker](#machine) in one exclusive reservation, three rounds, so every state is measured three times. One build, `perf/worker-dolphin` `8d87876e`, whose simulation is main `160ae506`'s (and `4770a3ca`'s), release with line tables as above. The ranges over the rounds and states:

| Match | Snapshot | Allocations | Clone | Restore | Restore allocations | Restore and 8 ticks (the worst state) | Allocations per tick |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `fox-fox-fd` | 1.95 to 2.02 MB | 495 to 498 | 215 to 237 µs | 147 to 164 µs | 0 | 2.73 to 3.86 ms (5,000: 3.74 to 3.86) | 1 |
| `fox-marth-bf` | 1.95 to 1.98 MB | 504 to 507 | 215 to 242 µs | 148 to 163 µs | 0 | 2.06 to 3.59 ms (10,000: up to 3.59) | 1 |
| `fox-marth-fd` | 1.98 to 2.03 MB | 509 to 517 | 217 to 239 µs | 149 to 164 µs | 0 | 2.73 to 3.39 ms (15,000: up to 3.39) | 1 |
| `fox-marth-ps` | 2.08 to 2.24 MB | 522 to 528 | 232 to 263 µs | 161 to 187 µs | 1 | 3.22 to 4.09 ms (12,500: 3.97, 4.09, 3.96; 5,000: 3.78, 3.94, 3.81) | 8 to 11 |
| `ganon-captain-fd` | 2.01 to 2.02 MB | 520 to 521 | 223 to 238 µs | 153 to 164 µs | 0 | 3.00 to 3.35 ms | 1 |
| Sarah's replay, model on | 2.16 to 2.23 MB | 614 to 623 | 241 to 267 µs | 165 to 182 µs | 0 | 2.46 to 3.84 ms (7,500: up to 3.84) | 1 |
| Sarah's replay, model off | 2.11 to 2.17 MB | 587 to 595 | 237 to 261 µs | 161 to 178 µs | 0 | 2.49 to 3.85 ms (7,500: up to 3.85) | 1 |

**Restore and 8 ticks is not yet under 4 ms on every state.** Every state is under it but one: `fox-marth-ps` at tick 12,500 ran 3.97, 4.09 and 3.96 ms (one run of three over, the median 0.03 ms under), and its tick 5,000 is next at 3.78 to 3.94 ms. Every other state is at most 3.86 ms. At `c6e274d5` eight states were over (above). Pokémon Stadium's ticks still allocate. The processor was 0% busy before and after each round.

```sh
melee-bench snapshot-match --every 2500 MATCH   # each of the five matches, three rounds
MELEE_BENCH_FP=slippi [MELEE_BENCH_HEAP=off] melee-bench snapshot-match --every 2500 --dir "$SLIPPI_REPLAYS/2026-01" Game_20260129T130802
```

## Pokémon Stadium's allocations and its worst state (2026-11-21)

[The refactor's](refactor.md#task-20-pokémon-stadiums-allocations-and-rollbacks-worst-state-done-2026-11-21-perfstadium-allocs-from-7871ac5b) task 20, `perf/stadium-allocs` from `7871ac5b`.

**Allocations.** `melee-bench alloc-sites --from 1000 --ticks 3600 --rollback fox-marth-ps` (the allocation test's stretch) found three sources. The text system's objects and blocks (`hsd::sis`) were `BTreeMap`s copied whole on a restore whenever they changed, which the screen's texts do every frame: one allocation a restore. The materials' rendermode marks (`Objects::rendermode_patches`) are a `HashMap`, whose `clone_from` builds a new table when the two sizes differ, so a transformation's load after a restore allocated it again (tick 3,810). And each text a display pass draws was laid out into a new glyph vector (7 allocations a pass) in a new box (3 a pass, one on the other matches). Now the text system's maps are flat sorted vectors (`flat_map::FlatMap`, which compares and hashes as the `BTreeMap` did), restored into the storage the target holds, and the static string buffers are a struct, so their bytes are copied in place; the marks are refilled on a restore (`#[clone_from(refill)]`); and a text is laid out into the storage of a draw the frontend gave back with its frame or a state-only pass dropped (`Display::spare_texts`). The stretch makes no allocation now (ticks, display passes and the restore), nor does any other benchmark match. `ticks_and_restores_dont_allocate` replays Pokémon Stadium too, and counts each display pass with its draws built and the frame given back. Mutation-tested: a new box for each text fails it on every match's display passes, the marks restored by `clone_from` at tick 3,810 and the restore, and the flat maps' `clone_from` building anew on every restore (4 allocations).

**What made tick 12,500 the worst.** `melee-bench rollback-split` (above) and sampled profiles of its windows alone (`perf record` on the worker; the survey's sampler on this PC). At tick 12,500 the stage is in its water transformation, and its ticks cost more than other states' by several small things, none dominant (shares of the window's samples): collision 9% on the worker (the transformation's lines: the floor and ceiling searches extended every candidate line's ends, `mpLib_8004ED5C`, a square root and four divisions, before testing it), each fighter's screen mark 5% (projecting its camera box onto the screen recomputed the camera's up vector from its roll, its look-at matrix and its perspective matrix every time; one fighter was off screen, which projects twice), the particles 6% and the screen's text 3%, these three on this PC. Two changes, beside the allocations:

- The camera's look-at matrix (`C_MTXLookAt`), perspective matrix (`MTXPerspective`) and up vector from a roll (`roll2upvec`) are kept in memo tables, as `Mtx34::srt` is: each depends only on its inputs' bits and the float mode. `camera_routines_match_uncached` holds them against the uncached routines in both float modes.
- The floor and ceiling searches (`mpCheckFloor` and the ceiling's, `Geometry::search_r5`) pass over a line at least a unit long whose vertices' box, four units wider each way, misses the segment's box. Its extended ends can't reach the segment (the extension moves the first end a unit and the second (length + 1) / length units), and both of the tests start by rejecting a line wholly beside the segment, so the test finds nothing there, as before. Only the r5 the last tested line leaves, which a computer player's DI reads, needs that line's test, which runs after the loop when it was passed over. `a_line_passed_over_is_never_crossed` holds the rule against the full test on random lines and segments, and `passing_over_lines_finds_what_testing_every_line_does` the search against one testing every line, hits and r5. Mutation-tested: without the length condition, with no margin, or without the r5 worked out after the loop, one of them fails. The verify list's 30 Ice Climbers replays (and three player-code replays of their games) pass with that r5 left unworked, so the unit test is what holds it.

**Every state on the worker.** `melee-bench rollback-split --every 2500 --runs 200 MATCH` (Sarah's replay with `MELEE_BENCH_FP=slippi`), restore and 8 ticks, the median of 200 windows a state; the base (`7871ac5b` with the branch's `tools/bench`) and the branch (`d3094097`) alternating over three rounds in one exclusive reservation, release with line tables. The worst state of each match, its three rounds, and the branch's range over every state:

| Match | Base, worst state | Branch, worst state | Branch, every state |
| --- | --- | --- | --- |
| `fox-fox-fd` | 5,000: 3.70, 3.80, 3.80 ms | 5,000: 3.63, 3.72, 3.63 ms | 2.64 to 3.72 ms |
| `fox-marth-bf` | 15,000: 3.40, 3.48, 3.42 | 15,000: 3.39, 3.40, 3.42 | 1.99 to 3.42 |
| `fox-marth-fd` | 15,000: 3.28, 3.37, 3.27 | 2,500: 3.30, 3.28, 3.28 | 2.70 to 3.30 |
| `fox-marth-ps` | 12,500: 3.96, 4.02, 3.93 | 5,000: 3.76, 3.75, 3.66 (12,500: 3.63, 3.63, 3.65) | 3.10 to 3.76 |
| `ganon-captain-fd` | 7,500: 3.30, 3.21, 3.22 | 7,500: 3.21, 3.13, 3.15 | 2.88 to 3.21 |
| Sarah's 3.19 replay | 7,500: 4.06, 3.84, 3.93 | 7,500: 3.74, 3.63, 3.75 | 2.35 to 3.75 |

**Restore and 8 ticks is under 4 ms on every state**, the worst 3.76 ms. At tick 12,500 the window's parts went from 183 to 195 µs (restore), 2.72 to 2.80 ms (the 8 engine ticks) and 0.99 to 1.02 ms (the 8 display passes) to 192 to 195 µs, 2.47 to 2.60 ms and 0.92 to 0.97 ms. Step by step there (`rollback-split --at 12500 --runs 400`, three rounds in each of two earlier reservations): the base 4.03 to 4.21 ms, the allocations removed 3.92 to 4.07, the memo tables 3.84 to 4.00, the collision searches 3.64 to 3.69. The other states' worst moved less, 0 to 5%. `snapshot-match`'s median of five windows moves by up to 0.4 ms between runs of one state there (12,500 at 3.86 to 4.32 ms in one build), so these numbers are `rollback-split`'s. The processor was idle before and after each round, and every run in sync.

```sh
melee-bench rollback-split --every 2500 --runs 200 MATCH   # the five matches, three rounds
MELEE_BENCH_FP=slippi melee-bench rollback-split --every 2500 --runs 200 --dir "$SLIPPI_REPLAYS/2026-01" Game_20260129T130802
melee-bench rollback-split --at 12500 --runs 400 fox-marth-ps
```

On this PC, with other agents' work running (indicative only): `rollback-split --at 12500` on the branch's allocation commit and on its memo commit, three rounds alternating, moved between 1.6 and 2.5 ms from run to run, more than the change, so it shows nothing either way.

## Which build (2026-11-21)

[The refactor's](refactor.md#task-17-the-players-build) task 17. Players run `dist` (`cargo run -p dist`: fat LTO, one codegen unit); the speed targets are measured on `release` (sixteen codegen units, no LTO), as Sarah decided (2026-11-21, design call 10: "if the quick build does, so too should the player's build"; to be revisited if the speed tasks run out of meaningful wins before a target is reached). The bench builds with `dist`'s settings as the `bench-dist` profile, to report the gap: `cargo build --locked --profile bench-dist -p melee-bench`, its binary in `target/bench-dist`.

**Measured on [the worker](#machine)**, main `8f421fde` with the profile added (`perf/player-build` `a8dd8d5c`; nothing in the simulation changed): four builds of `melee-bench` at `-j 3`: `release`, `bench-dist`, and two PGO builds of `bench-dist` (below). Three exclusive reservations of `melee-bench native --runs 5` (the six benchmark matches, and Sarah's 3.19 replay with `MELEE_BENCH_FP=slippi`), the builds' order reversed every other round, and two of `snapshot-match --every 2500`, the builds alternating match by match; the processor 0 to 3% busy before and after each build's runs. Every run was in sync. Median frames per second of the 15 runs, their range, and the change against `release`:

| Match | `release` | `bench-dist` | PGO, all | PGO, replays only |
| --- | --- | --- | --- | --- |
| `fox-fox-fd` | 2,620 (2,600 to 2,658) | 2,837 (2,808 to 2,895), +8.3% | 3,151 (3,040 to 3,206), +20.3% | 2,759, +5.3% |
| `fox-marth-bf` | 2,562 (2,546 to 2,576) | 2,794 (2,756 to 2,831), +9.1% | 3,093 (3,058 to 3,129), +20.7% | 2,697, +5.3% |
| `fox-marth-fd` | 2,430 (2,420 to 2,445) | 2,664 (2,614 to 2,689), +9.6% | 2,936 (2,896 to 2,968), +20.8% | 2,567, +5.6% |
| `fox-marth-ps` | 2,176 (2,159 to 2,190) | 2,374 (2,352 to 2,420), +9.1% | 2,659 (2,621 to 2,700), +22.2% | 2,347, +7.9% |
| `ics-peach-ps` | 1,696 (1,664 to 1,707) | 1,827 (1,817 to 1,845), +7.7% | 2,052 (2,033 to 2,094), +21.0% | 1,823, +7.5% |
| `ganon-captain-fd` | 2,436 (2,411 to 2,462) | 2,652 (2,634 to 2,661), +8.9% | 2,949 (2,917 to 3,012), +21.1% | 2,589, +6.3% |
| Sarah's 3.19 replay | 2,801 (2,765 to 2,817) | 3,161 (3,135 to 3,215), +12.9% | 3,604 (3,568 to 3,665), +28.7% | 3,652 (3,595 to 3,673), +30.4% |

The rounds' medians agree within 3% for every build. `release` here is faster than the [baselines above](#baselines-on-the-worker-2026-11-19) (`fox-fox-fd` 2,620 against 2,222 at `c6e274d5`): main has gained since (task 1's callback table among it), and these builds have no line tables.

**Restore and 8 ticks** (`snapshot-match --every 2500`, two rounds; each state's median of the rounds, then the mean over a match's states and its worst state):

| Match | `release`: mean, worst | `bench-dist` | PGO, all | PGO, replays only |
| --- | --- | --- | --- | --- |
| `fox-fox-fd` | 3.12, 3.82 ms (5,000) | 2.94, 3.59 (−5%) | 2.53, 3.15 (−19%) | 2.90, 3.79 (−7%) |
| `fox-marth-bf` | 2.86, 3.51 (15,000) | 2.67, 3.27 (−7%) | 2.40, 2.94 (−16%) | 2.66, 3.29 (−7%) |
| `fox-marth-fd` | 3.06, 3.35 (15,000) | 2.89, 3.17 (−6%) | 2.61, 3.10 (−15%) | 2.95, 3.35 (−4%) |
| `fox-marth-ps` | 3.75, 4.07 (12,500) | 3.50, 3.82 (−7%) | 3.14, 3.43 (−16%) | 3.49, 3.84 (−7%) |
| `ics-peach-ps` | 4.59, 5.71 (12,500) | 4.46, 5.58 (−3%) | 3.98, 5.08 (−13%) | 4.37, 5.48 (−5%) |
| `ganon-captain-fd` | 3.19, 3.38 (7,500) | 2.93, 3.06 (−8%) | 2.67, 2.80 (−16%) | 3.04, 3.25 (−5%) |
| Sarah's 3.19 replay | 3.09, 3.80 (7,500) | 2.76, 3.45 (−11%) | 2.44, 3.05 (−21%) | 2.43, 3.04 (−21%) |

The snapshot's clone and restore move by a few µs either way (clone 218 to 289 µs, restore 134 to 197 µs over every match and build); Pokémon Stadium's ticks still allocate (8 to 12 a tick). **`ics-peach-ps`, measured here for the first time since its new recording, is over the 4 ms target in every build**: in `release` its five states take 3.8 to 5.8 ms, four of them over 4 ms in both rounds, the worst tick 12,500 at 5.63 and 5.78 ms (5.18 and 4.98 with PGO); `fox-marth-ps`'s tick 12,500 is at the line (4.06 and 4.08 ms) as [before](#baselines-on-the-worker-2026-11-19).

**PGO.** Profile-guided optimization with rustc's instrumentation (`llvm-tools`, installed on the worker for this): `bench-dist` built with `-Cprofile-generate`, trained, the raw profiles merged with `llvm-profdata`, and `bench-dist` built again with `-Cprofile-use`. Two profiles from the same instrumented build: **all**, the benchmark matches (`native --runs 1` on the six and Sarah's replay, and `snapshot-match --every 5000` on `fox-fox-fd`, `fox-marth-ps` and Sarah's replay) and the author's replays (`corpus --jobs 3` on 202 of `$SLIPPI_REPLAYS`, Sarah's 3.19 replay left out: 171 played to their ends and 31 stop as their matches start on characters no port has, as [the oracle](oracle.md) records); and **replays only**, the corpus run alone, which plays under Slippi 3.19's rules in Slippi's float mode and never ran the replay it is measured on. The replays-only profile gives the full gain on Sarah's replay, a replay it never saw (+15.5% over `bench-dist`), but none on the Vanilla benchmark matches, which run the console's float mode and Vanilla's rules: −0.2 to −3.6% against `bench-dist`. PGO pays on the code its training ran and costs a little on code it didn't (unprofiled code is laid out and inlined as cold).

**Cost.** On the worker (`-j 3`, other jobs running beside some builds): `release` 4.7 minutes, `bench-dist` 7.3, the instrumented build 9.3, the training 34 (1.5 for the benchmark matches, 31 for the replays; the instrumented build ran the bench at about 70% of `bench-dist`'s speed), the merge seconds, each PGO build 4.8. Every build peaked at 9.0 to 9.7 GB (GNU `time`'s maximum resident size; one codegen unit doesn't take more than sixteen). A merged profile is 9.6 MB, over the repository's 5 MiB limit for a file, and a profile matches only the functions it was taken from (a changed function's counts are dropped, with a warning), so a PGO player would make its profile at build time: `dist` builds the player instrumented, plays a training set, merges, and builds again. That set would have to cover every path players take (both rulesets and float modes, the renderers, audio, the menus from power-on: the player itself, `melee-hd --bench` on replays and `--inputs` sessions, not the bench), or what it misses runs slower, as the replays-only profile shows. On this PC the player's `dist` build took 24 minutes (busy), so a PGO `dist` would take roughly an hour. Not made the player's build here: proposed, for Sarah's decision.

**Indicative, this PC** (busy with other agents' builds, `melee-bench native --runs 5`, three rounds alternating, the round medians): `fox-fox-fd` `release` 3,475, 3,483, 3,594; `bench-dist` 3,856, 3,722, 3,733; PGO (all, trained on this PC) 4,453, 4,496, 4,629. `fox-marth-ps` 3,024, 2,950, 3,091; 3,126, 3,197, 3,130; 3,706, 3,903, 4,021. `ics-peach-ps` 2,246, 2,289, 2,342; 2,452, 2,399, 2,406; 2,887, 2,854, 3,011. Sarah's replay 3,525, 3,488, 3,671; 4,254, 4,210, 4,137; 5,343, 5,233, 5,275. The same order as the worker: `bench-dist` 4 to 19% over `release` (medians of the rounds), PGO 20 to 25% over `bench-dist`. On this PC `bench-dist` built in 15 minutes, the instrumented build in 17 and the PGO build in 14, and the PGO build on Windows needs nothing more than on Linux (`llvm-tools` was already installed).

```sh
cargo build --locked --release -p melee-bench                 # the deciding build
cargo build --locked --profile bench-dist -p melee-bench      # the player's settings, target/bench-dist
# PGO (trial): instrument, train, merge, rebuild
CARGO_TARGET_DIR=target-pgo-gen RUSTFLAGS=-Cprofile-generate=/tmp/pgo cargo build --locked --profile bench-dist -p melee-bench
LLVM_PROFILE_FILE=/tmp/pgo/%p-%m.profraw target-pgo-gen/bench-dist/melee-bench native --runs 1    # and the replays: corpus --list FILE
"$(rustc --print sysroot)/lib/rustlib/x86_64-unknown-linux-gnu/bin/llvm-profdata" merge -o pgo.profdata /tmp/pgo/*.profraw
CARGO_TARGET_DIR=target-pgo RUSTFLAGS=-Cprofile-use=$PWD/pgo.profdata cargo build --locked --profile bench-dist -p melee-bench
```

## PGO for releases (2026-11-21)

Sarah's answer to the trial (2026-11-21): "i want our releases to be as performant as we can make them so if there's an optimization right on the table there we should take it. but yeah we only need that for release builds, we generally want to avoid things that slow the workflow like the plague. so, PGO off for everything except release builds, in which case PGO mandatory". So **every build a GitHub release carries is profile-guided, and no other build is**: the dist tool's `--pgo` (Windows), and for Linux its `--pgo-profile` on a machine with the disc and `--pgo-use` in the release workflow ([the player](player.md#playing-it), "Releases", has the steps and the checks). `release` stays the build speed targets are measured on ([design call 10](refactor.md#design-calls)); `cargo build`, the plain `cargo run -p dist`, CI, the bench and the tests are as they were.

| Build | Profile | Who makes it | What it's for |
| --- | --- | --- | --- |
| `release` | 16 codegen units, no LTO | anyone | development; the speed targets |
| `dist`, plain (`cargo run -p dist`) | fat LTO, 1 codegen unit | the coordinator, `dist/` after each push | playing between releases |
| `bench-dist` | `dist`'s settings, for `melee-bench` | anyone | the gap to the player's build |
| **`dist` with PGO** (`--pgo`, `--pgo-profile` and `--pgo-use`) | `dist`'s settings and a profile from a training set | the coordinator, for a release | **a GitHub release's Windows exe and Linux tarball** |

**The training set** (`tools/dist/src/train.rs`) is the instrumented player's own headless runs (`--bench`), since a profile matches only the program it was taken from, and it covers what players run, as task 17's replays-only profile showed what a narrow one costs: from power-on under each ruleset, the menus driven by scripted controllers (`--holds`) into VS mode, Fox (port 1 pressing a fixed pseudo-random sequence) against a level 9 computer Yoshi for two minutes (Fountain of Dreams under Vanilla with a memory card, Battlefield under the Slippi ruleset with everything unlocked), the results and back to the character select, with high refresh's predicted frames (Extrapolate, two a tick); the Slippi ruleset's match saved as a replay and played back (Slippi's rules and Slippi Dolphin's floating point); and the six benchmark matches (Vanilla's rules, the console's floating point, the disc at its fast speed so they play to their ends), one each with Extrapolate and widescreen, the exact renderer (6,000 ticks), MSAA 4 and widescreen alone, the rest the defaults (the fast renderer). Every run mixes the sound (headless, with no device). Left out, as host glue rather than per-tick work: the window and its events, presenting to a display, the just-in-time tick timing and the sound device. The power-on runs start from the host's clock, which Melee seeds its random numbers from, so the computer player fights differently each time; the menus' steps don't depend on it. `cargo run -p dist -- --train-with EXE` plays the set on any build.

**Measured on [the worker](#machine)** (Linux; exclusive reservation, the processor 1 to 3% busy before), the player's `--bench` at its defaults (the fast renderer, 1280 by 960, high refresh off), 6,000 ticks of each benchmark match and of Sarah's 3.19 replay (`Game_20260129T130802`, in no training run: Slippi's rules and floating point), and 1,500 of `fox-fox-fd` with the exact renderer (trained only on `fox-marth-fd`): three builds of `7b7acb25` (the player is the same as at `1b4d940c`), `release`, `dist` plain, and `dist` with PGO made as a release's is, its profile from `--pgo-profile` in one checkout and the build from `--pgo-use` in another (another path and target folder, as the workflow's). Five rounds, the builds' order reversed every other round, after a warm-up run of each that filled the pipeline cache. The median of the five, and the change against `release` and against `dist`:

| Run | `release`: tick + frame, p50 | `dist` | `dist` with PGO | PGO against `dist` |
| --- | --- | --- | --- | --- |
| `fox-fox-fd` | 3.29 ms, 2.61 ms | 3.12 (−5.0%), 2.41 (−7.5%) | 2.88 (−12.5%), 2.20 (−15.9%) | −7.8%, −9.0% |
| `fox-marth-bf` | 3.21, 2.67 | 3.25 (+1.2%), 2.49 (−6.7%) | 2.83 (−11.9%), 2.23 (−16.6%) | −12.9%, −10.5% |
| `fox-marth-fd` | 3.33, 2.63 | 3.10 (−7.1%), 2.45 (−7.1%) | 2.87 (−14.0%), 2.19 (−17.0%) | −7.4%, −10.6% |
| `fox-marth-ps` | 3.52, 2.95 | 3.28 (−6.9%), 2.69 (−8.8%) | 3.09 (−12.2%), 2.48 (−16.0%) | −5.7%, −7.9% |
| `ics-peach-ps` | 3.82, 3.14 | 3.50 (−8.5%), 2.86 (−8.9%) | 3.28 (−14.2%), 2.63 (−16.2%) | −6.2%, −8.1% |
| `ganon-captain-fd` | 3.53, 2.70 | 3.08 (−12.7%), 2.42 (−10.3%) | 2.86 (−18.8%), 2.19 (−18.9%) | −7.0%, −9.6% |
| Sarah's 3.19 replay (not trained) | 3.44, 2.57 | 2.97 (−13.6%), 2.30 (−10.6%) | 2.84 (−17.4%), 2.13 (−17.2%) | −4.4%, −7.3% |
| `fox-fox-fd`, exact renderer (not trained on this match) | 4.96, 4.31 | 5.02 (+1.2%), 4.15 (−3.8%) | 4.36 (−12.0%), 3.66 (−15.2%) | −13.1%, −11.8% |

"Tick + frame" is the bench's simulation a tick plus its drawing a frame (means over the run; one frame a tick); "p50" its median frame with its ticks. The PGO build is the fastest of the three on every run, by both measures, and in 79 of the 80 pairings round by round (the one exception, `fox-marth-fd`'s first round against `dist`: 3.046 against 3.042 ms on the mean, 2.27 against 2.42 at p50): the simulation alone 6 to 12% under `dist`'s (`fox-fox-fd` 1.02 to 0.95 ms a tick, `ics-peach-ps` 1.20 to 1.08, Sarah's replay 0.98 to 0.92), the drawing 4 to 14%. The two runs no training run played (Sarah's replay, the exact renderer on another match) gained too, the replay least (4.4% on the mean): no path measured is slower than in the plain build. The smoke run (`melee-hd --bench 900 --replay data/bench/fox-marth-ps.slp`, the PGO build, a cold pipeline cache) played to its end. The commands: `perf-pgo` folders on the worker (deleted).

**Indicative, this PC** (Windows, busy with other agents' builds; ranges up to 40% wide): the Windows `--pgo` exe against `release` and the plain `dist` build, the same bench, three rounds alternating, the medians of tick + frame: `fox-fox-fd` 3.43, 3.28 and 2.93 ms; `fox-marth-ps` 4.60, 4.77, 4.10; `ics-peach-ps` 7.65, 5.99, 5.90; Sarah's replay 4.47, 4.58, 4.12. PGO the fastest on each, 2 to 14% under `dist`, as on the worker; the Windows exe can't run there, so these are its only numbers. Its smoke run (`--bench 900 --replay data/bench/fox-marth-ps.slp`) played to its end.

**The profile.** On Linux 21.4 MB, on Windows 18.4 MB (task 17's bench-only profile was 9.6): too big for the repository even if it didn't go stale with every change, which is why each release makes its own. A profile made from the commit before, refused by `--pgo-use` before anything was built ("the profile was made from another build: its commit is `1b4d940c...`, this build's `7b7acb25...`"); one made in one checkout and used in another, with another path and target folder, gave the same `-C metadata` for the player crate (`4890cfb0c45edb1e`) and no hash mismatch. On Windows, the PGO `melee-hd.exe` has no local path in it (no user name among its strings), as the plain build.

**Cost**, one release: on the worker (`-j 3`, oracle jobs running beside some builds) `--pgo-profile` took 34 minutes (the training 21 of them: the Vanilla run 6.3, the Slippi run 2.2, its replay 0.8, the six matches 12.0; the instrumented build and the merge the rest), `--pgo-use` 8.8; on this PC (`-j 4`, other agents' builds running throughout) `--pgo` took 104 minutes: the instrumented build 34, the training 47 (the Vanilla run 29 of it, 6 to 7 alone), the final build 22. The plain `cargo run -p dist` built in 17.4 minutes on this PC, the same command as before (`cargo build --locked --profile dist -p melee-player -j 4`), and the worker's plain `dist` 18.2 and `release` 10.3 (with oracle jobs). The Linux workflow's build gains only the profile's download. The instrumented player plays at about the plain build's speed (the Vanilla menus' first 2,000 ticks in 75 s on both, this PC).

**The Linux build's glibc.** The worker (Ubuntu 26.04, glibc 2.43) builds a program needing glibc 2.43 (`acosf`, `asinf`, `atan2f`, `coshf`, `sinhf` and `acoshf` at 2.43, `pidfd_getpid` and `pidfd_spawnp` at 2.39), every build alike, so the release's Linux build stays on GitHub's Ubuntu 22.04 runner, where the workflow now fails a program needing anything newer than 2.35. That check runs only in the workflow, at a release: not run for this change (no workflow runs against the repository but the coordinator's).

**Vanilla's console is slow.** The training's Vanilla run (menus and a match from power-on, Extrapolate) ran its simulation at 11.3 to 11.6 ms a tick and its prediction at 12.6 to 13.3 more on this PC, against the Slippi ruleset's 2.3 and 2.2 for the same script: worth a look by the speed tasks, as Vanilla is still the player's default ruleset.

## Benchmark matches

| Match | Stage | Fighters (costume) | Seed | Frames | Length | Ends |
| --- | --- | --- | --- | --- | --- | --- |
| `fox-fox-fd` | Final Destination | Fox (0) vs Fox (1) | `1F0F0001` | 15,575 | 4:20 | port 2 out of stocks, port 1 on 1 |
| `fox-marth-bf` | Battlefield | Fox vs Marth | `1F0F0002` | 15,110 | 4:12 | Marth out of stocks, Fox on 3 |
| `fox-marth-fd` | Final Destination | Fox vs Marth | `1F0F0003` | 15,241 | 4:14 | Fox out of stocks, Marth on 1 |
| `fox-marth-ps` | Pokémon Stadium | Fox vs Marth | `1F0F0004` | 13,585 | 3:46 | Fox out of stocks, Marth on 1 |
| `ics-peach-ps` | Pokémon Stadium | Ice Climbers vs Peach | `1F0F0015` | 13,659 | 3:48 | Ice Climbers out of stocks, Peach on 1 |
| `ganon-captain-fd` | Final Destination | Ganondorf vs Captain Falcon | `1F0F0006` | 12,067 | 3:21 | Ganondorf out of stocks, Captain Falcon on 1 |

Each is a VS match (4 stocks, 8 minutes, items off, pausing off; settings as a Slippi Dolphin VS match records them) with both players human, and the controllers both played. `ics-peach-ps` was recorded again on 2026-11-19, in the heap layout playback gives ([Ice Climbers and the heap layout](#ice-climbers-and-the-heap-layout)); every `ics-peach-ps` figure dated before then is the old recording's, a different match. The Fox–Fox match was the first measured, when only Fox was ported. Frames count from the first (`-123`) to the last. The replays are in the repository, `data/bench/NAME.slp` (inputs the CPU AI played, no game data); `make-matches.sh` writes them with the Gecko codes (`NAME.ini`, below), ssbm-rs's start traces (`NAME.start.trace`) and the logs of making and checking them into its folder (ssbm-rs's `local/bench`, `$SSBM_RS_LOCAL/bench`, where `melee-bench native` finds the start traces).

**How they're made.** None of the author's replays is a Fox–Marth or Ice Climbers–Peach singles match, and a CPU match can't simply be replayed as a human one: a CPU's stick reads v/127 where a controller's reads r/80 after Melee's clamp, so its inputs mostly aren't a controller's, and the CPU AI draws from the game's random seed, so without it every later random number moves. So the game's level-9 CPU AI plays both fighters *through their controllers*: in ssbm-rs, around `Fighter_procCpu` each human player counts as a CPU, the AI decides, the random seed is put back as it was, and what the AI asked for (sticks scaled to a controller's range, triggers to its) becomes that controller's next poll. The fighters do only what the controllers say, and the match is the CPUs' closed-loop play. Before "GO" the controllers stay centred.

The match starts the way Slippi's playback starts a replay: Gecko codes (`melee-bench codes DIR`, from `tools/bench/gecko/*.s` and Slippi's own boot, Init Player Data, Init Stage Data and ExtendPlayerBlock codes) boot into DebugMelee's result screen, which loads the match's files and moves on to the match, and StartMelee takes the match's settings and seed where Slippi's playback restores a replay's game info. No EXI device is involved. Slippi's recording codes (loaded by its bootloader in ssbm-rs) record the match as a replay. Slippi's playback set also has "ScenePrep_DebugResult = ScenePrep_DebugMenu" (`041B16A8 801B09C0`), which would break the result screen; Dolphin's INI reader drops it (its comment has an `=`), so playback never applies it and neither do these codes.

The floating point is the console's (ssbm-rs's `Hardware` mode, which mainline Dolphin reproduces), not Slippi's Ishiiruka Dolphin's, so these replays are for this harness, ssbm-rs and melee-hd; Slippi Dolphin may play them differently.

**Checks.** Each match is played again two ways, and both must find no difference from the recording in any compared field of any frame:

- ssbm-rs's replay check: the match played back through Slippi's playback codes (a different boot path, the recorded inputs injected per frame), with `--fp hardware` and Show Player Names off (`SLIPPI_PLAYER_NAMES=0`, the heap layout the allocator model and the oracle's traces have; `NAME.check.log`).
- The recorded controllers alone (`BENCH_PADS`), without the AI (`NAME.pads.log`). This is what Dolphin and melee-hd get.

All six pass both: 95,826, 91,239, 92,122, 82,093, 111,829 and 72,403 events compared, 0 divergences (the five others' checks ran with the names on, before 2026-11-19; nothing in them reads a heap address). `ganon-captain-fd` is there for the oracle (no replay paired Ganondorf with ported characters on a ported stage when it was made), not for the speed target.

**Regenerating them.** Build ssbm-rs's runner from its main branch (`3f9ac3a` and later; `cargo build --locked --release -p ssbm-run`; its `tools/run/src/bench.rs` has `BENCH_CODES`, `BENCH_AI`, `BENCH_PADS` and `BENCH_SLP`), then:

```sh
SSBM_RUN=.../ssbm-run.exe SSBM_DISC=... JOBS=3 bash tools/bench/make-matches.sh "$SSBM_RS_LOCAL/bench"
```

For each match it writes the codes, records it (`BENCH_CODES=NAME.ini BENCH_AI=0,1 BENCH_SLP=NAME.slp ssbm-run --fields 31000`), runs both checks, and saves ssbm-rs's state trace of the match's first frames (`NAME.start.trace`, with `--port sysdolphin/baselib/perf`) for the fighters' heap addresses. It is deterministic: rerunning it after ssbm-rs's runtime fix ab3cd01 made byte-identical replays. Each run takes ssbm-rs 10 to 15 minutes on one core.

**Other matches.** The same generator (`tools/bench/src/matches.rs`) also sets a match's rules (stock or time, with a time limit), its item frequency and switch, two to four players and computer players of any CPU kind and level, on any VS stage. Milestone 4's oracle sets are made with it (`melee-bench oracle-sets`, `make-oracle.sh`; see [the oracle](oracle.md#oracle-sets)); the benchmark matches' codes are unchanged.

### Ice Climbers and the heap layout

**Why the layout matters.** Nana is the only fighter in these matches that runs computer player code (the controllers play the others), and the computer player's DI with no knockback (`ftCo_800AC5A0`) takes its stick from registers it never set: the Y stick is the low byte of `r30`, her `Fighter`'s address ([Melee's bugs](melee-bugs.md#a-computer-players-di-with-no-knockback-reads-two-registers-it-never-set)). So an Ice Climbers match goes the same way only where the fighters have the same addresses, and "same match and start" includes the heap's layout. melee-hd plays a replay in the layout Slippi's playback gives (the allocator model, `Layout::slippi_playback` with the replay's code list), as ssbm-rs's playback with Show Player Names off does, which the oracle's traces use.

**The old recording (before 2026-11-19; seed `1F0F0005`, 12,655 frames).** ssbm-rs recorded it under its recording boot (Slippi's bootloader with the recording codes only, whose code list carves a smaller heap from the arena, and whose codes hold fewer blocks), which put every fighter `0x2720` bytes below playback's (Nana at `0x80F62420` against `0x80F64B40`: a stick byte of `0x20` against `0x40`), and Dolphin's run of the boot codes alone put them a further `0x1DC0` below (`0x80F60660`, `0x60`). At frame 10,137 Peach back-throws Nana with no knockback, so melee-hd (in the layout of ssbm-rs's playback with the names off) left the recording at frame 10,139 (Nana's release), and Dolphin did too until a benchmark-only DI patch (`di.s`, removed) pinned that byte to the recording's. Every `ics-peach-ps` figure dated before 2026-11-19 is that match's; its files stay in `$SSBM_RS_LOCAL/bench/ics-peach-ps.*` (with its `.dolphin` calibration) and in git's history.

**The layout code.** A match listed in `matches::PLAYBACK_LAYOUT` (only `ics-peach-ps`) gets one more code (`tools/bench/gecko/layout.s`, injected at `0x80068EE4` in `Fighter_Create`, just before it allocates the fighter's block): if the heap's first free cell starts below the cell playback's first fighter takes (its address there, `0x80EB5F80`, less the cell's `0x20`-byte header), it allocates the difference once, so the first fighter lands where playback puts it, and so does everything the heap gives out after it (the fighters are made in the same order with the same allocations in every setup). Later fighters find the cell past it and allocate nothing. The code is part of the match's codes, so ssbm-rs's recording run and Dolphin both apply it (by the gaps above, the first fills `0x2720` bytes and the second `0x44E0`), and nothing else differs: Dolphin needs no patch of its own. Playback doesn't have it and doesn't need it. `melee-bench dolphin` refuses a match with a fighter running computer player code whose codes lack it. `melee-bench`'s `playback_layouts_are_the_models` checks the address against the allocator model playing the replay, and the recording run's start trace (`NAME.start.trace`) against the model: Popo, Nana and Peach at `0x80EB5F80`, `0x80F64B40` and `0x81002F40` in both. It fails with the address changed by `0x20`, and with the old recording's start trace beside the new replay.

The code places the fighters where it says in Dolphin too: the old recording, played with the layout code aimed at its own recording's addresses (`0x80EB3860`) and no DI patch, stays in sync to its end, final stocks and percents equal; aimed at playback's, it leaves the replay at frame 10,139, as before.

**The new recording (2026-11-19, seed `1F0F0015`).** Recorded with the layout code by `make-matches.sh` (ssbm-rs's `target/dev/release/ssbm-run.exe`, built 2026-11-06; the files in `$SSBM_RS_LOCAL/bench/ics-layout/`, the replay copied to `data/bench`), and recorded again byte for byte. Nana's DI with no knockback is reached: first at frame 6,808 (`melee-bench native` with the model off stops there by name, and so does `slippi_cpu_di_reads_neither_register`, which reads that frame now), and the match turns on it: with the layout code aimed at the old recording's addresses instead, Dolphin leaves the replay at frame 6,844. The same codes with seed `1F0F0005` made a match that, from frame 5,079 on, differs from the old one (the controllers the AI played, port 1's stick first) and never reached Nana's DI with no knockback, so that seed was left. Checks:

- ssbm-rs's replay check with Show Player Names off and the controllers alone: 111,829 events each through frame 13,535, no divergence. With the names on (Slippi Dolphin's playback default, which ssbm-rs's runner uses unless `SLIPPI_PLAYER_NAMES=0`) the heap is laid out otherwise and the check leaves the recording at Nana's DI, frame 6,808 (5,786 divergences); `make-matches.sh` now runs the check with the names off. The old recording's check had passed with the names on, whose layout happened to give Nana the recording's byte.
- `melee-bench native`: in sync to its last frame with the allocator model on, every one of 13,659 frames; with it off it stops at frame 6,808 by name.
- The oracle: ssbm-rs's state trace of the replay's playback (`ssbm-run-rf --replay --fp hardware --port all`, names off, memory every tick; `$SSBM_RS_LOCAL/traces/bench-ics/ics-peach-ps.trc`, 688 MB, 410 s): `melee-oracle diff --fp hardware --heap` compares all 13,735 ticks with no difference and no stale read, the fighters' addresses the model's. `melee-oracle sync-test --rollback 7 --fp hardware`: 13,653 ticks checked, no difference.
- `melee-bench dolphin`, without the DI patch: calibrated in two runs on this machine and on the worker, every run in sync with final stocks and percents equal (below).

## Dolphin

`melee-bench dolphin [--dir DIR] [--runs N] [--calibrate] [MATCH...]` (`tools/bench/src/dolphin.rs`) plays the matches in a portable mainline Dolphin build (the folder `DOLPHIN` names), on the disc `SSBM_DISC`. `DIR` is its working folder (Dolphin's user directory, the codes and the calibrations); a match's replay is read from it when it has one, else from `data/bench`.

- **Setup.** A user directory of its own (`DIR/dolphin-user`), so the user's own Dolphin configuration is untouched: the match's codes as `GameSettings/GALE01.ini`, batch mode, the Null video backend, no audio output, unlimited speed, the CPU on the emulation thread (`CPUThread=False`, one core), JIT64, HLE audio, fast disc speed, cheats on, no memory cards, panic handlers and stop confirmation off, and no device behind any controller (the movie drives them; nothing touches the GameCube adapter). The movie's header carries the same settings, which Dolphin applies.
- **Hidden.** Dolphin runs on a desktop of its own, as ssbm-rs's `tools/gx/hidden.ps1` does, inside a job object that ends it when the harness exits, however it exits. Each run stops Dolphin and waits for it to be gone.
- **Inputs.** A DTM movie of four controllers, the replay's pre-frame controllers (raw sticks, buttons, triggers as raw/140) on the first two. A movie has one entry per controller poll. Dolphin polls twice a field, and the game's ticks keep a clock a little faster than the fields, so the poll each frame takes drifts: two polls on from the frame before's, and one about every 500 frames. Near those steps the frame's tick reads close to a poll's time, so which poll it gets depends on how much the game did before it (frame 6814 of `fox-fox-fd` takes the poll after the one its neighbours' pace gives). `--calibrate` finds each frame's poll (`DIR/NAME.dolphin`): the third controller, an unused port, counts the movie's entries on its buttons and triggers (moving sticks would cost the game work and shift those reads; buttons and triggers cost the same whatever they hold), and the harness reads which entry each frame took. HSD's queue position moves one a tick, so against the frame counter it shows the reads taken after the next tick had already taken its poll; those are dropped. The first run has the fighters stand still; each next run plays the replay's controllers placed by what the runs so far showed, and keeps what it shows up to where it leaves the replay; it stops when a run plays the whole match as the replay did, and keeps that run's placement. So far that has taken two runs per match. Calibration plays at twice the console's speed, so the game idles between frames and the harness reads each in time.
- **Timing.** The harness reads Dolphin's emulated memory (the MEM1 mapping in Dolphin's process) as each frame ends and notes the time; frames per second are counted from the match's first frame it sees to its last, so boot and loading don't count.
- **Checking.** At each frame it sees, every fighter's (Nana's too) action state, position, facing, percent and stocks and the random seed must equal the replay's. A frame read while the next tick has begun can show a mix, so a run counts as out of sync only after 120 differing frames in a row (a real desync lasts the rest of the match). Once past the match's last frame (the game shows "GAME!"), every fighter's stocks and percent must equal the replay's last frame. A run is in sync only if both hold.

Run it as `SSBM_DISC=... DOLPHIN=... melee-bench dolphin --dir "$SSBM_RS_LOCAL/bench" --calibrate` once (a few minutes per match), then `melee-bench dolphin --dir ...` for the median of 5. A Dolphin build or configuration change needs a new calibration.

**On Linux** (`tools/bench/src/linux.rs`) `DOLPHIN` names the folder of a `dolphin-emu-nogui` build, run on its headless platform (`-p headless`, no window; nogui has no batch mode) with the same settings and movie. In place of the hidden desktop and job object, the kernel kills Dolphin when the harness's thread that started it ends, however the harness ends (`prctl(PR_SET_PDEATHSIG, SIGKILL)` before Dolphin runs), and the harness reads MEM1 as Dolphin's parent: the readable shared mapping of at least 24 MB that starts with the game's ID, from `/proc/PID/maps`, read through `/proc/PID/mem` (which a parent may do where ptrace's scope is 1). Mainline's nogui front end parses `-m` but never plays the movie (only the Qt front end calls `Movie::PlayInput`), so the game got no inputs and every calibration run left the replay at frame 0; `tools/bench/dolphin-nogui-movie.patch` adds the Qt front end's call before boot and touches nothing else.

### Provisional results (2026-10-07)

**Provisional, not the baseline:** measured while eight other agents were building and testing on the machine, with every core busy (Windows reported 100% CPU), so the numbers are low and swing by a factor of two or more between runs of the same match. The official numbers wait for a quiet machine. Two runs per match, Dolphin 2609-31 (master, portable), the machine above.

| Match | Frames/s (runs) | In sync |
| --- | --- | --- |
| `fox-fox-fd` | 180, 168 (and 73 to 160 in six earlier runs) | yes, every run: about 15,550 frames compared each, final stocks and percents equal |
| `fox-marth-bf` | 152, 187 | yes |
| `fox-marth-fd` | 176, 158 | yes |
| `fox-marth-ps` | 161, 47 (24, 18 earlier, at the busiest) | yes |
| `ics-peach-ps` | 106 without the DI patch; 61, 64 with it | without the patch no (from frame 10,139, heap layout; see below); with it yes, every frame and the final stocks and percents |

**Ice Climbers–Peach (the old recording).** Without the DI patch it left the replay at frame 10,139: at frame 10,137 Nana's DI with no knockback took the low byte of her address as its stick, `0x20` in ssbm-rs's recording and `0x60` in Dolphin, whose fighters were `0x1DC0` bytes lower (the recording boot's heap against the boot codes alone; [Ice Climbers and the heap layout](#ice-climbers-and-the-heap-layout)). Up to there every fighter struct, `stage_info`, the player blocks, Nana's input ring and every random-number call were the same on both sides; Dolphin's interpreter played it as its JIT did, and neither side had a disc read in flight or more than one tick per render near those frames. The DI patch (`di.s`, at `0x800AC6D0`, Dolphin only) set that byte to the recording's on that path alone, and with it Dolphin played the match in sync. The layout code replaced it on 2026-11-19.

### Dolphin on the worker (2026-11-19)

So the speed target's ratio is taken on one machine, [the worker](#machine) (Sarah, 2026-11-19: "Yes, install it").

**The build.** The same mainline revision as the Windows figures. The harness there ran `C:\Projects\dolphin-dev\Dolphin-x64\Dolphin.exe` (22,729,728 bytes, as the harness's logs name it; from `dolphin-master-2609-31-x64.7z`), whose strings name `Dolphin 2609-31`, branch `master`, revision `a475aba718746e7656e2e6b6e047dc98a4d05879` (`git describe`: `2609-31-ga475aba718`; 2026-10-02, "Merge pull request #14857"). On the worker it is cloned from `https://github.com/dolphin-emu/dolphin.git` at that revision into `~/Projects/melee-work/dolphin/src`, with its submodules but the Windows- and Android-only ones (`Qt`, `FFmpeg-bin`, `wil`, `libadrenotools`) and the nogui movie patch above, and built into `~/Projects/melee-work/dolphin/build` (`Binaries/dolphin-emu-nogui`, which reports `2609-31-dirty` for the patch) in a shared runner job under melee-hd's build lock, in about ten minutes:

```sh
cmake -S src -B build -G Ninja -DCMAKE_BUILD_TYPE=Release -DENABLE_QT=OFF -DENABLE_NOGUI=ON -DENABLE_TESTS=OFF \
  -DENABLE_AUTOUPDATE=OFF -DENABLE_ANALYTICS=OFF -DENABLE_EVDEV=OFF -DUSE_DISCORD_PRESENCE=OFF
cmake --build build -j 3
cp -r src/Data/Sys build/Binaries/Sys   # nogui has no post-build step that puts Sys beside it
```

No packages were installed: the worker's own (GCC 15.2, CMake 4.2, Ninja, and the X11, ALSA, PulseAudio, udev, systemd, curl, zstd, lz4, xxhash, zlib, SDL3, Vulkan, EGL and GL development files already there) and Dolphin's bundled externals cover the nogui build. Frame dumping (FFmpeg), Bluetooth passthrough, libusb and evdev are left out; the runs use none of them. Two differences from the Windows build remain: the compiler (GCC against the buildbot's MSVC) and the front end (nogui's headless platform against Qt in batch mode). The emulation is the same code.

**Calibration.** `melee-bench dolphin --dir ~/Projects/melee-work/dolphin/bench --calibrate` (the codes written there by `melee-bench codes`, byte-identical to the Windows ones) in a shared job. Every match took two runs, and every calibration is byte-identical to its Windows one: the emulated timing is the same on both machines. The folder keeps the calibrations, Dolphin's user directory and the movies for later measurements. `ics-peach-ps`'s new recording (2026-11-19) was calibrated the same way, without the DI patch: two runs, byte-identical to its Windows calibration; the old recording's codes and calibration are kept in the folder's `old-ics-peach-ps/`.

**Results.** The worker, quiet, one exclusive reservation per match: `melee-bench dolphin --runs 5 MATCH` and `melee-bench native --runs 5 MATCH`, the order alternating from one match to the next; median frames/s and the runs. melee-hd is `perf/worker-dolphin` `8d87876e`, whose simulation is main `160ae506`'s (its code equals `4770a3ca`'s), release with line tables as [above](#baselines-on-the-worker-2026-11-19), the allocator model on, the state-only display pass. Every Dolphin run was in sync with its final stocks and percents equal, and so was every melee-hd run but the old `ics-peach-ps`'s. The processor was 0% busy before and after each half.

| Match | Dolphin, median frames/s (runs) | melee-hd native (runs) | Ratio |
| --- | --- | --- | --- |
| `fox-marth-bf` | 267 (270, 267, 267, 270, 267) | 2,583 (2,580, 2,583, 2,570, 2,589, 2,594) | 9.7× |
| `fox-marth-fd` | 278 (278, 277, 280, 278, 278) | 2,471 (2,471, 2,466, 2,453, 2,477, 2,473) | 8.9× |
| `fox-marth-ps` | 232 (232, 231, 232, 230, 232) | 2,220 (2,212, 2,223, 2,212, 2,221, 2,220) | 9.6× |
| `ics-peach-ps`, new recording (below) | 228 (228, 228, 227, 230, 228) | 1,718 (1,725, 1,704, 1,711, 1,718, 1,720) | 7.5× |
| `ics-peach-ps`, old recording (Dolphin with the DI patch) | 229 (229, 229, 229, 231, 229) | no result: out of sync from frame 10,139 (the match ends at frame 10,986) | none |
| `fox-fox-fd` | 285 (281, 287, 283, 285, 285) | 2,699 (2,699, 2,699, 2,696, 2,678, 2,699) | 9.5× |
| `ganon-captain-fd` | 270 (270, 272, 270, 268, 270) | 2,479 (2,479, 2,478, 2,465, 2,491, 2,487) | 9.2× |

```sh
export DOLPHIN=~/Projects/melee-work/dolphin/build/Binaries SSBM_DISC=...
melee-bench dolphin --dir ~/Projects/melee-work/dolphin/bench --runs 5 MATCH
melee-bench native --runs 5 MATCH
```

**The new `ics-peach-ps` (2026-11-19).** Recorded in playback's heap layout ([Ice Climbers and the heap layout](#ice-climbers-and-the-heap-layout)), so Dolphin runs it without the DI patch and melee-hd plays it to its end. One exclusive reservation, Dolphin first, then `melee-bench native`, from `perf/bench-ics` (`5e685e86`, main `160ae506` with batch 6, and the layout code; release with line tables); every run in sync, Dolphin's final stocks and percents equal; the processor 0% busy before and after each half. The same build in a second exclusive reservation ran `fox-marth-ps` at 2,164 frames/s (2,153 to 2,168), 2.5% under the table's `8d87876e`, and `ics-peach-ps` at 1,682 (1,674 to 1,686, 7.4×), so on main `160ae506`'s simulation it would be about 7.6 to 7.7×. Dolphin plays the new match about as fast as it did the old (228 against 229 frames/s).

**Against the target: 7.5 to 9.7×, under half of 20×.** On the target's matches melee-hd needs 2.1 to 2.3 times main's speed for 20× (`fox-marth-fd` 5,560 frames/s, `fox-marth-ps` 4,640, `fox-marth-bf` 5,340) and 3.1 to 3.4 times for 30×; Ice Climbers–Peach, the slowest, needs 2.65 times (4,560 frames/s) for 20× and 4.0 times (6,840) for 30×, its three fighters and Nana's computer player costing melee-hd about a quarter more a frame than Fox–Marth on the same stage while Dolphin's frame costs about the same. The worker's ratios are lower than this machine's (12.7 to 14.0× on 2026-11-01): the worker runs Dolphin at 69 to 74% of this machine's speed (`fox-fox-fd` 285 against 388 frames/s) but melee-hd at about half, so a ratio taken on one machine doesn't carry to the other. Each side's runs spread by under 2.5%.

## Rendering speed

The player's rendering is measured in the player itself, in its window, presenting as it always does: headless runs (`melee-hd --bench`) wait for the GPU every frame and present to nothing, so they never showed the player's own GPU behavior. The target is large cuts in the GPU's time a frame on every stage, the heavy ones above all (Fountain of Dreams and Great Bay drop below 144 frames a second at 5x where Final Destination holds it), with headroom for weaker GPUs than the reference machine's.

**GPU time by stage.** Where the device has timestamp queries (`Features::TIMESTAMP_QUERY`, which the player asks for when the adapter has it), every render pass writes a timestamp at its start and end (`melee_gpu::gpu_time`): the renderer's EFB passes (the scene, one a flush, with the clears their load operations make), its copies from the EFB to textures, the copy to the XFB (encode and decode), the blit to the window and the F3 overlay. A presented frame's queries are resolved after its present and read back when the GPU gets there, a few frames late (a ring of 8 frames; a frame that finds none free isn't timed), so nothing waits. A frame's time is its passes' time summed by stage; its span, from the first pass's start to the last's end, adds the gaps where the GPU waited on the CPU. Readbacks (copies the CPU encodes, `xfb_readback`) aren't render passes and aren't timed; the match's frames make none. Without timestamp queries there's no timer and nothing changes; `MELEE_GPU_TIMING=0` turns it off to compare. The F3 overlay shows the last second's means: GPU milliseconds a frame with the stages largest first, and the CPU's a frame (building the frame, its GX stream and the renderer's submissions; waiting for the swap chain; the blit, overlay and present call).

**The benchmark.** `melee-hd --perf-report FILE` measures a run as it plays and, when the player quits, writes and prints frames a second, the time between presents (mean, median, 95th and 99th percentiles, worst), each GPU stage's time a frame (mean and percentiles, with passes a frame), the GPU's span, and the CPU's time a frame and a tick, from tick 180 on (the first 3 s load pipelines and textures). With `--ticks N` and a replay, or a match with a fixed `--seed` and no one at the controllers, every run is the same. `--refresh HZ` draws Interpolate's frames for a display of that rate instead of the window's monitor's: with `--present immediate` and a high rate the frames come as fast as the player can make them, so a speedup shows even where 144 is already reached (the GPU's times don't depend on it). `melee-bench player` runs it on a desktop of its own (nobody sees it; frames still go through the compositor at the display's rate, as a fixed-refresh display) over a matrix, one run at a time, each in a job object that ends it with the harness, and prints a table:

```sh
SSBM_DISC=... target/release/melee-bench player [--exe target/release/melee-hd.exe] [--out DIR] \
    [--matches fd,fod,gb,ps|FILE.slp,...] [--scales 1,3,5] [--sizes 1920x1080,fullscreen] [--modes interpolate,off] \
    [--present mailbox] [--refresh HZ] [--ticks 1980] [--runs 1] [-- PLAYER ARGS...]
```

A match is a stage's name, Fox against Marth on it with idle controllers (the fighters stand at their spawns; the stage plays out as it always does) and a fixed seed, or a replay, such as the benchmark matches in `data/bench`, for real play. melee-sim hasn't ported the VS CPUs' decisions yet (`ftCo_800B2AFC` for kind 4), so a CPU match isn't possible. Every run is widescreen (16:9), with `--no-adapter --mute`; a size is a window's, or `fullscreen` (borderless, the monitor's size). The defaults run each of Final Destination, Fountain of Dreams, Great Bay and Pokémon Stadium at 1x, 3x and 5x, in a 1920 by 1080 window and fullscreen, with Interpolate and Off, 1,980 ticks each (30 s measured): 48 runs, about half an hour. The table's columns are frames a second, frame time percentiles, the GPU's time a frame (mean and 95th percentile) and its scene, EFB copy, XFB copy and present stages, and the CPU's build time; each run's whole report is kept in `--out`.

Take it on a quiet machine: other agents' builds move the CPU's times and with them when frames are drawn.

### Baseline (2026-10-19)

melee-hd `928ca81` (main), release; AMD Ryzen 7 9800X3D, NVIDIA GeForce RTX 5070 Ti (driver 610.88, Vulkan), Windows 11 Pro, the display Sarah's 4K 144 Hz one; the machine quiet (no builds or other agents, CPU about 4% busy before the runs; OBS, a browser and a torrent client open but idle, melee-unlocked not running). `melee-bench player`, widescreen, 1,980 ticks a run (30 s measured), on a replay per stage, real play: `fox-marth-fd` and `fox-marth-ps` (`data/bench`), a Slippi ranked replay on Fountain of Dreams (`Game_20260129T214216`), and milestone 4's Great Bay oracle set (`stage-great-bay`, [the oracle](oracle.md#oracle-sets)). The last two aren't in the repository; any replay on those stages measures the same way. "Uncapped" is Interpolate with `--refresh 1000 --present immediate`: frames as fast as the player makes them. Fullscreen is 3840 by 2160 (5x: EFB 4266 by 2640). GPU and CPU figures are milliseconds a frame, means (95th percentile); the GPU's total is its passes' time, its stages summed. Every frame was timed.

**5x, fullscreen, uncapped** (two runs each, the mean; the runs agreed within 3%):

| Stage (replay) | Frames/s | Frame time p50 / p95 / p99 | GPU total | Scene | EFB copies | XFB copy | Present | CPU build | Bound by |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Final Destination | 145.7 | 6.86 / 8.33 / 9.43 | 6.73 (8.15) | 6.22 | 0.09 | 0.34 | 0.08 | 3.76 (4.28) | GPU |
| Great Bay | 143.8 | 6.76 / 8.90 / 9.56 | 6.81 (8.76) | 6.26 | 0.09 | 0.38 | 0.09 | 4.58 (5.06) | GPU |
| Fountain of Dreams | 110.6 | 9.00 / 10.77 / 11.96 | 8.14 (10.39) | 7.61 | 0.10 | 0.36 | 0.08 | 7.99 (8.89) | CPU and GPU alike |
| Pokémon Stadium | 100.6 | 9.53 / 13.64 / 15.06 | 9.77 (13.49) | 9.21 | 0.15 | 0.33 | 0.09 | 4.45 (4.96) | GPU |

The GPU's span (first pass's start to last's end) was within 0.15 ms of its total on the GPU-bound stages, and 0.8 ms more on Fountain of Dreams, where the GPU waits for the CPU's submissions. Waiting for the swap chain took 1.7 to 4.5 ms a frame on the GPU-bound stages and 0.3 ms on Fountain of Dreams.

**Scaling, fullscreen, uncapped** (one run each at 1x and 3x):

| Stage | Frames/s 1x / 3x / 5x | GPU total 1x / 3x / 5x | Scene 1x / 3x / 5x | CPU build (any scale) |
| --- | --- | --- | --- | --- |
| Final Destination | 244 / 243 / 146 | 0.62 / 3.33 / 6.73 | 0.45 / 3.06 / 6.22 | 3.7 |
| Great Bay | 202 / 201 / 144 | 0.57 / 3.20 / 6.81 | 0.40 / 2.90 / 6.26 | 4.5 |
| Fountain of Dreams | 118 / 116 / 111 | 0.60 / 3.17 / 8.14 | 0.43 / 2.93 / 7.61 | 7.9 |
| Pokémon Stadium | 211 / 200 / 101 | 0.66 / 4.26 / 9.77 | 0.53 / 4.00 / 9.21 | 4.3 |

At 1x and 3x every stage is bound by the CPU's frame building (with the present 1x's blit to 4K, 0.13 ms, is the GPU's biggest stage after the scene). 25 times the pixels (1x to 5x) made the scene 14 to 18 times slower.

**5x in a 1920 by 1080 window, uncapped**: Final Destination 144.7 frames/s (GPU 6.79), Great Bay 150.1 (6.52), Fountain of Dreams 110.5 (8.21), Pokémon Stadium 110.0 (8.92): the same as fullscreen but for the present (the blit to 1080p, 0.01 ms).

**5x fullscreen at the display's 144 Hz (mailbox), Interpolate and Off:**

| Stage | Interpolate: frames/s, GPU total | Off: frames/s, GPU total |
| --- | --- | --- |
| Final Destination | 128.0, 7.55 (9.46) | 60.0, 8.06 (9.11) |
| Great Bay | 134.2, 7.21 (8.50) | 60.0, 9.03 (10.46) |
| Fountain of Dreams | 109.9, 8.18 (10.29) | 60.0, 8.50 (10.84) |
| Pokémon Stadium | 103.6, 9.44 (13.66) | 60.0, 10.80 (14.24) |

A frame's GPU time rises when the GPU has less to do (up to 30% at Off's 60 frames a second), its clocks lower, so GPU times compare only within one mode; the uncapped runs are the reference. Paced to the hidden desktop's 144 Hz compositor, Interpolate fell short of 144 on Final Destination and Great Bay, which reach it uncapped: at 6.8 ms a frame against the 6.94 ms refresh, frames miss refreshes. (Sarah sees 144 on Final Destination on her own display.)

**Idle fighters, 5x fullscreen, uncapped** (`--matches fd,fod,gb,ps`): Final Destination 153.6 frames/s (GPU 6.37), Great Bay 136.6 (7.17), Fountain of Dreams 109.3 (8.21, CPU build 8.40), Pokémon Stadium 119.1 (8.19). Real play costs Pokémon Stadium 1.6 ms of GPU (its transformations), the others little.

**The timer's cost**: Fountain of Dreams at 5x uncapped with `MELEE_GPU_TIMING=0` ran 108.2 and 111.4 frames/s, against 111.0 and 110.2 timed: none measurable, so it stays on.

**Fountain of Dreams' frame building** (`GX_PROFILE=1 melee-hd --bench 1500 --replay ... --scale 1 --one-thread`, the renderer on the game's thread to split it): a frame has 582 draw calls, 57,600 vertices, 200,000 indices and 1.17 million words of vertex data (4.7 MB), and the renderer takes 10.2 ms, 4.1 of it transforming vertices; Final Destination's has 218 draw calls, 27,100 vertices, 41,500 indices and 219,000 words (0.9 MB), 4.1 ms in the renderer, 1.8 transforming. The player's threads overlap those stages, to 8.0 and 3.7 ms.

### Where the time goes at 5x

1. **The scene passes: 92 to 94% of the GPU's time on every stage** (6.2 to 9.2 ms of 6.7 to 9.8). They grow with the pixels (14 to 18 times from 1x to 5x) while their per-frame part is small (0.4 to 0.5 ms at 1x), so at 5x the cost is per pixel: the TEV shader run for every pixel of every draw. That is what early depth testing (the shader writes `frag_depth` on every draw, so the hardware shades overdrawn pixels in full) and specialized shaders (the uber shader's register-driven loops and branches) both cut, early-Z by shading fewer pixels and specialized shaders by making each cheaper. Pokémon Stadium (9.8 ms, p95 13.5) and Great Bay showed them most. Specialized shaders cut the scene 9 to 11 times (below): it's 0.6 to 1.0 ms now, and the CPU's frame building is the limit on every stage.
2. **The CPU's frame building: on Fountain of Dreams as long as the GPU's frame** (8.0 ms against 8.1), and the limit at every scale there (118 frames/s even at 1x). Its geometry is twice Final Destination's in vertices and five times in vertex data, and the renderer transforms every vertex on the CPU, as Dolphin's software renderer does, and uploads them each frame. GPU work alone won't lift Fountain of Dreams past about 120 frames/s: it also needs the frame building cut (transforming on the GPU, keeping static geometry's transformed vertices between frames, or more threads), and so does every stage below 5x (3.7 to 4.5 ms elsewhere, the 1x and 3x limit).
3. **The copy to the XFB: 0.33 to 0.38 ms** (4 to 5%), two full-frame passes at the EFB's size; folding it into the present's blit would save most of it, worth doing after the above.
4. **EFB copies to textures: 0.09 to 0.15 ms**, and **the present: 0.08 ms** at 4K: not worth work now.
5. **Uploads**: the GPU's per-frame fixed cost is small (the whole 1x frame is 0.6 ms), so per-draw uniform uploads matter on the CPU's side (the flush is 0.3 to 0.6 ms of the renderer's time on one thread) more than the GPU's; they belong with item 2.

### CPU frame building (2026-10-19)

The work on the CPU's frame building after the baseline, every frame exact. Profiled with samply (2 kHz) on Fountain of Dreams at 1x (`melee-hd --bench 900 --replay ... --scale 1`, the player's threads), symbols from a line-table build (`CARGO_PROFILE_RELEASE_DEBUG=line-tables-only`) read with dbghelp; other agents' builds kept the machine fully busy, so the shares are what to read, not the times.

**Before**, by thread (share of its samples):

- Game thread: building the frame's stream (HSD's display code to GX) about 0.6 to 0.8 ms, then running it: fetching vertices from their arrays (about 140,000 attributes a frame) 25%, the threaded sink 11% (state deltas, texture memory hashes, a copy of each draw's vertices), the registers' hashes on every write, testing all 86 rewritten display lists on every view of memory; waiting for the render thread 12%.
- The one transform thread, 88% busy: transforming 48% (lighting most of it: two spot lights in color channel 0 and a specular light in channel 1 on most vertices), decoding 22% (half of it copying each 132-byte decoded vertex), working out vertex layouts it never used 8%, a new output list per draw 7%.
- Render thread, 71% busy: its per-draw work 49% (whether the alpha test can fail, tested over all 256 alphas every draw, 14%; the GPU's words for every vertex, points as six vertices each), wgpu's encoding, buffer writes and submits 15%, freeing what the threads sent 4%.

On one thread (`GX_PROFILE=1 ... --one-thread`) a frame took 10.95 ms in the renderer: 7.95 ms of draws (4.6 of it transforming), 0.57 flushing, 0.11 reading back, about 2.3 running the stream.

**Changes** (melee-gx, melee-gpu, melee-render; the threaded design is in [the player](player.md#frame-time)):

- Transform threads (a quarter of the processor's threads, one to four) share each batch's draws out, each fetching, decoding, transforming and building its draws into the GPU's words and triangles; the render thread only appends them. The game thread no longer fetches vertices from arrays the host holds as they are: it sends each array once, as far as draws have reached it.
- Points and lines write four vertices a quad instead of six (a frame's vertex data 1.17 to 0.88 million words); uniforms are shared across a flush; the alpha test's question is answered once per register value; the render thread flushes every 160 draw calls so wgpu's encoding overlaps the stream.
- The game thread keeps the registers' hashes only when digests are asked for, looks for rewritten display lists only in archives that have them, keeps a batch's state changes and vertices in one buffer each, and caches the frame's few vertex formats.

**Exactness.** `GX_DIGEST=FILE` digests every frame's drawn vertices, their state, clears and copies. Before and after (main with only the digest, against this work) write identical lines for 400 ticks of each of the five benchmark replays used and the Fountain of Dreams replay, in Off, Interpolate (3 frames a tick) and Extrapolate (2 a tick), threaded and on one thread: 2,400 frames per replay. The frames and mix checks pass unchanged.

**Results, provisional** (the machine fully busy with other agents' work, alternating runs): Fountain of Dreams at 1x, headless, the renderer's time a frame 9.1 to 9.2 ms before against 4.5 to 4.6 after (about 2x); with one, two, three, four and six transform threads 9.6, 6.1, 5.0, 4.3 and 4.2 ms. The player's benchmark on a quiet machine is still to be taken.

### Specialized shaders (2026-10-19)

melee-hd `fe015fb` (perf/tev-specialize: a shader specialized to each draw's TEV configuration, [player](player.md#rendering)), the same machine, display and driver as the baseline, quiet (no builds or other agents; the CPU 8 to 20% busy before the runs with OBS, a browser and a torrent client open but idle). `melee-bench player`, 5x fullscreen (3840 by 2160, EFB 4266 by 2640), uncapped, 1,980 ticks a run, the baseline's four replays; the uber shader alone (`MELEE_TEV=0`) and specialized shaders run back to back, alternating, twice each, after one warm-up run of each replay that saved the specialized pipelines' list. The table is the two runs' mean; each pair agreed within 4% (Pokémon Stadium's uber runs within 8%). GPU figures are milliseconds a frame, means (95th percentile).

| Stage (replay) | Frames/s, uber / specialized | GPU total, uber / specialized | Scene, uber / specialized | Scene, times faster | CPU build |
| --- | --- | --- | --- | --- | --- |
| Final Destination | 135.3 / 243.3 | 7.19 (11.41) / 1.19 (1.52) | 6.62 / 0.68 | 9.8 | 3.7 |
| Great Bay | 150.4 / 197.6 | 6.35 (10.41) / 1.14 (1.40) | 5.72 / 0.60 | 9.5 | 4.6 |
| Fountain of Dreams | 109.0 / 114.4 | 7.87 (12.19) / 1.16 (1.59) | 7.33 / 0.67 | 11.0 | 8.0 |
| Pokémon Stadium | 101.8 / 206.2 | 9.56 (15.07) / 1.53 (2.11) | 8.92 / 0.96 | 9.3 | 4.4 |

The GPU's whole frame is 1.1 to 1.5 ms at 5x now, of which the scene is 53 to 63% and the copy to the XFB (0.32 to 0.36 ms) most of the rest. Every stage is bound by the CPU's frame building (3.7 to 8.0 ms, unchanged: working out each draw's uid costs nothing measurable), which the frame rates follow; frame time p99 fell from 12.1 to 18.3 ms to 5.3 to 10.7 ms. The uber shader's own runs here came within 7% of the baseline's GPU time either way, with early depth testing on since: run to run, the GPU's clocks move that much.

The specialized pipelines: 96 to 248 TEV configurations a run (those it drew with and those the saved list warmed; over a whole benchmark match at 1x, 114 to 177 configurations and 116 to 206 pipelines), made in 4.1 to 4.5 ms each on average and 10 ms at most, on the shader threads, from the driver's pipeline cache; made mostly cold (the first runs after gx.wgsl changed, in `melee-render-check` with `MELEE_TEV=sync`, one at a time), 35 to 370 ms on average a match and up to 1.8 s. The uber shader drew 0.03 to 0.13% of a measured run's draw calls (0.2 to 0.3% of the 600-tick warm-ups'), those before their pipelines were made, at the start.

### Results after the week's work (2026-10-19)

Main at `d154b3c` (the CPU frame building above, early depth testing, the specialized shaders) against the baseline build (`9362975` with only the frame digest, which costs nothing unless asked for), on the quiet machine of the baseline: `melee-bench player`, the baseline's four replays, fullscreen (3840 by 2160), Interpolate uncapped (`--refresh 1000 --present immediate`), 1,980 ticks a run, the two builds alternating, two rounds; the mean of the two. GPU and CPU figures are milliseconds a frame, means.

| Stage, scale | Frames/s | GPU total | CPU build | Frame time p99 |
| --- | --- | --- | --- | --- |
| Final Destination, 1x | 215 → 397 | 0.53 → 0.17 | 4.08 → 2.12 | 10.3 → 3.64 |
| Final Destination, 5x | 141 → 404 | 6.96 → 1.17 | 3.95 → 2.11 | 10.27 → 3.50 |
| Great Bay, 1x | 196 → 327 | 0.53 → 0.18 | 4.55 → 2.58 | 6.59 → 4.31 |
| Great Bay, 5x | 149 → 332 | 6.58 → 1.15 | 4.62 → 2.58 | 9.50 → 4.32 |
| Fountain of Dreams, 1x | 111 → 202 | 0.54 → 0.23 | 8.15 → 4.42 | 12.3 → 6.44 |
| Fountain of Dreams, 5x | 109 → 196 | 7.95 → 1.13 | 8.17 → 4.57 | 11.98 → 6.76 |
| Pokémon Stadium, 1x | 209 → 339 | 0.63 → 0.19 | 4.29 → 2.51 | 6.09 → 4.07 |
| Pokémon Stadium, 5x | 113 → 343 | 8.70 → 1.49 | 4.42 → 2.50 | 14.57 → 4.12 |

(The baseline's first Final Destination 1x run had a slow stretch, p99 15.1 against 5.5 in the second.) The CPU's frame building is 1.7 to 1.9 times faster on every stage and the GPU's frame 5.7 to 7 times at 5x, so every stage is now bound by the CPU, at 196 to 404 frames a second at either scale. At the display's own 144 Hz (mailbox, Interpolate, 5x, main only, one run each) every replay holds 144.0 frames a second: frame time p99 7.97 (Final Destination), 8.13 (Great Bay), 8.82 (Fountain of Dreams) and 8.18 ms (Pokémon Stadium), GPU 1.07 to 1.44 ms, CPU build 2.33 to 4.77 ms.

**Profile after** (samply at 2 kHz on main's line-table build, the player in its window on the hidden desktop, fullscreen 5x, Interpolate uncapped; Fountain of Dreams 189 frames/s and Final Destination 380 while profiled). Busy time a frame by thread, from each thread's samples outside waits:

- Fountain of Dreams (5.3 ms a frame): the game thread about 3.1 ms (building the frame's stream 20% of its samples, with the blend's matrix work; running the stream 34%: the vertex arrays' reach and views, state deltas, the sink; presenting 7%; waiting at the frame's end for the render thread 26%), the render thread about 3.2 ms (its per-draw work 42%: specialized pipeline lookups, uniforms, textures; flushing to wgpu 23%: encoding 11%, submits 7%, buffer writes 7%), and each of the four transform threads about 2.9 ms (transforming 25%, lighting most of it; fetching 11%; decoding 10%; zeroing and copying each vertex's output and GPU words 14%).
- Final Destination (2.6 ms a frame): the game thread about 1.7 ms (building the stream 19%, running it 23%, presenting 11%, waiting for the render thread 38%), the render thread about 1.4 ms (per-draw work 27%, flushing 19%), the transform threads about 1.2 ms each, half their time idle.

The critical path is the game thread's frame building and stream, then the render thread finishing the last batch and flushing it; on Fountain of Dreams the transform threads are nearly as busy. The next cuts, by expected payoff: the transform threads' copies (write each vertex's words straight from its output, without the intermediate GPU vertex, and zero only what a vertex's transform leaves unwritten); the render thread's per-draw work and wgpu calls (uniforms indexed per draw call instead of a dynamic offset per draw, fewer bind group changes, caching the pipeline lookup per state); the game thread's stream building (HSD's display code, the blend's decompositions) and running (the arrays' reach scanned once per display list rather than every draw); then lighting across vertices with SIMD, the same IEEE operations in four lanes.
