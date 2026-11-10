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

/// The single-player modes whose matches `gm_16A2`'s single-player setups make: all but
/// Training and the Stadium's Home-Run Contest, Target Test and Multi-Man Melee, whose paths
/// their own entries hold to the code ([`PATHS`]' unreached): each of those matches is set up
/// by its mode's own entry, not the single-player setups, and has no results or clear screens;
/// the contest and Training keep no stocks, and their records are their own, ported.
const SETUP_MODES: &[Mode] = &[EV, CL, AS, AD];
/// The 1P runs (Classic, All-Star, Adventure): [`SETUP_MODES`] without Event Match, for the
/// gaps no event reaches (each gap says why).
const ONE_P_RUNS: &[Mode] = &[CL, AS, AD];

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
    "melee/ft/kinds/ftMasterHand/ftmasterhandthrownmasterhand",
    "melee/ft/kinds/ftMasterHand/ftmasterhandwait10",
    "melee/ft/kinds/ftMasterHand/ftmasterhandwait12",
    "melee/it/kinds/itmasterhandbullet",
    "melee/it/kinds/itmasterhandlaser",
];

/// Crazy Hand's units (`ftcrazyhand*.c`) and his bomb.
const CRAZY_HAND: &[&str] = &[
    "melee/ft/kinds/ftCrazyHand/ftcrazyhandcapturewaitcrazyhand",
    "melee/ft/kinds/ftCrazyHand/ftcrazyhandfingerbeam",
    "melee/ft/kinds/ftCrazyHand/ftcrazyhandtagcancel",
    "melee/ft/kinds/ftCrazyHand/ftcrazyhandthrowncrazyhand",
    "melee/ft/kinds/ftCrazyHand/ftcrazyhandwait10",
    "melee/it/kinds/itcrazyhandbomb",
];

/// Each mode's own units and modules. A unit only some modes reach is listed under each of
/// them; the units every mode shares (`gm_16A2` and the rest) are [`MISSING`]'s alone.
pub const PATHS: &[Paths] = &[
    // Training's computer players' count and reset (`game/gm/training.rs`) make and free
    // fighters mid-match, each function checked by a session from power-on (ledger/checks.txt,
    // docs/oracle.md's sessions): the count, `fn_80188550`, with a player joining,
    // `gm_8016EDDC`, and leaving, `fn_8016EF98`, which frees its fighter
    // (`Player_80031EBC`, `Fighter_Unload_8006DABC` in `game/ft/unload.rs`, every item's
    // `evt_unk` in `game/it/evt_unk.rs`), in `m6-training/tr-count` and
    // `m6-fighter-unload/un-churn` and `un-held`; the reset, `fn_80188644`, putting each
    // slot back (`Player_800328D4`, `ftLib_80087140`), in `m6-fighter-unload/un-reset`. Their
    // modules are the mode's, so a stop added to them has to say why Training can't reach it.
    Paths {
        mode: TR,
        units: &[
            "melee/gm/gmtrainingmode",
            "melee/gm/gm_1884",
            "melee/it/kinds/itmetamon",
            "melee/it/kinds/itcoin",
        ],
        // The mode's own code, and the code its path from the main menu to its match and back
        // runs whose stops name the other single-player modes: its mode's states, the one-door
        // select, the stage select, the VS scene's match with Training's rules and its exit,
        // the notices after a trophy, the HUD and the bonuses; the Pokémon its balls let out,
        // Pokémon Stadium's screen in Training, the enemies' common code a beaten Birdo runs on
        // Mushroom Kingdom II, and the 1P coin, which Training never makes.
        modules: &[
            "gm/training",
            "ft/unload",
            "it/evt_unk",
            "gm/one_player",
            "gm/machine",
            "mn/charsel/one_door",
            "mn/stagesel",
            "gm/vs",
            "gm/match_exit",
            "gm/approach",
            "gm/challenger",
            "gm",
            "hud/stock",
            "pl/tally",
            "gm/decisions",
            "ft/co/dead",
            "it/pokemon/mod",
            "gr/stadium",
            "it/zako",
            "it/kinds/trophycoin",
        ],
        unreached: TRAINING_UNREACHED,
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
        // The modes' own code, and the code their path from the main menu to the matches and
        // back runs whose stops name the other single-player modes: the list, the modes'
        // states, the one-door select, the VS scene's match with the modes' rules and its
        // exit, the Wire Frames joining and leaving (their computer kinds, the unload), the
        // challenger approach after 100-Man Melee, the HUD and the bonuses.
        modules: &[
            "gm/multi_man",
            "mn/hyaku",
            "ft/zako",
            "ft/cpu_ai",
            "ft/unload",
            "it/evt_unk",
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
        unreached: MULTI_MAN_UNREACHED,
    },
    Paths {
        mode: EV,
        units: &[
            "melee/gm/gmevent",
            "melee/mn/mnevent",
            "melee/it/kinds/itevyoshiegg",
            "melee/gr/grfigure1",
            "melee/gr/grfigure2",
            "melee/gr/grfigure3",
        ],
        modules: &[
            "gm/event",
            "mn/event",
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
        unreached: &[
            (
                "an event's random fifth player",
                "no event's fifth or sixth player is a random character: onEnterVs runs each of an event's players through gm/event.rs's event_other_player as its match starts, and every one of the 51 events' sessions enters its match without stopping there (docs/oracle.md, sessions)",
            ),
            (
                "itEvYoshiEgg_Logic42_EvtUnk",
                "the egg's evt_unk runs only from a fighter's unload (it_8026B7F8 in Fighter_Unload_8006DABC), and Event 13, the one event with the egg, frees no fighter mid-match: it has no joiners and no player leaves, and a match's end frees none (ssbm-rs's CALLS over m6-cpu-kinds/ck17-ev13, the egg kept to the time's end, logs no Fighter_Unload_8006DABC)",
            ),
        ],
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
            "gm/classic",
            "gm/intro",
            "gm/intro_horde",
            "gm/endings/mod",
            "gm/endings/game_over",
            "gr/pushon",
            "gr/figureget",
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
        unreached: &[(
            "the cutscene scene",
            "Adventure's states (gm_Mode_Adventure_States, gm/machine.rs's ADVENTURE) name only the cutscene scenes 0x11 to 0x1B, each entered and its files read by vi/mod.rs's own case (vi0102 to vi1202); the stops are for any other scene, which only the debug cutscene mode's numbers could name",
        )],
    },
    Paths {
        mode: DB,
        units: &[],
        modules: &[
            "gm/intro_allstar",
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
/// challenger's record (`x58`), the joining opponents' setup (`gm.opponents`) or a player's leaving flag
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
        "HUD colour {c} past the table",
        "no caller passes a colour past the table's nine",
    ),
    (
        "a match's exit data in game mode",
        "the contest's match exits into VS mode's exit data (uses_vs_melee)",
    ),
    (
        "gm_80167320's joiners' leaving",
        "the joining opponents' setup's flag (gm.opponents, gm_8016A1F8), which only gm_16A2's single-player setups and the adventure routes' arenas set",
    ),
    (
        "gm_801647F8 reading past lbl_803D5168",
        "Target Test's match entry alone asks a character's target stage",
    ),
];

/// Why Target Test reaches nothing its paths' units and modules hold unported. Its match is a
/// VS scene the mode's own entry sets up (`gm_801B6428`: the select's rules with the mode's,
/// not `gm_16A2`'s single-player setups), so the single-player setups' fields stay as VS leaves
/// them: nothing ported writes the rules' horde setup (`x54`) or challenger's record (`x58`),
/// the joining opponents' setup (`gm.opponents`) or a player's leaving flag but a cutscene,
/// and the mode's entry clears `x4_4`.
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
        "HUD colour {c} past the table",
        "no caller passes a colour past the table's nine",
    ),
    (
        "a match's exit data in game mode",
        "the mode's match exits into its own exit data (match_exit_data's Target Test arm), and the challenger's match into the challenger approach mode's",
    ),
    (
        "gm_80167320's joiners' leaving",
        "the joining opponents' setup's flag (gm.opponents, gm_8016A1F8), which only gm_16A2's single-player setups and the adventure routes' arenas set",
    ),
    (
        "gm_801647F8 reading past lbl_803D5168",
        "every character the one-door select offers (character kinds 0 to 0x19, the Ice Climbers as Popo) has its entry in the table",
    ),
];

/// Why Multi-Man Melee reaches nothing its paths' units and modules hold unported. Its matches
/// are VS scenes the modes' own entries set up (`gm_801B6BE8` and the others: the mode's
/// rules with `gm_801B69C0`'s, not `gm_16A2`'s single-player setups), and their opponents join
/// through VS mode's `gm_8016EDDC` (the match isn't single-player, `is_singleplayer` 0), so
/// the single-player setups' fields stay as VS leaves them: nothing ported writes the rules'
/// horde setup (`x54`) or challenger's record (`x58`), the joining opponents' setup
/// (`gm.opponents`) or a player's leaving flag but a cutscene.
const MULTI_MAN_UNREACHED: &[(&str, &str)] = &[
    (
        "game mode {m:#x}'s states",
        "each mode's states (its select and its match: gm_Mode_10ManVs_States to gm_Mode_CruelVs_States) are in the scene machine, and it leaves only for them, the main menu (the select's B) or the challenger approach mode (0x14) after a won 100-Man Melee that earns Falco's challenge or a notice (gm_801B688C), which comes back to it (onExitVs@801BFBA8 after the challenger's match, onExitPrize after the notices)",
    ),
    (
        "game mode {m:#x}'s loading",
        "the six modes' load (gm_Mode_10ManVs_OnLoad) is in the scene machine, and so are the main menu's and the challenger approach mode's",
    ),
    (
        "the mode override",
        "the game installs one override, the boot's card check, ported",
    ),
    (
        "the VS scene's enter data in game mode",
        "the modes' matches enter with their own start data (gm_80495238), in the scene machine",
    ),
    (
        "game scene {k:#x}",
        "the mode's scenes are the one-door select and the VS scene, and the challenger approach mode's (the approach, the challenger's match, the notices' prize scene), each in the scene machine",
    ),
    (
        "Hook::Unported1P(name)",
        "the modes' states hook their selects' and matches' entries and exits (MultiManEnterCss, MultiManExitCss, MultiManEnterMatch, MultiManExitMatch), none unported",
    ),
    (
        "a 1P run's getter",
        "only All-Star's round setup asks a getter",
    ),
    (
        "the one-door screen's panel for match type",
        "the modes' selects are the Stadium's Multi-Man match types (0x11 to 0x16), whose panels are ported",
    ),
    (
        "mnCharSel_8025C020's records for match type",
        "the modes' selects show the Multi-Man records (match types 0x11 to 0x16), ported",
    ),
    (
        "HUD colour {c} past the table",
        "no caller passes a colour past the table's nine",
    ),
    (
        "a match's exit data in game mode",
        "the modes' matches exit into their own exit data (gm_80495370, match_exit_data's Multi-Man arm), and the challenger's match into the challenger approach mode's",
    ),
    (
        "gm_80167320's joiners' leaving",
        "the joining opponents' setup's flag (gm.opponents, gm_8016A1F8), which only gm_16A2's single-player setups and the adventure routes' arenas set",
    ),
    (
        "gm_801647F8 reading past lbl_803D5168",
        "only Target Test's match entry and select ask a character's target stage (gm_801647F8)",
    ),
    (
        "ftCo_800B2AFC for computer kind",
        "the hordes' computer kinds are GmKumite.dat's tables' 4, 16, 23, 24 and 25 (every table read off the disc), each ported, and the player is human",
    ),
    (
        "ftCo_800A8940 with no island picked",
        "its second pass takes every island its first did (it skips only ftCo_800A2718's test), so its running sum over the first's total reaches 1 by its last island, above every draw (HSD_Randf is under 1 - 2^-16): an island is always picked",
    ),
    (
        "ftCo_800AEFB8",
        "Nana's decision without Popo: the hordes are Wire Frames, and the player is human",
    ),
    (
        "ftCo_800AC434 at level 10",
        "the barrel cannon's coin flip divides by zero only at level 10: the tables' levels are 1 to 9",
    ),
];

/// Why Training reaches nothing its paths' units and modules hold unported but its gaps. Its
/// match is a VS scene on Training's own scene that the mode's own entry sets up
/// (`gm_801B1F70`: Training's entry of the modes table, `GmVsMode_Training`, which only
/// `gm_InitVsMode`'s defaults and Training's select write, with Training's flags and
/// `gm_80189CDC`'s rules; not `gm_16A2`'s single-player setups), so the single-player setups'
/// fields stay as VS leaves them: nothing ported writes the rules' horde setup (`x54`, so
/// `is_singleplayer` is 0), challenger's record (`x58`) or event Pokémon (`x28`), the joining
/// opponents' setup (`gm.opponents`) or a player's leaving flag but those setups and a
/// cutscene. Its match has no results screen (`gmresult`'s 1P lines, `gm_17AD`, `gm_17BA`)
/// and is no 1P run (game modes 3 to 5): its exit (`gm_801B2204`) goes back to the select or
/// to the notices.
const TRAINING_UNREACHED: &[(&str, &str)] = &[
    (
        "game mode {m:#x}'s states",
        "Training's states (gm_Mode_Training_States: the select, the stage select, the match) are in the scene machine, and it leaves only for them, the main menu (the select's B) or the challenger approach mode's notices (0x14, after a trophy its exit gives: gm_801B2204's gm_80173754 with no challenger), which go back to it (onExitPrize)",
    ),
    (
        "game mode {m:#x}'s loading",
        "Training's load (gm_Mode_Training_OnLoad) is in the scene machine, and so are the main menu's and the challenger approach mode's",
    ),
    (
        "the mode override",
        "the game installs one override, the boot's card check, ported",
    ),
    (
        "the VS scene's enter data in game mode",
        "Training's match enters with its own start data (gm_801B1F70's), in the scene machine",
    ),
    (
        "game scene {k:#x}",
        "Training's scenes are the one-door select, the stage select, its own match scene (GS_TRAINING) and the notices' prize scene, each in the scene machine with its exit",
    ),
    (
        "Hook::Unported1P(name)",
        "Training's states hook its select's, stage select's and match's entries and exits (TrainingEnterCss to TrainingExitVs), none unported",
    ),
    (
        "a 1P run's getter",
        "only All-Star's round setup asks a getter",
    ),
    (
        "the one-door screen's panel for match type",
        "Training's select is match type 0x17 (gm_801B1B74), whose panel is ported",
    ),
    (
        "mnCharSel_8025C020's records for match type",
        "Training's select shows the best combo (match type 0x17), ported",
    ),
    (
        "HUD colour {c} past the table",
        "no caller passes a colour past the table's nine",
    ),
    (
        "a match's exit data in game mode",
        "Training's match exits into its own exit data (match_exit_data's Training arm); the notices have no match",
    ),
    (
        "gm_80167320's joiners' leaving",
        "the joining opponents' setup's flag (gm.opponents, gm_8016A1F8), which only gm_16A2's single-player setups and the adventure routes' arenas set",
    ),
    (
        "gm_801647F8 reading past lbl_803D5168",
        "Target Test's match entry alone asks a character's target stage",
    ),
    (
        "it_8027B5B0 (an enemy of kind",
        "Training's enemies are Birdo and her eggs (Mushroom Kingdom II's), whose variables are typed: its menu offers no container, so no container's monster comes out, and it never makes the 1P coin (see the note on the 1P coin in Training below)",
    ),
    (
        "pl_8004049C (an enemy of kind",
        "Training's enemies are Birdo and her eggs, whose KO bonus is none, ported: no container's monster comes out in Training (its menu offers none) and it never makes the 1P coin (see the note on the 1P coin in Training below)",
    ),
    ("itMetamon_UnkMotion1", TRAINING_NO_DITTO),
    ("itMetamon_Logic19_EvtUnk", TRAINING_NO_DITTO),
    ("it_802D306C", TRAINING_NO_DITTO),
];

// Why Training never makes the 1P coin (milestone 6's task 13): its one maker there is a
// beaten enemy's trophy roll (`it_8027CC88`, the rest area's and the trophy points' being other
// modes'), and Training's one enemy, Mushroom Kingdom II's Birdo, never rolls. The roll runs
// on her sixth touch of the stage while she bounces away (state 10: `it_8027C79C` past the
// enemies' common data's `x0`, 5). She's hit only on her walk at her spawn's height
// (`grInishie2_801FD4F0` puts her beyond a blast zone at y 30.6, and states 0 to 8 keep her
// vertical speed 0: x -179.4 to -79.8 on the left, the mirror on the right). In state 10 she
// can't be hit again (her knock-off's `it_802756D0` sets `xD0C` to 2, which the fighters' and
// the items' hit checks skip; as hold kind 4 she has no appearance blink to end it; kind 233
// has no reflect, absorb or shield callbacks), and she flies straight at the common data's
// `x4` (5) with no gravity from the launch (`it_8027B964`): a hitbox's nine-bit angle (361 in
// the air ftCommonData's `x144`) or a hit with no knockback, either way she faces. melee-oracle's
// `birdo-sweep` runs that flight through melee-hd's own code on the stage's collision, the
// logs and the carpet where snapshots of the match have them: every angle 0 to 511 and the
// no-knockback hit, both facings, both sides, her x every 0.4 for the hits' angles (three
// snapshots) and every 2 for the rest (two): at most one touch, five short of the roll
// (docs/oracle.md, "Training's trophy roll").
//
// Training never makes the 1P coin: its one maker there, a beaten enemy's roll (it_8027CC88),
// needs Birdo's sixth touch bouncing away, and melee-oracle's birdo-sweep finds at most one over
// every launch angle, facing and place she's hit at (five short)

/// Why Training never lets Ditto out (milestone 6's task 13): its item menu offers item kinds
/// 5 to 0x22, no Pokémon; a Poké Ball's release (`it_8027AB64`) takes the event's Pokémon only
/// when the rules name one (`x28`, an event's), and in Training (`gm_8018841C`) draws from
/// Training's list (`common_pokemon`, `it_8027A780`) before the debug menu's choice is asked;
/// that draw takes the first entry whose running total passes the roll strictly, so Ditto's
/// weight of 0 in the ball's table (`ItCo.usd`) never comes out (docs/melee-bugs.md,
/// "Training's Pokémon list names Ditto, whom its draw can't pick").
const TRAINING_NO_DITTO: &str = "Ditto, whom Training's balls never let out: they draw from Training's list (it_8027AB64 under gm_8018841C, it_8027A780), where Ditto's weight of 0 (ItCo.usd) never passes the roll, and the item menu offers no Pokémon";

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
        // Not Event Match: the modes it enters (its own 0x2B with the one-door select and the
        // VS scene; 0x14 after a won event, for a trophy's notice or a challenger and its
        // match; the main menu) each have their states, loads and scenes (gm/machine.rs's
        // EVENT, CHALLENGER, MENU), checked by its sessions (docs/oracle.md); the
        // `Hook::Unported1P` hooks are Classic's, All-Star's and Adventure's coming soon.
        modes: &[CL, AS, AD, DB],
    },
    Gap {
        what: "the one-door character select",
        covers: &["mnCharSel_8025C020"],
        // Not Event Match: the records stop is for match types without a case, and Event
        // Match's (`EVENT_MATCH`, mn/charsel/one_door.rs) has one.
        modes: ONE_P_RUNS,
    },
    Gap {
        what: "the single-player matches' setup (teams, hordes, a leaving slot, the stage setups)",
        covers: &[
            "melee/gm/gm_17C0",
            "the VS scene's enter data in game mode",
            "a match's exit data in game mode",
            "a 1P run's ratio getter",
            "gm_8017D7AC's records after a no contest",
            "a single-player route's callback",
        ],
        // Not Event Match: gm_17C0's left are the 1P stage setups' match hooks and
        // gm_8017CBAC, gm_18A1's All-Star's ratio getters, none an event's (an event's match
        // takes its rules from gm_801BC00C, its clear is gm_801BC4F4 and no results screen
        // follows); the stops are the 1P runs' own (gm/one_player_match.rs, gm/opponents.rs's
        // routes) or a mode's whose enter and exit data Event Match has.
        modes: ONE_P_RUNS,
    },
    Gap {
        what: "the Kirbys' team's costumes (gm_80169540, read inline)",
        covers: &["gm_80169540"],
        // Only gmadventure.c calls it (the three Kirbys' cutscene and team, gm_801B4974 and
        // gm_801B4B28).
        modes: &[AD],
    },
    Gap {
        what: "a horde joining metal (fn_8016A450)",
        covers: &["fn_8016A450"],
        // Not Classic or Adventure (the intros' eighth round): only gm_8017CE34 calls it, for
        // a stage whose run flags (x8) hold both the horde's 8 and the metal 4, and no entry
        // of Classic's stage list (gmClassic_803DDEC8: the horde stage 0x08, the metal stage
        // 0x04) or Adventure's (gm_803DE650) holds both; All-Star's are its own task's.
        modes: &[AS],
    },
    Gap {
        what: "a horde's players leaving their slots",
        covers: &["gm_80167320's joiners' leaving"],
        // Not Event Match: a joiner leaves only through a crowd's end callback, and an event's
        // crowd has none (gm_8016A22C clears it; only a stage's arena, gm_801674C4, sets one),
        // so fn_80169444(2) answers 0 (Sarah's ev45, docs/oracle.md).
        modes: &[AD],
    },
    Gap {
        what: "the single-player matches' shared code (the stocks, the results, the clear screens)",
        covers: &["gm_8016F00C", "gm_GetCurrentSceneEnterData"],
        modes: SETUP_MODES,
    },
    Gap {
        what: "the 1P records and saves",
        covers: &[
            "gmMainLib_8015D1C8",
            "gmMainLib_8015D2F0",
            "gmMainLib_8015D418",
            "gm_801647F8",
        ],
        // Not Event Match: the gmMainLib getters are called only by gm_17C0.c's 1P stage
        // setups, fn_80160F58 only by Classic's intro (gm_1832.c), gm_801647F8 only by
        // gmclassic.c and gmmultiman.c; an event keeps its own records (gm_801BC4F4).
        modes: ONE_P_RUNS,
    },
    // The modes' own code.
    Gap {
        what: "Ditto",
        covers: &["melee/it/kinds/itmetamon"],
        // Not Event Match: Event 3 forces its Pokémon (0xB1), Event 15's list holds no Ditto
        // (0xB4), and every other event's balls draw from the weighted table, where Ditto
        // weighs 0 (ItCo.usd); only the debug menu's Pokémon setting picks him.
        modes: &[DB],
    },
    Gap {
        what: "a won 15-Minute Melee (ported; no session reaches it yet)",
        covers: &["gm_80173498", "gmMainLib_8015D780"],
        modes: &[MM],
    },
    Gap {
        what: "Event Match's list, events and egg",
        covers: &["melee/it/kinds/itevyoshiegg"],
        modes: &[EV],
    },
    Gap {
        what: "Classic's mode: Snag the Trophies without a coin, the units' held functions",
        covers: &["grFigureGet_80219898 with no coin", "melee/gm/gm_1832"],
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
            "ftMh_MS_371_801541C8",
            "Master Hand's function",
            "melee/ft/ftbosslib",
        ],
        modes: &[CL, EV, DB],
    },
    Gap {
        what: "All-Star's mode, its intro and the rest area",
        covers: &["a 1P run's getter"],
        modes: &[AS, DB],
    },
    Gap {
        what: "Adventure's mode and its intro",
        covers: &["melee/gm/gmadventure"],
        // Not Classic (the intros' eighth round): what gmadventure.c holds unported is hooks
        // of gm_Mode_Adventure_States alone (gm_801B50C4 and gm_801B518C, Giga Bowser's
        // path), whose only callers are that table, and Classic never enters game mode 4; it
        // named Classic while Adventure's intro stopped in gm/intro.rs, shared with Classic's.
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
            "stage kind {grkind}",
        ],
        modes: &[AD, DB],
    },
    Gap {
        what: "the adventure enemies and their spawner",
        covers: &[
            "melee/it/kinds/itlikelike",
            "melee/it/kinds/itnokonoko",
            "melee/it/kinds/itpatapata",
            "melee/it/kinds/itzgshell",
            "melee/it/kinds/itzrshell",
            "melee/it/kinds/itwhitebea",
            "melee/ft/ftCo_800C78B0",
            "fn_800C7890",
            "grZakoGenerator_801CAF08",
            "it_8027B5B0 (an enemy of kind",
            "pl_8004049C (an enemy of kind",
        ],
        modes: &[AD, DB],
    },
    Gap {
        what: "the cutscenes after the F-Zero one",
        covers: &["the cutscene scene"],
        // Not Adventure (the intros' eighth round): its PATHS entry's unreached says why.
        modes: &[DB],
    },
    Gap {
        what: "the cutscenes' fighters and stages",
        covers: &["demo setup", "ftDemo_CreateFighter's pose"],
        modes: &[AD, CL, AS, DB],
    },
    // The ends of a run.
    Gap {
        what: "the game overs' coming soon and the game over mode's Adventure load",
        covers: &["melee/gm/gm_19EF"],
        // Not Classic or Adventure (the intros' eighth round): gm_19EF's unported functions
        // are the coming soon scene's (GS_COMING_SOON), which only the modes' coming soon
        // states enter, Classic's 104 and Adventure's 0x68 (and All-Star's 0x68, and the debug
        // mode's state 8), and those two are entered only after a last state whose exit always
        // names the next mode (gmClassic_801B3F18 and gm_801B4408 in ledger/dead.txt say how).
        modes: &[AS, DB],
    },
    // The developer setting's debug routes.
    Gap {
        what: "All-Star's intro with a character past its place table (the debug menu's AllStar Enemy past its 28th choice)",
        covers: &["All-Star's intro's place of character"],
        modes: &[DB],
    },
    Gap {
        what: "the debug mode's milestone 6 states and the debug cutscene",
        covers: &[
            "onEnterIntroEasy",
            "un_80301BA8",
            "un_80301B48",
            "un_80301028",
            "un_80301074",
            "un_803010C0",
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
    /// and the modes whose gates are lifted are those the player offers (the Home-Run Contest,
    /// Target Test and Training, docs/player.md).
    #[test]
    fn modes_match_the_gates() {
        for m in Mode::ALL {
            assert_eq!(
                PATHS.iter().filter(|p| p.mode == m).count(),
                1,
                "{}",
                m.name()
            );
            let lifted = matches!(
                m,
                Mode::Debug | Mode::HomeRun | Mode::TargetTest | Mode::Training
            );
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
