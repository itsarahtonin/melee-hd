# The fast renderer

The fast renderer is a track of its own beside the refactor ([the refactor](refactor.md#the-fast-renderer)): a GPU-driven renderer for low-end PCs that becomes the default, with the exact renderer (melee-render's GX stream drawn by melee-gpu, [the player](player.md#rendering)) kept as the reference, the frames tests' oracle and an "accurate" option. It may differ from the exact renderer in vertex float maths, the rasterizer's 1/16 snap, depth precision and later enhancements; not in what's drawn, the order, materials, blending or effects (Sarah, 2026-10-19). Its own work needs no full verify: it is checked against the exact renderer (Sarah, 2026-11-14), by the harness below.

This document is F1, the comparison harness and the measurement setup, [F2](#f2-the-mesh-cache), the renderer's first part: the mesh cache, and [F3](#f3-transforms-skinning-and-lighting-on-the-gpu), the transforms, skinning and lighting on the GPU, and [F4](#f4-materials-as-records), materials as records, and [F5](#f5-the-whole-scene-first-round)'s first round: the frame run as it's built, and the player's choice of renderer, and its [second round](#f5-the-whole-scene-second-round): the flushes submitted and the stream run on threads of their own. F6 and the records from the `Draw` list add the rest.

## The comparison harness

`melee-render-check fast` (`tools/render-check/src/fast/`) draws a frame two ways from the same state, by a renderer under test and by the exact renderer, and compares each draw's transformed positions and the image.

```sh
melee-render-check fast DISC REPLAY [--renderer exact|f2|f3|f4|fast] [--mutate NAME] [--scale N] [--threaded] [--out DIR] FRAME...
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

How much a block may differ depends on the block: a flat wall doesn't change under a sub-pixel shift, the reflection or a grid seen edge-on changes a lot. So the reference also draws each frame with every vertex 1/32 of a pixel right and up, and left and down, before the snap (`Mutation::Spread`), and a block's spread is the most either shifted frame's block differs from the reference's. **A block is within tolerance when either measure is at most 3 ΔE₀₀ plus twice its spread** (`BLOCK_LIMIT`, `SPREAD`, `Block::excess`), and the frame when every block is, with at most 1% of its pixels far apart (shift-tolerant ΔE₀₀ over 10, `FAR_LIMIT`). Blocks are averaged from a summed-area table in fixed point (2^-32 of linear light), so its sums are exact: a block whose pixels are the same in both frames differs by exactly 0 on every platform. (In floats, the table's corners left a rounding error of about 1e-12 ΔE₀₀ that depended on everything above and left of the block, and which of those identical blocks the report named depended on the platform's maths library; `fast::image`'s tests failed on Linux and macOS for it until 2026-11-19.) The report names the block nearest its limit, among those that differ, with both measures and spreads; `--out`'s heatmap shows the plain ΔE₀₀ in gray, the shift-tolerant one in red, and frames the blocks over.

How far vertices may move is the geometry's to check; the image is there for what geometry can't see: materials, blending, effects, copies.

### Proving it: the exact renderer with a change

The harness's own stand-in for a renderer under test is the exact renderer with a change made on purpose (`--mutate NAME`, `fast::exact::Mutation`): melee-render's stream run through melee-gx by the harness itself, as `melee_render::gpu::Gpu` does, so it can change what reaches melee-gpu. A change aimed at a draw takes its target from the reference's frame.

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

The fast renderer implements `FrameRenderer` and is the renderer under test with `--renderer fast` (as far as it goes: F5's first round, [below](#f5-the-whole-scene-first-round), each draw with its specialized shader as after its warm-up), F4 with `--renderer f4`, F3 with `--renderer f3` and F2 alone with `--renderer f2`; `tests/fast.rs` runs the fast renderer over the same frames (`the_fast_renderer::*`) and expects every one within tolerance. `--threaded` runs the renderer under test on its own threads, as the player draws (its geometry is then the CPU's transforms of the meshes, the image the GPU's).

**Frames between ticks** (the player's High refresh, 2026-11-20). `--interpolate T` or `--extrapolate T` (T from 0 to 1) has every renderer, the reference, its spreads and the renderer under test, draw each tick's frame blended as the player draws its frames between ticks: Interpolate from the tick before's display pass (`blend::Capture`, kept as the player keeps it) toward the tick drawn, Extrapolate from the tick drawn toward the next, whose display pass is run ahead on a copy of the match with the replay's own inputs (`predict::Predictor`, the prediction made exactly; `native::drive_predicting`). The blend reaches the frame's stream (`Frames::frame_blended`, the fast renderer's `Gpu::run_streamed`), so the comparison is the same as a tick's. `the_fast_renderer_draws_blended_frames` (in `tests/fast.rs`) draws a sample, four fighters on Final Destination (600), Fox's ditto with its lasers and shines (600, 2600) and Fountain of Dreams (600), each way at T 0.25, 0.5 and 0.75 on one thread and at 0.5 on the renderer's threads: all 32 frames within tolerance on 2026-11-20, geometry identical on every one and the image on 29 (Fountain of Dreams' three others 0.18 ΔE₀₀ at worst). Mutation: the fast path dropping the blend (`Gpu::run_streamed` drawing the tick's own frame) fails them (Fox's ditto at 600: display draws moved by up to 20 pixels, interpolated and extrapolated, one thread and threaded; on its threads the image too, 7% of it far).

## Measuring speed

`melee-render-check bench` (`fast::bench`) draws a replay's frames with one renderer, headless, and measures each frame: the CPU's time building what it draws (the exact renderer's GX stream), submitting it (the stream run, the vertices transformed and the GPU's work submitted: on the renderer's own threads, as the player draws, until the frame's copy to the XFB returns), and the GPU's time finishing it after that. The simulation runs but isn't timed. Frames before `--from` are drawn but not measured (textures, pipelines and specialized shaders made), and the warm-up ends once every specialized shader they asked for is made; the frame stays on the GPU, with no readback. With `--warm-list FILE` the specialized shaders earlier runs needed (FILE, as the player saves its own) are asked for before the first frame, and this run's are added to FILE after: a replay's second run draws no call with the uber shader after the warm-up. It prints the measured frames' draw calls with their specialized shaders and with the uber shader (waiting for theirs). With `PERF_CTL_FIFO` and `PERF_ACK_FIFO` (`perf record -D -1 --control fifo:CTL,ACK`) a profile holds the measured frames alone.

```sh
melee-render-check bench DISC REPLAY [--renderer exact|f2|f3|f4|fast] [--scale N] [--from A] [--to B] [--one-thread] [--adapter NAME|fallback] [--warm-list FILE]
```

It prints mean and percentiles of each, frames a second (one over the mean of the three: each frame waits for the GPU before the next starts, so this is the renderer's alone with CPU and GPU in turn, a floor under the player, which overlaps them), and a table row. "CPU" (build and submit) is the player's "CPU build" (`--perf-report`), the quantity F2 and F3's targets name; the player's own rate in a window is `melee-bench player`'s ([performance](performance.md#rendering-speed)). `--renderer fast` measures the fast renderer through the same `FrameRenderer`, on the same frames.

**The fixed runs**: the benchmark matches in `data/bench` (in the repository, so every machine has them) and Fountain of Dreams' Slippi replay (`Game_20260129T214216`, `CHECK_FP=slippi`; any Fountain of Dreams replay measures the same way where it isn't), frames 600 to 1800 (1,201 frames, 20 seconds of play), at 1x, 2x, 3x and 5x:

```sh
for s in 1 2 3 5; do
  for r in fox-marth-fd fox-marth-bf fox-marth-ps ics-peach-ps; do
    target/release/melee-render-check bench "$SSBM_DISC" data/bench/$r.slp --scale $s
  done
  CHECK_FP=slippi target/release/melee-render-check bench "$SSBM_DISC" "$SLIPPI_REPLAYS/2026-01/Game_20260129T214216.slp" --scale $s
done
```

`data/bench/ics-peach-ps.slp` was recorded again on 2026-11-19, a different match ([performance](performance.md#ice-climbers-and-the-heap-layout)): every `ics-peach-ps` figure here dated before then is the old recording's. To compare with those, run the old replay, `$SSBM_RS_LOCAL/bench/ics-peach-ps.slp`, in its place; it renders frames 600 to 1800 as before (it leaves its recording only at frame 10,139).

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

**Measured on the worker (2026-11-19).** `feat/fast-f3` at `e596be85` (F2 and F3; F3 is `--renderer fast`), release with line tables; the NVIDIA GeForce GTX 1080 Ti through Vulkan (driver 580.178.04), the default adapter (`GX_ADAPTER=1`: "NVIDIA GeForce GTX 1080 Ti (Vulkan, NVIDIA)"). Each round ran in an exclusive reservation of the worker's runner after its quietness check (processor and GPU idle; 0 to 2% busy between scales). The fixed runs, frames 600 to 1800, in two rounds (the renderers' order exact, F2, F3 in the first and F3, F2, exact in the second); the Ice Climbers' match on Pokémon Stadium at 1x and 2x twice more. Each cell gives every round: frames a second; the CPU's frame (build and submit, the mean) in ms, and its 95th percentile in brackets. "F3 against F2" compares the rounds' mean CPU frames; "GPU after" is F3's mean wait for the GPU after the submit:

| Replay | Scale | Exact: frames/s; CPU ms (p95) | F2 | F3 | F3 against F2 | GPU after, F3 |
| --- | --- | --- | --- | --- | --- | --- |
| `fox-marth-fd` | 1x | 275/277; 3.40 / 3.43 (4.73/4.66) | 433/422; 2.22 / 2.29 (2.64/2.81) | 464/469; 2.04 / 2.01 (2.53/2.42) | -10% | 0.11 |
| `fox-marth-bf` | 1x | 271/293; 3.47 / 3.26 (4.68/3.85) | 421/423; 2.28 / 2.27 (2.67/2.74) | 435/440; 2.18 / 2.15 (2.48/2.40) | -5% | 0.12 |
| `fox-marth-ps` | 1x | 252/246; 3.83 / 3.92 (5.13/5.17) | 317/327; 3.02 / 2.93 (4.10/3.89) | 308/319; 2.97 / 2.91 (4.03/3.87) | -1% | 0.25 |
| `ics-peach-ps` | 1x | 262/268/268/266; 3.62 / 3.55 / 3.56 / 3.60 (4.88/4.79/4.88/4.88) | 333/348/323/334; 2.86 / 2.74 / 2.92 / 2.82 (4.11/3.87/4.09/3.94) | 323/334/314/336; 2.92 / 2.79 / 2.93 / 2.77 (4.16/4.08/4.40/3.68) | +1% | 0.21 |
| Fountain of Dreams | 1x | 110/108; 8.47 / 8.57 (10.20/10.25) | 172/169; 5.48 / 5.55 (7.21/7.07) | 248/237; 3.82 / 4.01 (4.67/5.04) | -29% | 0.21 |
| `fox-marth-fd` | 2x | 239/284; 3.28 / 3.19 (4.41/4.41) | 380/377; 2.28 / 2.30 (3.31/3.21) | 383/400; 2.22 / 2.08 (2.96/2.48) | -6% | 0.41 |
| `fox-marth-bf` | 2x | 273/222; 3.40 / 3.53 (4.55/4.79) | 383/387; 2.33 / 2.30 (3.06/2.92) | 350/388; 2.47 / 2.26 (3.65/2.61) | +2% | 0.35 |
| `fox-marth-ps` | 2x | 245/258; 3.87 / 3.66 (5.26/5.05) | 305/309; 3.07 / 3.00 (4.16/4.18) | 312/300; 2.95 / 3.08 (4.15/4.31) | -1% | 0.25 |
| `ics-peach-ps` | 2x | 269/267/259/250; 3.51 / 3.47 / 3.55 / 3.71 (4.92/4.76/5.01/5.10) | 322/322/315/310; 2.84 / 2.79 / 2.85 / 2.93 (4.17/4.14/4.17/4.27) | 298/315/306/304; 3.03 / 2.92 / 2.97 / 2.96 (4.45/4.26/4.31/4.27) | +4% | 0.30 |
| Fountain of Dreams | 2x | 114/136; 7.87 / 7.17 (9.36/8.41) | 166/209; 5.13 / 4.61 (6.58/5.82) | 251/250; 3.80 / 3.78 (4.66/4.29) | -22% | 0.20 |
| `fox-marth-fd` | 3x | 242/230; 3.27 / 3.26 (4.57/4.46) | 304/310; 2.42 / 2.35 (3.48/3.38) | 330/335; 2.13 / 2.10 (3.00/2.75) | -11% | 0.90 |
| `fox-marth-bf` | 3x | 229/242; 3.36 / 3.41 (4.30/4.62) | 310/320; 2.42 / 2.37 (3.58/3.18) | 320/329; 2.26 / 2.25 (2.70/2.61) | -6% | 0.82 |
| `fox-marth-ps` | 3x | 235/235; 3.71 / 3.73 (5.07/5.13) | 272/280; 3.07 / 2.92 (4.32/4.13) | 278/276; 2.93 / 2.95 (4.17/3.88) | -2% | 0.67 |
| `ics-peach-ps` | 3x | 229/236; 3.84 / 3.73 (5.32/5.22) | 280/284; 2.94 / 2.84 (4.35/4.15) | 268/266; 3.04 / 3.02 (4.40/4.49) | +5% | 0.71 |
| Fountain of Dreams | 3x | 127/128; 7.47 / 7.25 (9.70/8.74) | 196/190; 4.72 / 4.78 (6.08/6.29) | 225/241; 4.10 / 3.79 (5.45/4.38) | -17% | 0.35 |
| `fox-marth-fd` | 5x | 183/187; 3.31 / 3.18 (4.49/4.01) | 213/216; 2.52 / 2.46 (3.74/3.69) | 220/220; 2.23 / 2.24 (3.58/3.55) | -10% | 2.30 |
| `fox-marth-bf` | 5x | 180/187; 3.52 / 3.29 (4.75/4.15) | 219/218; 2.50 / 2.52 (3.66/3.81) | 220/224; 2.37 / 2.26 (3.26/2.75) | -8% | 2.19 |
| `fox-marth-ps` | 5x | 162/164; 3.87 / 3.76 (5.74/5.56) | 170/170; 3.14 / 3.12 (4.51/4.34) | 166/168; 3.16 / 3.02 (4.75/4.33) | -1% | 2.91 |
| `ics-peach-ps` | 5x | 163/165; 3.57 / 3.42 (5.23/4.66) | 163/165; 3.16 / 2.97 (4.76/4.27) | 159/160; 3.13 / 2.99 (4.77/4.32) | -0% | 3.20 |
| Fountain of Dreams | 5x | 118/120; 7.30 / 7.24 (9.41/8.66) | 160/165; 4.96 / 4.73 (6.66/6.04) | 180/181; 3.88 / 3.78 (5.16/4.68) | -21% | 1.70 |

The low-end runs, one round (the commands above with `--renderer`; two cores with `taskset -c 0,1`):

| Replay | Setting | Exact: frames/s; CPU ms (p95) | F2 | F3 | F3 against F2 | GPU after, F3 |
| --- | --- | --- | --- | --- | --- | --- |
| `fox-marth-fd` | `--one-thread` | 119; 7.08 (7.74) | 204; 4.37 (4.92) | 421; 2.17 (2.55) | -50% | 0.20 |
| `fox-marth-ps` | `--one-thread` | 90; 9.13 (10.16) | 145; 5.94 (6.73) | 265; 3.41 (4.32) | -43% | 0.36 |
| `ics-peach-ps` | `--one-thread` | 119; 8.35 (9.07) | 152; 5.76 (6.69) | 271; 3.39 (3.99) | -41% | 0.29 |
| Fountain of Dreams | `--one-thread` | 48; 17.74 (18.89) | 71; 11.54 (12.40) | 186; 4.45 (5.47) | -61% | 0.91 |
| `fox-marth-fd` | `MELEE_GPU_WORKERS=1` | 125; 6.77 (7.77) | 216; 4.21 (5.01) | 475; 2.00 (2.42) | -52% | 0.11 |
| `fox-marth-ps` | `MELEE_GPU_WORKERS=1` | 121; 7.57 (8.38) | 191; 4.87 (5.67) | 334; 2.83 (3.76) | -42% | 0.16 |
| `ics-peach-ps` | `MELEE_GPU_WORKERS=1` | 141; 7.02 (7.67) | 201; 4.58 (5.71) | 342; 2.76 (3.77) | -40% | 0.17 |
| Fountain of Dreams | `MELEE_GPU_WORKERS=1` | 62; 15.02 (16.92) | 92; 9.56 (11.07) | 245; 3.87 (5.10) | -60% | 0.22 |
| `fox-marth-fd` | `MELEE_GPU_WORKERS=1`, two cores | 126; 6.73 (7.53) | 216; 4.24 (4.87) | 415; 2.30 (2.68) | -46% | 0.11 |
| `fox-marth-ps` | `MELEE_GPU_WORKERS=1`, two cores | 113; 7.97 (9.03) | 174; 5.20 (6.28) | 276; 3.37 (4.20) | -35% | 0.25 |
| `ics-peach-ps` | `MELEE_GPU_WORKERS=1`, two cores | 121; 7.64 (8.57) | 174; 5.25 (6.36) | 265; 3.36 (4.40) | -36% | 0.41 |
| Fountain of Dreams | `MELEE_GPU_WORKERS=1`, two cores | 60; 15.43 (16.96) | 92; 9.56 (10.59) | 211; 4.42 (5.26) | -54% | 0.32 |

**What the worker says.** Threaded, F3's CPU frame is 2.0 to 3.2 ms on the benchmark matches and 3.8 to 4.1 on Fountain of Dreams: F2 cut the exact renderer's by 21 to 35%, and F3 takes Fountain of Dreams a further 17 to 29% below F2, Final Destination 6 to 11% below and Battlefield −8 to +2%; on Pokémon Stadium F3 and F2 are within the rounds' spread (Fox against Marth 1 to 2% below). With few threads F3 halves F2's frame or better (Fountain of Dreams on one thread 11.5 to 4.5 ms, with one transform thread 9.6 to 3.9). The 1 ms target isn't met on any stage; the worker's CPU is the low-end case here, its GPU's work after the submit 0.1 to 0.9 ms up to 3x (2.2 to 3.3 ms on the benchmark matches at 5x, where Pokémon Stadium's frame is as much the GPU's as the CPU's: F3's GPU wait 2.9 to 3.3 ms there against the exact renderer's 2.3 to 2.6).

**The Ice Climbers on Pokémon Stadium.** On the busy PC F3 was 7 to 18% slower than F2 threaded. On the worker it is 1% slower at 1x (four rounds each: F3 2.77 to 2.93 ms, F2 2.74 to 2.92), 4% at 2x, 5% at 3x and level at 5x: a small cost, not the busy PC's. A profile of the measured frames (`perf record -e task-clock -F 2000`, the frames 600 to 1800 alone, threaded at 1x; seconds on the CPU of the 4.5 s the frames took) shows the agent's guess in the direction it named, at a small size: the render thread ran 2.33 s under F2 and 2.46 s under F3 (1.9 and 2.05 ms a frame; under F3 its profile has the new work, `draw_gpu_mesh` and the meshes' pool flush, `GpuMeshes::flush`), each transform thread 1.05 s under F2 and 0.34 s under F3, and the game thread 3.05 and 3.1 s. Neither the render thread (about 52% busy) nor the game thread (67%) is saturated: the frame is their work in turn. So F3 adds about 0.1 ms a frame to the render thread, which is the critical path while the transform threads idle; giving the meshes' records to the transform threads, as the F3 work proposed (they build each mesh draw's record and hand it over, as they hand over the normals), would win back about that.

## F2: the mesh cache

**What it is.** The fast renderer's first part (`--renderer f2` in `melee-render-check fast` and `bench`, and under F3 in `--renderer fast`; `melee_render::gpu::Gpu::with_meshes` in code): the exact renderer with every display list it calls drawn from a mesh decoded once and kept, rather than fetched and decoded from the GX stream every frame. The player still draws with the exact renderer; switching it waits for F5.

- **Where the records come from** (design call 6): the presentation side. melee-render builds the frame's GX stream as before, which still carries every material, matrix and light (F3 and F4 take those over). melee-gx asks its host about each display list the stream calls (`Memory::call`, a new hook: a host that draws the list itself returns true and the list doesn't run), and the mesh cache (`melee_render::meshes::Meshes`) answers with the list's mesh.
- **A mesh** (`melee_gpu::mesh::Mesh`): the list's vertices fetched from their arrays and decoded, stream vertices whose attributes are the same bytes sharing one, and its primitives expanded into elements in the list's order and GX's: triangles (quads, strips and fans as melee-gpu expands them), lines (line strips as their segments) and points. A list holding anything else (a state change, another list, a vertex format other than 0, which HSD's shapes never use) is no mesh and runs as the stream has it. What a draw's state decides stays out of the mesh and comes at the draw: the matrix indices a vertex doesn't carry (XF's matrix index registers), the normals and binormals a format without them takes from the draws before, the scissor, and everything the transform unit does. The decoded attributes are plain arrays, one per attribute (positions, normals, colors, texture coordinates, matrix indices), and the elements' vertex numbers: the GPU buffers F3's transforms read. In F2 the transforms stay on the CPU and read the same arrays.
- **Drawing one** (`mesh::build`): each vertex through the transform unit as the exact renderer's are (`xform::Setup::transform` on the `vertex::Input` its bytes decode to), its words written once and its triangles naming them, back faces culled as melee-gpu culls them, lines and points as melee-gpu's quads. So a mesh draws what its list draws, bit for bit: the frames are the exact renderer's. On the renderer's threads a mesh is one packet (`Packet::Mesh`) built by a transform thread, and a large list is cut into parts of about 1,024 vertices (`Mesh::split`, whole elements in order) so the threads share it out: Fountain of Dreams' stars are lists of thousands of points, about 24,000 of the frame's 58,000 vertices, and with each list one packet the frame took 12 ms against the exact renderer's 8 (three transform threads idle while one built it).
- **The key** (`meshes::Key`): what the GPU reads to draw the list, named by where it is in the loaded archives rather than by address (`Space::locate`, a new accessor): the list (`PObj::display`) and its length; the game's rewrite of it, if any (refraction's matrix indices, `PObjs::refract_patches`, through `Space::patch`, a new accessor); the vertex descriptor and format 0; and each indexed attribute's array and stride (what `PObj::verts` set). So a rewritten list's key changes with its patch, and neither a shape's handle nor an address is part of a key: a freed shape whose place another takes, two shapes sharing a list with other arrays, and an archive's range of addresses given to another archive (`mem::Space`) all find other keys. Archives are immutable while loaded; the one thing that changes bytes under a key is a copy from the EFB landing in an archive's range (the magnifier's into `IfAll`'s image), so a list, or the arrays as far as its indices reach, with a copy over them draw as the stream has them (`Meshes::clear`). A mesh no frame drew for 600 frames goes.

**Decision: key by what the GPU reads, not by the shape.** The brief named `PObj::verts` and `display`. The vertex descriptions matter only through the vertex descriptor, formats and arrays they set, which the cache reads from the GX state at the call, so the key holds those with the list and the cache needs nothing from the shape: it covers HSD's vertex caches (`prev_vtxdesc`, which skip setting the same descriptions again) and any list the stream calls, and it sits on the GPU's side of the stream, where F3's buffers live.

**Results.** Every display list the frames measured call is drawn from a mesh: Fountain of Dreams keeps 741 meshes (3.0 MB) and draws about 500 lists a frame from them; the bench matches keep 348 to 513 (1.0 to 1.1 MB). What the stream still sends as draws is HSD's flush primitives (8 vertices each, about 170 a frame on Fountain of Dreams) and the immediate ones: sprites, particles, the HUD (about 210 quads a frame there).

**The frames.** `tests/fast.rs`'s `the_fast_renderer::*`, the documented frames of every stage the player offers (the 106 frames of 38 matches: the benchmark and Slippi replays, the 23 other stages' oracle matches, the power-ups matches, Sarah's metal and cloak match and Invisible Melee), each drawn by the fast renderer and by the exact one: every frame identical, geometry and image, on 2026-11-18 (`--test-threads=2`). No frame needed an exception. The exact renderer's own output is unchanged: the documented frames (`tests/documented.rs`, 535 with `--ignored`) pass, 533 in the full run and the other two (`tie4_stock_8741` and `8911`, which failed once there with the machine fully loaded) on their own and as a pair after it.

**Invalidation, proven** (each check made to fail by taking its part out, locally):

- `meshes::tests` (melee-render, with an archive's display list and arrays built in the test): a list rewritten after it was drawn (`a_rewritten_list_is_a_new_mesh`: refraction's patch, each vertex's texture matrix index its position matrix index) fails with the patch out of the key; a range given back and taken by another archive at the same addresses (`a_range_taken_by_another_archive_is_a_new_mesh`) fails with the list and arrays keyed by address; the same list with another shape's arrays (`the_same_list_with_other_arrays_is_a_new_mesh`) fails with the arrays out of the key; a copy from the EFB over an array (`a_copy_over_an_array_draws_from_the_stream`) fails without `Meshes::clear`.
- A list the game rewrites, in real frames: Sarah's `metal-cloak-rc` match, whose fighters' lists `lbRefract_PObjLoad` rewrites. With the meshes decoded from the archive's bytes, the rewrite ignored, its refracted frames 700 and 1000 (log frames 703 and 1003) fail (a refracting fighter's blocks 44 and 47 ΔE₀₀ apart, 20 and 19 blocks over the limit); 600 and 730, where nothing refracts, pass; with the rewrite all four are identical.
- `mesh_tests::a_mesh_draws_what_its_list_draws` (melee-gpu): 3,000 random lists (every primitive, random vertex formats, attributes direct and indexed, vertices shared), random transform states and normals left by the draws before: a mesh's elements' words and the normals it leaves are those of the list's draws built one by one (`melee_gpu::build`), whole and split in parts. Made to fail by a strip's odd triangles wound the other way, the matrix index registers ignored, one output kept from vertex to vertex where an emboss coordinate reads a later one, and the carried normals ignored. Writing it found one fault: a list sending no vertices left zeros for the normals where the draws before had left theirs.

**Speed** (2026-11-18, `feat/fast-f2`, the machine of the baselines above; **indicative only**: it was at 100% CPU throughout with other agents' oracle runs, builds and another `melee-render-check`, so runs of one build differed by up to 30%; design call 9: the worker's exclusive runner is what counts). The exact renderer and the fast one run alternately, the fixed runs' frames 600 to 1800 each; milliseconds a frame, means (95th percentile):

| Replay | Scale | Exact: frames/s, CPU (p95) | Fast: frames/s, CPU (p95) | CPU |
| --- | --- | --- | --- | --- |
| fox-marth-fd | 1x | 366, 2.60 (3.26) | 416, 2.25 (2.87) | -13% |
| fox-marth-bf | 1x | 325, 2.93 (3.59) | 447, 2.09 (2.61) | -29% |
| fox-marth-ps | 1x | 289, 3.30 (4.27) | 354, 2.68 (3.46) | -19% |
| ics-peach-ps | 1x | 324, 2.93 (3.99) | 346, 2.72 (3.67) | -7% |
| Fountain of Dreams | 1x | 136, 7.13 (9.27) | 205, 4.67 (6.17) | -35% |
| fox-marth-fd | 2x | 326, 2.88 (3.75) | 424, 2.19 (2.85) | -24% |
| fox-marth-bf | 2x | 316, 2.97 (3.90) | 447, 2.05 (2.65) | -31% |
| fox-marth-ps | 2x | 316, 3.00 (4.09) | 342, 2.75 (3.89) | -8% |
| ics-peach-ps | 2x | 309, 3.08 (4.62) | 365, 2.57 (4.02) | -17% |
| Fountain of Dreams | 2x | 146, 6.58 (8.27) | 222, 4.33 (5.76) | -34% |
| fox-marth-fd | 3x | 348, 2.65 (3.55) | 452, 1.99 (2.47) | -25% |
| fox-marth-bf | 3x | 351, 2.64 (3.50) | 465, 1.88 (2.34) | -29% |
| fox-marth-ps | 3x | 319, 2.91 (3.97) | 359, 2.55 (3.31) | -12% |
| ics-peach-ps | 3x | 299, 3.11 (4.60) | 346, 2.67 (3.75) | -14% |
| Fountain of Dreams | 3x | 147, 6.47 (8.18) | 221, 4.29 (6.04) | -34% |
| fox-marth-fd | 5x | 336, 2.47 (3.07) | 384, 2.09 (2.78) | -15% |
| fox-marth-bf | 5x | 265, 3.14 (5.63) | 384, 1.94 (2.48) | -38% |
| fox-marth-ps | 5x | 266, 3.33 (5.52) | 300, 2.87 (4.08) | -14% |
| ics-peach-ps | 5x | 284, 3.04 (4.59) | 298, 2.88 (3.94) | -5% |
| Fountain of Dreams | 5x | 139, 6.82 (9.44) | 206, 4.50 (6.29) | -34% |

Throttled, at 1x:

| Replay | Setting | Exact: frames/s, CPU (p95) | Fast: frames/s, CPU (p95) | CPU |
| --- | --- | --- | --- | --- |
| fox-marth-fd | `--one-thread` | 221, 4.35 (5.32) | 229, 4.21 (5.38) | -3% |
| fox-marth-fd | `MELEE_GPU_WORKERS=1` | 156, 6.14 (7.69) | 201, 4.82 (6.24) | -21% |
| fox-marth-ps | `--one-thread` | 155, 6.25 (8.50) | 169, 5.66 (8.13) | -9% |
| fox-marth-ps | `MELEE_GPU_WORKERS=1` | 150, 6.53 (7.82) | 195, 4.98 (6.35) | -24% |
| Fountain of Dreams | `--one-thread` | 79, 12.41 (18.55) | 74, 12.94 (17.18) | +4% |
| Fountain of Dreams | `MELEE_GPU_WORKERS=1` | 63, 15.69 (19.27) | 72, 13.60 (16.77) | -13% |

Fountain of Dreams on one thread is the noisiest row: four earlier alternating pairs the same day came out 10.8, 14.3, 11.7 and 13.2 ms for the fast renderer against 15.7, 15.0, 16.0 and 15.1 for the exact one (on one thread the meshes' building is the same transforms as the exact renderer's draws, less their fetching and decoding, so the cut there is smaller than threaded, where the game and render threads' share of the stream goes too).

The commands, as [above](#measuring-speed) with `--renderer`:

```sh
for s in 1 2 3 5; do
  for r in fox-marth-fd fox-marth-bf fox-marth-ps ics-peach-ps; do
    for which in exact f2; do
      target/release/melee-render-check bench "$SSBM_DISC" data/bench/$r.slp --scale $s --renderer $which
    done
  done
  for which in exact f2; do
    CHECK_FP=slippi target/release/melee-render-check bench "$SSBM_DISC" "$SLIPPI_REPLAYS/2026-01/Game_20260129T214216.slp" --scale $s --renderer $which
  done
done
# and --one-thread, MELEE_GPU_WORKERS=1, for fox-marth-fd, fox-marth-ps and Fountain of Dreams
```

**The target isn't met.** F2's target was Fountain of Dreams' CPU frame from about 4.5 to 5.4 ms to under 2.5. Here it went from 6.5 to 7.1 ms to 4.3 to 4.7 (a third less, at every scale), which on the baselines' 5.2 to 5.4 would be about 3.5; the bench matches gained 5 to 38%. Profiled (samply on a line-table build, Fountain of Dreams at 1x, threaded), what's left on the renderer's threads after F2:

- The transform threads build the meshes from their decoded arrays, nothing fetched or decoded, about 70% busy: the transform unit itself is most of it (`xform::Setup::transform`, lighting: two spot lights and a specular light on most vertices), then the vertices' words. On one thread the meshes' building is 53% of the frame's work and the transform 27%.
- The render thread is the critical path: each draw's state (textures, uniforms, specialized pipelines: 42% of its time) and wgpu (flushing 35%: encoding the render pass 19%, the vertices' and indices' upload 9%, submitting 10%), with the frame's 3.2 MB of vertex words and 0.8 MB of indices written every frame.
- The game thread: the stream built (0.8 to 0.9 ms) and run, the cache's lookups (`Meshes::get`, under 3% of the stream's run).

So the rest is F3's and F4's: transforms on the GPU take the transform threads' work and the per-frame vertex upload away (the meshes' arrays uploaded once, a draw's matrices and lights per draw), and materials as records take the render thread's per-draw state off the GX registers. One step that fits either: the meshes' triangles as GPU index buffers made once, with back faces culled by the GPU's pipelines (GX's back face is the clip-space determinant's sign, which the GPU's facing is; a pipeline per cull mode and viewport sign), which takes the indices' upload and the CPU's culling away; F2 keeps melee-gpu's per-frame indices, so its frames stay bit for bit the exact renderer's.

## F3: transforms, skinning and lighting on the GPU

**What it is.** The fast renderer as far as it goes (`--renderer fast`; `Renderer::gpu_transforms` with `Gpu::with_meshes` in code): F2's meshes kept on the GPU, and each mesh draw's vertices run through the transform unit there. The player still draws with the exact renderer; switching it waits for F5.

- **Kept on the GPU** (`melee_gpu::gpu_mesh`): each mesh's decoded attributes (positions, normals, colors, texture coordinates, matrix indices) in one pool and its elements' indices in another, uploaded at its first draw and kept while F2's cache keeps the mesh (600 frames unused). A pool grows by copying itself on the GPU, so every mesh keeps its place while a frame is drawn, and closes its gaps between frames once they're half of it. A large list is no longer cut into parts (F2's `Mesh::split` was for the transform threads).
- **Per draw** the CPU sends a record (the mesh's place, the matrix indices a vertex doesn't carry, the normals the draws before left, the scissor's offset, the texture coordinates' scales, the line and point sizes) and the draw's XF state in four sections: the position, texture and normal matrices (the joints' world and envelope matrices as HSD loads them), the post-transform matrices, the lights, and the registers. Each section is sent once a flush however many draws in a row share it.
- **The transforms** (`xform.wgsl`, a compute pass before each flush's draws): each vertex through the transform unit as `xform::Setup::transform` does it (positions by their matrix, skinned by the matrix index each vertex carries; normals and binormals; both color channels lit by up to eight lights, specular, spot and directional; the texture coordinates' generators, emboss, post-transform matrices and scales), then snapped to the rasterizer's sixteenths and written as the words the vertex shader reads (`GpuVertex::write`'s, read by `vs_mesh`). A line or point becomes its quad there, as melee-gpu's `line` and `point` make it.
- **Indices made once**, and back faces culled by the GPU's pipelines (GX's back face is the clip-space determinant's sign; the GPU's facing is the same with the viewport's width mirroring it: a pipeline per cull mode and facing, `Vertices`). A second compute entry (`cs_indices`) copies each draw's indices from the mesh's into the flush's run, offset to its output vertices, so a run of draws with the same state is one draw call. The draws the CPU still builds (HSD's flush primitives, sprites, particles, the HUD) go in the same buffers after the meshes': their words after the transforms' output, their indices (naming vertices) copied into the same run, so meshes and the rest share one vertex and one index buffer and merge into the same calls, as F2's did (Fountain of Dreams: 628 calls a frame, F2's 627; with buffers of their own, 997).
- **Threads**: the transform threads build only the CPU's draws now, and hand the render thread the normals the draws before a mesh left; the mesh's record and the draws' state are the render thread's.

**Decision: a compute pass, not the vertex shader.** The vertex shader could transform a mesh's vertices itself from the record. A compute pass transforms each vertex once (not once per triangle naming it), makes a line's or point's quad from its vertices once, and writes the very words the draws read, which the harness reads back as they are; the vertex shader stays the exact renderer's but for where a vertex's words start.

**The depth range's edge** ([above](#proving-it-the-exact-renderer-with-a-change)). Every product in the transforms goes through an operation the compiler can't see through (`o`, as gx.wgsl's `opaque`), so no multiply-add is fused; and so does every sum that feeds another add (`sum3` and `sum4` for the matrices' rows, the dot products and the spot lights' attenuation; the lights' running sums; each clip-space component; the screen position before a corner's offset), so no sum is reordered. Every add and multiply the GPU makes is then the CPU's IEEE one in the same order: the clip-space positions are the CPU's bit for bit, and what lies on the near or far plane stays exactly there. `gpu_mesh_tests` checks it (below); without the barrier in the position matrix NVIDIA's driver fuses, and positions move by ulps.

Why the sums too (2026-11-19): Metal compiles with fast maths (wgpu 30 leaves `MTLCompileOptions` at Apple's default, and has no setting for it), which reassociates additions. On CI's macOS runner the position rows `o(a x) + o(b y) + o(c z) + d` came out as `((d + a x) + b y) + c z`, one or two ulps off (`gpu_mesh_tests`, round 7, from batch 4 until this fix). NVIDIA's Vulkan compiler reorders sums too: the spot lights' `d.x + o(d.y dist) + o(d.z dist²)` and `c.x + ...`, whose random terms (`mesh_tests`' multiples of 1/500) often cancel, came out exactly 0 on one side and a tiny remainder on the other, so `safe_divide` gave 1 or a huge scale and a channel came out at its material's level or 0 (869 colors and 279 texture coordinates over tolerance on the worker's GTX 1080 Ti; no NaN or infinity involved). With the barriers, the worker's counts are D3D12's exactly. **Metal is verified only by CI's macOS runner** (GitHub's, its Metal adapter a real one, not a software fallback); no Mac here has run it. The barriers cost nothing measurable: on the worker's exclusive runner (GTX 1080 Ti, Vulkan, driver 580.178.04), `bench --renderer fast` at 1x, frames 600 to 1800, five runs of each build alternated, the medians of the runs' means before and after were 3.399 and 3.347 ms of CPU, 0.281 and 0.215 ms of GPU wait on Fountain of Dreams, and 2.559 and 2.605, 0.112 and 0.111 on fox-marth-ps: within the runs' own spread, in opposite directions. The shadow cameras' erase quads themselves are the CPU's draws until F5; `tests/fast.rs`'s `the_fast_renderer_keeps_the_depth_range_s_edge` holds the fast renderer to no vertex on the other side of the edge from the exact renderer's on the tournament stages' frames, where the erases are, and proves itself on `--mutate edge`, which it fails. On a GPU or shader compiler that fuses or reorders past the barriers anyway, the check is where it shows.

**The exact renderer on Metal** (checked in F5, [below](#gxwgsl-under-metals-fast-maths)): `gx.wgsl`'s one float sum that decides pixels, the screen depth, now keeps its product apart from its add, and a GPU test holds it to the CPU's depth bit for bit.

**The frames.** `tests/fast.rs`'s `the_fast_renderer::*`, the 106 frames of 38 matches (2026-11-18, `--test-threads=2`): every frame within tolerance. Every frame's geometry is identical to the exact renderer's (the harness reads the GPU's clip-space positions back, `Renderer::mesh_clips`, a dispatch of the same transforms per mesh draw); 74 images are identical, and 32 differ in a few pixels each (at most 0.005% of a frame over 2.3 ΔE₀₀, no block near its limit): lighting and texture coordinates go through square roots and divisions the GPU rounds otherwise. No frame needed an exception. `--threaded` (the renderer on its threads, as the player draws) was checked by hand on the Fox ditto's 600 and 11000, the Ice Climbers' 600 and 2000 and Fountain of Dreams' 600: the same. The exact renderer's own frames are unchanged: `gpu_transforms` is off for it, and the documented frames (`tests/documented.rs`, all 535 with `--ignored`) pass at `--test-threads=2`.

**The transforms, checked** (`gpu_mesh_tests::the_gpu_transforms_as_the_cpu_does`, melee-gpu, on the GPU; skipped without one): 1,500 of `mesh_tests`' random lists, transform states and carried normals (6,051 vertices; lines in 817 lists, points in 509). Each vertex's clip-space position is the CPU's bit for bit; the 43,023 output vertices compared (finite on the CPU) have their depth and w bit for bit, their positions within a 64th of a pixel or one snap step (all), colors within a level (all but 87, 0.2%) and texture coordinates within 1e-4 of their size (all but 34): random lights and states make the square roots and divisions' rounding count there. A line's caps turn by its slope, which float noise flips on a 45-degree line (168 corners, 0.4%, before the sums' barriers; 44 after, on D3D12 and on the worker's NVIDIA GPU alike; allowed 2%). The same counts on the worker's GTX 1080 Ti through Vulkan (43,230 output vertices compared there: 87 colors, 34 texture coordinates). Made to fail by: the position matrix's barrier taken out (clip positions differ, round 31), a line's corner on the wrong side (4,158 corners), a spot light's distance attenuation squared wrong (5,581 colors), and the texture scales swapped (310,516 coordinates).

**Speed** (2026-11-18, `feat/fast-f3`, the machine of the baselines above; **indicative only**: other agents' builds and checks ran throughout, the CPU 87% busy when the runs started, and runs of one build differed by up to 15%: fox-marth-ps at 1x came out 2.76 and then 2.63 and 3.13 ms for F3, against 2.64, 2.69 and 3.13 for F2; design call 9: the worker's exclusive runner is what counts, and these numbers decide nothing: Sarah, 2026-11-18, every speed number that decides anything runs there). Exact, F2 and F3 alternately, the fixed runs' frames 600 to 1800; milliseconds a frame, means (95th percentile):

| Replay | Scale | Exact: frames/s, CPU (p95) | F2: frames/s, CPU (p95) | F3: frames/s, CPU (p95) | F3 against F2 |
| --- | --- | --- | --- | --- | --- |
| fox-marth-fd | 1x | 358, 2.64 (3.19) | 441, 2.13 (2.57) | 502, 1.85 (2.24) | -13% |
| fox-marth-bf | 1x | 366, 2.60 (3.12) | 460, 2.03 (2.42) | 475, 1.97 (2.39) | -3% |
| fox-marth-ps | 1x | 313, 3.04 (3.69) | 360, 2.64 (3.23) | 345, 2.76 (3.44) | +5% |
| ics-peach-ps | 1x | 319, 2.99 (3.73) | 365, 2.59 (3.14) | 341, 2.78 (3.62) | +7% |
| Fountain of Dreams | 1x | 162, 6.04 (7.25) | 228, 4.21 (5.00) | 253, 3.75 (4.43) | -11% |
| fox-marth-fd | 2x | 347, 2.72 (3.26) | 449, 2.08 (2.47) | 487, 1.91 (2.34) | -8% |
| fox-marth-bf | 2x | 337, 2.81 (3.52) | 470, 1.99 (2.40) | 470, 1.98 (2.43) | -1% |
| fox-marth-ps | 2x | 317, 3.01 (3.65) | 359, 2.64 (3.21) | 372, 2.54 (3.29) | -4% |
| ics-peach-ps | 2x | 231, 4.15 (5.44) | 336, 2.82 (3.83) | 305, 3.13 (4.02) | +11% |
| Fountain of Dreams | 2x | 121, 7.96 (10.81) | 207, 4.62 (5.60) | 225, 4.22 (5.01) | -9% |
| fox-marth-fd | 3x | 347, 2.68 (3.23) | 402, 2.28 (2.87) | 449, 2.03 (2.57) | -11% |
| fox-marth-bf | 3x | 342, 2.73 (3.24) | 435, 2.10 (2.55) | 407, 2.25 (2.94) | +7% |
| fox-marth-ps | 3x | 264, 3.57 (4.68) | 303, 3.09 (4.03) | 266, 3.55 (4.80) | +15% |
| ics-peach-ps | 3x | 291, 3.22 (4.24) | 305, 3.05 (4.13) | 276, 3.40 (4.69) | +11% |
| Fountain of Dreams | 3x | 156, 6.13 (7.18) | 223, 4.22 (5.29) | 256, 3.71 (4.72) | -12% |
| fox-marth-fd | 5x | 311, 2.71 (3.38) | 343, 2.44 (3.14) | 352, 2.32 (2.99) | -5% |
| fox-marth-bf | 5x | 285, 2.90 (3.45) | 346, 2.23 (2.80) | 347, 2.24 (2.86) | +0% |
| fox-marth-ps | 5x | 260, 3.41 (4.35) | 293, 2.95 (3.66) | 279, 3.10 (3.91) | +5% |
| ics-peach-ps | 5x | 274, 3.18 (4.05) | 327, 2.54 (3.31) | 287, 2.99 (4.00) | +18% |
| Fountain of Dreams | 5x | 157, 5.98 (7.25) | 232, 3.95 (4.74) | 234, 3.90 (5.07) | -1% |

Throttled, at 1x:

| Replay | Setting | Exact: frames/s, CPU (p95) | F2: frames/s, CPU (p95) | F3: frames/s, CPU (p95) | F3 against F2 |
| --- | --- | --- | --- | --- | --- |
| fox-marth-fd | `--one-thread` | 207, 4.64 (7.11) | 254, 3.76 (5.20) | 573, 1.58 (2.28) | -58% |
| fox-marth-fd | `MELEE_GPU_WORKERS=1` | 176, 5.53 (6.57) | 212, 4.58 (5.68) | 463, 2.03 (2.72) | -56% |
| fox-marth-ps | `--one-thread` | 119, 7.93 (9.18) | 145, 6.57 (8.36) | 266, 3.60 (4.44) | -45% |
| fox-marth-ps | `MELEE_GPU_WORKERS=1` | 158, 6.19 (7.29) | 187, 5.20 (6.35) | 353, 2.71 (3.52) | -48% |
| Fountain of Dreams | `--one-thread` | 61, 15.83 (19.39) | 78, 12.29 (14.94) | 187, 5.19 (6.90) | -58% |
| Fountain of Dreams | `MELEE_GPU_WORKERS=1` | 77, 12.62 (15.08) | 101, 9.75 (11.81) | 245, 3.89 (5.12) | -60% |

So the transforms are off the CPU: where the renderer has few threads, the low-end case, F3 takes the CPU's frame to a half or less of F2's (Fountain of Dreams on one thread 12.3 to 5.2 ms, with one transform thread 9.8 to 3.9; the Ice Climbers' match on one thread, run again after, 7.0 and 9.5 ms to 4.6 and 3.7). With the default threads the gain is smaller (Fountain of Dreams 1 to 12% below F2, Final Destination 5 to 13%, Battlefield and the Fox match on Pokémon Stadium within the runs' noise), because F2's transform threads already did the transforms beside the render thread, which is the critical path there.

**One open point.** The Ice Climbers' match on Pokémon Stadium came out slower than F2 with the default threads in every run (7 to 18% in the table; run again three times later, 3.41, 3.45 and 2.97 ms against 2.82, 3.08 and 2.87, the machine at 100%), though its CPU work is far less (one thread and one transform thread above). F3 moves the meshes' records onto the render thread, the critical path, while the transform threads that shared F2's work are left nearly idle, and this match's render thread has the least to spare. Worth measuring on the worker's exclusive runner before acting; if it holds, the transform threads can build the records (each mesh draw's XF sections, which they already follow) and hand them over as they hand over the normals.

**The target, on this PC's numbers.** Under 1 ms of CPU a frame on every stage: whether it is met is the worker's runs' to say (below). Here F3 was 1.6 to 3.9 ms on the bench matches (one thread to threaded) and 3.7 to 5.2 on Fountain of Dreams, far enough above 1 ms that the worker's are unlikely to reach it. What's left, measured with GX_PROFILE's timers on Fountain of Dreams at 1x, one thread (milliseconds a frame):

- **The stream built**: 0.8 to 1.0 on Fountain of Dreams, 0.35 to 0.7 on the bench matches (melee-render building the GX stream from the display pass's draws), alone most of the target. Records built straight from the `Draw` list in place of the stream (F5's whole scene, with F4's materials) take it away.
- **The stream run** on the game thread (melee-gx's commands, the mesh cache's lookups, the threaded renderer's packets and the state's changes).
- **Each draw's state** on the render thread (`push_draw`: textures looked up and hashed, the TEV and pixel-engine uniforms, about 1.2 KB a draw, copied and hashed, specialized pipelines): 0.5 to 0.6. F4's materials as records.
- **wgpu**: about 1.4 in the flushes, of which encoding the render passes (`CommandEncoder::finish`, wgpu's validation and tracking of about 630 calls) 0.7, submitting 0.25, and the uploads and the transforms' pass 0.45. Fewer state changes a call (F4) and fewer calls cut it.
- **The transforms' records**: 0.1 to 0.2 for about 500 mesh draws.

The transforms were the largest part of F2's frame; what remains is spread over the stream, the per-draw state and wgpu, which F4 and F5 take.

**On the worker** (its exclusive runner, NVIDIA GTX 1080 Ti on Linux and Vulkan), the runs that decide, from this branch:

```sh
cargo build --locked --release -p melee-render-check
export SSBM_DISC=... SLIPPI_REPLAYS=...
FOD="$SLIPPI_REPLAYS/2026-01/Game_20260129T214216.slp"
for s in 1 2 3 5; do
  for r in fox-marth-fd fox-marth-bf fox-marth-ps ics-peach-ps; do
    for which in exact f2 fast; do
      target/release/melee-render-check bench "$SSBM_DISC" data/bench/$r.slp --scale $s --renderer $which
    done
  done
  for which in exact f2 fast; do
    CHECK_FP=slippi target/release/melee-render-check bench "$SSBM_DISC" "$FOD" --scale $s --renderer $which
  done
done
for which in exact f2 fast; do
  for r in fox-marth-fd fox-marth-ps ics-peach-ps; do
    target/release/melee-render-check bench "$SSBM_DISC" data/bench/$r.slp --renderer $which --one-thread
    MELEE_GPU_WORKERS=1 target/release/melee-render-check bench "$SSBM_DISC" data/bench/$r.slp --renderer $which
    MELEE_GPU_WORKERS=1 taskset -c 0,1 target/release/melee-render-check bench "$SSBM_DISC" data/bench/$r.slp --renderer $which
  done
  CHECK_FP=slippi target/release/melee-render-check bench "$SSBM_DISC" "$FOD" --renderer $which --one-thread
  CHECK_FP=slippi MELEE_GPU_WORKERS=1 target/release/melee-render-check bench "$SSBM_DISC" "$FOD" --renderer $which
done
```

The CPU column (build and submit, the 95th percentile beside it) is the target's quantity. `GX_PROFILE=1` prints the render thread's split every 300 frames (its draws, the meshes' transforms' records, the flushes) for what's left. The harness's frames there: `cargo test --release -p melee-render-check --features fast --test fast -- --ignored --test-threads=2 the_fast_renderer`, and `cargo test --release -p melee-gpu --lib gpu_mesh` for the transforms against the CPU's on that GPU.

## F4: materials as records

**What it is.** The fast renderer as far as it goes (`--renderer fast`; `Renderer::material_records` beside `gpu_transforms` and `Gpu::with_meshes` in code, `melee_gpu::material`): F3 with each draw's state looked up from its material's record rather than built from the GX registers, copied and hashed every draw. F3 as it was is `--renderer f3`. The player still draws with the exact renderer; switching it waits for F5.

- **A material** is the configuration a draw's pixels read: the BP registers gx.wgsl's fragment shader and the pixel engine read (`material::READ`: the gen mode, the indirect matrices and stages, the orders, blending, constant alpha and format, the maps' modes and sizes, the TEV's stages, fog, the alpha compare, the z texture and the konstant selections, and the z mode), less the maps' sizes and formats, and the viewport's z range and far z (which decide whether the depth test runs early). Each is made once into a record: its pipelines' keys, its passes (the depth pass first where an early z test must still write the depth the alpha test then discards), its specialized shader (asked for once per kind of vertices: mesh or not, cull mode, facing), the texture maps it samples, and its uniforms with every register outside `READ` zero.
- **A draw** gathers its material's registers and compares them with the last draw's (the same material again: 70 to 80% of draws), else finds the record by them. Its data (the TEV's color and konstant registers, which the game animates every frame, the maps' sizes and formats, and the viewport's width, which fog's range adjustment reads) goes into the record's uniforms, written once a flush for each record, data and textures' levels however many draws share them.
- **Its textures** are found by each map's registers (its modes, image and palette) where they were resolved before, until a copy, the game's writes, a range given back or an eviction changes the bytes they were made from (`Materials::forget`), or a palette's bytes in texture memory change (compared at each use); a texture whose bytes are hashed every frame (memory neither the host nor the game thread hands over whole) is kept for its frame alone. On Fountain of Dreams about 1,000 texture lookups a frame find their texture resolved, and 4 are resolved.
- **The uniforms hold only what the pixels read**, so draws that differ only in what they don't read (the scissor, the line width, the texture coordinates' scales, a copy's registers) share uniforms and, in a row, their draw call: on Fountain of Dreams on the worker 542 calls a frame against F3's 621, the uniforms set between calls 493 times against 622.

**Checked** (melee-gpu, on the GPU; skipped without one): `material_tests::the_pixels_read_only_the_material_s_registers` draws 64 random states (tev_tests' configurations, with random values in every other register) as they are and with only `READ`'s registers, and finds the EFB's color and depth the same bit for bit; made to fail by `READ` missing the konstant selections. `draws_from_records_draw_as_from_their_registers` draws 40 random states twice in a flush (the second with other colors, konstants, texture sizes, viewport width and registers the pixels don't read), from records and from the registers, with the uber shader and (every fifth) the specialized ones, and once more in the next flush, and finds the same EFB; made to fail by the texture sizes or the viewport width left out of a draw's data.

**Decision: the draw's data beside the record, not in its key.** The first records were keyed by everything the uniforms hold. The TEV's color registers are animated (materials' colors, lights, the fighters' flashes) and the viewport's width differs between cameras, so on Fountain of Dreams about 130 new records were made every frame, each with its uniforms; texture sizes made new combinations every few frames. Keyed by the configuration alone, a record is made about once every two frames there after the warm-up (new combinations as the match goes on; a record unused for 600 frames goes).

**Decision: uniforms per flush, as before.** A record's uniforms could live in a pool on the GPU, written once; but most draws' uniforms change with their data every frame, so the flush's buffer (one upload a flush) holds them, each made once a flush from its record without hashing its 1.2 KB again.

**The order.** Nothing is reordered: the state set between calls is cut only where draws in a row share it (the uniforms above, and a call joining the last when everything matches, as before). Pipelines and textures are set as often as under F3 (232 and 405 times a frame on Fountain of Dreams): draws in a row there differ in TEV configuration or textures, which only reordering would group.

**No uber-shader draws after the warm-up.** Specialized shaders are made on threads while the uber shader draws (`tev`). `melee-render-check bench`'s warm-up now ends once every specialized pipeline its frames asked for is made, and with `--warm-list` it first asks for those earlier runs needed, as the player does from its saved list. A configuration first seen in the measured frames still draws with the uber shader for the frames its pipeline takes to make: on Fountain of Dreams without a list about 300 to 400 calls of 650,000, from about 85 new TEV configurations and 40 new kinds of vertices for known ones after frame 600 (the exact renderer and F3 alike). With the lists, none of the 156 measured runs on the worker below (exact, F3 and F4) drew a call with the uber shader. The harness draws F4 as after its warm-up: every draw with its specialized shader, made as the draw first needs it (`TevMode::Sync` for the renderer under test).

**The frames.** `tests/fast.rs`'s `the_fast_renderer::*` (the 106 frames of 38 matches, and the depth range's edge) with F4 under test: every frame within tolerance, 2026-11-19 at `--test-threads=2`. The exact renderer's own frames are unchanged (`material_records` is off for it): the documented frames (`tests/documented.rs`, with `--ignored`) at `--test-threads=2`: all 535 pass.

**Speed on the worker (2026-11-19), the deciding numbers.** `feat/fast-f4` at `fee66c84`, release with line tables, on the worker's GTX 1080 Ti through Vulkan, in one exclusive reservation of its runner after its quietness check (processor and GPU idle; 0 to 2% busy between scales). Each replay's warm list was made first, by two runs of each renderer at 1x (a shared job). The fixed runs, frames 600 to 1800, threaded, in two rounds (exact, F3, F4 in the first and F4, F3, exact in the second), every run warm from its list; each cell gives both rounds: frames a second; the CPU's frame (build and submit, the mean) in ms, and its 95th percentile in brackets. "F4 against F3" compares the rounds' mean CPU frames; "GPU after" is F4's mean wait for the GPU after the submit:

| Replay | Scale | Exact | F3 | F4 | F4 against F3 | GPU after, F4 |
| --- | --- | --- | --- | --- | --- | --- |
| `fox-marth-fd` | 1x | 281/293; 3.39 / 3.25 (4.63/4.19) | 472/471; 2.01 / 2.01 (2.38/2.36) | 496/496; 1.88 / 1.88 (2.36/2.28) | -6% | 0.14 |
| `fox-marth-bf` | 1x | 283/272; 3.37 / 3.48 (4.34/4.48) | 436/434; 2.17 / 2.17 (2.54/2.52) | 427/464; 2.22 / 2.04 (3.11/2.29) | -2% | 0.12 |
| `fox-marth-ps` | 1x | 247/240; 3.91 / 3.98 (5.35/5.54) | 347/335; 2.77 / 2.81 (3.49/3.38) | 364/382; 2.63 / 2.51 (3.62/3.01) | -8% | 0.11 |
| `ics-peach-ps` | 1x | 268/255; 3.59 / 3.77 (5.01/5.19) | 332/332; 2.77 / 2.86 (3.84/4.14) | 367/380; 2.58 / 2.52 (3.87/3.75) | -9% | 0.13 |
| Fountain of Dreams | 1x | 112/109; 8.38 / 8.57 (9.68/10.76) | 250/246; 3.81 / 3.83 (4.53/4.53) | 255/274; 3.64 / 3.37 (4.85/4.09) | -8% | 0.28 |
| `fox-marth-fd` | 2x | 232/285; 3.38 / 3.18 (4.63/4.05) | 400/396; 2.08 / 2.11 (2.55/2.68) | 412/405; 1.94 / 2.00 (2.47/2.73) | -6% | 0.47 |
| `fox-marth-bf` | 2x | 284/283; 3.27 / 3.28 (4.01/4.07) | 381/385; 2.31 / 2.28 (2.88/2.79) | 401/403; 2.14 / 2.13 (2.66/2.69) | -7% | 0.35 |
| `fox-marth-ps` | 2x | 257/243; 3.69 / 3.83 (4.95/5.06) | 311/306; 2.88 / 2.95 (3.64/4.08) | 318/325; 2.73 / 2.66 (3.93/3.46) | -8% | 0.41 |
| `ics-peach-ps` | 2x | 266/266; 3.47 / 3.54 (4.72/4.78) | 309/314; 2.90 / 2.93 (4.16/4.26) | 311/324; 2.74 / 2.71 (4.15/4.06) | -7% | 0.43 |
| Fountain of Dreams | 2x | 114/133; 7.85 / 7.36 (9.21/9.48) | 252/252; 3.80 / 3.77 (4.74/4.72) | 277/278; 3.42 / 3.42 (4.28/4.25) | -10% | 0.18 |
| `fox-marth-fd` | 3x | 244/250; 3.23 / 3.21 (4.37/4.14) | 323/341; 2.24 / 2.06 (3.44/2.50) | 346/330; 1.99 / 2.12 (2.86/3.20) | -4% | 0.91 |
| `fox-marth-bf` | 3x | 226/232; 3.46 / 3.38 (4.75/4.42) | 326/321; 2.25 / 2.38 (2.64/3.24) | 338/330; 2.12 / 2.22 (2.51/3.37) | -6% | 0.82 |
| `fox-marth-ps` | 3x | 225/232; 3.95 / 3.87 (5.52/5.49) | 260/275; 3.18 / 2.99 (4.77/4.30) | 287/269; 2.64 / 2.92 (3.39/4.28) | -10% | 0.82 |
| `ics-peach-ps` | 3x | 251/242; 3.49 / 3.55 (4.62/4.93) | 275/274; 2.91 / 2.94 (4.27/4.30) | 270/279; 2.84 / 2.71 (4.28/4.00) | -5% | 0.87 |
| Fountain of Dreams | 3x | 126/133; 7.45 / 7.16 (9.66/8.17) | 236/244; 3.83 / 3.74 (5.03/4.54) | 242/240; 3.48 / 3.62 (4.64/4.94) | -6% | 0.60 |
| `fox-marth-fd` | 5x | 186/183; 3.23 / 3.30 (4.24/4.53) | 223/224; 2.16 / 2.13 (3.27/2.90) | 225/226; 2.01 / 2.02 (3.14/2.76) | -6% | 2.42 |
| `fox-marth-bf` | 5x | 185/184; 3.36 / 3.36 (4.53/4.32) | 221/220; 2.36 / 2.38 (3.15/3.33) | 220/222; 2.29 / 2.19 (3.51/3.23) | -5% | 2.29 |
| `fox-marth-ps` | 5x | 166/166; 3.72 / 3.71 (5.08/5.07) | 169/169; 2.99 / 3.04 (4.34/4.46) | 169/171; 2.87 / 2.76 (4.18/4.02) | -7% | 3.07 |
| `ics-peach-ps` | 5x | 165/164; 3.50 / 3.54 (4.79/5.00) | 160/159; 3.03 / 3.15 (4.50/4.74) | 161/160; 2.81 / 2.96 (4.22/4.49) | -7% | 3.33 |
| Fountain of Dreams | 5x | 120/120; 7.21 / 7.25 (8.89/9.03) | 181/181; 3.80 / 3.82 (4.79/5.08) | 183/182; 3.52 / 3.53 (4.76/4.77) | -7% | 1.96 |

The low-end runs, one round:

| Replay | Setting | Exact | F3 | F4 | F4 against F3 | GPU after, F4 |
| --- | --- | --- | --- | --- | --- | --- |
| `fox-marth-fd` | `--one-thread` | 117; 7.22 (8.01) | 417; 2.19 (2.61) | 451; 2.02 (2.28) | -8% | 0.20 |
| `fox-marth-fd` | `MELEE_GPU_WORKERS=1` | 127; 6.81 (7.93) | 481; 1.97 (2.37) | 506; 1.81 (2.20) | -8% | 0.17 |
| `fox-marth-fd` | `MELEE_GPU_WORKERS=1`, two cores | 123; 6.78 (7.68) | 407; 2.31 (2.71) | 451; 2.05 (2.41) | -11% | 0.17 |
| `fox-marth-ps` | `--one-thread` | 90; 9.13 (10.16) | 274; 3.39 (3.92) | 298; 3.16 (3.69) | -7% | 0.19 |
| `fox-marth-ps` | `MELEE_GPU_WORKERS=1` | 118; 7.69 (8.74) | 338; 2.81 (3.78) | 370; 2.53 (3.32) | -10% | 0.17 |
| `fox-marth-ps` | `MELEE_GPU_WORKERS=1`, two cores | 132; 7.49 (8.43) | 278; 3.27 (3.75) | 293; 3.08 (3.68) | -6% | 0.33 |
| `ics-peach-ps` | `--one-thread` | 118; 8.39 (9.34) | 279; 3.34 (3.96) | 296; 3.12 (3.74) | -7% | 0.26 |
| `ics-peach-ps` | `MELEE_GPU_WORKERS=1` | 130; 7.33 (8.41) | 340; 2.77 (4.07) | 370; 2.54 (3.90) | -8% | 0.17 |
| `ics-peach-ps` | `MELEE_GPU_WORKERS=1`, two cores | 110; 8.34 (9.82) | 278; 3.20 (4.08) | 289; 3.12 (4.13) | -3% | 0.35 |
| Fountain of Dreams | `--one-thread` | 61; 16.19 (17.47) | 183; 4.50 (5.39) | 201; 4.03 (4.90) | -10% | 0.94 |
| Fountain of Dreams | `MELEE_GPU_WORKERS=1` | 62; 15.03 (16.93) | 254; 3.76 (4.49) | 264; 3.47 (4.62) | -8% | 0.32 |
| Fountain of Dreams | `MELEE_GPU_WORKERS=1`, two cores | 60; 15.39 (16.45) | 214; 4.37 (5.28) | 227; 3.96 (4.65) | -9% | 0.45 |

**The per-draw state on the render thread** (`GX_PROFILE=1`'s "draws' state": textures, uniforms, pipelines and the calls queued, every draw; the mean of its four 300-frame windows over the measured frames, one run each, 1x threaded, in the same reservation). This is F4's target, and it is about halved everywhere:

| Replay | Per-draw state, F3 / F4 (ms a frame) | Change | Calls a frame, F3 / F4 | Uniforms set, F3 / F4 | Flushes, F3 / F4 (ms) |
| --- | --- | --- | --- | --- | --- |
| `fox-marth-fd` | 0.278 / 0.152 | -45% | 217 / 192 | 217 / 180 | 0.65 / 0.65 |
| `fox-marth-bf` | 0.303 / 0.166 | -45% | 254 / 241 | 254 / 228 | 0.71 / 0.73 |
| `fox-marth-ps` | 0.450 / 0.218 | -52% | 328 / 296 | 329 / 238 | 1.01 / 0.92 |
| `ics-peach-ps` | 0.454 / 0.221 | -51% | 336 / 304 | 337 / 231 | 1.09 / 0.98 |
| Fountain of Dreams | 0.693 / 0.348 | -50% | 621 / 542 | 622 / 493 | 1.51 / 1.40 |

So F4 takes the CPU frame 2 to 11% below F3's at every scale and setting (Fountain of Dreams from 3.8 to 3.4 to 3.6 ms threaded, 4.5 to 4.0 on one thread): the per-draw state's 0.13 to 0.35 ms, and a little of wgpu's encoding with fewer calls. The Ice Climbers on Pokémon Stadium, where F3 was a little slower than F2 threaded, are 5 to 9% below F3.

**Indicative, on the busy PC** (2026-11-19, the machine of the baselines above, with other agents' builds and checks running; these decide nothing): the same per-draw state, F3 / F4, 1x threaded, one run each: Final Destination 0.23 / 0.15 ms, Fox against Marth on Pokémon Stadium 0.35 / 0.21, the Ice Climbers 0.37 / 0.21, Fountain of Dreams 0.55 / 0.33; their CPU frames 1.84 / 1.74, 2.70 / 2.34, 2.78 / 2.47 and 3.59 / 3.39 ms.

**What's left toward 1 ms** (Fountain of Dreams on the worker: 3.4 to 3.6 ms threaded; the target is F5's since Sarah's call of 2026-11-19). The game thread builds the stream (about 0.97 ms there) and runs it; the render thread, the critical path while the game thread waits for the frame, spends about 1.4 ms a frame in its flushes (wgpu: encoding about 540 calls and the 1,170 state changes between them, the uploads, the transforms' compute pass, submitting 7 flushes), 0.5 to 0.65 in the meshes' records (`draw_gpu_mesh`: each mesh draw's XF sections and index copies) and 0.35 in the draws' state. A profile of the measured frames (`perf record -e task-clock`, F4 on Fountain of Dreams) has the render thread at about a third of the process's samples, most of it in the driver and wgpu's command encoding; the per-draw state's own functions are under 2% of the process. F5's records straight from the display pass take the stream's building and running away; the flushes are wgpu's work per call and per state change, which only fewer calls cut (F5's whole scene as records, longer runs of one state). F4's optional part, the meshes' records on the transform threads, isn't done: it would win back about the 0.1 ms F3 added to the render thread, little beside the flushes.

The commands (`cargo build --locked --release -p melee-render-check`, with `CARGO_PROFILE_RELEASE_DEBUG=line-tables-only`; `bench R ...` is `melee-render-check bench "$SSBM_DISC" R.slp --warm-list warm-R.bin ...`, Fountain of Dreams with `CHECK_FP=slippi`):

```sh
# The warm lists: each renderer twice on each replay (the second run draws no uber-shader call).
for r in fox-marth-fd fox-marth-bf fox-marth-ps ics-peach-ps fod; do
  for w in exact f3 fast exact f3 fast; do bench $r --renderer $w; done
done
# Two rounds, the renderers' order turned round in the second.
for s in 1 2 3 5; do
  for r in fox-marth-fd fox-marth-bf fox-marth-ps ics-peach-ps fod; do
    for w in exact f3 fast; do bench $r --scale $s --renderer $w; done
  done
done
# The low-end runs.
for w in exact f3 fast; do
  for r in fox-marth-fd fox-marth-ps ics-peach-ps fod; do
    bench $r --renderer $w --one-thread
    MELEE_GPU_WORKERS=1 bench $r --renderer $w
    MELEE_GPU_WORKERS=1 taskset -c 0,1 bench $r --renderer $w
  done
done
# The render thread's split.
for r in fox-marth-fd fox-marth-bf fox-marth-ps ics-peach-ps fod; do
  for w in f3 fast; do GX_PROFILE=1 bench $r --renderer $w; done
done
```

## F5: the whole scene (first round)

F5 is an L task in two rounds. This first round runs the frame as it's built, gives the player the fast renderer as a choice, and checks `gx.wgsl` under Metal's fast maths; what it measured decides the second ([what's left](#whats-left-toward-1-ms-after-f5s-first-round)).

**The frame run as it's built** (`--renderer fast`; `melee_render::gpu::Gpu::fast` and `Gpu::run_streamed`, `Frames::frame_streamed` in code). Through F4 the game thread built a frame's whole GX stream, then ran it, and the renderer's threads waited for the build (about 0.9 ms on Fountain of Dreams on the worker). Now the stream goes on in pieces as it's built: at each display draw's start, once at least 4 KB (`STREAM_PIECE`) wait and no display list is being built, the bytes so far run through melee-gx and on to the renderer's threads, which draw the frame's first draws while the rest are built. What's drawn is the same stream in the same order: only when each part of it starts running changes. `Frames::marks` still count from the frame's start, and the harness's geometry recorder takes them as they grow (`StreamObserver`). On one thread nothing draws beside the build, so there the fast renderer builds the frame first, as F4 did (`Gpu::draw`; on one thread the pieces cost 1 to 5% on the worker, below); the harness checks the streamed frame either way (`Exact::always_streamed`). F4 as it was is `--renderer f4`.

**Decision: stream the frame before replacing it.** The brief asked for records built straight from the `Draw` list in place of the GX stream, so the stream's building leaves the fast path. Measured first (the worker's profile below): with F4, the render thread is the critical path (about 2.5 ms of Fountain of Dreams' 3.4 ms frame), and the stream's building (0.9 ms) ran before it, not beside it. Running the stream as it's built takes the building off the frame's critical path at once, for every `Draw` kind, with nothing drawn differently; replacing the stream by records would then shorten the game thread's work, which only shows once the render thread is shorter than it. So the records wait for the render thread's work ([what's left](#whats-left-toward-1-ms-after-f5s-first-round)).

**The flushes' size.** "Fewer, larger submissions" was measured before it was made: the render thread flushes every 160 calls (`FLUSH_AT`), about 7 times a frame on Fountain of Dreams (4 of them for its copies). On the worker's exclusive runner, F5 with `MELEE_GPU_FLUSH_AT` 160, 320, 640 and unlimited (Fountain of Dreams and `fox-marth-ps`, 1x and 5x, two rounds): fewer flushes didn't cut the CPU's frame (Fountain of Dreams at 1x 2.70 to 2.88 ms at 160, 2.78 to 3.13 at the others; `fox-marth-ps` 2.29 to 2.32 against 2.40 to 2.59) and kept the GPU waiting longer after the submit (Fountain of Dreams at 5x 1.87 to 1.89 ms at 160, 2.16 to 2.21 at the others), since the GPU starts each flush's work only when it's submitted. The flushes' time is wgpu's work per call and per state change (encoding about 550 calls and 1,150 state changes a frame on Fountain of Dreams), not per submission, so 160 stays.

### `gx.wgsl` under Metal's fast maths

Metal compiles with fast maths (wgpu 30 leaves `MTLCompileOptions` at Apple's default, with no setting for it), which may fuse a multiply into an add and reorder sums. `gx.wgsl`'s TEV, texture filtering, alpha test, z textures, fog blend and EFB formats are integer maths, exact in any order. Its float maths: the screen depth (`depth.y + depth.x * (z - 1)`), the texture coordinates and colors moved back to the console's sample point (`uv - back * (dx + dy)`), the texture coordinates' projection, the level of detail's sums of two terms, and fog's range adjustment (products and sums of two, a square root and divisions). None sums three terms or more, so reordering can't change any; a fused multiply-add can. Of those, the depth decides pixels wherever two surfaces meet (the depth test compares integers truncated from it): NVIDIA's Vulkan compiler fused it, a depth unit off the CPU's for some depths (`0x3E165284` at z range 3437060 and far z 13936794: the GPU's 11004291, the CPU's 11004292). Its product now goes through `opaque` (as fog's subtraction already did), so no compiler fuses it. The rest change a texture coordinate by an ulp of a texel's 128th at most, which the exact renderer's frames' tolerance already takes from GPUs that fuse there; Metal's fast maths also computes divisions, square roots and `exp2` approximately, which no barrier stops (the projection and fog's range adjustment).

**Checked** (melee-gpu, on the GPU; skipped without one): `depth_tests::the_gpu_writes_the_cpu_s_depth` draws 6,656 flat quads at random depths (a quarter of them anywhere in the floats from 0 to 1), z ranges and far z, and finds every pixel's EFB depth the CPU's, unfused, bit for bit. Without the barrier it fails on this PC's RTX 5070 Ti through Vulkan (the depth above). **What CI's macOS runner now checks:** this test under Metal (GitHub's runner, a real Metal adapter), beside `tev_tests`' specialized shaders against the uber shader (both compiled with fast maths, so a reordering the two compile differently shows there) and `gpu_mesh_tests`' transforms against the CPU's. It doesn't check the rest of a frame's pixels against the CPU's; no Mac here has drawn one. The exact renderer's documented frames after the barrier: [below](#the-frames).

`copy.wgsl`'s box filter (a console pixel from the scaled EFB's pixels, above 1x) summed floats, whose ties' rounding followed the sum's order: it sums integers now, exact in any order (at 1x it averages one pixel, unchanged).

### The player's choice

The settings panel's Display section has a Renderer setting, Fast or Exact (`renderer = fast|exact` in `settings.txt`, `--renderer fast|exact` on the command line), fast by default since 2026-11-20 (Sarah: "fast default sounds good to me"), the exact one kept as the accurate choice ([the player](player.md#rendering)). Its wording, signed off the same day: Fast, "Draws the same picture with far less of the processor's time, giving high performance and the lowest latency."; Exact, "Draws every frame step by step as the console's graphics chip does: the reference the fast renderer is checked against."

### The frames

`tests/fast.rs`'s `the_fast_renderer::*` with F5 under test (the 106 frames of 38 matches, and the depth range's edge), 2026-11-20 at `--test-threads=2`: every frame within tolerance. The exact renderer's own frames with the depth barrier (`gx.wgsl` is both renderers'): the documented frames (`tests/documented.rs`, with `--ignored`) at `--test-threads=2` on this PC's RTX 5070 Ti through Vulkan, all 535 pass (2026-11-20, at `0702c3b2`): the depth unit the fusion moved changed no frame beyond the frames checks' tolerance against ssbm-rs's.

### Speed on the worker (2026-11-20), the deciding numbers

`feat/fast-f5` at `95a36a1e` with this section's docs (the worker's snapshot `357c13dd`), release with line tables, on the worker's GTX 1080 Ti through Vulkan, in one exclusive reservation of its runner after its quietness check (processor and GPU idle; 0 to 2% busy between scales). Each replay's warm list made first by one run of each renderer; then the fixed runs, frames 600 to 1800, threaded, in two rounds (exact, F4, F5 in the first and F5, F4, exact in the second), every run warm from its list and no measured call drawn with the uber shader. Each cell gives both rounds: frames a second; the CPU's frame (the mean) in ms, and its 95th percentile in brackets. "F5 against F4" compares the rounds' mean CPU frames; "GPU after" is F5's mean wait for the GPU after the submit:

| Replay | Scale | Exact | F4 | F5 | F5 against F4 | GPU after, F5 |
| --- | --- | --- | --- | --- | --- | --- |
| `fox-marth-fd` | 1x | 299/296; 3.30 / 3.20 (4.63/3.67) | 473/475; 2.01 / 1.97 (2.52/2.40) | 519/523; 1.82 / 1.78 (2.36/2.25) | -10% | 0.12 |
| `fox-marth-bf` | 1x | 297/270; 3.31 / 3.53 (4.14/4.73) | 453/455; 2.08 / 2.07 (2.35/2.29) | 484/482; 1.94 / 1.96 (2.32/2.33) | -6% | 0.12 |
| `fox-marth-ps` | 1x | 257/251; 3.73 / 3.84 (4.86/4.99) | 370/375; 2.57 / 2.57 (3.02/3.00) | 413/414; 2.32 / 2.31 (2.72/2.64) | -10% | 0.11 |
| `ics-peach-ps` | 1x | 259/258; 3.71 / 3.72 (4.77/5.03) | 361/335; 2.63 / 2.70 (3.69/3.87) | 404/408; 2.37 / 2.36 (3.21/3.27) | -11% | 0.10 |
| Fountain of Dreams | 1x | 107/111; 8.59 / 8.40 (10.47/9.34) | 289/265; 3.40 / 3.44 (4.13/4.06) | 353/328; 2.74 / 2.85 (3.02/3.69) | -18% | 0.15 |
| `fox-marth-fd` | 2x | 277/281; 3.28 / 3.22 (4.26/3.98) | 406/394; 1.98 / 2.06 (2.28/2.56) | 435/436; 1.82 / 1.81 (2.34/2.16) | -10% | 0.48 |
| `fox-marth-bf` | 2x | 239/270; 3.36 / 3.45 (4.03/4.33) | 395/383; 2.17 / 2.24 (2.42/2.94) | 407/417; 2.12 / 2.05 (3.01/2.68) | -5% | 0.34 |
| `fox-marth-ps` | 2x | 247/257; 3.85 / 3.61 (5.49/4.60) | 330/321; 2.69 / 2.70 (3.22/3.18) | 365/365; 2.39 / 2.39 (3.00/2.93) | -11% | 0.35 |
| `ics-peach-ps` | 2x | 264/253; 3.55 / 3.69 (4.84/5.00) | 310/301; 2.71 / 2.89 (3.96/4.21) | 357/361; 2.41 / 2.38 (3.58/3.48) | -14% | 0.39 |
| Fountain of Dreams | 2x | 115/134; 7.80 / 7.28 (8.95/8.86) | 269/268; 3.42 / 3.47 (3.92/4.48) | 307/325; 3.04 / 2.82 (4.10/3.73) | -15% | 0.24 |
| `fox-marth-fd` | 3x | 249/258; 3.14 / 3.15 (3.75/3.86) | 338/351; 2.03 / 1.99 (2.60/2.53) | 367/375; 1.84 / 1.81 (2.69/2.17) | -9% | 0.87 |
| `fox-marth-bf` | 3x | 247/239; 3.36 / 3.48 (4.33/4.50) | 323/325; 2.21 / 2.27 (3.04/3.12) | 346/368; 2.09 / 1.97 (2.72/2.28) | -9% | 0.77 |
| `fox-marth-ps` | 3x | 239/233; 3.70 / 3.84 (4.91/4.94) | 283/280; 2.68 / 2.73 (3.17/3.63) | 292/304; 2.59 / 2.46 (3.56/3.31) | -7% | 0.83 |
| `ics-peach-ps` | 3x | 237/232; 3.72 / 3.81 (5.08/5.21) | 269/272; 2.82 / 2.79 (4.23/4.12) | 305/298; 2.40 / 2.49 (3.45/3.73) | -13% | 0.87 |
| Fountain of Dreams | 3x | 131/128; 7.17 / 7.31 (8.26/8.74) | 245/242; 3.41 / 3.59 (3.86/4.71) | 299/288; 2.83 / 2.97 (3.73/4.08) | -17% | 0.51 |
| `fox-marth-fd` | 5x | 185/184; 3.24 / 3.27 (4.16/4.06) | 220/218; 2.10 / 2.18 (2.93/3.53) | 242/241; 1.82 / 1.86 (2.22/2.51) | -14% | 2.30 |
| `fox-marth-bf` | 5x | 183/182; 3.39 / 3.42 (4.36/4.46) | 217/217; 2.27 / 2.26 (3.39/3.63) | 237/234; 2.04 / 2.12 (2.69/2.97) | -8% | 2.17 |
| `fox-marth-ps` | 5x | 166/163; 3.65 / 3.81 (4.88/5.40) | 167/165; 2.79 / 2.98 (3.48/4.38) | 185/186; 2.62 / 2.55 (4.01/3.66) | -10% | 2.81 |
| `ics-peach-ps` | 5x | 156/156; 3.69 / 3.70 (5.16/5.18) | 153/154; 3.11 / 2.96 (4.85/4.50) | 170/171; 2.60 / 2.61 (4.16/4.00) | -14% | 3.26 |
| Fountain of Dreams | 5x | 120/120; 7.20 / 7.26 (8.60/8.99) | 180/177; 3.57 / 3.70 (4.80/5.02) | 210/207; 2.82 / 2.96 (3.89/4.04) | -20% | 1.91 |

The low-end runs, one round (on one thread F5 is F4, as `Gpu::draw` does; these ran the streamed frame there, so they show what the pieces cost without threads):

| Replay | Setting | Exact | F4 | F5 | F5 against F4 | GPU after, F5 |
| --- | --- | --- | --- | --- | --- | --- |
| `fox-marth-fd` | `--one-thread` | 115; 7.33 (8.08) | 441; 2.06 (2.36) | 437; 2.08 (2.37) | +1% | 0.21 |
| `fox-marth-fd` | `MELEE_GPU_WORKERS=1` | 124; 6.87 (7.93) | 486; 1.90 (2.37) | 526; 1.75 (2.28) | -8% | 0.15 |
| `fox-marth-fd` | `MELEE_GPU_WORKERS=1`, two cores | 120; 6.94 (7.77) | 441; 2.11 (2.50) | 492; 1.91 (2.30) | -9% | 0.12 |
| `fox-marth-ps` | `--one-thread` | 89; 9.27 (10.36) | 297; 3.17 (3.70) | 282; 3.28 (3.83) | +3% | 0.27 |
| `fox-marth-ps` | `MELEE_GPU_WORKERS=1` | 119; 7.69 (8.51) | 346; 2.67 (3.68) | 405; 2.37 (3.18) | -11% | 0.10 |
| `fox-marth-ps` | `MELEE_GPU_WORKERS=1`, two cores | 132; 7.52 (8.56) | 286; 3.10 (3.81) | 351; 2.61 (3.12) | -16% | 0.24 |
| `ics-peach-ps` | `--one-thread` | 113; 8.74 (9.62) | 281; 3.31 (4.03) | 269; 3.46 (4.19) | +5% | 0.25 |
| `ics-peach-ps` | `MELEE_GPU_WORKERS=1` | 129; 7.38 (8.08) | 350; 2.69 (4.03) | 412; 2.32 (3.42) | -14% | 0.10 |
| `ics-peach-ps` | `MELEE_GPU_WORKERS=1`, two cores | 118; 7.83 (8.73) | 273; 3.28 (4.13) | 348; 2.72 (3.44) | -17% | 0.15 |
| Fountain of Dreams | `--one-thread` | 46; 18.41 (19.74) | 204; 4.09 (4.89) | 201; 4.11 (4.99) | +0% | 0.86 |
| Fountain of Dreams | `MELEE_GPU_WORKERS=1` | 61; 15.14 (17.18) | 261; 3.50 (4.31) | 332; 2.82 (3.93) | -19% | 0.19 |
| Fountain of Dreams | `MELEE_GPU_WORKERS=1`, two cores | 60; 15.39 (16.46) | 218; 4.07 (4.85) | 259; 3.43 (3.96) | -16% | 0.43 |

**The player at 1440p** (`melee-hd --replay R --bench 1980 --size 2560x1440 --renderer exact|fast`, headless, internal 3x; two runs each, in the same reservation; a frame's ticks and drawing are the simulation, the GX stream and the renderer, and presenting adds the GPU's wait and the present):

| Replay | Renderer | A frame's ticks and drawing: p50, p99 (ms) | With presenting: p50, p99 | The renderer, a frame (mean) |
| --- | --- | --- | --- | --- |
| `fox-marth-ps` | exact | 5.77, 8.59 / 4.89, 7.74 | 9.69, 14.08 / 5.37, 9.18 | 3.69 / 3.11 (and the GX stream 0.86 / 0.76) |
| `fox-marth-ps` | fast | 3.71, 6.08 / 3.42, 5.66 | 4.61, 10.29 / 4.31, 10.43 | 4.92 / 2.43 (the stream within it) |
| Fountain of Dreams | exact | 8.86, 13.75 / 8.97, 14.30 | 9.53, 16.53 / 9.51, 17.11 | 6.70 / 6.81 (and the GX stream 1.03 / 1.03) |
| Fountain of Dreams | fast | 4.16, 7.24 / 4.21, 7.74 | 4.72, 11.21 / 4.75, 14.28 | 3.02 / 3.11 (the stream within it) |

The fast renderer's first run on `fox-marth-ps` spent 4.6 s on one frame (its worst; the driver compiling the barriered shaders the first time, which its shader cache keeps after), which is in that run's mean renderer time, not in its percentiles.

**Against the targets.** The player's 99th percentile at 1440p: the frame's ticks and drawing are under 8 ms with the fast renderer (5.7 to 7.7 ms; the exact renderer 7.7 to 14.3), the frame with presenting isn't (10.3 to 14.3 ms: the GPU's wait and the present, whose worst frames F5 doesn't touch). The CPU's frame under 1 ms on every stage: not met. F5 is 1.8 to 2.1 ms on Final Destination and Battlefield, 2.3 to 2.6 on Pokémon Stadium (both matches) and 2.7 to 3.0 on Fountain of Dreams threaded, 5 to 20% below F4 and 30 to 67% below the exact renderer. What's left:

### What's left toward 1 ms (after F5's first round)

From the worker's profile of the measured frames (`perf record -e task-clock -F 2000`, F5 on Fountain of Dreams at 1x threaded, the measured frames alone) and `GX_PROFILE`'s split:

- **The render thread is the critical path**: about 2.5 ms a frame (2.99 s on the CPU over 1,201 frames). Its flushes are about 1.3 ms of it (wgpu's encoding of about 550 calls and 1,150 state changes, the uploads, the transforms' pass and 7 submits; wgpu-core's trackers and binders and the driver, none of it a single hot spot), the draws' state 0.33, the rest the meshes' records, the state's changes and the packets. The flushes don't shrink with fewer submissions (above); they shrink with fewer calls or state changes, or move: encoding a flush on a thread of its own while the render thread builds the next takes them off the critical path (about 1.3 ms there), which every queue write and copy would then go through in order.
- **The game thread** builds and runs the stream beside it, about 1.7 ms a frame besides the simulation (3.2 ms with it): melee-gx's parsing (`State::run`, the game thread's largest function), HSD's matrices and materials, the state's changes for the threads (`take_delta_into`). Once the render thread is under it, this is the critical path, and records built straight from the `Draw` list (or, first, GX writing its registers straight into melee-gx's state with no bytes encoded and parsed) are what shorten it.
- **The meshes' records on the transform threads** (F4's optional part): not done. The profile has `GpuMeshes::draw` and `Batch::record` at under 2% of the render thread; the 0.6 ms `GX_PROFILE` calls the meshes' transforms includes the flushes they start.

So 1 ms needs both threads well under it: the render thread's flushes off its path and fewer calls, then the game thread's stream replaced. F5's second round.

The commands (the worker's scripts set `R` to the build and `bench R ...` to `melee-render-check bench "$SSBM_DISC" R.slp --warm-list warm-R.bin ...`, Fountain of Dreams with `CHECK_FP=slippi`):

```sh
# The warm lists, then two rounds, the renderers' order turned round in the second.
for r in fox-marth-fd fox-marth-bf fox-marth-ps ics-peach-ps fod; do
  for w in exact f4 fast; do bench $r --renderer $w; done
done
for s in 1 2 3 5; do
  for r in fox-marth-fd fox-marth-bf fox-marth-ps ics-peach-ps fod; do
    for w in exact f4 fast; do bench $r --scale $s --renderer $w; done
  done
done
# The low-end runs.
for w in exact f4 fast; do
  for r in fox-marth-fd fox-marth-ps ics-peach-ps fod; do
    bench $r --renderer $w --one-thread
    MELEE_GPU_WORKERS=1 bench $r --renderer $w
    MELEE_GPU_WORKERS=1 taskset -c 0,1 bench $r --renderer $w
  done
done
# The flush threshold.
for f in 160 320 640 100000; do MELEE_GPU_FLUSH_AT=$f GX_PROFILE=1 bench fod --renderer fast --scale 5; done
# The player at 1440p.
melee-hd --disc "$SSBM_DISC" --replay R.slp --bench 1980 --size 2560x1440 --renderer fast
```

**Indicative, on the busy PC** (2026-11-20, the machine at 100% with other agents' work; these decide nothing): Fountain of Dreams at 1x threaded, three alternating pairs of F4 and F5, 6.17 / 4.83, 4.77 / 4.71 and 5.63 / 4.98 ms; the player's `--bench 1200` on `fox-marth-fd` at 2x, p50 of a frame's ticks and drawing 2.39 and 2.59 ms fast against 3.74 exact.

## F5: the whole scene (second round)

The second round took F5's first round's leftovers in its handoff's order, measuring each step on the worker's exclusive runner before the next and keeping it only where it paid there. What it changed:

**The flushes encoded and submitted on a thread of their own** (`melee_gpu::submit::Ordered`). wgpu 30 records a pass's commands as they're made and does the work (validation, the resources' tracking and barriers, the driver's commands) when the encoder is finished and submitted: most of the render thread's 1.3 ms of flushes on Fountain of Dreams. The threaded renderer now sends each recorded encoder, and every write to a buffer, to a submission thread that makes them in the order sent; the render thread records the next flush meanwhile. Order is all that matters: a buffer written each flush (the vertices, indices, uniforms, records, XF sections) must take each write after the submission before it, which a write made on the render thread while an earlier flush waits wouldn't (`Queue::write_buffer` lands before the next submission, whichever thread makes it), so every write to a buffer goes through the thread too, the buffers handed over whole (about 1.4 MB a frame on Fountain of Dreams) and handed back to be built into again. A texture is written only as it's made, before anything reads it, so those stay on the render thread. Whatever reads the GPU back (readbacks, the harness's clips) and the frame's hand-over wait for everything sent (`Ordered::drain`). Checked by `submit::tests::each_submission_reads_the_write_before_it` (200 writes to one buffer, each read by the submission after it; making the writes on the caller's thread fails it) and the transforms' test, now through the thread. `GX_PROFILE` reports the thread's writes, finishing and submitting, and the frame's wait for it at its end. `MELEE_GPU_SUBMIT` 1 or 0 forces it on or off.

**A streamed frame's pieces run on a thread of their own** (`Frames::frame_piped`, `Gpu::run_streamed`). With the flushes off the render thread, the game thread was the critical path, and a profile of it on the worker (`perf` with frame pointers, Fountain of Dreams) put running the stream's pieces (melee-gx's parsing and registers, the threaded renderer's state changes, packets and texture hashing, the mesh cache's lookups) at about half its frame: 16% of the process's samples against 30% for the whole build. So the game thread now hands each piece to a stream thread that runs it while the next is built; the address space is shared meanwhile (`mem::Space`'s ranges and overlay behind locks; what a piece reads of it the build wrote before handing the piece on). Nothing is drawn differently: the same stream runs in the same order. The harness observes piped frames (its recorder is `Sync`), and `the_fast_renderer_threaded::*` runs every documented frame on the renderer's threads. `MELEE_RENDER_PIPE` 1 or 0 forces it.

**Both threads only where the processor runs four threads or more.** On two cores (`MELEE_GPU_WORKERS=1`, `taskset -c 0,1`, two rounds on the worker) they cost more than they saved: Fountain of Dreams ran 231 to 238 frames a second with both on, 254 to 255 with the stream thread off, and 261 to 271 with neither; the other replays alike (`fox-marth-ps` 332 to 349, 350 to 365 and 364 to 376). So each starts where `available_parallelism` is 4 or more (on Linux it counts the affinity mask).

**Not done, and why:**

- **Fewer uniform rebinds** (an index into a storage array of uniforms in place of a dynamic offset per call, in `gx.wgsl` and the specialized shaders, so both renderers). The rebinds are now encoded on the submission thread, off the frame's critical path: its whole encoding is about 0.8 ms a frame on Fountain of Dreams, and what reaches the frame is only the wait at its end for the last flush and the XFB copy (0.30 to 0.36 ms on the worker), of which the rebinds are a part. Not worth changing both renderers' shaders and their fragment cost for.
- **GX registers written straight into melee-gx's state.** Parsing is about a quarter of running the stream (`State::run`'s own time, 4% of the samples against the pieces' 16%); the stream thread took all of the running off the game thread instead, for every `Draw` kind at once.
- **Records from the `Draw` list in place of the stream**: still the way to shorten the stream thread, now the critical path (below).

**The bench's split** (`melee-render-check bench`): a streamed frame's "build" is now the game thread's time until it hands the frame over and "submit" the wait for the renderer's threads after it (their sum, the CPU frame, as before), with "running the stream" the stream's own time, beside the build.

### Speed on the worker (2026-11-20), the deciding numbers

`feat/fast-f5` at round 2's working tree (the stream thread and both threads' gates; the worker's snapshot of 17:13), release with line tables, the GTX 1080 Ti through Vulkan, one exclusive reservation after its quietness check (processor and GPU idle between scales). As in the first round: warm lists first, then frames 600 to 1800 threaded in two rounds (exact, F4, F5, then the order turned round), no measured call drawn with the uber shader. F4 here has the submission thread too (it's the threaded renderer's), so "F5 against F4" is now the stream thread's and the streamed frame's share; against the first round's F5 (above) the CPU frame at 1x is 14 to 23% lower (Fountain of Dreams 2.74 / 2.85 to 2.23 / 2.27 ms).

| Replay | Scale | Exact | F4 | F5 | F5 against F4 | GPU after, F5 |
| --- | --- | --- | --- | --- | --- | --- |
| `fox-marth-fd` | 1x | 278/277; 3.30 / 3.31 (3.99/4.03) | 511/506; 1.69 / 1.73 (1.98/2.21) | 596/580; 1.47 / 1.53 (1.91/2.06) | -12% | 0.20 |
| `fox-marth-bf` | 1x | 290/283; 3.28 / 3.32 (3.77/3.80) | 501/504; 1.81 / 1.80 (2.13/2.05) | 541/550; 1.70 / 1.67 (2.21/2.14) | -7% | 0.15 |
| `fox-marth-ps` | 1x | 256/262; 3.68 / 3.64 (4.53/4.38) | 415/408; 2.23 / 2.27 (2.62/2.84) | 519/515; 1.78 / 1.82 (2.36/2.50) | -20% | 0.14 |
| `ics-peach-ps` | 1x | 250/258; 3.68 / 3.67 (4.92/4.94) | 413/411; 2.24 / 2.23 (2.90/2.98) | 492/493; 1.89 / 1.87 (2.89/2.95) | -16% | 0.15 |
| Fountain of Dreams | 1x | 104/105; 8.30 / 8.32 (9.96/9.72) | 320/338; 2.71 / 2.68 (3.27/3.36) | 429/417; 2.23 / 2.27 (2.91/3.27) | -17% | 0.12 |
| `fox-marth-fd` | 2x | 270/268; 3.28 / 3.30 (4.19/4.35) | 419/423; 1.82 / 1.81 (2.36/2.36) | 462/469; 1.65 / 1.60 (2.44/2.41) | -10% | 0.52 |
| `fox-marth-bf` | 2x | 279/279; 3.19 / 3.19 (3.40/3.58) | 411/418; 1.90 / 1.87 (2.56/2.80) | 434/462; 1.76 / 1.68 (2.55/2.40) | -9% | 0.52 |
| `fox-marth-ps` | 2x | 248/241; 3.67 / 3.81 (4.66/5.12) | 338/331; 2.27 / 2.34 (3.15/3.11) | 402/401; 1.94 / 1.95 (3.13/3.23) | -16% | 0.55 |
| `ics-peach-ps` | 2x | 250/257; 3.64 / 3.49 (4.90/4.75) | 334/326; 2.36 / 2.40 (3.59/3.63) | 379/391; 2.12 / 2.01 (3.85/3.51) | -13% | 0.54 |
| Fountain of Dreams | 2x | 111/108; 7.61 / 7.72 (8.63/9.07) | 309/300; 2.76 / 2.74 (3.56/3.71) | 362/367; 2.40 / 2.31 (3.73/3.55) | -14% | 0.39 |
| `fox-marth-fd` | 3x | 230/243; 3.40 / 3.30 (4.44/4.23) | 356/366; 1.84 / 1.79 (2.58/2.28) | 383/383; 1.70 / 1.71 (2.49/2.51) | -6% | 0.91 |
| `fox-marth-bf` | 3x | 222/223; 3.42 / 3.38 (4.42/4.17) | 338/364; 2.06 / 1.86 (2.99/2.45) | 390/389; 1.71 / 1.72 (2.50/2.47) | -13% | 0.85 |
| `fox-marth-ps` | 3x | 244/236; 3.58 / 3.71 (4.64/4.74) | 299/294; 2.29 / 2.37 (3.34/3.56) | 329/329; 2.08 / 2.08 (3.46/3.57) | -11% | 0.95 |
| `ics-peach-ps` | 3x | 223/243; 3.72 / 3.38 (4.89/4.55) | 291/286; 2.39 / 2.47 (3.73/3.85) | 322/326; 2.16 / 2.11 (4.10/3.82) | -12% | 0.95 |
| Fountain of Dreams | 3x | 134/129; 7.13 / 7.00 (8.40/8.10) | 284/269; 2.72 / 2.94 (3.47/3.92) | 317/325; 2.46 / 2.35 (3.96/3.75) | -15% | 0.71 |
| `fox-marth-fd` | 5x | 183/181; 3.23 / 3.29 (3.75/3.96) | 219/219; 1.89 / 1.94 (2.80/2.89) | 235/238; 1.91 / 1.87 (2.69/2.71) | -1% | 2.33 |
| `fox-marth-bf` | 5x | 182/180; 3.32 / 3.36 (3.93/4.10) | 219/218; 1.92 / 1.93 (2.67/2.60) | 238/236; 1.93 / 1.98 (2.71/2.78) | +2% | 2.26 |
| `fox-marth-ps` | 5x | 161/163; 3.74 / 3.65 (4.83/4.69) | 165/166; 2.59 / 2.48 (3.86/3.63) | 180/180; 2.45 / 2.49 (4.21/4.24) | -3% | 3.08 |
| `ics-peach-ps` | 5x | 155/154; 3.61 / 3.62 (5.07/4.92) | 153/153; 2.54 / 2.57 (3.81/3.95) | 164/164; 2.70 / 2.73 (4.43/4.46) | +6% | 3.38 |
| Fountain of Dreams | 5x | 114/114; 7.10 / 7.10 (8.54/8.54) | 181/181; 2.94 / 2.97 (4.02/4.21) | 203/203; 2.67 / 2.68 (4.17/4.22) | -9% | 2.25 |

At 5x the GPU's wait after the submit (2.3 to 3.4 ms) is as long as the CPU's frame, and the CPU's frame gains little: the threads' work races the GPU's.

The step-by-step runs, F5 only, 1x, three rounds each, the order turned round each round: the submission thread on against off, Fountain of Dreams 2.64 to 2.85 ms against 2.90 to 2.92, `fox-marth-ps` 2.21 to 2.23 against 2.38 to 2.39, `fox-marth-fd` 1.78 to 1.81 against 1.76 to 1.81 (`GX_PROFILE` on Fountain of Dreams: the render thread's flushes 1.3 to 1.5 ms to 0.3; the submission thread's writes 0.36 to 0.58 ms, finishing 0.80, submitting 0.30; the frame's wait for it at its end 0.30 to 0.36); the stream thread on against off (both with the submission thread), Fountain of Dreams 2.27 to 2.39 against 2.67 to 2.73, `fox-marth-ps` 1.86 to 1.91 against 2.19 to 2.20, `fox-marth-fd` 1.58 to 1.61 against 1.72 to 1.74.

The low-end runs, one round (`--one-thread` draws on the game thread, as before; `MELEE_GPU_WORKERS=1` keeps every core, so both threads run; two cores start neither):

| Replay | Setting | Exact | F4 | F5 | F5 against F4 | GPU after, F5 |
| --- | --- | --- | --- | --- | --- | --- |
| `fox-marth-fd` | `--one-thread` | 142; 6.88 (7.52) | 449; 2.02 (2.28) | 451; 2.05 (2.33) | +1% | 0.17 |
| `fox-marth-fd` | `MELEE_GPU_WORKERS=1` | 152; 6.51 (6.98) | 516; 1.68 (1.99) | 586; 1.52 (2.02) | -10% | 0.19 |
| `fox-marth-ps` | `--one-thread` | 89; 9.30 (10.21) | 302; 3.14 (3.81) | 317; 3.07 (3.52) | -2% | 0.09 |
| `fox-marth-ps` | `MELEE_GPU_WORKERS=1` | 113; 7.87 (8.44) | 419; 2.19 (2.81) | 516; 1.84 (2.98) | -16% | 0.09 |
| `ics-peach-ps` | `--one-thread` | 115; 8.58 (9.41) | 273; 3.38 (4.02) | 280; 3.29 (3.97) | -3% | 0.28 |
| `ics-peach-ps` | `MELEE_GPU_WORKERS=1` | 124; 7.57 (8.23) | 395; 2.26 (3.59) | 479; 1.95 (3.30) | -14% | 0.14 |
| Fountain of Dreams | `--one-thread` | 47; 18.11 (19.11) | 199; 4.08 (4.85) | 201; 4.05 (4.82) | -1% | 0.91 |
| Fountain of Dreams | `MELEE_GPU_WORKERS=1` | 59; 15.56 (17.27) | 302; 2.89 (3.80) | 413; 2.29 (3.64) | -21% | 0.13 |

The two-core rows of that run were taken before the gates (both threads on): Fountain of Dreams 3.59 ms (F4) and 3.67 (F5), `fox-marth-ps` 2.87 and 2.90, `ics-peach-ps` 3.11 and 2.93, `fox-marth-fd` 1.95 and 1.98; with the gates (the two-core comparison above) F5 there runs with neither thread, as the first round's did.

**The player at 1440p** (as in the first round, `--bench 1980 --size 2560x1440`, two runs each): the fast renderer's ticks and drawing at p99 5.67 / 5.77 ms on `fox-marth-ps` and 6.34 / 6.78 on Fountain of Dreams (the first round's 5.66 to 7.74), with presenting 10.26 / 10.37 and 13.36 / 13.78; the exact renderer's 7.22 / 7.26 and 13.45 / 13.46, with presenting 8.68 / 9.32 and 16.86 / 17.05.

**The target**, Sarah's of 2026-11-20 in place of the 1440p placeholder: the player at 3840x2160 and 144 fps on the worker's GTX 1080 Ti, every displayed frame within 6.94 ms at the 99th percentile with the present and the GPU's wait, High refresh drawing the frames between ticks. Its measurement is [below](#the-player-at-4k-and-144-fps).

### The player at 4K and 144 fps

Sarah's target (2026-11-20): the player at 3840x2160 and 144 fps on the worker's GTX 1080 Ti, every displayed frame within 6.94 ms at the 99th percentile with the present and the GPU's wait, High refresh drawing the frames between ticks. Measured with the player's paced bench (`melee-hd --replay R --bench 1980 --size 3840x2160 --renderer fast|exact --high-refresh interpolate|extrapolate --frames-per-tick N`, headless: after each tick it draws N frames between ticks, each presented to an offscreen 4K target and waited for, so the GPU's time is in the frame's), internal 5x (an EFB of 2920 by 2400), on the worker's exclusive runner, two rounds (both shown), at 3 frames a tick (180 Hz, above 144's 2.4) and 2 (120 Hz). A frame's drawing is the CPU's (the fast renderer's stream and its threads; the exact renderer's stream, then the renderer); presenting adds the GPU's wait and the present. The tick (the simulation, and keeping its transforms or predicting the next) comes between frames, outside both.

| Replay | Mode | Renderer, frames a tick | A frame's drawing: p50, p99 (ms) | With presenting: p50, p99 | Mean a frame: stream, renderer, presenting | A tick: simulation, keeping or predicting |
| --- | --- | --- | --- | --- | --- | --- |
| Fountain of Dreams | Interpolate | fast, 3 | 2.49 / 2.48, 4.84 / 4.95 | 4.89 / 4.92, 22.3 / 22.3 | 0, 2.75, 2.53 | 1.47, 0.04 |
| Fountain of Dreams | Interpolate | fast, 2 | 2.42 / 2.41, 4.91 / 5.11 | 4.84 / 4.86, 26.1 / 26.3 | 0, 2.71, 2.66 | 1.43, 0.04 |
| Fountain of Dreams | Interpolate | exact, 3 | 7.64 / 7.67, 11.5 / 11.7 | 9.50 / 9.53, 18.2 / 17.2 | 1.45, 6.36, 1.97 | 1.60, 0.05 |
| Fountain of Dreams | Interpolate | exact, 2 | 7.53 / 7.50, 12.1 / 11.9 | 9.37 / 9.24, 23.7 / 23.9 | 1.34, 6.39, 1.96 | 1.59, 0.05 |
| Fountain of Dreams | Extrapolate | fast, 3 | 2.53 / 2.67, 4.99 / 5.10 | 4.96 / 4.99, 22.2 / 19.7 | 0, 2.90, 2.48 | 1.41, 1.12 |
| Fountain of Dreams | Extrapolate | fast, 2 | 2.53 / 2.54, 5.19 / 5.08 | 4.92 / 4.93, 26.3 / 26.2 | 0, 2.85, 2.61 | 1.39, 1.07 |
| Fountain of Dreams | Extrapolate | exact, 3 | 7.63 / 7.64, 11.7 / 12.0 | 9.64 / 9.65, 16.8 / 17.4 | 1.42, 6.37, 2.09 | 1.53, 1.30 |
| Fountain of Dreams | Extrapolate | exact, 2 | 7.50 / 7.51, 12.2 / 11.1 | 9.46 / 9.53, 23.5 / 24.4 | 1.32, 6.39, 2.13 | 1.53, 1.29 |
| `fox-marth-ps` | Interpolate | fast, 3 | 2.25 / 2.22, 4.65 / 4.69 | 4.96 / 4.96, 19.5 / 19.7 | 0, 2.45, 3.00 | 1.41, 0.04 |
| `fox-marth-ps` | Interpolate | fast, 2 | 2.15 / 2.16, 4.64 / 4.67 | 4.93 / 4.91, 21.8 / 21.8 | 0, 2.39, 3.15 | 1.39, 0.04 |
| `fox-marth-ps` | Interpolate | exact, 3 | 3.89 / 3.86, 6.75 / 6.37 | 5.98 / 5.98, 11.5 / 11.6 | 1.10, 2.86, 2.34 | 1.43, 0.04 |
| `fox-marth-ps` | Interpolate | exact, 2 | 3.76 / 3.80, 6.20 / 6.29 | 5.97 / 5.99, 12.6 / 12.5 | 1.02, 2.86, 2.40 | 1.41, 0.04 |
| `fox-marth-ps` | Extrapolate | fast, 3 | 2.26 / 2.27, 4.72 / 4.76 | 4.98 / 4.98, 18.6 / 18.7 | 0, 2.48, 3.00 | 1.36, 0.98 |
| `fox-marth-ps` | Extrapolate | fast, 2 | 2.25 / 2.22, 4.69 / 4.67 | 5.03 / 5.02, 21.8 / 21.9 | 0, 2.47, 3.11 | 1.33, 0.94 |
| `fox-marth-ps` | Extrapolate | exact, 3 | 3.87 / 3.89, 6.27 / 6.33 | 6.00 / 6.01, 11.7 / 11.6 | 1.10, 2.87, 2.35 | 1.36, 1.07 |
| `fox-marth-ps` | Extrapolate | exact, 2 | 3.79 / 3.81, 6.85 / 6.67 | 5.95 / 6.00, 12.6 / 12.4 | 1.01, 2.92, 2.39 | 1.33, 1.01 |

**Against the target: not met, and the CPU isn't what misses it.** The fast renderer's CPU side fits: a frame's drawing is 2.2 to 2.7 ms at p50 and 4.6 to 5.2 at p99 on both matches, either mode, and with the GPU's wait and the present the median frame is 4.8 to 5.0 ms, inside 6.94. Its 99th percentile with presenting is 18.6 to 26.3 ms: the tail is all in the present step (the GPU's wait and the present; the drawing's own p99 is 5 ms), and it's worse at 2 frames a tick than at 3, as if one frame a tick (or a few ticks in a hundred) waits on the GPU far longer than the rest. The exact renderer's CPU side doesn't fit on Fountain of Dreams (drawing p50 7.5 to 7.7 ms) and its tail with presenting is 11.5 to 24.4 ms. The GPU's mean wait at 4K is 2.5 to 3.1 ms a frame with the fast renderer (2.0 to 2.4 with the exact one, whose longer CPU frame gives the GPU more time to finish before the wait). So on the 1080 Ti 4K144 is bound by the GPU side's tail, not by the CPU: what to find next is the frames whose present takes 15 to 25 ms (the GPU's passes timed per frame, `GpuTimer`, in this bench, which doesn't time them yet). Two further notes: the tick's own CPU time (the simulation, 1.3 to 1.5 ms, and Extrapolate's prediction, 0.9 to 1.1) lands in one frame's interval of each tick, which at 144 Hz holds a frame and a tick in 6.94 ms only if the tick overlaps the GPU's work (this bench runs them one after the other); and this headless bench presents to an offscreen target, not a 144 Hz swap chain.

**Indicative, on this PC** (the RTX 5070 Ti, other agents' builds running; these decide nothing): the same runs, one round: the fast renderer's frame with presenting p50 2.8 to 4.1 ms, its GPU wait 0.4 to 1.6 ms a frame; the exact renderer's p50 3.5 to 8.2 ms. On that card the GPU's time at 4K is small; the busy machine's p99s (8 to 18 ms) say nothing.

### What's left toward 1 ms (after F5's second round)

First, for Sarah's target, the 4K frames whose present takes 15 to 25 ms on the worker ([above](#the-player-at-4k-and-144-fps)): the GPU's passes timed per frame in the paced bench, then whatever those frames hold. The CPU frame below matters to the target only after that.

On Fountain of Dreams at 1x (2.23 to 2.27 ms on the worker) the game thread hands the frame over at about 1.75 ms and the renderer's threads finish 0.5 ms after. The stream thread is now the longest: running the stream is 1.55 ms a frame there (`fox-marth-ps` 1.29, `fox-marth-fd` 0.74), more than the game thread's building of it, so the game thread ends waiting for it. What it runs is the threaded renderer's per-draw work as much as melee-gx's (the frame-pointer profile: the threaded renderer's draws 6% of the samples, its state changes 3.5% and the wake-ups of its threads 2.5%, beside `State::run`'s 4%), and records built from the `Draw` list are what remove it. Then the frame's end: the render thread's last packets, and the wait for the submission thread's last flush and XFB copy (0.3 ms).

The commands (the worker's scripts, as in the first round, in `fast-f5/w2`):

```sh
# The step-by-step runs.
for x in 1 0; do MELEE_GPU_SUBMIT=$x GX_PROFILE=1 bench fod --renderer fast; done
for p in 1 0; do MELEE_RENDER_PIPE=$p GX_PROFILE=1 bench fod --renderer fast; done
# Two cores.
MELEE_GPU_WORKERS=1 taskset -c 0,1 bench fod --renderer fast
MELEE_GPU_WORKERS=1 MELEE_RENDER_PIPE=1 taskset -c 0,1 bench fod --renderer fast
MELEE_GPU_WORKERS=1 MELEE_GPU_SUBMIT=0 taskset -c 0,1 bench fod --renderer fast
# The grid, the low-end runs and the player at 1440p: the first round's commands.
```

**Indicative, on the busy PC** (2026-11-20, other agents' work running; these decide nothing): Fountain of Dreams at 1x with the submission thread, 4.19 to 4.86 ms against 4.31 to 5.43 without, the render thread's flushes 1.6 to 2.0 ms to 0.27 to 0.37; the stream thread there didn't show a gain (2.55 and 2.91 ms with it, 2.43 and 2.62 without), the busy machine starving its extra thread.

## F5: the whole scene (third round)

The third round took Sarah's target (2026-11-20): the player at 3840x2160 and 144 fps on the worker's GTX 1080 Ti, every displayed frame within 6.94 ms at the 99th percentile with the present and the GPU's wait, with Interpolate and Extrapolate. The second round had measured the fast renderer's 99th percentile with presenting at 18.6 to 26.3 ms there, all of the tail in the present step.

**Each paced frame timed** (melee-player's `bench_frames`). The paced bench (`--bench` with `--high-refresh interpolate|extrapolate`) now keeps every frame's drawing, the present's submission and the wait for the GPU, with its passes' times on the GPU (timestamp queries, `GpuTimer`, by stage: the scene, the EFB copies, the XFB copy, the present), and reports the GPU's time a frame by stage, the frames over 144 Hz's interval and their place between ticks, and the slowest twelve with their parts. `MELEE_BENCH_FRAMES=FILE` writes every frame as a CSV row.

**What made the tail: the uber shader at the match's start.** On the worker (exclusive, 2026-11-21) nearly every frame over 6.94 ms fell in the bench's first second, and their scene pass took 20 to 25 ms on the GPU against 1.7 ms after it: the draws were waiting for their specialized pipelines (686 in the saved list, 5.8 to 6.0 ms each on two threads) and drew with the uber shader, whose per-pixel work at an EFB of 2920 by 2400 takes the 1080 Ti about 13 times as long. From tick 60 on the same runs were at the target or close (Fountain of Dreams at 2 frames a tick: p99 with presenting 26.06 ms over the whole run, 6.81 from tick 60; `fox-marth-ps` 21.78 and 7.64). `MELEE_GPU_TIMING=0` changed nothing (the queries cost nothing measurable). The player had the same at every match's start: the handoff makes a fresh renderer for the match, which made the saved list again.

**Specialized pipelines shared across a device's renderers** (`melee_gpu::SharedSpecialized`, `Renderer::share_specialized`, `present::Gpu::specialized`). Every renderer the player makes on its GPU takes the specialized pipelines the others made, by what each is made for, its TEV configuration and its samples a pixel, and puts its own there: the menus' renderer makes the saved list's during the boot and the menus, and a match's renderer takes them as it asks (its `warm_specialized` takes the whole list at once). One renderer's pipeline draws with another's bind groups because wgpu dedups equal bind group layouts and every renderer's are the same. Checked by `tev_tests::a_renderer_draws_with_the_specialized_pipelines_another_made` (a second renderer on the device draws twelve random configurations with the first's pipelines, making none, bit for bit as the first drew them; mutations: the lookup skipped, and the first renderer not putting what it made in the store: both fail it). The exact renderer is unchanged: the harness and the frames tests make no shared store. The paced bench now waits for the saved list before the match as the menus would (`finish_specialized` on its first renderer: "686 made in 2,000 ms before the match" on the worker), so no measured draw used the uber shader.

**Frames in flight** (`MELEE_BENCH_IN_FLIGHT=1`). The paced bench waits for the GPU after each frame, so a frame's time is the CPU's frame plus the GPU's; the player's swap chain (`desired_maximum_frame_latency` 1) draws the next frame while the GPU finishes the last. With `MELEE_BENCH_IN_FLIGHT=1` each frame's present is left in flight and waited for after the next frame's present, so a frame's time (its drawing, present and that wait) is the time between frames, and its GPU times land on the frame they're of. Still offscreen: no display's refresh paces it.

### The player at 4K and 144 fps (third round)

On the worker's exclusive runner (2026-11-21 07:09 to 07:24), `feat/fast-f5` with both changes, release with line tables, the GTX 1080 Ti through Vulkan, internal 5x (EFB 2920 by 2400), 1980 ticks of each replay. Waiting for each frame: two rounds, both shown; in flight: one round. Milliseconds.

| Replay | Mode, frames a tick | A frame's drawing, p99 | Waiting for each frame: p50, p99 | In flight: p50, p99 | The GPU a frame: p50, p99 |
| --- | --- | --- | --- | --- | --- |
| Fountain of Dreams | Interpolate, 3 | 4.92 / 4.72 | 4.91 / 4.91, 6.86 / 6.80 | 3.51, 4.61 | 3.13, 3.64 |
| Fountain of Dreams | Interpolate, 2 | 4.74 / 4.97 | 4.84 / 4.90, 6.74 / 6.93 | 2.92, 4.55 | 3.13, 3.63 |
| Fountain of Dreams | Extrapolate, 3 | 4.82 / 4.99 | 4.92 / 4.95, 6.77 / 6.94 | 2.84, 4.59 | 3.14, 3.65 |
| Fountain of Dreams | Extrapolate, 2 | 4.94 / 4.78 | 4.95 / 4.92, 6.97 / 6.76 | 2.65, 4.54 | 3.15, 3.66 |
| `fox-marth-ps` | Interpolate, 3 | 4.55 / 4.51 | 4.91 / 4.88, 7.60 / 7.58 | 3.97, 5.69 | 3.58, 5.13 |
| `fox-marth-ps` | Interpolate, 2 | 4.57 / 4.58 | 4.88 / 4.84, 7.60 / 7.62 | 3.87, 5.66 | 3.58, 5.13 |
| `fox-marth-ps` | Extrapolate, 3 | 4.60 / 4.60 | 4.91 / 4.91, 7.62 / 7.68 | 3.88, 5.70 | 3.57, 5.13 |
| `fox-marth-ps` | Extrapolate, 2 | 4.64 / 4.62 | 4.92 / 4.90, 7.57 / 7.67 | 3.31, 5.70 | 3.58, 5.13 |
| Fountain of Dreams, exact | Interpolate, 2 | 12.00 | 9.33, 13.49 | 7.54, 12.11 | 3.22, 3.85 |
| `fox-marth-ps`, exact | Interpolate, 2 | 7.16 | 5.90, 8.99 | 3.73, 6.20 | 3.57, 5.11 |

The GPU's frame at 4K: the scene 1.7 ms (Fountain of Dreams) to 2.0 (`fox-marth-ps`, 2.8 at p99), the EFB copies 0.45 to 0.57 (1.4 at p99 on Pokémon Stadium), the XFB copy 0.75 and the present 0.23.

**Against the target: met by the fast renderer with a frame in flight, as the player presents, on both matches in both modes and at 2 and 3 frames a tick**: p99 4.5 to 4.6 ms on Fountain of Dreams and 5.7 on `fox-marth-ps`, against 6.94. Waiting for each frame (the CPU's and the GPU's times added) Fountain of Dreams sits at the line (6.74 to 6.97) and `fox-marth-ps` over it (7.57 to 7.68), where Pokémon Stadium's later transformations make the GPU's frame 4.3 to 5.1 ms. The exact renderer meets it on neither (Fountain of Dreams' CPU frame alone is 7.4 ms at p50). What's over 6.94 ms with a frame in flight is 0.05 to 0.4% of the frames: the first frame (31 to 41 ms) and, on Fountain of Dreams, a frame at the same few ticks each run (5, 10, 84, 233, 585, 714, 890, 1143, 1220, 1295 and a few more) whose drawing on the CPU takes 8 to 17 ms, each the first frame after its tick: what first appears there (meshes into the cache, textures decoded and made) rather than the GPU. Each is a single hitched frame, below the 99th percentile. This is offscreen, and the tick (the simulation's 1.2 to 1.4 ms and Extrapolate's prediction, 0.9 to 1.1) runs between frames outside these times; in flight it overlaps the GPU's work on the frame before. The player in its window, below, has both.

**The player in its window on the worker** (Sarah's go-ahead, 2026-11-21; `s3.sh`). The worker's display is a 1920 by 1080 panel at 60 Hz on DVI-D (its modes read from the compositor: nothing above 1080p, nothing above 75 Hz), so neither 4K nor 144 Hz can be shown there. What it can show: the real player (`--replay R --ticks 1980 --perf-report FILE`, the run's ticks and frames and the swap chain's present, the perf report's frame time being the time between presents from tick 180 on) fullscreen at 1920 by 1080 with internal 5x, the same EFB of 2920 by 2400 and so the same scene, copies and XFB copy on the GPU as 4K; only the present's blit is smaller (0.09 ms against 0.23 at 4K). The driver offers no immediate present mode under GNOME's Wayland session, so `--present immediate` falls back to Mailbox, which doesn't wait for the panel. A window asked for at 3840 by 2160 is made 1853 by 1016 by the compositor. One run each (two for the fast renderer at 144), 2026-11-21 08:07 to 08:16, the same `release` build as the offscreen runs (line tables only added; not `dist`'s LTO build, as design call 10 has it):

| Replay | Renderer, mode, frames for | Frames a second | Frame time (between presents): p50, p99, max | The GPU a frame: mean, p99 | The CPU's build a frame: p50, p99 |
| --- | --- | --- | --- | --- | --- |
| Fountain of Dreams | fast, Interpolate, uncapped (`--refresh 1000`) | 268.8 | 3.65, 5.94, 20.7 | 2.97, 3.49 | 2.54, 4.52 |
| `fox-marth-ps` | fast, Interpolate, uncapped | 230.8 | 4.07, 5.87, 20.8 | 3.73, 4.97 | 2.22, 3.84 |
| Fountain of Dreams | fast, Interpolate, 144 Hz (`--refresh 144`) | 135.6 / 143.8 | 6.96 / 6.95, 23.6 / 9.79 | 3.39 / 2.98, 21.9 / 3.49 | 2.80 / 2.79, 5.49 / 5.28 |
| Fountain of Dreams | fast, Extrapolate, 144 Hz | 143.9 / 143.9 | 6.94 / 6.94, 10.71 / 10.43 | 2.98 / 2.98, 3.49 / 3.49 | 2.74 / 2.77, 5.09 / 5.20 |
| `fox-marth-ps` | fast, Interpolate, 144 Hz | 143.9 / 144.0 | 6.94 / 6.94, 9.37 / 9.30 | 3.81 / 3.80, 5.00 / 4.98 | 2.39 / 2.40, 4.55 / 4.49 |
| `fox-marth-ps` | fast, Extrapolate, 144 Hz | 144.0 / 144.0 | 6.94 / 6.95, 9.94 / 9.61 | 3.81 / 3.80, 5.00 / 4.99 | 2.36 / 2.38, 4.62 / 4.58 |
| Fountain of Dreams | exact, Interpolate, 144 Hz | 109.3 | 9.14, 14.00, 30.5 | 2.98, 3.48 | 7.57, 12.37 |
| `fox-marth-ps` | exact, Interpolate, 144 Hz | 144.0 | 6.95, 10.17, 12.8 | 3.80, 4.99 | 4.20, 7.49 |

**What the window says.** Uncapped, the fast renderer's frames come 3.7 to 4.1 ms apart at the median and 5.9 at the 99th percentile, present included, on a real swap chain with the tick between frames: every frame but a few in a thousand is made within 6.94 ms. Paced for 144 Hz, the fast renderer holds 143.8 to 144.0 frames a second on both matches in both modes (the first Interpolate run on Fountain of Dreams excepted: its pipelines still made slowly (17 ms each) after the session's cold first run, its GPU's 99th percentile 21.9 ms the uber shader's); the time between presents is then 6.94 ms at the median and 9.3 to 10.7 at the 99th percentile, but that spread is the pacing's, not missed frames: with no 144 Hz display the player paces by its own clock, so one present lands early and the next late around 6.94, where a 144 Hz panel's refresh would take each in its turn. **What a 60 Hz panel can't tell**: whether each frame makes its refresh on a real 144 Hz display (the vblank's timing against the frame's, the compositor's latency at 144 Hz, and Mailbox's or FIFO's behaviour there), and the present's cost at 4K (0.14 ms more on the GPU than at 1080p, by the offscreen runs). The uncapped numbers say there's room: the slowest 1% of frames take 5.9 ms against 6.94's budget.

Two more things the window showed. The first run in the session made its specialized pipelines at 175 ms each (the driver's cache cold for it) and drew all 30 s with the uber shader, 47.5 frames a second; every run after made them in 6 ms. A replay in the player has no menus before it, so its renderer makes the saved list from the replay's start (the perf report's first 3 s skip it: 23,000 to 39,000 draw calls with the uber shader each run, all before tick 180). The exact renderer misses 144 on Fountain of Dreams in the window too (109 frames a second, its CPU's build 7.6 ms).

The commands (the worker's scripts in `fast-f5/w3`, `s1.sh`, `s2.sh` and `s3.sh`):

```sh
# Each frame's parts and GPU passes, as CSV, waiting for each frame; then with one in flight.
MELEE_BENCH_FRAMES=out.csv melee-hd --disc D --replay R --bench 1980 --size 3840x2160 \
    --renderer fast --high-refresh interpolate --frames-per-tick 2
MELEE_BENCH_IN_FLIGHT=1 MELEE_BENCH_FRAMES=out.csv melee-hd ...   # the same arguments
# The player in its window: fullscreen, internal 5x, paced for 144 Hz or uncapped.
melee-hd --disc D --replay R --ticks 1980 --mute --renderer fast --high-refresh interpolate \
    --present immediate --refresh 144 --fullscreen --scale 5 --perf-report out.txt   # --refresh 1000: uncapped
```

**Indicative, on this PC** (the RTX 5070 Ti, other agents' work running; these decide nothing): Fountain of Dreams at 2 frames a tick, the GPU a frame 0.86 ms at p50; there the slow frames were the busy machine's, on the CPU.

### What's left (after F5's third round)

- **A 144 Hz display**: the worker's is 1080p at 60 Hz, so whether each frame makes a 144 Hz refresh at 4K is still unmeasured; the uncapped window (p99 5.9 ms) and the offscreen runs at 4K in flight (p99 4.5 to 5.7) say it should.
- **A cold start**: the first run on a driver cache that doesn't hold the pipelines draws with the uber shader for its first 30 s and more (47.5 frames a second at internal 5x on the 1080 Ti); a replay's first seconds draw with it while the list is made.
- **The first frame after new content** (the 8 to 17 ms frames above): what the CPU does when a mesh or texture first appears, which a warm-up of the match's meshes and textures before it starts could take, as the specialized pipelines' list does for shaders.
- **The uber shader at 4K** (20 to 25 ms a frame on the 1080 Ti): a configuration the saved list lacks still draws with it until its pipeline is made, a frame or a few; a cheaper fallback, or making that pipeline at once for the first draw that wants it, would remove the hitch.
- **The CPU frame's 1 ms** (unchanged this round): records from the `Draw` list to shorten the stream thread, and the frame's end (0.3 ms), as the second round left them.
