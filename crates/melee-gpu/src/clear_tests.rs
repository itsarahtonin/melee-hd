// SPDX-License-Identifier: GPL-3.0-or-later

//! Checks that a clear made by a render pass's load operation leaves the EFB (color, alpha and
//! depth) bit for bit as the clear drawn as a rectangle does: the same clears through a
//! renderer that makes them natively where it can and one that draws them all. Skips without
//! a GPU adapter, or on a software one.

use melee_gx::State;

use crate::{EFB_HEIGHT, EFB_WIDTH, Renderer};

fn renderer(samples: u32, rect_clears: bool) -> Option<Renderer> {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).ok()?;
    // Not on a software adapter: WARP, which CI's Windows runners fall back to, crashes in
    // d3d10warp.dll at random with gx.wgsl (melee-render's corner_copies test has the details).
    if adapter.get_info().device_type == wgpu::DeviceType::Cpu {
        return None;
    }
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_features: wgpu::Features::DUAL_SOURCE_BLENDING,
        ..Default::default()
    }))
    .ok()?;
    let mut r = Renderer::with_device_msaa(device, queue, 1, samples);
    r.rect_clears = rect_clears;
    Some(r)
}

/// A copy's clear: the EFB's pixel format, the clear color (ARGB) and depth, and the updates.
fn clear_state(format: u32, argb: u32, z: u32, color: bool, alpha: bool, depth: bool) -> State {
    let mut s = State::new();
    s.bp[0x40] = 0x07 | u32::from(depth) << 4;
    s.bp[0x41] = u32::from(color) << 3 | u32::from(alpha) << 4;
    s.bp[0x43] = format;
    s.bp[0x4F] = argb >> 16;
    s.bp[0x50] = argb & 0xFFFF;
    s.bp[0x51] = z;
    s
}

/// The EFB's depth as stored, read back (single-sampled targets only).
fn read_depth(r: &Renderer) -> Vec<u8> {
    let (width, height) = (EFB_WIDTH, EFB_HEIGHT);
    let row = width * 4;
    let buffer = r.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: u64::from(row * height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = r.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: r.depth_view.texture(),
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::DepthOnly,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    r.queue.submit([encoder.finish()]);
    let slice = buffer.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    r.device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("GPU readback");
    let data = slice.get_mapped_range().expect("mapped readback").to_vec();
    buffer.unmap();
    data
}

const WHOLE: [u32; 4] = [0, 0, EFB_WIDTH, EFB_HEIGHT];
const DRAWN: [u32; 4] = [0, 0, EFB_WIDTH, 480];

/// Each step's clear, and whether the native renderer makes it (color, depth) by load
/// operation.
struct Step {
    rect: [u32; 4],
    state: State,
    native: (bool, bool),
}

fn steps() -> Vec<Step> {
    let black = 0xFF00_0000;
    vec![
        // A new renderer's clear (melee-render's `clear_new`): RGB8, alpha kept.
        Step {
            rect: WHOLE,
            state: clear_state(0, 0xFF20_4060, 0xFF_FFFF, true, false, true),
            native: (true, true),
        },
        // A draw's stand-in, in the drawn part of the EFB.
        Step {
            rect: [16, 16, 100, 50],
            state: clear_state(0, 0x1122_3344, 0x12_3456, true, false, true),
            native: (false, false),
        },
        // The copy to the XFB's clear, of the drawn part, with what the rest holds.
        Step {
            rect: DRAWN,
            state: clear_state(0, 0xFF20_4060, 0xFF_FFFF, true, false, true),
            native: (true, true),
        },
        // Another color: the rest of the EFB keeps the old one, so it's drawn.
        Step {
            rect: DRAWN,
            state: clear_state(0, black, 0x40_0000, true, false, true),
            native: (false, false),
        },
        // A texture copy's clear of color alone.
        Step {
            rect: [0, 0, 256, 256],
            state: clear_state(0, black, 0xFF_FFFF, true, false, false),
            native: (false, false),
        },
        // RGBA6, all of it: six bits a channel.
        Step {
            rect: WHOLE,
            state: clear_state(1, 0x80FF_8041, 0x80_0001, true, true, true),
            native: (true, true),
        },
        // Alpha alone, drawn; then alpha isn't one value, so a clear keeping it is drawn.
        Step {
            rect: [8, 8, 32, 32],
            state: clear_state(1, 0x5500_0000, 0, false, true, false),
            native: (false, false),
        },
        Step {
            rect: WHOLE,
            state: clear_state(0, 0xFF10_2030, 0x00_0010, true, false, true),
            native: (false, true),
        },
        Step {
            rect: WHOLE,
            state: clear_state(1, 0x3366_99CC, 0x77_7777, true, true, false),
            native: (true, false),
        },
    ]
}

#[test]
fn load_operation_clears_match_drawn_ones() {
    for samples in [1, 4] {
        let (Some(mut native), Some(mut drawn)) =
            (renderer(samples, false), renderer(samples, true))
        else {
            eprintln!("skipped: no hardware GPU adapter");
            return;
        };
        for (i, step) in steps().into_iter().enumerate() {
            native.clear(&step.state, step.rect);
            drawn.clear(&step.state, step.rect);
            let pending = native.pending_clear;
            assert_eq!(
                (pending.color.is_some(), pending.depth.is_some()),
                step.native,
                "step {i}: which targets the load operation clears"
            );
            native.flush();
            drawn.flush();
            assert!(
                native.read_efb() == drawn.read_efb(),
                "step {i}, {samples} samples: the color differs"
            );
            if samples == 1 {
                assert!(
                    read_depth(&native) == read_depth(&drawn),
                    "step {i}: the depth differs"
                );
            }
        }
    }
}
