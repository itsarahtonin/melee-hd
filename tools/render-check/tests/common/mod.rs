//! What the frame checks share: running `melee-render-check compare` or `scaled` on a frame,
//! checking its result against what docs/player.md records, and leaving the result for the
//! dashboard.
//!
//! The references are ssbm-rs's FIFO logs and gxplay's frames of them,
//! `REFS/player/ref-GAME/fN.dff` and `fN/` (`REFS` defaults to ssbm-rs's `local/`,
//! `$SSBM_RS_LOCAL`), made as docs/player.md describes (Slippi's name plates off unless the
//! check says otherwise); the replays are the repository's benchmark matches and Slippi
//! replays in `$SLIPPI_REPLAYS` (the author's, not public). A frame whose first logged frame
//! samples a copy made before the log starts (a magnifier bubble's) takes a capture made with
//! ssbm-rs's renderer running, `ref-GAME/rendered/` ([`Capture::Rendered`]): without one, ssbm-rs's
//! copies never reach memory, and the log starts with the texture's old contents. Unlike the
//! tests that skip without
//! their data, these fail without it: they are a check to run on purpose.
//!
//! Each check writes what it measured to `RENDER_CHECK_RESULTS` (by default
//! `target/tmp/render-check-results`), which `melee-render-check dashboard DIR OUT_DIR`
//! makes a page of.

#![allow(dead_code)]

use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

use melee_local_data::{slippi_replays, ssbm_rs_local};

/// A replay a check plays.
#[derive(Clone, Copy)]
pub enum Replay {
    /// A benchmark match, `data/bench/NAME.slp`, played in the console's float mode.
    Bench(&'static str),
    /// A Slippi replay in `$SLIPPI_REPLAYS`, played in Slippi Dolphin's float mode; the
    /// variable named overrides the path.
    Slippi(&'static str, Option<&'static str>),
    /// A Slippi replay in ssbm-rs's `local/replays/hf/` (`$SSBM_RS_LOCAL`), played in Slippi
    /// Dolphin's float mode.
    Hf(&'static str),
    /// One of milestone 4's oracle matches, `$SSBM_RS_LOCAL/bench/oracle-m4/NAME.slp`, played
    /// in the console's float mode.
    Oracle(&'static str),
    /// A recorded console session from power-on, `$SSBM_RS_LOCAL/lockstep/idle-sessions/
    /// NAME.inputs`, `lockstep/rules-sessions/NAME.inputs` or `lockstep/menu-sessions/
    /// NAME.inputs`, or for `options-NAME` `lockstep/options-sessions/NAME.inputs`, for
    /// `select-NAME` `lockstep/select-sessions/NAME.inputs`, for `special-NAME`
    /// `lockstep/special-sessions/NAME.inputs`, for `trophy-NAME`
    /// `lockstep/trophy-sessions/NAME.inputs`, for `sound-NAME` `lockstep/sound-sessions/`,
    /// for `unlock-NAME` `lockstep/unlock-sessions/`, for `results-NAME`
    /// `lockstep/results-sessions/NAME.inputs`, for `tou-NAME`
    /// `lockstep/tournament-sessions/NAME.inputs` and for `celebiN`
    /// `lockstep/celebi-sessions/celebiN.inputs` (ssbm-rs's `SSBMINP1`), played in the
    /// console's float mode.
    Session(&'static str),
    /// Another replay in ssbm-rs's `local/` (`$SSBM_RS_LOCAL/PATH`): one of another set's
    /// oracle matches, or a match recorded with the player, played in the console's float
    /// mode.
    Local(&'static str),
}

impl Replay {
    fn path(self) -> PathBuf {
        match self {
            Replay::Bench(name) => melee_local_data::bench_replay(name),
            Replay::Oracle(name) => ssbm_rs_local(&format!("bench/oracle-m4/{name}.slp")).into(),
            Replay::Hf(rel) => ssbm_rs_local(&format!("replays/hf/{rel}")).into(),
            Replay::Session(name) => {
                if let Some(o) = name.strip_prefix("debug-") {
                    return ssbm_rs_local(&format!("lockstep/debug-sessions/{o}.inputs")).into();
                }
                if let Some(o) = name.strip_prefix("data-") {
                    return ssbm_rs_local(&format!("lockstep/data-sessions/{o}.inputs")).into();
                }
                if let Some(o) = name.strip_prefix("options-") {
                    return ssbm_rs_local(&format!("lockstep/options-sessions/{o}.inputs")).into();
                }
                if let Some(o) = name.strip_prefix("select-") {
                    return ssbm_rs_local(&format!("lockstep/select-sessions/{o}.inputs")).into();
                }
                if let Some(o) = name.strip_prefix("results-") {
                    return ssbm_rs_local(&format!("lockstep/results-sessions/{o}.inputs")).into();
                }
                if let Some(o) = name.strip_prefix("special-") {
                    return ssbm_rs_local(&format!("lockstep/special-sessions/{o}.inputs")).into();
                }
                if let Some(o) = name.strip_prefix("trophy-") {
                    return ssbm_rs_local(&format!("lockstep/trophy-sessions/{o}.inputs")).into();
                }
                if let Some(o) = name.strip_prefix("sound-") {
                    return ssbm_rs_local(&format!("lockstep/sound-sessions/{o}.inputs")).into();
                }
                if let Some(o) = name.strip_prefix("unlock-") {
                    return ssbm_rs_local(&format!("lockstep/unlock-sessions/{o}.inputs")).into();
                }
                if name.starts_with("tie") {
                    return ssbm_rs_local(&format!("lockstep/tie-sessions/{name}.inputs")).into();
                }
                if let Some(o) = name.strip_prefix("tou-") {
                    return ssbm_rs_local(&format!("lockstep/tournament-sessions/{o}.inputs"))
                        .into();
                }
                if name.starts_with("celebi") {
                    return ssbm_rs_local(&format!("lockstep/celebi-sessions/{name}.inputs"))
                        .into();
                }
                let idle = ssbm_rs_local(&format!("lockstep/idle-sessions/{name}.inputs"));
                let rules = ssbm_rs_local(&format!("lockstep/rules-sessions/{name}.inputs"));
                if std::path::Path::new(&idle).exists() {
                    idle.into()
                } else if std::path::Path::new(&rules).exists() {
                    rules.into()
                } else {
                    ssbm_rs_local(&format!("lockstep/menu-sessions/{name}.inputs")).into()
                }
            }
            Replay::Local(path) => ssbm_rs_local(path).into(),
            Replay::Slippi(rel, var) => var
                .and_then(std::env::var_os)
                .map_or_else(|| slippi_replays(rel).into(), PathBuf::from),
        }
    }

    fn slippi(self) -> bool {
        matches!(self, Replay::Slippi(..) | Replay::Hf(_))
    }

    /// How many frames ssbm-rs's log frame N comes after the native frame it draws: a
    /// replay's match starts its frames 3 before ssbm-rs's log does; a session's native frames
    /// count from its first render, the log's from power-on's first copy.
    fn log_offset(self) -> u32 {
        match self {
            Replay::Session(_) => SESSION_LOG_OFFSET,
            _ => 3,
        }
    }
}

/// A match the checks take frames of.
#[derive(Clone, Copy)]
pub struct Game {
    /// The stage, as the dashboard groups checks.
    pub stage: &'static str,
    /// The references' folder, `ref-NAME`.
    pub name: &'static str,
    /// The players, for the dashboard.
    pub players: &'static str,
    pub replay: Replay,
}

/// The standard set (the pre-push frames+audio step) or the opt-in sweep.
#[derive(Clone, Copy, PartialEq)]
pub enum Group {
    Standard,
    Sweep,
}

impl Group {
    fn name(self) -> &'static str {
        match self {
            Group::Standard => "standard",
            Group::Sweep => "sweep",
        }
    }
}

/// What a frame's two images against ssbm-rs's must be.
pub enum Expect {
    /// Identical to ssbm-rs's: every draw and copy the same, PSNR infinite, nothing noted as
    /// not drawn exactly.
    Identical,
    /// Identical images and nothing noted as not drawn exactly, but `same` events of each
    /// frame comparing the same, as recorded: a difference only the comparison sees (docs/
    /// player.md's open issues).
    SameImages { same: [usize; 2], why: &'static str },
    /// A known difference, docs/player.md's open issues: the images' lines exactly as
    /// recorded, and what melee-render notes it didn't draw exactly.
    Known {
        images: [&'static str; 2],
        notes: &'static [&'static str],
        why: &'static str,
    },
}

fn refs() -> PathBuf {
    std::env::var_os("REFS").map_or_else(|| ssbm_rs_local("").into(), PathBuf::from)
}

fn results_dir() -> PathBuf {
    std::env::var_os("RENDER_CHECK_RESULTS").map_or_else(
        || PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("render-check-results"),
        PathBuf::from,
    )
}

fn disc() -> String {
    std::env::var("SSBM_DISC").expect("SSBM_DISC names the disc")
}

/// The check's command, with only the variables the check means set.
fn command(game: &Game, id: &str) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_melee-render-check"));
    if game.replay.slippi() {
        cmd.env("CHECK_FP", "slippi");
    } else {
        cmd.env_remove("CHECK_FP");
    }
    for v in [
        "CHECK_DRAW_LIMIT",
        "CHECK_THREADED",
        "CHECK_EVENT",
        "CHECK_ALL_BP",
    ] {
        cmd.env_remove(v);
    }
    cmd.env("CHECK_RESULTS", results_dir()).env("CHECK_ID", id);
    cmd
}

/// The check's result, for the dashboard: its status file beside what the command recorded.
#[allow(clippy::too_many_arguments)]
fn status(
    id: &str,
    game: &Game,
    frame: u32,
    group: Group,
    kind: &str,
    expect: &str,
    why: &str,
    failure: Option<&str>,
    seconds: f64,
) {
    let (result, detail) = match failure {
        None if why.is_empty() => ("pass", String::new()),
        None => ("known", String::new()),
        Some(f) => ("fail", f.lines().next().unwrap_or("").to_owned()),
    };
    let text = format!(
        "stage\t{}\nmatch\t{}\nframe\t{frame}\ngroup\t{}\nkind\t{kind}\nexpect\t{expect}\nwhy\t{why}\nresult\t{result}\ndetail\t{detail}\nseconds\t{seconds:.1}\n",
        game.stage,
        game.players,
        group.name()
    );
    let dir = results_dir();
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join(format!("{id}.status")), text);
}

/// How a reference log was captured.
#[derive(Clone, Copy, PartialEq)]
pub enum Capture {
    /// ssbm-rs run without its renderer (`ref-GAME/`): its EFB copies leave memory as it
    /// was, so a copy made before the log starts isn't in the log's memory.
    Headless,
    /// ssbm-rs run with its renderer (`GX_RENDER=-`, `ref-GAME/rendered/`), whose copies
    /// write memory as the console's GPU does.
    Rendered,
}

/// Runs `compare` on the log of the game's frame `frame` (log frame N is native frame N - 3)
/// and checks the result.
pub fn compare(game: &Game, frame: u32, group: Group, expect: Expect) {
    compare_in(game, frame, group, expect, Capture::Headless);
}

/// [`compare`] against a log captured as `capture` says.
pub fn compare_in(game: &Game, frame: u32, group: Group, expect: Expect, capture: Capture) {
    let mut dir = refs().join("player").join(format!("ref-{}", game.name));
    if capture == Capture::Rendered {
        dir = dir.join("rendered");
    }
    let log = dir.join(format!("f{frame}.dff"));
    let images = dir.join(format!("f{frame}"));
    let replay = game.replay.path();
    for p in [&replay, &log, &images] {
        assert!(
            p.exists(),
            "{} is missing (set SSBM_RS_LOCAL or REFS, and SLIPPI_REPLAYS)",
            p.display()
        );
    }
    let id = format!("{}-f{frame:05}", game.name);
    let native = frame - game.replay.log_offset();
    let started = Instant::now();
    // The log's first frame is native frame N - 3 for a replay (asserted below), so a search of
    // two frames either way finds it. A session's frames can't be told apart by where their
    // draws sit (the movie's one sprite), so it takes the frame as it stands.
    let search = if matches!(game.replay, Replay::Session(_)) {
        "0"
    } else {
        "2"
    };
    let out = command(game, &id)
        .env("CHECK_SEARCH", search)
        // The rendered capture's copies wrote memory, so the native side's are written there too.
        .env(
            "CHECK_COPIES",
            if capture == Capture::Rendered {
                "memory"
            } else {
                ""
            },
        )
        .arg("compare")
        .arg(disc())
        .arg(&replay)
        .arg(&log)
        .arg(&images)
        .arg(native.to_string())
        .output()
        .expect("melee-render-check runs");
    let seconds = started.elapsed().as_secs_f64();
    let text = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<&str> = text.lines().collect();
    let aligned = format!("the log's first frame is native frame {native} (");
    let notes: Vec<&str> = lines
        .iter()
        .filter(|l| l.trim_start().starts_with("not drawn exactly"))
        .map(|l| l.trim())
        .collect();
    let found: Vec<&str> = lines
        .iter()
        .filter(|l| l.starts_with("  image:") || l.starts_with("  no "))
        .map(|l| l.trim())
        .collect();
    // Each frame's events against the log's, and how many compared the same: identical
    // draws pair every event and find no difference in any.
    let counts: Vec<(usize, usize)> = lines
        .iter()
        .filter_map(|l| {
            let (ours, rest) = l.trim().split_once(" events, the log's ")?;
            Some((ours.parse().ok()?, rest.parse().ok()?))
        })
        .collect();
    let same: Vec<usize> = lines
        .iter()
        .filter_map(|l| l.trim().strip_suffix(" events the same")?.parse().ok())
        .collect();
    let draws_identical = counts.len() == 2
        && same.len() == 2
        && counts
            .iter()
            .zip(&same)
            .all(|(&(ours, theirs), &same)| ours == theirs && same == ours);
    let name = format!("{} f{frame}", game.name);
    let failure = if !lines.iter().any(|l| l.starts_with(&aligned)) {
        Some(format!(
            "{name}: the log doesn't start at native frame {native}\n{text}{}",
            String::from_utf8_lossy(&out.stderr)
        ))
    } else {
        match &expect {
            Expect::Identical if !notes.is_empty() => Some(format!(
                "{name}: melee-render left things undrawn: {notes:#?}\n{text}"
            )),
            Expect::Identical if !draws_identical => Some(format!(
                "{name}: draws differ from the log's (events {counts:?}, the same {same:?})
{text}"
            )),
            Expect::Identical
                if !(found.len() == 2
                    && found.iter().all(|l| {
                        l.starts_with("image: PSNR inf dB,") && l.ends_with("within tolerance")
                    })
                    && out.status.success()) =>
            {
                Some(format!("{name}: not identical: {found:#?}\n{text}"))
            }
            Expect::SameImages { same: want, .. }
                if !(notes.is_empty()
                    && found.len() == 2
                    && found.iter().all(|l| {
                        l.starts_with("image: PSNR inf dB,") && l.ends_with("within tolerance")
                    })
                    && same == want) =>
            {
                Some(format!(
                    "{name}: not as recorded: images {found:#?}, notes {notes:#?}, events the same {same:?} (recorded {want:?})
{text}"
                ))
            }
            Expect::Known {
                images,
                notes: want,
                ..
            } if found != images || notes != *want => Some(format!(
                "{name}: not as recorded: images {found:#?} (recorded {images:#?}), notes {notes:#?} (recorded {want:#?})\n{text}"
            )),
            _ => None,
        }
    };
    let (expect_text, why) = match &expect {
        Expect::Identical => ("identical to ssbm-rs's frames", ""),
        Expect::SameImages { why, .. } => ("identical images, known draw difference", *why),
        Expect::Known { why, .. } => ("known difference, as recorded", *why),
    };
    status(
        &id,
        game,
        frame,
        group,
        "compare",
        expect_text,
        why,
        failure.as_deref(),
        seconds,
    );
    if let Some(f) = failure {
        panic!("{f}");
    }
}

/// Runs `scaled` on the game's native frame `frame - 3` (the frame `compare` checks as log
/// frame `frame`): drawn at 2x and 3x and box-downsampled, and widened to 16:9, against
/// melee-render's own frame at 1x, within the scaled tolerance.
pub fn scaled(game: &Game, frame: u32, group: Group) {
    scaled_as(game, frame, group, None);
}

/// [`scaled`] on a frame with a known difference between the scales: its four lines exactly as
/// recorded, within the tolerance or not.
pub fn scaled_known(
    game: &Game,
    frame: u32,
    group: Group,
    lines: [&'static str; 4],
    why: &'static str,
) {
    scaled_as(game, frame, group, Some((lines, why)));
}

fn scaled_as(
    game: &Game,
    frame: u32,
    group: Group,
    known: Option<([&'static str; 4], &'static str)>,
) {
    let replay = game.replay.path();
    assert!(
        replay.exists(),
        "{} is missing (set SLIPPI_REPLAYS)",
        replay.display()
    );
    let id = format!("{}-f{frame:05}-scaled", game.name);
    let started = Instant::now();
    let out = command(game, &id)
        .arg("scaled")
        .arg(disc())
        .arg(&replay)
        .arg((frame - game.replay.log_offset()).to_string())
        .output()
        .expect("melee-render-check runs");
    let seconds = started.elapsed().as_secs_f64();
    let text = String::from_utf8_lossy(&out.stdout);
    let measured: Vec<&str> = text
        .lines()
        .filter(|l| l.starts_with("  scaled "))
        .map(str::trim)
        .collect();
    let bad = match known {
        None => !out.status.success() || measured.len() != 4,
        Some((lines, _)) => measured != lines,
    };
    let failure = bad.then(|| {
        format!(
            "{} f{frame}: scaled frames not as expected: {measured:#?}\n{text}{}",
            game.name,
            String::from_utf8_lossy(&out.stderr)
        )
    });
    let (expect, why) = match known {
        None => (
            "2x, 3x and 16:9 against 1x, within the scaled tolerance",
            "",
        ),
        Some((_, why)) => ("known difference between the scales, as recorded", why),
    };
    status(
        &id,
        game,
        frame,
        group,
        "scaled",
        expect,
        why,
        failure.as_deref(),
        seconds,
    );
    if let Some(f) = failure {
        panic!("{f}");
    }
}

/// Runs `reflection` on the game's native frame `frame - 3` widened to 16:9: every vertex of
/// Fountain of Dreams' water on screen, the added sides' too, takes coordinates inside the
/// reflection's image (docs/player.md, **Copies in widescreen**).
pub fn reflection(game: &Game, frame: u32, group: Group) {
    let replay = game.replay.path();
    assert!(
        replay.exists(),
        "{} is missing (set SLIPPI_REPLAYS)",
        replay.display()
    );
    let id = format!("{}-f{frame:05}-reflection", game.name);
    let started = Instant::now();
    let out = command(game, &id)
        .arg("reflection")
        .arg(disc())
        .arg(&replay)
        .arg((frame - game.replay.log_offset()).to_string())
        .output()
        .expect("melee-render-check runs");
    let seconds = started.elapsed().as_secs_f64();
    let text = String::from_utf8_lossy(&out.stdout);
    let failure = (!out.status.success()).then(|| {
        format!(
            "{} f{frame}: the reflection leaves its image in widescreen\n{text}{}",
            game.name,
            String::from_utf8_lossy(&out.stderr)
        )
    });
    status(
        &id,
        game,
        frame,
        group,
        "reflection",
        "16:9: the water's coordinates inside the reflection's image",
        "",
        failure.as_deref(),
        seconds,
    );
    if let Some(f) = failure {
        panic!("{f}");
    }
}

/// How far a session's log frames run ahead of its native frames ([`Replay::log_offset`]):
/// ssbm-rs's log frame 300 of `idle-movie` is its native frame 299 (the movie's frames,
/// each shown two ticks, match there and at 300 against 301 only).
const SESSION_LOG_OFFSET: u32 = 1;

/// The opening movie played whole, nothing pressed (`idle-movie`): the movie's frames, the
/// title's mark and logo flash over it.
pub const IDLE_MOVIE: Game = Game {
    stage: "Opening movie",
    name: "idle-movie",
    players: "the opening movie (session idle-movie)",
    replay: Replay::Session("idle-movie"),
};

/// The title screen left idle to its attract match (`idle-title`).
pub const IDLE_TITLE: Game = Game {
    stage: "Title",
    name: "idle-title",
    players: "the title screen (session idle-title)",
    replay: Replay::Session("idle-title"),
};

/// The main menu's lists toured from the title (`menu-tour`): the main list's cursor, the C
/// stick turning the camera, the records' and Special Melee's lists.
pub const MENU_TOUR: Game = Game {
    stage: "Main menu",
    name: "menu-tour",
    players: "the main menu (session menu-tour)",
    replay: Replay::Session("menu-tour"),
};

/// The Data menu's movies (`gallery`): the Special Movie and How to Play played in the menu.
pub const GALLERY: Game = Game {
    stage: "Main menu",
    name: "gallery",
    players: "the Data menu's movies (session gallery)",
    replay: Replay::Session("gallery"),
};

/// The VS rules changed (`rules`): the mode, the stock count, the extra rules hovered.
pub const RULES: Game = Game {
    stage: "Main menu",
    name: "rules",
    players: "the VS rules (session rules)",
    replay: Replay::Session("rules"),
};

/// The item switch (`rules-items`): items toggled, the frequency hovered.
pub const RULES_ITEMS: Game = Game {
    stage: "Main menu",
    name: "rules-items",
    players: "the item switch (session rules-items)",
    replay: Replay::Session("rules-items"),
};

/// The extra rules and the random stage switch (`rules-plus`).
pub const RULES_PLUS: Game = Game {
    stage: "Main menu",
    name: "rules-plus",
    players: "the extra rules and the stage switch (session rules-plus)",
    replay: Replay::Session("rules-plus"),
};

/// Name entry (`names`): the keyboard, a random name, the erase prompt.
pub const NAMES: Game = Game {
    stage: "Main menu",
    name: "names",
    players: "name entry (session names)",
    replay: Replay::Session("names"),
};

/// Name entry's kana (`names-kana`): a glyph's pop-up, the katakana keys.
pub const NAMES_KANA: Game = Game {
    stage: "Main menu",
    name: "names-kana",
    players: "name entry's kana (session names-kana)",
    replay: Replay::Session("names-kana"),
};

/// The Options menus toured (`options-sessions/tour`): the rumble screen's ports and names,
/// the sound screen's balance, the screen options with the deflicker filter off, the language
/// screen and erasing data.
/// The Data menu's sessions (`data-sessions/NAME`): the special messages, the bonuses, the
/// other records, the VS records' three pages, and two special messages announced.
pub const DATA_INFO: Game = Game {
    stage: "Main menu",
    name: "data-info",
    players: "the special messages (session data-sessions/info)",
    replay: Replay::Session("data-info"),
};
pub const DATA_BONUS: Game = Game {
    stage: "Main menu",
    name: "data-bonus",
    players: "the bonuses (session data-sessions/bonus)",
    replay: Replay::Session("data-bonus"),
};
pub const DATA_COUNT: Game = Game {
    stage: "Main menu",
    name: "data-count",
    players: "the other records (session data-sessions/count)",
    replay: Replay::Session("data-count"),
};
pub const DATA_DIAGRAM: Game = Game {
    stage: "Main menu",
    name: "data-diagram",
    players: "the VS records (session data-sessions/diagram)",
    replay: Replay::Session("data-diagram"),
};
pub const DATA_PRIZE: Game = Game {
    stage: "Special messages",
    name: "data-prize",
    players: "two special messages announced (session data-sessions/prize)",
    replay: Replay::Session("data-prize"),
};
/// The album on a card of seven snapshots (`data-sessions/album-browse`): the slots, its pages
/// of thumbnails, the snapshot's menu and its view.
pub const DATA_ALBUM: Game = Game {
    stage: "Main menu",
    name: "data-album-browse",
    players: "the album's pages and a snapshot's view (session data-sessions/album-browse)",
    replay: Replay::Session("data-album-browse"),
};
/// The album's delete (`data-sessions/album-delete`): its question.
pub const DATA_ALBUM_DELETE: Game = Game {
    stage: "Main menu",
    name: "data-album-delete",
    players: "the album's delete question (session data-sessions/album-delete)",
    replay: Replay::Session("data-album-delete"),
};

pub const OPTIONS_TOUR: Game = Game {
    stage: "Main menu",
    name: "options-tour",
    players: "the Options menus (session options-sessions/tour)",
    replay: Replay::Session("options-tour"),
};

/// The debug menu (`debug-sessions/menu-enter`, at debug level 3): its first page.
pub const DEBUG_MENU_ENTER: Game = Game {
    stage: "Debug menu",
    name: "debug-menu-enter",
    players: "the debug menu's first page (session debug-sessions/menu-enter)",
    replay: Replay::Session("debug-menu-enter"),
};

/// The debug menu's submenus (`debug-sessions/menu-sub`): Global Data Edit over the first
/// page.
pub const DEBUG_MENU_SUB: Game = Game {
    stage: "Debug menu",
    name: "debug-menu-sub",
    players: "the debug menu's Global Data Edit (session debug-sessions/menu-sub)",
    replay: Replay::Session("debug-menu-sub"),
};

/// The sound test's debug mode (`debug-sessions/sound-debug`): its menu.
pub const DEBUG_SOUND: Game = Game {
    stage: "Debug menu",
    name: "debug-sound-debug",
    players: "the sound test's debug mode (session debug-sessions/sound-debug)",
    replay: Replay::Session("debug-sound-debug"),
};

/// The debug VS match's develop tools (`debug-sessions/vs-tools`): Link and Mario on Final
/// Destination with the computer players', the animations', the bonuses' and the sound's
/// readouts.
pub const DEBUG_VS_TOOLS: Game = Game {
    stage: "Final Destination",
    name: "debug-vs-tools",
    players: "Link vs Mario, the debug VS match's readouts (session debug-sessions/vs-tools)",
    replay: Replay::Session("debug-vs-tools"),
};

/// The develop tools' collision bubbles (`debug-sessions/vs-bubbles`): Link and Mario on Final
/// Destination, their hurt capsules and hits, a shield's bubble, alone and over the models.
pub const DEBUG_VS_BUBBLES: Game = Game {
    stage: "Final Destination",
    name: "debug-vs-bubbles",
    players: "Link vs Mario, the debug VS match's collision bubbles (session debug-sessions/vs-bubbles)",
    replay: Replay::Session("debug-vs-bubbles"),
};

/// The develop tools' fighter displays (`debug-sessions/vs-displays`): Fox, Ness and a
/// computer's Link on Final Destination, their bubbles alone and each display R and D-pad left
/// steps to.
pub const DEBUG_VS_DISPLAYS: Game = Game {
    stage: "Final Destination",
    name: "debug-vs-displays",
    players: "Fox vs Ness vs a computer's Link, the debug VS match's fighter displays (session debug-sessions/vs-displays)",
    replay: Replay::Session("debug-vs-displays"),
};

/// The stage's displays (`debug-sessions/vs-stage`): Link and Mario on Battlefield, the zones,
/// camera boxes and special points, the collisions by kind, by terrain and by ledge.
pub const DEBUG_VS_STAGE: Game = Game {
    stage: "Battlefield",
    name: "debug-vs-stage",
    players: "Link vs Mario, the debug VS match's stage displays (session debug-sessions/vs-stage)",
    replay: Replay::Session("debug-vs-stage"),
};

/// The coins' displays (`debug-sessions/vs-coins`): a Coin Mode debug VS match, Link and Mario
/// on Final Destination, the fighters' coin capsules and the coins' ranges.
pub const DEBUG_VS_COINS: Game = Game {
    stage: "Final Destination",
    name: "debug-vs-coins",
    players: "Link vs Mario in Coin Mode, the coins' displays (session debug-sessions/vs-coins)",
    replay: Replay::Session("debug-vs-coins"),
};

/// The sound test's songs (`sound-sessions/songs`): a song playing, its name and number.
pub const SOUND_SONGS: Game = Game {
    stage: "Main menu",
    name: "sound-songs",
    players: "the sound test's songs (session sound-sessions/songs)",
    replay: Replay::Session("sound-songs"),
};

/// The sound test's categories (`sound-sessions/sounds`): a category's icon, and a category's
/// sounds with the sound's number.
pub const SOUND_SOUNDS: Game = Game {
    stage: "Main menu",
    name: "sound-sounds",
    players: "the sound test's categories (session sound-sessions/sounds)",
    replay: Replay::Session("sound-sounds"),
};

/// The sound test in Japanese (`sound-sessions/jp`): a song's name in Japanese.
pub const SOUND_JP: Game = Game {
    stage: "Main menu",
    name: "sound-jp",
    players: "the sound test in Japanese (session sound-sessions/jp)",
    replay: Replay::Session("sound-jp"),
};

/// Tournament mode's sessions (`tournament-sessions/NAME`): the setup's options and its
/// bracket preview, the bracket, the entrants' list and the match-up screen.
pub const TOU_BRACKET64: Game = Game {
    stage: "Tournament",
    name: "tou-bracket64",
    players: "a bracket of 64, four a match (session tournament-sessions/bracket64)",
    replay: Replay::Session("tou-bracket64"),
};
pub const TOU_LIST: Game = Game {
    stage: "Tournament",
    name: "tou-list",
    players: "the entrants' list of eight (session tournament-sessions/list)",
    replay: Replay::Session("tou-list"),
};
pub const TOU_WINNER: Game = Game {
    stage: "Tournament",
    name: "tou-winner",
    players: "winner-out's match-up screen (session tournament-sessions/winner)",
    replay: Replay::Session("tou-winner"),
};
/// Celebi out of a Poké Ball on Fountain of Dreams (`celebi-sessions/celebi1`): sparkling
/// above the left platform, before it flies off.
pub const CELEBI1: Game = Game {
    stage: "Fountain of Dreams",
    name: "celebi1",
    players: "Fox and three computer players, Celebi out of a Poké Ball (session celebi-sessions/celebi1)",
    replay: Replay::Session("celebi1"),
};

/// The character select with a random computer player and the random stage
/// (`select-sessions/random`): a door opening under a hand holding its coin, Mario's
/// costumes, the computer player's door, the stage select's random icon and its pick.
pub const SELECT_RANDOM: Game = Game {
    stage: "Main menu",
    name: "select-random",
    players: "the character and stage selects (session select-sessions/random)",
    replay: Replay::Session("select-random"),
};

/// Three computer players' levels set on their sliders (`select-sessions/cpus`).
pub const SELECT_CPUS: Game = Game {
    stage: "Main menu",
    name: "select-cpus",
    players: "computer players' levels (session select-sessions/cpus)",
    replay: Replay::Session("select-cpus"),
};

/// A team battle with handicaps (`select-sessions/teams`): the teams' doors, a computer
/// player's door with both sliders, a handicap moved.
pub const SELECT_TEAMS: Game = Game {
    stage: "Main menu",
    name: "select-teams",
    players: "a team battle with handicaps (session select-sessions/teams)",
    replay: Replay::Session("select-teams"),
};

/// The stage select with Kongo Jungle hovered: its name and emblem (`select-sessions/
/// stage-kongo`).
pub const SELECT_KONGO: Game = Game {
    stage: "Main menu",
    name: "select-stage-kongo",
    players: "the stage select's preview (session select-sessions/stage-kongo)",
    replay: Replay::Session("select-stage-kongo"),
};

/// A one-stock match's results (`results-sessions/stock`): the panel coming in, Mario's
/// second pose, the rows and portraits, the statistics' pages turned and scrolled.
pub const RESULTS_STOCK: Game = Game {
    stage: "Results",
    name: "results-stock",
    players: "a stock match's results (session results-sessions/stock)",
    replay: Replay::Session("results-stock"),
};

/// A team battle's results (`results-sessions/teams`): three teams' rows, Ness's bat pose,
/// the KOs page.
pub const RESULTS_TEAMS: Game = Game {
    stage: "Results",
    name: "results-teams",
    players: "a team battle's results (session results-sessions/teams)",
    replay: Replay::Session("results-teams"),
};

/// A one-stock match's results won by Dr. Mario (`results-sessions/pills`): his second pose
/// with his two pills in hand.
pub const RESULTS_PILLS: Game = Game {
    stage: "Results",
    name: "results-pills",
    players: "Dr. Mario's results pose with his pills (session results-sessions/pills)",
    replay: Replay::Session("results-pills"),
};

/// The character select after a three-way tie for second under the automatic handicap
/// (`tie-sessions/tie3-time`): the winner's handicap moved by a stack byte the retrace count
/// indexes.
pub const TIE3_TIME: Game = Game {
    stage: "Character select",
    name: "tie-tie3-time",
    players: "the handicaps after a tie for second (session tie-sessions/tie3-time)",
    replay: Replay::Session("tie3-time"),
};

/// The character select after a tie for second of three under the automatic handicap
/// (`tie-sessions/tie4-stock`): the winner's and port 3's handicaps moved.
pub const TIE4_STOCK: Game = Game {
    stage: "Character select",
    name: "tie-tie4-stock",
    players: "the handicaps after a three-way tie for second (session tie-sessions/tie4-stock)",
    replay: Replay::Session("tie4-stock"),
};

/// The challenger approach's screen (`unlock-sessions/time`): "A new foe has appeared!" with
/// Mewtwo's name, as it comes in and as it waits for A.
pub const UNLOCK_TIME: Game = Game {
    stage: "Challenger approach",
    name: "unlock-time",
    players: "the challenger approach's screen (session unlock-sessions/time)",
    replay: Replay::Session("unlock-time"),
};

/// The challenger approach's screen for the last challenger (`unlock-sessions/gw`): "A new
/// foe has appeared!" with Mr. Game & Watch's name, waiting for A.
pub const UNLOCK_GW: Game = Game {
    stage: "Challenger approach",
    name: "unlock-gw",
    players: "the challenger approach's screen (session unlock-sessions/gw)",
    replay: Replay::Session("unlock-gw"),
};

/// Camera Mode (`special-sessions/camera-snaps`): the card check, the match with the
/// camera's info, the snapshot's offer and its question, the save.
pub const CAMERA_SNAPS: Game = Game {
    stage: "Main menu",
    name: "special-camera-snaps",
    players: "Camera Mode (session special-sessions/camera-snaps)",
    replay: Replay::Session("special-camera-snaps"),
};

/// Camera Mode with no card (`special-sessions/camera-nocard`).
pub const CAMERA_NOCARD: Game = Game {
    stage: "Main menu",
    name: "special-camera-nocard",
    players: "Camera Mode, no card (session special-sessions/camera-nocard)",
    replay: Replay::Session("special-camera-nocard"),
};

/// The trophy gallery round its six lights (`trophy-sessions/gallery-lights`): the view, the
/// lights' backs and the background's sprites, the close-up with its axes, the list.
pub const TROPHY_GALLERY: Game = Game {
    stage: "Trophies",
    name: "trophy-gallery-lights",
    players: "the trophy gallery (session trophy-sessions/gallery-lights)",
    replay: Replay::Session("trophy-gallery-lights"),
};

/// The trophy lottery's draws (`trophy-sessions/lottery-few`): the machine, the coins, the
/// draw's capsule and the trophy on its stand with its name.
pub const TROPHY_LOTTERY: Game = Game {
    stage: "Trophies",
    name: "trophy-lottery-few",
    players: "the trophy lottery (session trophy-sessions/lottery-few)",
    replay: Replay::Session("trophy-lottery-few"),
};

/// The trophy collection with 196 trophies (`trophy-sessions/coll-many`, the rings, and
/// `coll-l`, the rows).
pub const TROPHY_COLLECTION: Game = Game {
    stage: "Trophies",
    name: "trophy-coll-many",
    players: "the trophy collection (session trophy-sessions/coll-many)",
    replay: Replay::Session("trophy-coll-many"),
};

pub const TROPHY_COLLECTION_ROWS: Game = Game {
    stage: "Trophies",
    name: "trophy-coll-l",
    players: "the trophy collection in rows (session trophy-sessions/coll-l)",
    replay: Replay::Session("trophy-coll-l"),
};

pub const FOX_FOX_FD: Game = Game {
    stage: "Final Destination",
    name: "fox-fox-fd",
    players: "Fox vs Fox (benchmark)",
    replay: Replay::Bench("fox-fox-fd"),
};

pub const FOX_MARTH_BF: Game = Game {
    stage: "Battlefield",
    name: "fox-marth-bf",
    players: "Fox vs Marth (benchmark)",
    replay: Replay::Bench("fox-marth-bf"),
};

pub const FOX_MARTH_FD: Game = Game {
    stage: "Final Destination",
    name: "fox-marth-fd",
    players: "Fox vs Marth (benchmark)",
    replay: Replay::Bench("fox-marth-fd"),
};

/// Fox vs Sheik and Zelda on Final Destination, a Slippi replay whose references keep
/// Slippi's name plates on.
pub const FOX_SHEIK_FD: Game = Game {
    stage: "Final Destination",
    name: "fox-sheik-fd",
    players: "Fox vs Sheik (Slippi, name plates on)",
    replay: Replay::Slippi("2026-01/Game_20260129T133652.slp", Some("SLIPPI_FOX_SHEIK")),
};

pub const FOX_MARTH_PS: Game = Game {
    stage: "Pokémon Stadium",
    name: "fox-marth-ps",
    players: "Fox vs Marth (benchmark)",
    replay: Replay::Bench("fox-marth-ps"),
};

/// Flat Zone's oracle match: its flat fighters' normals take the fighters' joint class's
/// depth scale (`ftParts_JObjMakePositionMtx`).
pub const FLAT_ZONE: Game = Game {
    stage: "Flat Zone",
    name: "stage-flat-zone",
    players: "Young Link vs Jigglypuff (oracle match)",
    replay: Replay::Oracle("stage-flat-zone"),
};

/// Poké Floats' oracle match: the floats draw with the toon texture their render callback
/// sets around their draw (`fn_802130D0`), an image in the executable (`grPu_803E7620`).
pub const POKE_FLOATS: Game = Game {
    stage: "Poké Floats",
    name: "stage-poke-floats",
    players: "Ness vs Yoshi (oracle match)",
    replay: Replay::Oracle("stage-poke-floats"),
};

/// One of the stages set's oracle matches, `stage-KEY`: its references are in
/// `ref-stage-KEY`.
const fn stage(stage: &'static str, name: &'static str, players: &'static str) -> Game {
    Game {
        stage,
        name,
        players,
        replay: Replay::Oracle(name),
    }
}

pub const BIG_BLUE: Game = stage("Big Blue", "stage-big-blue", "Roy vs Zelda (oracle match)");
pub const BRINSTAR: Game = stage(
    "Brinstar",
    "stage-brinstar",
    "Donkey Kong vs Young Link (oracle match)",
);
pub const BRINSTAR_DEPTHS: Game = stage(
    "Brinstar Depths",
    "stage-brinstar-depths",
    "Dr. Mario vs Ganondorf (oracle match)",
);
pub const CORNERIA: Game = stage(
    "Corneria",
    "stage-corneria",
    "Falco vs Sheik (oracle match)",
);
pub const FOURSIDE: Game = stage("Fourside", "stage-fourside", "Fox vs Marth (oracle match)");
pub const GREAT_BAY: Game = stage(
    "Great Bay",
    "stage-great-bay",
    "Roy vs Zelda (oracle match)",
);
pub const GREEN_GREENS: Game = stage(
    "Green Greens",
    "stage-green-greens",
    "Bowser vs Pichu (oracle match)",
);
pub const HYRULE_TEMPLE: Game = stage(
    "Hyrule Temple",
    "stage-hyrule-temple",
    "Samus vs Luigi (oracle match)",
);
pub const ICICLE_MOUNTAIN: Game = stage(
    "Icicle Mountain",
    "stage-icicle-mountain",
    "Dr. Mario vs Ganondorf (oracle match)",
);
pub const JUNGLE_JAPES: Game = stage(
    "Jungle Japes",
    "stage-jungle-japes",
    "Mario vs Pikachu (oracle match)",
);
pub const KONGO_JUNGLE: Game = stage(
    "Kongo Jungle",
    "stage-kongo-jungle",
    "Ness vs Yoshi (oracle match)",
);
pub const KONGO_JUNGLE_64: Game = stage(
    "Kongo Jungle N64",
    "stage-kongo-jungle-64",
    "Falco vs Sheik (oracle match)",
);
pub const MUSHROOM_KINGDOM: Game = stage(
    "Mushroom Kingdom",
    "stage-mushroom-kingdom",
    "Samus vs Luigi (oracle match)",
);
pub const MUSHROOM_KINGDOM_2: Game = stage(
    "Mushroom Kingdom II",
    "stage-mushroom-kingdom-2",
    "Mario vs Pikachu (oracle match)",
);
pub const MUTE_CITY: Game = stage(
    "Mute City",
    "stage-mute-city",
    "Bowser vs Pichu (oracle match)",
);
/// Mewtwo against Mr. Game & Watch, whose render callback draws an outline, him and a
/// silhouette in one pass.
pub const ONETT: Game = stage(
    "Onett",
    "stage-onett",
    "Mewtwo vs Mr. Game & Watch (oracle match)",
);
pub const PEACHS_CASTLE: Game = stage(
    "Princess Peach's Castle",
    "stage-peachs-castle",
    "Link vs Jigglypuff (oracle match)",
);
pub const RAINBOW_CRUISE: Game = stage(
    "Rainbow Cruise",
    "stage-rainbow-cruise",
    "Donkey Kong vs Young Link (oracle match)",
);
pub const VENOM: Game = stage(
    "Venom",
    "stage-venom",
    "Captain Falcon vs Peach (oracle match)",
);
pub const YOSHIS_ISLAND: Game = stage(
    "Yoshi's Island",
    "stage-yoshis-island",
    "Captain Falcon vs Peach (oracle match)",
);
pub const YOSHIS_ISLAND_64: Game = stage(
    "Yoshi's Island N64",
    "stage-yoshis-island-64",
    "Fox vs Marth (oracle match)",
);

/// Milestone 4's power-ups oracle matches, `items-powerups-*`: metal fighters (the Metal Box,
/// `ft_804D6580`) and cloaked ones (the Cloaking Device, refraction).
pub const ITEMS_POWERUPS_PS: Game = Game {
    stage: "Pokémon Stadium",
    name: "items-powerups-ps",
    players: "Mewtwo vs Mr. Game & Watch, power-ups (oracle match)",
    replay: Replay::Oracle("items-powerups-ps"),
};
pub const ITEMS_POWERUPS_BF: Game = Game {
    stage: "Battlefield",
    name: "items-powerups-bf",
    players: "Bowser vs Pichu, power-ups (oracle match)",
    replay: Replay::Oracle("items-powerups-bf"),
};
pub const ITEMS_POWERUPS_FD: Game = Game {
    stage: "Final Destination",
    name: "items-powerups-fd",
    players: "Roy vs Zelda, power-ups (oracle match)",
    replay: Replay::Oracle("items-powerups-fd"),
};

/// A four-player match recorded with the player (2026-10-24, `player/replays/
/// metal-cloak-rc.slp` in ssbm-rs's `local/`), where Mr. Game & Watch goes metal at frame 555
/// and takes a Cloaking Device at 659 while still metal: the match that showed him white.
pub const METAL_CLOAK_RC: Game = Game {
    stage: "Rainbow Cruise",
    name: "metal-cloak-rc",
    players: "Donkey Kong, Fox, Dr. Mario and Mr. Game & Watch (a player match)",
    replay: Replay::Local("player/replays/metal-cloak-rc.slp"),
};

/// Invisible Melee (milestone 5's Special Melee oracle set): both fighters refract the whole
/// match.
pub const SPECIAL_INVISIBLE_YS: Game = Game {
    stage: "Yoshi's Story",
    name: "special-invisible-ys",
    players: "Mario vs Luigi, Invisible Melee (oracle match)",
    replay: Replay::Local("bench/oracle-m5-special/special-invisible-ys.slp"),
};

pub const SHEIK_FOX_FOD: Game = Game {
    stage: "Fountain of Dreams",
    name: "sheik-fox-fod",
    players: "Sheik vs Fox (Slippi)",
    replay: Replay::Slippi("2026-01/Game_20260129T214216.slp", None),
};

/// Bowser vs Fox on Fountain of Dreams with a player's reflection size code (`04 801CCE4C`):
/// the reflection renders 640 by 480, its camera's viewport the whole screen.
pub const PC_IZUMI_SIZE: Game = Game {
    stage: "Fountain of Dreams",
    name: "pc-izumi-size",
    players: "Bowser vs Fox (Slippi, the reflection size code)",
    replay: Replay::Hf("bowser-dd-a6/slp/diamond-diamond-7eb9584915b81291725ce78a.slp"),
};

pub const FOX_SHEIK_YS: Game = Game {
    stage: "Yoshi's Story",
    name: "fox-sheik-ys",
    players: "Fox vs Sheik (Slippi)",
    replay: Replay::Slippi("2026-01/Game_20260129T133446.slp", None),
};

pub const FOX_SHEIK_DL: Game = Game {
    stage: "Dream Land",
    name: "fox-sheik-dl",
    players: "Fox vs Sheik (Slippi)",
    replay: Replay::Slippi("2026-01/Game_20260129T130802.slp", None),
};

/// Four players in teams on Final Destination: two Foxes, Jigglypuff and Sheik.
pub const FOUR_FD: Game = Game {
    stage: "Final Destination",
    name: "4p-fd",
    players: "Fox, Fox, Jigglypuff, Sheik (Slippi, teams)",
    replay: Replay::Slippi("2026-05/Game_20260531T174953.slp", None),
};

/// The known differences docs/player.md's open issues list, as the dashboard names them.
pub mod why {
    /// Fountain of Dreams' reflection, an 80 by 60 copy drawn at the scale and averaged back,
    /// magnified over the water: its blocks legitimately differ between scales.
    /// Pokémon Stadium's screen samples an RGB565 copy melee-gpu keeps on the GPU, which the
    /// comparison hashes from memory the copy never writes.
    pub const SCREEN_COPY: &str =
        "Pokémon Stadium's screen: a copy kept on the GPU, compared by memory it never writes";
    /// The fighters' shadows are copies melee-gpu keeps on the GPU, which the comparison
    /// hashes from memory the copy never writes; ssbm-rs's renderer writes them.
    pub const SHADOW_COPY: &str =
        "the fighters' shadows: copies kept on the GPU, compared by memory they never write";
    /// The results screen's images: copies kept on the GPU (RGB5A3, as ssbm-rs keeps them),
    /// which the comparison hashes from memory holding what their blocks held before, a
    /// few of whose bytes the heap's image doesn't know (fields of freed fighters, lights and
    /// an archive's header no layout covers): the draws sample the copies, and the images
    /// are identical.
    pub const RESULTS_COPY: &str = "the results' images: copies kept on the GPU, compared by         memory holding their blocks' leftovers, a few bytes of which the heap's image doesn't know";
    pub const REFLECTION_SCALE: &str = "Fountain of Dreams' reflection: an 80 by 60 copy drawn at each scale, magnified over the water";
}
