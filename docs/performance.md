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

Not measured yet: the first Dolphin comparison comes with the one-character Final Destination milestone, once a whole match steps natively.
