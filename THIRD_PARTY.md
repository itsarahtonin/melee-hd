# Third party provenance

melee-hd is offered under GPL-3.0-or-later; [LICENSE](LICENSE) contains that license. Files ported from or following other projects say so in their headers, and copied files keep their SPDX notices. The project license doesn't replace upstream terms or grant rights to Nintendo's and HAL's original code or data: the game's models, textures, sounds and music are read from the player's own disc at run time and are never in the repository.

## ssbm-rs

[ssbm-rs](https://github.com/itsarahtonin/ssbm-rs) (GPL-3.0-or-later, same author) is the reference the simulation is checked against, at its [`melee-hd-base`](https://github.com/itsarahtonin/ssbm-rs/releases/tag/melee-hd-base) tag. Its crates (`gekko-fp`, `ssbm-disc`, `ssbm-game`, `ssbm-rt`, `ssbm-slippi`, `ssbm-types`) are git dependencies of the checking tools only; `crates/melee-player/tests/runtime_deps.rs` keeps them out of the player. Code ported or copied from it at that tag:

- `crates/gekko-fp`: a copy of ssbm-rs's `gekko-fp`, changed for speed; `tests/upstream` holds the unchanged source it is compared with.
- `crates/melee-ax`, `crates/melee-gx`, `crates/melee-gpu`: ported from ssbm-rs's `ssbm-ax`, `ssbm-gx` and `ssbm-render`.
- `crates/melee-slippi/src/{replay,denylist}.rs`: from ssbm-rs's `ssbm-slippi`.
- `crates/melee-input/src/adapter.rs`: the controller calibration, from ssbm-rs's player.

## Melee decompilation

Names, structure, type layouts and the generated tables (`tools/structgen`, `tools/tables`) follow [doldecomp/melee](https://github.com/doldecomp/melee). The decompilation states no license for the original game code. The tables transcribed from the game's executable (`crates/melee-sim/src/game/ft/motion_tables.rs`, `kind_tables.rs`, `co/kind_callbacks.rs`, `it/tables_gen.rs`) hold function addresses, flags and file names, the kind of data the decompilation's C source carries.

## Dolphin

Floating-point behavior, the DSP's AX mixing and the GPU's formats and rendering follow or are ported from [Dolphin](https://github.com/dolphin-emu/dolphin) (GPL-2.0-or-later), mostly by way of ssbm-rs:

- `crates/gekko-fp`: the interpreter's float semantics, paired singles and the estimate tables (`src/lib.rs`, `src/paired.rs`, `src/estimate.rs`, and the same in `tests/upstream`); the test vectors in `tests/estimate.rs` come from Dolphin's UnitTests.
- `crates/melee-ax`: the AX microcode's DSP HLE and the DSP accelerator (`src/lib.rs`, `src/accelerator.rs`), the parameter block's layout (`src/pb.rs`) and the free DSP ROM's coefficient generator (`src/coefs.rs`).
- `crates/melee-gx`: command formats and register layouts (`src/lib.rs`, `src/tev.rs`), texture and vertex formats (`src/texture.rs`, `src/vertex.rs`), the FIFO log format (`src/dff.rs`) and the software renderer's transform unit (`src/xform.rs`).
- `crates/melee-gpu`: the software renderer's TEV, sampler, rasterizer, texture encoder and XFB encoding (`src/lib.rs`, `src/encode.rs`, `src/xfb.rs`, `src/gx.wgsl`, `src/xfb.wgsl`).

## Slippi

- [slippi-ssbm-asm](https://github.com/project-slippi/slippi-ssbm-asm) (GPL-3.0): the Slippi ruleset's codes, UCF 0.84 and the determinism fixes (`crates/melee-sim/src/game/ucf.rs`, `game/slippi.rs`), and the playback codes (`game/playback.rs`), are ported from these codes' machine code. `tools/bench` embeds its Boot to Playback Scene, Init Player Data, Init Stage Data and ExtendPlayerBlock codes (`src/matches.rs`, `src/gecko_words.rs`).
- [project-slippi/dolphin](https://github.com/project-slippi/dolphin) (GPL-2.0-or-later): the replay format follows its `SlippiGame`, and the playback denylist is generated from its injection lists at `41a7a3a1` (`crates/melee-slippi`); the playback codes are the set it ships.

## Aurora

[Aurora](https://github.com/encounter/aurora) (MIT) is a design reference for the renderer's pipeline cache (`crates/melee-player/src/pipelines.rs`). No code is taken from it.

## Cargo dependencies

`Cargo.lock` records the crates and versions built; their licenses apply to them. All are compatible with GPL-3.0: most are MIT, Apache-2.0 or both, and the rest are permissive (Zlib, ISC, BSD-2-Clause, BSD-3-Clause, BSL-1.0, Unicode-3.0, Unlicense alternatives) or offer one of those alongside a copyleft choice. The player bundles egui's default fonts (`epaint_default_fonts`): Hack (MIT, with the Bitstream Vera License for the Vera glyphs it builds on), Noto Emoji (OFL-1.1), Ubuntu Light (Ubuntu Font Licence 1.0) and emoji-icon-font (MIT). A binary release must ship these crates' license notices beside the GPL text (`cargo about` or `cargo deny` can list and check them).
