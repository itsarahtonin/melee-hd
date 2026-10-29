//! The settings panel's dialogs ([`crate::front`]): the file pickers and the confirmations of
//! what restarts Melee, worded as Sarah approved them (2026-10-28).

use std::path::Path;

use rfd::{FileDialog, MessageButtons, MessageDialog, MessageDialogResult, MessageLevel};

use crate::saves;

/// Asks `question` with a button `yes` and Cancel: whether `yes` was pressed.
pub fn confirm(question: &str, yes: &str) -> bool {
    let r = MessageDialog::new()
        .set_level(MessageLevel::Warning)
        .set_title("melee-hd")
        .set_description(question)
        .set_buttons(MessageButtons::OkCancelCustom(
            yes.to_owned(),
            "Cancel".to_owned(),
        ))
        .show();
    matches!(r, MessageDialogResult::Ok) || matches!(&r, MessageDialogResult::Custom(s) if s == yes)
}

/// A file's name, for what the dialogs say.
fn name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

/// "Import a save…": a save or a card image picked, judged, confirmed and brought into the
/// saves folder, the old files kept in the backups. Whether it was (Melee restarts to load
/// it).
pub fn import() -> bool {
    let Some(path) = FileDialog::new()
        .set_title("Import a save")
        .add_filter(
            "Melee saves (.gci) and memory card images (.raw, .card, .gcp)",
            &["gci", "raw", "card", "gcp"],
        )
        .add_filter("All files", &["*"])
        .pick_file()
    else {
        return false;
    };
    let file = name(&path);
    let found = std::fs::read(&path)
        .map_err(|e| e.to_string())
        .and_then(|b| saves::import(&b));
    let import = match found {
        Ok(i) => i,
        Err(why) => {
            crate::settings::error_box(&format!(
                "{file} holds no Melee save melee-hd can load: {why}."
            ));
            return false;
        }
    };
    let snapshots = match import.snapshots() {
        0 => String::new(),
        1 => " (and its Camera Mode snapshot)".to_owned(),
        n => format!(" (and its {n} Camera Mode snapshots)"),
    };
    let question = format!(
        "Replace your save with the one in {file}{snapshots}? Your current save is kept in the backups folder. Melee restarts from power-on to load it."
    );
    if !confirm(&question, "Import and restart") {
        return false;
    }
    match saves::replace(&saves::dir(), &import) {
        Ok(()) => true,
        Err(e) => {
            crate::settings::error_box(&format!("The save couldn't be imported: {e}."));
            false
        }
    }
}

/// "Export the save…": the save as a `.gci`, or the whole card as a raw image (`.raw`), as the
/// name picked ends. What happened, for the panel.
pub fn export() -> Option<String> {
    let path = FileDialog::new()
        .set_title("Export the save")
        .set_file_name(saves::SAVE_FILE)
        .add_filter(".gci (Dolphin's GCI folder, other tools)", &["gci"])
        .add_filter(
            ".raw (a whole memory card image, for Dolphin's memory card slot or ssbm-rs)",
            &["raw"],
        )
        .save_file()?;
    let dir = saves::dir();
    let raw = path
        .extension()
        .is_some_and(|x| x.eq_ignore_ascii_case("raw") || x.eq_ignore_ascii_case("card"));
    let bytes = if raw {
        Some(saves::export_raw(&dir))
    } else {
        saves::export_gci(&dir)
    };
    let r = match bytes {
        Some(b) => std::fs::write(&path, b).map_err(|e| e.to_string()),
        None => Err("there's no save yet".to_owned()),
    };
    match r {
        Ok(()) => Some(format!("Exported to {}", path.display())),
        Err(e) => {
            crate::settings::error_box(&format!("The save couldn't be exported: {e}."));
            None
        }
    }
}

/// "Unlock everything…": confirmed, the save edited (`unlock::unlock_all`), the old kept in
/// the backups. Whether it was (Melee restarts to load it).
pub fn unlock_all() -> bool {
    if !confirm(
        "Unlock every character, stage, trophy, event and special message? Your records (matches, times, scores, coins) stay as they are. Your current save is kept in the backups folder, so you can undo this by importing it. Melee restarts from power-on.",
        "Unlock and restart",
    ) {
        return false;
    }
    let now = (crate::power::power_on_clock() / melee_sim::audio::clock::TB_HZ) as u32;
    match saves::unlock_all(&saves::dir(), now) {
        Ok(()) => true,
        Err(e) => {
            crate::settings::error_box(&format!("The save couldn't be unlocked: {e}."));
            false
        }
    }
}

/// "Change disc…": a disc image picked, checked and confirmed, then kept as the disc to play.
/// The new disc's path (Melee restarts with it).
pub fn change_disc() -> Option<String> {
    let path = FileDialog::new()
        .set_title("Choose your Super Smash Bros. Melee disc image (NTSC 1.02)")
        .add_filter(
            "Disc images",
            &["iso", "gcm", "rvz", "ciso", "gcz", "wia", "wbfs", "nfs"],
        )
        .add_filter("All files", &["*"])
        .pick_file()?;
    let file = name(&path);
    if let Err(e) = melee_data::disc::Disc::open(&path) {
        crate::settings::error_box(&format!(
            "melee-hd can't play {file}:\n{}",
            crate::settings::capitalized(&crate::settings::why(&e))
        ));
        return None;
    }
    if !confirm(
        &format!(
            "Change the disc to {file}? Melee restarts from power-on. Anything the game hasn't saved is lost."
        ),
        "Change and restart",
    ) {
        return None;
    }
    let p = path.to_string_lossy().into_owned();
    crate::settings::keep_disc(&p);
    Some(p)
}
