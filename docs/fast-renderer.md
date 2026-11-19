# The fast renderer

The fast renderer is a track of its own beside the refactor ([the refactor](refactor.md#the-fast-renderer)): a GPU-driven renderer for low-end PCs that becomes the default, with the exact renderer (melee-render's GX stream drawn by melee-gpu, [the player](player.md#rendering)) kept as the reference, the frames tests' oracle and an "accurate" option. It may differ from the exact renderer in vertex float maths, the rasterizer's 1/16 snap, depth precision and later enhancements; not in what's drawn, the order, materials, blending or effects (Sarah, 2026-10-19). Its own work needs no full verify: it is checked against the exact renderer (Sarah, 2026-11-14), by the harness below.

This document is F1, the comparison harness and the measurement setup, [F2](#f2-the-mesh-cache), the renderer's first part: the mesh cache, and [F3](#f3-transforms-skinning-and-lighting-on-the-gpu), the transforms, skinning and lighting on the GPU. F4 onward add the rest.

## The comparison harness

`melee-render-check fast` (`tools/render-check/src/fast/`) draws a frame two ways from the same state, by a renderer under test and by the exact renderer, and compares each draw's transformed positions and the image.

```sh
melee-render-check fast DISC REPLAY [--renderer exact|f2|fast] [--mutate NAME] [--scale N] [--threaded] [--out DIR] FRAME...
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

The fast renderer implements `FrameRenderer` and is the renderer under test with `--renderer fast` (as far as it goes: F3, [below](#f3-transforms-skinning-and-lighting-on-the-gpu)), and F2 alone with `--renderer f2`; `tests/fast.rs` runs the fast renderer over the same frames (`the_fast_renderer::*`) and expects every one within tolerance. `--threaded` runs the renderer under test on its own threads, as the player draws (its geometry is then the CPU's transforms of the meshes, the image the GPU's).

## Measuring speed

`melee-render-check bench` (`fast::bench`) draws a replay's frames with one renderer, headless, and measures each frame: the CPU's time building what it draws (the exact renderer's GX stream), submitting it (the stream run, the vertices transformed and the GPU's work submitted: on the renderer's own threads, as the player draws, until the frame's copy to the XFB returns), and the GPU's time finishing it after that. The simulation runs but isn't timed. Frames before `--from` are drawn but not measured (textures, pipelines and specialized shaders made); the frame stays on the GPU, with no readback.

```sh
melee-render-check bench DISC REPLAY [--renderer exact|f2|fast] [--scale N] [--from A] [--to B] [--one-thread] [--adapter NAME|fallback]
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

**The depth range's edge** ([above](#proving-it-the-exact-renderer-with-a-change)). Every product in the transforms goes through an operation the compiler can't see through (`o`, as gx.wgsl's `opaque`), so no multiply-add is fused, and every add and multiply the GPU makes is the CPU's IEEE one in the same order: the clip-space positions are the CPU's bit for bit, and what lies on the near or far plane stays exactly there. `gpu_mesh_tests` checks it (below); without the barrier in the position matrix NVIDIA's driver fuses, and positions move by ulps. The shadow cameras' erase quads themselves are the CPU's draws until F5; `tests/fast.rs`'s `the_fast_renderer_keeps_the_depth_range_s_edge` holds the fast renderer to no vertex on the other side of the edge from the exact renderer's on the tournament stages' frames, where the erases are, and proves itself on `--mutate edge`, which it fails. On a GPU or shader compiler that fuses anyway (Metal's fast maths, if wgpu turns it on there), the check is where it shows.

**The frames.** `tests/fast.rs`'s `the_fast_renderer::*`, the 106 frames of 38 matches (2026-11-18, `--test-threads=2`): every frame within tolerance. Every frame's geometry is identical to the exact renderer's (the harness reads the GPU's clip-space positions back, `Renderer::mesh_clips`, a dispatch of the same transforms per mesh draw); 74 images are identical, and 32 differ in a few pixels each (at most 0.005% of a frame over 2.3 ΔE₀₀, no block near its limit): lighting and texture coordinates go through square roots and divisions the GPU rounds otherwise. No frame needed an exception. `--threaded` (the renderer on its threads, as the player draws) was checked by hand on the Fox ditto's 600 and 11000, the Ice Climbers' 600 and 2000 and Fountain of Dreams' 600: the same. The exact renderer's own frames are unchanged: `gpu_transforms` is off for it, and the documented frames (`tests/documented.rs`, all 535 with `--ignored`) pass at `--test-threads=2`.

**The transforms, checked** (`gpu_mesh_tests::the_gpu_transforms_as_the_cpu_does`, melee-gpu, on the GPU; skipped without one): 1,500 of `mesh_tests`' random lists, transform states and carried normals (6,051 vertices; lines in 817 lists, points in 509). Each vertex's clip-space position is the CPU's bit for bit; the 43,023 output vertices compared (finite on the CPU) have their depth and w bit for bit, their positions within a 64th of a pixel or one snap step (all), colors within a level (all but 87, 0.2%) and texture coordinates within 1e-4 of their size (all but 34): random lights and states make the square roots and divisions' rounding count there. A line's caps turn by its slope, which float noise flips on a 45-degree line (168 corners, 0.4%; allowed 2%). Made to fail by: the position matrix's barrier taken out (clip positions differ, round 31), a line's corner on the wrong side (4,158 corners), a spot light's distance attenuation squared wrong (5,581 colors), and the texture scales swapped (310,516 coordinates).

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
