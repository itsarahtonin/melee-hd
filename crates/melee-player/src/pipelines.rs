//! The pipeline cache, after Aurora's design: every pipeline the renderer has needed is kept
//! by what it's made for (its blending, color and alpha writes and depth test), and made again
//! before the next run's first frame; the driver's own pipeline cache is kept beside it where
//! the device has one. Those pipelines run the TEV in one uber shader; the pipelines with
//! shaders specialized to a TEV configuration are kept too, each with its configuration. All are
//! made again on the renderer's shader threads from the start: until a draw's specialized
//! pipeline is made the uber shader draws it, and until its uber pipeline is made too its
//! specialized one is made on the spot (melee-gpu's `Specializer::make_now`).
//!
//! Beside the player's own lists, the shipped list (`data/pipelines`, built into the player):
//! the pipelines the fast renderer asked for over a corpus of matches and menus
//! (`tools/bench/collect-pipelines.sh`), made after the player's own, so a match of a kind the player
//! hasn't played seldom waits for one.

use std::ffi::OsString;
use std::path::PathBuf;

use crate::channel::Channel;
use crate::present::{Aspect, Gpu};

/// The longest a replay or a match started from the command line waits for the saved specialized
/// pipelines before its first frame: the 1080 Ti makes the 700 of a few matches' list in about a
/// second with its driver's cache warm.
pub const SAVED_LIST_WAIT: std::time::Duration = std::time::Duration::from_secs(3);

/// Where the player keeps its caches: `%LOCALAPPDATA%\melee-hd` on Windows,
/// `~/Library/Caches/melee-hd` on macOS, `$XDG_CACHE_HOME/melee-hd` or `~/.cache/melee-hd`
/// elsewhere (`melee-hd-dev` for a dev build, [`crate::channel`]), or the folder
/// `MELEE_HD_CACHE` names (the shipped list's collection).
pub fn dir() -> Option<PathBuf> {
    dir_of(crate::channel::channel(), crate::settings::MAC, |name| {
        std::env::var_os(name)
    })
}

/// The cache folder of a build of `channel`, on macOS if `mac`, with `var` the environment.
fn dir_of(channel: Channel, mac: bool, var: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    if let Some(d) = var("MELEE_HD_CACHE") {
        return Some(PathBuf::from(d));
    }
    if mac {
        let home = PathBuf::from(var("HOME")?);
        return Some(home.join("Library").join("Caches").join(channel.folder()));
    }
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

/// A list of specialized pipelines: a mark, then entries of a fixed number of words. A list
/// whose mark isn't this build's (another build's, whose entries could mean otherwise) gives
/// none; the renderer leaves out any entry it doesn't read.
fn specialized_list(b: &[u8]) -> Vec<Vec<u32>> {
    let (n, mark) = melee_gpu::Renderer::specialized_key_format();
    let w = words(b);
    match w.split_first() {
        Some((&m, rest)) if m == mark => rest.chunks_exact(n).map(<[u32]>::to_vec).collect(),
        _ => Vec::new(),
    }
}

/// A list of uber pipelines: what each is made for, eight words each.
fn pipeline_list(b: &[u8]) -> Vec<[u32; 8]> {
    words(b).as_chunks::<8>().0.to_vec()
}

/// The saved specialized pipelines.
fn specialized_keys() -> Vec<Vec<u32>> {
    specialized_path()
        .and_then(|p| std::fs::read(p).ok())
        .map_or_else(Vec::new, |b| specialized_list(&b))
}

/// Whether the shipped list is made: `MELEE_SHIPPED_PIPELINES=0` leaves it out (its collection,
/// and measuring without it).
fn use_shipped() -> bool {
    std::env::var("MELEE_SHIPPED_PIPELINES").map_or(true, |v| v != "0")
}

/// The shipped list's uber pipelines (`data/pipelines/pipelines.bin`), as the saved
/// `pipelines.bin` holds them.
const SHIPPED_PIPELINES: &[u8] = include_bytes!("../../../data/pipelines/pipelines.bin");

/// The shipped list's specialized pipelines (`data/pipelines/specialized-pipelines.bin`), as the
/// saved `specialized-pipelines.bin` holds them.
const SHIPPED_SPECIALIZED: &[u8] =
    include_bytes!("../../../data/pipelines/specialized-pipelines.bin");

/// A renderer at `scale`, `samples` samples a pixel and an EFB as wide as `aspect` shows it,
/// with the cached pipelines made.
pub fn renderer(gpu: &Gpu, scale: u32, samples: u32, aspect: Aspect) -> melee_gpu::Renderer {
    let mut r = gpu.renderer(scale, samples, aspect);
    let driver = driver_path().and_then(|p| std::fs::read(p).ok());
    r.use_pipeline_cache(driver.as_deref());
    // The fast renderer's transforms, made beside the lists from here.
    r.make_ahead();
    let keys: Vec<[u32; 8]> = keys_path()
        .and_then(|p| std::fs::read(p).ok())
        .map(|b| pipeline_list(&b))
        .unwrap_or_default();
    let shipped = use_shipped();
    let more: Vec<[u32; 8]> = pipeline_list(if shipped { SHIPPED_PIPELINES } else { &[] })
        .into_iter()
        .filter(|k| !keys.contains(k))
        .collect();
    let start = std::time::Instant::now();
    r.warm(&keys);
    r.warm_shipped_pipelines(&more);
    if !keys.is_empty() || !more.is_empty() {
        eprintln!(
            "pipelines: {} queued ({} saved) in {:.0} ms",
            keys.len() + more.len(),
            keys.len(),
            start.elapsed().as_secs_f64() * 1000.0
        );
    }
    let specialized = specialized_keys();
    let shipped = specialized_list(if shipped { SHIPPED_SPECIALIZED } else { &[] });
    let start = std::time::Instant::now();
    let saved = r.warm_specialized(&specialized);
    let added = r.warm_shipped(&shipped);
    if saved + added > 0 {
        eprintln!(
            "specialized pipelines: {saved} saved and {added} more shipped queued in {:.0} ms",
            start.elapsed().as_secs_f64() * 1000.0
        );
    }
    r
}

/// The saved list `on_disk` with `ours` added: each entry once, those on disk first in their
/// order, then ours in our order. Every renderer saves its own list, which holds the list as it
/// was when the renderer was made and what it asked for; merging keeps what the other renderers
/// of the run (the menus' and each match's) saved meanwhile.
fn merged<T: Clone + Eq + std::hash::Hash>(on_disk: Vec<T>, ours: Vec<T>) -> Vec<T> {
    let mut seen: std::collections::HashSet<T> = on_disk.iter().cloned().collect();
    let mut list = on_disk;
    list.extend(ours.into_iter().filter(|k| seen.insert(k.clone())));
    list
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
    let on_disk = keys_path()
        .and_then(|p| std::fs::read(p).ok())
        .map_or_else(Vec::new, |b| pipeline_list(&b));
    let bytes: Vec<u8> = merged(on_disk, r.pipeline_keys())
        .iter()
        .flat_map(|k| k.iter().flat_map(|w| w.to_le_bytes()))
        .collect();
    if let Some(p) = keys_path() {
        let _ = std::fs::write(p, bytes);
    }
    let (_, mark) = melee_gpu::Renderer::specialized_key_format();
    let specialized: Vec<u8> = std::iter::once(mark)
        .chain(
            merged(specialized_keys(), r.specialized_keys())
                .into_iter()
                .flatten(),
        )
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

    /// The shipped list is one this build reads whole: its mark is this build's (a change to
    /// what a TEV configuration holds changes the mark, and this fails until the list is made
    /// again with `tools/bench/collect-pipelines.sh`), every entry is one the renderer takes, none
    /// twice, and it stays small.
    #[test]
    fn the_shipped_list_is_one_this_build_reads() {
        let (n, mark) = melee_gpu::Renderer::specialized_key_format();
        let w = words(SHIPPED_SPECIALIZED);
        assert_eq!(
            w.first(),
            Some(&mark),
            "the shipped list is another build's: make it again (tools/bench/collect-pipelines.sh)"
        );
        assert_eq!((w.len() - 1) % n, 0, "a whole number of entries");
        let entries = specialized_list(SHIPPED_SPECIALIZED);
        assert!(entries.len() > 500, "{} entries", entries.len());
        let unread = entries
            .iter()
            .filter(|e| !melee_gpu::Renderer::reads_specialized_entry(e))
            .count();
        assert_eq!(unread, 0, "entries this build doesn't read");
        let distinct: std::collections::HashSet<&Vec<u32>> = entries.iter().collect();
        assert_eq!(distinct.len(), entries.len(), "an entry twice");
        let keys = pipeline_list(SHIPPED_PIPELINES);
        assert_eq!(SHIPPED_PIPELINES.len() % 32, 0);
        assert!(!keys.is_empty());
        assert!(keys.iter().all(melee_gpu::Renderer::reads_pipeline_key));
        let bytes = SHIPPED_SPECIALIZED.len() + SHIPPED_PIPELINES.len();
        assert!(bytes < 2 << 20, "{bytes} bytes");
    }

    /// A list from another build (its mark not this build's) is left out whole; a list cut
    /// short keeps its whole entries.
    #[test]
    fn a_list_from_another_build_is_left_out() {
        let (n, mark) = melee_gpu::Renderer::specialized_key_format();
        let entry: Vec<u32> = specialized_list(SHIPPED_SPECIALIZED)[0].clone();
        let list = |m: u32, entries: usize, cut: usize| -> Vec<u8> {
            let mut w = vec![m];
            for _ in 0..entries {
                w.extend_from_slice(&entry);
            }
            let mut b: Vec<u8> = w.iter().flat_map(|w| w.to_le_bytes()).collect();
            b.truncate(b.len() - cut);
            b
        };
        assert_eq!(specialized_list(&list(mark, 3, 0)).len(), 3);
        assert_eq!(specialized_list(&list(mark, 3, 6)).len(), 2);
        assert!(specialized_list(&list(mark ^ 1, 3, 0)).is_empty());
        assert!(specialized_list(&list(mark, 0, 0)).is_empty());
        assert!(specialized_list(&[]).is_empty());
        assert_eq!(entry.len(), n);
    }

    /// Saving keeps what's saved and adds what the renderer asked for that isn't, in order: the
    /// menus' renderer saving after a match's keeps the match's.
    #[test]
    fn saving_adds_to_the_saved_list() {
        let saved = vec![vec![1, 2], vec![3, 4], vec![5, 6]];
        let ours = vec![vec![1, 2], vec![7, 8], vec![5, 6], vec![9, 10]];
        assert_eq!(
            merged(saved, ours),
            vec![vec![1, 2], vec![3, 4], vec![5, 6], vec![7, 8], vec![9, 10]]
        );
        assert_eq!(
            merged(Vec::new(), vec![[2; 8], [1; 8]]),
            vec![[2; 8], [1; 8]]
        );
    }

    #[test]
    fn a_dev_build_keeps_its_own_caches() {
        let windows = env(&[("LOCALAPPDATA", "LA"), ("HOME", "H")]);
        let elsewhere = env(&[("HOME", "H")]);
        let xdg = env(&[("XDG_CACHE_HOME", "X")]);
        let cache = |b: &str, f: &str| Some(Path::new(b).join(f));
        assert_eq!(
            dir_of(Channel::Stable, false, &windows),
            cache("LA", "melee-hd")
        );
        assert_eq!(
            dir_of(Channel::Local, false, &windows),
            cache("LA", "melee-hd")
        );
        assert_eq!(
            dir_of(Channel::Dev, false, &windows),
            cache("LA", "melee-hd-dev")
        );
        assert_eq!(
            dir_of(Channel::Dev, false, &xdg),
            cache("X", "melee-hd-dev")
        );
        assert_eq!(
            dir_of(Channel::Dev, false, &elsewhere),
            Some(Path::new("H").join(".cache").join("melee-hd-dev"))
        );
        // MELEE_HD_SETTINGS names the settings folder only.
        let named = env(&[("MELEE_HD_SETTINGS", "S"), ("LOCALAPPDATA", "LA")]);
        assert_eq!(
            dir_of(Channel::Dev, false, &named),
            cache("LA", "melee-hd-dev")
        );
        assert_eq!(dir_of(Channel::Dev, false, env(&[])), None);
        // MELEE_HD_CACHE names the cache folder itself.
        let cache = env(&[("MELEE_HD_CACHE", "C"), ("LOCALAPPDATA", "LA")]);
        assert_eq!(
            dir_of(Channel::Dev, false, &cache),
            Some(PathBuf::from("C"))
        );
        assert_eq!(dir_of(Channel::Dev, true, &cache), Some(PathBuf::from("C")));
    }

    /// macOS keeps them in `~/Library/Caches`, whatever XDG's variables say.
    #[test]
    fn macos_keeps_caches_in_library_caches() {
        let caches = |f: &str| Some(Path::new("H").join("Library").join("Caches").join(f));
        let xdg = env(&[("HOME", "H"), ("XDG_CACHE_HOME", "X")]);
        assert_eq!(dir_of(Channel::Stable, true, &xdg), caches("melee-hd"));
        assert_eq!(dir_of(Channel::Dev, true, &xdg), caches("melee-hd-dev"));
        assert_eq!(dir_of(Channel::Dev, true, env(&[])), None);
    }
}
