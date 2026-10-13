//! The pipeline cache, after Aurora's design: every pipeline the renderer has needed is kept
//! by what it's made for (its blending, color and alpha writes and depth test, which is all
//! that differs between the renderer's pipelines: the TEV runs in one shader, so no material
//! needs a pipeline of its own), and made again before the next run's first frame; the
//! driver's own pipeline cache is kept beside it where the device has one. A frame never waits
//! for a pipeline the player has used before.

use std::path::PathBuf;

use crate::present::{Aspect, Gpu};

/// Where the player keeps its caches: `%LOCALAPPDATA%\melee-hd` on Windows,
/// `$XDG_CACHE_HOME/melee-hd` or `~/.cache/melee-hd` elsewhere.
fn dir() -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("XDG_CACHE_HOME"))
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))?;
    Some(base.join("melee-hd"))
}

fn keys_path() -> Option<PathBuf> {
    dir().map(|d| d.join("pipelines.bin"))
}

fn driver_path() -> Option<PathBuf> {
    dir().map(|d| d.join("driver-pipelines.bin"))
}

/// A renderer at `scale`, `samples` samples a pixel and an EFB as wide as `aspect` shows it,
/// with the cached pipelines made.
pub fn renderer(gpu: &Gpu, scale: u32, samples: u32, aspect: Aspect) -> melee_gpu::Renderer {
    let mut r = gpu.renderer(scale, samples, aspect);
    let driver = driver_path().and_then(|p| std::fs::read(p).ok());
    r.use_pipeline_cache(driver.as_deref());
    let keys: Vec<[u32; 8]> = keys_path()
        .and_then(|p| std::fs::read(p).ok())
        .map(|b| {
            b.as_chunks::<32>()
                .0
                .iter()
                .map(|c| {
                    std::array::from_fn(|i| {
                        u32::from_le_bytes(c[4 * i..4 * i + 4].try_into().unwrap())
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let start = std::time::Instant::now();
    r.warm(&keys);
    if !keys.is_empty() {
        eprintln!(
            "pipelines: {} made in {:.0} ms",
            keys.len(),
            start.elapsed().as_secs_f64() * 1000.0
        );
    }
    r
}

/// Keeps the pipelines the run needed, with any it had from before, for the next.
pub fn save(r: &melee_gpu::Renderer) {
    let Some(d) = dir() else { return };
    if std::fs::create_dir_all(&d).is_err() {
        return;
    }
    let bytes: Vec<u8> = r
        .pipeline_keys()
        .iter()
        .flat_map(|k| k.iter().flat_map(|w| w.to_le_bytes()))
        .collect();
    if let Some(p) = keys_path() {
        let _ = std::fs::write(p, bytes);
    }
    if let (Some(p), Some(data)) = (driver_path(), r.pipeline_cache_data()) {
        let _ = std::fs::write(p, data);
    }
}
