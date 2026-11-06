//! A range of the address space that goes back once the space is full (`mem::Space`) and is
//! given to another archive draws the new archive's bytes, not what the renderers kept of the
//! old one by address: the hashes of textures in loaded archives, which they keep from frame
//! to frame, and the threaded renderer's copies of vertex arrays and textures.
//!
//! Frame 1 draws a sprite of archive A's red texture, its range at the space's start, and
//! gives a filler archive the rest of the space; frame 2 draws the same sprite from archive B's
//! green texture, whose range is A's, the space being full and neither drawn in that frame.
//! Skips without a GPU adapter, or on a software one (`melee_gpu::testing`).

use melee_data::archive::{Archive, ArchiveId, Archives, DataRef};
use melee_render::frame::Frames;
use melee_render::gpu::Gpu;
use melee_render::mem::ARCHIVES;
use melee_sim::game::state::Game;
use melee_sim::hsd::cobj::{self, CObjId};
use melee_sim::hsd::display::{CameraView, Draw};
use melee_sim::hsd::sobj::{Image, SObj, SObjId, Tex};
use melee_sim::mtx::Mtx34;

/// GX's RGB565 texture format.
const RGB565: u32 = 4;

/// A DAT archive whose data is a pointer to a 4 by 4 RGB565 texture of one color.
fn texture_archive(color: u16) -> Archive {
    let mut data = vec![0u8; 0x40];
    data[..4].copy_from_slice(&0x20u32.to_be_bytes());
    for t in data[0x20..].as_chunks_mut::<2>().0 {
        t.copy_from_slice(&color.to_be_bytes());
    }
    let mut file = vec![0u8; 0x20];
    file.extend_from_slice(&data);
    // The relocation table: the pointer at 0.
    file.extend_from_slice(&0u32.to_be_bytes());
    let len = file.len() as u32;
    file[..4].copy_from_slice(&len.to_be_bytes());
    file[4..8].copy_from_slice(&(data.len() as u32).to_be_bytes());
    file[8..12].copy_from_slice(&1u32.to_be_bytes());
    Archive::parse(&file).expect("the texture's archive")
}

/// The whole screen as an orthographic camera from -1 to 1 each way.
fn camera() -> CameraView {
    CameraView {
        viewport: [0.0, 640.0, 0.0, 480.0],
        scissor: [0, 640, 0, 480],
        projection_type: cobj::ORTHO,
        projection: [1.0, -1.0, -1.0, 1.0],
        near: -1.0,
        far: 1.0,
        view_mtx: Mtx34::IDENTITY,
    }
}

/// A sprite of the texture `image` points at, centered and covering the camera's view.
fn sprite(image: DataRef) -> SObj {
    SObj {
        x: 0.0,
        y: 0.0,
        scale_x: 1.0,
        scale_y: 1.0,
        s1: 1.0,
        t1: 1.0,
        width: 4,
        height: 4,
        color0: [255; 4],
        color1: [255; 4],
        flags: 2,
        image: Image::Data(image),
        tex: Tex {
            width: 4,
            height: 4,
            format: RGB565,
            ..Default::default()
        },
        ..Default::default()
    }
}

/// The middle pixel of each of the two frames, drawing A's sprite then B's, by a renderer on
/// the game's thread or on threads of its own.
fn middles(threaded: bool) -> Option<[[u8; 4]; 2]> {
    let mut renderer = melee_gpu::testing::renderer()?;
    renderer.xfb_on_gpu = true;
    renderer.xfb_readback = true;
    let mut gpu = if threaded {
        Gpu::threaded(renderer)
    } else {
        Gpu::new(renderer)
    };
    let mut data = Archives::default();
    let a = data.add(texture_archive(0xF800));
    // The rest of the space after A's range (0x40 bytes and the 0x20 between ranges), so that
    // the next range would end past it.
    let filler = data.add(Archive::raw(&vec![
        0;
        (ARCHIVES.end - ARCHIVES.start) as usize
            - 0x80
    ]));
    let b = data.add(texture_archive(0x07E0));
    let mut game = Game::default();
    game.objs.display.cameras = vec![camera()];
    let mut frames = Frames::new();
    let mut out = [[0; 4]; 2];
    for (i, archive) in [a, b].into_iter().enumerate() {
        let id = SObjId(
            game.objs
                .sobjs
                .alloc(sprite(DataRef { archive, offset: 0 })),
        );
        let draws = [
            Draw::Camera(CObjId(0), 0),
            Draw::SpriteSetup,
            Draw::SObj(id),
            Draw::SpriteEnd,
            Draw::EndCamera,
        ];
        let fifo = frames.frame(&game, &data, &draws);
        if i == 0 {
            // Frame 1 fills the space, which leaves B no room but A's once A goes back.
            frames.space.addr(
                &data,
                DataRef {
                    archive: filler,
                    offset: 0,
                },
            );
        }
        let f = gpu
            .run(&frames.space, &data, &fifo, None)
            .pop()
            .expect("a frame");
        let at = ((f.height / 2 * f.width + f.width / 2) * 4) as usize;
        out[i] = f.rgba[at..at + 4].try_into().unwrap();
        game.objs.sobjs.free(id.0);
    }
    // B took A's range: the space's start.
    let start = |x: ArchiveId| {
        frames.space.addr(
            &data,
            DataRef {
                archive: x,
                offset: 0,
            },
        )
    };
    assert_eq!(start(b), ARCHIVES.start, "B's range is A's");
    Some(out)
}

fn check(threaded: bool) {
    let Some([first, second]) = middles(threaded) else {
        return;
    };
    let red = |p: [u8; 4]| p[0] > 200 && p[1] < 60 && p[2] < 60;
    let green = |p: [u8; 4]| p[1] > 200 && p[0] < 60 && p[2] < 60;
    assert!(red(first), "A's sprite is red: {first:?}");
    assert!(
        green(second),
        "B's sprite at A's addresses is green: {second:?}"
    );
}

#[test]
fn a_reused_range_draws_the_new_archive() {
    check(false);
}

#[test]
fn a_reused_range_draws_the_new_archive_threaded() {
    check(true);
}
