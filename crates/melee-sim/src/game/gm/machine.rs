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
    match mode {
        GM_BOOT => &BOOT,
        GM_MENU => &MENU,
        GM_VS => &VS,
        GM_MEMCARD => &MEMCARD,
        GM_OPENING_MV => &OPENING,
        GM_TITLE => &TITLE,
        GM_PROGRESSIVE_SCAN => &PROG_SCAN,
        m => unported_call(&format!("game mode {m:#x}'s states")),
    }
}

/// `GS_MOVIE_HOWTO`, `GS_MOVIE_OMAKE15`.
const GS_MOVIE_HOWTO: u8 = 0x1E;
const GS_MOVIE_OMAKE15: u8 = 0x1F;

/// A mode's `on_load` and `on_unload`, and whether its files are expected preloaded.
fn mode_hooks(mode: u8) -> (bool, Option<Hook>, Option<Hook>) {
    match mode {
        GM_BOOT | GM_MEMCARD | GM_TITLE | GM_MENU => (true, None, None),
        GM_OPENING_MV => (true, Some(Hook::OpeningOnLoad), None),
        GM_PROGRESSIVE_SCAN => (false, None, None),
        // `gm_Mode_Vs_OnUnload` does nothing.
        GM_VS => (false, Some(Hook::VsOnLoad), None),
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
            DECIDE_GAME_MODE => self.game.card_game.decide_game_mode(),
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
            GM_VS => &self.game.vs_melee.start,
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
            crate::game::mn::charsel::ON_ENTER => self.charsel_on_enter(),
            crate::game::mn::stagesel::ON_ENTER => self.stagesel_on_enter(),
            super::howto::HOWTO_ON_ENTER => self.howto_on_enter(),
            super::howto::OMAKE_ON_ENTER => self.omake_on_enter(),
            _ => unreachable!(),
        }
        self.game.gm.on_frame = Some(on_frame);
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
            GS_MOVIE_OPENING | GS_TITLE | GS_MENU | GS_MOVIE_HOWTO | GS_MOVIE_OMAKE15 => {}
            GS_VS => self.vs_scene_on_exit(),
            GS_CSS => self.charsel_on_exit(),
            GS_SSS => self.stagesel_on_exit(),
            GS_PROG_SCAN => self.prog_scan_on_exit(),
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
            Hook::VsExitVs => {
                unported_call("onExitVs (gmVsMelee_ExitVs, the match's records: task 9)")
            }
            Hook::VsEnterSuddenDeath => {
                unported_call("onEnterSuddenDeath (gmVsMelee_EnterSuddenDeath: task 9)")
            }
            Hook::VsExitSuddenDeath => unported_call("onExitSuddenDeath (task 9)"),
            Hook::VsEnterResults => {
                unported_call("onEnterResults (gmVsMelee_EnterResults: task 9)")
            }
            Hook::VsExitResults => unported_call("onExitResults (task 9)"),
            Hook::ApproachOnEnter | Hook::ApproachVsOnEnter | Hook::ApproachVsOnExit => {
                unported_call("the challenger approach's states (task 17)")
            }
            Hook::PrizeOnEnter | Hook::PrizeOnExit => {
                unported_call("the prize interface's states (task 17)")
            }
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
        self.game.audio.preload_refresh(self.data);
        // `lbCardNew_ForgetMemory`, `lbCardGame_Reset`, `lbSnap_8001E27C`, `Toy_803127D4`,
        // `tyDisplay_8031C8B8`.
        self.game.card_tasks.forget_memory();
        self.game.card_game.reset();
        self.game.snap.forget();
        self.game.gm.trophy_data = false;
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
