//! The player's settings folder: the disc it plays, asked for once with a file picker and
//! remembered, the settings (`front`), the saves and the session recordings, and the log when there's no console (`console`).
//! It's `%APPDATA%\melee-hd` on Windows and `$XDG_CONFIG_HOME/melee-hd` or `~/.config/melee-hd`
//! elsewhere (`melee-hd-dev` for a dev build, [`crate::channel`]), or the folder
//! `MELEE_HD_SETTINGS` names.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::channel::Channel;
use melee_data::disc::{Disc, DiscError, GAME_ID, REVISION};
use rfd::{FileDialog, MessageButtons, MessageDialog, MessageDialogResult, MessageLevel};

/// The file that keeps the disc's path.
const DISC: &str = "disc.txt";

/// The settings folder.
pub fn folder() -> PathBuf {
    folder_of(crate::channel::channel(), |name| std::env::var_os(name))
}

/// The settings folder of a build of `channel`, with `var` the environment: the dev build's is
/// `melee-hd-dev` (`channel`).
fn folder_of(channel: Channel, var: impl Fn(&str) -> Option<OsString>) -> PathBuf {
    if let Some(dir) = var("MELEE_HD_SETTINGS") {
        return dir.into();
    }
    let base = var("APPDATA")
        .or_else(|| var("XDG_CONFIG_HOME"))
        .map(PathBuf::from)
        .or_else(|| var("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join(channel.folder())
}

/// A settings file's text; none when it's missing or unreadable.
pub fn read(name: &str) -> Option<String> {
    std::fs::read_to_string(folder().join(name)).ok()
}

/// Writes a settings file, making the folder if need be; a failure is only logged.
pub fn write(name: &str, text: &str) {
    let folder = folder();
    if let Err(e) =
        std::fs::create_dir_all(&folder).and_then(|()| std::fs::write(folder.join(name), text))
    {
        eprintln!("couldn't keep {name} in {}: {e}", folder.display());
    }
}

/// Keeps `path` as the disc to play.
pub fn keep_disc(path: &str) {
    write(DISC, path);
}

/// Why a disc can't be played, in words for whoever picked it.
pub fn why(e: &DiscError) -> String {
    let wanted =
        format!("melee-hd plays NTSC version 1.02 (game ID {GAME_ID}, revision {REVISION})");
    match e {
        DiscError::WrongGame { game_id, revision } => match game_id.as_str() {
            GAME_ID => format!("it's Melee's NTSC version 1.0{revision}, and {wanted}."),
            "GALP01" => format!("it's Melee's PAL version, and {wanted}."),
            "GALJ01" => format!("it's Melee's Japanese version, and {wanted}."),
            _ => format!("it isn't Super Smash Bros. Melee (its game ID is {game_id})."),
        },
        DiscError::Nod(_) => format!("it isn't a disc image melee-hd can read ({e})."),
        e => format!("{e}."),
    }
}

/// Shows an error in a message box.
pub fn error_box(text: &str) {
    MessageDialog::new()
        .set_level(MessageLevel::Error)
        .set_title("melee-hd")
        .set_description(text)
        .set_buttons(MessageButtons::Ok)
        .show();
}

/// The first launch's word on what melee-hd needs, after `note` if any: true to browse for it,
/// false to quit.
fn welcome(note: &str) -> bool {
    const BROWSE: &str = "Browse\u{2026}";
    let result = MessageDialog::new()
        .set_level(MessageLevel::Info)
        .set_title("melee-hd")
        .set_description(format!(
            "{note}melee-hd plays Super Smash Bros. Melee from your own copy of the game.\n\n\
             Choose a disc image of Melee, NTSC version 1.02 (game ID {GAME_ID}, revision \
             {REVISION}), such as an .iso or .rvz file. melee-hd remembers it, so you only do \
             this once."
        ))
        .set_buttons(MessageButtons::OkCancelCustom(
            BROWSE.to_owned(),
            "Quit".to_owned(),
        ))
        .show();
    // On Linux the dialog is zenity's, whose Quit answers Custom: plain Cancel means zenity
    // couldn't run, so go on to the disc picker (the desktop's portal) rather than quit unasked.
    matches!(result, MessageDialogResult::Ok)
        || matches!(&result, MessageDialogResult::Custom(s) if s == BROWSE)
        || cfg!(target_os = "linux") && matches!(result, MessageDialogResult::Cancel)
}

/// The disc to play when neither `--disc` nor `SSBM_DISC` names one: the one remembered, or
/// one asked for and then remembered. None when asked and quit instead.
pub fn disc() -> Option<String> {
    let saved = read(DISC)
        .map(|s| PathBuf::from(s.trim()))
        .filter(|p| !p.as_os_str().is_empty());
    let mut note = String::new();
    if let Some(p) = &saved {
        match Disc::open(p) {
            Ok(_) => return Some(p.to_string_lossy().into_owned()),
            Err(e) => {
                let reason = if p.exists() {
                    why(&e)
                } else {
                    "the file isn't there anymore.".to_owned()
                };
                note = format!(
                    "melee-hd can't play the disc image it played before:\n{}\n\n{}\n\n",
                    p.display(),
                    capitalized(&reason)
                );
            }
        }
    }
    loop {
        if !welcome(&note) {
            return None;
        }
        let Some(path) = FileDialog::new()
            .set_title("Choose your Super Smash Bros. Melee disc image (NTSC 1.02)")
            .add_filter(
                "Disc images",
                &["iso", "gcm", "rvz", "ciso", "gcz", "wia", "wbfs", "nfs"],
            )
            .add_filter("All files", &["*"])
            .pick_file()
        else {
            continue;
        };
        match Disc::open(&path) {
            Ok(_) => {
                write(DISC, &path.to_string_lossy());
                return Some(path.to_string_lossy().into_owned());
            }
            Err(e) => {
                note = format!(
                    "melee-hd can't play {}:\n{}\n\n",
                    file_name(&path),
                    capitalized(&why(&e))
                )
            }
        }
    }
}

fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

pub fn capitalized(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map_or_else(String::new, |f| f.to_uppercase().chain(c).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channel::env;

    #[test]
    fn a_wrong_disc_is_explained() {
        let wrong = |game_id: &str, revision| {
            capitalized(&why(&DiscError::WrongGame {
                game_id: game_id.to_owned(),
                revision,
            }))
        };
        assert_eq!(
            wrong("GALE01", 0),
            "It's Melee's NTSC version 1.00, and melee-hd plays NTSC version 1.02 (game ID GALE01, revision 2)."
        );
        assert!(wrong("GALP01", 0).starts_with("It's Melee's PAL version"));
        assert_eq!(
            wrong("GMSE01", 0),
            "It isn't Super Smash Bros. Melee (its game ID is GMSE01)."
        );
    }

    #[test]
    fn a_dev_build_keeps_its_own_settings() {
        let windows = env(&[("APPDATA", "AD"), ("HOME", "H")]);
        let elsewhere = env(&[("HOME", "H")]);
        let xdg = env(&[("XDG_CONFIG_HOME", "X")]);
        let base = |b: &str, f: &str| Path::new(b).join(f);
        assert_eq!(folder_of(Channel::Stable, &windows), base("AD", "melee-hd"));
        assert_eq!(folder_of(Channel::Local, &windows), base("AD", "melee-hd"));
        assert_eq!(
            folder_of(Channel::Dev, &windows),
            base("AD", "melee-hd-dev")
        );
        assert_eq!(folder_of(Channel::Dev, &xdg), base("X", "melee-hd-dev"));
        assert_eq!(
            folder_of(Channel::Dev, &elsewhere),
            Path::new("H").join(".config").join("melee-hd-dev")
        );
        assert_eq!(
            folder_of(Channel::Local, &elsewhere),
            Path::new("H").join(".config").join("melee-hd")
        );
        // MELEE_HD_SETTINGS wins in every channel.
        let named = env(&[("MELEE_HD_SETTINGS", "S"), ("APPDATA", "AD")]);
        for c in [Channel::Stable, Channel::Dev, Channel::Local] {
            assert_eq!(folder_of(c, &named), PathBuf::from("S"));
        }
    }
}
