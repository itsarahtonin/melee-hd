//! What the frame checks share: running `melee-render-check compare` or `scaled` on a frame,
//! checking its result against what docs/player.md records, and leaving the result for the
//! dashboard.
//!
//! The references are ssbm-rs's FIFO logs and gxplay's frames of them,
//! `REFS/player/ref-GAME/fN.dff` and `fN/` (`REFS` defaults to ssbm-rs's `local/`,
//! `$SSBM_RS_LOCAL`), made as docs/player.md describes (Slippi's name plates off unless the
//! check says otherwise); the replays are the repository's benchmark matches and Slippi
//! replays in `$SLIPPI_REPLAYS` (the author's, not public). Unlike the tests that skip without
//! their data, these fail without it: they are a check to run on purpose.
//!
//! Each check writes what it measured to `RENDER_CHECK_RESULTS` (by default
//! `target/tmp/render-check-results`), which `melee-render-check dashboard DIR OUT.html`
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
}

impl Replay {
    fn path(self) -> PathBuf {
        match self {
            Replay::Bench(name) => melee_local_data::bench_replay(name),
            Replay::Slippi(rel, var) => var
                .and_then(std::env::var_os)
                .map_or_else(|| slippi_replays(rel).into(), PathBuf::from),
        }
    }

    fn slippi(self) -> bool {
        matches!(self, Replay::Slippi(..))
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
    /// Identical to ssbm-rs's: PSNR infinite, nothing noted as not drawn exactly.
    Identical,
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

/// Runs `compare` on the log of the game's frame `frame` (log frame N is native frame N - 3)
/// and checks the result.
pub fn compare(game: &Game, frame: u32, group: Group, expect: Expect) {
    let dir = refs().join("player").join(format!("ref-{}", game.name));
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
    let native = frame - 3;
    let started = Instant::now();
    // The log's first frame is native frame N - 3 (asserted below), so a search of two
    // frames either way finds it.
    let out = command(game, &id)
        .env("CHECK_SEARCH", "2")
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
            Expect::Identical
                if !(found.len() == 2
                    && found.iter().all(|l| {
                        l.starts_with("image: PSNR inf dB,") && l.ends_with("within tolerance")
                    })
                    && out.status.success()) =>
            {
                Some(format!("{name}: not identical: {found:#?}\n{text}"))
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
        .arg((frame - 3).to_string())
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

pub const SHEIK_FOX_FOD: Game = Game {
    stage: "Fountain of Dreams",
    name: "sheik-fox-fod",
    players: "Sheik vs Fox (Slippi)",
    replay: Replay::Slippi("2026-01/Game_20260129T214216.slp", None),
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
    /// Fountain of Dreams' stars are drawn as points whose size melee-render doesn't set.
    pub const STARS: &str = "Fountain of Dreams' stars: point size not drawn";
    /// A fighter's second lit vertex color, a few levels off on some draws.
    pub const VERTEX_COLORS: &str =
        "a fighter's second lit vertex color a few levels off; a few pixels";
    /// A magnifier bubble's background, the average of its corner colors rather than the
    /// blend the original makes by where the fighter is.
    pub const BUBBLE: &str =
        "a magnifier bubble's background: the corner colors' average, not the original's blend";
    /// A fighter texture's palette, different past the entries the frame's draws show.
    pub const PALETTE: &str = "a fighter texture's palette contents; a few pixels";
    /// Fountain of Dreams' water, a few levels greener than ssbm-rs's once the reflection shows.
    pub const WATER: &str = "Fountain of Dreams' water a few levels greener (the reflection copy), and the stars' point size";
    /// Pokémon Stadium's screen showing the stage's feed where ssbm-rs's is dark.
    pub const SCREEN: &str = "Pokémon Stadium's screen shows the feed where ssbm-rs's is dark";
    /// Fountain of Dreams' reflection, an 80 by 60 copy drawn at the scale and averaged back,
    /// magnified over the water: its blocks legitimately differ between scales.
    pub const REFLECTION_SCALE: &str = "Fountain of Dreams' reflection: an 80 by 60 copy drawn at each scale, magnified over the water";
}
