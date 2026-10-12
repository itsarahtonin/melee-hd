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

`compare` lines the two streams up event by event and reports each draw whose raster or TEV registers, TEV colors, sampled textures (by content), transformed vertices (screen positions, lit colors, texture coordinates) or copies differ, then compares the images with ssbm-rs's render tolerance: a PSNR of at least 38 dB, and at most 0.25% of the pixels off by more than 32 levels. It exits with failure when a frame is out of tolerance.

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
