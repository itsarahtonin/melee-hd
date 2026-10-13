# The player

melee-hd's player presents the simulation: it draws the display pass's commands, mixes the audio driver's voices into sound, and reads the controllers into the simulation's input. The [architecture](architecture.md) draws the line: everything here is presentation, and nothing here feeds back into `melee-sim`.

## Crates

| Crate | What it does |
| --- | --- |
| `melee-render` | Turns a frame's `display::Draw` commands into a GX command stream (HSD's display code ported to issue GX SDK calls), and draws that stream with melee-gpu |
| `melee-gx` | The GPU's command stream and register state: runs a stream, handing each draw and EFB copy to a renderer (ported from ssbm-rs's `ssbm-gx`) |
| `melee-gpu` | Draws the stream with wgpu (ported from ssbm-rs's renderer, `ssbm-render`) |
| `melee-audio` | Mixes the voices the simulation keeps into samples, as the DSP's AX microcode does, and plays them |
| `melee-ax` | The DSP's AX microcode (ported from ssbm-rs's `ssbm-ax`) |
| `melee-slippi` | Slippi replays (`.slp`) and what Slippi's EXI device answers playback's codes from them |
| `melee-input` | The keyboard, standard gamepads and the GameCube controller adapter, as the simulation's pad input |
| `melee-player` | The `melee-hd` binary: a VS match in a window |
| `tools/render-check` | Compares melee-render's frames with ssbm-rs's renderer's |
| `tools/audio-check` | Compares melee-audio's mix of a replay with ssbm-rs's recording of its audio |

`melee-sim` depends on none of them.

**Decision: no ssbm-rs code runs in the player.** ssbm-rs is the test oracle. The player's crates are melee-hd's own, ported from ssbm-rs where it has the piece (the GX stream decoder, the renderer, the AX microcode, the replay reader), as `crates/gekko-fp` is: idiomatic melee-hd code to change, not frozen copies. ssbm-rs's crates stay dependencies of the check tools, which compare against it. `crates/melee-player/tests/runtime_deps.rs` fails if `cargo tree` finds any ssbm-rs crate among the player's normal and build dependencies, and `leaf-check`'s `slippi` test parses every benchmark replay and a sample of Slippi replays with both melee-slippi and ssbm-rs's `ssbm_slippi` and compares the results.

## Rendering

The display pass names the objects it draws (a joint's display object's shape, a camera, a light set, a particle link) and leaves their state as it was when the pass drew them. `melee-render` ports what HSD would have sent GX for each: the camera's viewport, scissor and projection (`HSD_CObjSetCurrent`), the lights' GX objects (`HSD_LObjSetupInit`, `HSD_LObjSetup`), the material's colors, textures, texture coordinates, TEV stages and pixel-engine mode (`HSD_MObjSetup`, HAL's TEV expression compiler, `HSD_SetupRenderModeWithCustomPE`), and each shape's matrices, vertex descriptions and display list (`HSD_PObjDisp`). Those go through a port of the GX SDK (`melee_render::gx`), which encodes the same command stream the console's SDK would, register write for register write.

**Decision: the GX command stream is the interface to the GPU.** The stream is what melee-gpu (ssbm-rs's renderer, `ssbm-render`, ported) draws, and its frames match Dolphin's software renderer within tolerance. Encoding the stream and running it through `melee-gx` costs little next to drawing, and buys two things: the renderer is the one whose output is already checked, and each frame can be compared with ssbm-rs's draw by draw (`melee-render-check`), with every register, transformed vertex and texture of each draw, not only as an image.

**Decision: per-draw state the pass changes is snapshotted.** Most objects keep the same state through a frame's pass, so the renderer reads them as the pass left them. A few change between draws, and the simulation records what the presentation needs at the moment of the draw (in `display::Display`, so drawing allocates nothing once warmed up):

- a camera can draw more than once a frame, moved in between (the magnifier's, once per bubble): `Draw::Camera` names a `CameraView` with the viewport, scissor, projection and view matrix;
- the lights' GX slots are given by every `HSD_LObjSetupInit` (a light GObj's callback, a fighter's lights, a fighter's overlay light): `Draw::Lights` names a `LightSet` with the active table, the light masks and each light's specular slot and vector;
- materials read the GObj being drawn (`HSD_GObj_804D7814`: a fighter's color overlays and translucency, a stage object's or item's overlay): `Draw::Rendering` names it.

These are additions to `melee-sim`'s output only; no simulated value depends on them.

### Checking frames

ssbm-rs records a run's GPU command stream as a Dolphin FIFO log, and its `gxplay` draws the log with ssbm-render:

```sh
GX_DFF=f600.dff GX_DFF_FRAMES=600,2 ssbm-run "$SSBM_DISC" --replay fox-fox-fd.slp --fp hardware --port all --fields 640
gxplay f600.dff f600
```

`melee-render-check` plays the same replay natively, draws its frames through melee-render, and compares:

```sh
melee-render-check align "$SSBM_DISC" fox-fox-fd.slp f600.dff 560 600      # which native frame the log starts at
melee-render-check compare "$SSBM_DISC" fox-fox-fd.slp f600.dff f600 FRAME out/
```

`compare` lines the two streams up event by event and reports each draw whose raster or TEV registers (TEV stages and colors included), the transform unit's registers (channel colors and controls, viewport, projection, texture coordinate generation), sampled textures and palettes (by content), transformed vertices (screen positions, lit colors, texture coordinates) or copies differ, then compares the images with ssbm-rs's render tolerance: a PSNR of at least 38 dB, and at most 0.25% of the pixels off by more than 32 levels. It exits with failure when a frame is out of tolerance. The native match runs on the timeline the boot's audio setup starts (see [The boot](#audio)), so its scene loop renders on the same rounds as ssbm-rs's, and a log's frame N is native frame N - 3 from the first checked frame to the last (without the timeline, rounds that tick twice and render once came at other times, and the two drifted apart by 13 frames over a match). `compare` takes the native frame within 16 either way whose draws share the most exact vertices with the log's (`CHECK_SEARCH` changes the range). `CHECK_FP=slippi` plays a Slippi replay in Slippi Dolphin's float mode; `CHECK_EVENT=N` prints one event of each side in full; `CHECK_DRAW_LIMIT=N` draws only the first N draws of each side, to find the draw where two images part; `melee-render-check play LOG.dff OUT/` draws a log alone.

**Results.** The references are ssbm-rs runs (in ssbm-rs's `local/player/ref-*`) of three benchmark replays and a real Slippi replay, two consecutive frames from each of these log frames:

| Match | Frames | Result |
| --- | --- | --- |
| Fox vs Fox, Final Destination | 140, 300, 600, 1000, 2600, 5000, 8000, 11000, 14000 | Identical (PSNR infinite) |
| Fox vs Marth, Battlefield | 600, 2600, 6000, 11000 | Identical |
| Fox vs Marth, Final Destination | 600, 6000 | Identical |
| Fox vs Sheik and Zelda, Final Destination (Slippi replay) | 600, 1400, 6000 | Out of tolerance (PSNR 31.5 to 32.9 dB, 0.22% to 0.56% of the pixels off by 32): every differing pixel is in the two players' name plates, which Slippi's code draws and melee-sim doesn't (35 or 36 draws); the rest is identical |

The differences left in the draws themselves don't change a pixel: a few vertices' specular colors (the second color channel) one to four levels apart, palette bytes past the entries a texture uses, the texture cache's layout registers (compared with `CHECK_ALL_BP`) for texture slots a draw doesn't sample, and an extra draw of zero area off screen in some frames.

### Resolution, widescreen and pipelines

**Resolution scaling.** melee-gpu is ssbm-render ported, with one addition: an EFB `scale` times the console's 640 by 480. Draws rasterize at the scaled size; EFB copies to textures (shadows, the magnifier's bubbles) average each block back to the console's size, so the game's textures stay what it expects, and the copy to the XFB keeps the scaled size for the window. The dither pattern scales with it. `--scale N` sets it; by default it's the smallest whose 480 lines cover the window's height (3 for 1440p). At scale 1 the output is ssbm-render's, which is what the frame checks compare.

**MSAA.** `--msaa 4` draws with four samples a pixel: the draws go to a multisampled EFB (color and depth), resolved at the end of each pass into the EFB that copies to textures, readbacks and copies to the XFB read, so the game's textures and frames still come from one image. The TEV runs once a pixel, as before; triangles' edges are what it smooths. Off by default, where the output is unchanged. At 1440p it adds next to nothing to a frame (the GPU's work, inside presenting).

**Decision: widescreen widens the drawn cameras, not the gameplay camera.** With `--widescreen` (16:9) or `--aspect W:H`, each camera whose viewport covers the whole screen sees `aspect / (4/3)` times as wide at the same height (a perspective camera's aspect ratio, an orthographic or frustum camera's left and right about their center), and the frame is shown at that aspect ratio. The game camera, which the simulation runs and which decides who is off screen, stays 4:3, as Slippi's widescreen code keeps it; only the rendered view sees more of the stage. The HUD's orthographic cameras widen the same way, so the HUD keeps its shape and its place in the 4:3 middle. Cameras that draw into textures (shadows, the magnifier's bubbles) are left alone. The render pass can't tell those apart by its offscreen flag, since the original leaves the frame in the shadows' offscreen pass, so the viewport decides.

**Decision: pipelines are kept by what they're made for, after Aurora's design.** melee-gpu runs the TEV in one shader, so pipelines differ only in blending, color and alpha writes and the depth test. The player records every pipeline key it has needed in `%LOCALAPPDATA%\melee-hd\pipelines.bin` (`$XDG_CACHE_HOME/melee-hd` elsewhere) and makes them all before the first frame of the next run, with the driver's own pipeline cache (wgpu's `PipelineCache`, where the backend has one) kept beside it. A frame never waits on a pipeline the player has made before.

## Audio

The simulation keeps the audio driver's voices, both copies of their parameter blocks, the sound banks' places in ARAM, the music stream and a DSP model that walks each voice through its samples (`docs/engine.md`). What it doesn't do is decode and mix the samples: that is `melee-audio`'s.

**The mix log** (`melee_sim::audio::mix`, `Audio::mix`) is the simulation's only addition, and it is presentation output: `None` unless the presentation turns it on, never read by simulated code, left alone by a rollback restore and empty in a clone. Each audio frame, right after `__AXSyncPBs`, where ssbm-rs's DSP stand-in runs the command list, it records a `MixFrame`: the DSP's 64 parameter blocks, each voice's sync flags, the voices whose depop the sync adds to the studio's sums (`__AXDepopVoice`, from a depop flag or a drop for the DSP's budget), the command list's mode and aux mixes as `__AXNextFrame` built it at the frame before, and the aux callbacks as `__AXProcessAux` finds them. Every write into ARAM is recorded as it completes (`AramWrite`, placed before the frame it precedes): a disc file's bytes from the device command queue (sound bank samples, music boxes), ARAM to ARAM (a bank's groups moved down), zeros, and each bank group again in whole when its load finishes. The presentation empties the log as it mixes; its vectors keep their storage, so recording allocates nothing once warmed up.

**The mixer** (`melee_audio::Mixer`) keeps a 16 MB ARAM image: the boot state's bank groups (each group's samples come from its `.ssm` file after the header: the file's first word plus 0x10, rounded up to 32 bytes), then the log's writes in order. Each frame it puts the frame's blocks, the studio (the depop sums faded as `__AXPrintStudio` does) and the command list in a small stand-in for main memory and runs the AX microcode (`melee_ax`, ported from ssbm-rs's `ssbm_ax`) over it. The words that depend on the samples' values (the ADPCM predictor and history, the resampler's last samples, the buses' last samples) come from the mixer's own last output for that voice unless the frame's sync flags say the CPU just wrote them (a new sound). The rest comes from the simulation, and the mixer counts any voice where the two disagree with no CPU write (`Stats::drift`; none so far). It then runs the aux effects, as the CPU's `__AXProcessAux` does, on the triple-buffered aux buses: the DSP sends a frame's aux mix, the CPU's effect processes it the next frame, and the DSP mixes the result back the frame after. Melee's boot sets up AXFX's standard reverb on aux A and its delay on aux B (`lbAudioAx_8002838C`), and a match uses both. Both are ported (`melee_audio::fx`): the reverb's float math follows the original's instructions through `gekko_fp`, and its comb filters' feedback comes from the game's own `powf`, as the original links it. The mixer makes them as that setup does (`AXFXReverbStdInit`, `AXFXDelayInit`, with the types the driver recorded), or, given a boot state's memory, takes their state, delay lines included, the depop sums, the surround buffer and the aux buffers from it. Each frame gives 160 stereo samples at 32 kHz, as the audio interface plays them.

**Output** (`melee_audio::Output`) plays them on the default device through cpal, in shared mode. The game loop pushes each tick's samples into a lock-free ring and never waits. The device's callback (about 10 ms buffers) resamples to the device's rate, nudges the rate by up to 0.5% to keep about 40 ms queued, drops the backlog past 160 ms, and after running dry waits for 40 ms again.

**The boot.** Voices need the boot's audio state: the sound bank, the banks every scene keeps, and the timeline the audio frames run on. melee-sim's match start runs the boot's audio setup (`game::boot`, [the engine notes](engine.md)), so a match from its settings or a replay plays sound from its first tick, with nothing from ssbm-rs. The player turns the mix log on before that setup runs (`Match::start_from`'s `boot`, which runs first), so the mixer sees the boot's own ARAM writes (the cleared silence, the banks' samples) and frames (silent ones, the effects running over them) and starts the effects new. A replay's start also plays the sound Slippi's playback scene does.

### Checking the mix

ssbm-rs records what its audio interface plays (`AUDIO_OUT`) and the boot state in the same run:

```sh
SLIPPI_PLAYER_NAMES=0 STATE_TRACE=audio-fox-fox-fd.trc STATE_TRACE_MEMORY=0..600 AUDIO_OUT=audio-fox-fox-fd.wav \
  ssbm-run "$SSBM_DISC" --replay fox-fox-fd.slp --fp hardware --port all --fields 2400
melee-audio-check audio-fox-fox-fd.trc fox-fox-fd.slp audio-fox-fox-fd.wav --fp hardware --ticks 2500
```

`melee-audio-check` plays the replay natively from the trace's boot state (`tools/audio-check/src/boot.rs` reuses `tools/oracle`'s trace reader and seeding), mixes it, and aligns the two streams: an exact match of 1024 samples if there is one, else cross-correlation. ssbm-rs's recording starts at power-on, and the native mix starts at the boot state, 4000 samples (25 frames) later. It then reports each window's SNR, largest difference and share of equal stereo samples, and the first sample that differs. On the Fox ditto on Final Destination (39.9 s compared) and Fox against Marth on Battlefield (59.9 s compared), the mix is sample-exact from the match's start to the recording's end, reverb and delay included: both effects processed sound in about 86% of the frames. The trace's game files that the fighters cache in ARAM (`PlFxAJ.dat`, `PlMsAJ.dat`) aren't sound and aren't in the device command queue's files, so the mixer skips them. On this machine mixing takes about 0.22 ms a tick.

With `--port` (and `-` for the trace) `melee-audio-check` doesn't read the trace: the match starts as the player starts one, from melee-sim's boot, and the mix covers the boot's frames too. Against the same recordings it is sample-exact from the native mix's first sample (the AX library's first frame, which lines up with the recording's sample 320) to the end: 40.0 s of the Fox ditto and 60.0 s of Fox against Marth.

`melee-audio-check --out MIX.wav` writes the native mix.

## Input

`melee_input::Inputs::read` returns the four ports' `PADStatus` just before each tick. GameCube controllers on a Wii U adapter (or a copy, such as Mayflash's in Wii U mode) take their own ports. The adapter is read raw over USB with nusb, as Dolphin's GCAdapter reads it, not through SDL, which rescales the axes; on Windows it needs the WinUSB driver (Zadig installs it). A thread keeps the latest report and sends rumble, and each read turns the report into what `PADRead` returns, with the origin calibration the console's SI and PAD library do (ported from ssbm-rs's player). Standard gamepads (gilrs) and then the keyboard take the first ports left. The keyboard: arrows for the stick, IJKL for the C-stick, X, Z, C and S for A, B, X and Y, D for Z, Q and W for L and R, Enter for start, and left shift to halve the stick. `--no-adapter` leaves the adapter alone.

## The player

```sh
melee-hd --p1 fox --p2 marth --stage bf [--stocks 4] [--minutes 8] [--widescreen | --aspect 21:9] [--scale N] [--msaa 4] [--size 1920x1440]
melee-hd --replay fox-fox-fd.slp                       # a replay's match, its inputs played back
melee-hd --bench 1200 --size 2560x1440                 # headless, timing each frame
```

Characters and stages are what melee-sim has ported (`--help` lists them). The match runs at 60 ticks a second. The window keeps the frame's aspect ratio, letterboxed when resized.

**Decision: the loop keeps one frame of latency.** Each tick reads the controllers immediately before it runs, and the frame it produces is turned into GX commands, drawn and presented at once; the swap chain is one frame deep and presents in mailbox mode where the display has it, else immediate (no vsync wait), else FIFO. The event loop sleeps until the next tick is due; after a stall of more than four ticks it drops the backlog instead of racing to catch up. Nothing waits on audio: the tick pushes its samples into the output's ring.

`--bench` plays a match headless, draws every frame and presents it to an offscreen target of `--size`, and reports the time per tick for the simulation, the GX stream, the renderer and presenting, and the median, 99th percentile and worst of a frame's ticks and drawing (the simulation, the GX stream and the renderer), with and without presenting. `--one-thread` keeps the renderer on the game's thread.

### Frame time

**Decision: the renderer runs on threads of its own, a frame at a time.** The renderer's CPU work on a frame comes in three stages: running the GX stream (decoding commands and fetching each draw's vertices), decoding and transforming the vertices, and building the draws (the GPU's vertices and indices, textures, uniforms, pipelines) and handing them to wgpu. The game's thread runs the stream and sends each draw, with the state's changes since the last, to a transform thread, which sends the transformed draws on to a render thread (melee-gpu's `Threaded`, ssbm-render's design). The three work at once on the same frame, and the game's thread waits at the frame's copy to the XFB until the render thread has drawn it (`Threaded::spawn_synced`), so the frame is presented as soon as it's drawn, no frame later than before: the loop keeps its one frame of latency, and the frame costs about what its slowest stage does rather than all three. Draws go over in batches of 128, small enough for the stages to overlap within a frame. `CHECK_THREADED=1` runs `melee-render-check` the same way.

The rest of the cuts change no output (every checked frame stays identical, at scale 1 and at 1440p, where a frame drawn by the old and the new renderer saves to the same PNG):

- Each indexed vertex array is looked up once a draw, over the stretch the draw's indices reach, and read in place where the address space holds it in one piece (`Memory::view`); display lists run where they lie instead of being copied out.
- A draw's vertices are transformed by a setup read once for the draw (`xform::Setup`: the projection, lights, channels and texture coordinate scales), and the binormals only when a texture coordinate reads them; a test checks it against the per-vertex steps on random states.
- A draw's vertices are written once and its triangles name them through an index buffer, instead of three vertices a triangle; the vertex shader reads each vertex's words from a storage buffer with only the texture coordinates its draw generates (most draws one), so a frame's vertex data went from about 6.4 MB (144 bytes for each of a triangle's three vertices) to 0.9 MB.
- Consecutive draws with the same state join into one draw call, and a pass sets its pipeline, bind groups and scissor only when they change.
- The hashes of textures in loaded archives, which never change, are kept between frames, and a copy's footprint found unchanged is checked again only the next frame; the vertex format's layout is kept while its registers stay the same; the renderer's maps use a fast hash.

**Results** at 1440p (internal scale 3), `--bench 1800` on the Fox–Marth Battlefield replay, the old renderer and the new run alternately twice each, with other agents' builds and checks keeping about six of the machine's eight cores busy throughout (so both are slower than on an idle machine):

| | Ticks and drawing, median | 99th percentile | Renderer, mean | With presenting, 99th percentile |
| --- | --- | --- | --- | --- |
| Before | 11.8 ms | 14.4 to 15.2 ms | 9.8 ms | 17.0 to 17.7 ms |
| After | 5.0 ms | 6.8 ms | 3.5 ms | 9.2 to 9.4 ms |

The GX stream takes about 0.75 ms of that and the simulation about 0.7 ms a tick; presenting adds about 2.2 ms, most of it the GPU's own drawing at that scale. On the game's thread alone (`--one-thread`) the new renderer takes about 4.9 ms a frame (99th percentile 9.1 ms).

## Open issues

- **Name plates.** Slippi's playback draws each player's display name under the damage counters: its code set enables `Optional: Show Player Names` by default (a `C2` code at `0x8016E9B4`, after the VS scene's entry, `Playback/Show Player Names/InitInGame.asm`). The code makes a GObj of its own with a joint loaded from a HUD archive and a render callback, and the names' text, so it adds objects to the game's state; the oracle's traces leave it off (`SLIPPI_PLAYER_NAMES=0`) and melee-sim doesn't run it, so a Slippi replay's frames miss the plates. Every other draw of those frames matches. Drawing them without touching the simulation needs the presentation to draw HSD objects the simulation doesn't own (the plate's joint and its text), which melee-render can't yet: it draws only from the display pass's commands over the game's objects.
- **Refraction images.** Draws that sample the refraction copies (`ImageRef::Refract`, `ImageRef::RefractFrame`) aren't drawn exactly yet.
- **Shape animations** aren't drawn; no checked frame uses one.
- **The particle pass's boundaries** are inferred from the display commands (runs of `Draw::Particles`), and the magnifier bubble's erase color is an average of the colors the original reads; both match every checked frame.
- **State cache resets** that stage, fighter and item draw callbacks make (`grDisplay`, `ftDrawCommon`, `itDraw`) are mirrored only where a checked frame needed them (the refraction callback's TEV reset).
- **Speed.** At 1440p a frame's ticks and drawing take 6.8 ms at the 99th percentile on a busy machine (see Frame time); the GPU's drawing at high scales is the larger part of presenting, which the CPU work doesn't overlap.
- **Unchecked ports.** The port ledger counts a presentation port as checked when the frame or audio checks run it (measured with coverage over the references). Eighteen ported functions no reference reaches yet (refraction's material setup and indirect texture matrices, a few TEV register and alpha register helpers, GX getters) carry no ledger alias, only their names in their doc comments, until one does. The GX SDK functions Melee doesn't link (ssbm-rs's symbol table lacks them) are named the same way.
