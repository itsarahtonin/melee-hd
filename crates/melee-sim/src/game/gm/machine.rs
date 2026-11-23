//! The game modes and their scenes (`melee/gm/gm_1A3F.c`, `gmscene.c`, `gmscdata.c`): `main`
//! ends in the modes' loop (`gm_801A4510`), which runs one mode after another
//! (`runGameMode`); a mode runs its scenes in turn (`gm_801A4014`: the scene's preparation,
//! the mode state's and the scene's entries, the scene loop, their exits), and a mode's
//! override (the memory card check's) can run a scene of another mode in between.
//!
//! The original nests these as loops around the scene loop; melee-sim runs a scene loop one
//! engine tick at a time, so the loops' places are kept as data ([`Phase`]): when a scene
//! ends, [`Running::next_scene`] runs from where its loop left off to the next scene loop's
//! start.

use super::super::code::unported_call;
use super::super::handles::CodeRef;
use super::super::state::Running;
use super::registry::{ModeState, states};
use crate::game::gm::{GameMode, GameScene};

/// `lbCardGame_DecideGameMode`, the override the boot mode leaves.
pub const DECIDE_GAME_MODE: CodeRef = CodeRef(0x8001_CE78);

/// `struct routingInfo`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Routing {
    pub curr_mode: u8,
    pub pending_mode: u8,
    pub prev_mode: u8,
    pub curr_state_id: u8,
    pub prev_state_id: u8,
    pub next_state_id: u8,
}

/// Where the modes' loops stand between engine ticks: not memory of the original's, which
/// keeps it in its stack.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub enum Phase {
    /// Before the modes' loop (`main` hasn't reached it).
    #[default]
    Boot,
    /// `gm_801A4510`'s loop is to run `routing.curr_mode` (`runGameMode`'s start).
    StartMode,
    /// `runGameMode`'s loop over the mode's scenes: its next round.
    ModeLoop { mode: u8 },
    /// A scene's preparation and its mode state's entry have run, and its own entry waits
    /// for the frontend to read the files it loads ([`Machine::files_wanted`]).
    Entering {
        mode: u8,
        scene_mode: u8,
        state: u8,
        overridden: bool,
    },
    /// A scene's loop is running: the mode the loop's round runs, the scene's mode state,
    /// and whether the round is the override's (its routing to put back).
    Scene {
        mode: u8,
        scene_mode: u8,
        state: u8,
        overridden: bool,
    },
}

/// `state_machine` (`struct stateMachine`), and where its loops stand.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Machine {
    pub routing: Routing,
    pub backup_routing: Routing,
    pub pending_mode_change: bool,
    pub get_override: Option<CodeRef>,
    pub phase: Phase,
    /// `gm_804D6720`, the running scene's info: its mode and mode state, whose enter and exit
    /// data the scene's code finds.
    pub scene_info: Option<(u8, u8)>,
    /// The scene about to enter loads a match's files ([`Running::scene_start_data`]), for the
    /// frontend to read first (a session's console): not memory of the original's.
    pub files_wanted: bool,
    /// The scene's match ran as a live match of the Slippi ruleset (`game::live`), which
    /// handed its exit back with its results: the scene's own exit doesn't run again. Not
    /// memory of the original's.
    pub live_match: bool,
}

/// Whether the mode's match scenes take VS mode's enter and exit data (`gmVsMelee_StartData`,
/// `gmVsMelee_VsExitInfo`): VS mode, the Special Melee modes, Camera Mode, debug VS, the
/// Home-Run Contest and the challenger approach mode's match (a single-player mode's
/// challenger, `gm_Mode_ChallengerApproach_States`' VS state).
pub fn uses_vs_melee(mode: u8) -> bool {
    mode == GameMode::Vs
        || mode == GameMode::CameraMode
        || mode == GameMode::ChallengerApproach
        || mode == GameMode::HomeRunContest
        || mode == GameMode::DebugVs
        || super::vs_mode::Special::from_game_mode(mode).is_some()
}

/// The game scene (`GS_*`) the machine's current scene runs, if any.
pub fn scene_kind(info: Option<(u8, u8)>) -> Option<GameScene> {
    let (mode, state) = info?;
    states(mode).get(usize::from(state)).map(|s| s.scene)
}

/// Whether a scene is a match's (`fn_8016E730`'s): the VS scene's, Sudden Death's or
/// Training's.
pub fn is_match_scene(kind: Option<GameScene>) -> bool {
    matches!(
        kind,
        Some(GameScene::Vs | GameScene::SuddenDeath | GameScene::Training)
    )
}

/// `gm_Scene_Results_OnEnter`.
const RESULTS_ON_ENTER: u32 = 0x8017_7368;

impl Running<'_> {
    /// The modes' loop's setup, `gm_801A4510` up to its loop: the machine cleared, every
    /// mode's `on_init`, and the first mode: progressive scan's prompt when the video
    /// interface can show it and B is held at power-on (or the console is set to it),
    /// else the boot. `dtv` is the VI's DTV status (`VIGetDTVStatus`) and `progressive` the
    /// SRAM's progressive scan setting (`OSGetProgressiveMode`), inputs of the console's
    /// (`session::Setup`).
    #[doc(alias = "gm_801A4510")]
    #[doc(alias = "gm_GetAllGameModes")]
    pub fn start_modes(&mut self, dtv: bool, progressive: bool) {
        self.game.machine = super::machine::Machine::default();
        self.game.gm.sram_progressive = progressive;
        self.modes_on_init();
        // `VIGetDTVStatus` reads the video interface's register.
        self.game
            .audio
            .clock_read(self.data, super::super::session::REG_READ);
        // `OSGetProgressiveMode` reads the SRAM's flags, in memory.
        let m = &mut self.game.machine;
        m.routing.curr_mode = if dtv
            && (self.game.gm.launch_buttons & crate::input::pad::BUTTON_B != 0 || progressive)
        {
            GameMode::ProgressiveScan.id()
        } else {
            GameMode::Boot.id()
        };
        m.routing.prev_mode = GameMode::NONE;
        m.phase = Phase::StartMode;
        self.game.game_mode = m.routing.curr_mode;
        self.game.gr.game_mode = m.routing.curr_mode;
    }

    /// Runs the modes' loops from where they stand to the next scene loop's start: the
    /// scene's preparation and entries (`gm_801A4014`), or first the end of the last scene
    /// ([`Running::end_scene`]) and of its mode.
    pub fn next_scene(&mut self) {
        loop {
            match self.game.machine.phase {
                Phase::Boot => panic!("the modes' loop hasn't started (gm_801A4510)"),
                Phase::Scene { .. } => self.end_scene(),
                Phase::Entering { .. } => {
                    if !self.game.machine.files_wanted {
                        self.finish_entering();
                    }
                    return;
                }
                Phase::StartMode => self.run_game_mode(),
                Phase::ModeLoop { mode } => {
                    let m = &mut self.game.machine;
                    if m.pending_mode_change {
                        // `runGameMode`'s end: the mode's `on_unload`, and `gm_801A4510`'s
                        // loop takes the next.
                        let on_unload = GameMode::of(mode).mode().on_unload;
                        if !self.game.main_lib.resetting
                            && let Some(h) = on_unload
                        {
                            h(self);
                        }
                        let m = &mut self.game.machine;
                        let next = m.routing.pending_mode;
                        if self.game.main_lib.resetting {
                            self.game.main_lib.resetting = false;
                        }
                        let m = &mut self.game.machine;
                        m.routing.prev_mode = m.routing.curr_mode;
                        m.routing.curr_mode = next;
                        m.phase = Phase::StartMode;
                        continue;
                    }
                    let over = m.get_override.map(|f| self.call_override(f));
                    match over {
                        Some(o) if o != GameMode::NONE => {
                            let m = &mut self.game.machine;
                            m.backup_routing = m.routing;
                            m.pending_mode_change = false;
                            m.routing.curr_state_id = 0;
                            m.routing.prev_state_id = 0;
                            m.routing.next_state_id = 0;
                            self.enter_scene(mode, o, true);
                        }
                        _ => self.enter_scene(mode, mode, false),
                    }
                    if !self.game.machine.files_wanted {
                        self.finish_entering();
                    }
                    return;
                }
            }
        }
    }

    /// `runGameMode`'s start: the routing's state ids cleared, whether the mode's files are
    /// expected preloaded, and its `on_load`.
    #[doc(alias = "runGameMode")]
    fn run_game_mode(&mut self) {
        // The oracle's test hook (`oracle_hooks`): does nothing in the player, which never
        // installs one.
        crate::game::oracle_hooks::reach(self.game, crate::game::oracle_hooks::Entry::RunGameMode);
        let mode = self.game.machine.routing.curr_mode;
        self.game.game_mode = mode;
        self.game.gr.game_mode = mode;
        let entry = GameMode::of(mode).mode();
        let (preloaded, on_load) = (entry.preloaded, entry.on_load);
        let m = &mut self.game.machine;
        m.pending_mode_change = false;
        m.routing.curr_state_id = 0;
        m.routing.prev_state_id = 0;
        m.routing.next_state_id = 0;
        self.dvd_mut().set_preloaded(preloaded);
        if let Some(h) = on_load {
            h(self);
        }
        self.game.machine.phase = Phase::ModeLoop { mode };
    }

    /// The override's question (`get_override`): the mode whose scene to run first, or
    /// `GM_COUNT` for none.
    fn call_override(&mut self, f: CodeRef) -> u8 {
        match f {
            DECIDE_GAME_MODE => self.card_decide_game_mode(),
            other => unported_call(&format!("the mode override {:#010x}", other.0)),
        }
    }

    /// `gm_801A4014` up to the scene loop: the mode state the routing points at (the first
    /// from its id on), its scene's preparation (`preloadState`), the state's and the scene's
    /// entries, and the scene loop's start (`gm_801A4D34`'s prologue).
    #[doc(alias = "gm_801A4014")]
    fn enter_scene(&mut self, mode: u8, scene_mode: u8, overridden: bool) {
        let list = states(scene_mode);
        let cur = self.game.machine.routing.curr_state_id;
        // `findState`: the first state whose id is the current one or above.
        let state = (cur..u8::MAX)
            .find_map(|i| list.iter().position(|s| s.id == i))
            .expect("a mode state");
        let s = &list[state];
        self.game.machine.routing.curr_state_id = s.id;
        self.preload_state(s.preload, s.scene);
        if let Some(h) = s.on_enter {
            h(self);
        }
        self.game.machine.phase = Phase::Entering {
            mode,
            scene_mode,
            state: state as u8,
            overridden,
        };
        // The VS scene loads its match's files, the Result Test's results screen its players'
        // (with no match before it): the frontend reads them first.
        self.game.machine.files_wanted = s.scene == GameScene::Vs
            || s.scene == GameScene::Training
            || (scene_mode == GameMode::Debug && s.id == 11)
            || s.scene == GameScene::TrophyFall
            || s.scene == GameScene::Congratulations
            || s.scene == GameScene::StaffRoll
            || s.scene == GameScene::EndingMovie
            || crate::game::vi::scene_handlers(s.scene).is_some()
            || super::intro::is_intro(s.scene)
            || super::intro_allstar::is_scene(s.scene);
    }

    /// The players whose files the scene about to enter loads, and whether it's a match (else
    /// only the results screen's files): the match's start data, or the Result Test's
    /// players as a start data (each one's character, kind and costume).
    pub fn scene_files_data(&self) -> (crate::game::structs::StartMeleeData, bool) {
        if let Phase::Entering {
            scene_mode, state, ..
        } = self.game.machine.phase
            && scene_mode == GameMode::Debug
            && states(GameMode::Debug.id())[usize::from(state)].id == 11
        {
            let e = &self.game.debug_mode.results1;
            let mut d = crate::game::structs::StartMeleeData::default();
            for (p, s) in d.players.iter_mut().zip(&e.match_end.player_standings) {
                p.ckind = s.ckind;
                p.slot_type = s.pkind;
                // The costume the screen's fighters take (`x3_u`'s top six bits).
                p.color = s.x3_u[0] >> 2;
            }
            return (d, false);
        }
        (self.scene_start_data().clone(), true)
    }

    /// The scene about to enter, if one is.
    pub fn entering_scene(&self) -> Option<GameScene> {
        let Phase::Entering {
            scene_mode, state, ..
        } = self.game.machine.phase
        else {
            return None;
        };
        Some(states(scene_mode)[usize::from(state)].scene)
    }

    /// The match the VS scene about to enter starts: its mode state's enter data.
    pub fn scene_start_data(&self) -> &crate::game::structs::StartMeleeData {
        let Phase::Entering { scene_mode, .. } = self.game.machine.phase else {
            unreachable!("no scene entering")
        };
        let mode = GameMode::of(scene_mode);
        match mode.mode().start {
            Some(d) => (d.get)(self.game),
            None => unported_call(&format!("the VS scene's enter data in game mode {mode:?}")),
        }
    }

    /// The rest of `gm_801A4014` up to the scene loop, once the scene's files are read: the
    /// scene's setup (`gm_801A4BD4`) and its entry.
    fn finish_entering(&mut self) {
        let Phase::Entering {
            mode,
            scene_mode,
            state,
            overridden,
        } = self.game.machine.phase
        else {
            unreachable!()
        };
        let state = usize::from(state);
        let scene = states(scene_mode)[state].scene;
        let (on_enter, on_frame) = match scene {
            GameScene::MemCard => (super::memcard::ON_ENTER, super::memcard::ON_FRAME),
            GameScene::OpeningMovie => (super::opening::ON_ENTER, super::opening::ON_FRAME),
            GameScene::Title => (super::title::ON_ENTER, super::title::ON_FRAME),
            GameScene::ProgressiveScan => {
                (super::progressive::ON_ENTER, super::progressive::ON_FRAME)
            }
            GameScene::Menu => (
                crate::game::mn::main::ON_ENTER,
                crate::game::mn::main::ON_FRAME,
            ),
            GameScene::Vs => (super::vs::SCENE_ON_ENTER, CodeRef(super::vs::ON_FRAME)),
            GameScene::SuddenDeath => (super::vs::SD_SCENE_ON_ENTER, CodeRef(super::vs::ON_FRAME)),
            GameScene::Training => (super::training::ON_ENTER, super::training::ON_FRAME),
            GameScene::CharacterSelect => (
                crate::game::mn::charsel::ON_ENTER,
                crate::game::mn::charsel::ON_FRAME,
            ),
            GameScene::StageSelect => (
                crate::game::mn::stagesel::ON_ENTER,
                crate::game::mn::stagesel::ON_FRAME,
            ),
            GameScene::HowToPlay => (super::howto::HOWTO_ON_ENTER, super::howto::HOWTO_ON_FRAME),
            GameScene::SpecialMovie => (super::howto::OMAKE_ON_ENTER, super::howto::OMAKE_ON_FRAME),
            // No frame function: the model GObj's process runs the screen.
            GameScene::Results => (CodeRef(RESULTS_ON_ENTER), CodeRef(0)),
            GameScene::CameraCardCheck => {
                (super::camera_mode::ON_ENTER, super::camera_mode::ON_FRAME)
            }
            GameScene::TrophyGallery => (
                crate::game::ty::gallery::ON_ENTER,
                crate::game::ty::gallery::ON_FRAME,
            ),
            GameScene::TrophyLottery => (
                crate::game::ty::lottery::ON_ENTER,
                crate::game::ty::lottery::ON_FRAME,
            ),
            GameScene::TrophyCollection => (
                crate::game::ty::collection::ON_ENTER,
                crate::game::ty::collection::ON_FRAME,
            ),
            // No frame function: the panel's process runs the interface.
            GameScene::Prize => (crate::game::hud::prize::ON_ENTER, CodeRef(0)),
            GameScene::ChallengerApproach => (super::approach::ON_ENTER, super::approach::ON_FRAME),
            GameScene::TournamentSetup => (
                super::tournament::SETUP_ON_ENTER,
                super::tournament::SETUP_ON_FRAME,
            ),
            GameScene::TournamentBracket => (
                super::tournament::BRACKET_ON_ENTER,
                super::tournament::BRACKET_ON_FRAME,
            ),
            GameScene::TournamentAlt => (
                super::tournament::ALT_ON_ENTER,
                super::tournament::ALT_ON_FRAME,
            ),
            // No frame function: the menus' process runs the scene.
            GameScene::DebugMenu => (crate::game::hud::debug_menu::ON_ENTER, CodeRef(0)),
            GameScene::TrophyFall => (
                super::endings::toy_fall::ON_ENTER,
                super::endings::toy_fall::ON_FRAME,
            ),
            GameScene::Congratulations => (
                super::endings::congrats::ON_ENTER,
                super::endings::congrats::ON_FRAME,
            ),
            GameScene::StaffRoll => (
                super::endings::staff_roll::ON_ENTER,
                super::endings::staff_roll::ON_FRAME,
            ),
            GameScene::EndingMovie => (
                super::endings::movie_end::ON_ENTER,
                super::endings::movie_end::ON_FRAME,
            ),
            // No frame functions: the models' processes run the screens.
            GameScene::GameOver => (super::endings::game_over::ON_ENTER, CodeRef(0)),
            GameScene::ComingSoon => (super::endings::game_over::SOON_ON_ENTER, CodeRef(0)),
            GameScene::ClassicIntro => (super::intro::EASY_ON_ENTER, super::intro::EASY_ON_FRAME),
            GameScene::AdventureIntro => {
                (super::intro::NORMAL_ON_ENTER, super::intro::NORMAL_ON_FRAME)
            }
            GameScene::AllStarIntro => (
                super::intro_allstar::ON_ENTER,
                super::intro_allstar::ON_FRAME,
            ),
            k if crate::game::vi::scene_handlers(k).is_some() => {
                crate::game::vi::scene_handlers(k).unwrap()
            }
            k => unported_call(&format!("game scene {k:?}")),
        };
        // `gm_FindGameSceneHandler`, `gm_801A4BD4`, `gm_801A4B88`.
        self.init_mode_scene();
        self.game.machine.scene_info = Some((scene_mode, state as u8));
        self.game.machine.phase = Phase::Scene {
            mode,
            scene_mode,
            state: state as u8,
            overridden,
        };
        match on_enter {
            super::memcard::ON_ENTER => self.memcard_on_enter(),
            super::opening::ON_ENTER => self.opening_on_enter(),
            super::title::ON_ENTER => self.title_on_enter(),
            super::progressive::ON_ENTER => self.prog_scan_on_enter(),
            crate::game::mn::main::ON_ENTER => self.menu_on_enter(),
            super::vs::SCENE_ON_ENTER => self.vs_scene_on_enter(scene_mode),
            super::vs::SD_SCENE_ON_ENTER => self.sudden_death_scene_on_enter(scene_mode),
            super::training::ON_ENTER => self.training_on_enter(),
            crate::game::mn::charsel::ON_ENTER => self.charsel_on_enter(),
            crate::game::mn::stagesel::ON_ENTER => self.stagesel_on_enter(),
            super::howto::HOWTO_ON_ENTER => self.howto_on_enter(),
            super::howto::OMAKE_ON_ENTER => self.omake_on_enter(),
            CodeRef(RESULTS_ON_ENTER) => self.results_on_enter(),
            super::camera_mode::ON_ENTER => self.camera_check_on_enter(),
            crate::game::ty::gallery::ON_ENTER => self.toy_on_enter(),
            crate::game::ty::lottery::ON_ENTER => self.lot_on_enter(),
            crate::game::ty::collection::ON_ENTER => self.coll_on_enter(),
            crate::game::hud::prize::ON_ENTER => self.prize_on_enter(),
            super::approach::ON_ENTER => self.approach_on_enter(),
            super::tournament::SETUP_ON_ENTER => self.tou_setup_on_enter(),
            super::tournament::BRACKET_ON_ENTER => self.tou_bracket_on_enter(),
            super::tournament::ALT_ON_ENTER => self.tou_alt_on_enter(),
            crate::game::hud::debug_menu::ON_ENTER => self.debug_menu_on_enter(),
            super::endings::toy_fall::ON_ENTER => self.toy_fall_on_enter(),
            super::endings::congrats::ON_ENTER => self.congrats_on_enter(),
            super::endings::staff_roll::ON_ENTER => self.staff_roll_on_enter(),
            super::endings::movie_end::ON_ENTER => self.movie_end_on_enter(),
            super::endings::game_over::ON_ENTER => self.game_over_on_enter(),
            super::endings::game_over::SOON_ON_ENTER => self.coming_soon_on_enter(),
            super::intro::EASY_ON_ENTER => self.intro_easy_on_enter(),
            super::intro::NORMAL_ON_ENTER => self.intro_normal_on_enter(),
            super::intro_allstar::ON_ENTER => self.intro_allstar_on_enter(),
            _ => self.vi_on_enter(scene),
        }
        self.game.gm.on_frame = (on_frame.0 != 0).then_some(on_frame);
    }

    /// `gm_801A4D34`'s prologue, once [`Running::next_scene`] has entered the scene (and the
    /// pad alarm's reads meanwhile are queued): the debug pause state and the counters
    /// cleared, the pad queue left with its newest poll, the card indicator, and the
    /// timeline's loop start.
    #[doc(alias = "lb_800198E0")]
    #[doc(alias = "lb_80019894")]
    #[doc(alias = "gm_801A4D34")]
    pub fn begin_scene_loop(&mut self) {
        self.game.start_scene_loop();
        self.card_game_init_scene();
        self.game.audio.start_scene_loop(self.data);
    }

    /// `gm_801A4BD4`, a scene's setup before its entry, with the rumble data it ends on
    /// (`lb_80014534`).
    #[doc(alias = "fn_801A4BD0")]
    #[doc(alias = "HSD_SObjLib_803A44A4")]
    #[doc(alias = "HSD_GObjSetInitDefaults")]
    #[doc(alias = "HSD_GObj_803912A8")]
    #[doc(alias = "gm_801A4BD4")]
    #[doc(alias = "gm_801A4B88")]
    #[doc(alias = "gm_GetAllGameScenes")]
    #[doc(alias = "gm_FindGameSceneHandler")]
    fn init_mode_scene(&mut self) {
        super::super::vs_match::init_scene(self);
        // `HSD_SObjLib_804D7960`'s kind registered again and the sprites' pool made anew
        // (`HSD_SObjLib_803A44A4`); `gm_804D6724` cleared.
        self.game.objs.sobjs = Default::default();
        self.game.perf_scene_setup();
        self.game.opening.perf_display = false;
        self.load_rumble_data();
    }

    /// `lbMthp_8001F800`; How to Play's and the Special Movie's scene exits, `onExitHowto` and
    /// `onExitOmake15`.
    #[doc(alias = "onExitMovie")]
    #[doc(alias = "onExitHowto")]
    #[doc(alias = "onExitOmake15")]
    pub fn mthp_release(&mut self) {
        self.game.audio.mthp_close(self.data);
    }

    /// The rest of `gm_801A4014` after the scene loop: the scene's exit and the mode
    /// state's (unless the console is resetting), the routing's next state, the save's
    /// writes finished (`lb_8001CDB4`), the card's tasks finished and the movie player let
    /// go; then, for an override's round, the mode's routing put back.
    fn end_scene(&mut self) {
        let Phase::Scene {
            mode,
            scene_mode,
            state,
            overridden,
        } = self.game.machine.phase
        else {
            unreachable!()
        };
        // `HSD_VIWaitXFBFlush`, the scene loop's end on the timeline: the last round's frame
        // is rendered unless the scene ended at once.
        let copied = self.game.scene.unk_C != 2;
        self.game.audio.end_scene_loop_after(self.data, copied);
        let resetting = self.game.main_lib.resetting;
        assert!(!resetting, "a reset (gm_801A4014's) isn't ported");
        let s = &states(scene_mode)[state as usize];
        match s.scene {
            GameScene::MemCard => self.memcard_on_exit(),
            // No exit of their own.
            // No exit of their own.
            GameScene::OpeningMovie
            | GameScene::Title
            | GameScene::Menu
            | GameScene::HowToPlay
            | GameScene::SpecialMovie => {}
            GameScene::CameraCardCheck => self.camera_check_on_exit(),
            GameScene::Vs | GameScene::SuddenDeath | GameScene::Training => {
                if !std::mem::take(&mut self.game.machine.live_match) {
                    self.vs_scene_on_exit(scene_mode, state);
                }
            }
            GameScene::CharacterSelect => self.charsel_on_exit(),
            GameScene::StageSelect => self.stagesel_on_exit(),
            GameScene::ProgressiveScan => self.prog_scan_on_exit(),
            GameScene::Results => self.results_on_exit(),
            // No exit of their own.
            GameScene::TrophyGallery | GameScene::TrophyLottery | GameScene::TrophyCollection => {}
            // `ifPrize_Scene_OnExit` does nothing; the debug menu has no exit.
            GameScene::Prize | GameScene::DebugMenu => {}
            GameScene::ChallengerApproach => self.approach_on_exit(),
            // The trophy fall, congratulations and the ending movie have no exits of their own;
            // the staff roll's (`gm_Scene_StaffRoll_OnExit`) does nothing.
            GameScene::TrophyFall | GameScene::Congratulations | GameScene::EndingMovie => {}
            GameScene::StaffRoll => self.staff_roll_on_exit(),
            GameScene::GameOver => self.game_over_on_exit(),
            GameScene::ComingSoon => self.coming_soon_on_exit(),
            // Classic's intro has no exit of its own.
            GameScene::ClassicIntro => {}
            GameScene::AdventureIntro => self.intro_normal_on_leave(),
            // All-Star's intro has no exit of its own.
            GameScene::AllStarIntro => {}
            GameScene::TournamentSetup => self.tou_setup_on_exit(),
            GameScene::TournamentBracket => self.tou_bracket_on_exit(),
            GameScene::TournamentAlt => self.tou_alt_on_exit(),
            // The cutscenes have no exit of their own.
            k if crate::game::vi::scene_handlers(k).is_some() => {}
            k => unported_call(&format!("game scene {k:?}'s exit")),
        }
        if let Some(h) = s.on_exit {
            h(self);
        }
        let m = &mut self.game.machine;
        m.routing.prev_state_id = m.routing.curr_state_id;
        if m.routing.next_state_id != 0 {
            m.routing.curr_state_id = m.routing.next_state_id - 1;
            m.routing.next_state_id = 0;
        } else {
            m.routing.curr_state_id = next_state(states(scene_mode), m.routing.curr_state_id);
        }
        self.card_game_flush();
        self.card_complete_all_tasks();
        self.mthp_release();
        let m = &mut self.game.machine;
        if overridden {
            m.routing = m.backup_routing;
        }
        m.phase = Phase::ModeLoop { mode };
    }

    /// `gm_Mode_Opening_OnLoad`: the opening mode starts at the state the boot left
    /// (`gm_801BF718`).
    #[doc(alias = "gm_801BF718")]
    #[doc(alias = "gm_Mode_Opening_OnLoad")]
    pub fn opening_mode_on_load(&mut self) {
        let id = self.game.boot_mode.opening_state;
        self.set_game_mode_state_id(id);
    }

    /// Whether the running scene's mode state is its list's last
    /// (`state[1].id == GM_GAMEMODESTATE_TERMINATE`, the results' exits' question): the
    /// state after it in its mode's list, read while the scene ends.
    pub fn next_state_terminates(&self) -> bool {
        let Phase::Scene {
            scene_mode, state, ..
        } = self.game.machine.phase
        else {
            unreachable!("no scene running")
        };
        states(scene_mode)
            .get(usize::from(state) + 1)
            .is_none_or(|d| d.id == 0xFF)
    }

    /// VS mode's results state's exit (`onExitResults`, `gmvsmode.c`): VS mode's results'
    /// exit, then the match's statistics, unless it was cancelled.
    pub fn vs_exit_results_state(&mut self) {
        let terminates = self.next_state_terminates();
        self.vs_mode_exit_results(terminates, super::results_exit::VS_STATE_CSS, 0);
        let me = self.game.vs_melee.results_enter.match_end.clone();
        if !super::results_scene::canceled(me.outcome) {
            self.game.add_match_stats(&me);
        }
    }

    /// `onExitResults0`: the debug mode's results state goes back to the debug menu's first
    /// page.
    #[doc(alias = "onExitResults0")]
    pub fn debug_mode_exit_results0(&mut self) {
        self.set_next_game_mode_state_id(0);
    }

    /// `gm_SetGameModeStateId`.
    #[doc(alias = "gm_SetGameModeStateId")]
    pub fn set_game_mode_state_id(&mut self, id: u8) {
        let r = &mut self.game.machine.routing;
        r.curr_state_id = id;
        r.prev_state_id = id;
    }

    /// `preloadState`, a scene's preparation: the preload cache's class for the scene's mode
    /// state, which makes the scene's heaps, the text heap, whether the preload heaps are
    /// made, what the cache should hold, and the libraries' forgetting of the last scene's
    /// heap (the card's work areas, the save's archives, the snapshots', the trophies').
    #[doc(alias = "preloadState")]
    fn preload_state(&mut self, class: i32, scene: GameScene) {
        self.set_preload_class(class);
        let text = match scene {
            GameScene::StaffRoll | GameScene::Results => 0xC000,
            GameScene::CharacterSelect => 0x2400,
            _ => 0x4800,
        };
        self.game.sis.init(text);
        let block = crate::hsd::heap::with(|h| h.mem_alloc(text)).unwrap_or(0);
        self.game.sis.take_block(self.data, block);
        let d = self.dvd_mut();
        if d.heaps.status(crate::lb::heap::SEQ) == crate::lb::heap::CREATED {
            d.cache.scene.is_heap_persistent[0] = true;
        }
        if d.heaps.status(crate::lb::heap::STAY) == crate::lb::heap::CREATED {
            d.cache.scene.is_heap_persistent[1] = true;
        }
        self.preload_refresh();
        // `lbCardNew_ForgetMemory`, `lbCardGame_Reset`, `lbSnap_8001E27C`, `Toy_803127D4`,
        // `tyDisplay_8031C8B8`.
        self.game.card_tasks.forget_memory();
        self.game.card_game.reset();
        self.game.snap.forget();
        self.game.gm.trophy_data = false;
        self.game.toys.forget();
    }

    /// `gm_SetGameModeOverride`.
    #[doc(alias = "gm_SetGameModeOverride")]
    pub fn set_game_mode_override(&mut self, f: Option<CodeRef>) {
        self.game.machine.get_override = f;
    }

    /// `gm_ChangeGameModeAfterCurrentScene`.
    #[doc(alias = "gm_ChangeGameModeAfterCurrentScene")]
    pub fn change_game_mode_after_scene(&mut self, mode: GameMode) {
        let m = &mut self.game.machine;
        m.routing.pending_mode = mode.id();
        m.pending_mode_change = true;
    }

    /// `gm_SetPendingGameMode`.
    #[doc(alias = "gm_SetPendingGameMode")]
    pub fn set_pending_game_mode(&mut self, mode: GameMode) {
        self.game.machine.routing.pending_mode = mode.id();
    }

    /// `gm_SetNewGameModePending`.
    #[doc(alias = "gm_SetNewGameModePending")]
    pub fn set_new_game_mode_pending(&mut self) {
        self.game.machine.pending_mode_change = true;
    }
}

/// `nextState`: the first state with an id above `cur`, else the list's first.
fn next_state(states: &[ModeState], cur: u8) -> u8 {
    states
        .iter()
        .find(|s| s.id > cur)
        .or(states.first())
        .map_or(0, |s| s.id)
}

/// `gm_GetCurrentGameMode`.
// Ports `gm_GetCurrentGameMode`; aliased once a check reaches it.
pub fn current_mode(m: &Machine) -> u8 {
    m.routing.curr_mode
}
