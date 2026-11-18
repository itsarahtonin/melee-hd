# The fast renderer

The fast renderer is a track of its own beside the refactor ([the refactor](refactor.md#the-fast-renderer)): a GPU-driven renderer for low-end PCs that becomes the default, with the exact renderer (melee-render's GX stream drawn by melee-gpu, [the player](player.md#rendering)) kept as the reference, the frames tests' oracle and an "accurate" option. It may differ from the exact renderer in vertex float maths, the rasterizer's 1/16 snap, depth precision and later enhancements; not in what's drawn, the order, materials, blending or effects (Sarah, 2026-10-19). Its own work needs no full verify: it is checked against the exact renderer (Sarah, 2026-11-14), by the harness below.

This document is F1: the comparison harness and the measurement setup. F2 onward add the renderer itself.

## The comparison harness

`melee-render-check fast` (`tools/render-check/src/fast/`) draws a frame two ways from the same state, by a renderer under test and by the exact renderer, and compares each draw's transformed positions and the image.

```sh
melee-render-check fast DISC REPLAY [--mutate NAME] [--scale N] [--out DIR] FRAME...
```

FRAME is a native frame (the documented frames' log frame less 3 for a replay; frames named in this document are log frames, as the documented frames name them; `CHECK_FP=slippi` for a Slippi replay, as for `compare`). Each frame is drawn with the eight before it, whose copies it may sample, by both renderers; with `--out`, each compared frame's two images and a heatmap are saved (`frame_N_reference.png`, `frame_N_test.png`, `frame_N_heat.png`). It exits with failure when a frame is out of tolerance, and prints a report per frame:

```text
== frame 597 (replay frame 474): FAIL
  geometry: 59 display draws, 65316 vertices: OUT OF TOLERANCE
    missing: display draw 105, Shape { jobj: JObjId(39), dobj: DObjId(51), pobj: PObjId(51), trsp: 1, rendermode: 0, vmtx: None }
  image 640x480: ΔE00 mean 5.656, max 43.0, 44.656% over 2.3; ...: OUT OF TOLERANCE
```

**Decision: the harness lives in `tools/render-check`.** It needs what `melee-render-check` already has: the native match driven from a replay or a recorded session (`native::drive`), the views and scales, the frames tests' list of matches (`tests/common`), and a renderer to stand for the reference. A tool beside it would copy all of that.

### What a renderer reports

A renderer under test implements `fast::FrameRenderer`: given a tick (`native::Tick`: the game's state after its display pass, the pass's `Draw` list, the archives and `main.dol`), it builds and draws the frame and returns the image and, when asked, each draw's transformed vertices. That is what the display pass hands on, so the harness works whether the fast renderer's records are built on the presentation side from it or come from melee-sim (the refactor's design call 6, open): either way they come from the tick.

Each draw it reports names the display pass's draw it came from (its index in `Tick::draws`), and gives its vertices in clip space (what a vertex shader outputs) with the viewport that maps them to the screen, as triangles, lines and points in the order GX expands its primitives (melee-gpu's: quads as (0, 1, 2) and (0, 2, 3), strips alternating their winding, fans around their first vertex). A renderer that draws a shape's display list as one indexed draw reports its triangles in that order. The exact renderer, which sends several of the pass's draws at once (a joint's shapes in a pass), names the first of them; the harness keys the other renderer's draws by the same group (`Frames::marks`, the stream's offset where each of the pass's draws starts, is the one addition to melee-render). Lines and points are compared as their vertices, before the quads melee-gpu makes of them.

### Geometry

Draws are matched by the display draw they came from, and the report names each display draw (its `Draw`, shortened) that is:

- **missing** or **extra**: drawn by one renderer and not the other;
- **out of order**: drawn by both, but not in the reference's order (the longest run in order is kept, the rest named);
- with **elements** of another kind or number (a shape drawn with fewer triangles);
- **moved**: with vertices out of tolerance, with how many and the worst.

**Positions** are compared on screen, in the console's pixels, wherever the vertex is in front of the eye (w above 1/1000 of the clip coordinates' size): at most **1/8 of a pixel** apart (`POSITION_PX`), and depths at most **1024** of the EFB's 24-bit units (1/16384 of the range, `DEPTH`). The rasterizer's snap moves a vertex at most 1/32 of a pixel each way, and float maths in another order a few ulps, far less than 1/8; a joint's matrices or a draw's vertices wrong move them by pixels. Off screen the tolerance grows with the distance from the viewport's middle, in half-diagonals (a vertex k half-diagonals out turns the parts of its edges on screen k times less than it moves), so a vertex far off screen, whose position float noise moves by pixels, still passes. A vertex near the eye's plane or behind it has no stable screen position (its w is a small difference of large numbers): its direction from the eye (clip space's x, y and w) is compared instead, within **1e-4 radians** (`ANGLE`). A vertex inside the depth range in one and outside in the other is noted, not failed: the GPU clips there, so it can change what's drawn however small the move (see **The depth range's edge** below), and the image measures what it changed.

### The image

Measured by CIEDE2000 (ΔE₀₀), the perceptual difference between two colors (about 2.3 is just noticeable), in blocks of 8 by 8 of the console's pixels (16 by 16 at 2x, and so on). Pixel by pixel, a renderer whose vertices move by fractions of a pixel changes a lot: edge pixels take the color across the edge, a thin line seen edge-on (Final Destination's edges, a platform's rim) flips a whole run of pixels, textures minified at a grazing angle sample other texels, and a copy magnified onto a surface magnifies its own flips (Fountain of Dreams' reflection, 80 by 60 texels over the water: a sixteenth of a pixel moved its blocks' mean colors by up to 65 ΔE₀₀). A wrong frame changes what a region looks like. Each block is measured two ways:

- **its mean color**, averaged in linear light as the eye does from far enough away, against the other frame's block there or a pixel off (`block_differences`): a texture sampling other texels or a copy flipping texels keeps a region's mean, a wrong material doesn't;
- **its pixels**: each pixel's ΔE₀₀ to the nearest color among the other frame's pixel there and its eight neighbors, both ways (`shift_tolerant`), averaged: a run of pixels flipped along an edge takes the color of the row beside it, a wrong material takes no color around it.

How much a block may differ depends on the block: a flat wall doesn't change under a sub-pixel shift, the reflection or a grid seen edge-on changes a lot. So the reference also draws each frame with every vertex 1/32 of a pixel right and up, and left and down, before the snap (`Mutation::Spread`), and a block's spread is the most either shifted frame's block differs from the reference's. **A block is within tolerance when either measure is at most 3 ΔE₀₀ plus twice its spread** (`BLOCK_LIMIT`, `SPREAD`, `Block::excess`), and the frame when every block is, with at most 1% of its pixels far apart (shift-tolerant ΔE₀₀ over 10, `FAR_LIMIT`). The report names the block nearest its limit with both measures and spreads; `--out`'s heatmap shows the plain ΔE₀₀ in gray, the shift-tolerant one in red, and frames the blocks over.

How far vertices may move is the geometry's to check; the image is there for what geometry can't see: materials, blending, effects, copies.

How far vertices may move is the geometry's to check; the image is there for what geometry can't see: materials, blending, effects, copies.

### Proving it: the exact renderer with a change

Until the fast renderer exists, the renderer under test is the exact renderer with a change made on purpose (`--mutate NAME`, `fast::exact::Mutation`): melee-render's stream run through melee-gx by the harness itself, as `melee_render::gpu::Gpu` does, so it can change what reaches melee-gpu. A change aimed at a draw takes its target from the reference's frame.

| Name | Change | Kind |
| --- | --- | --- |
| `none` | nothing | identical |
| `restate` | every draw drawn from a copy of its state, unchanged (the copy the others change is faithful) | identical |
| `color` | the largest draw that shows its material color (color channel 0 from the material register, read by a TEV stage rasterizing it): its material colors' red, green and blue 64 levels off | wrong frame |
| `drop` | the largest draw missing | wrong frame |
| `order` | the largest draw drawn after the one that follows it | wrong frame |
| `joint` | every draw of the largest joint in front of the eye with its position matrices moved sideways by 1% of their depth (a few pixels) | wrong frame |
| `edge` | the projection's depth offset one ulp further from zero | wrong frame (see below) |
| `ulp` | every draw's position matrices one ulp off, alternately up and down | float noise |
| `snap` | every vertex 1/32 of a pixel right and down before the snap, so each moves 0 or 1/16 of a pixel | float noise |

**Results** (2026-11-18, `feat/fast-f1`). Unchanged (`none`), every frame of the frames tests' matches is identical in geometry and image (`tests/fast.rs`, the documented frames of every stage the player offers: the benchmark and Slippi replays, the 23 other stages' oracle matches, the power-ups matches, Sarah's metal and cloak match and Invisible Melee), and so is `restate` on the Fox ditto's 600. Over the 101 frames of the 33 matches the documented stage frames are in (`tests/fast.rs`'s first 33 matches, before the power-ups):

| Change | Geometry | Image |
| --- | --- | --- |
| `ulp` | within on all 101 (worst 0.28 of the tolerance, a vertex 32 px off screen) | within on all 101 (the worst block needed 0.84 times its spread above 3 ΔE₀₀, of the 2 allowed); without the spreads and the pixel measure, a platform's rim on Icicle Mountain's 26100 flips a row of pixels (a block's mean 8 ΔE₀₀ apart) |
| `snap` | within on all 101 (0.044 px, 0.35 of the tolerance, everywhere) | within on all 101 (the worst block needed 1.66 times its spread above 3 ΔE₀₀); with blocks' mean colors alone at a fixed 2 ΔE₀₀, 15 of the sample's 21 frames failed, Fountain of Dreams' 9000 by 65 ΔE₀₀ |
| `color` | identical (the change is the material's alone) | the draw it picks showed on 84 frames: 79 fail; 5 change no block by more than about 3 ΔE₀₀ (the Fox ditto's 1000 and 5000, Fox vs Marth on Final Destination's 6000, Fountain of Dreams' 2600, the four-player 600), a dark draw's color off by a shade; on 17 the picked draw doesn't reach the screen at all (all four of Battlefield's) |

and over a sample of 21 of those frames, the wrong frames:

| Change | Geometry | Image |
| --- | --- | --- |
| `drop` | fails on all 21 (`missing`) | out of tolerance on 13; on the rest the largest draw is wholly hidden (a background behind the stage) |
| `order` | fails on all 21 (`out of order`) | changes on 3, out of tolerance on 2: draws that don't overlap draw the same in either order |
| `joint` | fails on all 21 (`moved`) | out of tolerance on 20 |
| `edge` | within, with the shadow cameras' erases noted on the depth range's other side | fails on 19 (ΔE₀₀ over 10 on up to 43% of the frame); Poké Floats' 900 and Icicle Mountain's 3000 erase no shadow texture that's sampled |

So geometry decides what's drawn, where and in what order; the image decides materials, and passes the noise a renderer of its own will make. The limits came from these frames: at 2 ΔE₀₀ plus twice the spread two `ulp` frames (Pokémon Stadium's 600, Icicle Mountain's 26100) and one `snap` frame (Fountain of Dreams' 9000) failed, at 3 none, and 3 is under what a wrong material does to a block wherever it shows clearly. A stricter limit would fail noise; a looser one would pass more of the shades `color` makes on dark draws. F2 onward should rerun the survey (the `fast` test, and the changes over all frames) when its renderer differs on purpose, and recalibrate here if it must.

The harness's own changes are its mutation tests: each wrong frame fails and each kind of noise passes, and breaking the harness fails them, each made locally: without the spreads (`SPREAD` 0) `a_sixteenth_of_a_pixel_passes` fails; with positions never out of tolerance `a_joint_off_fails` fails; with the order unchecked `a_draw_out_of_order_fails` fails. Deciding by the pixel measure alone (no block means) still passes every test on the sample: the means are a second, more lenient way for a block to pass (a run of pixels flipped along an edge in a block with little else), kept because the survey above was taken with both; a later survey could show them unneeded.

**The depth range's edge.** The shadow cameras erase their textures with a quad exactly on the near plane (clip z = -w, depth 0: `Camera` draws 14 and 52 of the Fox ditto's frames). One ulp in the projection's depth offset puts it outside, the GPU clips it, the shadows' textures aren't erased, and every surface that takes a shadow goes dark (Final Destination's top: ΔE₀₀ over 10 on 13% of the frame). So depth precision may differ only away from the range's edges: a fast renderer must put what lies on the near or far plane exactly there (or clamp depth rather than clip), as the exact renderer does by computing every vertex as the GameCube does. The harness notes every vertex that changes side.

### Running it

```sh
# One match's frames, unchanged or with a change.
SSBM_DISC=... target/release/melee-render-check fast "$SSBM_DISC" data/bench/fox-fox-fd.slp 597 4997
SSBM_DISC=... target/release/melee-render-check fast "$SSBM_DISC" data/bench/fox-fox-fd.slp --mutate joint --out out/ 597

# The frames tests' matches (every stage the player offers), and the changes on a sample of them.
SSBM_DISC=... SSBM_RS_LOCAL=... SLIPPI_REPLAYS=... \
    cargo test --release -p melee-render-check --features fast --test fast -- --ignored --test-threads=4
```

`tests/fast.rs` builds only with the `fast` feature, so the pre-push frames step doesn't run it. Unit tests of the measures (CIEDE2000 against Sharma's published pairs, the tolerances, the primitives' expansion, the matching) run with `cargo test -p melee-render-check`.

When the fast renderer exists, it implements `FrameRenderer` and takes the stand-in's place in `fast::cli` (a `--renderer` choice); the tests then run it over the same frames and expect every one within tolerance.

## Measuring speed

`melee-render-check bench` (`fast::bench`) draws a replay's frames with one renderer, headless, and measures each frame: the CPU's time building what it draws (the exact renderer's GX stream), submitting it (the stream run, the vertices transformed and the GPU's work submitted: on the renderer's own threads, as the player draws, until the frame's copy to the XFB returns), and the GPU's time finishing it after that. The simulation runs but isn't timed. Frames before `--from` are drawn but not measured (textures, pipelines and specialized shaders made); the frame stays on the GPU, with no readback.

```sh
melee-render-check bench DISC REPLAY [--scale N] [--from A] [--to B] [--one-thread] [--adapter NAME|fallback]
```

It prints mean and percentiles of each, frames a second (one over the mean of the three: each frame waits for the GPU before the next starts, so this is the renderer's alone with CPU and GPU in turn, a floor under the player, which overlaps them), and a table row. "CPU" (build and submit) is the player's "CPU build" (`--perf-report`), the quantity F2 and F3's targets name; the player's own rate in a window is `melee-bench player`'s ([performance](performance.md#rendering-speed)). When the fast renderer exists, it runs through the same `FrameRenderer` and the two compare on the same frames.

**The fixed runs**: the benchmark matches in `data/bench` (in the repository, so every machine has them) and Fountain of Dreams' Slippi replay (`Game_20260129T214216`, `CHECK_FP=slippi`; any Fountain of Dreams replay measures the same way where it isn't), frames 600 to 1800 (1,201 frames, 20 seconds of play), at 1x, 2x, 3x and 5x:

```sh
for s in 1 2 3 5; do
  for r in fox-marth-fd fox-marth-bf fox-marth-ps ics-peach-ps; do
    target/release/melee-render-check bench "$SSBM_DISC" data/bench/$r.slp --scale $s
  done
  CHECK_FP=slippi target/release/melee-render-check bench "$SSBM_DISC" "$SLIPPI_REPLAYS/2026-01/Game_20260129T214216.slp" --scale $s
done
```

**Low-end runs**, the CPU throttled (the low-end target is the CPU's frame building: the GPU's frame is 1.1 to 1.5 ms even at 5x):

- the renderer on the game's thread alone: `--one-thread`;
- one transform thread: `MELEE_GPU_WORKERS=1`;
- two cores: on Linux `taskset -c 0,1 target/release/melee-render-check bench ...`; on Windows, PowerShell's `$p = Start-Process -PassThru -NoNewWindow target\release\melee-render-check.exe -ArgumentList ...; $p.ProcessorAffinity = 3` (the first frames run on every core; they aren't measured);
- no GPU at all, `--adapter fallback` (WARP on Direct3D 12, llvmpipe on Vulkan): not measured: the fallback adapter takes hours for 300 frames on this machine (it ran 3.5 hours at every core's full use before it was stopped). Measure at most 10 frames (`--from 600 --to 610`) under a time limit, if at all.

### Baselines (2026-11-18, this machine)

melee-hd `feat/fast-f1` (main at `ab6df932` with F1), release; AMD Ryzen 7 9800X3D, NVIDIA GeForce RTX 5070 Ti (Vulkan), Windows 11 Pro. **Not quiet**: other agents' builds and checks ran throughout (the CPU 17% busy before the runs), so these are a starting point to compare builds on the same machine at the same time, not the reference numbers (design call 9: speed targets on the worker's exclusive runner). Milliseconds a frame, means (95th percentile for the CPU):

| Replay | Scale | Frames/s | CPU (p95) | Build | Submit | GPU after |
| --- | --- | --- | --- | --- | --- | --- |
| fox-marth-fd | 1x | 415 | 2.29 (3.00) | 0.43 | 1.85 | 0.13 |
| fox-marth-bf | 1x | 408 | 2.34 (2.93) | 0.49 | 1.85 | 0.11 |
| fox-marth-ps | 1x | 351 | 2.72 (3.33) | 0.54 | 2.18 | 0.13 |
| ics-peach-ps | 1x | 340 | 2.81 (3.35) | 0.50 | 2.31 | 0.14 |
| Fountain of Dreams | 1x | 181 | 5.40 (6.32) | 0.89 | 4.52 | 0.11 |
| fox-marth-fd | 2x | 381 | 2.45 (3.03) | 0.47 | 1.99 | 0.17 |
| fox-marth-bf | 2x | 358 | 2.62 (3.15) | 0.53 | 2.09 | 0.17 |
| fox-marth-ps | 2x | 347 | 2.72 (3.39) | 0.56 | 2.16 | 0.16 |
| ics-peach-ps | 2x | 354 | 2.66 (3.32) | 0.49 | 2.17 | 0.16 |
| Fountain of Dreams | 2x | 180 | 5.39 (6.19) | 0.88 | 4.51 | 0.16 |
| fox-marth-fd | 3x | 391 | 2.31 (2.87) | 0.47 | 1.85 | 0.24 |
| fox-marth-bf | 3x | 371 | 2.43 (2.99) | 0.51 | 1.92 | 0.26 |
| fox-marth-ps | 3x | 337 | 2.72 (3.32) | 0.55 | 2.17 | 0.25 |
| ics-peach-ps | 3x | 350 | 2.61 (3.16) | 0.49 | 2.12 | 0.25 |
| Fountain of Dreams | 3x | 185 | 5.18 (5.96) | 0.84 | 4.34 | 0.24 |
| fox-marth-fd | 5x | 316 | 2.40 (2.87) | 0.48 | 1.92 | 0.77 |
| fox-marth-bf | 5x | 288 | 2.50 (2.94) | 0.53 | 1.97 | 0.97 |
| fox-marth-ps | 5x | 283 | 2.78 (3.36) | 0.57 | 2.21 | 0.75 |
| ics-peach-ps | 5x | 289 | 2.57 (3.16) | 0.46 | 2.10 | 0.89 |
| Fountain of Dreams | 5x | 179 | 5.19 (5.91) | 0.82 | 4.37 | 0.40 |

Throttled, at 1x:

| Replay | Setting | Frames/s | CPU (p95) | Build | Submit | GPU after |
| --- | --- | --- | --- | --- | --- | --- |
| fox-marth-fd | `--one-thread` | 172 | 5.20 (6.29) | 0.47 | 4.73 | 0.62 |
| fox-marth-ps | `--one-thread` | 137 | 6.71 (8.08) | 0.57 | 6.14 | 0.57 |
| Fountain of Dreams | `--one-thread` | 79 | 12.01 (14.39) | 0.80 | 11.21 | 0.60 |
| fox-marth-fd | `MELEE_GPU_WORKERS=1` | 231 | 4.20 (5.44) | 0.45 | 3.75 | 0.13 |
| fox-marth-ps | `MELEE_GPU_WORKERS=1` | 201 | 4.85 (5.92) | 0.54 | 4.30 | 0.12 |
| Fountain of Dreams | `MELEE_GPU_WORKERS=1` | 113 | 8.72 (10.78) | 0.80 | 7.92 | 0.12 |

The CPU's frame is the same at every scale (2.3 to 2.8 ms, Fountain of Dreams 5.2 to 5.4: about 1.2 ms over the player's 4.4 on a quiet machine, [performance](performance.md#results-after-the-weeks-work-2026-10-19)) and the GPU's work after it grows only at 5x: every run is bound by the CPU's frame building, mostly the submit (the stream run and the vertices transformed). On one thread Fountain of Dreams falls under 80 frames a second: the low-end case the fast renderer is for. The two-core runs and the fallback adapter weren't measured here.

### On the worker and the MacBook

The worker (Ubuntu, Intel Core i9-7900X, its NVIDIA GPU through Vulkan; [the exactness round](performance.md#the-exactness-rounds-cost-2026-11-13-the-ubuntu-worker) describes it), in its exclusive runner, as design call 9 recommends:

```sh
cargo build --locked --release -p melee-render-check
export SSBM_DISC=... SLIPPI_REPLAYS=...
# the fixed runs, as above, then the low-end ones:
for r in fox-marth-fd fox-marth-ps; do
  target/release/melee-render-check bench "$SSBM_DISC" data/bench/$r.slp --one-thread
  MELEE_GPU_WORKERS=1 target/release/melee-render-check bench "$SSBM_DISC" data/bench/$r.slp
  MELEE_GPU_WORKERS=1 taskset -c 0,1 target/release/melee-render-check bench "$SSBM_DISC" data/bench/$r.slp
done
```

The worker has no integrated GPU; its GPU's frame is far from the limit, so the throttled CPU runs stand in for a low-end machine there. `GX_ADAPTER=1` prints the adapter the default picks; `--adapter NAME` picks another by name.

The MacBook (macOS through Metal, integrated graphics: the low-end GPU of the two), Sarah's call: the same build and fixed runs (`cargo build --locked --release -p melee-render-check`, then the loop above), and `--one-thread` and `MELEE_GPU_WORKERS=1` (macOS has no CPU affinity to set). `melee-bench player` runs the player on a hidden desktop on Windows only; elsewhere the player's own report is `melee-hd --replay R --ticks 1980 --scale N --perf-report FILE`.

Each machine's numbers go in a table here like the one above, with its build, driver and how quiet it was.
