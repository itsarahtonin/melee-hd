# Performance

Measured with `tools/bench` (`melee-bench`), release builds, other applications closed, median of 5 runs. Every number names the machine it came from.

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

## Machine

AMD Ryzen 7 9800X3D (8 cores), 62 GB RAM, Windows 11 Pro, Rust 1.98.1.

## Snapshots (2026-10-07)

`melee-bench snapshot CALL...` imports a real mid-match state from ssbm-rs's saved calls and measures a rollback snapshot of it: the `Game` value plus everything it owns on the heap, a snapshot (`clone`) and a restore (`clone_from`), and their allocations. The state was what melee-sim modeled then (scheduler, HSD objects, fighters, stage, camera, collision), before items and effects were ported; the whole-match figures below include them.

| Capture (stage) | GObjs | Joints | Snapshot | Allocations | Clone | Restore | Restore allocations |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Final Destination | 72 | 731 | 0.64 MB | 1,895 | 138.5 µs | 110.0 µs | 1,895 |
| Pokémon Stadium | 77 | 651 | 0.58 MB | 1,501 | 125.0 µs | 87.9 µs | 1,501 |
| Battlefield | 65 | 780 | 0.64 MB | 1,865 | 129.0 µs | 134.0 µs | 1,865 |

`melee-bench snapshot-match [--dir DIR] [--every TICKS] [MATCH]` measures a real match's states the same way, as the match plays natively (as `melee-bench native` plays it, `fox-fox-fd` by default) as far as it runs: every `TICKS` ticks (default 1000) the state is kept, and afterwards each is measured, together with a restore followed by 8 ticks (rollback's window) and the allocations each of those ticks makes, which the "no allocations" target counts.

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

Pokémon Stadium allocated in every tick, straight or replayed: its screens' text (`gr::stadium::text_names`, `hsd::sis::printf`) and the TEV compiles its display changes make (`hsd::mobj::compile_tev` from `gr::stadium::apply_display`), about 18 allocations an engine tick and 59 a display pass between ticks 5,000 and 5,020 (`melee-bench alloc-sites`). Since 2026-10-16 its engine ticks allocate only when a transformation loads (the material marks' map, `gr::setup::mark_materials`, and new shapes' envelope lists when the spares run out), and a restore allocates when the text system's maps changed (they're copied whole); its display passes still lay their text out into new vectors (`hsd::sis::layout_text`), about 7 a pass besides the text draws' boxes. That is open against the "no allocations" target. The sync test ([the oracle](oracle.md#sync-test)) times the same operations as it runs, the ring's snapshot taken with `clone_from` into kept storage: on the Fox ditto with rollbacks of 1, a snapshot about 60 µs and a restore about 55 µs (median); with rollbacks of 7, run two at a time beside other agents' builds, a restore and 7 ticks 2.0 to 3.0 ms (median) on the benchmark matches.

Size, clone and restore are well inside their targets. A restore reuses the target's storage: the state's types derive `melee_derive::CloneFrom`, a `Clone` whose `clone_from` copies field by field (each field's own `clone_from`, so vectors, boxes and pools keep their capacity), and the few containers that would build anew (maps, file lists of strings) are copied only when they differ. A snapshot (`clone`) still allocates, since it makes a new copy. Restore and 8 ticks was at the 4 ms target then, over it on some states: the ticks are the cost (see the profile below). After the speed work below it is 2 to 3 ms on the whole match.

**Allocations.** An engine tick allocated about 1,550 times on this match (2026-10-08), almost all in the display pass and joint walks that collected vectors to walk over. Now walks read each next link as they go (none of their bodies changes the links they walk), envelopes are lent out instead of copied, small fixed-size lists are held inline, symbol names and voice ids are read in place, a freed shape's envelope lists are kept for the next shapes to reuse (spare storage a restore keeps and a snapshot leaves out), and `Match::recycle` takes a frame's event and draw buffers back. A material's compiled TEV is written over the storage its last compile, or a freed material's, held (`hsd::mobj::compile_tev`, in the room the compiles keep; a restore moves the compiled TEVs it would drop to the spares), and a reused texture slot keeps its palette list's storage. A frame's only allocation left is the text draw's box (`Draw::SisText`), the presentation's output. An engine tick still allocates the first time something grows past what it has held (a pool's first new slot, a fighter's first handle word): `melee-bench`'s check `ticks_and_restores_dont_allocate` plays each benchmark match but Pokémon Stadium's to tick 4,600, restores its state from tick 1,000 and plays ticks 1,000 to 4,600 again, as a rollback does, and asserts that the restore and every one of those ticks make no allocation. The same stretch replayed from one Slippi replay of each ported character (2026-10-16, `alloc-sites --dir DIR --rollback --fp slippi`, ticks 1,000 to 4,000) makes no allocation off Pokémon Stadium either. `melee-bench alloc-sites [--from TICK] [--ticks N] [--rollback] [--fp slippi] [MATCH]` tallies where a stretch of ticks allocates, by backtrace (with a profiling build: line tables on, `CARGO_PROFILE_RELEASE_DEBUG=line-tables-only`).

## Simulation speed

`melee-bench native [--dir DIR] [--runs N] [MATCH...]` (`tools/bench/src/native.rs`) plays each benchmark match (`DIR` defaults to `data/bench`) natively and headless on one thread: from the replay's start through `vs_match`'s replay path (Slippi's playback codes, `Match::step_replay`), with the display pass, in the console's floating point (`FpMode::Hardware`, as the matches were made). The match's files are read from the disc before the clock starts, and the fighters' addresses come from the match's start trace (`NAME.start.trace`, in `DIR` or `$SSBM_RS_LOCAL/bench`), as the oracle takes them. A run is timed from the match's first tick to its last frame, and afterwards every frame's fighters (action state, position, facing, percent and stocks), sampled before the frame's display pass as Slippi's post-frame updates record them, are checked against the replay's, the fields the Dolphin harness checks. (Sampled after it, a fighter KO'd toward the screen, which the pass places, reads as out of sync: the Battlefield and Ganondorf–Captain Falcon matches did.) It prints `melee-bench dolphin`'s table (median of 5 runs, with the machine), so the two divide. A run that stops before the match's last frame is no result: its row says where it stopped and why, and gives no speed.

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

## Benchmark matches

| Match | Stage | Fighters (costume) | Seed | Frames | Length | Ends |
| --- | --- | --- | --- | --- | --- | --- |
| `fox-fox-fd` | Final Destination | Fox (0) vs Fox (1) | `1F0F0001` | 15,575 | 4:20 | port 2 out of stocks, port 1 on 1 |
| `fox-marth-bf` | Battlefield | Fox vs Marth | `1F0F0002` | 15,110 | 4:12 | Marth out of stocks, Fox on 3 |
| `fox-marth-fd` | Final Destination | Fox vs Marth | `1F0F0003` | 15,241 | 4:14 | Fox out of stocks, Marth on 1 |
| `fox-marth-ps` | Pokémon Stadium | Fox vs Marth | `1F0F0004` | 13,585 | 3:46 | Fox out of stocks, Marth on 1 |
| `ics-peach-ps` | Pokémon Stadium | Ice Climbers vs Peach | `1F0F0005` | 12,655 | 3:31 | Ice Climbers out of stocks, Peach on 1 |
| `ganon-captain-fd` | Final Destination | Ganondorf vs Captain Falcon | `1F0F0006` | 12,067 | 3:21 | Ganondorf out of stocks, Captain Falcon on 1 |

Each is a VS match (4 stocks, 8 minutes, items off, pausing off; settings as a Slippi Dolphin VS match records them) with both players human, and the controllers both played. The Fox–Fox match was the first measured, when only Fox was ported. Frames count from the first (`-123`) to the last. The replays are in the repository, `data/bench/NAME.slp` (inputs the CPU AI played, no game data); `make-matches.sh` writes them with the Gecko codes (`NAME.ini`, below), ssbm-rs's start traces (`NAME.start.trace`) and the logs of making and checking them into its folder (ssbm-rs's `local/bench`, `$SSBM_RS_LOCAL/bench`, where `melee-bench native` finds the start traces).

**How they're made.** None of the author's replays is a Fox–Marth or Ice Climbers–Peach singles match, and a CPU match can't simply be replayed as a human one: a CPU's stick reads v/127 where a controller's reads r/80 after Melee's clamp, so its inputs mostly aren't a controller's, and the CPU AI draws from the game's random seed, so without it every later random number moves. So the game's level-9 CPU AI plays both fighters *through their controllers*: in ssbm-rs, around `Fighter_procCpu` each human player counts as a CPU, the AI decides, the random seed is put back as it was, and what the AI asked for (sticks scaled to a controller's range, triggers to its) becomes that controller's next poll. The fighters do only what the controllers say, and the match is the CPUs' closed-loop play. Before "GO" the controllers stay centred.

The match starts the way Slippi's playback starts a replay: Gecko codes (`melee-bench codes DIR`, from `tools/bench/gecko/*.s` and Slippi's own boot, Init Player Data, Init Stage Data and ExtendPlayerBlock codes) boot into DebugMelee's result screen, which loads the match's files and moves on to the match, and StartMelee takes the match's settings and seed where Slippi's playback restores a replay's game info. No EXI device is involved. Slippi's recording codes (loaded by its bootloader in ssbm-rs) record the match as a replay. Slippi's playback set also has "ScenePrep_DebugResult = ScenePrep_DebugMenu" (`041B16A8 801B09C0`), which would break the result screen; Dolphin's INI reader drops it (its comment has an `=`), so playback never applies it and neither do these codes.

The floating point is the console's (ssbm-rs's `Hardware` mode, which mainline Dolphin reproduces), not Slippi's Ishiiruka Dolphin's, so these replays are for this harness, ssbm-rs and melee-hd; Slippi Dolphin may play them differently.

**Checks.** Each match is played again two ways, and both must find no difference from the recording in any compared field of any frame:

- ssbm-rs's replay check: the match played back through Slippi's playback codes (a different boot path, the recorded inputs injected per frame), with `--fp hardware` (`NAME.check.log`).
- The recorded controllers alone (`BENCH_PADS`), without the AI (`NAME.pads.log`). This is what Dolphin and melee-hd get.

All six pass both: 95,826, 91,239, 92,122, 82,093, 101,048 and 72,403 events compared, 0 divergences. `ganon-captain-fd` is there for the oracle (no replay paired Ganondorf with ported characters on a ported stage when it was made), not for the speed target.

**Regenerating them.** Build ssbm-rs's runner from its main branch (`3f9ac3a` and later; `cargo build --locked --release -p ssbm-run`; its `tools/run/src/bench.rs` has `BENCH_CODES`, `BENCH_AI`, `BENCH_PADS` and `BENCH_SLP`), then:

```sh
SSBM_RUN=.../ssbm-run.exe SSBM_DISC=... JOBS=3 bash tools/bench/make-matches.sh "$SSBM_RS_LOCAL/bench"
```

For each match that writes the codes, records it (`BENCH_CODES=NAME.ini BENCH_AI=0,1 BENCH_SLP=NAME.slp ssbm-run --fields 31000`), runs both checks, and saves ssbm-rs's state trace of the match's first frames (`NAME.start.trace`, with `--port sysdolphin/baselib/perf`) for the fighters' heap addresses. It is deterministic: rerunning it after ssbm-rs's runtime fix ab3cd01 made byte-identical replays. Each run takes ssbm-rs 10 to 15 minutes on one core.

**Other matches.** The same generator (`tools/bench/src/matches.rs`) also sets a match's rules (stock or time, with a time limit), its item frequency and switch, two to four players and computer players of any CPU kind and level, on any VS stage. Milestone 4's oracle sets are made with it (`melee-bench oracle-sets`, `make-oracle.sh`; see [the oracle](oracle.md#oracle-sets)); the benchmark matches' codes are unchanged.

## Dolphin

`melee-bench dolphin [--dir DIR] [--runs N] [--calibrate] [MATCH...]` (`tools/bench/src/dolphin.rs`) plays the matches in a portable mainline Dolphin build (the folder `DOLPHIN` names), on the disc `SSBM_DISC`. `DIR` is its working folder (Dolphin's user directory, the codes and the calibrations); a match's replay is read from it when it has one, else from `data/bench`.

- **Setup.** A user directory of its own (`DIR/dolphin-user`), so the user's own Dolphin configuration is untouched: the match's codes as `GameSettings/GALE01.ini`, batch mode, the Null video backend, no audio output, unlimited speed, the CPU on the emulation thread (`CPUThread=False`, one core), JIT64, HLE audio, fast disc speed, cheats on, no memory cards, panic handlers and stop confirmation off, and no device behind any controller (the movie drives them; nothing touches the GameCube adapter). The movie's header carries the same settings, which Dolphin applies.
- **Hidden.** Dolphin runs on a desktop of its own, as ssbm-rs's `tools/gx/hidden.ps1` does, inside a job object that ends it when the harness exits, however it exits. Each run stops Dolphin and waits for it to be gone.
- **Inputs.** A DTM movie of four controllers, the replay's pre-frame controllers (raw sticks, buttons, triggers as raw/140) on the first two. A movie has one entry per controller poll. Dolphin polls twice a field, and the game's ticks keep a clock a little faster than the fields, so the poll each frame takes drifts: two polls on from the frame before's, and one about every 500 frames. Near those steps the frame's tick reads close to a poll's time, so which poll it gets depends on how much the game did before it (frame 6814 of `fox-fox-fd` takes the poll after the one its neighbours' pace gives). `--calibrate` finds each frame's poll (`DIR/NAME.dolphin`): the third controller, an unused port, counts the movie's entries on its buttons and triggers (moving sticks would cost the game work and shift those reads; buttons and triggers cost the same whatever they hold), and the harness reads which entry each frame took. HSD's queue position moves one a tick, so against the frame counter it shows the reads taken after the next tick had already taken its poll; those are dropped. The first run has the fighters stand still; each next run plays the replay's controllers placed by what the runs so far showed, and keeps what it shows up to where it leaves the replay; it stops when a run plays the whole match as the replay did, and keeps that run's placement. So far that has taken two runs per match. Calibration plays at twice the console's speed, so the game idles between frames and the harness reads each in time.
- **Timing.** The harness reads Dolphin's emulated memory (the MEM1 mapping in Dolphin's process) as each frame ends and notes the time; frames per second are counted from the match's first frame it sees to its last, so boot and loading don't count.
- **Checking.** At each frame it sees, every fighter's (Nana's too) action state, position, facing, percent and stocks and the random seed must equal the replay's. A frame read while the next tick has begun can show a mix, so a run counts as out of sync only after 120 differing frames in a row (a real desync lasts the rest of the match). Once past the match's last frame (the game shows "GAME!"), every fighter's stocks and percent must equal the replay's last frame. A run is in sync only if both hold.

Run it as `SSBM_DISC=... DOLPHIN=... melee-bench dolphin --dir "$SSBM_RS_LOCAL/bench" --calibrate` once (a few minutes per match), then `melee-bench dolphin --dir ...` for the median of 5. A Dolphin build or configuration change needs a new calibration.

### Provisional results (2026-10-07)

**Provisional, not the baseline:** measured while eight other agents were building and testing on the machine, with every core busy (Windows reported 100% CPU), so the numbers are low and swing by a factor of two or more between runs of the same match. The official numbers wait for a quiet machine. Two runs per match, Dolphin 2609-31 (master, portable), the machine above.

| Match | Frames/s (runs) | In sync |
| --- | --- | --- |
| `fox-fox-fd` | 180, 168 (and 73 to 160 in six earlier runs) | yes, every run: about 15,550 frames compared each, final stocks and percents equal |
| `fox-marth-bf` | 152, 187 | yes |
| `fox-marth-fd` | 176, 158 | yes |
| `fox-marth-ps` | 161, 47 (24, 18 earlier, at the busiest) | yes |
| `ics-peach-ps` | 106 without the DI patch; 61, 64 with it | without the patch no (from frame 10,139, heap layout; see below); with it yes, every frame and the final stocks and percents |

**Ice Climbers–Peach leaves the replay because the game reads an uninitialized register, whose value is a heap address.** Nana is the only fighter in these replays that runs CPU code (the controllers play the others). At frame 10,137 Peach is back-throwing her, her knockback velocity is still zero, and the CPU's DI routine `ftCo_800AC5A0` (marked `@bug` in the decomp) takes its stick values from registers it never set in that case: the Y stick gets the low byte of `r30`, which its caller `ftCo_800B2790` set to the fighter pointer. Nana's `Fighter` is at `0x80F62420` in ssbm-rs and at `0x80F60660` in Dolphin, so `cpu.lstick.y` becomes `0x20` in one and `0x60` in the other. The throw releases her at frame 10,139 with different DI (velocity (-1.96, 1.76) against (-1.78, 1.93)), she lands at frame 10,160 in Dolphin (DownBoundU, a missed tech, which draws 6 more random numbers) and keeps falling in ssbm-rs. Up to that point every fighter struct, `stage_info`, the player blocks, Nana's input ring and every random-number call (caller and seed, logged in Dolphin by a diagnostic Gecko code) are the same on both sides, and the scene loop rendered once per tick on both. With a probe code that gives that uninitialized stick ssbm-rs's byte (`0x20`), Dolphin plays the whole match in sync, final stocks and percents included. Dolphin's interpreter (CPU core 0) plays the match exactly as its JIT does, frame for frame, so it isn't a JIT difference; and neither side had a disc read in flight or more than one tick per render near those frames.

All three fighters' structs are `0x1DC0` bytes lower in Dolphin: the heap layout differs, because ssbm-rs records the match with Slippi's bootloader (which carves a heap for its code list out of the arena) and recording codes (which allocate buffers), and Dolphin runs only the boot codes. Neither side is wrong: ssbm-rs ran the original machine code here. It means the start of a match that runs CPU code includes the fighters' addresses: "same match and start" for Ice Climbers (and any CPU player) needs the same heap layout, and melee-hd has to take the uninitialized byte from the address the oracle had (ssbm-rs's state trace records each fighter's address). Fox–Marth and Fox–Fox run no CPU code during the match, so they are unaffected.

**The DI patch (benchmark only).** So Ice Climbers–Peach is the same match in Dolphin, the harness adds one Gecko code to that match's Dolphin run (`tools/bench/gecko/di.s`, injected at `0x800AC6D0`): on the nearly-zero-knockback path only, it sets `r30` to the byte the recording read there, the low byte of Nana's `Fighter` address in ssbm-rs, read from ssbm-rs's state trace of the match's start (`NAME.start.trace`, which `make-matches.sh` writes; it is `0x20` here). It is added for any match whose replay has a follower, and never to ssbm-rs's codes. It makes the game read the value it read when the match was recorded; it adds two instructions to a path taken a few times a match and does no other work, so it changes nothing measurable about Dolphin's speed. With it, Dolphin plays the match in sync (both runs above).

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
