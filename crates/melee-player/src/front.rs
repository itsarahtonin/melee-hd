//! The player's front end (milestone 5's task 18, Sarah's call of 2026-10-28): melee-hd boots
//! straight into Melee, and the window's only screen of its own is the settings panel, which
//! Escape opens over the game, paused, and closes again. It holds what belongs to this
//! machine and the player rather than the game: the display, the latency, the ruleset Melee
//! starts with, the save's import, export and unlock-all, and recording; matches start from the
//! game's own menus. The panel
//! is egui on the player's own device and window ([`crate::ui`]).
//!
//! The settings are kept in the settings folder's `settings.txt` (`key = value` lines; an
//! older `last-match.txt`, the launcher's, is read when there's none yet), except those the
//! command line gives.
//!
//! The panel has three tabs (Sarah, 2026-11-14): Settings, the panel as it was; Quick match,
//! laid out until the Slippi ruleset can start its matches ([`crate::quick`]); and Recordings,
//! the replays in a folder, any of them played from start to finish ([`crate::recordings`]).

use melee_sim::game::state::Ruleset;

use crate::jit::Timing;
use crate::pacing::Mode;
use crate::present::{Aspect, Present};

/// The ruleset a player's Melee starts with until they pick one (design call 2 of
/// `docs/slippi-ruleset.md`): Slippi since the Slippi ruleset's milestone was done (Sarah,
/// 2026-11-22, after the spot check in Slippi Dolphin), Vanilla before. The one place the default
/// is set.
pub const DEFAULT_RULESET: Ruleset = Ruleset::Slippi;

/// The file that keeps the settings.
pub const FILE: &str = "settings.txt";
/// The launcher's file, read once for a settings file that isn't there yet.
const OLD_FILE: &str = "last-match.txt";

/// The settings file's version: none before 2026-11-14's new defaults (just-in-time ticks,
/// high refresh on), which a file without one moves to once ([`Settings::restore`]).
pub const SETTINGS_VERSION: u32 = 2;

/// The widest timing margin, in milliseconds.
pub const MAX_MARGIN_MS: f32 = 8.0;

/// A just-in-time margin in milliseconds, 0 to [`MAX_MARGIN_MS`].
pub fn parse_margin(v: &str) -> Option<f32> {
    v.parse()
        .ok()
        .filter(|m: &f32| (0.0..=MAX_MARGIN_MS).contains(m))
}

/// The settings the command line gave, which win over the kept ones.
#[derive(Clone, Copy, Default)]
pub struct Given {
    pub aspect: bool,
    pub fullscreen: bool,
    pub present: bool,
    pub added_delay: bool,
    pub high_refresh: bool,
    pub tick_timing: bool,
    pub jit_margin: bool,
    pub scale: bool,
    pub msaa: bool,
    pub renderer: bool,
    pub record: bool,
    pub replays: bool,
    pub compress_replays: bool,
    pub record_sessions: bool,
    pub debug_level: bool,
    pub audio_exclusive: bool,
    pub ruleset: bool,
}

/// The player's settings.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    /// The aspect ratio frames are shown at.
    pub aspect: Aspect,
    pub fullscreen: bool,
    /// The internal resolution's scale: 0 follows the window, else that many times the
    /// console's.
    pub scale: u32,
    /// Four samples a pixel.
    pub msaa: bool,
    /// Draws with the fast renderer (docs/fast-renderer.md) rather than the exact one.
    pub fast_renderer: bool,
    /// High refresh: on is Extrapolate (the panel's toggle); the command line can give
    /// Interpolate, which the file keeps as on.
    pub high_refresh: Mode,
    pub present: Present,
    /// Ticks each controller read is held back.
    pub added_delay: u32,
    pub tick_timing: Timing,
    /// The just-in-time tick's margin, in milliseconds.
    pub jit_margin: f32,
    /// Saves each VS match as a Slippi replay, to `replays` (empty: the settings folder's).
    pub record: bool,
    pub replays: String,
    /// Saves them compressed, as `.slpz` (design call 12 of docs/milestone-7.md: off by
    /// default; reading is always on).
    pub compress_replays: bool,
    /// Records each session from power-on (`session_rec`).
    pub record_sessions: bool,
    /// The developer setting: the debug level Melee's next power-on raises `DbLevel` to
    /// ([`DEBUG_LEVELS`]), 0 for retail's.
    pub debug_level: i32,
    /// The sound's volume in percent, the output device by name (empty: the system's default,
    /// followed as it changes) and whether it's taken exclusively (Windows).
    pub volume: u32,
    /// The music's volume within it, in percent (the mix scales the stream's voices).
    pub music: u32,
    pub audio_device: String,
    pub audio_exclusive: bool,
    /// Whether a sound effect starts a mix frame (5 ms) sooner where it can (`Mixer::set_early`).
    pub early_sfx: bool,
    /// The folder the Recordings tab lists (empty: the replays folder).
    pub recordings: String,
    /// The ruleset Melee's next power-on runs (the Game section's Ruleset): Vanilla, or Slippi's
    /// with no memory card ([`DEFAULT_RULESET`] until picked).
    pub ruleset: Ruleset,
    /// The Controllers tab's: the ports chosen and the bindings changed (`port` and `bindings`
    /// lines, [`melee_input::Config::save`]).
    pub controllers: melee_input::Config,
    /// Low-spec mode (the Display section's switch, [`LOW_SPEC`]): none while it's off; while
    /// it's on, the settings it replaced, each by its key with the value it had in the file's
    /// words (`low-spec-saved` lines), which [`Settings::low_spec_off`] puts back.
    pub low_spec: Option<Vec<(&'static str, String)>>,
}

/// A setting low-spec mode sets: its key in `settings.txt`, the value it sets in the file's
/// words, what its help line calls what it turns off, and how the setting is read and written
/// in the file's words (`set` refuses a value the file wouldn't take).
pub struct LowSpec {
    pub key: &'static str,
    pub low: &'static str,
    pub what: &'static str,
    get: fn(&Settings) -> String,
    set: fn(&mut Settings, &str) -> bool,
}

/// What low-spec mode sets (design call 3 of `docs/milestone-7.md`, Sarah 2026-11-27): what
/// costs the most to draw on a weak machine, in the Display section's order. Resolution 1x,
/// Antialiasing off, the fast renderer and High refresh off (no prediction, a frame a tick);
/// the latency settings, the audio and the game are untouched. Tasks 17's and 18's options
/// (texture filtering, supersampling, sharpening, the screen filter) join the list, off, as
/// they land: an entry here is all that's needed, the help line included.
pub const LOW_SPEC: [LowSpec; 4] = [
    LowSpec {
        key: "scale",
        low: "1",
        what: "resolutions above the console's",
        get: |s| match s.scale {
            0 => "auto".to_owned(),
            n => n.to_string(),
        },
        set: |s, v| match v {
            "auto" => {
                s.scale = 0;
                true
            }
            v => match v.parse().ok().filter(|n| (1..=8).contains(n)) {
                Some(n) => {
                    s.scale = n;
                    true
                }
                None => false,
            },
        },
    },
    LowSpec {
        key: "msaa",
        low: "off",
        what: "antialiasing",
        get: |s| if s.msaa { "4" } else { "off" }.to_owned(),
        set: |s, v| match v {
            "4" => {
                s.msaa = true;
                true
            }
            "off" | "1" => {
                s.msaa = false;
                true
            }
            _ => false,
        },
    },
    LowSpec {
        key: "renderer",
        low: "fast",
        what: "the exact renderer",
        get: |s| if s.fast_renderer { "fast" } else { "exact" }.to_owned(),
        set: |s, v| match v {
            "fast" | "exact" => {
                s.fast_renderer = v == "fast";
                true
            }
            _ => false,
        },
    },
    LowSpec {
        // The mode by its own name, so a command line's Interpolate comes back as it was.
        key: "high-refresh",
        low: "off",
        what: "high refresh",
        get: |s| s.high_refresh.key().to_owned(),
        set: |s, v| match Mode::parse(v) {
            Ok(m) => {
                s.high_refresh = m;
                true
            }
            Err(_) => false,
        },
    },
];

/// The Low-spec mode switch's help line (the plan's, its list from [`LOW_SPEC`]; proposed
/// 2026-11-28, awaiting Sarah's word).
pub fn low_spec_help() -> String {
    let what: Vec<&str> = LOW_SPEC.iter().map(|l| l.what).collect();
    let list = match what.split_last() {
        Some((last, [])) => (*last).to_owned(),
        Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
        None => String::new(),
    };
    format!(
        "Turns off what costs the most to draw: {list}. Turning it off puts your settings back."
    )
}

/// The developer setting's levels (Sarah, 2026-10-29: Off, Debug ROM and Develop): each
/// `DbLKind`, its name and what it turns on. Levels 1 and 2 only add the title's build date and
/// the disc cache's assertions, which can only stop the game.
pub const DEBUG_LEVELS: [(i32, &str, &str); 3] = [
    (0, "Off", "Melee as sold."),
    (
        3,
        "Debug ROM",
        "HAL's debug menu (Y at the title), debug VS (A) and the sound test's debug mode (X); in matches, the develop tools, the debug pause and the frame step.",
    ),
    (
        4,
        "Develop",
        "Debug ROM's tools and the develop level's extras: the effect-pool limits and the item counts' reset.",
    ),
];

/// The developer setting's help line.
pub const DEBUG_HELP: &str = "Raises Melee's debug level, as an Action Replay code does. Applies from Melee's next start (Restart Melee\u{2026}), and never to a replay.";
/// The warning while the debug level isn't Off.
pub const DEBUG_WARNING: &str = "The debug menu switches your save's language to Japanese as it opens (Melee's own bug), and its entries can change unlocks and records, which Melee saves to your card. Its Yoshiki Test page can erase your save and your Camera Mode snapshots; melee-hd keeps a copy of each in the backups folder. Some debug tools aren't in melee-hd yet and stop the game.";
/// The "Save replays" help line while the chosen ruleset is Vanilla, whose box is off and
/// greyed: only the Slippi ruleset saves replays (Sarah, 2026-11-19: "Slippi only"; this line
/// signed off 2026-11-20).
pub const REPLAYS_VANILLA: &str = "Slippi ruleset only: Slippi Dolphin can't play Vanilla's matches back exactly. Record sessions, below, keeps every session exactly.";
/// The note under Save replays while the chosen ruleset isn't the one Melee runs (proposed
/// 2026-11-20, awaiting Sarah's word).
pub const REPLAYS_NEXT_START: &str = "Applies from Melee's next start (Restart Melee\u{2026}).";

/// How the Recording section shows Save replays: it follows the ruleset chosen (the Game
/// section's, which Melee's next start runs), as the other next-start settings do, while what
/// saves follows the one Melee runs (`crate::saves_replays`). Whether the box can be changed,
/// its help line, and the note while the two differ.
fn replays_box(chosen: Ruleset, running: Ruleset) -> (bool, &'static str, Option<&'static str>) {
    let saves = crate::saves_replays(chosen);
    let help = if saves {
        REPLAYS_SLIPPI_HELP
    } else {
        REPLAYS_VANILLA
    };
    (
        saves,
        help,
        (chosen != running).then_some(REPLAYS_NEXT_START),
    )
}

/// The replays' compression's box and its line (milestone 7's task 14, proposed 2026-11-27).
pub const COMPRESS_LABEL: &str = "Compress replays (.slpz)";
pub const COMPRESS_HELP: &str =
    "Saves each replay about ten times smaller. Slippi's own tools may need it uncompressed first.";

/// The "Save replays" help line while Melee runs under the Slippi ruleset (approved 2026-11-18;
/// Slo-Mo Melee dropped from it as it became a live match, proposed the same day, awaiting
/// Sarah's word): the matches design call 6 of `docs/slippi-ruleset.md` leaves unrecorded,
/// named.
pub const REPLAYS_SLIPPI_HELP: &str = "VS matches' replays play back exactly in Slippi Dolphin. Sudden Death, Super Sudden Death, Stamina and Fixed-Camera Mode, a challenger's match, Camera Mode, Training and the single-player modes save none: Slippi Dolphin couldn't play them back.";
/// The Game section's rulesets (proposed 2026-11-17, awaiting Sarah's word): each, its name and
/// what it is.
pub const RULESETS: [(Ruleset, &str, &str); 2] = [
    (
        Ruleset::Vanilla,
        "Vanilla",
        "Melee as sold, with your save.",
    ),
    (
        Ruleset::Slippi,
        "Slippi",
        "Slippi Dolphin's gameplay rules (UCF and its fixes), everything unlocked and no memory card, so nothing is saved.",
    ),
];
/// The Ruleset setting's help line (proposed 2026-11-17, awaiting Sarah's word).
pub const RULESET_HELP: &str =
    "Applies from Melee's next start (Restart Melee\u{2026}), and never to a replay.";
/// The Save section while Melee runs under the Slippi ruleset (proposed 2026-11-17, awaiting
/// Sarah's word).
pub const SAVE_SLIPPI: &str = "The Slippi ruleset runs with no memory card: this save is Vanilla's, and stays as it is until Melee runs under Vanilla again.";
/// The developer setting under the Slippi ruleset (proposed 2026-11-17, awaiting Sarah's word).
pub const DEBUG_SLIPPI: &str = "Vanilla only: under the Slippi ruleset Melee runs as sold, since Slippi Dolphin can't play back a raised debug level.";
/// The Music line's help (proposed 2026-11-27, awaiting Sarah's word).
pub const MUSIC_HELP: &str = "The music's volume within Volume. Melee's own sound balance (Options) sets the announcer against the effects.";
/// The Audio section's other help lines (proposed 2026-11-14, awaiting Sarah's word).
pub const AUDIO_DEVICE_HELP: &str =
    "The default follows the system's. An unplugged device falls back to it until it's back.";
pub const EXCLUSIVE_HELP: &str =
    "The lowest latency, but no other program's sound plays while melee-hd runs.";
/// The Early sound effects line's help (proposed 2026-11-27, awaiting Sarah's word).
pub const EARLY_SFX_HELP: &str =
    "Starts each sound effect 5 ms sooner where it can. The music is unchanged.";
/// What the panel says when Melee stops (proposed 2026-11-14, awaiting Sarah's word): where
/// the simulation stops by name, at a place the original would crash or melee-hd can't follow.
pub const STOP_TITLE: &str = "Melee stopped";
pub const STOP_TEXT: &str = "Melee reached something melee-hd can't play yet, or a place where the original game would crash. It isn't anything you did. Report a bug saves what's needed to see it happen again; Restart Melee\u{2026} starts over from power-on.";
/// Report a bug's button and help line (proposed 2026-11-14, awaiting Sarah's word).
pub const REPORT_BUTTON: &str = "Report a bug";

/// The panel's footer lines for a controller ([`crate::pad_nav`], [`crate::hooks`]).
pub const PAD_KEYS: &str =
    "Controller: the D-pad or stick moves, A: choose, B: back, Start: close.";
pub const PAD_OPEN: &str = "Press Z on Melee's Options list, or hold Z for a second in its main menu, to open these settings with a controller.";
pub const REPORT_HELP: &str = "Saves a zip in the settings folder with this session's recording, the log, your latest captures and your settings, and opens the folder. Nothing is sent anywhere: attach the zip to your report yourself.";

/// The Renderer setting's choices and help lines (Sarah, 2026-11-20: signed off, Fast's line
/// with "giving high performance" for "for low-end PCs").
fn renderer_name(fast: bool) -> &'static str {
    if fast { "Fast" } else { "Exact" }
}
pub const RENDERER_FAST_HELP: &str = "Draws the same picture with far less of the processor's time, giving high performance and the lowest latency.";
pub const RENDERER_EXACT_HELP: &str = "Draws every frame step by step as the console's graphics chip does: the reference the fast renderer is checked against.";

/// A debug level's line of [`DEBUG_LEVELS`] (Off's for one not offered).
fn debug_level(level: i32) -> (i32, &'static str, &'static str) {
    DEBUG_LEVELS
        .into_iter()
        .find(|l| l.0 == level)
        .unwrap_or(DEBUG_LEVELS[0])
}

/// A ruleset's line of [`RULESETS`].
fn ruleset(r: Ruleset) -> (Ruleset, &'static str, &'static str) {
    RULESETS
        .into_iter()
        .find(|l| l.0 == r)
        .unwrap_or(RULESETS[0])
}

/// A ruleset as `settings.txt` and `--ruleset` name it.
pub fn ruleset_key(r: Ruleset) -> &'static str {
    match r {
        Ruleset::Vanilla => "vanilla",
        Ruleset::Slippi => "slippi",
    }
}

/// [`ruleset_key`]'s ruleset.
pub fn parse_ruleset(v: &str) -> Option<Ruleset> {
    match v {
        "vanilla" => Some(Ruleset::Vanilla),
        "slippi" => Some(Ruleset::Slippi),
        _ => None,
    }
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            aspect: Aspect::GAME,
            fullscreen: false,
            scale: 0,
            msaa: false,
            // The fast renderer by default since 2026-11-20 (Sarah).
            fast_renderer: true,
            // Both on by default since 2026-11-14 (Sarah).
            high_refresh: Mode::Extrapolate,
            present: Present::Mailbox,
            added_delay: 0,
            tick_timing: Timing::Auto,
            jit_margin: (crate::jit::DEFAULT_MARGIN.as_secs_f64() * 1000.0) as f32,
            record: true,
            replays: String::new(),
            compress_replays: false,
            record_sessions: true,
            debug_level: 0,
            volume: 100,
            music: 100,
            audio_device: String::new(),
            audio_exclusive: false,
            early_sfx: true,
            recordings: String::new(),
            ruleset: DEFAULT_RULESET,
            controllers: melee_input::Config::default(),
            low_spec: None,
        }
    }
}

impl Settings {
    /// The settings as the settings folder keeps them, one `key = value` a line.
    pub fn save(&self) -> String {
        let aspect = match self.aspect {
            a if a.widen => format!("{}:1", a.shown),
            Aspect::STRETCHED => "4:3".to_owned(),
            _ => "73:60".to_owned(),
        };
        let scale = match self.scale {
            0 => "auto".to_owned(),
            n => n.to_string(),
        };
        let present = match self.present {
            Present::Mailbox => "mailbox",
            Present::Fifo => "fifo",
            Present::Immediate => "immediate",
        };
        let on = |b: bool| if b { "on" } else { "off" };
        format!(
            "aspect = {aspect}\nfullscreen = {}\nscale = {scale}\nmsaa = {}\nhigh-refresh = {}\npresent = {present}\nadded-delay = {}\ntick-timing = {}\njit-margin = {}\nrecord = {}\nreplays = {}\nrecord-sessions = {}\ndebug-level = {}\nvolume = {}\nmusic = {}\naudio-device = {}\naudio-exclusive = {}\n",
            self.fullscreen,
            if self.msaa { "4" } else { "off" },
            self.high_refresh.setting(),
            self.added_delay,
            self.tick_timing.key(),
            self.jit_margin,
            on(self.record),
            self.replays,
            on(self.record_sessions),
            match self.debug_level {
                3 => "debug-rom",
                4 => "develop",
                _ => "off",
            },
            self.volume,
            self.music,
            self.audio_device,
            on(self.audio_exclusive),
        ) + &format!(
            "early-sfx = {}\nrecordings = {}\ncompress-replays = {}\nruleset = {}\nrenderer = {}\nsettings-version = {SETTINGS_VERSION}\n",
            on(self.early_sfx),
            self.recordings,
            on(self.compress_replays),
            ruleset_key(self.ruleset),
            if self.fast_renderer { "fast" } else { "exact" }
        ) + &self.low_spec_lines()
            + &self.controllers.save()
    }

    /// Low-spec mode's lines: whether it's on, and while it is, each setting it replaced.
    fn low_spec_lines(&self) -> String {
        let on = if self.low_spec.is_some() { "on" } else { "off" };
        let mut lines = format!("low-spec = {on}\n");
        for (key, was) in self.low_spec.iter().flatten() {
            lines += &format!("low-spec-saved = {key} = {was}\n");
        }
        lines
    }

    /// Low-spec mode on: each of [`LOW_SPEC`]'s settings to its low value, what it replaced
    /// kept (a setting already there keeps nothing). Already on, nothing changes.
    pub fn low_spec_on(&mut self) {
        if self.low_spec.is_some() {
            return;
        }
        let mut saved = Vec::new();
        for l in &LOW_SPEC {
            let was = (l.get)(self);
            if was != l.low {
                (l.set)(self, l.low);
                saved.push((l.key, was));
            }
        }
        self.low_spec = Some(saved);
    }

    /// Low-spec mode off: what it replaced back, except a setting the player changed while it
    /// was on, which keeps the player's change (design call 3). A setting still at its low
    /// value counts as unchanged, even one changed and changed back. Already off, nothing
    /// changes.
    pub fn low_spec_off(&mut self) {
        let Some(saved) = self.low_spec.take() else {
            return;
        };
        for (key, was) in saved {
            if let Some(l) = LOW_SPEC.iter().find(|l| l.key == key)
                && (l.get)(self) == l.low
            {
                (l.set)(self, &was);
            }
        }
    }

    /// How the sound's output device is opened.
    pub fn output(&self) -> melee_audio::OutputOptions {
        melee_audio::OutputOptions {
            exclusive: self.audio_exclusive,
            device: Some(self.audio_device.clone()).filter(|d| !d.is_empty()),
        }
    }

    /// Takes back what [`Settings::save`] kept, except what the command line gave; a line
    /// missing or unreadable leaves its setting as it is.
    pub fn restore(&mut self, text: &str, given: &Given) {
        let mut versioned = false;
        let (mut old_timer, mut old_off) = (false, false);
        // The controllers' lines replace what's set, all together.
        let mut controllers = melee_input::Config::default();
        let mut any_controllers = false;
        // Low-spec mode's lines, together: whether it's on, and what it replaced.
        let (mut low_spec, mut low_saved) = (None, Vec::new());
        let on = |v: &str| match v {
            "on" => Some(true),
            "off" => Some(false),
            _ => None,
        };
        for line in text.lines() {
            let Some((key, v)) = line.split_once('=') else {
                continue;
            };
            let v = v.trim();
            match key.trim() {
                "aspect" if !given.aspect => {
                    if let Ok(a) = Aspect::parse(v) {
                        self.aspect = a;
                    }
                }
                "fullscreen" if !given.fullscreen => {
                    if let Ok(f) = v.parse() {
                        self.fullscreen = f;
                    }
                }
                "scale" if !given.scale => {
                    if v == "auto" {
                        self.scale = 0;
                    } else if let Some(n) = v.parse().ok().filter(|n| (1..=8).contains(n)) {
                        self.scale = n;
                    }
                }
                "msaa" if !given.msaa => match v {
                    "4" => self.msaa = true,
                    "off" | "1" => self.msaa = false,
                    _ => {}
                },
                "renderer" if !given.renderer => match v {
                    "fast" => self.fast_renderer = true,
                    "exact" => self.fast_renderer = false,
                    _ => {}
                },
                "high-refresh" if !given.high_refresh => {
                    if let Some(m) = Mode::from_setting(v) {
                        self.high_refresh = m;
                    }
                    old_off = v == "off";
                }
                "present" if !given.present => {
                    if let Ok(p) = Present::parse(v) {
                        self.present = p;
                    }
                }
                "added-delay" if !given.added_delay => {
                    if let Some(n) = v.parse().ok().filter(|n| *n <= 15) {
                        self.added_delay = n;
                    }
                }
                "tick-timing" if !given.tick_timing => {
                    if let Ok(t) = Timing::parse(v) {
                        self.tick_timing = t;
                    }
                    old_timer = v == "timer";
                }
                "settings-version" => versioned = true,
                "jit-margin" if !given.jit_margin => {
                    if let Some(m) = parse_margin(v) {
                        self.jit_margin = m;
                    }
                }
                "record" if !given.record => {
                    if let Some(b) = on(v) {
                        self.record = b;
                    }
                }
                "replays" if !given.replays => self.replays = v.to_owned(),
                "compress-replays" if !given.compress_replays => {
                    if let Some(b) = on(v) {
                        self.compress_replays = b;
                    }
                }
                "record-sessions" if !given.record_sessions => {
                    if let Some(b) = on(v) {
                        self.record_sessions = b;
                    }
                }
                "debug-level" if !given.debug_level => match v {
                    "off" => self.debug_level = 0,
                    "debug-rom" => self.debug_level = 3,
                    "develop" => self.debug_level = 4,
                    _ => {}
                },
                "volume" => {
                    if let Some(n) = v.parse().ok().filter(|n| *n <= 100) {
                        self.volume = n;
                    }
                }
                "music" => {
                    if let Some(n) = v.parse().ok().filter(|n| *n <= 100) {
                        self.music = n;
                    }
                }
                "audio-device" => self.audio_device = v.to_owned(),
                "audio-exclusive" if !given.audio_exclusive => {
                    if let Some(b) = on(v) {
                        self.audio_exclusive = b;
                    }
                }
                "early-sfx" => {
                    if let Some(b) = on(v) {
                        self.early_sfx = b;
                    }
                }
                "recordings" => self.recordings = v.to_owned(),
                "ruleset" if !given.ruleset => {
                    if let Some(r) = parse_ruleset(v) {
                        self.ruleset = r;
                    }
                }
                "low-spec" => low_spec = on(v).or(low_spec),
                "low-spec-saved" => {
                    // `KEY = VALUE`, kept as the setting reads it back; one it wouldn't take is
                    // dropped.
                    let (k, was) = v.split_once('=').unwrap_or((v, ""));
                    let mut read = Settings::default();
                    if let Some(l) = LOW_SPEC.iter().find(|l| l.key == k.trim())
                        && (l.set)(&mut read, was.trim())
                    {
                        low_saved.retain(|(k, _)| *k != l.key);
                        low_saved.push((l.key, (l.get)(&read)));
                    }
                }
                key => any_controllers |= controllers.restore_line(key, v),
            }
        }
        if any_controllers {
            self.controllers = controllers;
        }
        if let Some(on) = low_spec {
            self.low_spec = on.then_some(low_saved);
        }
        // Settings kept before 2026-11-14 (no version) hold the old defaults in every key, as
        // every save writes them all: the fixed timer and high refresh off move to the new
        // defaults once. The version written from now on keeps a player's later choice.
        if !versioned {
            if old_timer {
                self.tick_timing = Timing::Auto;
            }
            if old_off {
                self.high_refresh = Mode::Extrapolate;
            }
        }
    }

    /// The kept settings over `self`, but what the command line gave.
    pub fn load(&mut self, given: &Given) {
        if let Some(text) = crate::settings::read(FILE).or_else(|| crate::settings::read(OLD_FILE))
        {
            self.restore(&text, given);
        }
    }

    /// Keeps the settings in the settings folder.
    pub fn keep(&self) {
        crate::settings::write(FILE, &self.save());
    }

    /// The folder the Recordings tab lists: the one picked, else the replays folder, where the
    /// player saves its matches.
    pub fn recordings_folder(&self) -> std::path::PathBuf {
        match (self.recordings.as_str(), self.replays.as_str()) {
            ("", "") => crate::recording::default_dir(),
            ("", r) => r.into(),
            (f, _) => f.into(),
        }
    }
}

/// What the panel asks the player to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Resume,
    Restart,
    Quit,
    ChangeDisc,
    Import,
    Export,
    UnlockAll,
    ReportBug,
    /// Plays the replay the Recordings tab chose.
    Play,
    /// Ends the replay playing and goes back to Melee.
    BackToMelee,
    /// Picks the folder the Recordings tab lists, or goes back to the replays folder.
    ChooseFolder,
    ReplaysFolder,
}

/// The panel's tabs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Settings,
    Controllers,
    QuickMatch,
    Recordings,
}

impl Tab {
    /// The tabs in order, with their names.
    pub const ALL: [(Tab, &'static str); 4] = [
        (Tab::Settings, "Settings"),
        (Tab::Controllers, crate::controllers::TAB),
        (Tab::QuickMatch, "Quick match"),
        (Tab::Recordings, "Recordings"),
    ];

    /// A tab by the name `--settings-tab` takes.
    pub fn parse(v: &str) -> Result<Tab, String> {
        match v {
            "settings" => Ok(Tab::Settings),
            "controllers" => Ok(Tab::Controllers),
            "quick-match" => Ok(Tab::QuickMatch),
            "recordings" => Ok(Tab::Recordings),
            _ => Err(format!(
                "unknown tab {v} (settings, controllers, quick-match or recordings)"
            )),
        }
    }
}

/// The panel: the settings and what it shows of the save and the disc.
pub struct Panel {
    /// The tab showing.
    pub tab: Tab,
    pub quick: crate::quick::QuickMatch,
    pub recordings: crate::recordings::Recordings,
    /// The replay from the Recordings tab playing, by its file's name.
    pub playing: Option<String>,
    /// Melee or a replay is loading: nothing more starts meanwhile.
    pub busy: bool,
    pub settings: Settings,
    /// Whether it shows (the console paused behind it).
    pub open: bool,
    /// Why Melee couldn't start, or what the last action did.
    pub message: Option<String>,
    /// The message is news of something done (a report or an export saved), shown in the
    /// normal color rather than as a problem.
    pub message_ok: bool,
    /// Melee stopped: it can't be resumed, only restarted.
    pub halted: bool,
    /// What stopped Melee, when the simulation stopped by name ([`STOP_TEXT`]).
    pub stop: Option<String>,
    /// The output devices, as the panel last looked.
    pub devices: Vec<String>,
    /// The save's line, and whether everything is unlocked in it (none: no save).
    pub save_line: String,
    pub unlocked: Option<bool>,
    /// The disc's path.
    pub disc: String,
    /// The ruleset Melee runs now (the setting's takes effect at the next start): what the Save
    /// and Recording sections say.
    pub running: Ruleset,
    /// The Controllers tab's state.
    pub controllers: crate::controllers::Controllers,
    /// The adapter ports' PhobGCC sections.
    pub phob: crate::phob_ui::PhobPanel,
}

impl Panel {
    pub fn new(settings: Settings, disc: String) -> Panel {
        let running = settings.ruleset;
        let mut p = Panel {
            running,
            tab: Tab::Settings,
            quick: Default::default(),
            recordings: Default::default(),
            playing: None,
            busy: false,
            settings,
            open: false,
            message: None,
            message_ok: false,
            halted: false,
            stop: None,
            devices: Vec::new(),
            save_line: String::new(),
            unlocked: None,
            disc,
            controllers: Default::default(),
            phob: Default::default(),
        };
        p.look_at_save();
        p
    }

    /// Reads what the saves folder holds, for the panel's save line.
    pub fn look_at_save(&mut self) {
        let dir = crate::saves::dir();
        let path = dir.join(crate::saves::SAVE_FILE);
        self.unlocked = crate::saves::unlocked(&dir);
        self.save_line = match std::fs::metadata(&path).and_then(|m| m.modified()) {
            Ok(t) => format!(
                "Your save: {}, last written {}",
                crate::saves::SAVE_FILE,
                written_at(t)
            ),
            Err(_) => "Your save: none yet".to_owned(),
        };
    }

    /// Looks at the output devices there are now, for the Audio section.
    pub fn look_at_devices(&mut self) {
        self.devices = melee_audio::output::devices();
    }

    /// Lays the panel out; what was pressed, if anything.
    pub fn show(&mut self, ui: &mut egui::Ui) -> Option<Action> {
        let mut action = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            crate::ui::keep_focus_shown(ui);
            action = self.show_panel(ui);
        });
        action
    }

    fn show_panel(&mut self, ui: &mut egui::Ui) -> Option<Action> {
        let mut action = None;
        ui.vertical_centered(|ui| {
            ui.add_space(24.0);
            ui.horizontal(|ui| {
                ui.add_space(((ui.available_width() - 500.0) / 2.0).max(0.0));
                for (tab, name) in Tab::ALL {
                    ui.selectable_value(&mut self.tab, tab, egui::RichText::new(name).heading());
                    ui.add_space(8.0);
                }
            });
            ui.add_space(12.0);
            let width = match self.tab {
                Tab::Settings => 440.0,
                Tab::Controllers => 520.0,
                Tab::QuickMatch | Tab::Recordings => 640.0,
            };
            ui.horizontal(|ui| {
                ui.add_space(((ui.available_width() - width) / 2.0).max(0.0));
                ui.vertical(|ui| {
                    ui.set_max_width(width);
                    // A stop shows whichever tab is open.
                    if let Some(a) = self.show_stop(ui) {
                        action = Some(a);
                    }
                    let tab = match self.tab {
                        Tab::Settings => self.show_sections(ui),
                        Tab::Controllers => {
                            // Under each adapter port's controller, its PhobGCC section.
                            let phob = &mut self.phob;
                            self.controllers.show(
                                ui,
                                &mut self.settings.controllers,
                                &mut |ui, port, adapter| phob.show(ui, usize::from(port), adapter),
                            );
                            None
                        }
                        Tab::QuickMatch => {
                            self.quick.show(ui);
                            None
                        }
                        Tab::Recordings => self.recordings.show(
                            ui,
                            &self.settings.recordings_folder(),
                            !self.settings.recordings.is_empty(),
                            self.playing.as_deref(),
                            self.busy,
                        ),
                    };
                    if tab.is_some() {
                        action = tab;
                    }
                });
            });
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                ui.add_space(((ui.available_width() - 420.0) / 2.0).max(0.0));
                let size = egui::vec2(130.0, 30.0);
                if ui
                    .add_enabled(!self.halted, egui::Button::new("Resume").min_size(size))
                    .clicked()
                {
                    action = Some(Action::Resume);
                }
                if ui
                    .add(egui::Button::new("Restart Melee\u{2026}").min_size(size))
                    .clicked()
                {
                    action = Some(Action::Restart);
                }
                if ui
                    .add(egui::Button::new("Quit melee-hd").min_size(size))
                    .clicked()
                {
                    action = Some(Action::Quit);
                }
            });
            if let Some(m) = &self.message {
                ui.add_space(8.0);
                ui.scope(|ui| {
                    ui.set_max_width(440.0);
                    if self.message_ok {
                        ui.label(m);
                    } else {
                        ui.colored_label(ui.visuals().error_fg_color, m);
                    }
                });
            }
            // Once Melee stopped, its message offers it.
            if self.stop.is_none()
                && let Some(a) = show_report(ui)
            {
                action = Some(a);
            }
            ui.add_space(16.0);
            ui.weak("Esc: settings. F11 or Alt+Enter: fullscreen. F3: latency overlay. F12: bug capture.");
            ui.weak(PAD_KEYS);
            ui.weak(PAD_OPEN);
            ui.add_space(16.0);
        });
        action
    }

    /// When Melee stopped: what happened in plain words, what stopped it, and Report a bug.
    fn show_stop(&mut self, ui: &mut egui::Ui) -> Option<Action> {
        let why = self.stop.as_ref()?;
        let mut action = None;
        ui.add_space(4.0);
        egui::Frame::group(ui.style())
            .stroke(egui::Stroke::new(1.0, ui.visuals().warn_fg_color))
            .inner_margin(12.0)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.strong(egui::RichText::new(STOP_TITLE).color(ui.visuals().warn_fg_color));
                ui.add_space(4.0);
                ui.label(STOP_TEXT);
                ui.add_space(4.0);
                ui.add(egui::Label::new(egui::RichText::new(why).weak().small()).selectable(true));
                ui.add_space(8.0);
                if ui.button(REPORT_BUTTON).clicked() {
                    action = Some(Action::ReportBug);
                }
            });
        ui.add_space(8.0);
        action
    }

    /// The sound: its volume and the music's, its output device and whether it takes the
    /// device for itself.
    fn show_audio(&mut self, ui: &mut egui::Ui) {
        let s = &mut self.settings;
        section(ui, "Audio");
        egui::Grid::new("audio")
            .num_columns(2)
            .spacing([16.0, 8.0])
            .show(ui, |ui| {
                ui.label("Volume");
                // Steps of 1 for a controller's left and right (`crate::pad_nav`) in its box too.
                ui.add(
                    egui::Slider::new(&mut s.volume, 0..=100)
                        .step_by(1.0)
                        .suffix("%"),
                );
                ui.end_row();
                ui.label("Music");
                ui.add(egui::Slider::new(&mut s.music, 0..=100).suffix("%"));
                ui.end_row();
                help(ui, MUSIC_HELP);
                ui.label("Output device");
                let device = &mut s.audio_device;
                let name = |d: &str| match d {
                    "" => "System default".to_owned(),
                    d if self.devices.iter().any(|x| x == d) => d.to_owned(),
                    d => format!("{d} (not connected)"),
                };
                egui::ComboBox::from_id_salt("audio-device")
                    .selected_text(name(device))
                    .width(220.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(device, String::new(), name(""));
                        for d in &self.devices {
                            ui.selectable_value(device, d.clone(), d);
                        }
                    });
                ui.end_row();
                help(ui, AUDIO_DEVICE_HELP);
                if cfg!(windows) {
                    ui.label("");
                    ui.checkbox(&mut s.audio_exclusive, "Exclusive mode");
                    ui.end_row();
                    help(ui, EXCLUSIVE_HELP);
                }
                ui.label("");
                ui.checkbox(&mut s.early_sfx, "Early sound effects");
                ui.end_row();
                help(ui, EARLY_SFX_HELP);
            });
    }

    fn show_sections(&mut self, ui: &mut egui::Ui) -> Option<Action> {
        let mut action = None;
        let s = &mut self.settings;
        section(ui, "Display");
        egui::Grid::new("display")
            .num_columns(2)
            .spacing([16.0, 8.0])
            .show(ui, |ui| {
                ui.label("");
                let mut low = s.low_spec.is_some();
                if ui.checkbox(&mut low, "Low-spec mode").changed() {
                    if low {
                        s.low_spec_on();
                    } else {
                        s.low_spec_off();
                    }
                }
                ui.end_row();
                help(ui, &low_spec_help());
                ui.label("Resolution");
                let (scale, aspect) = (&mut s.scale, s.aspect);
                let name = |n: u32| match n {
                    0 => "Auto (the window's)".to_owned(),
                    n => format!("{n}x ({}x{})", aspect.efb_width(n), 480 * n),
                };
                *scale = (*scale).min(aspect.max_scale());
                egui::ComboBox::from_id_salt("scale")
                    .selected_text(name(*scale))
                    .width(220.0)
                    .show_ui(ui, |ui| {
                        for n in 0..=aspect.max_scale() {
                            ui.selectable_value(scale, n, name(n));
                        }
                    });
                ui.end_row();
                ui.label("");
                ui.checkbox(&mut s.msaa, "Antialiasing (MSAA 4x)");
                ui.end_row();
                ui.label("Renderer");
                let fast = &mut s.fast_renderer;
                egui::ComboBox::from_id_salt("renderer")
                    .selected_text(renderer_name(*fast))
                    .width(220.0)
                    .show_ui(ui, |ui| {
                        for f in [true, false] {
                            ui.selectable_value(fast, f, renderer_name(f));
                        }
                    });
                ui.end_row();
                help(ui, if s.fast_renderer { RENDERER_FAST_HELP } else { RENDERER_EXACT_HELP });
                ui.label("Aspect ratio");
                let aspect = &mut s.aspect;
                egui::ComboBox::from_id_salt("aspect")
                    .selected_text(aspect.name())
                    .width(220.0)
                    .show_ui(ui, |ui| {
                        let mut choices: Vec<Aspect> =
                            Aspect::CHOICES.iter().map(|(a, _)| *a).collect();
                        if !choices.contains(aspect) {
                            choices.push(*aspect);
                        }
                        for a in choices {
                            ui.selectable_value(aspect, a, a.name());
                        }
                    });
                ui.end_row();
                help(
                    ui,
                    "Wider ratios show more of the stage at the sides; gameplay is unchanged.",
                );
                ui.label("");
                ui.checkbox(&mut s.fullscreen, "Fullscreen");
                ui.end_row();
                ui.label("");
                let mut on = s.high_refresh.paced();
                if ui.checkbox(&mut on, "High refresh").changed() {
                    s.high_refresh = if on { Mode::Extrapolate } else { Mode::Off };
                }
                ui.end_row();
                help(
                    ui,
                    "Draws frames between the game's 60 a second at your display's own rate by predicting the next one. Adds no latency.",
                );
            });
        section(ui, "Latency");
        egui::Grid::new("latency")
            .num_columns(2)
            .spacing([16.0, 8.0])
            .show(ui, |ui| {
                ui.label("Present");
                let present = &mut s.present;
                egui::ComboBox::from_id_salt("present")
                    .selected_text(present.name())
                    .width(220.0)
                    .show_ui(ui, |ui| {
                        for (p, name) in Present::CHOICES {
                            ui.selectable_value(present, p, name);
                        }
                    });
                ui.end_row();
                ui.label("Added delay");
                ui.add(
                    egui::DragValue::new(&mut s.added_delay)
                        .range(0..=15)
                        // A frame a step, so a controller's left and right step it (egui's
                        // whole-number speed, 0.25, rounds each step away).
                        .speed(1.0)
                        .suffix(" frames"),
                );
                ui.end_row();
                help(
                    ui,
                    "Holds the controllers back on purpose, for a CRT's feel.",
                );
                ui.label("Tick timing");
                let timing = &mut s.tick_timing;
                egui::ComboBox::from_id_salt("tick-timing")
                    .selected_text(timing.name())
                    .width(220.0)
                    .show_ui(ui, |ui| {
                        for (t, name) in Timing::CHOICES {
                            ui.selectable_value(timing, t, name);
                        }
                    });
                ui.end_row();
                ui.label("Timing margin");
                ui.add_enabled(
                    s.tick_timing != Timing::Timer,
                    egui::DragValue::new(&mut s.jit_margin)
                        .range(0.0..=MAX_MARGIN_MS)
                        .speed(0.05)
                        .fixed_decimals(2)
                        .suffix(" ms"),
                );
                ui.end_row();
            });
        self.show_audio(ui);
        let s = &mut self.settings;
        section(ui, "Game");
        egui::Grid::new("game")
            .num_columns(2)
            .spacing([16.0, 8.0])
            .show(ui, |ui| {
                ui.label("Ruleset");
                let chosen = &mut s.ruleset;
                egui::ComboBox::from_id_salt("ruleset")
                    .selected_text(ruleset(*chosen).1)
                    .width(220.0)
                    .show_ui(ui, |ui| {
                        for (r, name, _) in RULESETS {
                            ui.selectable_value(chosen, r, name);
                        }
                    });
                ui.end_row();
                help(ui, ruleset(*chosen).2);
                help(ui, RULESET_HELP);
            });
        section(ui, "Save");
        ui.label(&self.save_line);
        if self.running == Ruleset::Slippi {
            ui.weak(SAVE_SLIPPI);
        }
        ui.horizontal_wrapped(|ui| {
            if ui.button("Import a save\u{2026}").clicked() {
                action = Some(Action::Import);
            }
            if ui
                .add_enabled(
                    self.unlocked.is_some(),
                    egui::Button::new("Export the save\u{2026}"),
                )
                .clicked()
            {
                action = Some(Action::Export);
            }
            let b = ui.add_enabled(
                self.unlocked == Some(false),
                egui::Button::new("Unlock everything\u{2026}"),
            );
            let b = if self.unlocked == Some(true) {
                b.on_disabled_hover_text("Everything is unlocked.")
            } else {
                b
            };
            if b.clicked() {
                action = Some(Action::UnlockAll);
            }
        });
        if self.unlocked == Some(true) {
            ui.weak("Everything is unlocked.");
        }
        // Replays are saved under the Slippi ruleset only: with Vanilla chosen the box shows off
        // and greyed, and the setting stays kept for the Slippi ruleset.
        let (saves, replays_help, replays_note) = replays_box(self.settings.ruleset, self.running);
        let s = &mut self.settings;
        section(ui, "Recording");
        egui::Grid::new("recording")
            .num_columns(2)
            .spacing([16.0, 8.0])
            .show(ui, |ui| {
                ui.label("");
                if saves {
                    ui.checkbox(&mut s.record, "Save replays");
                } else {
                    ui.add_enabled(false, egui::Checkbox::new(&mut false, "Save replays"));
                }
                ui.end_row();
                help(ui, replays_help);
                if let Some(note) = replays_note {
                    help(ui, note);
                }
                ui.label("Replays folder");
                ui.add_enabled(
                    saves && s.record,
                    egui::TextEdit::singleline(&mut s.replays)
                        .hint_text("replays in the settings folder")
                        .desired_width(220.0),
                );
                ui.end_row();
                ui.label("");
                ui.add_enabled(
                    saves && s.record,
                    egui::Checkbox::new(&mut s.compress_replays, COMPRESS_LABEL),
                );
                ui.end_row();
                help(ui, COMPRESS_HELP);
                ui.label("");
                ui.checkbox(&mut s.record_sessions, "Record sessions");
                ui.end_row();
                help(
                    ui,
                    "Keeps a recording of each session from power-on, to report a bug.",
                );
            });
        section(ui, "Developer");
        egui::Grid::new("developer")
            .num_columns(2)
            .spacing([16.0, 8.0])
            .show(ui, |ui| {
                ui.label("Debug level");
                // Vanilla's only: the level stays kept for it, and a Slippi start runs at
                // retail's (`power::setup`).
                let vanilla = s.ruleset == Ruleset::Vanilla;
                let level = &mut s.debug_level;
                ui.add_enabled_ui(vanilla, |ui| {
                    egui::ComboBox::from_id_salt("debug-level")
                        .selected_text(debug_level(if vanilla { *level } else { 0 }).1)
                        .width(220.0)
                        .show_ui(ui, |ui| {
                            for (l, name, _) in DEBUG_LEVELS {
                                ui.selectable_value(level, l, name);
                            }
                        });
                });
                ui.end_row();
                if !vanilla {
                    help(ui, DEBUG_SLIPPI);
                    return;
                }
                help(ui, debug_level(*level).2);
                help(ui, DEBUG_HELP);
                if *level != 0 {
                    ui.label("");
                    let warn = ui.visuals().warn_fg_color;
                    ui.colored_label(warn, DEBUG_WARNING);
                    ui.end_row();
                }
            });
        section(ui, "Disc");
        ui.label(&self.disc);
        if ui.button("Change disc\u{2026}").clicked() {
            action = Some(Action::ChangeDisc);
        }
        action
    }
}

/// Report a bug, which the panel offers whenever it shows, under its buttons.
fn show_report(ui: &mut egui::Ui) -> Option<Action> {
    ui.add_space(12.0);
    let pressed = ui.button(REPORT_BUTTON).clicked();
    ui.add_space(4.0);
    ui.scope(|ui| {
        ui.set_max_width(440.0);
        ui.weak(REPORT_HELP);
    });
    pressed.then_some(Action::ReportBug)
}

/// A section's heading.
fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(12.0);
    ui.strong(title);
    ui.separator();
}

/// A help line under a grid's row.
fn help(ui: &mut egui::Ui, text: &str) {
    ui.label("");
    ui.weak(text);
    ui.end_row();
}

/// A file's time as the panel says it: the local date and time to the minute (UTC off
/// Windows).
pub fn written_at(t: std::time::SystemTime) -> String {
    let secs = t
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let offset = local_offset();
    let local = (secs as i64 + offset).max(0) as u64;
    let (y, mo, d) = civil(local / 86_400);
    let (h, mi) = (local % 86_400 / 3600, local % 3600 / 60);
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}")
}

/// The local time's offset from UTC now, in seconds.
fn local_offset() -> i64 {
    let [y, mo, d, h, mi, s] = crate::recording::local_now();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    let days = crate::power::days_since_2000(y, mo, d) as i64 + 10_957;
    let local = days * 86_400 + i64::from(h * 3600 + mi * 60 + s);
    // Rounded to the quarter hour, which every time zone's offset is a multiple of.
    ((local - now) as f64 / 900.0).round() as i64 * 900
}

/// A date from days since 1970-01-01.
fn civil(days: u64) -> (u64, u64, u64) {
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y as u64, m as u64, d as u64)
}

/// The note over the game when a gated menu entry is refused (`gates`): a dark box with
/// white text near the bottom of the frame.
pub fn note(
    painter: &mut crate::ui::Painter,
    gpu: &crate::present::Gpu,
    text: &str,
    view: &wgpu::TextureView,
    w: u32,
    h: u32,
) {
    let zoom = painter.ctx.zoom_factor();
    let size = egui::vec2(w as f32, h as f32) / zoom;
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
        ..Default::default()
    };
    let out = painter.ctx.run_ui(input, |ui| {
        egui::Area::new(egui::Id::new("gate-note"))
            .anchor(egui::Align2::CENTER_BOTTOM, [0.0, -size.y * 0.08])
            .show(ui.ctx(), |ui| {
                egui::Frame::new()
                    .fill(egui::Color32::from_black_alpha(210))
                    .inner_margin(12.0)
                    .corner_radius(4.0)
                    .show(ui, |ui| {
                        ui.set_max_width(size.x * 0.8);
                        ui.label(egui::RichText::new(text).color(egui::Color32::WHITE));
                    });
            });
    });
    painter.paint(gpu, out, view, w, h);
}

/// Where the Options list's line goes, as fractions of the game's 73:60 picture (its left edge
/// and its top): under the list's last entry, aligned with its entries.
pub const OPTIONS_LINE_AT: [f32; 2] = [0.075, 0.782];

/// The line under Melee's Options list ([`crate::hooks::OPTIONS_LINE`]) over a frame `w` by
/// `h` shown at `aspect`: placed in the game's own picture, which a widened frame keeps in its
/// middle and a stretched one stretches.
pub fn options_line(
    painter: &mut crate::ui::Painter,
    gpu: &crate::present::Gpu,
    aspect: Aspect,
    view: &wgpu::TextureView,
    w: u32,
    h: u32,
) {
    let zoom = painter.ctx.zoom_factor();
    let [left, top, pw, ph] = crate::present::picture(w, h);
    let game_w = if aspect.widen {
        ph * melee_render::frame::GAME_ASPECT
    } else {
        pw
    };
    let x = left + (pw - game_w) / 2.0 + OPTIONS_LINE_AT[0] * game_w;
    let y = top + OPTIONS_LINE_AT[1] * ph;
    let size = egui::vec2(w as f32, h as f32) / zoom;
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
        ..Default::default()
    };
    // The text's size follows the picture's, as the menu's own text does.
    let text = (ph / zoom * 0.026).max(9.0);
    let out = painter.ctx.run_ui(input, |ui| {
        egui::Area::new(egui::Id::new("options-line"))
            .fixed_pos(egui::pos2(x, y) / zoom)
            .show(ui.ctx(), |ui| {
                egui::Frame::new()
                    .fill(egui::Color32::from_black_alpha(170))
                    .inner_margin(egui::Margin::symmetric(8, 4))
                    .corner_radius(4.0)
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(crate::hooks::OPTIONS_LINE)
                                .size(text)
                                .color(egui::Color32::WHITE),
                        );
                    });
            });
    });
    painter.paint(gpu, out, view, w, h);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picked() -> Settings {
        Settings {
            aspect: Aspect::wide(16.0 / 9.0),
            fullscreen: true,
            scale: 3,
            msaa: true,
            fast_renderer: false,
            high_refresh: Mode::Off,
            present: Present::Immediate,
            added_delay: 2,
            tick_timing: Timing::Vrr,
            jit_margin: 2.5,
            record: false,
            replays: "D:/Slippi/melee-hd".to_owned(),
            compress_replays: true,
            record_sessions: false,
            debug_level: 4,
            volume: 35,
            music: 60,
            audio_device: "Speakers (USB Audio)".to_owned(),
            audio_exclusive: true,
            early_sfx: false,
            recordings: "D:/Slippi".to_owned(),
            ruleset: Ruleset::Slippi,
            controllers: picked_controllers(),
            low_spec: None,
        }
    }

    /// A small xorshift, for random settings.
    struct Rng(u64);

    impl Rng {
        fn below(&mut self, n: u64) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0 % n
        }
        fn pick<T: Copy>(&mut self, of: &[T]) -> T {
            of[self.below(of.len() as u64) as usize]
        }
        fn yes(&mut self) -> bool {
            self.below(2) == 1
        }
    }

    /// Random settings: every setting the panel shows, low-spec's and the rest (the
    /// controllers' and the folders' as [`picked`]'s), low-spec mode off.
    fn random(rng: &mut Rng) -> Settings {
        Settings {
            aspect: rng.pick(&Aspect::CHOICES.map(|(a, _)| a)),
            fullscreen: rng.yes(),
            scale: rng.below(9) as u32,
            msaa: rng.yes(),
            fast_renderer: rng.yes(),
            high_refresh: rng.pick(&[Mode::Off, Mode::Interpolate, Mode::Extrapolate]),
            present: rng.pick(&Present::CHOICES.map(|(p, _)| p)),
            added_delay: rng.below(16) as u32,
            tick_timing: rng.pick(&Timing::CHOICES.map(|(t, _)| t)),
            jit_margin: rng.below(33) as f32 / 4.0,
            record: rng.yes(),
            record_sessions: rng.yes(),
            debug_level: rng.pick(&[0, 3, 4]),
            volume: rng.below(101) as u32,
            music: rng.below(101) as u32,
            audio_exclusive: rng.yes(),
            early_sfx: rng.yes(),
            ruleset: rng.pick(&[Ruleset::Vanilla, Ruleset::Slippi]),
            ..picked()
        }
    }

    /// Through the settings file and back, as quitting and starting again does.
    fn kept(s: &Settings) -> Settings {
        let mut back = Settings::default();
        back.restore(&s.save(), &Given::default());
        back
    }

    /// Low-spec mode sets what design call 3 lists and nothing else; switched off, every
    /// setting is as it was, for random settings, and the same across a quit while it's on.
    #[test]
    fn low_spec_gives_every_setting_back() {
        let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
        for _ in 0..2000 {
            let was = random(&mut rng);
            let mut s = was.clone();
            s.low_spec_on();
            let low = Settings {
                scale: 1,
                msaa: false,
                fast_renderer: true,
                high_refresh: Mode::Off,
                low_spec: s.low_spec.clone(),
                ..was.clone()
            };
            assert_eq!(s, low);
            assert_eq!(kept(&s), s, "{}", s.save());
            // On again changes nothing.
            s.low_spec_on();
            assert_eq!(s, low);
            let mut quit = kept(&s);
            s.low_spec_off();
            assert_eq!(s, was);
            quit.low_spec_off();
            assert_eq!(quit, was);
            // Off again changes nothing.
            s.low_spec_off();
            assert_eq!(s, was);
        }
    }

    /// A setting the player changed while low-spec mode was on keeps the change when it goes
    /// off (design call 3), on its own list's and anything else; the rest come back.
    #[test]
    fn a_change_while_low_spec_survives() {
        let mut rng = Rng(0x2545_f491_4f6c_dd1d);
        let mut kept_changes = 0;
        for _ in 0..2000 {
            let was = random(&mut rng);
            // The panel sets High refresh on or off; Interpolate is the command line's.
            let other = random(&mut rng);
            let other = Settings {
                high_refresh: Mode::from_setting(other.high_refresh.setting()).unwrap(),
                ..other
            };
            let mut s = was.clone();
            s.low_spec_on();
            let mut want = was.clone();
            for l in &LOW_SPEC {
                let v = (l.get)(&other);
                if rng.yes() && v != l.low {
                    (l.set)(&mut s, &v);
                    (l.set)(&mut want, &v);
                    kept_changes += 1;
                }
            }
            s.volume = other.volume;
            want.volume = other.volume;
            if rng.yes() {
                s = kept(&s);
            }
            s.low_spec_off();
            assert_eq!(s, want);
        }
        assert!(kept_changes > 1000);
        // Changed and changed back to the low value counts as unchanged.
        let was = Settings {
            scale: 3,
            ..Settings::default()
        };
        let mut s = was.clone();
        s.low_spec_on();
        s.scale = 5;
        s.scale = 1;
        s.low_spec_off();
        assert_eq!(s, was);
    }

    /// Each of [`LOW_SPEC`]'s settings is the file's own: its key and low value read as the
    /// settings file reads them.
    #[test]
    fn low_spec_names_the_files_settings() {
        let mut rng = Rng(0xd1b5_4a32_d192_ed03);
        for _ in 0..200 {
            let mut s = random(&mut rng);
            for l in &LOW_SPEC {
                // Versioned: an older file's `high-refresh = off` moves to on.
                s.restore(
                    &format!(
                        "{} = {}\nsettings-version = {SETTINGS_VERSION}\n",
                        l.key, l.low
                    ),
                    &Given::default(),
                );
                assert_eq!((l.get)(&s), l.low, "{}", l.key);
                let mut set = random(&mut rng);
                assert!((l.set)(&mut set, l.low));
                assert_eq!((l.get)(&set), l.low);
                assert!(!(l.set)(&mut set, "sometimes"));
            }
        }
    }

    /// The file keeps low-spec mode's lines; ones it can't read change nothing, and a file
    /// without them leaves the mode as it is.
    #[test]
    fn low_spec_is_kept() {
        let mut s = Settings {
            scale: 0,
            msaa: true,
            high_refresh: Mode::Interpolate,
            ..Settings::default()
        };
        s.low_spec_on();
        let file = s.save();
        assert!(
            file.contains(
                "low-spec = on\nlow-spec-saved = scale = auto\nlow-spec-saved = msaa = 4\nlow-spec-saved = high-refresh = interpolate\n"
            ),
            "{file}"
        );
        assert!(Settings::default().save().contains("low-spec = off\n"));
        let mut back = Settings::default();
        back.restore(
            "low-spec = on\nlow-spec-saved = scale = 9\nlow-spec-saved = volume = 3\nlow-spec-saved = msaa\nlow-spec-saved = renderer = exact\n",
            &Given::default(),
        );
        assert_eq!(back.low_spec, Some(vec![("renderer", "exact".to_owned())]));
        back.restore("volume = 20\n", &Given::default());
        assert!(back.low_spec.is_some());
        back.restore("low-spec = sometimes\n", &Given::default());
        assert!(back.low_spec.is_some());
        back.restore("low-spec = off\n", &Given::default());
        assert_eq!(back.low_spec, None);
    }

    /// The switch's help line, its list from [`LOW_SPEC`].
    #[test]
    fn low_spec_help_lists_what_it_turns_off() {
        assert_eq!(
            low_spec_help(),
            "Turns off what costs the most to draw: resolutions above the console's, antialiasing, the exact renderer and high refresh. Turning it off puts your settings back."
        );
    }

    /// Ports placed (one gamepad away, one device on no port) and two devices rebound.
    fn picked_controllers() -> melee_input::Config {
        use melee_input::{Bindings, Chosen, DeviceId, Source, Target};
        let pad = DeviceId::Gamepad {
            uuid: [0x5e; 16],
            nth: 1,
        };
        let mut c = melee_input::Config {
            ports: vec![
                Chosen {
                    device: DeviceId::Adapter(1),
                    port: Some(0),
                    name: melee_input::ports::adapter_name(1),
                },
                Chosen {
                    device: pad.clone(),
                    port: Some(3),
                    name: "Xbox Wireless Controller (2)".into(),
                },
                Chosen {
                    device: DeviceId::Keyboard,
                    port: None,
                    name: "Keyboard".into(),
                },
            ],
            ..Default::default()
        };
        // A gamepad set to read exact, the keyboard on the box layout with a key moved.
        c.set_exact(&pad, true);
        let mut keys = melee_input::boxes::BoxKeys::default();
        keys.set(melee_phob::haybox::Button::Mb2, Some("Digit6".into()));
        c.set_box_layout(&DeviceId::Keyboard, Some(keys));
        let mut b = Bindings::keyboard();
        b.set(Target::A, Some(Source::Key("Space".into())));
        b.set(Target::Modifier, None);
        c.set_bindings(&DeviceId::Keyboard, b);
        let mut b = Bindings::gamepad();
        b.set(
            Target::Z,
            Some(Source::Button(melee_input::gilrs::Button::LeftTrigger)),
        );
        b.set(
            Target::CUp,
            Some(Source::Axis(melee_input::gilrs::Axis::RightStickY, false)),
        );
        c.set_bindings(&pad, b);
        c
    }

    /// The controllers' settings replace what was set, all together; a file without them
    /// leaves them as they were.
    #[test]
    fn the_controllers_come_back() {
        let mut s = Settings::default();
        s.restore(&picked().save(), &Given::default());
        assert_eq!(s.controllers, picked_controllers());
        // Over other ports set, the file's replace them.
        let mut other = Settings::default();
        other.controllers.ports.push(melee_input::Chosen {
            device: melee_input::DeviceId::Adapter(3),
            port: Some(2),
            name: String::new(),
        });
        other.restore(&picked().save(), &Given::default());
        assert_eq!(other.controllers, picked_controllers());
        let mut s = Settings {
            controllers: picked_controllers(),
            ..Settings::default()
        };
        s.restore("volume = 20\n", &Given::default());
        assert_eq!(s.controllers, picked_controllers());
        // Nothing set saves no controllers' lines.
        assert!(!Settings::default().save().contains("port ="));
    }

    #[test]
    fn the_settings_come_back() {
        for aspect in Aspect::CHOICES.map(|(a, _)| a) {
            for mode in [Mode::Off, Mode::Extrapolate] {
                let was = Settings {
                    aspect,
                    high_refresh: mode,
                    ..picked()
                };
                let mut s = Settings::default();
                s.restore(&was.save(), &Given::default());
                assert_eq!(s, was, "{}", was.save());
            }
        }
    }

    #[test]
    fn the_command_line_wins() {
        let mut s = Settings {
            scale: 2,
            aspect: Aspect::STRETCHED,
            ..Settings::default()
        };
        let given = Given {
            scale: true,
            aspect: true,
            ..Given::default()
        };
        s.restore(&picked().save(), &given);
        assert_eq!((s.scale, s.aspect), (2, Aspect::STRETCHED));
        assert!(s.msaa && !s.record_sessions);
    }

    /// The fast renderer is the default (Sarah, 2026-11-20); a file that names the exact one
    /// keeps it, and one that names neither takes the default.
    #[test]
    fn the_fast_renderer_is_the_default() {
        assert!(Settings::default().fast_renderer);
        let mut s = Settings::default();
        s.restore("renderer = exact\n", &Given::default());
        assert!(!s.fast_renderer);
        let mut s = Settings::default();
        s.restore("msaa = 4\n", &Given::default());
        assert!(s.fast_renderer);
    }

    /// High refresh and just-in-time ticks are on by default; an older file's Interpolate reads
    /// as on, and the file keeps on or off.
    #[test]
    fn high_refresh_and_timing_defaults() {
        let s = Settings::default();
        assert_eq!(
            (s.high_refresh, s.tick_timing),
            (Mode::Extrapolate, Timing::Auto)
        );
        let mut s = Settings::default();
        s.restore("high-refresh = interpolate\n", &Given::default());
        assert_eq!(s.high_refresh, Mode::Extrapolate);
        s.restore(
            "high-refresh = off\nsettings-version = 2\n",
            &Given::default(),
        );
        assert_eq!(s.high_refresh, Mode::Off);
        let given = Settings {
            high_refresh: Mode::Interpolate,
            ..Settings::default()
        };
        assert!(given.save().contains("high-refresh = on\n"));
    }

    /// A settings file from before the new defaults (no version) that holds the old ones moves
    /// to the new once, keeping anything else it chose; one with the version keeps the old
    /// defaults as the player's choice, and the move writes the version.
    #[test]
    fn old_settings_move_to_the_new_defaults_once() {
        let restored = |text: &str, given: &Given| {
            let mut s = Settings::default();
            s.restore(text, given);
            (s.tick_timing, s.high_refresh)
        };
        let old = "tick-timing = timer\nhigh-refresh = off\nvolume = 50\n";
        assert_eq!(
            restored(old, &Given::default()),
            (Timing::Auto, Mode::Extrapolate)
        );
        // The old code's other values are kept (`interpolate` reads as on).
        assert_eq!(
            restored(
                "tick-timing = vrr\nhigh-refresh = interpolate\n",
                &Given::default()
            ),
            (Timing::Vrr, Mode::Extrapolate)
        );
        // Turned off after the move: kept.
        let mut s = Settings::default();
        s.restore(old, &Given::default());
        s.tick_timing = Timing::Timer;
        s.high_refresh = Mode::Off;
        let saved = s.save();
        assert!(saved.contains("settings-version = 2\n"));
        assert_eq!(
            restored(&saved, &Given::default()),
            (Timing::Timer, Mode::Off)
        );
        // The command line still wins over both.
        let given = Given {
            tick_timing: true,
            high_refresh: true,
            ..Given::default()
        };
        let mut s = Settings {
            tick_timing: Timing::Timer,
            high_refresh: Mode::Off,
            ..Settings::default()
        };
        s.restore(old, &given);
        assert_eq!((s.tick_timing, s.high_refresh), (Timing::Timer, Mode::Off));
    }

    #[test]
    fn an_unreadable_file_changes_nothing() {
        let mut s = Settings::default();
        s.restore(
            "aspect = 1:2\nfullscreen = maybe\nscale = 9\nmsaa = 2\nadded-delay = -1\nhigh-refresh = sometimes\ntick-timing = soon\njit-margin = 99\nrecord = perhaps\nvolume = 101\nmusic = 101\naudio-exclusive = sometimes\nearly-sfx = soon\nlow-spec = maybe\nlow-spec-saved = scale = 99\n\u{0}garbage\n",
            &Given::default(),
        );
        assert_eq!(s, Settings::default());
    }

    /// The launcher's last match file still gives its display and latency settings.
    #[test]
    fn the_launchers_file_is_read() {
        let mut s = Settings::default();
        s.restore(
            "p1 = fox\nstage = fd\nscale = 4\nmsaa = 4\npresent = fifo\nrecord = off\n",
            &Given::default(),
        );
        assert_eq!(
            (s.scale, s.msaa, s.present, s.record),
            (4, true, Present::Fifo, false)
        );
    }

    /// The developer setting keeps its level, off by default, and nothing else reads as one.
    #[test]
    fn the_debug_level_is_kept() {
        assert_eq!(Settings::default().debug_level, 0);
        for level in [0, 3, 4] {
            let s = Settings {
                debug_level: level,
                ..Settings::default()
            };
            let mut back = Settings::default();
            back.restore(&s.save(), &Given::default());
            assert_eq!(back, s);
        }
        let mut s = Settings::default();
        s.restore("debug-level = 2\n", &Given::default());
        assert_eq!(s.debug_level, 0);
    }

    /// The Ruleset setting is kept, the default ([`DEFAULT_RULESET`]) until one is picked, and the
    /// command line's wins; nothing else reads as one.
    #[test]
    fn the_ruleset_is_kept() {
        assert_eq!(Settings::default().ruleset, DEFAULT_RULESET);
        for r in [Ruleset::Vanilla, Ruleset::Slippi] {
            let s = Settings {
                ruleset: r,
                ..Settings::default()
            };
            let mut back = Settings::default();
            back.restore(&s.save(), &Given::default());
            assert_eq!(back, s);
            assert_eq!(parse_ruleset(ruleset_key(r)), Some(r));
        }
        let mut s = Settings::default();
        s.restore("ruleset = dolphin\n", &Given::default());
        assert_eq!(s.ruleset, DEFAULT_RULESET);
        let given = Given {
            ruleset: true,
            ..Given::default()
        };
        let mut s = Settings {
            ruleset: Ruleset::Vanilla,
            ..Settings::default()
        };
        s.restore("ruleset = slippi\n", &given);
        assert_eq!(s.ruleset, Ruleset::Vanilla);
    }

    /// Only the Slippi ruleset saves replays, and a `settings.txt` with `record = on` under
    /// Vanilla stays as it was: Save replays stays on, kept for the Slippi ruleset, and is
    /// written back unchanged.
    #[test]
    fn save_replays_is_kept_under_vanilla() {
        assert!(!crate::saves_replays(Ruleset::Vanilla));
        assert!(crate::saves_replays(Ruleset::Slippi));
        let file = "ruleset = vanilla\nrecord = on\nreplays = D:/replays\n";
        let mut s = Settings::default();
        s.restore(file, &Given::default());
        assert_eq!((s.ruleset, s.record), (Ruleset::Vanilla, true));
        assert_eq!(s.replays, "D:/replays");
        let mut back = Settings::default();
        back.restore(&s.save(), &Given::default());
        assert_eq!(back, s);
        assert!(s.save().contains("\nrecord = on\n"), "{}", s.save());
    }

    /// Save replays follows the ruleset chosen, not the one Melee runs: chosen Slippi, it can be
    /// changed while Vanilla still runs; chosen Vanilla, it's off and greyed while Slippi still
    /// runs; either way with the note that it applies from the next start.
    #[test]
    fn save_replays_follows_the_chosen_ruleset() {
        let (v, s) = (Ruleset::Vanilla, Ruleset::Slippi);
        assert_eq!(
            replays_box(s, v),
            (true, REPLAYS_SLIPPI_HELP, Some(REPLAYS_NEXT_START))
        );
        assert_eq!(
            replays_box(v, s),
            (false, REPLAYS_VANILLA, Some(REPLAYS_NEXT_START))
        );
        assert_eq!(replays_box(s, s), (true, REPLAYS_SLIPPI_HELP, None));
        assert_eq!(replays_box(v, v), (false, REPLAYS_VANILLA, None));
    }

    /// The audio settings come back, and the command line's exclusive mode wins; no device is
    /// the system's default.
    #[test]
    fn the_audio_settings_are_kept() {
        let mut s = Settings::default();
        assert_eq!(
            (s.volume, s.music, s.early_sfx, s.output()),
            (100, 100, true, melee_audio::OutputOptions::default())
        );
        s.restore(&picked().save(), &Given::default());
        assert_eq!(
            (s.volume, s.music, s.audio_exclusive, s.early_sfx),
            (35, 60, true, false)
        );
        assert_eq!(s.output().device.as_deref(), Some("Speakers (USB Audio)"));
        let mut s = Settings::default();
        let given = Given {
            audio_exclusive: true,
            ..Given::default()
        };
        s.restore("audio-exclusive = on\naudio-device = \n", &given);
        assert!(!s.audio_exclusive);
        assert_eq!(s.output().device, None);
    }

    #[test]
    fn dates_are_civil() {
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(civil(20_754), (2026, 10, 28));
    }
}
