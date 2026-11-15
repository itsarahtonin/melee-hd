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
/// may not play back exactly. Onett's cars, the other such read, aren't ported.
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct DisplayPhase {
    /// The root translation the display pass last gave each fighter it places by the camera,
    /// by fighter slot.
    pub placed: [Option<(u16, crate::vec::Vec3)>; PLACED],
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

    /// A read whose outcome differs between the placements, in game tick `tick`.
    pub fn note(&mut self, tick: u32) {
        self.differing = self.differing.saturating_add(1);
        self.first.get_or_insert(tick);
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
}
