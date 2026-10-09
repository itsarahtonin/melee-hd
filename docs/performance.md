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

`melee-bench snapshot CALL...` imports a real mid-match state from ssbm-rs's saved calls and measures a rollback snapshot of it: the `Game` value plus everything it owns on the heap, a snapshot (`clone`) and a restore (`clone_from`), and their allocations. The state is what melee-sim models so far (scheduler, HSD objects, fighters, stage, camera, collision), so the size will grow as items, effects and the rest land.

| Capture (stage) | GObjs | Joints | Snapshot | Allocations | Clone | Restore | Restore allocations |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Final Destination | 72 | 731 | 0.64 MB | 1,895 | 138.5 µs | 110.0 µs | 1,895 |
| Pokémon Stadium | 77 | 651 | 0.58 MB | 1,501 | 125.0 µs | 87.9 µs | 1,501 |
| Battlefield | 65 | 780 | 0.64 MB | 1,865 | 129.0 µs | 134.0 µs | 1,865 |

`melee-bench snapshot-match [--dir DIR] [--every TICKS] [MATCH]` measures a real match's states the same way, as the match plays natively (as `melee-bench native` plays it, `fox-fox-fd` by default) as far as it runs: every `TICKS` ticks (default 1000) the state is kept, and afterwards each is measured, together with a restore followed by 8 ticks (rollback's window) and the allocations each of those ticks makes, which the "no allocations" target counts.

On the Fox–Fox match (2026-10-09, as far as it runs: to frame 4,572 of 15,451, tick 4,695, where it stops at `Player_GetMoreFlagsBit5`), every 750 ticks:

| Tick (frame) | GObjs | Joints | Snapshot | Allocations | Clone | Restore | Restore allocations | Restore and 8 ticks | Allocations per frame |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 750 (626) | 66 | 523 | 0.76 MB | 1,536 | 99.0 µs | 19.8 µs | 0 | 3.68 ms | 1 |
| 1500 (1376) | 70 | 533 | 0.77 MB | 1,537 | 100.2 µs | 29.5 µs | 0 | 4.12 ms | 1 |
| 2250 (2126) | 66 | 529 | 0.78 MB | 1,536 | 100.5 µs | 28.8 µs | 0 | 3.46 ms | 1 |
| 3000 (2876) | 69 | 540 | 0.77 MB | 1,540 | 104.6 µs | 29.4 µs | 0 | 4.31 ms | 1 (max 4) |
| 3750 (3626) | 65 | 521 | 0.78 MB | 1,537 | 106.0 µs | 29.3 µs | 0 | 3.65 ms | 1 |
| 4500 (4376) | 67 | 528 | 0.78 MB | 1,536 | 96.1 µs | 24.6 µs | 0 | 3.23 ms | 1 |

Size, clone and restore are well inside their targets. A restore reuses the target's storage: the state's types derive `melee_derive::CloneFrom`, a `Clone` whose `clone_from` copies field by field (each field's own `clone_from`, so vectors, boxes and pools keep their capacity), and the few containers that would build anew (maps, file lists of strings) are copied only when they differ. A snapshot (`clone`) still allocates, since it makes a new copy. Restore and 8 ticks is at the 4 ms target, over it on some states: the ticks are the cost (see the profile below).

**Allocations.** An engine tick allocated about 1,550 times on this match (2026-10-08), almost all in the display pass and joint walks that collected vectors to walk over. Now walks read each next link as they go (none of their bodies changes the links they walk), envelopes are lent out instead of copied, small fixed-size lists are held inline, symbol names and voice ids are read in place, a freed shape's envelope lists are kept for the next shapes to reuse (spare storage a restore keeps and a snapshot leaves out), and `Match::recycle` takes a frame's event and draw buffers back. A frame's only allocation left is the text draw's box (`Draw::SisText`), the presentation's output. An engine tick still allocates the first time something grows past what it has held (a pool's first new slot, a fighter's first handle word): `melee-bench`'s check `ticks_and_restores_dont_allocate` plays the match to tick 4,600, restores its state from tick 1,000 and plays ticks 1,000 to 4,600 again, as a rollback does, and asserts that the restore and every one of those ticks make no allocation. `melee-bench alloc-sites [--from TICK] [--ticks N] [--rollback] [MATCH]` tallies where a stretch of ticks allocates, by backtrace (with a profiling build: line tables on, `CARGO_PROFILE_RELEASE_DEBUG=line-tables-only`).

## Simulation speed

`melee-bench native [--dir DIR] [--runs N] [MATCH...]` (`tools/bench/src/native.rs`) plays each benchmark match natively and headless on one thread: from the replay's start through `vs_match`'s replay path (Slippi's playback codes, `Match::step_replay`), with the display pass, in the console's floating point (`FpMode::Hardware`, as the matches were made). The match's files are read from the disc before the clock starts, and the fighters' addresses come from the match's start trace (`NAME.start.trace`), as the oracle takes them. A run is timed from the match's first tick to its last frame, and afterwards every frame's fighters (action state, position, facing, percent and stocks) are checked against the replay's post-frame updates, the fields the Dolphin harness checks. It prints `melee-bench dolphin`'s table (median of 5 runs, with the machine), so the two divide. A run that stops before the match's last frame is no result: its row says where it stopped and why, and gives no speed.

melee-hd's side isn't measured yet: it comes with the one-character Final Destination milestone, once the Fox–Fox match steps natively to its end. As a look so far (2026-10-09, not a result): the Fox–Fox match runs in sync to frame 4,572 of 15,451 (4,695 ticks) before code no port covers yet stops it, at about 2,230 frames per second over that stretch (about 1,830 before the allocations came out). Dolphin's side, the baseline, is below.

**Profile.** A sampled profile of that run (samply at 4 kHz, about 10,000 samples on the simulation's thread; `melee-bench symbolize RVA...` names the functions at a profile's offsets in its own executable, when the profiler can't find the PDB), by share of samples including what each calls: the engine ticks 69%, the display pass 19%, and the match's setup and disc reads, outside the timed stretch, the rest. In the ticks, joint matrices (`HSD_JObjSetupMatrix`: `Mtx34::srt` 21%, `Mtx34::concat` 13%) take 34%, about half of it from the playback codes' pass over every dirty matrix of each fighter's model (16%); the fighters' bone dynamics (`swing_chains`) 24%; animation interpretation (AObjs and FObjs) about 13%; collision about 5%. By function alone, the bit-exact fused float operations (`gekko_fp::fmadds`, `fmsub`, `fnmsubs`, the paired `ps_madds0` and `ps_madds1`, and the C library's `fma` under them) take about 33%, and `sin` and `cos` with their range reduction about 10%. Nothing is optimized for speed yet beyond the allocations; no change may alter a checked bit.

## Benchmark matches

| Match | Stage | Fighters (costume) | Seed | Frames | Length | Ends |
| --- | --- | --- | --- | --- | --- | --- |
| `fox-fox-fd` | Final Destination | Fox (0) vs Fox (1) | `1F0F0001` | 15,575 | 4:20 | port 2 out of stocks, port 1 on 1 |
| `fox-marth-bf` | Battlefield | Fox vs Marth | `1F0F0002` | 15,110 | 4:12 | Marth out of stocks, Fox on 3 |
| `fox-marth-fd` | Final Destination | Fox vs Marth | `1F0F0003` | 15,241 | 4:14 | Fox out of stocks, Marth on 1 |
| `fox-marth-ps` | Pokémon Stadium | Fox vs Marth | `1F0F0004` | 13,585 | 3:46 | Fox out of stocks, Marth on 1 |
| `ics-peach-ps` | Pokémon Stadium | Ice Climbers vs Peach | `1F0F0005` | 12,655 | 3:31 | Ice Climbers out of stocks, Peach on 1 |

Each is a VS match (4 stocks, 8 minutes, items off, pausing off; settings as a Slippi Dolphin VS match records them) with both players human, and the controllers both played. The Fox–Fox match is there for the first measurement, while only Fox is ported. Frames count from the first (`-123`) to the last. The matches live outside the repository, in ssbm-rs's `local/bench` on this machine: `NAME.ini` (the Gecko codes below), `NAME.slp` (the match), and the logs of making and checking it.

**How they're made.** None of Sarah's replays is a Fox–Marth or Ice Climbers–Peach singles match, and a CPU match can't simply be replayed as a human one: a CPU's stick reads v/127 where a controller's reads r/80 after Melee's clamp, so its inputs mostly aren't a controller's, and the CPU AI draws from the game's random seed, so without it every later random number moves. So the game's level-9 CPU AI plays both fighters *through their controllers*: in ssbm-rs, around `Fighter_procCpu` each human player counts as a CPU, the AI decides, the random seed is put back as it was, and what the AI asked for (sticks scaled to a controller's range, triggers to its) becomes that controller's next poll. The fighters do only what the controllers say, and the match is the CPUs' closed-loop play. Before "GO" the controllers stay centred.

The match starts the way Slippi's playback starts a replay: Gecko codes (`melee-bench codes DIR`, from `tools/bench/gecko/*.s` and Slippi's own boot, Init Player Data, Init Stage Data and ExtendPlayerBlock codes) boot into DebugMelee's result screen, which loads the match's files and moves on to the match, and StartMelee takes the match's settings and seed where Slippi's playback restores a replay's game info. No EXI device is involved. Slippi's recording codes (loaded by its bootloader in ssbm-rs) record the match as a replay. Slippi's playback set also has "ScenePrep_DebugResult = ScenePrep_DebugMenu" (`041B16A8 801B09C0`), which would break the result screen; Dolphin's INI reader drops it (its comment has an `=`), so playback never applies it and neither do these codes.

The floating point is the console's (ssbm-rs's `Hardware` mode, which mainline Dolphin reproduces), not Slippi's Ishiiruka Dolphin's, so these replays are for this harness, ssbm-rs and melee-hd; Slippi Dolphin may play them differently.

**Checks.** Each match is played again two ways, and both must find no difference from the recording in any compared field of any frame:

- ssbm-rs's replay check: the match played back through Slippi's playback codes (a different boot path, the recorded inputs injected per frame), with `--fp hardware` (`NAME.check.log`).
- The recorded controllers alone (`BENCH_PADS`), without the AI (`NAME.pads.log`). This is what Dolphin and melee-hd get.

All five pass both: 95,826, 91,239, 92,122, 82,093 and 101,048 events compared, 0 divergences.

**Regenerating them.** Build ssbm-rs's runner from its branch `feat/bench-matches` (`cargo build --locked --release -p ssbm-run`; its `tools/run/src/bench.rs` has `BENCH_CODES`, `BENCH_AI`, `BENCH_PADS` and `BENCH_SLP`), then:

```sh
SSBM_RUN=.../ssbm-run.exe SSBM_DISC=... JOBS=3 bash tools/bench/make-matches.sh /c/Projects/ssbm-rs-publication/local/bench
```

For each match that writes the codes, records it (`BENCH_CODES=NAME.ini BENCH_AI=0,1 BENCH_SLP=NAME.slp ssbm-run --fields 31000`), runs both checks, and saves ssbm-rs's state trace of the match's first frames (`NAME.start.trace`, with `--port sysdolphin/baselib/perf`) for the fighters' heap addresses. It is deterministic: rerunning it after ssbm-rs's runtime fix ab3cd01 made byte-identical replays. Each run takes ssbm-rs 10 to 15 minutes on one core.

## Dolphin

`melee-bench dolphin [--dir DIR] [--runs N] [--calibrate] [MATCH...]` (`tools/bench/src/dolphin.rs`) plays the matches in the portable mainline Dolphin build at `C:\Projects\dolphin-dev\Dolphin-x64` (`DOLPHIN`), on the disc `SSBM_DISC`.

- **Setup.** A user directory of its own (`DIR/dolphin-user`), so Sarah's Dolphin configuration is untouched: the match's codes as `GameSettings/GALE01.ini`, batch mode, the Null video backend, no audio output, unlimited speed, the CPU on the emulation thread (`CPUThread=False`, one core), JIT64, HLE audio, fast disc speed, cheats on, no memory cards, panic handlers and stop confirmation off, and no device behind any controller (the movie drives them; nothing touches the GameCube adapter). The movie's header carries the same settings, which Dolphin applies.
- **Hidden.** Dolphin runs on a desktop of its own, as ssbm-rs's `tools/gx/hidden.ps1` does, inside a job object that ends it when the harness exits, however it exits. Each run stops Dolphin and waits for it to be gone.
- **Inputs.** A DTM movie of four controllers, the replay's pre-frame controllers (raw sticks, buttons, triggers as raw/140) on the first two. A movie has one entry per controller poll. Dolphin polls twice a field, and the game's ticks keep a clock a little faster than the fields, so the poll each frame takes drifts: two polls on from the frame before's, and one about every 500 frames. Near those steps the frame's tick reads close to a poll's time, so which poll it gets depends on how much the game did before it (frame 6814 of `fox-fox-fd` takes the poll after the one its neighbours' pace gives). `--calibrate` finds each frame's poll (`DIR/NAME.dolphin`): the third controller, an unused port, counts the movie's entries on its buttons and triggers (moving sticks would cost the game work and shift those reads; buttons and triggers cost the same whatever they hold), and the harness reads which entry each frame took. HSD's queue position moves one a tick, so against the frame counter it shows the reads taken after the next tick had already taken its poll; those are dropped. The first run has the fighters stand still; each next run plays the replay's controllers placed by what the runs so far showed, and keeps what it shows up to where it leaves the replay; it stops when a run plays the whole match as the replay did, and keeps that run's placement. So far that has taken two runs per match. Calibration plays at twice the console's speed, so the game idles between frames and the harness reads each in time.
- **Timing.** The harness reads Dolphin's emulated memory (the MEM1 mapping in Dolphin's process) as each frame ends and notes the time; frames per second are counted from the match's first frame it sees to its last, so boot and loading don't count.
- **Checking.** At each frame it sees, every fighter's (Nana's too) action state, position, facing, percent and stocks and the random seed must equal the replay's. A frame read while the next tick has begun can show a mix, so a run counts as out of sync only after 120 differing frames in a row (a real desync lasts the rest of the match). Once past the match's last frame (the game shows "GAME!"), every fighter's stocks and percent must equal the replay's last frame. A run is in sync only if both hold.

Run it as `SSBM_DISC=... melee-bench dolphin --dir /c/Projects/ssbm-rs-publication/local/bench --calibrate` once (a few minutes per match), then `melee-bench dolphin --dir ...` for the median of 5. A Dolphin build or configuration change needs a new calibration.

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
