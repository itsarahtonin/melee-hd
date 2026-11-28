//! An NKit image chosen as the disc ([`melee_data::nkit`]): restored once, with the player's yes,
//! to an RVZ in the cache folder (`discs/`, [`crate::pipelines::dir`]), which is then the disc
//! melee-hd plays (milestone 7's design call 16: restoring takes seconds and a check of the whole
//! disc, too long for every start). Nothing is written beside the player's file. The settings keep
//! the NKit file's path, and each start finds its RVZ by the retail disc's CRC32, which the NKit
//! header records and the RVZ was checked against.

use std::path::{Path, PathBuf};

use melee_data::disc::{GAME_ID, REVISION};
use melee_data::nkit::Header;

/// The question before restoring `file`.
pub fn question(file: &str) -> String {
    format!(
        "{file} is an NKit image, which melee-hd can't play as it is. Restore it once to a disc \
         image melee-hd keeps (about 1 GB)? It takes a few seconds, and melee-hd goes on when it's \
         done. Your NKit file isn't changed."
    )
}

/// The words for an NKit image that doesn't restore, `e` being why.
pub fn refusal(e: &dyn std::fmt::Display) -> String {
    format!("it's an NKit image that doesn't restore to Melee NTSC 1.02 ({e}).")
}

/// The RVZ an NKit image of the retail disc with CRC32 `crc32` restores to, in `cache`: named by
/// that CRC32, so any NKit image of the same disc finds it.
fn rvz_in(cache: &Path, crc32: u32) -> PathBuf {
    cache
        .join("discs")
        .join(format!("{GAME_ID}-{REVISION}-{crc32:08X}.rvz"))
}

/// The RVZ the NKit image at `path` was restored to, if it is one and has been.
fn restored(path: &Path) -> Option<PathBuf> {
    let header = Header::of(path).ok().flatten()?;
    Some(rvz_in(&crate::pipelines::dir()?, header.crc32)).filter(|p| p.is_file())
}

/// The file to open as the disc at `path`: the RVZ an NKit image was restored to, else `path`.
pub fn playable(path: &str) -> PathBuf {
    restored(Path::new(path)).unwrap_or_else(|| PathBuf::from(path))
}

/// For the NKit image of Melee at `path` ([`melee_data::disc::DiscError::NKit`]): its RVZ when
/// restored before, else the question and, on a yes, the restoring. Why it can't be played (none
/// when the player said no).
pub fn prepare(path: &Path, file: &str) -> Result<(), Option<String>> {
    if restored(path).is_some() {
        return Ok(());
    }
    let header = match Header::of(path) {
        Ok(Some(h)) => h,
        Ok(None) => return Ok(()),
        Err(e) => return Err(Some(refusal(&e))),
    };
    let Some(cache) = crate::pipelines::dir() else {
        return Err(Some(refusal(&"melee-hd has no cache folder to keep it in")));
    };
    if !crate::dialogs::confirm(&question(file), "Restore") {
        return Err(None);
    }
    let out = rvz_in(&cache, header.crc32);
    eprintln!("Restoring the NKit image\u{2026}");
    let start = std::time::Instant::now();
    let result = std::fs::create_dir_all(out.parent().expect("discs/"))
        .map_err(|e| refusal(&e))
        .and_then(|()| melee_data::nkit::restore(path, &out, |_, _| {}).map_err(|e| refusal(&e)));
    match &result {
        Ok(()) => eprintln!(
            "restored {} to {} in {:.1} s",
            path.display(),
            out.display(),
            start.elapsed().as_secs_f64()
        ),
        Err(why) => eprintln!("{}: {why}", path.display()),
    }
    result.map_err(Some)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rvz_is_named_by_the_discs_crc() {
        assert_eq!(
            rvz_in(Path::new("C"), 0x0123_ABCD),
            Path::new("C").join("discs").join("GALE01-2-0123ABCD.rvz")
        );
    }

    #[test]
    fn a_refusal_says_why() {
        assert_eq!(
            crate::settings::capitalized(&refusal(&"it's cut short")),
            "It's an NKit image that doesn't restore to Melee NTSC 1.02 (it's cut short)."
        );
    }
}
