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

Size and clone time are well inside the targets. Restoring still allocates: derived `clone_from` rebuilds every owned `Vec` and map rather than reusing the target's storage, so the state's containers need `clone_from` implementations that reuse capacity (pools already have `restore_from`) before restores are allocation-free.

## Simulation speed

melee-hd's side isn't measured yet: it comes with the one-character Final Destination milestone, once a whole match steps natively. Dolphin's side, the baseline, is below.

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

For each match that writes the codes, records it (`BENCH_CODES=NAME.ini BENCH_AI=0,1 BENCH_SLP=NAME.slp ssbm-run --fields 31000`), and runs both checks. It is deterministic: rerunning it after ssbm-rs's runtime fix ab3cd01 made byte-identical replays. Each run takes ssbm-rs 10 to 15 minutes on one core.

## Dolphin

`melee-bench dolphin [--dir DIR] [--runs N] [--calibrate] [MATCH...]` (`tools/bench/src/dolphin.rs`) plays the matches in the portable mainline Dolphin build at `C:\Projects\dolphin-dev\Dolphin-x64` (`DOLPHIN`), on the disc `SSBM_DISC`.

- **Setup.** A user directory of its own (`DIR/dolphin-user`), so Sarah's Dolphin configuration is untouched: the match's codes as `GameSettings/GALE01.ini`, batch mode, the Null video backend, no audio output, unlimited speed, the CPU on the emulation thread (`CPUThread=False`, one core), JIT64, HLE audio, fast disc speed, cheats on, no memory cards, panic handlers and stop confirmation off, and no device behind any controller (the movie drives them; nothing touches the GameCube adapter). The movie's header carries the same settings, which Dolphin applies.
- **Hidden.** Dolphin runs on a desktop of its own, as ssbm-rs's `tools/gx/hidden.ps1` does, inside a job object that ends it when the harness exits, however it exits. Each run stops Dolphin and waits for it to be gone.
- **Inputs.** A DTM movie of four controllers, the replay's pre-frame controllers (raw sticks, buttons, triggers as raw/140) on the first two. A movie has one entry per controller poll. Dolphin polls twice a field, and the game's ticks keep a clock a little faster than the fields, so the poll each frame takes drifts: two polls on from the frame before's, and one about every 500 frames. Near those steps the frame's tick reads close to a poll's time, so which poll it gets depends on how much the game did before it (frame 6814 of `fox-fox-fd` takes the poll after the one its neighbours' pace gives). `--calibrate` finds each frame's poll (`DIR/NAME.dolphin`): the third controller, an unused port, counts the movie's entries on its buttons and triggers (moving sticks would cost the game work and shift those reads; buttons and triggers cost the same whatever they hold), and the harness reads which entry each frame took. HSD's queue position moves one a tick, so against the frame counter it shows the reads taken after the next tick had already taken its poll; those are dropped. The first run has the fighters stand still; each next run plays the replay's controllers placed by what the runs so far showed, and keeps what it shows up to where it leaves the replay; it stops when a run plays the whole match as the replay did, and keeps that run's placement. So far that has taken two runs per match. Calibration plays at twice the console's speed, so the game idles between frames and the harness reads each in time.
- **Timing.** The harness reads Dolphin's emulated memory (the MEM1 mapping in Dolphin's process) as each frame ends and notes the time; frames per second are counted from the match's first frame it sees to its last, so boot and loading don't count.
- **Checking.** At each frame it sees, every fighter's action state, position, facing, percent and stocks and the random seed must equal the replay's. A frame read while the next tick has begun can show a mix, so a run counts as out of sync only after 120 differing frames in a row (a real desync lasts the rest of the match). Once past the match's last frame (the game shows "GAME!"), every fighter's stocks and percent must equal the replay's last frame. A run is in sync only if both hold.
