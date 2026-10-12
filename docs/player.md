# The player

melee-hd's player presents the simulation: it draws the display pass's commands, mixes the audio driver's voices into sound, and reads the controllers into the simulation's input. The [architecture](architecture.md) draws the line: everything here is presentation, and nothing here feeds back into `melee-sim`.

## Crates

| Crate | What it does |
| --- | --- |
| `melee-render` | Turns a frame's `display::Draw` commands into a GX command stream (HSD's display code ported to issue GX SDK calls), and draws that stream with ssbm-rs's renderer |
| `melee-audio` | Mixes the voices the simulation keeps into samples, as the DSP's AX microcode does, and plays them |
| `melee-input` | The keyboard, standard gamepads and the GameCube controller adapter, as the simulation's pad input |
| `melee-player` | The `melee-hd` binary: a VS match in a window |
| `tools/render-check` | Compares melee-render's frames with ssbm-rs's renderer's |
| `tools/audio-check` | Compares melee-audio's mix of a replay with ssbm-rs's recording of its audio |

`melee-sim` depends on none of them.

## Rendering

The display pass names the objects it draws (a joint's display object's shape, a camera, a light set, a particle link) and leaves their state as it was when the pass drew them. `melee-render` ports what HSD would have sent GX for each: the camera's viewport, scissor and projection (`HSD_CObjSetCurrent`), the lights' GX objects (`HSD_LObjSetupInit`, `HSD_LObjSetup`), the material's colors, textures, texture coordinates, TEV stages and pixel-engine mode (`HSD_MObjSetup`, HAL's TEV expression compiler, `HSD_SetupRenderModeWithCustomPE`), and each shape's matrices, vertex descriptions and display list (`HSD_PObjDisp`). Those go through a port of the GX SDK (`melee_render::gx`), which encodes the same command stream the console's SDK would, register write for register write.

**Decision: the GX command stream is the interface to the GPU.** The stream is what ssbm-rs's renderer (`ssbm-render`) draws, and its frames match Dolphin's software renderer within tolerance. Encoding the stream and running it through `ssbm-gx` costs little next to drawing, and buys two things: the renderer is the one whose output is already checked, and each frame can be compared with ssbm-rs's draw by draw (`melee-render-check`), with every register, transformed vertex and texture of each draw, not only as an image. `ssbm-gx` and `ssbm-render` are used as dependencies at the `melee-hd-base` tag, as the oracle's crates are.

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

`compare` lines the two streams up event by event and reports each draw whose raster or TEV registers (TEV stages and colors included), the transform unit's registers (channel colors and controls, viewport, projection, texture coordinate generation), sampled textures and palettes (by content), transformed vertices (screen positions, lit colors, texture coordinates) or copies differ, then compares the images with ssbm-rs's render tolerance: a PSNR of at least 38 dB, and at most 0.25% of the pixels off by more than 32 levels. It exits with failure when a frame is out of tolerance. The log's frame count drifts from the native one over a match (about 3 frames early on, 13 by frame 14,000), so `compare` takes the native frame within 16 either way whose draws share the most exact vertices with the log's (`CHECK_SEARCH` changes the range). `CHECK_FP=slippi` plays a Slippi replay in Slippi Dolphin's float mode; `CHECK_EVENT=N` prints one event of each side in full; `CHECK_DRAW_LIMIT=N` draws only the first N draws of each side, to find the draw where two images part; `melee-render-check play LOG.dff OUT/` draws a log alone.

**Results.** The references are ssbm-rs runs (in ssbm-rs's `local/player/ref-*`) of three benchmark replays and a real Slippi replay, two consecutive frames from each of these log frames:

| Match | Frames | Result |
| --- | --- | --- |
| Fox vs Fox, Final Destination | 140, 300, 600, 1000, 2600, 5000, 8000, 11000, 14000 | Identical (PSNR infinite) |
| Fox vs Marth, Battlefield | 600, 2600, 6000, 11000 | Identical |
| Fox vs Marth, Final Destination | 600, 6000 | Identical |
| Fox vs Sheik and Zelda, Final Destination (Slippi replay) | 600, 1400, 6000 | Out of tolerance (PSNR 31.5 to 32.9 dB, 0.22% to 0.37% of the pixels off by 32): every differing pixel is in the two players' name plates, which Slippi's code draws and melee-sim doesn't (35 or 36 draws); the rest is identical |

The differences left in the draws themselves don't change a pixel: a few vertices' specular colors (the second color channel) one to four levels apart, palette bytes past the entries a texture uses, the texture cache's layout registers (compared with `CHECK_ALL_BP`) for texture slots a draw doesn't sample, and an extra draw of zero area off screen in some frames.

### Resolution, widescreen and pipelines

**Resolution scaling.** melee-gpu is ssbm-render with one addition: an EFB `scale` times the console's 640 by 480. Draws rasterize at the scaled size; EFB copies to textures (shadows, the magnifier's bubbles) average each block back to the console's size, so the game's textures stay what it expects, and the copy to the XFB keeps the scaled size for the window. The dither pattern scales with it. `--scale N` sets it; by default it's the smallest whose 480 lines cover the window's height (3 for 1440p). At scale 1 the output is ssbm-render's, which is what the frame checks compare.

**Decision: widescreen widens the drawn cameras, not the gameplay camera.** With `--widescreen` (16:9) or `--aspect W:H`, each camera whose viewport covers the whole screen sees `aspect / (4/3)` times as wide at the same height (a perspective camera's aspect ratio, an orthographic or frustum camera's left and right about their center), and the frame is shown at that aspect ratio. The game camera, which the simulation runs and which decides who is off screen, stays 4:3, as Slippi's widescreen code keeps it; only the rendered view sees more of the stage. The HUD's orthographic cameras widen the same way, so the HUD keeps its shape and its place in the 4:3 middle. Cameras that draw into textures (shadows, the magnifier's bubbles) are left alone. The render pass can't tell those apart by its offscreen flag, since the original leaves the frame in the shadows' offscreen pass, so the viewport decides.

**Decision: pipelines are kept by what they're made for, after Aurora's design.** ssbm-render runs the TEV in one shader, so pipelines differ only in blending, color and alpha writes and the depth test. The player records every pipeline key it has needed in `%LOCALAPPDATA%\melee-hd\pipelines.bin` (`$XDG_CACHE_HOME/melee-hd` elsewhere) and makes them all before the first frame of the next run, with the driver's own pipeline cache (wgpu's `PipelineCache`, where the backend has one) kept beside it. A frame never waits on a pipeline the player has made before.

## Audio

The simulation keeps the audio driver's voices, both copies of their parameter blocks, the sound banks' places in ARAM, the music stream and a DSP model that walks each voice through its samples (`docs/engine.md`). What it doesn't do is decode and mix the samples: that is `melee-audio`'s.

**The mix log** (`melee_sim::audio::mix`, `Audio::mix`) is the simulation's only addition, and it is presentation output: `None` unless the presentation turns it on, never read by simulated code, left alone by a rollback restore and empty in a clone. Each audio frame, right after `__AXSyncPBs`, where ssbm-rs's DSP stand-in runs the command list, it records a `MixFrame`: the DSP's 64 parameter blocks, each voice's sync flags, the voices whose depop the sync adds to the studio's sums (`__AXDepopVoice`, from a depop flag or a drop for the DSP's budget), the command list's mode and aux mixes as `__AXNextFrame` built it at the frame before, and the aux callbacks as `__AXProcessAux` finds them. Every write into ARAM is recorded as it completes (`AramWrite`, placed before the frame it precedes): a disc file's bytes from the device command queue (sound bank samples, music boxes), ARAM to ARAM (a bank's groups moved down), zeros, and each bank group again in whole when its load finishes. The presentation empties the log as it mixes; its vectors keep their storage, so recording allocates nothing once warmed up.

**The mixer** (`melee_audio::Mixer`) keeps a 16 MB ARAM image: the boot state's bank groups (each group's samples come from its `.ssm` file after the header: the file's first word plus 0x10, rounded up to 32 bytes), then the log's writes in order. Each frame it puts the frame's blocks, the studio (the depop sums faded as `__AXPrintStudio` does) and the command list in a small stand-in for main memory and runs ssbm-rs's AX microcode (`ssbm_ax`) over it. The words that depend on the samples' values (the ADPCM predictor and history, the resampler's last samples, the buses' last samples) come from the mixer's own last output for that voice unless the frame's sync flags say the CPU just wrote them (a new sound). The rest comes from the simulation, and the mixer counts any voice where the two disagree with no CPU write (`Stats::drift`; none so far). It then runs the aux effects, as the CPU's `__AXProcessAux` does, on the triple-buffered aux buses: the DSP sends a frame's aux mix, the CPU's effect processes it the next frame, and the DSP mixes the result back the frame after. Melee's boot sets up AXFX's standard reverb on aux A and its delay on aux B (`lbAudioAx_8002838C`), and a match uses both. Both are ported (`melee_audio::fx`): the reverb's float math follows the original's instructions through `gekko_fp`. Their state, delay lines included, the depop sums, the surround buffer and the aux buffers start from the boot state's memory. Each frame gives 160 stereo samples at 32 kHz, as the audio interface plays them.

**Output** (`melee_audio::Output`) plays them on the default device through cpal, in shared mode. The game loop pushes each tick's samples into a lock-free ring and never waits. The device's callback (about 10 ms buffers) resamples to the device's rate, nudges the rate by up to 0.5% to keep about 40 ms queued, drops the backlog past 160 ms, and after running dry waits for 40 ms again.

**Boot state.** Voices need the boot's audio state: a match started from its settings alone has no banks or timeline, because the boot's audio setup isn't ported in melee-sim. That is an open issue. Until it's ported, the player runs silent without a boot state. With `--boot-trace TRACE` it seeds the match with the boot state from an ssbm-rs state trace with memory over the boot tick, as the oracle does (`src/boot.rs` reuses `tools/oracle`'s trace reader and seeding), and plays sound. A replay's match also takes its fighters' addresses and the save data's unlocks from the trace, and a VS match from settings takes the unlocks.

### Checking the mix

ssbm-rs records what its audio interface plays (`AUDIO_OUT`) and the boot state in the same run:

```sh
SLIPPI_PLAYER_NAMES=0 STATE_TRACE=audio-fox-fox-fd.trc STATE_TRACE_MEMORY=0..600 AUDIO_OUT=audio-fox-fox-fd.wav \
  ssbm-run "$SSBM_DISC" --replay fox-fox-fd.slp --fp hardware --port all --fields 2400
melee-audio-check audio-fox-fox-fd.trc fox-fox-fd.slp audio-fox-fox-fd.wav --fp hardware --ticks 2500
```

`melee-audio-check` plays the replay natively from the trace's boot state, mixes it, and aligns the two streams: an exact match of 1024 samples if there is one, else cross-correlation. ssbm-rs's recording starts at power-on, and the native mix starts at the boot state, 4000 samples (25 frames) later. It then reports each window's SNR, largest difference and share of equal stereo samples, and the first sample that differs. On the Fox ditto on Final Destination (39.9 s compared) and Fox against Marth on Battlefield (59.9 s compared), the mix is sample-exact from the match's start to the recording's end, reverb and delay included: both effects processed sound in about 86% of the frames. The trace's game files that the fighters cache in ARAM (`PlFxAJ.dat`, `PlMsAJ.dat`) aren't sound and aren't in the device command queue's files, so the mixer skips them. On this machine mixing takes about 0.22 ms a tick.

`melee-audio-check --out MIX.wav` writes the native mix.

## Input

`melee_input::Inputs::read` returns the four ports' `PADStatus` just before each tick. GameCube controllers on a Wii U adapter (or a copy, such as Mayflash's in Wii U mode) take their own ports. The adapter is read raw over USB with nusb, as Dolphin's GCAdapter reads it, not through SDL, which rescales the axes; on Windows it needs the WinUSB driver (Zadig installs it). A thread keeps the latest report and sends rumble, and each read turns the report into what `PADRead` returns, with the origin calibration the console's SI and PAD library do (ported from ssbm-rs's player). Standard gamepads (gilrs) and then the keyboard take the first ports left. The keyboard: arrows for the stick, IJKL for the C-stick, X, Z, C and S for A, B, X and Y, D for Z, Q and W for L and R, Enter for start, and left shift to halve the stick. `--no-adapter` leaves the adapter alone.

## The player

```sh
melee-hd --p1 fox --p2 marth --stage bf [--stocks 4] [--minutes 8] [--widescreen | --aspect 21:9] [--scale N] [--size 1920x1440]
melee-hd --replay fox-fox-fd.slp                       # a replay's match, its inputs played back
melee-hd --boot-trace audio-fox-fox-fd.trc ...         # with sound (see Boot state)
melee-hd --bench 1200 --size 2560x1440                 # headless, timing each frame
```

Characters and stages are what melee-sim has ported (`--help` lists them). The match runs at 60 ticks a second. The window keeps the frame's aspect ratio, letterboxed when resized.

**Decision: the loop keeps one frame of latency.** Each tick reads the controllers immediately before it runs, and the frame it produces is turned into GX commands, drawn and presented at once; the swap chain is one frame deep and presents in mailbox mode where the display has it, else immediate (no vsync wait), else FIFO. The event loop sleeps until the next tick is due; after a stall of more than four ticks it drops the backlog instead of racing to catch up. Nothing waits on audio: the tick pushes its samples into the output's ring.

`--bench` plays a match headless, draws every frame and presents it to an offscreen target of `--size`, and reports the time per tick for the simulation, the GX stream, the renderer and presenting.

## Open issues

- **Boot audio.** The boot's audio setup isn't ported in melee-sim, so sound needs `--boot-trace` (see Boot state).
- **Name plates.** Slippi draws each player's display name under the damage counters; melee-sim doesn't run that code, so a Slippi replay's frames miss them. Every other draw of those frames matches.
- **Refraction images.** Draws that sample the refraction copies (`ImageRef::Refract`, `ImageRef::RefractFrame`) aren't drawn exactly yet.
- **Shape animations** aren't drawn; no checked frame uses one.
- **The particle pass's boundaries** are inferred from the display commands (runs of `Draw::Particles`), and the magnifier bubble's erase color is an average of the colors the original reads; both match every checked frame.
- **State cache resets** that stage, fighter and item draw callbacks make (`grDisplay`, `ftDrawCommon`, `itDraw`) are mirrored only where a checked frame needed them (the refraction callback's TEV reset).
- **Speed.** ssbm-render builds each frame's draws on the CPU; at 1440p (internal scale 3) a frame takes 11.5 to 13 ms on average on the development machine (`--bench 1800`: GX stream 0.8 ms, renderer 8 to 10 ms, presenting 2 ms; the simulation 0.7 ms a tick), and the worst about 21 ms, so a frame now and then misses its 16.7 ms tick.
- **No MSAA.**
- **Unchecked ports.** The port ledger counts a presentation port as checked when the frame or audio checks run it (measured with coverage over the references). Eighteen ported functions no reference reaches yet (refraction's material setup and indirect texture matrices, a few TEV register and alpha register helpers, GX getters) carry no ledger alias, only their names in their doc comments, until one does. The GX SDK functions Melee doesn't link (ssbm-rs's symbol table lacks them) are named the same way.
