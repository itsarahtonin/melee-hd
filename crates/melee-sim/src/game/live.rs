//! The Slippi ruleset's live matches (`docs/slippi-ruleset.md`, task 4 and design call 3): each
//! VS match a session under the ruleset starts is built from the game start its recording will
//! hold, exactly as Slippi's playback builds a replay's (`vs_match::Settings::live`), so its
//! `.slp` plays back as it played. The menus' session is set aside across the match's scene
//! (`session::Session`'s handoff) and takes back what the match leaves for the scenes after
//! it ([`Back`]).
//!
//! A match whose setup isn't wholly in its game start can't be built that way
//! ([`rebuild_refusal`], design call 6): it runs in the menus' session as the console runs it,
//! under the ruleset's rules, and saves no `.slp`.

use melee_data::archive::Archives;

use super::gm::machine::{self, GS_SUDDEN_DEATH, GS_VS, Phase};
use super::gm::match_exit::MatchExitInfo;
use super::gm::vs_mode::Special;
use super::state::{Game, Ruleset, Running};

/// Why the VS-scene match about to enter in `r` can't be built from its game start, as
/// Slippi's playback builds a replay's match (design call 6 of `docs/slippi-ruleset.md`), or
/// none when it can. Playback runs every match in the debug VS mode's VS scene, from the
/// `StartMeleeData` its recording holds with the callbacks cleared (0x40 to 0x5C): a match is
/// rebuilt only where its scene, its mode and its start data leave nothing else to the match.
/// Only a session under the Slippi ruleset builds live matches.
pub fn rebuild_refusal(r: &Running) -> Option<&'static str> {
    let g = &*r.game;
    if g.ruleset != Ruleset::Slippi {
        return Some("Vanilla's matches run in the menus' session");
    }
    let Phase::Entering {
        scene_mode, state, ..
    } = g.machine.phase
    else {
        return Some("no scene is entering");
    };
    match machine::scene_kind(Some((scene_mode, state))) {
        Some(GS_VS) => {}
        Some(GS_SUDDEN_DEATH) => return Some(SUDDEN_DEATH),
        _ => return Some("not a VS scene's match"),
    }
    if let Some(why) = mode_refusal(scene_mode) {
        return Some(why);
    }
    let start = super::record::export_start(r.scene_start_data());
    if start[CALLBACKS].iter().any(|&b| b != 0) {
        return Some(CALLBACK);
    }
    None
}

/// `StartMeleeData`'s callbacks, which playback clears.
const CALLBACKS: core::ops::Range<usize> = 0x40..0x5C;

/// The reasons [`rebuild_refusal`] gives, for the docs and the tests.
pub const SUDDEN_DEATH: &str = "Sudden Death's scene enters its match with its own announcement (`gm_Scene_SuddenDeath_OnEnter`), where playback enters every match through the VS scene's";
pub const CALLBACK: &str = "its start data holds a callback, which playback clears (0x40 to 0x5C)";
pub const SUPER_SUDDEN_DEATH: &str = "Super Sudden Death's music follows its game mode (`Stage_80225074`), which playback's debug VS mode doesn't have";
pub const STAMINA: &str = "Stamina Mode's match start is a callback playback clears, and its knockout's slowdown follows its game mode (`gm_80167470`)";
pub const FIXED_CAMERA: &str =
    "Fixed-Camera Mode's camera is its unpause hook, a callback playback clears";
pub const SINGLE_PLAYER: &str =
    "a single-player mode's match (design call 7): playback can't set up the mode";
pub const ATTRACT: &str =
    "the title's attract match: its camera is a start hook playback clears, and nobody plays it";
pub const CHALLENGER: &str = "a challenger's match: only the first player pauses it (`gm_DefaultVSGetPauser` reads the game mode)";
pub const DEBUG: &str = "the debug menus' matches, which only Vanilla's developer setting reaches";

/// Why matches of game mode `mode` can't be rebuilt, whatever their start data: what the
/// match reads of its game mode, beside the start data (`docs/slippi-ruleset.md`, call 6).
fn mode_refusal(mode: u8) -> Option<&'static str> {
    use super::gm::{GM_DEBUG, GM_DEBUG_VS, one_player, tournament};
    match mode {
        machine::GM_VS | tournament::GM_TOURNAMENT => None,
        machine::GM_OPENING_MV => Some(ATTRACT),
        machine::GM_CHALLENGER_APPROACH => Some(CHALLENGER),
        GM_DEBUG | GM_DEBUG_VS => Some(DEBUG),
        one_player::GM_HOME_RUN => Some(SINGLE_PLAYER),
        m => match Special::from_game_mode(m) {
            Some(Special::SuperSuddenDeath) => Some(SUPER_SUDDEN_DEATH),
            Some(Special::Stamina) => Some(STAMINA),
            Some(Special::FixedCamera) => Some(FIXED_CAMERA),
            // Their entries fill in only start data the match reads (`docs/oracle.md`,
            // "Special Melee"): the players' size, ratios and invisibility, the single
            // button, the game's speed. A slowed game's recording plays back without most of
            // its sound effects, which playback's sound log cuts (`docs/engine.md`), as Slippi
            // Dolphin's own do; the live match keeps them (Sarah, 2026-11-18).
            Some(
                Special::Invisible
                | Special::Tiny
                | Special::Giant
                | Special::Lightning
                | Special::SlowMo
                | Special::SingleButton,
            ) => None,
            None => Some(SINGLE_PLAYER),
        },
    }
}

/// What a live match leaves the menus' session for the scenes after it, as the console's one
/// session would hold it: the scene's exit data with the match's results
/// (`gm_Scene_Vs_OnExit`'s, which reads the match), the characters its humans played (for
/// the save, which the menus' session holds), the random seed and UCF's pad buffer as the
/// match leaves them, and how the scene ended (`gm_80479D58.unk_C`: whether its last round
/// rendered).
#[derive(Clone, Debug)]
pub struct Back {
    pub exit: MatchExitInfo,
    pub played: Vec<u8>,
    pub seed: u32,
    pub ucf: super::ucf::Pads,
    pub end: i32,
}

impl Running<'_> {
    /// What the live match in `self` leaves at its scene's end ([`Back`]): its VS scene's exit
    /// computed on the match.
    pub fn live_match_back(&mut self) -> Back {
        let exit = self.vs_scene_exit_info();
        Back {
            exit,
            played: self.game.characters_played(),
            seed: self.game.objs.rng.seed,
            ucf: self.game.ucf,
            end: self.game.scene.unk_C,
        }
    }
}

impl Game {
    /// The menus' session, set aside as its VS scene's match entered, takes `back`: the
    /// scene stands entered and ended as the match's did, its exit data and the save as its
    /// exit leaves them ([`Running::vs_scene_on_exit`], which then doesn't run again), and the
    /// seed and UCF's buffer as the match left them. The menus' heap, image and pools, its
    /// audio and timeline and the preload cache stay as the scene's entry left them: the
    /// match's own were playback's.
    pub fn take_back_live_match(&mut self, back: Back) {
        let Phase::Entering {
            mode,
            scene_mode,
            state,
            overridden,
        } = self.machine.phase
        else {
            panic!("a live match's handback with no scene entering");
        };
        let m = &mut self.machine;
        m.files_wanted = false;
        m.scene_info = Some((scene_mode, state));
        m.phase = Phase::Scene {
            mode,
            scene_mode,
            state,
            overridden,
        };
        m.live_match = true;
        *self.match_exit_data(scene_mode, state) = back.exit;
        self.mark_played(&back.played);
        self.objs.rng.seed = back.seed;
        self.ucf = back.ucf;
        self.scene.unk_C = back.end;
    }
}

/// The counters a match carries on from the scenes before it, which no scene resets: the
/// collision joints' stamp (`Ground_804D6954`), the collision geometry's update counter
/// (`mpColl_804D64AC`), the particle generators' last family id (`lbl_804D6368`) and the
/// particles' display frame with the frame each link was last sorted in (`psFrameNum`,
/// `HSD_PSDisp_8040C360`). A live match starts them as playback's boot leaves them, the
/// console's match as the menus left them, so the menus' session takes each back moved on as
/// far as the match moved it ([`Game::take_match_statics`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Counters {
    pub coll_stamp: i16,
    pub coll_generation: i32,
    pub idnum: u16,
    pub ps_frame: u8,
    pub ps_sorted: [u8; crate::hsd::particle::LINKS],
}

impl Counters {
    /// The counters as `g` holds them.
    pub fn of(g: &Game) -> Self {
        Counters {
            coll_stamp: g.gr.coll_stamp,
            coll_generation: super::gr::ground::generation(g),
            idnum: g.objs.generators.idnum,
            ps_frame: g.objs.psdisp.frame,
            ps_sorted: g.objs.psdisp.sorted,
        }
    }
}

/// What the console's VS scene's entry leaves that a live match, built as playback builds a
/// replay's, doesn't start with ([`Session`]'s handoff, `session::Aside`): the [`Counters`];
/// the VS controller's players (`fn_8016DCC0`'s: a human's Zelda held A is Zelda there, made
/// Sheik with the `0x08` mark, where the live match's game start holds Sheik); and the game
/// camera, whose bounds' width average (`Camera.x2B0`, its sum and count) and pause camera's
/// eye, target and limits (`Camera_Init` leaves them) the console's match takes on from the
/// matches before it.
///
/// [`Session`]: super::session::Session
#[derive(Clone, Debug)]
pub struct ConsoleAt {
    pub counters: Counters,
    pub vs_fighters: [super::structs::VsSceneFighter; 6],
    pub camera: super::structs::Camera,
    /// The statics no scene resets that a match writes only in passing, which the console's
    /// match takes on from the ones before it ([`Game::take_console_statics`]).
    pub carried: Carried,
}

/// Statics no scene resets, which a match writes only in passing and reads only where it has
/// written them first: the off-screen arrows' colors and slots (`un_802FE260` resets which
/// are shown, not these; an arrow shows only once `un_802FD928` colors it), efLib's parameter
/// table's effect ids (`efLib_Init` clears an entry's GObj and alpha; an id is read only for an
/// effect whose spawn set it), the players' start points (measured by the setups that read
/// them), the items' copy of the fighters (written each tick before the items read it) and
/// mpcoll's last wall check (its counts reset per check, its correction read only once a wall
/// beat it). Nothing in a match reads them before writing, so a live match can take the
/// console's and stay its playback's in play.
#[derive(Clone, Debug)]
pub struct Carried {
    pub arrows: ([super::structs::_GXColor; 4], [u8; 4]),
    pub ef_ids: [u16; 8],
    pub start_points: [crate::vec::Vec3; 4],
    pub ft_track: super::structs::Item_FtTrack,
    pub coll: crate::mp::coll::CollStatics,
}

impl Carried {
    fn of(g: &Game) -> Self {
        Carried {
            arrows: (g.hud.arrows.x14.clone(), g.hud.arrows.x2C),
            ef_ids: g.ef.params.map(|p| p.gfx_id),
            start_points: g.gr.start_points,
            ft_track: g.it.ft_track.clone(),
            coll: g.coll.clone(),
        }
    }

    fn put(&self, g: &mut Game) {
        (g.hud.arrows.x14, g.hud.arrows.x2C) = (self.arrows.0.clone(), self.arrows.1);
        for (p, &id) in g.ef.params.iter_mut().zip(&self.ef_ids) {
            p.gfx_id = id;
        }
        g.gr.start_points = self.start_points;
        g.it.ft_track.clone_from(&self.ft_track);
        // The pass's floor filter's GObj (none between passes) stays the game's own.
        let c = &mut g.coll;
        let (gobj, stale, watchdog) = (c.floor_filter_gobj, c.stale_reads, c.watchdog);
        c.clone_from(&self.coll);
        (c.floor_filter_gobj, c.stale_reads, c.watchdog) = (gobj, stale, watchdog);
    }
}

impl ConsoleAt {
    /// Where the console stands in `g`, its VS scene's entry run.
    pub fn of(g: &Game) -> Self {
        ConsoleAt {
            counters: Counters::of(g),
            vs_fighters: g.vs.state.fighters.clone(),
            camera: g.camera.cam.clone(),
            carried: Carried::of(g),
        }
    }
}

impl Game {
    /// A live match built from its game start goes on with what the console's camera keeps
    /// from the matches before it (`at.camera`; `Camera_Init` leaves it): the bounds' width
    /// average, which only the bonuses read (`pl::tally`, the results' camera bonuses), and the
    /// pause camera's eye, target, player and limits, which only the pause reads (its orbit and
    /// pan, `Camera_8002BAA8` to `Camera_8002C010`, and the slot it steps from). Slippi
    /// Dolphin's live play has the console's; playback's own width average, which its camera
    /// then holds, is what the display-phase tracker follows (`PlaybackCamera`), taken now as
    /// playback's camera starts it. And the statics no scene resets that a match reads only
    /// where it wrote them ([`Carried`]).
    pub fn take_console_statics(&mut self, at: &ConsoleAt) {
        at.carried.put(self);
        let c = &mut self.camera.cam;
        self.display_phase.playback.take(c);
        let k = &at.camera;
        (c.x2B0, c.x2B4, c.x2B8) = (k.x2B0, k.x2B4, k.x2B8);
        (c.x2C4, c.x2C5) = (k.x2C4, k.x2C5);
        c.x2D0.clone_from(&k.x2D0);
        (c.x304, c.x305, c.x306, c.x307) = (k.x304, k.x305, k.x306, k.x307);
        (c.x308, c.x314, c.pause_eye_offset) = (k.x308, k.x314, k.pause_eye_offset);
        (c.x32C, c.pause_eye_distance, c.pause_up) = (k.x32C, k.pause_eye_distance, k.pause_up);
    }
}

/// `x` moved on by `from` to `to` in a counter that runs `lo..=hi` and starts again at `lo`.
fn moved_on(x: u32, from: u32, to: u32, (lo, hi): (u32, u32)) -> u32 {
    let n = hi - lo + 1;
    let steps = (to + n - lo - ((from - lo) % n)) % n;
    lo + (x - lo + steps) % n
}

/// `VsSceneFighter.x4`'s mark of a human's Zelda made Sheik by A held (`fn_8016D8AC`).
const ZELDA_HELD_A: u8 = 0x08;

/// The family ids' range (`hsd_8039D1EC`) and the display frame's (`psDispParticles`).
const IDNUMS: (u32, u32) = (0x100, 0xFFFF);
const PS_FRAMES: (u32, u32) = (1, 0xFF);

impl Game {
    /// The menus' session, taking back a live match's scene, takes the statics the console's
    /// match leaves for the scenes after it ([`Back`]'s are the scene's exit's): the VS
    /// controller (`gmvs.c`'s, its rules, timer and results), the live standings
    /// (`gm_16F1.c`), the crowd, the random items' tables and the fighters' places the items
    /// keep clear of, the hit check's clank switches, the game camera but its GObjs, the
    /// stage's default field of view, the magnifier's colors, states and bubbles' images, the
    /// HUD's arrows' and announcements' values, the background flash's scale, the pauser,
    /// efLib's count and load kind, the [`Carried`] statics (the live match took the console's
    /// at the handoff), the stage's music flags, and the [`Counters`], each moved on
    /// as far as the match moved it from `at` (the live match's as its entry left them) to
    /// where the console's entry left them (`entry`); the VS controller's players as the
    /// console's entry made them (`entry`). Handles and pointers into the match's heap and
    /// files stay the menus': on the console they name freed memory, which nothing reads
    /// before setting; and the image knows nothing of the main heap, which the console's match
    /// held (`docs/oracle.md`, "Past a live match's handback").
    pub fn take_match_statics(&mut self, live: &Game, (entry, at): (&ConsoleAt, &Counters)) {
        #[cfg(feature = "oracle-hooks")]
        if super::oracle_hooks::handback_statics_off() {
            return;
        }
        let console = &entry.counters;
        self.vs.clone_from(&live.vs);
        // The players as the console's entry made them (Zelda held A), the match's since.
        for (f, c) in self.vs.state.fighters.iter_mut().zip(&entry.vs_fighters) {
            f.x0 = c.x0;
            f.x4[0] = (f.x4[0] & !ZELDA_HELD_A) | (c.x4[0] & ZELDA_HELD_A);
        }
        self.gm.ranks.clone_from(&live.gm.ranks);
        self.crowd = live.crowd;
        self.it.spawn.clone_from(&live.it.spawn);
        self.it.clank_boxes = live.it.clank_boxes;
        let (gobj, quake) = (self.camera.cam.gobj, self.camera.cam.quake_gobj);
        self.camera.cam.clone_from(&live.camera.cam);
        (self.camera.cam.gobj, self.camera.cam.quake_gobj) = (gobj, quake);
        self.camera.fov = live.camera.fov;
        let (m, l) = (&mut self.magnify, &live.magnify);
        (m.x4, m.x8, m.xC, m.x10) = (l.x4.clone(), l.x8.clone(), l.xC.clone(), l.x10.clone());
        m.image_descs.clone_from(&l.image_descs);
        for (p, q) in m.player.iter_mut().zip(&l.player) {
            p.state.clone_from(&q.state);
            if let Some(crate::hsd::tobj::ImageRef::Static(_)) = q.idesc.0 {
                p.idesc = q.idesc;
            }
        }
        // The HUD's values the next match's setup leaves: the off-screen arrows' colors and
        // slots (`un_802FE260` resets which are shown), the announcements' callbacks, lines
        // and marks; their models and GObjs stay the menus'.
        let (a, b) = (&mut self.hud.arrows, &live.hud.arrows);
        (a.x24, a.x28, a.x30, a.x31, a.x32) = (b.x24, b.x28, b.x30, b.x31, b.x32);
        for (e, l) in self
            .hud
            .announcements
            .iter_mut()
            .zip(&live.hud.announcements)
        {
            let (gobj, model) = (e.x0, e.x14);
            e.clone_from(l);
            (e.x0, e.x14) = (gobj, model);
        }
        self.bgflash.scale = live.bgflash.scale;
        self.gm.pause.slot = live.gm.pause.slot;
        self.ef.count = live.ef.count;
        self.ef.load_kind = live.ef.load_kind;
        Carried::of(live).put(self);
        self.stage_info.unk8C.clone_from(&live.stage_info.unk8C);
        // The counters, moved on by the match.
        let now = Counters::of(live);
        self.gr.coll_stamp = console
            .coll_stamp
            .wrapping_add(now.coll_stamp.wrapping_sub(at.coll_stamp));
        let generation = console
            .coll_generation
            .wrapping_add(now.coll_generation.wrapping_sub(at.coll_generation));
        match &mut self.geometry {
            Some(g) => g.generation = generation,
            None => self.gr.coll_generation = generation,
        }
        let shift = |x: u8, range| {
            moved_on(
                u32::from(x),
                u32::from(at.ps_frame),
                u32::from(console.ps_frame),
                range,
            ) as u8
        };
        self.objs.generators.idnum = moved_on(
            u32::from(now.idnum),
            u32::from(at.idnum),
            u32::from(console.idnum),
            IDNUMS,
        ) as u16;
        let d = &mut self.objs.psdisp;
        d.frame = shift(now.ps_frame, PS_FRAMES);
        for link in 0..crate::hsd::particle::LINKS {
            // A link the match sorted: its frame in the console's numbering, and its cached
            // particle freed with the match's (`Globals::stale`).
            if now.ps_sorted[link] != at.ps_sorted[link] {
                d.sorted[link] = shift(now.ps_sorted[link], PS_FRAMES);
                d.non_edge[link] = None;
                d.stale[link] = live.objs.psdisp.non_edge[link].is_some();
            } else {
                d.sorted[link] = console.ps_sorted[link];
            }
        }
    }

    /// The menus' session, taking back a live match's scene, forgets what its heap's image
    /// held of the main heap: the console's match held it, laid out and written as melee-hd
    /// doesn't model (the live match's heap is playback's), so a new object's leftovers there
    /// are unknown (`hsd::image::LIVE_MATCH`) and read as zero.
    pub fn forget_live_match_heap(&mut self) {
        let h = &mut self.objs.heap;
        if let Some((lo, hi)) = h.os.range() {
            h.image.write(
                lo,
                hi,
                crate::hsd::image::Src::Unknown(crate::hsd::image::LIVE_MATCH),
            );
        }
    }
}

/// Where the console stands between a live match and the menus' session: its audio, the
/// timeline in it, and its pad library, whose queue holds the polls the timeline counted, the
/// statuses the last tick renewed from it, and the controllers' rumble. The console's VS
/// scene's entry stalls (its sound banks' loads, `HSD_SynthSFXWaitForLoadCompletion`, 1.4 to
/// 2.8 fields at Slippi's fast disc speed), the pad alarm polling meanwhile, so the live match
/// takes up the console's timeline as the scene's loop starts ([`Game::go_on_from`]): its
/// ticks wait for the same polls and render on the same rounds as the console's, and each
/// takes the same reads; the menus' session takes up the match's as its scene ends. Only which
/// read becomes which frame's input follows from it, and the timing: the game start, the
/// frames, their inputs once read and the seed are the match's own (`docs/slippi-ruleset.md`,
/// "The scene entry's stall").
///
/// The audio goes with it whole (Sarah, 2026-11-16: "definitely a thing we want"): the
/// driver's voices and their handles, the synth, the AX library, the sound banks and the audio
/// heap, the music stream and the device command queue, the game's audio library, the movie
/// player and the save's writer, all with their work in flight on the timeline
/// ([`crate::audio::Audio::take_up`]). So the console's last menu sound plays on into the
/// match and the match's sounds take the voices and handles the console's would, and the
/// scenes after the match go on from the match's sounds. The preload cache, which the audio
/// holds for its timeline, stays out of the live match (the game's code reads its presence as a
/// session's) and goes back to the menus' session as the console's entry left it
/// (`session::Aside`). Nothing of it is recorded: a `.slp` holds the match's start, inputs and
/// outcomes, none of its audio.
#[derive(Clone, Debug)]
pub struct Timeline {
    pub audio: Box<crate::audio::Audio>,
    pub lib: crate::input::controller::PadLib,
    pub master: [crate::input::controller::Status; 4],
    pub copy: [crate::input::controller::Status; 4],
    pub game: [crate::input::controller::Status; 4],
    pub rumble: crate::input::rumble::Rumble,
    pub rumble_ahead: u32,
}

impl Game {
    /// Where the console stands in this game ([`Timeline`]), a copy: the audio's mix log
    /// (presentation, `Audio::mix`) stays.
    pub fn timeline(&self) -> Timeline {
        let p = &self.pads;
        Timeline {
            audio: Box::new(self.audio.clone()),
            lib: p.lib.clone(),
            master: p.master,
            copy: p.copy,
            game: p.game,
            rumble: p.rumble.clone(),
            rumble_ahead: p.rumble_ahead,
        }
    }

    /// [`Game::timeline`] for a game that goes no further: its audio moved out, the mix log
    /// with it, so the presentation's mixer goes on with the game that takes it up.
    pub fn leave_timeline(&mut self) -> Timeline {
        let mut at = self.timeline();
        at.audio.mix = self.audio.mix.take();
        at
    }

    /// Goes on from where the console stands in `at` ([`Timeline`]), its archives numbered in
    /// `theirs`: its audio and timeline, their archives found or shared in `ours`
    /// ([`crate::audio::Audio::rehome`]) and the preload cache kept
    /// ([`crate::audio::Audio::take_up`]), its pad library's queue and statuses, and the
    /// rumble, its scripts read from this game's rumble data (the same file). This game's own
    /// audio goes: a live match's is playback's, the menus' as their scene's entry began.
    pub fn go_on_from(&mut self, at: Timeline, theirs: &Archives, ours: &mut Archives) {
        let mut audio = at.audio;
        audio.rehome(theirs, ours);
        self.audio.take_up(*audio);
        let p = &mut self.pads;
        p.lib.clone_from(&at.lib);
        p.master = at.master;
        p.copy = at.copy;
        p.game = at.game;
        if let Some(d) = self.rumble_data {
            p.rumble = at.rumble.with_data_in(d.archive);
            p.rumble_ahead = at.rumble_ahead;
        }
    }
}

/// What the sounds the VS scene's entry starts do (`gm_Scene_Vs_OnEnter`, [`in_entry`]: a
/// stage's at its setup, Big Blue's first cars' engines), on this thread.
#[derive(Default)]
enum EntrySounds {
    /// Start as always.
    #[default]
    Off,
    /// Start as always, each sound and the handle the driver gave it kept: the console's
    /// entry ([`record_entry_sounds`]).
    Record(Vec<(i32, i32)>),
    /// Start nowhere, each taking the handle the console's entry gave the same sound: the live
    /// match's entry ([`with_entry_sounds`]).
    Replay(std::collections::VecDeque<(i32, i32)>),
}

std::thread_local! {
    static ENTRY_SOUNDS: core::cell::RefCell<EntrySounds> = const {
        core::cell::RefCell::new(EntrySounds::Off)
    };
    static IN_ENTRY: core::cell::Cell<bool> = const { core::cell::Cell::new(false) };
}

/// Puts the thread's [`EntrySounds`] back to [`EntrySounds::Off`] when dropped (a stop
/// unwinding through it included), and the entry's mark with it.
struct EntrySoundsOff;

impl Drop for EntrySoundsOff {
    fn drop(&mut self) {
        ENTRY_SOUNDS.with(|e| *e.borrow_mut() = EntrySounds::Off);
        IN_ENTRY.set(false);
    }
}

/// Runs `f` as the VS scene's entry (`gm::vs`'s `vs_on_enter`): the sounds it starts are the
/// entry's ([`EntrySounds`]).
pub fn in_entry<R>(f: impl FnOnce() -> R) -> R {
    let was = IN_ENTRY.replace(true);
    let r = f();
    IN_ENTRY.set(was);
    r
}

/// Runs `f`, the console's scene change into the VS scene ([`Running::next_scene`] on a copy
/// of the menus' session), keeping each sound its entry starts and the handle it got, in order.
pub fn record_entry_sounds<R>(f: impl FnOnce() -> R) -> (R, Vec<(i32, i32)>) {
    let off = EntrySoundsOff;
    ENTRY_SOUNDS.with(|e| *e.borrow_mut() = EntrySounds::Record(Vec::new()));
    let r = f();
    let kept = ENTRY_SOUNDS.with(|e| std::mem::take(&mut *e.borrow_mut()));
    drop(off);
    let EntrySounds::Record(sounds) = kept else {
        unreachable!("the entry's sounds were being kept")
    };
    (r, sounds)
}

/// Runs `f`, the live match's construction (`vs_match::Match::start`), its VS scene's entry
/// giving each sound it starts the handle the console's entry gave it (`sounds`, from
/// [`record_entry_sounds`]), and starting none: the match takes the console's audio as the
/// entry left it ([`Game::go_on_from`]), those sounds playing in it with those handles, which
/// the match's stage then holds as the console's does. Stops by name where the two entries'
/// sounds differ, but for the console's Sheik transformation sound ([`SHEIK_TRANSFORM`]).
pub fn with_entry_sounds<R>(sounds: Vec<(i32, i32)>, f: impl FnOnce() -> R) -> R {
    let off = EntrySoundsOff;
    ENTRY_SOUNDS.with(|e| *e.borrow_mut() = EntrySounds::Replay(sounds.into()));
    let r = f();
    let left = ENTRY_SOUNDS.with(|e| std::mem::take(&mut *e.borrow_mut()));
    drop(off);
    if let EntrySounds::Replay(left) = left
        && let Some((id, _)) = left.iter().find(|s| s.0 != SHEIK_TRANSFORM)
    {
        panic!(
            "the live match's VS scene entry didn't start sound {id:#x}, which the console's started ({} not started)",
            left.len()
        );
    }
    r
}

/// The handle the VS scene's entry gives sound `id` instead of starting it, in the live match's
/// entry ([`with_entry_sounds`]).
pub(crate) fn entry_sound_handle(id: i32) -> Option<i32> {
    if !IN_ENTRY.get() {
        return None;
    }
    ENTRY_SOUNDS.with(|e| match &mut *e.borrow_mut() {
        EntrySounds::Replay(q) => {
            while id != SHEIK_TRANSFORM && q.front().is_some_and(|s| s.0 == SHEIK_TRANSFORM) {
                q.pop_front();
            }
            let Some((theirs, handle)) = q.pop_front() else {
                panic!(
                    "the live match's VS scene entry started sound {id:#x}, which the console's didn't"
                );
            };
            assert_eq!(
                id, theirs,
                "the live match's VS scene entry started sound {id:#x} where the console's started {theirs:#x}"
            );
            Some(handle)
        }
        _ => None,
    })
}

/// The sound the VS scene's entry plays for a Zelda held A into Sheik (`gm::vs`, the
/// player's flag `x4 & 8`), its handle kept by no one: the console's entry plays it, and the
/// live match, whose game start already holds Sheik (`record::live_start`, as the recording
/// codes write it and playback builds it), plays none. It plays on in the console's audio the
/// match takes, as in Slippi Dolphin's live play, so the match's entry passes over it
/// ([`with_entry_sounds`]).
const SHEIK_TRANSFORM: i32 = 0x41F4E;

/// The VS scene's entry started sound `id` with `handle`, kept in the console's entry
/// ([`record_entry_sounds`]).
pub(crate) fn entry_sound_started(id: i32, handle: i32) {
    if !IN_ENTRY.get() {
        return;
    }
    ENTRY_SOUNDS.with(|e| {
        if let EntrySounds::Record(v) = &mut *e.borrow_mut() {
            v.push((id, handle));
        }
    });
}

/// Fighter slots [`DisplayPhase`] keeps a placement for.
const PLACED: usize = 8;

/// Design call 9 of `docs/slippi-ruleset.md`: display-phase reads Slippi doesn't fix, computed
/// both ways and noted where the outcomes differ, never stopping the match. A fighter KO'd
/// toward the screen (`x2220_b7`) has its root joint placed by the camera in the display
/// pass (`ft::co::dead`'s `place_by_camera`; Slippi's FreezeDeadUpFallPhysics keeps its
/// position out of it) and put back at its position by `Fighter_procMap` each tick, so a
/// read through its joints before that (a homing missile's or a nearest-opponent item's
/// target, `it::owner`) sees the pass's placement after a display pass and the logic's on a
/// doubled round's second tick, where no pass ran: which one depends on where the doubled
/// rounds fall, which Slippi Dolphin's playback decides for itself. Each such read is also
/// computed with the other placement (the joints shifted by the root's difference), and a
/// different outcome is counted; the recording's metadata and the player's log say the match
/// may not play back exactly.
///
/// Three stages' logic reads the game camera's view edges (`camera::ground_span`), which only
/// the display pass sets in the camera's CObj: Onett's cars (their horn and centre decisions),
/// Kongo Jungle 64's birds and Dream Land 64's clouds and stars (where their flights start,
/// a stage object's joint the oracle compares). A tick after a pass reads the view that pass
/// set, a doubled round's second tick the one before. On those stages the view a pass would
/// give is kept at each tick's end ([`Game::display_phase_tick_end`]), and each such read is
/// also made through the other phase's view ([`DisplayPhase::other_view`]), a different
/// outcome counted the same way (`gr::onett::proc_cars`, `gr::oldkongo::proc_birds`,
/// `gr::oldpupupu::proc_flights`). Pokémon Stadium's close-up reads the CObj whole (where its
/// player's camera bone falls on the screen, which decides where the monitor copies from,
/// `gr::stadium::zoom_origin`): on Stadium the views are kept too, and where it copies from is
/// also taken through the other phase's view ([`Game::other_phase_screen`]), a different origin
/// counted. After a pause, every such read is counted ([`DisplayPhase::paused`]): the pause
/// leaves the camera off the course playback's takes.
///
/// The pause reaches the camera's other readers in the logic too (`docs/engine.md`, "The
/// camera's readers after a pause"): those that read its transform or its CObj count each
/// read after a pause ([`Game::note_camera_read`]: Corneria's city part, Mute City's wrecked
/// cars, the enemies flung toward the screen, Scizor, the Unown, Pokémon Stadium's close-up,
/// a nearest-opponent read through a fighter the camera places); the width average and the
/// quake counters, which a pause moves only where its camera runs the standard update, are
/// followed as playback's camera has them ([`PlaybackCamera`]), and the bonuses' and the
/// looping quake's reads of them are made both ways.
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct DisplayPhase {
    /// The root translation the display pass last gave each fighter it places by the camera,
    /// by fighter slot.
    pub placed: [Option<(u16, crate::vec::Vec3)>; PLACED],
    /// The views a display pass would give the game camera after each of the last two ticks,
    /// the last first (on the stages that read them, [`Game::keeps_display_views`]; none in a
    /// camera mode that leaves the CObj alone), and the engine tick count
    /// (`gm_80479D58.unk_0`) after the last.
    pub views: [Option<super::camera::View>; 2],
    pub view_tick: u32,
    /// The game paused (under Slippi's rules, on any stage): the camera's state has left the
    /// course a replay's playback takes, which never pauses, for the rest of the match
    /// (`docs/slippi-bugs.md`, "A paused match's recording"), and each read through its view
    /// or its transform is counted ([`Game::view_read_off_course`],
    /// [`Game::note_camera_read`]).
    pub paused: bool,
    /// How many reads were counted: their outcomes differ between the two placements or
    /// phases, or they were made off playback's course.
    pub differing: u32,
    /// The game ticks of the scene (`gm_80479D58.unk_8`) at the first of them.
    pub first: Option<u32>,
    /// The camera state playback's would hold that the logic reads besides the view.
    pub playback: PlaybackCamera,
}

/// What playback's camera holds, under Slippi's rules, of the camera state the logic reads
/// besides the view: the bounds' width average (`Camera.x2B0`, its sum `x2B4` and count
/// `x2B8`, read by the bonuses, `pl::tally`) and the quake counters (`quake_frames_left`)
/// with whether the looping quake's model stands (`quake_gobj`, which `Camera_RequestQuake`
/// reads and `Camera_UpdateQuakes` frees). Each camera update moves them the same way in any
/// mode but the pause's, which playback never runs; and playback's first frame settles each
/// fighter with a camera update of its own (`Camera_8002F3AC`, `docs/slippi-bugs.md`,
/// "Playback settles each fighter on the first frame, which live play doesn't"), which live
/// play doesn't run. So they're followed here from the camera's, those updates added and the
/// paused ones left out, and the reads made both ways.
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct PlaybackCamera {
    /// Taken from the camera: at its first update, quake request or settling.
    pub on: bool,
    /// The width average, its sum and count.
    pub width: f32,
    pub width_sum: f32,
    pub width_count: i16,
    /// The quake counters, and whether the looping quake's model stands.
    pub quake_frames: [i32; 5],
    pub quake_model: bool,
    /// Playback's settling updates in this tick that the width average and the quake counters
    /// haven't taken yet (the settling runs in the fighters' input, before the camera's own
    /// process; cleared at the tick's end, as a mode that doesn't run one leaves it).
    pub settles_width: u8,
    pub settles_quakes: u8,
}

impl PlaybackCamera {
    /// Takes the camera's state, the first time.
    pub fn take(&mut self, c: &super::structs::Camera) {
        if self.on {
            return;
        }
        *self = PlaybackCamera {
            on: true,
            width: c.x2B0,
            width_sum: c.x2B4,
            width_count: c.x2B8,
            quake_frames: c.quake_frames_left,
            quake_model: c.quake_gobj.is_some(),
            ..*self
        };
    }

    /// The width average as `Camera_80030E10` reads it: 10000 before the first update.
    pub fn average_width(&self) -> f32 {
        if self.width_count < 1 {
            return 10000.0;
        }
        self.width
    }
}

impl DisplayPhase {
    /// The display pass placed fighter `slot`'s root at `root`.
    pub fn place(&mut self, slot: usize, root: crate::vec::Vec3) {
        let slot = slot as u16;
        let at = self
            .placed
            .iter()
            .position(|p| p.is_some_and(|(s, _)| s == slot))
            .or_else(|| self.placed.iter().position(Option::is_none));
        if let Some(i) = at {
            self.placed[i] = Some((slot, root));
        }
    }

    /// The pass's last placement of fighter `slot`'s root.
    pub fn placed(&self, slot: usize) -> Option<crate::vec::Vec3> {
        self.placed
            .iter()
            .flatten()
            .find(|(s, _)| usize::from(*s) == slot)
            .map(|&(_, v)| v)
    }

    /// A tick ended, the engine tick count now `tick`: a display pass now would give the game
    /// camera `due` (none in a mode that leaves the CObj alone).
    pub fn tick_end(&mut self, tick: u32, due: Option<super::camera::View>) {
        self.views = [due, self.views[0]];
        self.view_tick = tick;
    }

    /// The camera view a read in the logic in the tick after engine tick count `now` would see
    /// with the display pass's other phase, the CObj holding `held`. Where `held` is what a
    /// pass gave after the last tick (`views[0]`), the other phase is one that skipped that
    /// pass, a doubled round, its CObj holding what a pass gave after the tick before
    /// (`views[1]`); where it isn't, no pass ran after the last tick (this is a doubled round's
    /// second tick), and the other phase is one whose pass did (`views[0]`). (Where the camera
    /// stood still over the last tick and no pass ran, `held` is both, and so is the other.)
    /// None where the other is `held`, or not known: the match's first tick, a camera mode
    /// that leaves the CObj alone.
    ///
    /// The views are what a pass would give from the camera's state at each tick's end, not
    /// at the read: a pause's start or end between them changes the camera's mode before the
    /// stage's processes run (`gm_DoPauseChecksAndRoutine` in the scene's frame function),
    /// though the camera's own process (priority 0x12) runs after them.
    pub fn other_view(&self, held: super::camera::View, now: u32) -> Option<super::camera::View> {
        if self.view_tick != now {
            return None;
        }
        let other = if self.views[0] == Some(held) {
            self.views[1]
        } else {
            self.views[0]
        };
        other.filter(|&v| v != held)
    }

    /// A read whose outcome differs between the placements, in game tick `tick`.
    pub fn note(&mut self, tick: u32) {
        self.differing = self.differing.saturating_add(1);
        self.first.get_or_insert(tick);
    }
}

impl Game {
    /// Whether [`DisplayPhase`] keeps the views a display pass would give: under Slippi's
    /// rules, on the stages whose logic reads the camera's view edges, and on Pokémon Stadium,
    /// whose close-up reads where its player falls on the screen.
    pub fn keeps_display_views(&self) -> bool {
        use super::gr::{GR_KIND_OLDKONGO, GR_KIND_OLDPUPUPU, GR_KIND_ONETT, GR_KIND_PSTADIUM};
        self.rules.slippi
            && matches!(
                self.stage_info.grkind,
                GR_KIND_ONETT | GR_KIND_OLDKONGO | GR_KIND_OLDPUPUPU | GR_KIND_PSTADIUM
            )
    }

    /// A tick's end, its tick counts done ([`DisplayPhase::tick_end`]); under Slippi's rules,
    /// in the pause camera's mode, the camera off playback's course from now on
    /// ([`DisplayPhase::paused`]).
    pub fn display_phase_tick_end(&mut self) {
        if self.keeps_display_views() {
            let due = super::camera::pass_view(self);
            let tick = self.scene.unk_0;
            self.display_phase.tick_end(tick, due);
        }
        if self.rules.slippi && self.camera.cam.mode == super::camera::PAUSE {
            self.display_phase.paused = true;
        }
        let p = &mut self.display_phase.playback;
        p.settles_width = 0;
        p.settles_quakes = 0;
    }

    /// A read in the logic of the game camera's transform or CObj, under Slippi's rules:
    /// after a pause or in the settling window, which leave them off playback's course
    /// ([`Game::view_read_off_course`]),
    /// it's counted ([`DisplayPhase::note`]) whatever it decides. Playback's camera isn't
    /// followed (that would take a second run of the camera's update over its own boxes, its
    /// quakes and playback's first frame's settling), so the read can't be made both ways.
    pub fn note_camera_read(&mut self) {
        if self.rules.slippi && self.view_read_off_course() {
            let tick = self.scene.unk_8;
            self.display_phase.note(tick);
        }
    }

    /// Fighter `gobj`'s input in a match where playback's first frame's settling doesn't run
    /// (a live match, or a replay played without it): under Slippi's rules, on that frame,
    /// the camera update the settling would run for the fighter is added to what
    /// [`PlaybackCamera`] follows (`playback::playback_inject`, whose fighters it settles).
    pub fn playback_camera_settles(&mut self, gobj: crate::hsd::gobj::GObjId) {
        let Some(p) = self.playback.as_ref() else {
            return;
        };
        if !self.rules.slippi
            || p.frame_index != super::playback::FIRST_FRAME
            || !(p.live || p.no_settling)
            || !self.playback_injects(gobj)
        {
            return;
        }
        let c = &mut self.display_phase.playback;
        c.take(&self.camera.cam);
        c.settles_width = c.settles_width.wrapping_add(1);
        c.settles_quakes = c.settles_quakes.wrapping_add(1);
    }

    /// Whether a read in the logic through the game camera's view is off playback's course,
    /// so that the read's outcome can't be shown to match playback's, and it's counted
    /// ([`DisplayPhase::note`]) whatever it decides:
    ///
    /// - After a pause ([`DisplayPhase::paused`]): the pause camera eases back to the match's
    ///   view but leaves its state a little off the one playback's camera, which never paused,
    ///   has (the interest's depth, the field of view a few ulps short, the eye and interest
    ///   some hundred-thousandths of a unit off, to the match's end in `melee-player`'s
    ///   `a_paused_live_match_notes_its_view_reads`).
    /// - In the settling window ([`SETTLING_WINDOW`]): playback's first frame snaps its camera
    ///   to its targets as it settles each fighter, which live play doesn't, so the live
    ///   match's camera, easing there, differs from playback's until the two meet.
    pub fn view_read_off_course(&self) -> bool {
        self.display_phase.paused || self.in_settling_window()
    }

    /// Whether this match's camera is in the settling window ([`SETTLING_WINDOW`]): under
    /// Slippi's rules, a match whose playback state doesn't run the first frame's settling (a
    /// live match, or a replay played without it), before the window's last frame.
    pub fn in_settling_window(&self) -> bool {
        self.rules.slippi
            && self
                .playback
                .as_ref()
                .is_some_and(|p| (p.live || p.no_settling) && p.frame_index < SETTLING_WINDOW)
    }

    /// Where `pos` falls on the screen (`lb::screen::world_to_screen`) through the game camera's
    /// CObj `c` holding the view the display pass's other phase would leave it
    /// ([`DisplayPhase::other_view`]), where that view isn't the one it holds: the CObj takes
    /// that view for the projection, and it and its eye's and interest's WObjs are put back
    /// whole after, so nothing of it reaches the state.
    pub fn other_phase_screen(
        &mut self,
        data: &melee_data::archive::Archives,
        c: crate::hsd::cobj::CObjId,
        pos: crate::vec::Vec3,
    ) -> Option<crate::vec::Vec3> {
        use crate::hsd::cobj;
        let held = super::camera::View::of_camera(self, data);
        let other = self.display_phase.other_view(held, self.scene.unk_0)?;
        let objs = &mut self.objs;
        let kept = objs.cobjs[c].clone();
        let wobjs = [kept.eyepos, kept.interest].map(|w| w.map(|w| (w, objs.wobjs[w.0].clone())));
        cobj::set_fov(&mut objs.cobjs, Some(c), other.fov);
        cobj::set_interest(objs, c, other.interest);
        cobj::set_eye_position(objs, c, other.eye);
        let s = crate::lb::screen::world_to_screen(objs, data, c, pos, false);
        objs.cobjs[c] = kept;
        for (w, o) in wobjs.into_iter().flatten() {
            objs.wobjs[w.0] = o;
        }
        s
    }

    /// The game camera's ground span (`camera::ground_span`'s left, centre and right) through
    /// the view the display pass's other phase would leave it ([`DisplayPhase::other_view`]),
    /// where that view isn't the one it holds.
    pub fn other_phase_span(&mut self, data: &melee_data::archive::Archives) -> Option<[f32; 3]> {
        let held = super::camera::View::of_camera(self, data);
        let other = self.display_phase.other_view(held, self.scene.unk_0)?;
        Some(other.ground_span(self).0)
    }
}

/// The replay frame (`Playback::frame_index`, -123 the first) before which a live match's
/// camera is taken for off playback's course ([`Game::view_read_off_course`]). Playback's first
/// frame settles each fighter with a camera update and snaps both transforms to their targets
/// (`Camera_8002F3AC`, `docs/slippi-bugs.md`, "Playback settles each fighter on the first frame,
/// which live play doesn't"); the live match's camera eases there instead, and the two differ
/// until they meet, bit for bit, as the fighters move the targets. Measured (2026-11-18, the
/// transforms and the camera boxes' eased extents and timers, settled against unsettled
/// playback): in each of 167 of Sarah's Slippi 3.19 replays (six tournament stages) and the
/// oracle's 23 stage matches played under 3.19's rules (every other stage with a moving camera)
/// the cameras differ from the first frame to a last one, at most frame 266 (Yoshi's Island
/// 64; 241 among Sarah's), and are equal from then on to the 1,200th or 2,500th frame. The
/// window runs over twice that. The camera needs the fighters to move to meet playback's, so
/// a match whose fighters stand still the whole window can still differ after it: that read
/// isn't counted (`melee-player`'s `a_live_match_equals_its_playback` holds its match's camera
/// equal to its settled playback's from the window's end).
pub const SETTLING_WINDOW: i32 = 600;

/// Brinstar Depths' haze off playback's course, under Slippi's rules (`docs/slippi-bugs.md`,
/// "Playback's inputs never reach the controllers"): A on the second controller steps Kraid's
/// haze (`gr::kraid::proc_copy`, from the pad library's master status), which a replay's
/// playback never does, and the animations the haze then plays draw random numbers in the
/// logic: in its first process (priority 1, the animation's particle keys) and in the step
/// itself (priority 4), before the fighters' and items' processes at those priorities and
/// after. Playback puts the recorded seed back only at the frame's start, so a draw the logic
/// makes after the haze's process in such a tick can come out differently in playback.
///
/// From a step until Kraid restarts the haze (`gr::kraid::set_anim`, which starts the same
/// animation either way; the particles left behind draw at priority 15, after the logic's
/// draws), each tick in which the seed moves between the frame's start and the haze's step, or
/// between the step and the particles' priority (the recording codes' item process, the first
/// there), is counted into the playback note ([`DisplayPhase::note`]): the haze's draws may come
/// before a draw the gameplay reads. Draws before the frame's start (priority 0 ahead of
/// playback's seed process) aren't watched: Brinstar Depths makes none there.
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Haze {
    /// A step put the haze off playback's course, and Kraid hasn't restarted it since.
    pub off: bool,
    /// Kraid restarted it this tick: back on course after the tick.
    pub restarted: bool,
    /// The random seed at the frame's start, while off.
    pub start: u32,
    /// The random seed after the haze's process this tick, while off.
    pub after: Option<u32>,
    /// The game tick last counted.
    pub counted: Option<u32>,
}

impl Game {
    /// Playback's seed process, where it restores the recorded seed: the seed the frame starts
    /// with ([`Haze`]).
    pub fn haze_frame_start(&mut self) {
        if self.haze.off {
            self.haze.start = self.objs.rng.seed;
        }
    }

    /// The haze's process starts ([`Haze`]): counts the tick when the seed moved since the
    /// frame's start while the haze was off course.
    pub fn haze_process_start(&mut self) {
        if self.haze.off && self.objs.rng.seed != self.haze.start {
            self.haze_note();
        }
    }

    /// The haze's process is done, `stepped` when it stepped the haze ([`Haze`]).
    pub fn haze_process_end(&mut self, stepped: bool) {
        if stepped && self.rules.slippi {
            self.haze.off = true;
            self.haze.restarted = false;
        }
        if self.haze.off {
            self.haze.after = Some(self.objs.rng.seed);
        }
    }

    /// Kraid restarted the haze on his state's animation ([`Haze`]).
    pub fn haze_restarted(&mut self) {
        if self.haze.off {
            self.haze.restarted = true;
        }
    }

    /// The logic's processes are done for the tick, the particles' about to run ([`Haze`]):
    /// counts the tick when the seed moved since the haze's process; back on course after a
    /// restart.
    pub fn haze_logic_done(&mut self) {
        if !self.haze.off {
            return;
        }
        if self
            .haze
            .after
            .take()
            .is_some_and(|a| a != self.objs.rng.seed)
        {
            self.haze_note();
        }
        if self.haze.restarted {
            self.haze = Haze::default();
        }
    }

    /// One tick counted at most once.
    fn haze_note(&mut self) {
        let tick = self.scene.unk_8;
        if self.haze.counted != Some(tick) {
            self.haze.counted = Some(tick);
            self.display_phase.note(tick);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::gm::machine::{GM_CHALLENGER_APPROACH, GM_OPENING_MV, scene_kind};
    use crate::game::gm::one_player::{GM_CLASSIC, GM_HOME_RUN};
    use crate::game::gm::tournament::GM_TOURNAMENT;
    use crate::game::handles::CodeRef;
    use melee_data::archive::Archives;

    /// What [`rebuild_refusal`] says of mode `mode`'s first state entering scene `scene`, in a
    /// session under `ruleset`, after `f` changes the game.
    fn refusal(
        ruleset: Ruleset,
        mode: u8,
        scene: u8,
        f: impl FnOnce(&mut Game),
    ) -> Option<&'static str> {
        let state = (0..32)
            .find(|&s| scene_kind(Some((mode, s))) == Some(scene))
            .unwrap_or_else(|| panic!("mode {mode:#x} has no scene {scene:#x}"));
        let mut g = Box::<Game>::default();
        g.ruleset = ruleset;
        g.machine.phase = Phase::Entering {
            mode,
            scene_mode: mode,
            state,
            overridden: false,
        };
        f(&mut g);
        let data = Archives::default();
        rebuild_refusal(&Running {
            game: &mut g,
            data: &data,
        })
    }

    /// Design call 6's list: the VS-scene matches built from their game start, and why each
    /// other one isn't (`docs/slippi-ruleset.md`).
    #[test]
    fn the_matches_playback_rebuilds() {
        let slippi = |mode, scene| refusal(Ruleset::Slippi, mode, scene, |_| {});
        assert_eq!(slippi(machine::GM_VS, GS_VS), None);
        assert_eq!(slippi(GM_TOURNAMENT, GS_VS), None);
        for m in [
            Special::Invisible,
            Special::Tiny,
            Special::Giant,
            Special::Lightning,
            Special::SlowMo,
            Special::SingleButton,
        ] {
            assert_eq!(slippi(m.game_mode(), GS_VS), None, "{m:?}");
        }
        assert_eq!(
            slippi(Special::SuperSuddenDeath.game_mode(), GS_VS),
            Some(SUPER_SUDDEN_DEATH)
        );
        assert_eq!(slippi(Special::Stamina.game_mode(), GS_VS), Some(STAMINA));
        assert_eq!(
            slippi(Special::FixedCamera.game_mode(), GS_VS),
            Some(FIXED_CAMERA)
        );
        assert_eq!(slippi(machine::GM_VS, GS_SUDDEN_DEATH), Some(SUDDEN_DEATH));
        assert_eq!(slippi(GM_OPENING_MV, GS_VS), Some(ATTRACT));
        assert_eq!(slippi(GM_CHALLENGER_APPROACH, GS_VS), Some(CHALLENGER));
        assert_eq!(slippi(GM_CLASSIC, GS_VS), Some(SINGLE_PLAYER));
        assert_eq!(slippi(GM_HOME_RUN, GS_VS), Some(SINGLE_PLAYER));
        // A callback in VS mode's start data, which playback would clear.
        let hooked = refusal(Ruleset::Slippi, machine::GM_VS, GS_VS, |g| {
            g.vs_melee.start.rules.on_match_start = Some(CodeRef(0x8016_5290));
        });
        assert_eq!(hooked, Some(CALLBACK));
        // Vanilla's matches all run in the menus' session.
        assert!(refusal(Ruleset::Vanilla, machine::GM_VS, GS_VS, |_| {}).is_some());
    }

    fn view(x: f32) -> crate::game::camera::View {
        crate::game::camera::View {
            fov: 30.0,
            interest: crate::vec::Vec3::new(x, 0.0, 0.0),
            eye: crate::vec::Vec3::new(x, 20.0, 200.0),
        }
    }

    /// The view the other display-pass phase reads: in a tick after a pass, the view a pass
    /// gave after the tick before; on a doubled round's second tick (the CObj holding the view
    /// from before the last tick), the one a pass after the last tick would have given; none
    /// where that's the view held, in the match's first tick, or where the last tick's end
    /// wasn't kept.
    #[test]
    fn the_other_phase_sees_the_other_view() {
        let mut d = DisplayPhase::default();
        // The first tick: no tick's end is known.
        assert_eq!(d.other_view(view(1.0), 0), None);
        d.tick_end(1, Some(view(1.0)));
        assert_eq!(d.other_view(view(1.0), 1), None);
        d.tick_end(2, Some(view(2.0)));
        assert_eq!(d.views, [Some(view(2.0)), Some(view(1.0))]);
        // A tick after a pass: the other phase skipped it.
        assert_eq!(d.other_view(view(2.0), 2), Some(view(1.0)));
        // A doubled round's second tick: the other phase's pass ran.
        assert_eq!(d.other_view(view(1.0), 2), Some(view(2.0)));
        // A stale end.
        assert_eq!(d.other_view(view(2.0), 3), None);
        // The camera standing still: the same either way.
        d.tick_end(3, Some(view(2.0)));
        assert_eq!(d.other_view(view(2.0), 3), None);
        // A mode that leaves the CObj alone.
        d.tick_end(4, None);
        assert_eq!(d.other_view(view(2.0), 4), None);
        // A pause's end: the pause camera's last view held, its mode back to the match's
        // before the stage's processes run, the camera still: nothing differs.
        d.tick_end(5, Some(view(7.0)));
        d.tick_end(6, Some(view(7.0)));
        assert_eq!(d.other_view(view(7.0), 6), None);
    }

    /// The haze's tripwire on a constructed game: off course from a step under Slippi's rules,
    /// a tick counted (once) where the seed moves between the frame's start and the haze's
    /// process or between the process and the particles' priority, none where it doesn't, and
    /// back on course after the tick Kraid restarts the haze. Vanilla's steps count nothing.
    #[test]
    fn the_haze_off_course_counts_draws_around_it() {
        let mut g = Box::<Game>::default();
        g.rules.slippi = true;
        let tick = |g: &mut Game, t: u32, before: u32, step: bool, during: u32, after: u32| {
            g.scene.unk_8 = t;
            g.haze_frame_start();
            for _ in 0..before {
                g.objs.rng.draw();
            }
            g.haze_process_start();
            for _ in 0..during {
                g.objs.rng.draw();
            }
            g.haze_process_end(step);
            for _ in 0..after {
                g.objs.rng.draw();
            }
            g.haze_logic_done();
        };
        // On course, draws anywhere count nothing.
        tick(&mut g, 1, 2, false, 0, 2);
        assert_eq!(g.display_phase.differing, 0);
        // The step's own draws, with nothing after them: nothing yet.
        tick(&mut g, 2, 1, true, 3, 0);
        assert!(g.haze.off);
        assert_eq!(g.display_phase.differing, 0);
        // Off course: a draw before the haze's process, one after, both.
        tick(&mut g, 3, 1, false, 0, 0);
        assert_eq!(
            (g.display_phase.differing, g.display_phase.first),
            (1, Some(3))
        );
        tick(&mut g, 4, 0, false, 0, 1);
        assert_eq!(g.display_phase.differing, 2);
        tick(&mut g, 5, 1, false, 0, 1);
        assert_eq!(g.display_phase.differing, 3);
        // A tick with no draw around the haze.
        tick(&mut g, 6, 0, false, 0, 0);
        assert_eq!(g.display_phase.differing, 3);
        // Kraid restarts it: the tick still counts, then it's back on course.
        g.scene.unk_8 = 7;
        g.haze_frame_start();
        g.haze_restarted();
        g.objs.rng.draw();
        g.haze_process_start();
        g.haze_process_end(false);
        g.haze_logic_done();
        assert_eq!(g.display_phase.differing, 4);
        assert_eq!(g.haze, Haze::default());
        tick(&mut g, 8, 1, false, 0, 1);
        assert_eq!(g.display_phase.differing, 4);
        // Under Vanilla a step leaves the haze on course.
        let mut v = Box::<Game>::default();
        tick(&mut v, 1, 0, true, 1, 0);
        tick(&mut v, 2, 1, false, 0, 1);
        assert!(!v.haze.off);
        assert_eq!(v.display_phase.differing, 0);
    }

    /// The sounds the console's VS scene entry starts, kept with their handles, give the live
    /// match's entry the same handles in the same order, nothing outside the entry taking part
    /// ([`in_entry`]); the console's Sheik transformation sound is passed over, and any other
    /// difference stops by name.
    #[test]
    fn the_entry_takes_the_consoles_handles() {
        let entry = |starts: &[(i32, i32)]| {
            for &(id, handle) in starts {
                entry_sound_started(id, handle);
            }
        };
        let ((), kept) = record_entry_sounds(|| {
            entry(&[(1, 0x81)]);
            in_entry(|| entry(&[(SHEIK_TRANSFORM, 0x102), (7, 0x183)]));
        });
        assert_eq!(kept, [(SHEIK_TRANSFORM, 0x102), (7, 0x183)]);
        let handles = with_entry_sounds(kept.clone(), || {
            assert_eq!(entry_sound_handle(7), None, "outside the entry");
            in_entry(|| entry_sound_handle(7))
        });
        assert_eq!(handles, Some(0x183));
        assert_eq!(entry_sound_handle(7), None, "off again");
        let other = std::panic::catch_unwind(|| {
            with_entry_sounds(kept.clone(), || in_entry(|| entry_sound_handle(8)))
        });
        assert!(other.is_err(), "another sound");
        let missing = std::panic::catch_unwind(|| with_entry_sounds(kept.clone(), || {}));
        assert!(missing.is_err(), "a sound the match's entry didn't start");
        assert_eq!(entry_sound_handle(7), None, "off after a stop");
    }
}
