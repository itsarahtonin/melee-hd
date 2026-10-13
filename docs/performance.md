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

Size, clone and restore are well inside their targets. A restore reuses the target's storage: the state's types derive `melee_derive::CloneFrom`, a `Clone` whose `clone_from` copies field by field (each field's own `clone_from`, so vectors, boxes and pools keep their capacity), and the few containers that would build anew (maps, file lists of strings) are copied only when they differ. A snapshot (`clone`) still allocates, since it makes a new copy. Restore and 8 ticks was at the 4 ms target then, over it on some states: the ticks are the cost (see the profile below). After the speed work below it is 2 to 3 ms on the whole match.

**Allocations.** An engine tick allocated about 1,550 times on this match (2026-10-08), almost all in the display pass and joint walks that collected vectors to walk over. Now walks read each next link as they go (none of their bodies changes the links they walk), envelopes are lent out instead of copied, small fixed-size lists are held inline, symbol names and voice ids are read in place, a freed shape's envelope lists are kept for the next shapes to reuse (spare storage a restore keeps and a snapshot leaves out), and `Match::recycle` takes a frame's event and draw buffers back. A frame's only allocation left is the text draw's box (`Draw::SisText`), the presentation's output. An engine tick still allocates the first time something grows past what it has held (a pool's first new slot, a fighter's first handle word): `melee-bench`'s check `ticks_and_restores_dont_allocate` plays the match to tick 4,600, restores its state from tick 1,000 and plays ticks 1,000 to 4,600 again, as a rollback does, and asserts that the restore and every one of those ticks make no allocation. `melee-bench alloc-sites [--from TICK] [--ticks N] [--rollback] [MATCH]` tallies where a stretch of ticks allocates, by backtrace (with a profiling build: line tables on, `CARGO_PROFILE_RELEASE_DEBUG=line-tables-only`).

## Simulation speed

`melee-bench native [--dir DIR] [--runs N] [MATCH...]` (`tools/bench/src/native.rs`) plays each benchmark match (`DIR` defaults to `data/bench`) natively and headless on one thread: from the replay's start through `vs_match`'s replay path (Slippi's playback codes, `Match::step_replay`), with the display pass, in the console's floating point (`FpMode::Hardware`, as the matches were made). The match's files are read from the disc before the clock starts, and the fighters' addresses come from the match's start trace (`NAME.start.trace`, in `DIR` or `$SSBM_RS_LOCAL/bench`), as the oracle takes them. A run is timed from the match's first tick to its last frame, and afterwards every frame's fighters (action state, position, facing, percent and stocks) are checked against the replay's post-frame updates, the fields the Dolphin harness checks. It prints `melee-bench dolphin`'s table (median of 5 runs, with the machine), so the two divide. A run that stops before the match's last frame is no result: its row says where it stopped and why, and gives no speed.

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

Each is a VS match (4 stocks, 8 minutes, items off, pausing off; settings as a Slippi Dolphin VS match records them) with both players human, and the controllers both played. The Fox–Fox match was the first measured, when only Fox was ported. Frames count from the first (`-123`) to the last. The replays are in the repository, `data/bench/NAME.slp` (inputs the CPU AI played, no game data); `make-matches.sh` writes them with the Gecko codes (`NAME.ini`, below), ssbm-rs's start traces (`NAME.start.trace`) and the logs of making and checking them into its folder (ssbm-rs's `local/bench`, `$SSBM_RS_LOCAL/bench`, where `melee-bench native` finds the start traces).

**How they're made.** None of the author's replays is a Fox–Marth or Ice Climbers–Peach singles match, and a CPU match can't simply be replayed as a human one: a CPU's stick reads v/127 where a controller's reads r/80 after Melee's clamp, so its inputs mostly aren't a controller's, and the CPU AI draws from the game's random seed, so without it every later random number moves. So the game's level-9 CPU AI plays both fighters *through their controllers*: in ssbm-rs, around `Fighter_procCpu` each human player counts as a CPU, the AI decides, the random seed is put back as it was, and what the AI asked for (sticks scaled to a controller's range, triggers to its) becomes that controller's next poll. The fighters do only what the controllers say, and the match is the CPUs' closed-loop play. Before "GO" the controllers stay centred.

The match starts the way Slippi's playback starts a replay: Gecko codes (`melee-bench codes DIR`, from `tools/bench/gecko/*.s` and Slippi's own boot, Init Player Data, Init Stage Data and ExtendPlayerBlock codes) boot into DebugMelee's result screen, which loads the match's files and moves on to the match, and StartMelee takes the match's settings and seed where Slippi's playback restores a replay's game info. No EXI device is involved. Slippi's recording codes (loaded by its bootloader in ssbm-rs) record the match as a replay. Slippi's playback set also has "ScenePrep_DebugResult = ScenePrep_DebugMenu" (`041B16A8 801B09C0`), which would break the result screen; Dolphin's INI reader drops it (its comment has an `=`), so playback never applies it and neither do these codes.

The floating point is the console's (ssbm-rs's `Hardware` mode, which mainline Dolphin reproduces), not Slippi's Ishiiruka Dolphin's, so these replays are for this harness, ssbm-rs and melee-hd; Slippi Dolphin may play them differently.

**Checks.** Each match is played again two ways, and both must find no difference from the recording in any compared field of any frame:

- ssbm-rs's replay check: the match played back through Slippi's playback codes (a different boot path, the recorded inputs injected per frame), with `--fp hardware` (`NAME.check.log`).
- The recorded controllers alone (`BENCH_PADS`), without the AI (`NAME.pads.log`). This is what Dolphin and melee-hd get.

All five pass both: 95,826, 91,239, 92,122, 82,093 and 101,048 events compared, 0 divergences.

**Regenerating them.** Build ssbm-rs's runner from its main branch (`3f9ac3a` and later; `cargo build --locked --release -p ssbm-run`; its `tools/run/src/bench.rs` has `BENCH_CODES`, `BENCH_AI`, `BENCH_PADS` and `BENCH_SLP`), then:

```sh
SSBM_RUN=.../ssbm-run.exe SSBM_DISC=... JOBS=3 bash tools/bench/make-matches.sh "$SSBM_RS_LOCAL/bench"
```

For each match that writes the codes, records it (`BENCH_CODES=NAME.ini BENCH_AI=0,1 BENCH_SLP=NAME.slp ssbm-run --fields 31000`), runs both checks, and saves ssbm-rs's state trace of the match's first frames (`NAME.start.trace`, with `--port sysdolphin/baselib/perf`) for the fighters' heap addresses. It is deterministic: rerunning it after ssbm-rs's runtime fix ab3cd01 made byte-identical replays. Each run takes ssbm-rs 10 to 15 minutes on one core.

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
