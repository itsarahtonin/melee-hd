//! Cameras that render into the EFB's top left corner and copy it into a texture with the copy's
//! clear bit (Fountain of Dreams' reflection, `grIzumi_801CCEA0`; Pokémon Stadium's text screen,
//! `fn_801D2ED0`) leave nothing in the frame: the copy clears the corner for the screen's pass,
//! at the console's size and at a scaled EFB's. Before melee-render sent those copies, the
//! reflection's 80 by 60 render stayed in the corner and showed through the screen's pass.
//!
//! Each case draws a red quad through a camera onto the corner, then the copy, then the frame's
//! copy to the XFB, and reads the frame back. Skips without a GPU adapter, or on a software one
//! (`melee_gpu::testing`).

use melee_data::archive::{ArchiveId, Archives, DataRef};
use melee_render::frame::Frames;
use melee_render::gpu::Gpu;
use melee_sim::game::gr::{izumi, stadium};
use melee_sim::game::state::Game;
use melee_sim::hsd::cobj::{self, CObjId};
use melee_sim::hsd::display::{CameraView, Draw, Quad};
use melee_sim::mtx::Mtx34;

const RED: [u8; 4] = [0xFF, 0, 0, 0xFF];

/// A camera onto the corner `w` by `h` console pixels: orthographic, its view's middle at the
/// view space's origin, where the quads are.
fn corner_camera(w: u16, h: u16) -> CameraView {
    CameraView {
        viewport: [0.0, f32::from(w), 0.0, f32::from(h)],
        scissor: [0, w, 0, h],
        projection_type: cobj::ORTHO,
        projection: [1.0, -1.0, -1.0, 1.0],
        near: -1.0,
        far: 1.0,
        view_mtx: Mtx34::IDENTITY,
    }
}

/// A match's state with Fountain of Dreams' reflection image and Pokémon Stadium's text screen,
/// as the stages make them (`grIzumi_801CCD98`, `grStadium_801D2BEC`).
fn game() -> Game {
    let mut game = Game::default();
    game.gr.izumi.reflection = Some(izumi::Reflection {
        image: Some(DataRef {
            archive: ArchiveId(0),
            offset: 0,
        }),
        desc: stadium::screen_image(80, 60, 4),
        ..Default::default()
    });
    game.gr.stadium.screens[stadium::SCREEN_TEXT as usize] = Some(stadium::Screen {
        desc: stadium::screen_image(250, 160, 4),
        ..Default::default()
    });
    game
}

/// The last of two frames, each a red quad drawn onto the `w` by `h` corner and then `copy`,
/// drawn by a renderer `scale` times the console's tall and `width` wide: its size and RGBA.
fn frame(w: u16, h: u16, copy: &Draw, scale: u32, width: u32) -> Option<(u32, u32, Vec<u8>)> {
    // None without a GPU or on a software adapter, where these checks don't hold.
    let mut renderer = melee_gpu::testing::renderer()?;
    renderer.resize_efb(scale, width);
    renderer.xfb_on_gpu = true;
    renderer.xfb_readback = true;
    let mut gpu = Gpu::new(renderer);
    let mut game = game();
    game.objs.display.cameras = vec![corner_camera(w, h)];
    let draws = [
        Draw::Camera(CObjId(0), 0),
        Draw::Quad(Quad {
            x0: -2.0,
            y0: -2.0,
            x1: 2.0,
            y1: 2.0,
            color: RED,
        }),
        Draw::EndCamera,
        copy.clone(),
    ];
    let data = Archives::default();
    let mut frames = Frames::new();
    let mut last = None;
    // The second frame starts from the EFB the first's copy to the XFB cleared, as a match's.
    for _ in 0..2 {
        let fifo = frames.frame(&game, &data, &draws);
        last = gpu.run(&frames.space, &data, &fifo, None).pop();
    }
    let f = last.expect("a frame");
    Some((f.width, f.height, f.rgba))
}

/// How many pixels of the corner (`w` by `h` console pixels, at the frame's scale) are red, and
/// how many it has; with `inset`, its last line left out, which the copy to the XFB's vertical
/// filter blends with the black line below.
fn red_in_corner(
    w: u16,
    h: u16,
    copy: &Draw,
    scale: u32,
    width: u32,
    inset: bool,
) -> Option<(usize, usize)> {
    let (fw, fh, rgba) = frame(w, h, copy, scale, width)?;
    assert_eq!((fw, fh), (width, 480 * scale), "the frame's size");
    let lines = u32::from(h) - u32::from(inset);
    let (cw, ch) = (
        (u32::from(w) * width / 640) as usize,
        (lines * scale) as usize,
    );
    let mut red = 0;
    for y in 0..ch {
        for x in 0..cw {
            let p = &rgba[(y * fw as usize + x) * 4..][..3];
            if p[0] > 0xC0 && p[1] < 0x40 && p[2] < 0x40 {
                red += 1;
            }
        }
    }
    Some((red, cw * ch))
}

/// The scales checked: the console's 640 by 480, and 3x at the game's 73:60 (584 pixels for the
/// console's 640), as the player draws 1440p.
const SCALES: [(u32, u32); 2] = [(1, 640), (3, 3 * 584)];

#[test]
fn the_reflection_copy_clears_its_corner() {
    for (scale, width) in SCALES {
        let Some((red, of)) = red_in_corner(80, 60, &Draw::ReflectionCopy, scale, width, false)
        else {
            return;
        };
        assert_eq!(
            red, 0,
            "{red} of the reflection corner's {of} pixels kept its render at {scale}x"
        );
    }
}

#[test]
fn the_text_screen_copy_clears_its_corner() {
    let copy = Draw::ScreenCopy {
        screen: stadium::SCREEN_TEXT,
        x: 0,
        y: 0,
        clear: true,
    };
    for (scale, width) in SCALES {
        let Some((red, of)) = red_in_corner(250, 160, &copy, scale, width, false) else {
            return;
        };
        assert_eq!(
            red, 0,
            "{red} of the text screen corner's {of} pixels kept its render at {scale}x"
        );
    }
}

/// The control: a copy without the clear bit leaves the corner's render, so the quad does draw
/// there and the clear is what takes it away.
#[test]
fn a_copy_without_the_clear_bit_keeps_its_corner() {
    let copy = Draw::ScreenCopy {
        screen: stadium::SCREEN_TEXT,
        x: 0,
        y: 0,
        clear: false,
    };
    for (scale, width) in SCALES {
        let Some((red, of)) = red_in_corner(250, 160, &copy, scale, width, true) else {
            return;
        };
        assert_eq!(
            red, of,
            "only {red} of the corner's {of} pixels kept the render at {scale}x"
        );
    }
}
