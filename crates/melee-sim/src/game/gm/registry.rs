//! The game modes' registry: the game's numbers for its modes ([`GameMode`], `GameModeKind`,
//! what the community calls major scenes) and for its scenes ([`GameScene`],
//! `GameSceneKind`, the minor scenes' kinds).
//!
//! The game numbers the two separately, so each is its own enum with the game's numbers: a
//! mode (`routing.curr_mode`, `gm_GetAllGameModes`' table) runs a list of mode states, and
//! each state names the scene it runs by its scene kind, the index of `gm_GetAllGameScenes`'
//! table (the scene's entry, frame and exit functions). One type can't hold both: VS mode is
//! mode 2 and runs the character select (scene 8), the stage select (9), the match (2),
//! Sudden Death (3) and the results (5); Training is mode 0x1C and its match scene 4; one
//! scene (the character select) serves a dozen modes. The state keeps the numbers
//! (`routing`, `Game::game_mode`), so it and the oracle are unchanged; code reads them through
//! the enums (`game.game_mode == GameMode::Classic`, `GameMode::from_id(mode)`).
//!
//! Every mode is one entry of [`MODES`], at its number: its name, whether its files are
//! expected preloaded, its hooks (`on_load`, `on_unload`), its states (each state's id,
//! preload class, entry and exit handlers and scene, `gm_Mode_*_States`) and where its match
//! scenes' start data lives. The scene machine ([`machine`](super::machine)) reads the entries;
//! a new mode's port writes its module (its handlers, methods of `Running`) and fills its
//! entry here, and nothing else lists modes' states or hooks.

use super::super::code::unported_call;
use super::super::state::{Game, Running};
use crate::game::structs::StartMeleeData;

/// A game mode (`GameModeKind`): the game's own numbers, the index of `gm_GetAllGameModes`'
/// table.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum GameMode {
    /// The title screen.
    #[doc(alias("GM_TITLE"))]
    Title = 0x00,
    /// The main menu and its submenus.
    #[doc(alias("GM_MENU"))]
    Menu = 0x01,
    /// VS mode: the selects, the match, Sudden Death and the results.
    #[doc(alias("GM_VS"))]
    Vs = 0x02,
    #[doc(alias("GM_CLASSIC"))]
    Classic = 0x03,
    #[doc(alias("GM_ADVENTURE"))]
    Adventure = 0x04,
    #[doc(alias("GM_ALLSTAR"))]
    AllStar = 0x05,
    /// The debug menu's mode, which only a debug level's title exits reach.
    #[doc(alias("GM_DEBUG"))]
    Debug = 0x06,
    /// The sound test's debug mode.
    #[doc(alias("GM_DEBUG_SOUND_TEST"))]
    DebugSoundTest = 0x07,
    /// The Hanyu selects, the debug menu's character and stage select tests.
    #[doc(alias("GM_HANYU_CSS"))]
    HanyuCss = 0x08,
    #[doc(alias("GM_HANYU_SSS"))]
    HanyuSss = 0x09,
    /// Camera Mode: the card check, the selects and a match with a camera.
    #[doc(alias("GM_CAMERA_MODE"))]
    CameraMode = 0x0A,
    /// The trophy screens: the gallery, the lottery and the collection.
    #[doc(alias("GM_TOY_GALLERY"))]
    TrophyGallery = 0x0B,
    #[doc(alias("GM_TOY_LOTTERY"))]
    TrophyLottery = 0x0C,
    #[doc(alias("GM_TOY_COLLECTION"))]
    TrophyCollection = 0x0D,
    /// The debug VS mode, which Slippi's playback codes run a replay's match in.
    #[doc(alias("GM_DEBUG_VS"))]
    DebugVs = 0x0E,
    #[doc(alias("GM_TARGET_TEST"))]
    TargetTest = 0x0F,
    /// The Special Melee modes with VS mode's states.
    #[doc(alias("GM_SUPER_SUDDEN_DEATH_VS"))]
    SuperSuddenDeath = 0x10,
    #[doc(alias("GM_INVISIBLE_VS"))]
    Invisible = 0x11,
    #[doc(alias("GM_SLOMO_VS"))]
    SlowMo = 0x12,
    #[doc(alias("GM_LIGHTNING_VS"))]
    Lightning = 0x13,
    /// A challenger approaching: the notice, the match and the prize.
    #[doc(alias("GM_CHALLENGER_APPROACH"))]
    ChallengerApproach = 0x14,
    /// The endings after a single-player run's clear (`gmgover.c`, the game's "game over"
    /// modes): the trophy fall, the staff roll, the ending movie and congratulations.
    #[doc(alias("GM_CLASSIC_GOVER"))]
    ClassicEnding = 0x15,
    #[doc(alias("GM_ADVENTURE_GOVER"))]
    AdventureEnding = 0x16,
    #[doc(alias("GM_ALLSTAR_GOVER"))]
    AllStarEnding = 0x17,
    /// The opening movie and the attract loop (its matches, the title, How to Play and the
    /// Special Movie).
    #[doc(alias("GM_OPENING_MV"))]
    Opening = 0x18,
    /// The debug menu's cutscenes, each in turn.
    #[doc(alias("GM_DEBUG_CUTSCENE"))]
    DebugCutscenes = 0x19,
    /// The debug menu's ending: the trophy fall and congratulations.
    #[doc(alias("GM_DEBUG_GOVER"))]
    DebugEnding = 0x1A,
    #[doc(alias("GM_TOURNAMENT"))]
    Tournament = 0x1B,
    #[doc(alias("GM_TRAINING"))]
    Training = 0x1C,
    #[doc(alias("GM_TINY_VS"))]
    Tiny = 0x1D,
    #[doc(alias("GM_GIANT_VS"))]
    Giant = 0x1E,
    #[doc(alias("GM_STAMINA_VS"))]
    Stamina = 0x1F,
    #[doc(alias("GM_HOME_RUN_CONTEST"))]
    #[doc(alias("GM_HOME_RUN"))]
    HomeRunContest = 0x20,
    /// Multi-Man Melee's six modes.
    #[doc(alias("GM_10MAN_VS"))]
    #[doc(alias("GM_10MAN"))]
    TenMan = 0x21,
    #[doc(alias("GM_100MAN_VS"))]
    #[doc(alias("GM_100MAN"))]
    HundredMan = 0x22,
    #[doc(alias("GM_3MIN_VS"))]
    #[doc(alias("GM_3MIN"))]
    ThreeMinute = 0x23,
    #[doc(alias("GM_15MIN_VS"))]
    #[doc(alias("GM_15MIN"))]
    FifteenMinute = 0x24,
    #[doc(alias("GM_ENDLESS_VS"))]
    #[doc(alias("GM_ENDLESS"))]
    Endless = 0x25,
    #[doc(alias("GM_CRUEL_VS"))]
    #[doc(alias("GM_CRUEL"))]
    Cruel = 0x26,
    /// The progressive scan prompt.
    #[doc(alias("GM_PROGRESSIVE_SCAN"))]
    ProgressiveScan = 0x27,
    /// The boot: the memory card check and its prompts.
    #[doc(alias("GM_BOOT"))]
    Boot = 0x28,
    /// The memory card prompt a mode's override runs.
    #[doc(alias("GM_MEMCARD"))]
    MemCard = 0x29,
    /// Fixed-Camera Mode, a Special Melee mode with VS mode's states.
    #[doc(alias("GM_CAMERA_VS"))]
    FixedCamera = 0x2A,
    /// Event Match.
    #[doc(alias("GM_EVENT"))]
    Event = 0x2B,
    #[doc(alias("GM_SINGLE_BUTTON_VS"))]
    SingleButton = 0x2C,
}

impl GameMode {
    /// How many there are (`GM_COUNT`).
    pub const COUNT: usize = 0x2D;

    /// `GM_COUNT` as the routing and the game cache hold it: no mode (the override's answer
    /// for none, the previous mode at power-on).
    #[doc(alias("GM_COUNT"))]
    pub const NONE: u8 = 0x2D;

    /// Every mode, by number.
    pub const ALL: [GameMode; Self::COUNT] = [
        GameMode::Title,
        GameMode::Menu,
        GameMode::Vs,
        GameMode::Classic,
        GameMode::Adventure,
        GameMode::AllStar,
        GameMode::Debug,
        GameMode::DebugSoundTest,
        GameMode::HanyuCss,
        GameMode::HanyuSss,
        GameMode::CameraMode,
        GameMode::TrophyGallery,
        GameMode::TrophyLottery,
        GameMode::TrophyCollection,
        GameMode::DebugVs,
        GameMode::TargetTest,
        GameMode::SuperSuddenDeath,
        GameMode::Invisible,
        GameMode::SlowMo,
        GameMode::Lightning,
        GameMode::ChallengerApproach,
        GameMode::ClassicEnding,
        GameMode::AdventureEnding,
        GameMode::AllStarEnding,
        GameMode::Opening,
        GameMode::DebugCutscenes,
        GameMode::DebugEnding,
        GameMode::Tournament,
        GameMode::Training,
        GameMode::Tiny,
        GameMode::Giant,
        GameMode::Stamina,
        GameMode::HomeRunContest,
        GameMode::TenMan,
        GameMode::HundredMan,
        GameMode::ThreeMinute,
        GameMode::FifteenMinute,
        GameMode::Endless,
        GameMode::Cruel,
        GameMode::ProgressiveScan,
        GameMode::Boot,
        GameMode::MemCard,
        GameMode::FixedCamera,
        GameMode::Event,
        GameMode::SingleButton,
    ];

    /// The mode numbered `id`, if the game has one.
    pub const fn from_id(id: u8) -> Option<GameMode> {
        if (id as usize) < Self::COUNT {
            Some(Self::ALL[id as usize])
        } else {
            None
        }
    }

    /// The mode numbered `id`, a number the game holds as a mode: stops if it isn't one.
    pub fn of(id: u8) -> GameMode {
        Self::from_id(id).unwrap_or_else(|| panic!("{id:#x} is no game mode"))
    }

    /// Its number, as the routing and the game's tables hold it.
    pub const fn id(self) -> u8 {
        self as u8
    }
}

impl PartialEq<GameMode> for u8 {
    fn eq(&self, m: &GameMode) -> bool {
        *self == m.id()
    }
}

impl PartialEq<u8> for GameMode {
    fn eq(&self, m: &u8) -> bool {
        self.id() == *m
    }
}

/// A game scene (`GameSceneKind`, a minor scene's kind): the game's own numbers, the index of
/// `gm_GetAllGameScenes`' table, which a mode state names its scene by.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum GameScene {
    #[doc(alias("GS_TITLE"))]
    Title = 0x00,
    #[doc(alias("GS_MENU"))]
    Menu = 0x01,
    /// A match.
    #[doc(alias("GS_VS"))]
    Vs = 0x02,
    #[doc(alias("GS_SUDDEN_DEATH"))]
    SuddenDeath = 0x03,
    /// Training's match.
    #[doc(alias("GS_TRAINING"))]
    Training = 0x04,
    #[doc(alias("GS_RESULTS"))]
    Results = 0x05,
    /// Skipped in the table.
    #[doc(alias("GS_0x6"))]
    Unused06 = 0x06,
    #[doc(alias("GS_DEBUG_MENU"))]
    DebugMenu = 0x07,
    #[doc(alias("GS_CSS"))]
    CharacterSelect = 0x08,
    #[doc(alias("GS_SSS"))]
    StageSelect = 0x09,
    /// Empty in the table.
    #[doc(alias("GS_UNK10"))]
    Unused0A = 0x0A,
    #[doc(alias("GS_TOY_GALLERY"))]
    TrophyGallery = 0x0B,
    #[doc(alias("GS_TOY_LOTTERY"))]
    TrophyLottery = 0x0C,
    #[doc(alias("GS_TOY_COLLECTION"))]
    TrophyCollection = 0x0D,
    /// Adventure's stage intro.
    #[doc(alias("GS_INTRO_NORMAL"))]
    AdventureIntro = 0x0E,
    /// The trophy fall after a clear.
    #[doc(alias("GS_REGEND_TOYFALL"))]
    TrophyFall = 0x0F,
    #[doc(alias("GS_REGEND_CONGRATS"))]
    Congratulations = 0x10,
    /// Adventure's cutscenes (`Vi*.dat`).
    #[doc(alias("GS_CUTSCENE_LUIGI"))]
    CutsceneLuigi = 0x11,
    #[doc(alias("GS_CUTSCENE_BRINSTAR"))]
    CutsceneBrinstar = 0x12,
    #[doc(alias("GS_CUTSCENE_EXPLOSION"))]
    CutsceneExplosion = 0x13,
    #[doc(alias("GS_CUTSCENE_3KIRBYS"))]
    CutsceneThreeKirbys = 0x14,
    #[doc(alias("GS_CUTSCENE_GIANTKIRBY"))]
    CutsceneGiantKirby = 0x15,
    #[doc(alias("GS_CUTSCENE_STARFOX"))]
    CutsceneStarFox = 0x16,
    #[doc(alias("GS_CUTSCENE_FZERO"))]
    CutsceneFZero = 0x17,
    #[doc(alias("GS_CUTSCENE_METAL"))]
    CutsceneMetal = 0x18,
    #[doc(alias("GS_CUTSCENE_BOWSERTOY"))]
    CutsceneBowserTrophy = 0x19,
    #[doc(alias("GS_CUTSCENE_GIGATRANSFORM"))]
    CutsceneGigaTransform = 0x1A,
    #[doc(alias("GS_CUTSCENE_GIGADEFEATED"))]
    CutsceneGigaDefeated = 0x1B,
    #[doc(alias("GS_MOVIE_OPENING"))]
    OpeningMovie = 0x1C,
    /// A single-player run's ending movie.
    #[doc(alias("GS_MOVIE_END"))]
    EndingMovie = 0x1D,
    #[doc(alias("GS_MOVIE_HOWTO"))]
    HowToPlay = 0x1E,
    /// The Special Movie.
    #[doc(alias("GS_MOVIE_OMAKE15"))]
    SpecialMovie = 0x1F,
    /// Classic's stage intro.
    #[doc(alias("GS_INTRO_EASY"))]
    ClassicIntro = 0x20,
    #[doc(alias("GS_INTRO_ALLSTAR"))]
    AllStarIntro = 0x21,
    #[doc(alias("GS_GAMEOVER"))]
    GameOver = 0x22,
    #[doc(alias("GS_COMING_SOON"))]
    ComingSoon = 0x23,
    /// The tournament's selections and settings.
    #[doc(alias("GS_TOU_SETUP"))]
    TournamentSetup = 0x24,
    #[doc(alias("GS_TOU_BRACKET"))]
    TournamentBracket = 0x25,
    /// The tournament's Winner Out and Loser Out screen.
    #[doc(alias("GS_TOU_ALT"))]
    TournamentAlt = 0x26,
    /// The notices of unlocks and special messages.
    #[doc(alias("GS_PRIZE_INTERFACE"))]
    #[doc(alias("GS_PRIZE"))]
    Prize = 0x27,
    #[doc(alias("GS_PROG_SCAN"))]
    ProgressiveScan = 0x28,
    /// "A new challenger approaching".
    #[doc(alias("GS_APPROACH"))]
    ChallengerApproach = 0x29,
    #[doc(alias("GS_MEMCARD"))]
    MemCard = 0x2A,
    /// The staff roll.
    #[doc(alias("GS_STAFFROLL"))]
    StaffRoll = 0x2B,
    /// Camera Mode's memory card check.
    #[doc(alias("GS_CAMERA_VS"))]
    CameraCardCheck = 0x2C,
}

impl GameScene {
    /// How many there are (`GS_COUNT`).
    pub const COUNT: usize = 0x2D;

    /// Every scene, by number.
    pub const ALL: [GameScene; Self::COUNT] = [
        GameScene::Title,
        GameScene::Menu,
        GameScene::Vs,
        GameScene::SuddenDeath,
        GameScene::Training,
        GameScene::Results,
        GameScene::Unused06,
        GameScene::DebugMenu,
        GameScene::CharacterSelect,
        GameScene::StageSelect,
        GameScene::Unused0A,
        GameScene::TrophyGallery,
        GameScene::TrophyLottery,
        GameScene::TrophyCollection,
        GameScene::AdventureIntro,
        GameScene::TrophyFall,
        GameScene::Congratulations,
        GameScene::CutsceneLuigi,
        GameScene::CutsceneBrinstar,
        GameScene::CutsceneExplosion,
        GameScene::CutsceneThreeKirbys,
        GameScene::CutsceneGiantKirby,
        GameScene::CutsceneStarFox,
        GameScene::CutsceneFZero,
        GameScene::CutsceneMetal,
        GameScene::CutsceneBowserTrophy,
        GameScene::CutsceneGigaTransform,
        GameScene::CutsceneGigaDefeated,
        GameScene::OpeningMovie,
        GameScene::EndingMovie,
        GameScene::HowToPlay,
        GameScene::SpecialMovie,
        GameScene::ClassicIntro,
        GameScene::AllStarIntro,
        GameScene::GameOver,
        GameScene::ComingSoon,
        GameScene::TournamentSetup,
        GameScene::TournamentBracket,
        GameScene::TournamentAlt,
        GameScene::Prize,
        GameScene::ProgressiveScan,
        GameScene::ChallengerApproach,
        GameScene::MemCard,
        GameScene::StaffRoll,
        GameScene::CameraCardCheck,
    ];

    /// The scene numbered `id`, if the game has one.
    pub const fn from_id(id: u8) -> Option<GameScene> {
        if (id as usize) < Self::COUNT {
            Some(Self::ALL[id as usize])
        } else {
            None
        }
    }

    /// Its number, as `gm_GetAllGameScenes`' table holds it.
    pub const fn id(self) -> u8 {
        self as u8
    }
}

impl PartialEq<GameScene> for u8 {
    fn eq(&self, s: &GameScene) -> bool {
        *self == s.id()
    }
}

/// A mode's or a mode state's handler (`GameMode`'s `on_load` and `on_unload`,
/// `GameModeState`'s `on_enter` and `on_exit`).
pub type Hook = fn(&mut Running<'_>);

/// A mode state (`GameModeState`, a minor scene): its id (the routing's, by which the mode
/// finds it), its preload class, its handlers and its scene.
#[derive(Clone, Copy)]
pub struct ModeState {
    pub id: u8,
    pub preload: i32,
    pub on_enter: Option<Hook>,
    pub on_exit: Option<Hook>,
    pub scene: GameScene,
}

/// Where a mode keeps the start data its match scenes take (the scene's enter data): the
/// match the scene starts and the files it loads, which the frontend reads first
/// ([`Running::scene_start_data`]).
#[derive(Clone, Copy)]
pub struct StartData {
    pub get: fn(&Game) -> &StartMeleeData,
    pub get_mut: fn(&mut Game) -> &mut StartMeleeData,
}

/// One game mode (`GameMode`, a major scene), by its number.
#[derive(Clone, Copy)]
pub struct Mode {
    pub kind: GameMode,
    pub name: &'static str,
    /// Whether its files are expected preloaded as it starts (`GameMode::preloaded`).
    pub preloaded: bool,
    /// The ports of its `on_load` and `on_unload` (none where the original's does nothing).
    pub on_load: Option<Hook>,
    pub on_unload: Option<Hook>,
    /// Its states, in the original's order (`gm_Mode_*_States`).
    pub states: &'static [ModeState],
    /// The start data its match scenes take, for a mode with a match scene.
    pub start: Option<StartData>,
}

impl std::fmt::Debug for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Mode")
            .field("kind", &self.kind)
            .field("name", &self.name)
            .field("preloaded", &self.preloaded)
            .finish_non_exhaustive()
    }
}

impl GameMode {
    /// Its registry entry.
    pub fn mode(self) -> &'static Mode {
        &MODES[self as usize]
    }
}

/// The mode numbered `mode`'s states; stops if the number isn't a mode's.
pub fn states(mode: u8) -> &'static [ModeState] {
    GameMode::of(mode).mode().states
}

const ATTRACT: StartData = StartData {
    get: |g| &g.attract.start,
    get_mut: |g| &mut g.attract.start,
};
/// VS mode's start data (`gmVsMelee_StartData`), which the Special Melee modes, Camera Mode,
/// debug VS, the Home-Run Contest and a challenger's match take too
/// ([`uses_vs_melee`](super::machine::uses_vs_melee)).
const VS_MELEE: StartData = StartData {
    get: |g| &g.vs_melee.start,
    get_mut: |g| &mut g.vs_melee.start,
};
const TOURNAMENT_START: StartData = StartData {
    get: |g| &g.tou.start,
    get_mut: |g| &mut g.tou.start,
};
const DEBUG_START: StartData = StartData {
    get: |g| &g.debug_mode.vs_enter,
    get_mut: |g| &mut g.debug_mode.vs_enter,
};
const EVENT_START: StartData = StartData {
    get: |g| &g.event.start,
    get_mut: |g| &mut g.event.start,
};
const TARGET_START: StartData = StartData {
    get: |g| &g.one_player.target_start,
    get_mut: |g| &mut g.one_player.target_start,
};
const TRAINING_START: StartData = StartData {
    get: |g| &g.training.enter,
    get_mut: |g| &mut g.training.enter,
};
/// Classic's, Adventure's and All-Star's.
const ONE_PLAYER_START: StartData = StartData {
    get: |g| &g.one_player.match_start,
    get_mut: |g| &mut g.one_player.match_start,
};
const MULTI_MAN_START: StartData = StartData {
    get: |g| &g.multi_man.start,
    get_mut: |g| &mut g.multi_man.start,
};

/// A Special Melee mode's states but Stamina Mode's (`gmsupersudden.c` and the others): VS
/// mode's, its selects', match's, Sudden Death's and results' handlers the mode's own
/// (`special`), the challenger's and the prize's VS mode's.
macro_rules! special_vs {
    ($m:ident) => {
        [
            ModeState {
                id: 0,
                preload: 3,
                on_enter: Some(|r| r.special_enter_css(GameMode::$m.id())),
                on_exit: Some(|r| r.special_exit_css(GameMode::$m.id())),
                scene: GameScene::CharacterSelect,
            },
            ModeState {
                id: 1,
                preload: 3,
                on_enter: Some(|r| r.special_enter_sss(GameMode::$m.id())),
                on_exit: Some(|r| r.special_exit_sss(GameMode::$m.id())),
                scene: GameScene::StageSelect,
            },
            ModeState {
                id: 2,
                preload: 3,
                on_enter: Some(|r| r.special_enter_vs(GameMode::$m.id())),
                on_exit: Some(|r| r.special_exit_vs(GameMode::$m.id())),
                scene: GameScene::Vs,
            },
            ModeState {
                id: 3,
                preload: 3,
                on_enter: Some(|r| r.special_enter_sudden_death(GameMode::$m.id())),
                on_exit: Some(|r| r.special_exit_sudden_death(GameMode::$m.id())),
                scene: GameScene::SuddenDeath,
            },
            ModeState {
                id: 4,
                preload: 3,
                on_enter: Some(|r| r.special_enter_results(GameMode::$m.id())),
                on_exit: Some(|r| {
                    let terminates = r.next_state_terminates();
                    r.special_exit_results(GameMode::$m.id(), terminates)
                }),
                scene: GameScene::Results,
            },
            VS[5],
            VS[6],
            VS[7],
        ]
    };
}
const BOOT: [ModeState; 1] = [ModeState {
    id: 0,
    preload: 1,
    on_enter: Some(|r| r.boot_on_load()),
    on_exit: Some(|r| r.boot_on_leave()),
    scene: GameScene::MemCard,
}];
const MEMCARD: [ModeState; 1] = [ModeState {
    id: 0,
    preload: 3,
    on_enter: Some(|r| r.memcard_mode_on_load()),
    on_exit: None,
    scene: GameScene::MemCard,
}];
// `gm_Mode_Opening_States`: the movie, then the attract loop (its match, the title, a
// second match, How to Play and the Special Movie), which is task 6's.
const OPENING: [ModeState; 6] = [
    ModeState {
        id: 0,
        preload: 3,
        on_enter: Some(|r| r.opening_on_enter_movie()),
        on_exit: Some(|r| r.mthp_release()),
        scene: GameScene::OpeningMovie,
    },
    ModeState {
        id: 1,
        preload: 3,
        on_enter: Some(|r| r.opening_on_enter_vs()),
        on_exit: None,
        scene: GameScene::Vs,
    },
    ModeState {
        id: 2,
        preload: 3,
        on_enter: Some(|r| r.title_mode_on_enter()),
        on_exit: Some(|r| r.opening_on_exit_title()),
        scene: GameScene::Title,
    },
    ModeState {
        id: 3,
        preload: 3,
        on_enter: Some(|r| r.opening_on_enter_vs()),
        on_exit: None,
        scene: GameScene::Vs,
    },
    ModeState {
        id: 4,
        preload: 3,
        on_enter: None,
        on_exit: Some(|r| r.mthp_release()),
        scene: GameScene::HowToPlay,
    },
    ModeState {
        id: 5,
        preload: 3,
        on_enter: None,
        on_exit: Some(|r| r.mthp_release()),
        scene: GameScene::SpecialMovie,
    },
];
// `gm_Mode_Title_States`.
const TITLE: [ModeState; 1] = [ModeState {
    id: 0,
    preload: 3,
    on_enter: Some(|r| r.title_mode_on_enter()),
    on_exit: Some(|r| r.title_mode_on_exit()),
    scene: GameScene::Title,
}];
// `gm_Mode_ProgScan_States`.
const PROG_SCAN: [ModeState; 1] = [ModeState {
    id: 0,
    preload: 2,
    on_enter: Some(|r| r.prog_scan_mode_on_enter()),
    on_exit: Some(|r| r.prog_scan_mode_on_exit()),
    scene: GameScene::ProgressiveScan,
}];
// `gm_Mode_Menu_States`.
const MENU: [ModeState; 1] = [ModeState {
    id: 0,
    preload: 2,
    on_enter: Some(|r| r.menu_mode_on_enter()),
    on_exit: Some(|r| r.menu_mode_on_exit()),
    scene: GameScene::Menu,
}];
// `gm_Mode_Vs_States`.
const VS: [ModeState; 8] = [
    ModeState {
        id: 0,
        preload: 3,
        on_enter: Some(|r| r.vs_mode_enter_css()),
        on_exit: Some(|r| r.vs_mode_exit_css()),
        scene: GameScene::CharacterSelect,
    },
    ModeState {
        id: 1,
        preload: 3,
        on_enter: Some(|r| r.vs_mode_enter_sss()),
        on_exit: Some(|r| r.vs_mode_exit_sss()),
        scene: GameScene::StageSelect,
    },
    ModeState {
        id: 2,
        preload: 3,
        on_enter: Some(|r| r.vs_mode_enter_vs()),
        on_exit: Some(|r| r.vs_mode_exit_vs()),
        scene: GameScene::Vs,
    },
    ModeState {
        id: 3,
        preload: 3,
        on_enter: Some(|r| r.vs_mode_enter_sudden_death()),
        on_exit: Some(|r| r.vs_mode_exit_sudden_death()),
        scene: GameScene::SuddenDeath,
    },
    ModeState {
        id: 4,
        preload: 3,
        on_enter: Some(|r| r.vs_mode_enter_results()),
        on_exit: Some(|r| r.vs_exit_results_state()),
        scene: GameScene::Results,
    },
    ModeState {
        id: 0x80,
        preload: 2,
        on_enter: Some(|r| r.approach_state_on_enter()),
        on_exit: None,
        scene: GameScene::ChallengerApproach,
    },
    ModeState {
        id: 0x81,
        preload: 2,
        on_enter: Some(|r| r.approach_vs_on_enter()),
        on_exit: Some(|r| r.approach_vs_on_exit()),
        scene: GameScene::Vs,
    },
    ModeState {
        id: 0xC0,
        preload: 2,
        on_enter: Some(|r| r.prize_state_on_enter()),
        on_exit: Some(|r| r.vs_prize_state_on_exit()),
        scene: GameScene::Prize,
    },
];
// `gm_Mode_ToyGallery_States`, `gm_Mode_ToyLottery_States`, `gm_Mode_ToyCollection_States`.
const TOY_GALLERY: [ModeState; 1] = [ModeState {
    id: 0,
    preload: 2,
    on_enter: None,
    on_exit: Some(|r| r.toy_mode_exit_to_menu()),
    scene: GameScene::TrophyGallery,
}];
const TOY_LOTTERY: [ModeState; 1] = [ModeState {
    id: 0,
    preload: 2,
    on_enter: Some(|r| r.lot_mode_enter()),
    on_exit: Some(|r| r.lot_mode_exit()),
    scene: GameScene::TrophyLottery,
}];
const TOY_COLLECTION: [ModeState; 1] = [ModeState {
    id: 0,
    preload: 2,
    on_enter: None,
    on_exit: Some(|r| r.toy_mode_exit_to_menu()),
    scene: GameScene::TrophyCollection,
}];
// `gm_Mode_ChallengerApproach_States`.
const CHALLENGER: [ModeState; 3] = [
    ModeState {
        id: 0,
        preload: 2,
        on_enter: Some(|r| r.approach_state_on_enter()),
        on_exit: None,
        scene: GameScene::ChallengerApproach,
    },
    ModeState {
        id: 1,
        preload: 2,
        on_enter: Some(|r| r.approach_vs_on_enter()),
        on_exit: Some(|r| r.challenger_approach_vs_on_exit()),
        scene: GameScene::Vs,
    },
    ModeState {
        id: 2,
        preload: 2,
        on_enter: Some(|r| r.prize_state_on_enter()),
        on_exit: Some(|r| r.prize_state_on_exit()),
        scene: GameScene::Prize,
    },
];
// `gm_Mode_StaminaVs_States`: the selects and the match; the character select's exit and the
// stage select's entry are VS mode's as a Special Melee mode's (`special`).
const STAMINA: [ModeState; 3] = [
    ModeState {
        id: 0,
        preload: 3,
        on_enter: Some(|r| r.stamina_enter_css()),
        on_exit: Some(|r| r.special_exit_css(GameMode::Stamina.id())),
        scene: GameScene::CharacterSelect,
    },
    ModeState {
        id: 1,
        preload: 3,
        on_enter: Some(|r| r.special_enter_sss(GameMode::Stamina.id())),
        on_exit: Some(|r| r.stamina_exit_sss()),
        scene: GameScene::StageSelect,
    },
    ModeState {
        id: 2,
        preload: 3,
        on_enter: Some(|r| r.stamina_enter_vs()),
        on_exit: Some(|r| r.stamina_exit_vs()),
        scene: GameScene::Vs,
    },
];
// `gm_Mode_Camera_States`: the card check, the selects and the match.
const CAMERA: [ModeState; 4] = [
    ModeState {
        id: 0,
        preload: 3,
        on_enter: Some(|r| r.camera_enter_check()),
        on_exit: Some(|r| r.camera_exit_check()),
        scene: GameScene::CameraCardCheck,
    },
    ModeState {
        id: 1,
        preload: 3,
        on_enter: Some(|r| r.camera_enter_css()),
        on_exit: Some(|r| r.camera_exit_css()),
        scene: GameScene::CharacterSelect,
    },
    ModeState {
        id: 2,
        preload: 3,
        on_enter: Some(|r| r.camera_enter_sss()),
        on_exit: Some(|r| r.camera_exit_sss()),
        scene: GameScene::StageSelect,
    },
    ModeState {
        id: 3,
        preload: 3,
        on_enter: Some(|r| r.camera_enter_vs()),
        on_exit: Some(|r| r.camera_exit_vs()),
        scene: GameScene::Vs,
    },
];
// `gm_Mode_Tournament_States`.
const TOURNAMENT: [ModeState; 7] = [
    ModeState {
        id: 0,
        preload: 3,
        on_enter: Some(|r| r.tou_enter_setup()),
        on_exit: None,
        scene: GameScene::TournamentSetup,
    },
    ModeState {
        id: 1,
        preload: 3,
        on_enter: None,
        on_exit: None,
        scene: GameScene::TournamentBracket,
    },
    ModeState {
        id: 2,
        preload: 3,
        on_enter: None,
        on_exit: None,
        scene: GameScene::TournamentAlt,
    },
    ModeState {
        id: 3,
        preload: 3,
        on_enter: Some(|r| r.tou_enter_sss()),
        on_exit: Some(|r| r.tou_exit_sss()),
        scene: GameScene::StageSelect,
    },
    ModeState {
        id: 4,
        preload: 3,
        on_enter: Some(|r| r.tou_enter_vs()),
        on_exit: Some(|r| r.tou_exit_vs()),
        scene: GameScene::Vs,
    },
    ModeState {
        id: 5,
        preload: 3,
        on_enter: Some(|r| r.tou_enter_sudden_death()),
        on_exit: Some(|r| r.tou_exit_sudden_death()),
        scene: GameScene::SuddenDeath,
    },
    ModeState {
        id: 6,
        preload: 3,
        on_enter: Some(|r| r.tou_enter_results()),
        on_exit: Some(|r| r.tou_exit_results()),
        scene: GameScene::Results,
    },
];
// `gm_Mode_Debug_States`: the debug menu's three pages, then the scenes its entries go
// to, each back to the first page.
const fn debug(
    id: u8,
    on_enter: Option<Hook>,
    on_exit: Option<Hook>,
    scene: GameScene,
) -> ModeState {
    ModeState {
        id,
        preload: 2,
        on_enter,
        on_exit,
        scene,
    }
}
const DEBUG: [ModeState; 15] = [
    debug(
        0,
        Some(|r| r.debug_mode_enter_menu0()),
        None,
        GameScene::DebugMenu,
    ),
    debug(
        1,
        Some(|r| r.debug_mode_enter_menu1()),
        None,
        GameScene::DebugMenu,
    ),
    debug(
        2,
        Some(|r| r.debug_mode_enter_menu2()),
        None,
        GameScene::DebugMenu,
    ),
    debug(
        3,
        Some(|r| r.debug_mode_enter_prize()),
        Some(|r| r.debug_mode_exit_prize()),
        GameScene::Prize,
    ),
    debug(4, Some(|r| r.debug_mode_enter_vs()), None, GameScene::Vs),
    debug(
        5,
        Some(|r| r.debug_mode_enter_results0()),
        Some(|r| r.debug_mode_exit_results0()),
        GameScene::Results,
    ),
    debug(
        6,
        Some(|r| r.debug_enter_intro_easy()),
        Some(|r| r.debug_mode_back_to_menu()),
        GameScene::ClassicIntro,
    ),
    debug(
        7,
        Some(|r| r.debug_enter_intro_allstar()),
        Some(|r| r.debug_mode_back_to_menu()),
        GameScene::AllStarIntro,
    ),
    debug(
        8,
        None,
        Some(|r| r.debug_mode_back_to_menu()),
        GameScene::ComingSoon,
    ),
    debug(
        9,
        Some(|r| r.debug_enter_game_over()),
        Some(|r| r.debug_mode_back_to_menu()),
        GameScene::GameOver,
    ),
    debug(
        10,
        Some(|r| r.debug_mode_enter_approach()),
        Some(|r| r.debug_mode_back_to_menu()),
        GameScene::ChallengerApproach,
    ),
    debug(
        11,
        Some(|r| r.debug_mode_enter_results1()),
        Some(|r| r.debug_mode_exit_results1()),
        GameScene::Results,
    ),
    debug(
        12,
        None,
        Some(|r| r.debug_mode_back_to_menu()),
        GameScene::OpeningMovie,
    ),
    debug(
        13,
        Some(|r| r.debug_mode_enter_memcard()),
        Some(|r| r.debug_mode_back_to_menu()),
        GameScene::MemCard,
    ),
    debug(14, None, None, GameScene::StaffRoll),
];
// `gm_Mode_DebugVs_States` (`gmvsmode.c`): the match, then VS mode's results, and the
// match again.
const DEBUG_VS: [ModeState; 2] = [
    debug(1, Some(|r| r.debug_vs_enter_vs()), None, GameScene::Vs),
    debug(
        3,
        Some(|r| r.vs_mode_enter_results()),
        None,
        GameScene::Results,
    ),
];
// `gm_Mode_DebugMenu_States`: the sound test's debug menu.
const SOUND_TEST: [ModeState; 1] = [debug(
    0,
    Some(|r| r.sound_test_mode_enter()),
    None,
    GameScene::DebugMenu,
)];
// `gm_Mode_HanyuCss_States`, `gm_Mode_HanyuSss_States`.
const HANYU_CSS: [ModeState; 1] = [debug(
    0,
    Some(|r| r.hanyu_css_enter()),
    Some(|r| r.hanyu_css_exit()),
    GameScene::CharacterSelect,
)];
const HANYU_SSS: [ModeState; 1] = [debug(
    0,
    Some(|r| r.hanyu_sss_enter()),
    None,
    GameScene::StageSelect,
)];
// `gm_Mode_DebugCutscene_States`: the eleven cutscenes in turn, then congratulations.
const DEBUG_CUTSCENE: [ModeState; 12] = [
    debug(
        0,
        Some(|r| r.debug_cutscene_enter()),
        None,
        GameScene::CutsceneLuigi,
    ),
    debug(1, None, None, GameScene::CutsceneBrinstar),
    debug(2, None, None, GameScene::CutsceneExplosion),
    debug(3, None, None, GameScene::CutsceneThreeKirbys),
    debug(4, None, None, GameScene::CutsceneGiantKirby),
    debug(5, None, None, GameScene::CutsceneStarFox),
    debug(6, None, None, GameScene::CutsceneFZero),
    debug(7, None, None, GameScene::CutsceneMetal),
    debug(8, None, None, GameScene::CutsceneBowserTrophy),
    debug(9, None, None, GameScene::CutsceneGigaTransform),
    debug(10, None, None, GameScene::CutsceneGigaDefeated),
    debug(
        11,
        None,
        Some(|r| r.debug_cutscene_exit()),
        GameScene::Congratulations,
    ),
];
// `gm_Mode_DebugGOver_States` (`gmgover.c`): the trophy fall, then congratulations.
const DEBUG_GOVER: [ModeState; 2] = [
    debug(0, None, None, GameScene::TrophyFall),
    debug(
        1,
        None,
        Some(|r| r.gover_congrats_exit()),
        GameScene::Congratulations,
    ),
];
// `gm_Mode_Event_States`: the select, then the match.
const EVENT: [ModeState; 2] = [
    ModeState {
        id: 0,
        preload: 3,
        on_enter: Some(|r| r.event_enter_css()),
        on_exit: Some(|r| r.event_exit_css()),
        scene: GameScene::CharacterSelect,
    },
    ModeState {
        id: 1,
        preload: 3,
        on_enter: Some(|r| r.event_enter_vs()),
        on_exit: Some(|r| r.event_exit_vs()),
        scene: GameScene::Vs,
    },
];
// `gm_Mode_GOver_States` (`gmgover.c`): the trophy fall, the staff roll, the ending movie
// and congratulations, the three game over modes'.
const GOVER: [ModeState; 4] = [
    debug(0, None, None, GameScene::TrophyFall),
    debug(1, None, None, GameScene::StaffRoll),
    debug(
        2,
        None,
        Some(|r| r.movie_end_exit()),
        GameScene::EndingMovie,
    ),
    debug(
        3,
        None,
        Some(|r| r.gover_congrats_exit()),
        GameScene::Congratulations,
    ),
];

const fn st(
    id: u8,
    preload: i32,
    on_enter: Option<Hook>,
    on_exit: Option<Hook>,
    scene: GameScene,
) -> ModeState {
    ModeState {
        id,
        preload,
        on_enter,
        on_exit,
        scene,
    }
}
/// A state's handler that isn't ported: the mode's own task's.
macro_rules! u {
    ($name:literal) => {
        Some(|_| unported_call($name))
    };
}
// Eleven rounds of an intro and a match, ids 8 apart.
const CLASSIC: [ModeState; 25] = {
    let mut out = [st(0, 0, None, None, GameScene::Title); 25];
    let mut r = 0;
    while r < 11 {
        let id = 8 * r as u8;
        out[2 * r] = st(
            id,
            3,
            Some(|r| r.classic_enter_intro()),
            None,
            GameScene::ClassicIntro,
        );
        out[2 * r + 1] = st(
            id + 1,
            3,
            Some(|r| r.classic_enter_vs()),
            Some(|r| r.classic_exit_vs()),
            GameScene::Vs,
        );
        r += 1;
    }
    out[22] = st(
        104,
        3,
        None,
        u!("gmClassic_801B3F18"),
        GameScene::ComingSoon,
    );
    out[23] = st(
        105,
        3,
        Some(|r| r.classic_enter_game_over()),
        Some(|r| r.classic_exit_game_over()),
        GameScene::GameOver,
    );
    out[24] = st(
        112,
        3,
        Some(|r| r.classic_enter_css()),
        Some(|r| r.classic_exit_css()),
        GameScene::CharacterSelect,
    );
    out
};
// `gm_Mode_Adventure_States`, by their ids: the stages' intros, matches and cutscenes,
// coming soon, the game over and the select. Every handler but the select's is the mode's
// own; the unported ones stop by name.
const ADVENTURE: [ModeState; 48] = [
    // ADVENTURE_INTRO
    st(
        0x00,
        2,
        Some(|r| r.adventure_enter_intro()),
        None,
        GameScene::AdventureIntro,
    ),
    // ADVENTURE_MUSHROOM_KINGDOM
    st(
        0x01,
        2,
        Some(|r| r.adventure_enter_vs()),
        Some(|r| r.adventure_exit_vs()),
        GameScene::Vs,
    ),
    // ADVENTURE_LUIGI_CUTSCENE
    st(
        0x02,
        2,
        Some(|r| r.adventure_enter_luigi()),
        None,
        GameScene::CutsceneLuigi,
    ),
    // ADVENTURE_MARIO_PEACH_FIGHT
    st(
        0x03,
        2,
        Some(|r| r.adventure_enter_peach()),
        Some(|r| r.adventure_exit_peach()),
        GameScene::Vs,
    ),
    // ADVENTURE_KONGO_INTRO
    st(
        0x08,
        2,
        Some(|r| r.adventure_enter_intro()),
        None,
        GameScene::AdventureIntro,
    ),
    // ADVENTURE_TINYDK_FIGHT
    st(
        0x09,
        2,
        Some(|r| r.adventure_enter_tiny()),
        Some(|r| r.adventure_exit_stage()),
        GameScene::Vs,
    ),
    // ADVENTURE_GIANTDK_FIGHT
    st(
        0x0a,
        2,
        Some(|r| r.adventure_enter_vs()),
        Some(|r| r.adventure_exit_stage()),
        GameScene::Vs,
    ),
    // ADVENTURE_MAZE_INTRO
    st(
        0x10,
        2,
        Some(|r| r.adventure_enter_intro()),
        None,
        GameScene::AdventureIntro,
    ),
    // ADVENTURE_UNDERGROUND_MAZE
    st(
        0x11,
        2,
        Some(|r| r.adventure_enter_vs()),
        Some(|r| r.adventure_exit_stage()),
        GameScene::Vs,
    ),
    // ADVENTURE_ZELDA_FIGHT
    st(
        0x12,
        2,
        Some(|r| r.adventure_enter_vs()),
        Some(|r| r.adventure_exit_stage()),
        GameScene::Vs,
    ),
    // ADVENTURE_BRINSTAR_INTRO
    st(
        0x18,
        2,
        Some(|r| r.adventure_enter_intro()),
        None,
        GameScene::AdventureIntro,
    ),
    // ADVENTURE_SAMUS_FIGHT
    st(
        0x19,
        2,
        Some(|r| r.adventure_enter_vs()),
        Some(|r| r.adventure_exit_stage()),
        GameScene::Vs,
    ),
    // ADVENTURE_BRINSTAR_CUTSCENE
    st(
        0x1a,
        2,
        Some(|r| r.adventure_enter_cutscene()),
        None,
        GameScene::CutsceneBrinstar,
    ),
    // ADVENTURE_ESCAPE_FROM_BRINSTAR
    st(
        0x1b,
        2,
        Some(|r| r.adventure_enter_escape()),
        Some(|r| r.adventure_exit_escape()),
        GameScene::Vs,
    ),
    // ADVENTURE_EXPLOSION_CUTSCENE
    st(0x1c, 2, None, None, GameScene::CutsceneExplosion),
    // ADVENTURE_KIRBY_INTRO
    st(
        0x20,
        2,
        Some(|r| r.adventure_enter_intro()),
        None,
        GameScene::AdventureIntro,
    ),
    // ADVENTURE_KIRBY_FIGHT
    st(
        0x21,
        2,
        Some(|r| r.adventure_enter_vs()),
        Some(|r| r.adventure_exit_stage()),
        GameScene::Vs,
    ),
    // ADVENTURE_TEAMKIRBY_INTRO
    st(
        0x22,
        2,
        Some(|r| r.adventure_enter_kirbys()),
        None,
        GameScene::CutsceneThreeKirbys,
    ),
    // ADVENTURE_TEAMKIRBY_FIGHT
    st(
        0x23,
        2,
        Some(|r| r.adventure_enter_team_kirby()),
        Some(|r| r.adventure_exit_team_kirby()),
        GameScene::Vs,
    ),
    // ADVENTURE_GIANTKIRBY_INTRO
    st(
        0x24,
        2,
        Some(|r| r.adventure_enter_cutscene()),
        None,
        GameScene::CutsceneGiantKirby,
    ),
    // ADVENTURE_GIANTKIRBY_FIGHT
    st(
        0x25,
        2,
        Some(|r| r.adventure_enter_vs()),
        Some(|r| r.adventure_exit_stage()),
        GameScene::Vs,
    ),
    // ADVENTURE_CORNERIA_INTRO
    st(
        0x28,
        2,
        Some(|r| r.adventure_enter_intro()),
        None,
        GameScene::AdventureIntro,
    ),
    // ADVENTURE_FOX_FIGHT
    st(
        0x29,
        2,
        Some(|r| r.adventure_enter_vs()),
        Some(|r| r.adventure_exit_stage()),
        GameScene::Vs,
    ),
    // ADVENTURE_STARFOX_CUTSCENE
    st(
        0x2a,
        2,
        Some(|r| r.adventure_enter_star_fox()),
        None,
        GameScene::CutsceneStarFox,
    ),
    // ADVENTURE_FOX_FALCO_FIGHT
    st(
        0x2b,
        2,
        Some(|r| r.adventure_enter_fox_falco()),
        Some(|r| r.adventure_exit_stage()),
        GameScene::Vs,
    ),
    // ADVENTURE_POKEMON_INTRO
    st(
        0x30,
        2,
        Some(|r| r.adventure_enter_intro()),
        None,
        GameScene::AdventureIntro,
    ),
    // ADVENTURE_POKEMON_FIGHT
    st(
        0x31,
        2,
        Some(|r| r.adventure_enter_vs()),
        Some(|r| r.adventure_exit_stage()),
        GameScene::Vs,
    ),
    // ADVENTURE_FZERO_CUTSCENE
    st(0x38, 2, None, None, GameScene::CutsceneFZero),
    // ADVENTURE_FZERO_INTRO
    st(
        0x39,
        2,
        Some(|r| r.adventure_enter_intro()),
        None,
        GameScene::AdventureIntro,
    ),
    // ADVENTURE_FZERO_GRAND_PRIX
    st(
        0x3a,
        2,
        Some(|r| r.adventure_enter_vs()),
        Some(|r| r.adventure_exit_stage()),
        GameScene::Vs,
    ),
    // ADVENTURE_CAPTAIN_FALCON_FIGHT
    st(
        0x3b,
        2,
        Some(|r| r.adventure_enter_vs()),
        Some(|r| r.adventure_exit_stage()),
        GameScene::Vs,
    ),
    // ADVENTURE_ONETT_INTRO
    st(
        0x40,
        2,
        Some(|r| r.adventure_enter_intro()),
        None,
        GameScene::AdventureIntro,
    ),
    // ADVENTURE_NESS_FIGHT
    st(
        0x41,
        2,
        Some(|r| r.adventure_enter_ness()),
        Some(|r| r.adventure_exit_stage()),
        GameScene::Vs,
    ),
    // ADVENTURE_ICICLE_INTRO
    st(
        0x48,
        2,
        Some(|r| r.adventure_enter_intro()),
        None,
        GameScene::AdventureIntro,
    ),
    // ADVENTURE_ICICLE_MOUNTAIN
    st(
        0x49,
        2,
        Some(|r| r.adventure_enter_vs()),
        Some(|r| r.adventure_exit_stage()),
        GameScene::Vs,
    ),
    // ADVENTURE_BATTLEFIELD_INTRO
    st(
        0x50,
        2,
        Some(|r| r.adventure_enter_intro()),
        None,
        GameScene::AdventureIntro,
    ),
    // ADVENTURE_WIREFRAME_FIGHT
    st(
        0x51,
        2,
        Some(|r| r.adventure_enter_vs()),
        Some(|r| r.adventure_exit_stage()),
        GameScene::Vs,
    ),
    // ADVENTURE_METAL_CUTSCENE
    st(
        0x52,
        2,
        Some(|r| r.adventure_enter_cutscene()),
        None,
        GameScene::CutsceneMetal,
    ),
    // ADVENTURE_METAL_MARIO_FIGHT
    st(
        0x53,
        2,
        Some(|r| r.adventure_enter_metal()),
        Some(|r| r.adventure_exit_stage()),
        GameScene::Vs,
    ),
    // ADVENTURE_FINAL_DESTINATION_INTRO
    st(
        0x58,
        2,
        Some(|r| r.adventure_enter_intro()),
        None,
        GameScene::AdventureIntro,
    ),
    // ADVENTURE_BOWSER_FIGHT
    st(
        0x59,
        2,
        Some(|r| r.adventure_enter_bowser()),
        Some(|r| r.adventure_exit_bowser()),
        GameScene::Vs,
    ),
    // ADVENTURE_BOWSERTOY_CUTSCENE
    st(
        0x5a,
        2,
        Some(|r| r.adventure_enter_cutscene()),
        Some(|r| r.adventure_exit_toy()),
        GameScene::CutsceneBowserTrophy,
    ),
    // ADVENTURE_GIGATRANSFORM_CUTSCENE
    st(
        0x5b,
        2,
        Some(|r| r.adventure_enter_cutscene()),
        None,
        GameScene::CutsceneGigaTransform,
    ),
    // ADVENTURE_GIGABOWSER_FIGHT
    st(
        0x5c,
        2,
        Some(|r| r.adventure_enter_vs()),
        Some(|r| r.adventure_exit_giga()),
        GameScene::Vs,
    ),
    // ADVENTURE_GIGADEFEATED_CUTSCENE
    st(
        0x5d,
        2,
        Some(|r| r.adventure_enter_cutscene()),
        Some(|r| r.adventure_exit_giga_defeated()),
        GameScene::CutsceneGigaDefeated,
    ),
    // ADVENTURE_COMING_SOON
    st(0x68, 2, None, u!("gm_801B4408"), GameScene::ComingSoon),
    // ADVENTURE_GAME_OVER
    st(
        0x69,
        2,
        Some(|r| r.adventure_enter_game_over()),
        Some(|r| r.adventure_exit_game_over()),
        GameScene::GameOver,
    ),
    // ADVENTURE_BACK_TO_CSS
    st(
        0x70,
        2,
        Some(|r| r.adventure_enter_css()),
        Some(|r| r.adventure_exit_css()),
        GameScene::CharacterSelect,
    ),
];
// Thirteen rounds of a match and (but the last) the rest area, ids 8 apart.
const ALLSTAR: [ModeState; 28] = {
    let mut out = [st(0, 0, None, None, GameScene::Title); 28];
    let mut r = 0;
    while r < 13 {
        let id = 8 * r as u8;
        out[2 * r] = st(
            id,
            3,
            Some(|r| r.allstar_enter_vs()),
            Some(|r| r.allstar_exit_vs()),
            GameScene::Vs,
        );
        if r < 12 {
            out[2 * r + 1] = st(
                id + 1,
                3,
                Some(|r| r.allstar_enter_rest()),
                Some(|r| r.allstar_exit_rest()),
                GameScene::Vs,
            );
        }
        r += 1;
    }
    out[25] = st(0x68, 3, None, u!("gm_801B607C"), GameScene::ComingSoon);
    out[26] = st(
        0x69,
        3,
        Some(|r| r.allstar_enter_game_over()),
        Some(|r| r.allstar_exit_game_over()),
        GameScene::GameOver,
    );
    out[27] = st(
        0x70,
        3,
        Some(|r| r.allstar_enter_css()),
        Some(|r| r.allstar_exit_css()),
        GameScene::CharacterSelect,
    );
    out
};
const TARGET: [ModeState; 2] = [
    st(
        0,
        3,
        Some(|r| r.target_enter_css()),
        Some(|r| r.target_exit_css()),
        GameScene::CharacterSelect,
    ),
    st(
        1,
        3,
        Some(|r| r.target_enter_match()),
        Some(|r| r.target_exit_match()),
        GameScene::Vs,
    ),
];
const TRAINING: [ModeState; 3] = [
    st(
        0,
        3,
        Some(|r| r.training_enter_css()),
        Some(|r| r.training_exit_css()),
        GameScene::CharacterSelect,
    ),
    st(
        1,
        3,
        Some(|r| r.training_enter_sss()),
        Some(|r| r.training_exit_sss()),
        GameScene::StageSelect,
    ),
    st(
        2,
        3,
        Some(|r| r.training_enter_vs()),
        Some(|r| r.training_exit_vs()),
        GameScene::Training,
    ),
];
const HOME_RUN: [ModeState; 2] = [
    st(
        0,
        3,
        Some(|r| r.homerun_enter_css()),
        Some(|r| r.homerun_exit_css()),
        GameScene::CharacterSelect,
    ),
    st(
        1,
        3,
        Some(|r| r.homerun_enter_vs()),
        Some(|r| r.homerun_exit_vs()),
        GameScene::Vs,
    ),
];
// `gm_Mode_10ManVs_States` to `gm_Mode_CruelVs_States`: the select, then the match.
macro_rules! multi {
    ($m:ident) => {
        [
            st(
                0,
                3,
                Some(|r| r.multi_man_enter_css(GameMode::$m)),
                Some(|r| r.multi_man_exit_css()),
                GameScene::CharacterSelect,
            ),
            st(
                1,
                3,
                Some(|r| r.multi_man_enter_match(GameMode::$m)),
                Some(|r| r.multi_man_exit_match(GameMode::$m)),
                GameScene::Vs,
            ),
        ]
    };
}
const M10: [ModeState; 2] = multi!(TenMan);
const M100: [ModeState; 2] = multi!(HundredMan);
const M3: [ModeState; 2] = multi!(ThreeMinute);
const M15: [ModeState; 2] = multi!(FifteenMinute);
const MEND: [ModeState; 2] = multi!(Endless);
const MCRUEL: [ModeState; 2] = multi!(Cruel);

// The Special Melee modes with VS mode's states.
const SUPER_SUDDEN_DEATH: [ModeState; 8] = special_vs!(SuperSuddenDeath);
const INVISIBLE: [ModeState; 8] = special_vs!(Invisible);
const SLOW_MO: [ModeState; 8] = special_vs!(SlowMo);
const LIGHTNING: [ModeState; 8] = special_vs!(Lightning);
const TINY: [ModeState; 8] = special_vs!(Tiny);
const GIANT: [ModeState; 8] = special_vs!(Giant);
const FIXED_CAMERA: [ModeState; 8] = special_vs!(FixedCamera);
const SINGLE_BUTTON: [ModeState; 8] = special_vs!(SingleButton);

/// Every game mode, by number (`gm_GetAllGameModes`' table). No `on_unload` is filled: the
/// ones the modes ported this far have do nothing (VS mode's `gm_Mode_Vs_OnUnload`, Event
/// Match's `gm_Mode_Event_OnUnload`).
#[doc(alias = "gm_Mode_Event_OnUnload")]
#[doc(alias = "gm_Mode_Vs_OnUnload")]
pub static MODES: [Mode; GameMode::COUNT] = [
    Mode {
        kind: GameMode::Title,
        name: "Title",
        preloaded: true,
        on_load: None,
        on_unload: None,
        states: &TITLE,
        start: None,
    },
    Mode {
        kind: GameMode::Menu,
        name: "Main menu",
        preloaded: true,
        on_load: None,
        on_unload: None,
        states: &MENU,
        start: None,
    },
    Mode {
        kind: GameMode::Vs,
        name: "VS mode",
        preloaded: false,
        on_load: Some(|r| r.vs_mode_on_load()),
        on_unload: None,
        states: &VS,
        start: Some(VS_MELEE),
    },
    Mode {
        kind: GameMode::Classic,
        name: "Classic",
        preloaded: true,
        on_load: Some(|r| r.classic_on_load()),
        on_unload: None,
        states: &CLASSIC,
        start: Some(ONE_PLAYER_START),
    },
    Mode {
        kind: GameMode::Adventure,
        name: "Adventure",
        preloaded: false,
        on_load: Some(|r| r.adventure_on_load()),
        on_unload: None,
        states: &ADVENTURE,
        start: Some(ONE_PLAYER_START),
    },
    Mode {
        kind: GameMode::AllStar,
        name: "All-Star",
        preloaded: true,
        on_load: Some(|r| r.allstar_on_load()),
        on_unload: None,
        states: &ALLSTAR,
        start: Some(ONE_PLAYER_START),
    },
    Mode {
        kind: GameMode::Debug,
        name: "Debug menu",
        preloaded: false,
        on_load: None,
        on_unload: None,
        states: &DEBUG,
        start: Some(DEBUG_START),
    },
    Mode {
        kind: GameMode::DebugSoundTest,
        name: "Sound test (debug menu)",
        preloaded: false,
        on_load: None,
        on_unload: None,
        states: &SOUND_TEST,
        start: None,
    },
    Mode {
        kind: GameMode::HanyuCss,
        name: "Character select test",
        preloaded: false,
        on_load: None,
        on_unload: None,
        states: &HANYU_CSS,
        start: None,
    },
    Mode {
        kind: GameMode::HanyuSss,
        name: "Stage select test",
        preloaded: false,
        on_load: None,
        on_unload: None,
        states: &HANYU_SSS,
        start: None,
    },
    Mode {
        kind: GameMode::CameraMode,
        name: "Camera Mode",
        preloaded: true,
        on_load: None,
        on_unload: None,
        states: &CAMERA,
        start: Some(VS_MELEE),
    },
    Mode {
        kind: GameMode::TrophyGallery,
        name: "Trophy gallery",
        preloaded: false,
        on_load: None,
        on_unload: None,
        states: &TOY_GALLERY,
        start: None,
    },
    Mode {
        kind: GameMode::TrophyLottery,
        name: "Trophy lottery",
        preloaded: false,
        on_load: None,
        on_unload: None,
        states: &TOY_LOTTERY,
        start: None,
    },
    Mode {
        kind: GameMode::TrophyCollection,
        name: "Trophy collection",
        preloaded: false,
        on_load: None,
        on_unload: None,
        states: &TOY_COLLECTION,
        start: None,
    },
    Mode {
        kind: GameMode::DebugVs,
        name: "Debug VS",
        preloaded: false,
        on_load: None,
        on_unload: None,
        states: &DEBUG_VS,
        start: Some(VS_MELEE),
    },
    Mode {
        kind: GameMode::TargetTest,
        name: "Target Test",
        preloaded: true,
        on_load: Some(|r| r.target_on_load()),
        on_unload: None,
        states: &TARGET,
        start: Some(TARGET_START),
    },
    Mode {
        kind: GameMode::SuperSuddenDeath,
        name: "Super Sudden Death",
        preloaded: false,
        on_load: Some(|r| r.special_on_load(GameMode::SuperSuddenDeath.id())),
        on_unload: None,
        states: &SUPER_SUDDEN_DEATH,
        start: Some(VS_MELEE),
    },
    Mode {
        kind: GameMode::Invisible,
        name: "Invisible Melee",
        preloaded: false,
        on_load: Some(|r| r.special_on_load(GameMode::Invisible.id())),
        on_unload: None,
        states: &INVISIBLE,
        start: Some(VS_MELEE),
    },
    Mode {
        kind: GameMode::SlowMo,
        name: "Slo-Mo Melee",
        preloaded: false,
        on_load: Some(|r| r.special_on_load(GameMode::SlowMo.id())),
        on_unload: None,
        states: &SLOW_MO,
        start: Some(VS_MELEE),
    },
    Mode {
        kind: GameMode::Lightning,
        name: "Lightning Melee",
        preloaded: false,
        on_load: Some(|r| r.special_on_load(GameMode::Lightning.id())),
        on_unload: None,
        states: &LIGHTNING,
        start: Some(VS_MELEE),
    },
    Mode {
        kind: GameMode::ChallengerApproach,
        name: "A challenger approaching",
        preloaded: false,
        on_load: Some(|r| r.challenger_mode_on_load()),
        on_unload: None,
        states: &CHALLENGER,
        start: Some(VS_MELEE),
    },
    Mode {
        kind: GameMode::ClassicEnding,
        name: "Classic's ending",
        preloaded: false,
        on_load: Some(|r| r.gover_on_load(1)),
        on_unload: None,
        states: &GOVER,
        start: None,
    },
    Mode {
        kind: GameMode::AdventureEnding,
        name: "Adventure's ending",
        preloaded: false,
        on_load: Some(|r| r.gover_on_load(0)),
        on_unload: None,
        states: &GOVER,
        start: None,
    },
    Mode {
        kind: GameMode::AllStarEnding,
        name: "All-Star's ending",
        preloaded: false,
        on_load: Some(|r| r.gover_on_load(2)),
        on_unload: None,
        states: &GOVER,
        start: None,
    },
    Mode {
        kind: GameMode::Opening,
        name: "Opening and attract loop",
        preloaded: true,
        on_load: Some(|r| r.opening_mode_on_load()),
        on_unload: None,
        states: &OPENING,
        start: Some(ATTRACT),
    },
    Mode {
        kind: GameMode::DebugCutscenes,
        name: "Cutscenes (debug menu)",
        preloaded: false,
        on_load: None,
        on_unload: None,
        states: &DEBUG_CUTSCENE,
        start: None,
    },
    Mode {
        kind: GameMode::DebugEnding,
        name: "Ending (debug menu)",
        preloaded: false,
        on_load: None,
        on_unload: None,
        states: &DEBUG_GOVER,
        start: None,
    },
    Mode {
        kind: GameMode::Tournament,
        name: "Tournament",
        preloaded: true,
        on_load: None,
        on_unload: None,
        states: &TOURNAMENT,
        start: Some(TOURNAMENT_START),
    },
    Mode {
        kind: GameMode::Training,
        name: "Training",
        preloaded: true,
        on_load: Some(|r| r.training_on_load()),
        on_unload: None,
        states: &TRAINING,
        start: Some(TRAINING_START),
    },
    Mode {
        kind: GameMode::Tiny,
        name: "Tiny Melee",
        preloaded: false,
        on_load: Some(|r| r.special_on_load(GameMode::Tiny.id())),
        on_unload: None,
        states: &TINY,
        start: Some(VS_MELEE),
    },
    Mode {
        kind: GameMode::Giant,
        name: "Giant Melee",
        preloaded: false,
        on_load: Some(|r| r.special_on_load(GameMode::Giant.id())),
        on_unload: None,
        states: &GIANT,
        start: Some(VS_MELEE),
    },
    Mode {
        kind: GameMode::Stamina,
        name: "Stamina Mode",
        preloaded: true,
        on_load: Some(|r| r.special_on_load(GameMode::Stamina.id())),
        on_unload: None,
        states: &STAMINA,
        start: Some(VS_MELEE),
    },
    Mode {
        kind: GameMode::HomeRunContest,
        name: "Home-Run Contest",
        preloaded: true,
        on_load: Some(|r| r.homerun_on_load()),
        on_unload: None,
        states: &HOME_RUN,
        start: Some(VS_MELEE),
    },
    Mode {
        kind: GameMode::TenMan,
        name: "10-Man Melee",
        preloaded: true,
        on_load: Some(|r| r.multi_man_on_load()),
        on_unload: None,
        states: &M10,
        start: Some(MULTI_MAN_START),
    },
    Mode {
        kind: GameMode::HundredMan,
        name: "100-Man Melee",
        preloaded: true,
        on_load: Some(|r| r.multi_man_on_load()),
        on_unload: None,
        states: &M100,
        start: Some(MULTI_MAN_START),
    },
    Mode {
        kind: GameMode::ThreeMinute,
        name: "3-Minute Melee",
        preloaded: true,
        on_load: Some(|r| r.multi_man_on_load()),
        on_unload: None,
        states: &M3,
        start: Some(MULTI_MAN_START),
    },
    Mode {
        kind: GameMode::FifteenMinute,
        name: "15-Minute Melee",
        preloaded: true,
        on_load: Some(|r| r.multi_man_on_load()),
        on_unload: None,
        states: &M15,
        start: Some(MULTI_MAN_START),
    },
    Mode {
        kind: GameMode::Endless,
        name: "Endless Melee",
        preloaded: true,
        on_load: Some(|r| r.multi_man_on_load()),
        on_unload: None,
        states: &MEND,
        start: Some(MULTI_MAN_START),
    },
    Mode {
        kind: GameMode::Cruel,
        name: "Cruel Melee",
        preloaded: true,
        on_load: Some(|r| r.multi_man_on_load()),
        on_unload: None,
        states: &MCRUEL,
        start: Some(MULTI_MAN_START),
    },
    Mode {
        kind: GameMode::ProgressiveScan,
        name: "Progressive scan prompt",
        preloaded: false,
        on_load: None,
        on_unload: None,
        states: &PROG_SCAN,
        start: None,
    },
    Mode {
        kind: GameMode::Boot,
        name: "Boot",
        preloaded: true,
        on_load: None,
        on_unload: None,
        states: &BOOT,
        start: None,
    },
    Mode {
        kind: GameMode::MemCard,
        name: "Memory card prompt",
        preloaded: true,
        on_load: None,
        on_unload: None,
        states: &MEMCARD,
        start: None,
    },
    Mode {
        kind: GameMode::FixedCamera,
        name: "Fixed-Camera Mode",
        preloaded: false,
        on_load: Some(|r| r.special_on_load(GameMode::FixedCamera.id())),
        on_unload: None,
        states: &FIXED_CAMERA,
        start: Some(VS_MELEE),
    },
    Mode {
        kind: GameMode::Event,
        name: "Event Match",
        preloaded: true,
        on_load: Some(|r| r.event_on_load()),
        on_unload: None,
        states: &EVENT,
        start: Some(EVENT_START),
    },
    Mode {
        kind: GameMode::SingleButton,
        name: "Single-Button Melee",
        preloaded: false,
        on_load: Some(|r| r.special_on_load(GameMode::SingleButton.id())),
        on_unload: None,
        states: &SINGLE_BUTTON,
        start: Some(VS_MELEE),
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_enums_hold_the_games_numbers() {
        for (i, m) in GameMode::ALL.iter().enumerate() {
            assert_eq!(usize::from(m.id()), i, "{m:?}");
        }
        for (i, s) in GameScene::ALL.iter().enumerate() {
            assert_eq!(usize::from(s.id()), i, "{s:?}");
        }
    }

    #[test]
    fn the_registry_is_indexed_by_game_mode() {
        for (i, m) in MODES.iter().enumerate() {
            assert_eq!(usize::from(m.kind.id()), i, "{}", m.name);
            assert!(!m.states.is_empty(), "{} has no states", m.name);
        }
    }

    /// Each mode's states in the order `findState` and `nextState` walk them: ids rising.
    #[test]
    fn each_modes_states_rise_by_id() {
        for m in &MODES {
            for w in m.states.windows(2) {
                assert!(
                    w[0].id < w[1].id,
                    "{}: {} then {}",
                    m.name,
                    w[0].id,
                    w[1].id
                );
            }
        }
    }

    /// The modes whose match scenes take VS mode's start data are the ones whose exits take
    /// its exit data ([`uses_vs_melee`](super::super::machine::uses_vs_melee)), and the others
    /// take their own.
    #[test]
    fn the_start_data_follows_vs_melee() {
        let g = Box::<Game>::default();
        let vs = std::ptr::from_ref(&g.vs_melee.start);
        for m in &MODES {
            let takes_vs = m.start.is_some_and(|d| std::ptr::eq((d.get)(&g), vs));
            assert_eq!(
                takes_vs,
                super::super::machine::uses_vs_melee(m.kind.id()),
                "{}",
                m.name
            );
        }
        // Each pair reads and writes the same data.
        let mut g = g;
        for m in &MODES {
            if let Some(d) = m.start {
                let r = std::ptr::from_ref((d.get)(&g));
                assert!(std::ptr::eq(r, (d.get_mut)(&mut g)), "{}", m.name);
            }
        }
    }
}
