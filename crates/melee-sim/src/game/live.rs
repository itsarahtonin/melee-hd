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
pub const SLOW_MO: &str = "playback's sound log cuts a slowed game's sound effects (`Playback/Core/Sounds`, docs/engine.md): built as playback builds it, the match would lose them";
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
/// Onett's cars read the game camera's view edges (`camera::ground_span`), which only the
/// display pass sets in the camera's CObj: a tick after a pass reads the view that pass set,
/// a doubled round's second tick the one before. On Onett the pass's views are kept
/// ([`DisplayPhase::pass_view`]), and each car decision through the edges is also taken with
/// the other phase's view ([`DisplayPhase::other_view`]), a different outcome counted the same
/// way (`gr::onett::proc_cars`).
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct DisplayPhase {
    /// The root translation the display pass last gave each fighter it places by the camera,
    /// by fighter slot.
    pub placed: [Option<(u16, crate::vec::Vec3)>; PLACED],
    /// The views the last two display passes gave the game camera, the last first (on Onett
    /// only), and the game tick of the last.
    pub views: [Option<super::camera::View>; 2],
    pub view_tick: u32,
    /// How many reads' outcomes differ between the two placements.
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

    /// A display pass in game tick `tick` gave the game camera view `v`; a second pass in the
    /// same tick (the game paused) replaces the first.
    pub fn pass_view(&mut self, tick: u32, v: super::camera::View) {
        if self.views[0].is_none() || self.view_tick != tick {
            self.views[1] = self.views[0];
        }
        self.views[0] = Some(v);
        self.view_tick = tick;
    }

    /// The camera view a read in the logic would see with the display pass's other phase, the
    /// CObj holding `held` and a pass now giving it `due`: where they're the same, a pass ran
    /// after the last tick, and the other phase is the one that skipped it (the view of the
    /// pass before); where they differ, none did (this is a doubled round's second tick), and
    /// the other phase is the one whose pass ran (`due`). None where both are the same, or no
    /// earlier pass is known.
    pub fn other_view(
        &self,
        held: super::camera::View,
        due: Option<super::camera::View>,
    ) -> Option<super::camera::View> {
        let other = if due == Some(held) {
            self.views[1]
        } else {
            due
        };
        other.filter(|&v| v != held)
    }

    /// A read whose outcome differs between the placements, in game tick `tick`.
    pub fn note(&mut self, tick: u32) {
        self.differing = self.differing.saturating_add(1);
        self.first.get_or_insert(tick);
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

    /// The view the other display-pass phase reads: after a pass, the one before it; on a
    /// doubled round's second tick (the CObj behind the transform), the one a pass would set
    /// now; none where they're the same or no earlier pass is known. A second pass in one tick
    /// (paused) replaces the first.
    #[test]
    fn the_other_phase_sees_the_other_view() {
        let mut d = DisplayPhase::default();
        d.pass_view(10, view(1.0));
        assert_eq!(d.other_view(view(1.0), Some(view(1.0))), None);
        d.pass_view(11, view(2.0));
        assert_eq!(d.views, [Some(view(2.0)), Some(view(1.0))]);
        // A tick after a pass: the CObj is what a pass would set now.
        assert_eq!(d.other_view(view(2.0), Some(view(2.0))), Some(view(1.0)));
        // A doubled round's second tick: the CObj lags the transform.
        assert_eq!(d.other_view(view(2.0), Some(view(3.0))), Some(view(3.0)));
        // A mode that leaves the CObj alone.
        assert_eq!(d.other_view(view(2.0), None), None);
        // The camera standing still.
        d.pass_view(12, view(2.0));
        assert_eq!(d.other_view(view(2.0), Some(view(2.0))), None);
        // Paused: passes with no tick between.
        d.pass_view(13, view(4.0));
        d.pass_view(13, view(5.0));
        assert_eq!(d.views, [Some(view(5.0)), Some(view(2.0))]);
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
}
