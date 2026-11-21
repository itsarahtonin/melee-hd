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
}
