//! The pipeline cache, after Aurora's design: every pipeline the renderer has needed is kept
//! by what it's made for (its blending, color and alpha writes and depth test), and made again
//! before the next run's first frame; the driver's own pipeline cache is kept beside it where
//! the device has one. A frame never waits for a pipeline the player has used before. Those
//! pipelines run the TEV in one uber shader; the pipelines with shaders specialized to a TEV
//! configuration are kept too, each with its configuration, and made again on the renderer's
//! shader threads from the start, the uber shader drawing until each is made.

use std::ffi::OsString;
use std::path::PathBuf;

use crate::channel::Channel;
use crate::present::{Aspect, Gpu};

/// Where the player keeps its caches: `%LOCALAPPDATA%\melee-hd` on Windows,
/// `$XDG_CACHE_HOME/melee-hd` or `~/.cache/melee-hd` elsewhere (`melee-hd-dev` for a dev build,
/// [`crate::channel`]).
pub fn dir() -> Option<PathBuf> {
    dir_of(crate::channel::channel(), |name| std::env::var_os(name))
}

/// The cache folder of a build of `channel`, with `var` the environment.
fn dir_of(channel: Channel, var: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let base = var("LOCALAPPDATA")
        .or_else(|| var("XDG_CACHE_HOME"))
        .map(PathBuf::from)
        .or_else(|| var("HOME").map(|h| PathBuf::from(h).join(".cache")))?;
    Some(base.join(channel.folder()))
}

fn keys_path() -> Option<PathBuf> {
    dir().map(|d| d.join("pipelines.bin"))
}

fn driver_path() -> Option<PathBuf> {
    dir().map(|d| d.join("driver-pipelines.bin"))
}

fn specialized_path() -> Option<PathBuf> {
    dir().map(|d| d.join("specialized-pipelines.bin"))
}

/// The words of a saved list.
fn words(b: &[u8]) -> Vec<u32> {
    b.as_chunks::<4>()
        .0
        .iter()
        .map(|c| u32::from_le_bytes(*c))
        .collect()
}

/// The saved specialized pipelines: a mark, then entries of a fixed number of words.
fn specialized_keys() -> Vec<Vec<u32>> {
    let (n, mark) = melee_gpu::Renderer::specialized_key_format();
    let Some(w) = specialized_path().and_then(|p| std::fs::read(p).ok()) else {
        return Vec::new();
    };
    let w = words(&w);
    match w.split_first() {
        Some((&m, rest)) if m == mark => rest.chunks_exact(n).map(<[u32]>::to_vec).collect(),
        _ => Vec::new(),
    }
}

/// A renderer at `scale`, `samples` samples a pixel and an EFB as wide as `aspect` shows it,
/// with the cached pipelines made.
pub fn renderer(gpu: &Gpu, scale: u32, samples: u32, aspect: Aspect) -> melee_gpu::Renderer {
    let mut r = gpu.renderer(scale, samples, aspect);
    let driver = driver_path().and_then(|p| std::fs::read(p).ok());
    r.use_pipeline_cache(driver.as_deref());
    let keys: Vec<[u32; 8]> = keys_path()
        .and_then(|p| std::fs::read(p).ok())
        .map(|b| words(&b).as_chunks::<8>().0.to_vec())
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
    let specialized = specialized_keys();
    let start = std::time::Instant::now();
    r.warm_specialized(&specialized);
    if !specialized.is_empty() {
        eprintln!(
            "specialized pipelines: {} queued in {:.0} ms",
            specialized.len(),
            start.elapsed().as_secs_f64() * 1000.0
        );
    }
    r
}

/// Keeps the pipelines the run needed, with any it had from before, for the next. Returns
/// what the specialized shaders did, if they did anything.
pub fn save(r: &melee_gpu::Renderer) -> Option<String> {
    let stats = r.specialized_stats();
    let summary = (stats.pipelines > 0).then(|| stats.to_string());
    if let Some(s) = &summary {
        eprintln!("specialized shaders: {s}");
    }
    let Some(d) = dir() else { return summary };
    if std::fs::create_dir_all(&d).is_err() {
        return summary;
    }
    let bytes: Vec<u8> = r
        .pipeline_keys()
        .iter()
        .flat_map(|k| k.iter().flat_map(|w| w.to_le_bytes()))
        .collect();
    if let Some(p) = keys_path() {
        let _ = std::fs::write(p, bytes);
    }
    let (_, mark) = melee_gpu::Renderer::specialized_key_format();
    let specialized: Vec<u8> = std::iter::once(mark)
        .chain(r.specialized_keys().into_iter().flatten())
        .flat_map(u32::to_le_bytes)
        .collect();
    if let Some(p) = specialized_path() {
        let _ = std::fs::write(p, specialized);
    }
    if let (Some(p), Some(data)) = (driver_path(), r.pipeline_cache_data()) {
        let _ = std::fs::write(p, data);
    }
    summary
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::channel::env;

    #[test]
    fn a_dev_build_keeps_its_own_caches() {
        let windows = env(&[("LOCALAPPDATA", "LA"), ("HOME", "H")]);
        let elsewhere = env(&[("HOME", "H")]);
        let xdg = env(&[("XDG_CACHE_HOME", "X")]);
        let cache = |b: &str, f: &str| Some(Path::new(b).join(f));
        assert_eq!(dir_of(Channel::Stable, &windows), cache("LA", "melee-hd"));
        assert_eq!(dir_of(Channel::Local, &windows), cache("LA", "melee-hd"));
        assert_eq!(dir_of(Channel::Dev, &windows), cache("LA", "melee-hd-dev"));
        assert_eq!(dir_of(Channel::Dev, &xdg), cache("X", "melee-hd-dev"));
        assert_eq!(
            dir_of(Channel::Dev, &elsewhere),
            Some(Path::new("H").join(".cache").join("melee-hd-dev"))
        );
        // MELEE_HD_SETTINGS names the settings folder only.
        let named = env(&[("MELEE_HD_SETTINGS", "S"), ("LOCALAPPDATA", "LA")]);
        assert_eq!(dir_of(Channel::Dev, &named), cache("LA", "melee-hd-dev"));
        assert_eq!(dir_of(Channel::Dev, env(&[])), None);
    }
}
