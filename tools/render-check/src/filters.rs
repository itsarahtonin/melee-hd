// SPDX-License-Identifier: GPL-3.0-or-later

//! `melee-render-check filters`: native frames drawn with each of the player's Texture
//! filtering, Supersampling and Sharpening on (task 17 of docs/milestone-7.md), beside the frame
//! with them off, for looking at by eye. Each image is the picture at the scale's size: the
//! frame as drawn for off and texture filtering, filtered down for supersampling (the EFB drawn
//! at the scale times its factor) and sharpened from the plain frame for sharpening, as the
//! player's present makes them (`melee_gpu::post`).

use std::path::Path;

use melee_gpu::post::{Filters, Post};
use melee_gpu::{Anisotropy, Frame};

use crate::native;

/// What each renderer draws: its name in the files, its scale's factor and texture filtering.
const RENDERERS: [(&str, u32, u32); 7] = [
    ("off", 1, 1),
    ("af2", 1, 2),
    ("af4", 1, 4),
    ("af8", 1, 8),
    ("af16", 1, 16),
    ("ssaa2", 2, 1),
    ("ssaa4", 4, 1),
];

/// Draws `at`'s frames of the replay at `scale` (as wide as `aspect` shows them) into `dir`:
/// `frame_N_NAME.png` for each, listed on stdout.
pub fn run(disc: &str, replay: &str, dir: &Path, scale: u32, aspect: f32, at: &[u32]) {
    std::fs::create_dir_all(dir).expect("making the output directory");
    let width = |s: u32| (480.0 * s as f32 * aspect / 2.0).round() as u32 * 2;
    let widescreen = (aspect > melee_render::frame::GAME_ASPECT + 0.001).then_some(aspect);
    let mut gpus = Vec::new();
    let renderers = RENDERERS
        .iter()
        .map(|&(_, factor, samples)| {
            let mut r = native::renderer();
            let s = (scale * factor).min(8);
            r.resize_efb(s, width(s));
            (r.xfb_on_gpu, r.xfb_readback) = (true, true);
            r.set_anisotropy(Anisotropy::new(samples));
            gpus.push((r.device().clone(), r.queue().clone()));
            r
        })
        .collect();
    let (w, h) = (width(scale), 480 * scale);
    let save = |n: u32, name: &str, frame: &Frame| {
        let path = dir.join(format!("frame_{n}_{name}.png"));
        frame.save_png(&path).expect("saving a frame");
        println!("{}", path.display());
    };
    let filtered = |gpu: &(wgpu::Device, wgpu::Queue), frame: &Frame, filters: Filters| {
        let (device, queue) = gpu;
        let mut post = Post::new(device, wgpu::TextureFormat::Rgba8Unorm);
        let texture = frame.texture.as_ref().expect("the frame on the GPU");
        Frame {
            width: w,
            height: h,
            rgba: post.image(device, queue, texture, w, h, filters),
            texture: None,
        }
    };
    for (n, images) in native::draw_with(disc, replay, at, renderers, widescreen) {
        for ((name, factor, _), (image, gpu)) in RENDERERS.iter().zip(images.iter().zip(&gpus)) {
            if *factor == 1 {
                save(n, name, image);
            } else {
                let filters = Filters {
                    supersampling: *factor,
                    sharpening: None,
                };
                save(n, name, &filtered(gpu, image, filters));
            }
        }
        for strength in [50, 100] {
            let filters = Filters {
                supersampling: 1,
                sharpening: Some(strength),
            };
            save(
                n,
                &format!("sharpen{strength}"),
                &filtered(&gpus[0], &images[0], filters),
            );
        }
        let both = Filters {
            supersampling: 2,
            sharpening: Some(50),
        };
        save(n, "ssaa2_sharpen50", &filtered(&gpus[5], &images[5], both));
    }
}
