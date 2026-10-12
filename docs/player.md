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
