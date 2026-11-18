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
pub const SLOW_MO: &str = "Slo-Mo Melee: playback's sound log cuts a slowed game's sound effects (`Playback/Core/Sounds`, docs/engine.md), so its recording would play back without them; the live match itself, which runs no sound code since 2026-11-17, would keep them (refused until Sarah decides, docs/slippi-ruleset.md)";
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
            Some(Special::SlowMo) => Some(SLOW_MO),
            // Their entries fill in only start data the match reads (`docs/oracle.md`,
            // "Special Melee"): the players' size, ratios and invisibility, the single
            // button, the game's speed.
            Some(
                Special::Invisible
                | Special::Tiny
                | Special::Giant
                | Special::Lightning
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
/// `gr::oldpupupu::proc_flights`). After a pause on those stages, every such read is counted
/// ([`DisplayPhase::paused`]): the pause leaves the camera off the course playback's takes.
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
    /// The game paused on such a stage: the camera's state has left the course a replay's
    /// playback takes, which never pauses, for the rest of the match (`docs/slippi-bugs.md`,
    /// "A paused match's recording"), and each read through its view is counted
    /// ([`Game::view_read_off_course`]).
    pub paused: bool,
    /// How many reads were counted: their outcomes differ between the two placements or
    /// phases, or they were made off playback's course.
    pub differing: u32,
    /// The game ticks of the scene (`gm_80479D58.unk_8`) at the first of them.
    pub first: Option<u32>,
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
    /// rules, on the stages whose logic reads the camera's view edges.
    pub fn keeps_display_views(&self) -> bool {
        use super::gr::{GR_KIND_OLDKONGO, GR_KIND_OLDPUPUPU, GR_KIND_ONETT};
        self.rules.slippi
            && matches!(
                self.stage_info.grkind,
                GR_KIND_ONETT | GR_KIND_OLDKONGO | GR_KIND_OLDPUPUPU
            )
    }

    /// A tick's end, its tick counts done ([`DisplayPhase::tick_end`]).
    pub fn display_phase_tick_end(&mut self) {
        if self.keeps_display_views() {
            let due = super::camera::pass_view(self);
            let tick = self.scene.unk_0;
            self.display_phase.tick_end(tick, due);
            if self.camera.cam.mode == super::camera::PAUSE {
                self.display_phase.paused = true;
            }
        }
    }

    /// Whether a read in the logic through the game camera's view is off playback's course
    /// ([`DisplayPhase::paused`]): the pause camera eases back to the match's view but leaves
    /// its state a little off the one playback's camera, which never paused, has (the
    /// interest's depth, the field of view a few ulps short, the eye and interest some
    /// hundred-thousandths of a unit off, to the match's end in `melee-player`'s
    /// `a_paused_live_match_notes_its_view_reads`), so the read's outcome can't be shown to
    /// match playback's, and it's counted ([`DisplayPhase::note`]) whatever it decides.
    pub fn view_read_off_course(&self) -> bool {
        self.display_phase.paused
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
        assert_eq!(slippi(Special::SlowMo.game_mode(), GS_VS), Some(SLOW_MO));
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
