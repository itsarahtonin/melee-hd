//! The single-player audit (docs/milestone-6.md, design call 11): per single-player mode, the
//! units, modules and stops its paths reach that aren't ported, held to the port ledger and the
//! code as the stages', items', CPU players', Special Melee's and the menus' audits are
//! ([`super::roster`], [`super::items_audit`], [`super::cpu_audit`], [`super::special_audit`],
//! [`super::menus_audit`]). A mode's gate ([`super::menus_audit::GATES`]) may be lifted only
//! once no gap names the mode ([`complete`]). Presentation data only: nothing in the simulation
//! reads it.
//!
//! Each mode's task fills its entry as it ports: a gap's covers that no longer name anything
//! unported fail the test, as does an unported function of a mode's unit, or a stop in a mode's
//! module, that no gap of the mode names. Every `gated` line of `ledger/menus.txt` and every
//! menus audit stop that cites the single-player modes is some mode's gap, so what the menus'
//! gates keep out is accounted for mode by mode.

/// A single-player mode, or the developer setting's debug routes into the same content.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Training,
    HomeRun,
    TargetTest,
    MultiMan,
    Event,
    Classic,
    AllStar,
    Adventure,
    /// The debug menu's routes at a raised debug level (the developer setting): its Versus
    /// page with any boss on any stage, the Otoguro test's adventure routes, the debug
    /// cutscene (0x19) and the debug mode's milestone 6 states.
    Debug,
}

impl Mode {
    pub const ALL: [Mode; 9] = [
        Mode::Training,
        Mode::HomeRun,
        Mode::TargetTest,
        Mode::MultiMan,
        Mode::Event,
        Mode::Classic,
        Mode::AllStar,
        Mode::Adventure,
        Mode::Debug,
    ];

    /// The mode's name, as the main menu's gate names it.
    pub fn name(self) -> &'static str {
        match self {
            Mode::Training => "Training",
            Mode::HomeRun => "Home-Run Contest",
            Mode::TargetTest => "Target Test",
            Mode::MultiMan => "Multi-Man Melee",
            Mode::Event => "Event Match",
            Mode::Classic => "Classic",
            Mode::AllStar => "All-Star",
            Mode::Adventure => "Adventure",
            Mode::Debug => "the debug routes",
        }
    }

    /// The main menu's gate that keeps the mode out ([`super::menus_audit::GATES`]); the debug
    /// routes have none (the developer setting's debug menu reaches them).
    pub fn gate(self) -> Option<&'static super::menus_audit::Gate> {
        super::menus_audit::GATES
            .iter()
            .find(|g| g.what == self.name())
    }
}

/// A mode's own code: the ledger's units and melee-sim's modules only it (and the modes it
/// shares them with) reaches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Paths {
    pub mode: Mode,
    /// The decompilation's units, each of whose unaccounted functions a gap of the mode covers.
    pub units: &'static [&'static str],
    /// melee-sim's modules (`game/<module>.rs`), each of whose stops a gap of the mode covers.
    pub modules: &'static [&'static str],
    /// What the mode's units and modules hold that the mode can't reach, each by a piece of
    /// its name or its stop's text, with why: an unported function or a stop only another
    /// mode's path reaches.
    pub unreached: &'static [(&'static str, &'static str)],
}

/// Something a mode reaches that no port covers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gap {
    /// What it is, as the player would say so.
    pub what: &'static str,
    /// The ledger's functions and units, and pieces of the stops' text, it stands for.
    pub covers: &'static [&'static str],
    /// The modes that reach it.
    pub modes: &'static [Mode],
}

/// The gaps `mode` reaches.
pub fn missing(mode: Mode) -> Vec<&'static Gap> {
    MISSING.iter().filter(|g| g.modes.contains(&mode)).collect()
}

/// Whether `mode` reaches nothing unported, so its gate may be lifted.
pub fn complete(mode: Mode) -> bool {
    missing(mode).is_empty()
}

use Mode::{
    Adventure as AD, AllStar as AS, Classic as CL, Debug as DB, Event as EV, HomeRun as HR,
    MultiMan as MM, TargetTest as TT, Training as TR,
};

/// The single-player modes but the Stadium's Home-Run Contest and Target Test, whose paths
/// their own entries hold to the code ([`PATHS`]' unreached): each match is set up by its
/// mode's own entry, not the single-player setups, and has no results or clear screens; the
/// contest keeps no stocks, and their records are their own, ported.
const NOT_STADIUM: &[Mode] = &[TR, MM, EV, CL, AS, AD];
/// The modes that end with a game over, the endings, the staff roll and the trophy fall.
const ENDINGS: &[Mode] = &[CL, AS, AD, DB];

/// Target Test's units: the targets and the 26 target stages (`grt*.c`, one template).
const TARGET_TEST: &[&str] = &[
    "melee/it/kinds/itmato",
    "melee/gr/grtcaptain",
    "melee/gr/grtclink",
    "melee/gr/grtdonkey",
    "melee/gr/grtdrmario",
    "melee/gr/grtemblem",
    "melee/gr/grtfalco",
    "melee/gr/grtfox",
    "melee/gr/grtgamewatch",
    "melee/gr/grtganon",
    "melee/gr/grticeclimber",
    "melee/gr/grtkirby",
    "melee/gr/grtkoopa",
    "melee/gr/grtlink",
    "melee/gr/grtluigi",
    "melee/gr/grtmario",
    "melee/gr/grtmars",
    "melee/gr/grtmewtwo",
    "melee/gr/grtness",
    "melee/gr/grtpeach",
    "melee/gr/grtpichu",
    "melee/gr/grtpikachu",
    "melee/gr/grtpurin",
    "melee/gr/grtsamus",
    "melee/gr/grtseak",
    "melee/gr/grtyoshi",
    "melee/gr/grtzelda",
];

/// Master Hand's units (`ftmasterhand*.c`).
const MASTER_HAND: &[&str] = &[
    "melee/ft/kinds/ftMasterHand/ftmasterhandbackdisappear",
    "melee/ft/kinds/ftMasterHand/ftmasterhandcapturedamagemasterhand",
    "melee/ft/kinds/ftMasterHand/ftmasterhanddamage0",
    "melee/ft/kinds/ftMasterHand/ftmasterhandsweep",
    "melee/ft/kinds/ftMasterHand/ftmasterhandthrownmasterhand",
    "melee/ft/kinds/ftMasterHand/ftmasterhandwait10",
    "melee/ft/kinds/ftMasterHand/ftmasterhandwait12",
    "melee/it/kinds/itmasterhandbullet",
    "melee/it/kinds/itmasterhandlaser",
];

/// Crazy Hand's units (`ftcrazyhand*.c`) and his bomb.
const CRAZY_HAND: &[&str] = &[
    "melee/ft/kinds/ftCrazyHand/ftcrazyhandcapturewaitcrazyhand",
    "melee/ft/kinds/ftCrazyHand/ftcrazyhanddamage0",
    "melee/ft/kinds/ftCrazyHand/ftcrazyhandfingerbeam",
    "melee/ft/kinds/ftCrazyHand/ftcrazyhandtagcancel",
    "melee/ft/kinds/ftCrazyHand/ftcrazyhandtaggrab",
    "melee/ft/kinds/ftCrazyHand/ftcrazyhandthrowncrazyhand",
    "melee/ft/kinds/ftCrazyHand/ftcrazyhandwait10",
    "melee/it/kinds/itcrazyhandbomb",
];

/// Each mode's own units and modules. A unit only some modes reach is listed under each of
/// them; the units every mode shares (`gm_16A2` and the rest) are [`MISSING`]'s alone.
pub const PATHS: &[Paths] = &[
    Paths {
        mode: TR,
        units: &[
            "melee/gm/gmtrainingmode",
            "melee/gm/gm_1884",
            "melee/it/kinds/itmetamon",
        ],
        modules: &[],
        unreached: &[],
    },
    Paths {
        mode: HR,
        units: &[
            "melee/gm/gmhomerun",
            "melee/gm/gm_180A",
            "melee/gr/grhomerun",
            "melee/ft/kinds/ftSandbag/ftsandbag",
            "melee/it/kinds/itbat",
        ],
        // The contest's own code, and the code its path from the main menu to the contest and
        // back runs whose stops name the other single-player modes: its mode's states, the
        // one-door select, the VS scene's match with the contest's rules and its exit, the HUD
        // and the bonuses.
        modules: &[
            "gm/home_run",
            "gr/homerun",
            "ft/sandbag",
            "gm/one_player",
            "gm/machine",
            "mn/charsel/one_door",
            "gm/vs",
            "gm/match_exit",
            "gm",
            "hud/stock",
            "pl/tally",
            "gm/decisions",
            "ft/co/dead",
        ],
        unreached: HOME_RUN_UNREACHED,
    },
    Paths {
        mode: TT,
        units: TARGET_TEST,
        // The mode's own code, and the code its path from the main menu to its matches and
        // back runs whose stops name the other single-player modes: its mode's states, the
        // one-door select, the VS scene's match with the mode's rules and its exit, the
        // challenger approach after a clear, the HUD and the bonuses.
        modules: &[
            "gr/target",
            "it/kinds/mato",
            "gm/one_player",
            "gm/machine",
            "mn/charsel/one_door",
            "gm/vs",
            "gm/match_exit",
            "gm/approach",
            "gm/challenger",
            "gm",
            "hud/stock",
            "pl/tally",
            "gm/decisions",
            "ft/co/dead",
        ],
        unreached: TARGET_TEST_UNREACHED,
    },
    Paths {
        mode: MM,
        units: &[
            "melee/gm/gmmultiman",
            "melee/mn/mnhyaku",
            "melee/gm/gm_181A",
            "melee/ft/kinds/ftZakoBoy/ftboy",
            "melee/ft/kinds/ftZakoGirl/ftgirl",
        ],
        modules: &["ft/zako"],
        unreached: &[],
    },
    Paths {
        mode: EV,
        units: &[
            "melee/gm/gmevent",
            "melee/mn/mnevent",
            "melee/it/kinds/itevyoshiegg",
            "melee/it/kinds/itmetamon",
            "melee/gr/grfigure1",
            "melee/gr/grfigure2",
            "melee/gr/grfigure3",
        ],
        modules: &[
            "ft/zako",
            "ft/gkoopa",
            "gr/figure",
            "ft/bosslib",
            "ft/masterhand/airplane",
            "ft/masterhand/common",
            "ft/masterhand/crush",
            "ft/masterhand/damage",
            "ft/masterhand/entry",
            "ft/masterhand/finger",
            "ft/masterhand/grab",
            "ft/masterhand/init",
            "ft/masterhand/mod",
            "ft/masterhand/sweep",
            "ft/masterhand/wait",
            "it/kinds/masterhandbullet",
            "it/kinds/masterhandlaser",
        ],
        unreached: &[],
    },
    Paths {
        mode: CL,
        units: &[
            "melee/gm/gmclassic",
            "melee/gm/gm_1832",
            "melee/gr/grfigureget",
            "melee/gr/grpushon",
            "melee/it/kinds/itmato",
            "melee/ft/ftbosslib",
        ],
        modules: &[
            "gm/endings/mod",
            "gm/endings/game_over",
            "gr/pushon",
            "ft/bosslib",
            "ft/masterhand/airplane",
            "ft/masterhand/common",
            "ft/masterhand/crush",
            "ft/masterhand/damage",
            "ft/masterhand/entry",
            "ft/masterhand/finger",
            "ft/masterhand/grab",
            "ft/masterhand/init",
            "ft/masterhand/mod",
            "ft/masterhand/sweep",
            "ft/masterhand/wait",
            "it/kinds/masterhandbullet",
            "it/kinds/masterhandlaser",
        ],
        unreached: &[],
    },
    Paths {
        mode: AS,
        units: &[
            "melee/gm/gmallstar",
            "melee/gm/gm_17EB",
            "melee/gm/gm_186E",
            "melee/gr/grheal",
        ],
        modules: &["gm/endings/mod", "gm/endings/game_over"],
        unreached: &[],
    },
    Paths {
        mode: AD,
        units: &[
            "melee/gm/gmadventure",
            "melee/gm/gm_17E4",
            "melee/gm/gm_1879",
            "melee/gr/grkinokoroute",
            "melee/gr/grshrineroute",
            "melee/gr/grzebesroute",
            "melee/gr/grbigblueroute",
            "melee/it/kinds/itlikelike",
            "melee/it/kinds/itnokonoko",
            "melee/it/kinds/itpatapata",
            "melee/it/kinds/itzgshell",
            "melee/it/kinds/itzrshell",
            "melee/it/kinds/itwhitebea",
            "melee/it/kinds/itcoin",
            "melee/ft/kinds/ftGigaKoopa/ftgkoopa",
            "melee/ft/kinds/ftZakoBoy/ftboy",
            "melee/ft/kinds/ftZakoGirl/ftgirl",
            "melee/ft/ftCo_800C78B0",
        ],
        modules: &[
            "gm/endings/mod",
            "gm/endings/game_over",
            "ft/zako",
            "ft/gkoopa",
            "gr/bigblueroute",
            "gr/kinokoroute",
            "gr/zebesroute",
            "gr/icemt",
            "it/kinds/nokonoko",
            "it/kinds/patapata",
            "it/kinds/zgshell",
            "it/kinds/zrshell",
            "it/kinds/whitebea",
            "vi/mod",
            "vi/v0102",
            "vi/v0401",
            "vi/v0402",
            "vi/v0501",
            "vi/v0502",
            "vi/v0601",
            "vi/v0801",
        ],
        unreached: &[],
    },
    Paths {
        mode: DB,
        units: &[],
        modules: &[
            "gm/endings/mod",
            "gm/endings/game_over",
            "ft/zako",
            "ft/gkoopa",
            "gr/figure",
            "gr/pushon",
            "ft/bosslib",
            "ft/masterhand/airplane",
            "ft/masterhand/common",
            "ft/masterhand/crush",
            "ft/masterhand/damage",
            "ft/masterhand/entry",
            "ft/masterhand/finger",
            "ft/masterhand/grab",
            "ft/masterhand/init",
            "ft/masterhand/mod",
            "ft/masterhand/sweep",
            "ft/masterhand/wait",
            "it/kinds/masterhandbullet",
            "it/kinds/masterhandlaser",
            "gr/bigblueroute",
            "gr/kinokoroute",
            "gr/zebesroute",
            "gr/icemt",
            "it/kinds/nokonoko",
            "it/kinds/patapata",
            "it/kinds/zgshell",
            "it/kinds/zrshell",
            "it/kinds/whitebea",
            "vi/mod",
            "vi/v0102",
            "vi/v0401",
            "vi/v0402",
            "vi/v0501",
            "vi/v0502",
            "vi/v0601",
            "vi/v0801",
        ],
        unreached: &[],
    },
];

/// Why the Home-Run Contest reaches nothing its paths' units and modules hold unported. Its
/// match is a VS scene the contest's own entry sets up (`gm_801B9A3C`: VS mode's rules with
/// the contest's, not `gm_16A2`'s single-player setups), so the single-player setups' fields
/// stay as VS leaves them: nothing ported writes the rules' horde setup (`x54`) or
/// challenger's record (`x58`), the spawn flags (`gm.spawn_flags`) or a player's leaving flag
/// but a cutscene, and the contest's entry clears `x4_2` and `x4_4`.
const HOME_RUN_UNREACHED: &[(&str, &str)] = &[
    (
        "game mode {m:#x}'s states",
        "the contest's states (gm_Mode_Homerun_States: the select, the match) are in the scene machine, and it leaves only for them, the main menu (the select's B) or the challenger approach mode's notices (0x14, after a trophy the distance earns: gm_80173754 with no challenger), which go back to it (onExitPrize)",
    ),
    (
        "game mode {m:#x}'s loading",
        "the contest's load (gm_Mode_Homerun_OnLoad) is in the scene machine, and so are the main menu's and the challenger approach mode's",
    ),
    (
        "the mode override",
        "the game installs one override, the boot's card check, ported",
    ),
    (
        "the VS scene's enter data in game mode",
        "the contest's match enters with VS mode's start data (uses_vs_melee)",
    ),
    (
        "game scene {k:#x}",
        "the contest's scenes are the one-door select and the VS scene, and the notices' prize scene, each in the scene machine",
    ),
    (
        "gm_801BEE9C (congratulations' exit)",
        "the game over modes' congratulations: the contest has no ending",
    ),
    (
        "Hook::Unported1P(name)",
        "the contest's states hook its select's and its match's entries and exits (HomeRunEnterCss, HomeRunExitCss, HomeRunEnterVs, HomeRunExitVs), none unported",
    ),
    (
        "a 1P run's getter",
        "only All-Star's round setup asks a getter",
    ),
    (
        "the one-door screen's panel for match type",
        "the contest's select is the Stadium's Home-Run match type (0x10, which gm_801B98E8 sets), whose panel is ported",
    ),
    (
        "mnCharSel_8025C020's records for match type",
        "the contest's select shows the Home-Run records (match type 0x10), ported",
    ),
    (
        "gm_80167320 for a leaving slot",
        "a player's leaving flag (Player_SetFlagsBit1), which only the giant Kirby cutscene (vi0501) sets",
    ),
    (
        "fn_80169444 (a single-player mode's last stock)",
        "the spawn flags' 0x40, which only gm_16A2's single-player setups set",
    ),
    (
        "gm_8016AC44 with a challenger's record",
        "the rules' challenger's record (x58), which only gm_16A2's single-player setups set",
    ),
    (
        "HUD colour {c} past the table",
        "no caller passes a colour past the table's nine",
    ),
    (
        "single-player match setup",
        "the rules' horde setup (x54), which only gm_16A2's single-player setups set",
    ),
    (
        "a match's exit data in game mode",
        "the contest's match exits into VS mode's exit data (uses_vs_melee)",
    ),
    (
        "fn_8017F294 (a single-player run's exit value)",
        "the rules' x4_4, which the contest's entry clears (gm_801B9A3C)",
    ),
    (
        "fn_8016C46C's single-player results",
        "Classic's, Adventure's and All-Star's results (game modes 3 to 5)",
    ),
    (
        "gm_801674C4 (a stage's match event)",
        "only the adventure routes' stages start one (Mushroom Kingdom's, the Underground Maze's, Icicle Mountain's climb)",
    ),
    (
        "fn_802FA6C4 in a single-player mode",
        "the spawn flags' 0x40, which only gm_16A2's single-player setups set",
    ),
    (
        "fn_802FAC34 with a challenger's record",
        "the rules' challenger's record (x58), which only gm_16A2's single-player setups set",
    ),
    (
        "ifStock_802FA5BC",
        "the spawn flags' 0x10 and 0x40, which only gm_16A2's single-player setups set",
    ),
    (
        "the multi-man counter of ifStock_802FAEC4",
        "the Multi-Man modes' (game modes 0x21 to 0x26)",
    ),
    (
        "gm_801720B4 in a single-player run",
        "Classic's, Adventure's and All-Star's runs (is_1p_run)",
    ),
    (
        "gm_80172140 in a 1P run",
        "Classic's, Adventure's and All-Star's runs (is_1p_run)",
    ),
    (
        "fn_80171A88 in a 1P run",
        "Classic's, Adventure's and All-Star's runs (is_1p_run)",
    ),
    (
        "gm_801693BC for a slot that leaves",
        "a player's leaving flag, which only the giant Kirby cutscene (vi0501) sets",
    ),
    (
        "gm_801647F8 reading past lbl_803D5168",
        "Target Test's match entry alone asks a character's target stage",
    ),
    (
        "itBat_Logic11_EvtUnk",
        "an item's evt_unk runs only from it_8026B7F8, in a fighter's unload (Fighter_Unload_8006DABC): mid-match only through Player_80031EBC, whose callers are Training's count (fn_8016EF98), a leaving slot (gm_80167320) and the trophy fall, none in the contest; at the scene's end as in VS (items_audit's EVT_UNK)",
    ),
];

/// Why Target Test reaches nothing its paths' units and modules hold unported. Its match is a
/// VS scene the mode's own entry sets up (`gm_801B6428`: the select's rules with the mode's,
/// not `gm_16A2`'s single-player setups), so the single-player setups' fields stay as VS leaves
/// them: nothing ported writes the rules' horde setup (`x54`) or challenger's record (`x58`),
/// the spawn flags (`gm.spawn_flags`) or a player's leaving flag but a cutscene, and the
/// mode's entry clears `x4_4`.
const TARGET_TEST_UNREACHED: &[(&str, &str)] = &[
    (
        "game mode {m:#x}'s states",
        "the mode's states (its select and its match) are in the scene machine, and it leaves only for them, the main menu (the select's B) or the challenger approach mode (0x14) after a clear that earns Mr. Game & Watch's challenge or a notice (gm_801B65D4), which comes back to it (onExitVs@801BFBA8 after the challenger's match, onExitPrize after the notices)",
    ),
    (
        "game mode {m:#x}'s loading",
        "the mode's load is in the scene machine, and so are the main menu's and the challenger approach mode's",
    ),
    (
        "the mode override",
        "the game installs one override, the boot's card check, ported",
    ),
    (
        "the VS scene's enter data in game mode",
        "the mode's match enters with its own start data (gm_801B6428's), in the scene machine",
    ),
    (
        "game scene {k:#x}",
        "the mode's scenes are the one-door select and the VS scene, and the challenger approach mode's (the approach, the challenger's match, the notices' prize scene), each in the scene machine",
    ),
    (
        "gm_801BEE9C (congratulations' exit)",
        "the game over modes' congratulations: Target Test has no ending",
    ),
    (
        "Hook::Unported1P(name)",
        "the mode's states hook its select's and its match's entries and exits, none unported",
    ),
    (
        "a 1P run's getter",
        "only All-Star's round setup asks a getter",
    ),
    (
        "the one-door screen's panel for match type",
        "the mode's select is the Stadium's Target Test match type (0xF), whose panel is ported",
    ),
    (
        "mnCharSel_8025C020's records for match type",
        "the mode's select shows the Target Test records (match type 0xF), ported",
    ),
    (
        "gm_80167320 for a leaving slot",
        "a player's leaving flag (Player_SetFlagsBit1), which only the giant Kirby cutscene (vi0501) sets",
    ),
    (
        "fn_80169444 (a single-player mode's last stock)",
        "the spawn flags' 0x40, which only gm_16A2's single-player setups set",
    ),
    (
        "gm_8016AC44 with a challenger's record",
        "the rules' challenger's record (x58), which only gm_16A2's single-player setups set",
    ),
    (
        "HUD colour {c} past the table",
        "no caller passes a colour past the table's nine",
    ),
    (
        "single-player match setup",
        "the rules' horde setup (x54), which only gm_16A2's single-player setups set",
    ),
    (
        "a match's exit data in game mode",
        "the mode's match exits into its own exit data (match_exit_data's Target Test arm), and the challenger's match into the challenger approach mode's",
    ),
    (
        "fn_8017F294 (a single-player run's exit value)",
        "the rules' x4_4, which the mode's entry clears (gm_801B6428)",
    ),
    (
        "fn_8016C46C's single-player results",
        "Classic's, Adventure's and All-Star's results (game modes 3 to 5)",
    ),
    (
        "gm_801674C4 (a stage's match event)",
        "only the adventure routes' stages start one (Mushroom Kingdom's, the Underground Maze's, Icicle Mountain's climb)",
    ),
    (
        "fn_802FA6C4 in a single-player mode",
        "the spawn flags' 0x40, which only gm_16A2's single-player setups set",
    ),
    (
        "fn_802FAC34 with a challenger's record",
        "the rules' challenger's record (x58), which only gm_16A2's single-player setups set",
    ),
    (
        "ifStock_802FA5BC",
        "the spawn flags' 0x10 and 0x40, which only gm_16A2's single-player setups set",
    ),
    (
        "the multi-man counter of ifStock_802FAEC4",
        "the Multi-Man modes' (game modes 0x21 to 0x26)",
    ),
    (
        "gm_801720B4 in a single-player run",
        "Classic's, Adventure's and All-Star's runs (is_1p_run)",
    ),
    (
        "gm_80172140 in a 1P run",
        "Classic's, Adventure's and All-Star's runs (is_1p_run)",
    ),
    (
        "fn_80171A88 in a 1P run",
        "Classic's, Adventure's and All-Star's runs (is_1p_run)",
    ),
    (
        "gm_801693BC for a slot that leaves",
        "a player's leaving flag, which only the giant Kirby cutscene (vi0501) sets",
    ),
    (
        "gm_801647F8 reading past lbl_803D5168",
        "every character the one-door select offers (character kinds 0 to 0x19, the Ice Climbers as Popo) has its entry in the table",
    ),
];

/// What the single-player modes reach that isn't ported, each with the modes that reach it.
pub const MISSING: &[Gap] = &[
    // The way in: the main menu's modes, their scenes and the matches' setup.
    Gap {
        what: "the single-player modes' scenes and states",
        covers: &[
            "Hook::Unported1P(name)",
            "game mode {m:#x}'s states",
            "game mode {m:#x}'s loading",
            "game scene {k:#x}",
        ],
        modes: &[TR, MM, EV, CL, AS, AD, DB],
    },
    Gap {
        what: "the one-door character select",
        covers: &["mnCharSel_8025C020"],
        modes: NOT_STADIUM,
    },
    Gap {
        what: "the single-player matches' setup (teams, hordes, a leaving slot, the stage setups)",
        covers: &[
            "melee/gm/gm_16A2",
            "melee/gm/gm_17AD",
            "melee/gm/gm_17BA",
            "melee/gm/gm_17C0",
            "melee/gm/gm_18A1",
            "single-player match setup",
            "the VS scene's enter data in game mode",
            "a match's exit data in game mode",
            "gm_8016AC44 with a challenger's record",
            "fn_802FAC34 with a challenger's record",
        ],
        modes: NOT_STADIUM,
    },
    Gap {
        what: "a horde's players leaving their slots",
        covers: &[
            "gm_80167320 for a leaving slot",
            "gm_801693BC for a slot that leaves",
        ],
        modes: &[MM, AD, EV],
    },
    Gap {
        what: "the single-player matches' shared code (the stocks, the results, the clear screens)",
        covers: &[
            "melee/gm/gmregclear",
            "melee/gm/gmregcommon",
            "fn_8016AE60",
            "fn_8016B138",
            "gm_8016B350",
            "gm_8016B364",
            "gm_8016B378",
            "fn_8016B4BC",
            "fn_8016B510",
            "fn_8016B738",
            "fn_8016D538",
            "gm_8016EDDC",
            "fn_8016EF98",
            "gm_8016F00C",
            "fn_8016F030",
            "gm_GetPreviousSceneIndex",
            "gm_GetCurrentSceneEnterData",
            "lbAudioAx_80028B6C",
            "lbAudioAx_80028B90",
            "fn_80169444 (a single-player mode's last stock)",
            "fn_8016C46C's single-player results",
            "fn_8017F294 (a single-player run's exit value)",
        ],
        modes: NOT_STADIUM,
    },
    Gap {
        what: "the 1P HUD (the stocks and counters)",
        covers: &["ifStock_802FA5BC", "fn_802FA6C4 in a single-player mode"],
        modes: NOT_STADIUM,
    },
    Gap {
        what: "the 1P records and saves",
        covers: &[
            "gmMainLib_8015D134",
            "gmMainLib_8015D1C8",
            "gmMainLib_8015D25C",
            "gmMainLib_8015D2F0",
            "gmMainLib_8015D344",
            "gmMainLib_8015D384",
            "gmMainLib_8015D3FC",
            "gmMainLib_8015D418",
            "gmMainLib_8015D6A4",
            "gmMainLib_8015D6BC",
            "gmMainLib_8015D6D8",
            "gmMainLib_8015D6F8",
            "gmMainLib_8015D72C",
            "gmMainLib_8015D74C",
            "gmMainLib_8015D780",
            "gmMainLib_8015D7A4",
            "gmMainLib_8015D7BC",
            "gmMainLib_8015D7D4",
            "fn_80160DE8",
            "fn_80160F58",
            "fn_80162BFC",
            "fn_80162DF8",
            "fn_80162FF4",
            "gm_8016365C",
            "gm_80163690",
            "gm_801636D8",
            "gm_80163838",
            "gm_801639C0",
            "gm_801639F4",
            "gm_80163A3C",
            "gm_80163B9C",
            "gm_Get3MinMultimanHighscore",
            "gm_Get3MinMultimanTotalHighscore",
            "gm_Get15MinMultimanHighscore",
            "gm_Get15MinMultimanTotalHighscore",
            "gm_GetEndlessHighscore",
            "gm_GetEndlessTotalHighscore",
            "gm_GetCruelHighscore",
            "gm_GetCruelTotalHighscore",
            "gm_801647F8",
            "gm_801674C4",
            "fn_801689E4",
            "fn_80168A6C",
            "gm_801720B4 in a single-player run",
            "fn_8017DF28 (a single-player mode's record)",
        ],
        modes: NOT_STADIUM,
    },
    Gap {
        what: "the unlocks, trophies and challengers after a mode",
        covers: &[
            "melee/gm/gm_17DB",
            "fn_8016F344",
            "fn_8016F39C",
            "fn_8016F548",
            "fn_8016F740",
            "fn_8016F870",
            "fn_8016F9A8",
            "fn_80173098",
            "gm_80173224",
            "gm_80173460",
            "gm_80173498",
            "fn_80173834",
            "gm_8017390C",
            "gm_80173C70",
        ],
        modes: NOT_STADIUM,
    },
    Gap {
        what: "the 1P runs' falls and progress the bonuses ask about",
        covers: &["gm_80172140 in a 1P run", "fn_80171A88 in a 1P run"],
        modes: &[CL, AS, AD],
    },
    // The modes' own code.
    Gap {
        what: "Training's computer players' count and reset (a fighter made and unloaded mid-match)",
        covers: &[
            "gm_8016EDDC (a player joining a match)",
            "fn_8016EF98 (a player leaving a match)",
            "Player_800328D4 (the training menu's reset)",
            "fn_80188550",
            "fn_80188644",
        ],
        modes: &[TR],
    },
    Gap {
        what: "Ditto",
        covers: &["melee/it/kinds/itmetamon"],
        modes: &[TR, EV],
    },
    Gap {
        what: "the Multi-Man modes' part of gmmultiman.c (Target Test's is ported)",
        covers: &["melee/gm/gmmultiman"],
        modes: &[MM],
    },
    Gap {
        what: "Multi-Man Melee's list, mode and counter, and the Wire Frames",
        covers: &[
            "the one-door screen's panel for match type",
            "mnCharSel_8025C020's records for match type",
            "melee/mn/mnhyaku",
            "melee/gm/gm_181A",
            "mnHyaku_8024CD64 (the Multi-Man list)",
            "the multi-man counter of ifStock_802FAEC4",
        ],
        modes: &[MM],
    },
    Gap {
        what: "the Wire Frames",
        covers: &[
            "melee/ft/kinds/ftZakoBoy/ftboy",
            "melee/ft/kinds/ftZakoGirl/ftgirl",
        ],
        modes: &[MM, AD, EV, DB],
    },
    Gap {
        what: "Event Match's list, events and egg",
        covers: &[
            "melee/gm/gmevent",
            "melee/mn/mnevent",
            "melee/it/kinds/itevyoshiegg",
            "mnEvent_8024E838 (the Event list)",
            "lbDvd_80018A2C",
            "lbDvd_80018C2C",
        ],
        modes: &[EV],
    },
    Gap {
        what: "the events' trophy stages",
        covers: &[
            "melee/gr/grfigure1",
            "melee/gr/grfigure2",
            "melee/gr/grfigure3",
        ],
        modes: &[EV, DB],
    },
    Gap {
        what: "Classic's mode, its intro and its bonus rounds",
        covers: &[
            "melee/gm/gmclassic",
            "melee/gm/gm_1832",
            "melee/gr/grfigureget",
            "melee/gr/grpushon",
        ],
        modes: &[CL, DB],
    },
    Gap {
        what: "Master Hand",
        covers: MASTER_HAND,
        modes: &[CL, EV, DB],
    },
    Gap {
        what: "Crazy Hand",
        covers: CRAZY_HAND,
        modes: &[CL, EV, DB],
    },
    Gap {
        what: "the hands' shared moves, sounds and boss stage",
        covers: &[
            "it_8026C3FC",
            "it_8026C42C",
            "ftCo_800D4F24",
            "ftMh_MS_371_801541C8",
            "Master Hand's function",
            "melee/ft/ftbosslib",
            "lbAudioAx_8002438C",
            "gm_8016ECE8",
        ],
        modes: &[CL, EV, DB],
    },
    Gap {
        what: "All-Star's mode, its intro and the rest area",
        covers: &[
            "a 1P run's getter",
            "melee/gm/gmallstar",
            "melee/gm/gm_17EB",
            "melee/gm/gm_186E",
            "melee/gr/grheal",
        ],
        modes: &[AS, DB],
    },
    Gap {
        what: "Adventure's mode and its intro",
        covers: &[
            "melee/gm/gmadventure",
            "melee/gm/gm_17E4",
            "melee/gm/gm_1879",
        ],
        modes: &[AD],
    },
    Gap {
        what: "the adventure routes (Mushroom Kingdom, the Underground Maze, Brinstar's escape, the F-Zero race, Icicle Mountain's climb)",
        covers: &[
            "a car out with all thirty out",
            "melee/gr/grkinokoroute",
            "melee/gr/grshrineroute",
            "melee/gr/grzebesroute",
            "melee/gr/grbigblueroute",
            "stageGObj10_Callback3",
            "fn_801FA4CC",
            "stage kind {grkind}",
        ],
        modes: &[AD, DB],
    },
    Gap {
        what: "the adventure enemies, their spawner and the 1P coin",
        covers: &[
            "melee/it/kinds/itlikelike",
            "melee/it/kinds/itnokonoko",
            "melee/it/kinds/itpatapata",
            "melee/it/kinds/itzgshell",
            "melee/it/kinds/itzrshell",
            "melee/it/kinds/itwhitebea",
            "melee/it/kinds/itcoin",
            "melee/ft/ftCo_800C78B0",
            "fn_800C7890",
            "it_802EA9FC",
            "grZakoGenerator_801CAF08",
            "it_8027B5B0 (an enemy of kind",
            "grZakoGenerator_801CAC14",
            "grZakoGenerator_801CACB8",
            "pl_8004049C (an enemy of kind",
        ],
        modes: &[AD, DB],
    },
    Gap {
        what: "a beaten enemy's or a trophy stage's trophy",
        covers: &[
            "it_8027CC88 (a 1P mode's trophy from an enemy)",
            "Toy_803049F4",
            "Toy_80304A58",
            "tyDisplay_8031C354",
            "tyDisplay_8031C5E4",
        ],
        modes: &[AD, CL, DB],
    },
    Gap {
        what: "the cutscenes after the F-Zero one",
        covers: &["the cutscene scene"],
        modes: &[AD, DB],
    },
    Gap {
        what: "the cutscenes' fighters and stages",
        covers: &["demo setup", "ftDemo_CreateFighter's pose"],
        modes: &[AD, CL, AS, DB],
    },
    // The ends of a run.
    Gap {
        what: "the game overs, the endings, the staff roll, the trophy fall and congratulations",
        covers: &[
            "gm_801BEE9C (congratulations' exit)",
            "melee/gm/gmgover",
            "melee/gm/gm_19EF",
            "melee/gm/gmmovieend",
            "melee/lb/lb_01F8",
            "melee/gm/gmstaffroll",
            "melee/gm/gmregtyfall",
            "melee/gm/gmregenddisp",
            "melee/gm/gm_1A7A",
            "melee/gm/gm_1A9B",
            "gm_Mode_ClassicGOver_OnLoad",
            "gm_Mode_AdventureGOver_OnLoad",
            "gm_Mode_AllstarGOver_OnLoad",
            "lbFile_80016760",
            "tyDisplay_8031BB34",
            "tyDisplay_8031BB94",
        ],
        modes: ENDINGS,
    },
    // The developer setting's debug routes.
    Gap {
        what: "the debug mode's milestone 6 states and the debug cutscene",
        covers: &[
            "onEnterIntroEasy",
            "onEnterIntroAllstar",
            "un_80301BA8",
            "un_80301C64",
            "un_80301B48",
            "onExitRegendCongrats",
            "un_80301028",
            "un_80301074",
            "un_803010C0",
            "un_80301280",
            "fn_803011EC",
            "un_80301D7C",
        ],
        modes: &[DB],
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::roster::tests::{game_modules, ledger, stops};
    use std::collections::{BTreeMap, BTreeSet};

    const TAB: char = '\t';

    /// `ledger/menus.txt`'s lines: name, class and why.
    fn menus_txt() -> Vec<(String, String, String)> {
        let path = format!("{}/../../ledger/menus.txt", env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
        text.lines()
            .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
            .map(|l| {
                let mut f = l.splitn(3, TAB);
                let n = f.next().unwrap_or("").to_owned();
                let c = f.next().unwrap_or("").to_owned();
                let w = f.next().unwrap_or("").to_owned();
                (n, c, w)
            })
            .collect()
    }

    /// Whether `cover` still names something unported: an unaccounted function, a unit with an
    /// unaccounted function that `ledger/menus.txt` doesn't call unchecked or unreached, or a
    /// piece of a stop's text.
    fn live(
        cover: &str,
        unaccounted: &BTreeMap<String, String>,
        held: &BTreeSet<String>,
        every_stop: &[String],
    ) -> bool {
        unaccounted.contains_key(cover)
            || unaccounted
                .iter()
                .any(|(n, u)| u == cover && !held.contains(n))
            || every_stop.iter().any(|s| s.contains(cover))
    }

    /// The audit is held to the ledger and the code: every unaccounted function of a mode's
    /// units and every stop in a mode's modules is a gap of that mode; every gap's covers still
    /// name something unported; every `gated` line of `ledger/menus.txt` and every menus audit
    /// stop that cites the single-player modes is some gap's; and a mode whose gate is lifted
    /// has no gap. So a port that lands, a new stop or a lifted gate has to update the audit.
    #[test]
    fn single_player_modes_reach_nothing_unaccounted() {
        let ledger = ledger();
        let unaccounted: BTreeMap<String, String> = ledger
            .iter()
            .filter(|(_, _, s)| s == "unaccounted")
            .map(|(n, u, _)| (n.clone(), u.clone()))
            .collect();
        let menus = menus_txt();
        // Functions the menus' ledger calls ported inline or unreached: not the modes' gaps.
        let held: BTreeSet<String> = menus
            .iter()
            .filter(|(_, c, _)| c != "gated")
            .map(|(n, _, _)| n.clone())
            .collect();
        let modules = game_modules();
        let every_stop: Vec<String> = modules.iter().flat_map(|m| stops(m)).collect();
        let mut out = Vec::new();

        for p in PATHS {
            let gaps = missing(p.mode);
            let mut used = vec![false; p.unreached.len()];
            let mut covered = |text: &str, unit: Option<&str>| {
                let mut hit = false;
                for (i, (k, _)) in p.unreached.iter().enumerate() {
                    if text.contains(k) {
                        used[i] = true;
                        hit = true;
                    }
                }
                hit || gaps.iter().any(|g| {
                    g.covers
                        .iter()
                        .any(|c| text.contains(c) || unit.is_some_and(|u| u == *c))
                })
            };
            for u in p.units {
                if !ledger.iter().any(|(_, unit, _)| unit == u) {
                    out.push(format!("{}: the ledger has no unit {u}", p.mode.name()));
                }
                for (n, unit) in unaccounted.iter().filter(|(_, unit)| unit == u) {
                    if !covered(n, Some(unit)) {
                        out.push(format!(
                            "{}: {n} ({unit}) isn't ported, and no gap of the mode names it",
                            p.mode.name()
                        ));
                    }
                }
            }
            for m in p.modules {
                if !modules.iter().any(|n| n == m) {
                    out.push(format!("{}: the module {m} is gone", p.mode.name()));
                    continue;
                }
                for stop in stops(m) {
                    if !covered(&stop, None) {
                        out.push(format!(
                            "{}: game/{m}.rs stops, and no gap of the mode names it: {stop}",
                            p.mode.name()
                        ));
                    }
                }
            }
            for ((k, why), used) in p.unreached.iter().zip(used) {
                if !used {
                    out.push(format!(
                        "{}: its unreached {k:?} names nothing unported in its units or a stop in its modules",
                        p.mode.name()
                    ));
                }
                if why.is_empty() {
                    out.push(format!(
                        "{}: its unreached {k:?} says no why",
                        p.mode.name()
                    ));
                }
            }
        }
        for g in MISSING {
            if g.modes.is_empty() {
                out.push(format!("the gap {:?} names no mode", g.what));
            }
            for c in g.covers {
                if !live(c, &unaccounted, &held, &every_stop) {
                    out.push(format!(
                        "the gap {:?} names {c}, which is no longer unported or a stop",
                        g.what
                    ));
                }
            }
        }
        // What the menus' gates keep out is each some mode's.
        let named = |n: &str| MISSING.iter().any(|g| g.covers.contains(&n));
        for (n, c, _) in &menus {
            if c != "gated" {
                continue;
            }
            let unit = unaccounted.get(n).map(String::as_str);
            if !named(n) && !unit.is_some_and(named) {
                out.push(format!(
                    "ledger/menus.txt gates {n}, and no single-player gap names it"
                ));
            }
        }
        for (key, why) in crate::game::menus_audit::STOPS {
            if (why.contains("single-player") || why.contains("GATES") || why.contains("gate"))
                && !named(key)
            {
                out.push(format!(
                    "the menus audit's stop {key:?} cites the single-player modes, and no single-player gap names it"
                ));
            }
        }
        // A mode's gate goes only once nothing it reaches is missing.
        for m in Mode::ALL {
            if m != Mode::Debug && m.gate().is_none() && !complete(m) {
                out.push(format!(
                    "{}'s gate is lifted, and the mode still reaches: {}",
                    m.name(),
                    missing(m)
                        .iter()
                        .map(|g| g.what)
                        .collect::<Vec<_>>()
                        .join("; ")
                ));
            }
        }
        assert!(out.is_empty(), "{}\n{} problems", out.join("\n"), out.len());
    }

    /// Every mode has its paths once and every gate its mode; the debug routes have no gate,
    /// and the modes whose gates are lifted are those the player offers (the Home-Run Contest
    /// and Target Test, docs/player.md).
    #[test]
    fn modes_match_the_gates() {
        for m in Mode::ALL {
            assert_eq!(
                PATHS.iter().filter(|p| p.mode == m).count(),
                1,
                "{}",
                m.name()
            );
            let lifted = matches!(m, Mode::Debug | Mode::HomeRun | Mode::TargetTest);
            assert_eq!(m.gate().is_none(), lifted, "{}", m.name());
        }
        for g in crate::game::menus_audit::GATES {
            assert!(
                Mode::ALL.iter().any(|m| m.name() == g.what),
                "the gate {} names no single-player mode",
                g.what
            );
        }
    }
}
