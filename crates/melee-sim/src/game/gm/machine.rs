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

/// `GameModeKind`s.
pub const GM_TITLE: u8 = 0x00;
pub const GM_MENU: u8 = 0x01;
pub const GM_VS: u8 = 0x02;
pub const GM_CAMERA_MODE: u8 = super::camera_mode::GM_CAMERA_MODE;
pub const GM_STAMINA_VS: u8 = 0x1F;
pub const GM_CHALLENGER_APPROACH: u8 = 0x14;
pub const GM_OPENING_MV: u8 = 0x18;
pub const GM_PROGRESSIVE_SCAN: u8 = 0x27;
pub const GM_BOOT: u8 = 0x28;
pub const GM_MEMCARD: u8 = 0x29;
pub const GM_COUNT: u8 = 0x2D;

/// `GameSceneKind`s.
pub const GS_TITLE: u8 = 0x00;
pub const GS_MENU: u8 = 0x01;
pub const GS_VS: u8 = 0x02;
pub const GS_SUDDEN_DEATH: u8 = 0x03;
pub const GS_RESULTS: u8 = 0x05;
pub const GS_CSS: u8 = 0x08;
pub const GS_SSS: u8 = 0x09;
pub const GS_PRIZE: u8 = 0x27;
pub const GS_APPROACH: u8 = 0x29;
pub const GS_MOVIE_OPENING: u8 = 0x1C;
pub const GS_PROG_SCAN: u8 = 0x28;
pub const GS_MEMCARD: u8 = 0x2A;
pub const GS_DEBUG_MENU: u8 = crate::game::hud::debug_menu::GS_DEBUG_MENU;

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
}

/// What a mode or mode state calls (`GameMode`'s and `GameModeState`'s handlers).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Hook {
    BootOnLoad,
    BootOnLeave,
    MemCardOnLoad,
    OpeningOnLoad,
    OpeningOnEnterMovie,
    OpeningOnExitMovie,
    TitleModeOnEnter,
    TitleModeOnExit,
    OpeningOnEnterVs,
    OpeningOnExitTitle,
    /// `onExitHowto`, `onExitOmake15`: the movie player let go.
    MovieOnExit,
    ProgScanOnEnter,
    ProgScanOnExit,
    MenuOnEnter,
    MenuOnExit,
    VsOnLoad,
    VsEnterCss,
    VsExitCss,
    VsEnterSss,
    VsExitSss,
    VsEnterVs,
    VsExitVs,
    VsEnterSuddenDeath,
    VsExitSuddenDeath,
    VsEnterResults,
    VsExitResults,
    ApproachOnEnter,
    ApproachVsOnEnter,
    ApproachVsOnExit,
    PrizeOnEnter,
    PrizeOnExit,
    /// A Special Melee mode's `OnLoad`.
    SpecialOnLoad,
    StaminaEnterCss,
    StaminaExitSss,
    StaminaEnterVs,
    StaminaExitVs,
    CameraEnterCheck,
    CameraExitCheck,
    CameraEnterCss,
    CameraExitCss,
    CameraEnterSss,
    CameraExitSss,
    CameraEnterVs,
    CameraExitVs,
    /// `onExit` (`gmtoygallery.c`), `gm_801BED14` (`gmtoycollection.c`).
    ToyToMenu,
    /// `onEnter` and `onExit` (`gmtoylottery.c`).
    LotteryOnEnter,
    LotteryOnExit,
    ChallengerOnLoad,
    ChallengerApproachOnExit,
    ChallengerPrizeOnExit,
    TouEnterSetup,
    TouEnterSss,
    TouExitSss,
    TouEnterVs,
    TouExitVs,
    TouEnterSuddenDeath,
    TouExitSuddenDeath,
    TouEnterResults,
    TouExitResults,
    /// `gmdebugmode.c`'s states' handlers.
    DebugEnterMenu0,
    DebugUnported(&'static str),
}

/// Whether `mode` is a Special Melee mode with VS mode's states (`gmsupersudden.c` and the
/// others but Stamina Mode's).
fn vs_like(mode: u8) -> bool {
    super::vs_mode::Special::from_game_mode(mode)
        .is_some_and(|m| m != super::vs_mode::Special::Stamina)
}

/// Whether the mode's match scenes take VS mode's enter and exit data (`gmVsMelee_StartData`,
/// `gmVsMelee_VsExitInfo`): VS mode, the Special Melee modes and Camera Mode.
pub fn uses_vs_melee(mode: u8) -> bool {
    mode == GM_VS
        || mode == GM_CAMERA_MODE
        || super::vs_mode::Special::from_game_mode(mode).is_some()
}

/// A mode state (`GameModeState`): its id, preload class, handlers and scene.
struct StateDesc {
    id: u8,
    preload: i32,
    on_enter: Option<Hook>,
    on_exit: Option<Hook>,
    scene: u8,
}

/// The mode states of the modes ported this far, by mode.
/// The game scene (`GS_*`) the machine's current scene runs, if any.
pub fn scene_kind(info: Option<(u8, u8)>) -> Option<u8> {
    let (mode, state) = info?;
    states(mode).get(usize::from(state)).map(|s| s.scene)
}

fn states(mode: u8) -> &'static [StateDesc] {
    const BOOT: [StateDesc; 1] = [StateDesc {
        id: 0,
        preload: 1,
        on_enter: Some(Hook::BootOnLoad),
        on_exit: Some(Hook::BootOnLeave),
        scene: GS_MEMCARD,
    }];
    const MEMCARD: [StateDesc; 1] = [StateDesc {
        id: 0,
        preload: 3,
        on_enter: Some(Hook::MemCardOnLoad),
        on_exit: None,
        scene: GS_MEMCARD,
    }];
    // `gm_Mode_Opening_States`: the movie, then the attract loop (its match, the title, a
    // second match, How to Play and the Special Movie), which is task 6's.
    const OPENING: [StateDesc; 6] = [
        StateDesc {
            id: 0,
            preload: 3,
            on_enter: Some(Hook::OpeningOnEnterMovie),
            on_exit: Some(Hook::OpeningOnExitMovie),
            scene: GS_MOVIE_OPENING,
        },
        StateDesc {
            id: 1,
            preload: 3,
            on_enter: Some(Hook::OpeningOnEnterVs),
            on_exit: None,
            scene: GS_VS,
        },
        StateDesc {
            id: 2,
            preload: 3,
            on_enter: Some(Hook::TitleModeOnEnter),
            on_exit: Some(Hook::OpeningOnExitTitle),
            scene: GS_TITLE,
        },
        StateDesc {
            id: 3,
            preload: 3,
            on_enter: Some(Hook::OpeningOnEnterVs),
            on_exit: None,
            scene: GS_VS,
        },
        StateDesc {
            id: 4,
            preload: 3,
            on_enter: None,
            on_exit: Some(Hook::MovieOnExit),
            scene: GS_MOVIE_HOWTO,
        },
        StateDesc {
            id: 5,
            preload: 3,
            on_enter: None,
            on_exit: Some(Hook::MovieOnExit),
            scene: GS_MOVIE_OMAKE15,
        },
    ];
    // `gm_Mode_Title_States`.
    const TITLE: [StateDesc; 1] = [StateDesc {
        id: 0,
        preload: 3,
        on_enter: Some(Hook::TitleModeOnEnter),
        on_exit: Some(Hook::TitleModeOnExit),
        scene: GS_TITLE,
    }];
    // `gm_Mode_ProgScan_States`.
    const PROG_SCAN: [StateDesc; 1] = [StateDesc {
        id: 0,
        preload: 2,
        on_enter: Some(Hook::ProgScanOnEnter),
        on_exit: Some(Hook::ProgScanOnExit),
        scene: GS_PROG_SCAN,
    }];
    // `gm_Mode_Menu_States`.
    const MENU: [StateDesc; 1] = [StateDesc {
        id: 0,
        preload: 2,
        on_enter: Some(Hook::MenuOnEnter),
        on_exit: Some(Hook::MenuOnExit),
        scene: GS_MENU,
    }];
    // `gm_Mode_Vs_States`.
    const VS: [StateDesc; 8] = [
        StateDesc {
            id: 0,
            preload: 3,
            on_enter: Some(Hook::VsEnterCss),
            on_exit: Some(Hook::VsExitCss),
            scene: GS_CSS,
        },
        StateDesc {
            id: 1,
            preload: 3,
            on_enter: Some(Hook::VsEnterSss),
            on_exit: Some(Hook::VsExitSss),
            scene: GS_SSS,
        },
        StateDesc {
            id: 2,
            preload: 3,
            on_enter: Some(Hook::VsEnterVs),
            on_exit: Some(Hook::VsExitVs),
            scene: GS_VS,
        },
        StateDesc {
            id: 3,
            preload: 3,
            on_enter: Some(Hook::VsEnterSuddenDeath),
            on_exit: Some(Hook::VsExitSuddenDeath),
            scene: GS_SUDDEN_DEATH,
        },
        StateDesc {
            id: 4,
            preload: 3,
            on_enter: Some(Hook::VsEnterResults),
            on_exit: Some(Hook::VsExitResults),
            scene: GS_RESULTS,
        },
        StateDesc {
            id: 0x80,
            preload: 2,
            on_enter: Some(Hook::ApproachOnEnter),
            on_exit: None,
            scene: GS_APPROACH,
        },
        StateDesc {
            id: 0x81,
            preload: 2,
            on_enter: Some(Hook::ApproachVsOnEnter),
            on_exit: Some(Hook::ApproachVsOnExit),
            scene: GS_VS,
        },
        StateDesc {
            id: 0xC0,
            preload: 2,
            on_enter: Some(Hook::PrizeOnEnter),
            on_exit: Some(Hook::PrizeOnExit),
            scene: GS_PRIZE,
        },
    ];
    // `gm_Mode_ToyGallery_States`, `gm_Mode_ToyLottery_States`, `gm_Mode_ToyCollection_States`.
    const TOY_GALLERY: [StateDesc; 1] = [StateDesc {
        id: 0,
        preload: 2,
        on_enter: None,
        on_exit: Some(Hook::ToyToMenu),
        scene: crate::game::ty::GS_TOY_GALLERY,
    }];
    const TOY_LOTTERY: [StateDesc; 1] = [StateDesc {
        id: 0,
        preload: 2,
        on_enter: Some(Hook::LotteryOnEnter),
        on_exit: Some(Hook::LotteryOnExit),
        scene: crate::game::ty::GS_TOY_LOTTERY,
    }];
    const TOY_COLLECTION: [StateDesc; 1] = [StateDesc {
        id: 0,
        preload: 2,
        on_enter: None,
        on_exit: Some(Hook::ToyToMenu),
        scene: crate::game::ty::GS_TOY_COLLECTION,
    }];
    // `gm_Mode_ChallengerApproach_States`.
    const CHALLENGER: [StateDesc; 3] = [
        StateDesc {
            id: 0,
            preload: 2,
            on_enter: Some(Hook::ApproachOnEnter),
            on_exit: None,
            scene: GS_APPROACH,
        },
        StateDesc {
            id: 1,
            preload: 2,
            on_enter: Some(Hook::ApproachVsOnEnter),
            on_exit: Some(Hook::ChallengerApproachOnExit),
            scene: GS_VS,
        },
        StateDesc {
            id: 2,
            preload: 2,
            on_enter: Some(Hook::PrizeOnEnter),
            on_exit: Some(Hook::ChallengerPrizeOnExit),
            scene: GS_PRIZE,
        },
    ];
    // `gm_Mode_StaminaVs_States`: the selects and the match.
    const STAMINA: [StateDesc; 3] = [
        StateDesc {
            id: 0,
            preload: 3,
            on_enter: Some(Hook::StaminaEnterCss),
            on_exit: Some(Hook::VsExitCss),
            scene: GS_CSS,
        },
        StateDesc {
            id: 1,
            preload: 3,
            on_enter: Some(Hook::VsEnterSss),
            on_exit: Some(Hook::StaminaExitSss),
            scene: GS_SSS,
        },
        StateDesc {
            id: 2,
            preload: 3,
            on_enter: Some(Hook::StaminaEnterVs),
            on_exit: Some(Hook::StaminaExitVs),
            scene: GS_VS,
        },
    ];
    // `gm_Mode_Camera_States`: the card check, the selects and the match.
    const CAMERA: [StateDesc; 4] = [
        StateDesc {
            id: 0,
            preload: 3,
            on_enter: Some(Hook::CameraEnterCheck),
            on_exit: Some(Hook::CameraExitCheck),
            scene: super::camera_mode::GS_CAMERA_VS,
        },
        StateDesc {
            id: 1,
            preload: 3,
            on_enter: Some(Hook::CameraEnterCss),
            on_exit: Some(Hook::CameraExitCss),
            scene: GS_CSS,
        },
        StateDesc {
            id: 2,
            preload: 3,
            on_enter: Some(Hook::CameraEnterSss),
            on_exit: Some(Hook::CameraExitSss),
            scene: GS_SSS,
        },
        StateDesc {
            id: 3,
            preload: 3,
            on_enter: Some(Hook::CameraEnterVs),
            on_exit: Some(Hook::CameraExitVs),
            scene: GS_VS,
        },
    ];
    // `gm_Mode_Tournament_States`.
    const TOURNAMENT: [StateDesc; 7] = [
        StateDesc {
            id: 0,
            preload: 3,
            on_enter: Some(Hook::TouEnterSetup),
            on_exit: None,
            scene: super::tournament::GS_TOU_SETUP,
        },
        StateDesc {
            id: 1,
            preload: 3,
            on_enter: None,
            on_exit: None,
            scene: super::tournament::GS_TOU_BRACKET,
        },
        StateDesc {
            id: 2,
            preload: 3,
            on_enter: None,
            on_exit: None,
            scene: super::tournament::GS_TOU_ALT,
        },
        StateDesc {
            id: 3,
            preload: 3,
            on_enter: Some(Hook::TouEnterSss),
            on_exit: Some(Hook::TouExitSss),
            scene: GS_SSS,
        },
        StateDesc {
            id: 4,
            preload: 3,
            on_enter: Some(Hook::TouEnterVs),
            on_exit: Some(Hook::TouExitVs),
            scene: GS_VS,
        },
        StateDesc {
            id: 5,
            preload: 3,
            on_enter: Some(Hook::TouEnterSuddenDeath),
            on_exit: Some(Hook::TouExitSuddenDeath),
            scene: GS_SUDDEN_DEATH,
        },
        StateDesc {
            id: 6,
            preload: 3,
            on_enter: Some(Hook::TouEnterResults),
            on_exit: Some(Hook::TouExitResults),
            scene: GS_RESULTS,
        },
    ];
    // `gm_Mode_Debug_States`: the debug menu's three pages, then the scenes its entries go
    // to, each back to the first page.
    const fn debug(id: u8, on_enter: Option<Hook>, on_exit: Option<Hook>, scene: u8) -> StateDesc {
        StateDesc {
            id,
            preload: 2,
            on_enter,
            on_exit,
            scene,
        }
    }
    const DEBUG: [StateDesc; 15] = [
        debug(0, Some(Hook::DebugEnterMenu0), None, GS_DEBUG_MENU),
        debug(
            1,
            Some(Hook::DebugUnported("onEnterMenu1")),
            None,
            GS_DEBUG_MENU,
        ),
        debug(
            2,
            Some(Hook::DebugUnported("onEnterMenu2")),
            None,
            GS_DEBUG_MENU,
        ),
        debug(
            3,
            Some(Hook::DebugUnported("onEnterPrize")),
            Some(Hook::DebugUnported("onExitPrize")),
            GS_PRIZE,
        ),
        debug(4, Some(Hook::DebugUnported("onEnterVs")), None, GS_VS),
        debug(
            5,
            Some(Hook::DebugUnported("onEnterResults0")),
            Some(Hook::DebugUnported("onExitResults0")),
            GS_RESULTS,
        ),
        debug(
            6,
            Some(Hook::DebugUnported("onEnterIntroEasy")),
            Some(Hook::DebugUnported("onExitIntro")),
            0x20,
        ),
        debug(
            7,
            Some(Hook::DebugUnported("onEnterIntroAllstar")),
            Some(Hook::DebugUnported("onExitIntro")),
            0x21,
        ),
        debug(8, None, Some(Hook::DebugUnported("onExitIntro")), 0x23),
        debug(
            9,
            Some(Hook::DebugUnported("onEnterGameOver")),
            Some(Hook::DebugUnported("onExitIntro")),
            0x22,
        ),
        debug(
            10,
            Some(Hook::DebugUnported("onEnterApproach")),
            Some(Hook::DebugUnported("onExitIntro")),
            GS_APPROACH,
        ),
        debug(
            11,
            Some(Hook::DebugUnported("onEnterResults1")),
            Some(Hook::DebugUnported("onExitResults1")),
            GS_RESULTS,
        ),
        debug(
            12,
            None,
            Some(Hook::DebugUnported("onExitIntro")),
            GS_MOVIE_OPENING,
        ),
        debug(
            13,
            Some(Hook::DebugUnported("onEnterMemCard")),
            Some(Hook::DebugUnported("onExitMemCard")),
            GS_MEMCARD,
        ),
        debug(14, None, None, 0x2B),
    ];
    match mode {
        super::tournament::GM_TOURNAMENT => &TOURNAMENT,
        crate::game::ty::GM_TOY_GALLERY => &TOY_GALLERY,
        crate::game::ty::GM_TOY_LOTTERY => &TOY_LOTTERY,
        crate::game::ty::GM_TOY_COLLECTION => &TOY_COLLECTION,
        GM_BOOT => &BOOT,
        GM_CHALLENGER_APPROACH => &CHALLENGER,
        GM_MENU => &MENU,
        // The Special Melee modes' states are VS mode's, each with its own handlers
        // ([`Running::mode_hook`] takes the mode).
        m if m == GM_VS || vs_like(m) => &VS,
        GM_STAMINA_VS => &STAMINA,
        GM_CAMERA_MODE => &CAMERA,
        GM_MEMCARD => &MEMCARD,
        GM_OPENING_MV => &OPENING,
        GM_TITLE => &TITLE,
        GM_PROGRESSIVE_SCAN => &PROG_SCAN,
        super::GM_DEBUG => &DEBUG,
        m => unported_call(&format!("game mode {m:#x}'s states")),
    }
}

/// `gm_Scene_Results_OnEnter`.
const RESULTS_ON_ENTER: u32 = 0x8017_7368;

/// `GS_MOVIE_HOWTO`, `GS_MOVIE_OMAKE15`.
const GS_MOVIE_HOWTO: u8 = 0x1E;
const GS_MOVIE_OMAKE15: u8 = 0x1F;

/// A mode's `on_load` and `on_unload`, and whether its files are expected preloaded.
fn mode_hooks(mode: u8) -> (bool, Option<Hook>, Option<Hook>) {
    match mode {
        GM_BOOT | GM_MEMCARD | GM_TITLE | GM_MENU => (true, None, None),
        super::tournament::GM_TOURNAMENT => (true, None, None),
        GM_OPENING_MV => (true, Some(Hook::OpeningOnLoad), None),
        GM_PROGRESSIVE_SCAN => (false, None, None),
        crate::game::ty::GM_TOY_GALLERY
        | crate::game::ty::GM_TOY_LOTTERY
        | crate::game::ty::GM_TOY_COLLECTION => (false, None, None),
        // `gm_Mode_Vs_OnUnload` does nothing.
        GM_VS => (false, Some(Hook::VsOnLoad), None),
        GM_STAMINA_VS => (true, Some(Hook::SpecialOnLoad), None),
        m if vs_like(m) => (false, Some(Hook::SpecialOnLoad), None),
        GM_CAMERA_MODE => (true, None, None),
        GM_CHALLENGER_APPROACH => (false, Some(Hook::ChallengerOnLoad), None),
        super::GM_DEBUG => (false, None, None),
        m => unported_call(&format!("game mode {m:#x}'s loading")),
    }
}

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
            GM_PROGRESSIVE_SCAN
        } else {
            GM_BOOT
        };
        m.routing.prev_mode = GM_COUNT;
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
                        let (_, _, on_unload) = mode_hooks(mode);
                        if !self.game.main_lib.resetting
                            && let Some(h) = on_unload
                        {
                            self.mode_hook(h, None);
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
                        Some(o) if o != GM_COUNT => {
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
        let mode = self.game.machine.routing.curr_mode;
        self.game.game_mode = mode;
        self.game.gr.game_mode = mode;
        let (preloaded, on_load, _) = mode_hooks(mode);
        let m = &mut self.game.machine;
        m.pending_mode_change = false;
        m.routing.curr_state_id = 0;
        m.routing.prev_state_id = 0;
        m.routing.next_state_id = 0;
        self.dvd_mut().set_preloaded(preloaded);
        if let Some(h) = on_load {
            self.mode_hook(h, None);
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
            self.mode_hook(h, Some((scene_mode, state as u8)));
        }
        self.game.machine.phase = Phase::Entering {
            mode,
            scene_mode,
            state: state as u8,
            overridden,
        };
        // The VS scene loads its match's files: the frontend reads them first.
        self.game.machine.files_wanted = s.scene == GS_VS;
    }

    /// The match the VS scene about to enter starts: its mode state's enter data.
    pub fn scene_start_data(&self) -> &crate::game::structs::StartMeleeData {
        let Phase::Entering { scene_mode, .. } = self.game.machine.phase else {
            unreachable!("no scene entering")
        };
        match scene_mode {
            GM_OPENING_MV => &self.game.attract.start,
            m if uses_vs_melee(m) => &self.game.vs_melee.start,
            super::tournament::GM_TOURNAMENT => &self.game.tou.start,
            m => unported_call(&format!("the VS scene's enter data in game mode {m:#x}")),
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
            GS_MEMCARD => (super::memcard::ON_ENTER, super::memcard::ON_FRAME),
            GS_MOVIE_OPENING => (super::opening::ON_ENTER, super::opening::ON_FRAME),
            GS_TITLE => (super::title::ON_ENTER, super::title::ON_FRAME),
            GS_PROG_SCAN => (super::progressive::ON_ENTER, super::progressive::ON_FRAME),
            GS_MENU => (
                crate::game::mn::main::ON_ENTER,
                crate::game::mn::main::ON_FRAME,
            ),
            GS_VS => (super::vs::SCENE_ON_ENTER, CodeRef(super::vs::ON_FRAME)),
            GS_SUDDEN_DEATH => (super::vs::SD_SCENE_ON_ENTER, CodeRef(super::vs::ON_FRAME)),
            GS_CSS => (
                crate::game::mn::charsel::ON_ENTER,
                crate::game::mn::charsel::ON_FRAME,
            ),
            GS_SSS => (
                crate::game::mn::stagesel::ON_ENTER,
                crate::game::mn::stagesel::ON_FRAME,
            ),
            GS_MOVIE_HOWTO => (super::howto::HOWTO_ON_ENTER, super::howto::HOWTO_ON_FRAME),
            GS_MOVIE_OMAKE15 => (super::howto::OMAKE_ON_ENTER, super::howto::OMAKE_ON_FRAME),
            // No frame function: the model GObj's process runs the screen.
            GS_RESULTS => (CodeRef(RESULTS_ON_ENTER), CodeRef(0)),
            super::camera_mode::GS_CAMERA_VS => {
                (super::camera_mode::ON_ENTER, super::camera_mode::ON_FRAME)
            }
            crate::game::ty::GS_TOY_GALLERY => (
                crate::game::ty::gallery::ON_ENTER,
                crate::game::ty::gallery::ON_FRAME,
            ),
            crate::game::ty::GS_TOY_LOTTERY => (
                crate::game::ty::lottery::ON_ENTER,
                crate::game::ty::lottery::ON_FRAME,
            ),
            crate::game::ty::GS_TOY_COLLECTION => (
                crate::game::ty::collection::ON_ENTER,
                crate::game::ty::collection::ON_FRAME,
            ),
            // No frame function: the panel's process runs the interface.
            GS_PRIZE => (crate::game::hud::prize::ON_ENTER, CodeRef(0)),
            GS_APPROACH => (super::approach::ON_ENTER, super::approach::ON_FRAME),
            super::tournament::GS_TOU_SETUP => (
                super::tournament::SETUP_ON_ENTER,
                super::tournament::SETUP_ON_FRAME,
            ),
            super::tournament::GS_TOU_BRACKET => (
                super::tournament::BRACKET_ON_ENTER,
                super::tournament::BRACKET_ON_FRAME,
            ),
            super::tournament::GS_TOU_ALT => (
                super::tournament::ALT_ON_ENTER,
                super::tournament::ALT_ON_FRAME,
            ),
            // No frame function: the menus' process runs the scene.
            GS_DEBUG_MENU => (crate::game::hud::debug_menu::ON_ENTER, CodeRef(0)),
            k => unported_call(&format!("game scene {k:#x}")),
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
            _ => unreachable!(),
        }
        self.game.gm.on_frame = (on_frame.0 != 0).then_some(on_frame);
    }

    /// `gm_801A4D34`'s prologue, once [`Running::next_scene`] has entered the scene (and the
    /// pad alarm's reads meanwhile are queued): the debug pause state and the counters
    /// cleared, the pad queue left with its newest poll, the card indicator, and the
    /// timeline's loop start.
    #[doc(alias = "gm_801A4D34")]
    pub fn begin_scene_loop(&mut self) {
        self.game.start_scene_loop();
        self.card_game_init_scene();
        self.game.audio.start_scene_loop(self.data);
    }

    /// `gm_801A4BD4`, a scene's setup before its entry, with the rumble data it ends on
    /// (`lb_80014534`).
    #[doc(alias = "gm_801A4BD4")]
    #[doc(alias = "gm_801A4B88")]
    #[doc(alias = "gm_FindGameSceneHandler")]
    fn init_mode_scene(&mut self) {
        super::super::vs_match::init_scene(self);
        // `HSD_SObjLib_804D7960`'s kind registered again and the sprites' pool made anew
        // (`HSD_SObjLib_803A44A4`); `gm_804D6724` cleared.
        self.game.objs.sobjs = Default::default();
        self.game.opening.perf_display = false;
        self.load_rumble_data();
    }

    /// `lbMthp_8001F800`.
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
            GS_MEMCARD => self.memcard_on_exit(),
            // No exit of their own.
            // No exit of their own.
            GS_MOVIE_OPENING | GS_TITLE | GS_MENU | GS_MOVIE_HOWTO | GS_MOVIE_OMAKE15 => {}
            super::camera_mode::GS_CAMERA_VS => self.camera_check_on_exit(),
            GS_VS | GS_SUDDEN_DEATH => self.vs_scene_on_exit(scene_mode, state),
            GS_CSS => self.charsel_on_exit(),
            GS_SSS => self.stagesel_on_exit(),
            GS_PROG_SCAN => self.prog_scan_on_exit(),
            GS_RESULTS => self.results_on_exit(),
            // No exit of their own.
            crate::game::ty::GS_TOY_GALLERY
            | crate::game::ty::GS_TOY_LOTTERY
            | crate::game::ty::GS_TOY_COLLECTION => {}
            // `ifPrize_Scene_OnExit` does nothing; the debug menu has no exit.
            GS_PRIZE | GS_DEBUG_MENU => {}
            GS_APPROACH => self.approach_on_exit(),
            super::tournament::GS_TOU_SETUP => self.tou_setup_on_exit(),
            super::tournament::GS_TOU_BRACKET => self.tou_bracket_on_exit(),
            super::tournament::GS_TOU_ALT => self.tou_alt_on_exit(),
            k => unported_call(&format!("game scene {k:#x}'s exit")),
        }
        if let Some(h) = s.on_exit {
            self.mode_hook(h, Some((scene_mode, state)));
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

    /// A mode's or mode state's handler.
    fn mode_hook(&mut self, h: Hook, state: Option<(u8, u8)>) {
        // A Special Melee mode's handler for one of VS mode's states, by its mode.
        let special = state.map(|s| s.0).filter(|&m| m != GM_VS);
        if let Some(m) = special {
            let done = match h {
                Hook::VsEnterCss => {
                    self.special_enter_css(m);
                    true
                }
                Hook::VsExitCss => {
                    self.special_exit_css(m);
                    true
                }
                Hook::VsEnterSss => {
                    self.special_enter_sss(m);
                    true
                }
                Hook::VsExitSss => {
                    self.special_exit_sss(m);
                    true
                }
                Hook::VsEnterVs => {
                    self.special_enter_vs(m);
                    true
                }
                Hook::VsExitVs => {
                    self.special_exit_vs(m);
                    true
                }
                Hook::VsEnterResults => {
                    self.special_enter_results(m);
                    true
                }
                Hook::VsEnterSuddenDeath => {
                    self.special_enter_sudden_death(m);
                    true
                }
                Hook::VsExitSuddenDeath => {
                    // VS mode's (`gmVsMelee_ExitSuddenDeath`).
                    self.vs_mode_exit_sudden_death();
                    true
                }
                Hook::VsExitResults => {
                    let next = state.and_then(|(m, s)| states(m).get(usize::from(s) + 1));
                    let terminates = next.is_none_or(|d| d.id == 0xFF);
                    self.special_exit_results(m, terminates);
                    true
                }
                _ => false,
            };
            if done {
                return;
            }
        }
        match h {
            Hook::BootOnLoad => self.boot_on_load(),
            Hook::BootOnLeave => self.boot_on_leave(),
            Hook::MemCardOnLoad => self.memcard_mode_on_load(),
            Hook::OpeningOnLoad => {
                // `gm_Mode_Opening_OnLoad`: the opening mode starts at the state the boot
                // left (`gm_801BF718`).
                let id = self.game.boot_mode.opening_state;
                self.set_game_mode_state_id(id);
            }
            Hook::OpeningOnEnterMovie => self.opening_on_enter_movie(),
            // `onExitMovie`.
            Hook::OpeningOnExitMovie => self.mthp_release(),
            Hook::TitleModeOnEnter => self.title_mode_on_enter(),
            Hook::TitleModeOnExit => self.title_mode_on_exit(),
            Hook::OpeningOnEnterVs => self.opening_on_enter_vs(),
            Hook::OpeningOnExitTitle => self.opening_on_exit_title(),
            Hook::MovieOnExit => self.mthp_release(),
            Hook::ProgScanOnEnter => self.prog_scan_mode_on_enter(),
            Hook::ProgScanOnExit => self.prog_scan_mode_on_exit(),
            Hook::MenuOnEnter => self.menu_mode_on_enter(),
            Hook::MenuOnExit => self.menu_mode_on_exit(),
            Hook::VsOnLoad => self.vs_mode_on_load(),
            Hook::VsEnterCss => self.vs_mode_enter_css(),
            Hook::VsExitCss => self.vs_mode_exit_css(),
            Hook::VsEnterSss => self.vs_mode_enter_sss(),
            Hook::VsExitSss => self.vs_mode_exit_sss(),
            Hook::VsEnterVs => self.vs_mode_enter_vs(),
            Hook::VsExitVs => self.vs_mode_exit_vs(),
            Hook::VsEnterSuddenDeath => self.vs_mode_enter_sudden_death(),
            Hook::VsExitSuddenDeath => self.vs_mode_exit_sudden_death(),
            Hook::VsEnterResults => self.vs_mode_enter_results(),
            Hook::VsExitResults => {
                // `state[1].id != GM_GAMEMODESTATE_TERMINATE`: the state after the results'.
                let next = state.and_then(|(m, s)| states(m).get(usize::from(s) + 1));
                let terminates = next.is_none_or(|d| d.id == 0xFF);
                self.vs_mode_exit_results(terminates, super::results_exit::VS_STATE_CSS, 0);
                // `onExitResults`'s own: the match's statistics, unless it was cancelled.
                let me = self.game.vs_melee.results_enter.match_end.clone();
                if !super::results_scene::canceled(me.outcome) {
                    self.game.add_match_stats(&me);
                }
            }
            Hook::ApproachOnEnter => self.approach_state_on_enter(),
            Hook::ApproachVsOnEnter => self.approach_vs_on_enter(),
            Hook::ApproachVsOnExit => self.approach_vs_on_exit(),
            Hook::ChallengerApproachOnExit => self.challenger_approach_vs_on_exit(),
            Hook::ToyToMenu => self.toy_mode_exit_to_menu(),
            Hook::LotteryOnEnter => self.lot_mode_enter(),
            Hook::LotteryOnExit => self.lot_mode_exit(),
            Hook::PrizeOnEnter => self.prize_state_on_enter(),
            Hook::PrizeOnExit => self.vs_prize_state_on_exit(),
            Hook::ChallengerOnLoad => self.challenger_mode_on_load(),
            Hook::ChallengerPrizeOnExit => self.prize_state_on_exit(),
            Hook::SpecialOnLoad => {
                let mode = self.game.machine.routing.curr_mode;
                self.special_on_load(mode);
            }
            Hook::StaminaEnterCss => self.stamina_enter_css(),
            Hook::StaminaExitSss => self.stamina_exit_sss(),
            Hook::StaminaEnterVs => self.stamina_enter_vs(),
            Hook::StaminaExitVs => self.stamina_exit_vs(),
            Hook::CameraEnterCheck => self.camera_enter_check(),
            Hook::CameraExitCheck => self.camera_exit_check(),
            Hook::CameraEnterCss => self.camera_enter_css(),
            Hook::CameraExitCss => self.camera_exit_css(),
            Hook::CameraEnterSss => self.camera_enter_sss(),
            Hook::CameraExitSss => self.camera_exit_sss(),
            Hook::CameraEnterVs => self.camera_enter_vs(),
            Hook::CameraExitVs => self.camera_exit_vs(),
            Hook::TouEnterSetup => self.tou_enter_setup(),
            Hook::TouEnterSss => self.tou_enter_sss(),
            Hook::TouExitSss => self.tou_exit_sss(),
            Hook::TouEnterVs => self.tou_enter_vs(),
            Hook::TouExitVs => self.tou_exit_vs(),
            Hook::TouEnterSuddenDeath => self.tou_enter_sudden_death(),
            Hook::TouExitSuddenDeath => self.tou_exit_sudden_death(),
            Hook::TouEnterResults => self.tou_enter_results(),
            Hook::TouExitResults => self.tou_exit_results(),
            Hook::DebugEnterMenu0 => self.debug_mode_enter_menu0(),
            Hook::DebugUnported(name) => unported_call(name),
        }
        let _ = state;
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
    fn preload_state(&mut self, class: i32, scene: u8) {
        self.set_preload_class(class);
        let text = match scene {
            // GS_STAFFROLL, GS_RESULTS.
            0x2B | GS_RESULTS => 0xC000,
            GS_CSS => 0x2400,
            _ => 0x4800,
        };
        self.game.sis.init(text);
        let block = crate::hsd::heap::with(|h| h.mem_alloc(text)).unwrap_or(0);
        self.game.sis.arena_block = block;
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
    pub fn change_game_mode_after_scene(&mut self, mode: u8) {
        let m = &mut self.game.machine;
        m.routing.pending_mode = mode;
        m.pending_mode_change = true;
    }

    /// `gm_SetPendingGameMode`.
    #[doc(alias = "gm_SetPendingGameMode")]
    pub fn set_pending_game_mode(&mut self, mode: u8) {
        self.game.machine.routing.pending_mode = mode;
    }

    /// `gm_SetNewGameModePending`.
    #[doc(alias = "gm_SetNewGameModePending")]
    pub fn set_new_game_mode_pending(&mut self) {
        self.game.machine.pending_mode_change = true;
    }
}

/// `nextState`: the first state with an id above `cur`, else the list's first.
fn next_state(states: &[StateDesc], cur: u8) -> u8 {
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
