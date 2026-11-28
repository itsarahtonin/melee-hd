// SPDX-License-Identifier: GPL-3.0-or-later

//! The present's filters (`post`): supersampling's filter down averages every texel under a
//! pixel, and sharpening leaves flat areas alone and steepens edges, more with more strength.
//! Skips without a GPU adapter, or on a software one.

use crate::post::{Filters, Post};

fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let adapter = crate::testing::adapter()?;
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()
}

/// An RGBA8 texture `w` by `h` of `rgba`, as the frames the renderer leaves on the GPU are.
fn texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    w: u32,
    h: u32,
    rgba: &[u8],
) -> wgpu::Texture {
    let size = wgpu::Extent3d {
        width: w,
        height: h,
        depth_or_array_layers: 1,
    };
    let t = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        t.as_image_copy(),
        rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(w * 4),
            rows_per_image: Some(h),
        },
        size,
    );
    t
}

#[test]
fn off_is_the_plain_blit() {
    assert!(Filters::OFF.is_off());
    let on = |supersampling, sharpening| Filters {
        supersampling,
        sharpening,
    };
    assert!(!on(2, None).is_off() && !on(1, Some(0)).is_off());
}

/// Frames at twice and four times the picture's size each way, filtered down to it: every pixel
/// the average of the texels under it. A checkerboard of single texels comes out mid gray; at
/// four times, a grid with one texel in each 4 by 4 block white comes out a sixteenth of white,
/// where one bilinear sample at the pixel's middle (the plain blit's, which reads only the
/// block's middle 2 by 2) would give black.
#[test]
fn supersampling_averages_the_texels_under_each_pixel() {
    let Some((device, queue)) = device() else {
        return;
    };
    let mut post = Post::new(&device, wgpu::TextureFormat::Rgba8Unorm);
    fn checkerboard(x: u32, y: u32) -> bool {
        (x + y) % 2 == 1
    }
    fn grid(x: u32, y: u32) -> bool {
        x.is_multiple_of(4) && y.is_multiple_of(4)
    }
    // Each case: the factor, which texels are white, and the gray every pixel comes out.
    type Case = (u32, fn(u32, u32) -> bool, i32);
    let cases: [Case; 3] = [
        (2, checkerboard, 128),
        (4, checkerboard, 128),
        (4, grid, 16),
    ];
    for (factor, white, expected) in cases {
        let (w, h) = (64, 48);
        let (fw, fh) = (w * factor, h * factor);
        let rgba: Vec<u8> = (0..fw * fh)
            .flat_map(|i| {
                let v = if white(i % fw, i / fw) { 255 } else { 0 };
                [v, v, v, 255]
            })
            .collect();
        let frame = texture(&device, &queue, fw, fh, &rgba);
        let filters = Filters {
            supersampling: factor,
            sharpening: None,
        };
        let out = post.image(&device, &queue, &frame, w, h, filters);
        let worst = out
            .chunks(4)
            .map(|p| (i32::from(p[0]) - expected).abs())
            .max()
            .unwrap();
        assert!(worst <= 1, "{factor}x: a pixel {worst} off {expected}");
    }
}

/// Sharpening: a flat image comes back as it was; across an edge between 64 and 192, the dark
/// side darkens and the bright side brightens, more at full strength than at none, and away from
/// the edge nothing changes.
#[test]
fn sharpening_steepens_edges_and_leaves_flat_areas() {
    let Some((device, queue)) = device() else {
        return;
    };
    let mut post = Post::new(&device, wgpu::TextureFormat::Rgba8Unorm);
    let (w, h) = (32, 8);
    let sharpened = |post: &mut Post, rgba: &[u8], strength| {
        let frame = texture(&device, &queue, w, h, rgba);
        let filters = Filters {
            supersampling: 1,
            sharpening: Some(strength),
        };
        post.image(&device, &queue, &frame, w, h, filters)
    };
    let flat: Vec<u8> = [100, 150, 200, 255].repeat((w * h) as usize);
    let out = sharpened(&mut post, &flat, 100);
    assert!(
        out.chunks(4).all(|p| p[..3]
            .iter()
            .zip([100, 150, 200])
            .all(|(&a, b)| a.abs_diff(b) <= 1)),
        "a flat image changed"
    );
    let edge: Vec<u8> = (0..w * h)
        .flat_map(|i| {
            let v = if i % w < w / 2 { 64 } else { 192 };
            [v, v, v, 255]
        })
        .collect();
    let at = |out: &[u8], x: u32| out[((3 * w + x) * 4) as usize];
    let none = sharpened(&mut post, &edge, 0);
    let full = sharpened(&mut post, &edge, 100);
    let (dark, bright) = (w / 2 - 1, w / 2);
    assert!(
        at(&none, dark) < 64 && at(&none, bright) > 192,
        "no steeper at 0"
    );
    assert!(
        at(&full, dark) < at(&none, dark) && at(&full, bright) > at(&none, bright),
        "full strength no steeper than none"
    );
    for out in [&none, &full] {
        assert!(at(out, 4).abs_diff(64) <= 1 && at(out, w - 4).abs_diff(192) <= 1);
    }
}
