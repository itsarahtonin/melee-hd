// SPDX-License-Identifier: GPL-3.0-or-later

//! The GPU for tests. Every test that draws gets its adapter or renderer here, so it skips
//! without a GPU and on a software adapter alike; `tests/gpu_tests.rs` checks that no test in
//! the workspace asks wgpu for an adapter, or makes a renderer, any other way.
//!
//! A software adapter can't run them: WARP, the one CI's Windows runners (which have no GPU)
//! fall back to, gets gx.wgsl wrong. With the same commands and shader (FXC's) that a GPU draws
//! right and D3D12's debug layer finds nothing in, WARP's TEV stage loop leaves no mark
//! (corner_copies' red quad came out black; the loop bounded to one stage draws it), DXC's
//! version of the shader doesn't finish its first draw in 30 minutes, and the process crashes
//! inside d3d10warp.dll at random.

use crate::Renderer;

/// The adapter the player and [`Renderer::new`] pick (the high-performance one), or `None`,
/// noted on stderr, when there is none or it is a software adapter: the test skips.
pub fn adapter() -> Option<wgpu::Adapter> {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = match pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    })) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("skipped: no GPU adapter ({e})");
            return None;
        }
    };
    let info = adapter.get_info();
    if info.device_type == wgpu::DeviceType::Cpu {
        eprintln!("skipped: {} is a software adapter", info.name);
        return None;
    }
    Some(adapter)
}

/// A renderer as [`Renderer::new`] makes it, on [`adapter`]'s adapter, or `None` (skip).
pub fn renderer() -> Option<Renderer> {
    match Renderer::with_adapter(&adapter()?) {
        Ok(r) => Some(r),
        Err(e) => {
            eprintln!("skipped: {e}");
            None
        }
    }
}
