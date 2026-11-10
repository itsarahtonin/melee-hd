//! The menus' audit (docs/player.md, "The menus the player offers"): every path a player can
//! reach from power-on through the game's own menus is ported, or the player gates it. Held to
//! the port ledger and the menus' modules as the stages', items', CPU players' and Special
//! Melee's audits are ([`super::roster`], [`super::items_audit`], [`super::cpu_audit`],
//! [`super::special_audit`]). Presentation data only: nothing in the simulation reads it.
//!
//! **Gates.** A gated entry is a menu list's entry whose confirm leads into unported code (the
//! single-player modes, until milestone 6). The player holds back the confirming buttons (A and
//! Start, every port) while the list's cursor is on one ([`gate`]), so the game runs as a
//! console's on which nobody confirmed there: no game code changes, and a session the player
//! records is still a console's.

use super::gm::machine::{GM_MENU, Phase};
use super::mn::main::{ONE_P, REG, STADIUM};
use super::state::Game;

/// A menu entry the player doesn't let a player confirm.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gate {
    /// The main menu's list (`MenuKind`) and its entry (the hovered selection).
    pub list: u8,
    pub entry: u16,
    /// What the entry is, as the menu names it.
    pub what: &'static str,
    /// The stops and the ledger's functions it keeps the game from reaching.
    pub covers: &'static [&'static str],
}

/// The main menu's gated entries: the single-player modes (milestone 6).
pub const GATES: &[Gate] = &[
    Gate {
        list: REG,
        entry: 0,
        what: "Classic",
        covers: &["melee/gm/gmclassic"],
    },
    Gate {
        list: REG,
        entry: 1,
        what: "Adventure",
        covers: &["melee/gm/gmadventure"],
    },
    Gate {
        list: REG,
        entry: 2,
        what: "All-Star",
        // Its own units are accounted for (task 23); what it still reaches unported is the
        // single-player modes' shared code (game::single_player_audit's gaps): the Very Hard
        // clear's record among them.
        covers: &["gmMainLib_8015D418"],
    },
    Gate {
        list: ONE_P,
        entry: 1,
        what: "Event Match",
        covers: &["melee/gm/gmevent"],
    },
    Gate {
        list: STADIUM,
        entry: 2,
        what: "Multi-Man Melee",
        // Nothing of the mode's own is missing (game::single_player_audit); kept until Sarah
        // decides on Master Hand as a player, whom the character select's name entry glitch
        // reaches in every select (docs/melee-bugs.md): his units are the audit's gaps for
        // Classic, Event Match and the debug routes.
        covers: &["Master Hand as a player"],
    },
];

/// The gate the game's state is at: the main menu's scene running with its list's cursor on a
/// gated entry.
pub fn gate(game: &Game) -> Option<&'static Gate> {
    let Phase::Scene { mode, .. } = game.machine.phase else {
        return None;
    };
    if mode != GM_MENU {
        return None;
    }
    let f = &game.menu.flow;
    GATES
        .iter()
        .find(|g| g.list == f.cur_menu && g.entry == f.hovered_selection)
}

/// The units the audit holds besides the menus' and the single-player modes' (`melee/gm`,
/// `melee/mn`, `melee/ty`, `melee/vi`): the libraries the menus' code calls into.
pub const LIB_UNITS: &[&str] = &[
    "melee/ft/ftdemo",
    "melee/if/if_2FFC",
    "melee/if/if_3004",
    "melee/if/soundtest",
    "melee/lb/lbarchive",
    "melee/lb/lbarq",
    "melee/lb/lbaudio_ax",
    "melee/lb/lbcardgame",
    "melee/lb/lbdvd",
    "melee/lb/lbfile",
    "melee/lb/lbheap",
    "melee/lb/lblanguage",
    "melee/lb/lbmemory",
    "melee/lb/lbmthp",
    "melee/lb/lbtime",
    "sysdolphin/baselib/axdriver",
];

/// Whether the audit holds `unit` (unless another audit does).
pub fn holds(unit: &str) -> bool {
    ["melee/gm/", "melee/mn/", "melee/ty/", "melee/vi/"]
        .iter()
        .any(|p| unit.starts_with(p))
        || LIB_UNITS.contains(&unit)
}

/// The game modules (`game/<module>.rs`) whose stops the audit holds: the menus', the modes',
/// the HUD's and the player library's.
pub const MODULES: &[&str] = &["gm/", "mn/", "ty/", "vi/", "hud/", "pl/"];

/// The stops in the modules the audit holds ([`MODULES`], those no other audit holds), each by
/// a piece of its text, with why no path a player can reach from power-on gets there. "Open:"
/// marks one a player can reach, with what's left to do.
pub const STOPS: &[(&str, &str)] = &[
    (
        "the cutscene scene",
        "a cutscene scene past the eleven (0x11 to 0x1B, all ported): the scene table names no other",
    ),
    (
        "mnDiagram2_GetStatValue for statistic",
        "the records' statistics are 0 to 23 (`stat`), every one handled: the pages list no other",
    ),
    (
        "mnDiagram2_GetAggregatedFighterRank for statistic",
        "called for the icon-only statistics alone (0x15 to 0x17, `is_icon_only`), each handled",
    ),
    (
        "PopulateRankings for a rank past the unlocked characters",
        "the rankings show five rows, and a save has at least the fourteen starting characters unlocked",
    ),
    (
        "mnInfoBonus's debug view",
        "drawn only at a debug level (ledger/deferred.txt, milestone 5's task 19)",
    ),
    (
        "an event's random fifth player",
        "no event's fifth player has a random character (GmEvent.dat's table: only the first four do)",
    ),
    (
        "the slot's image pointing at an album image",
        "the album loads its model anew, so slot A's joint's texture is the file's blank image when its setup reads it",
    ),
    (
        "Hook::Unported1P(name)",
        "the single-player modes', which only the entries behind GATES lead to: each mode's states past its select (gm::one_player), its own task's",
    ),
    (
        "the one-door screen's panel for match type",
        "every match type a one-door select is entered with (the 1P modes', the Stadium's, Training's and Event Match's) has its panel",
    ),
    (
        "mnCharSel_8025C020's records for match type",
        "every match type a one-door select is entered with (the 1P modes', the Stadium's, Training's and Event Match's) has its records",
    ),
    (
        "a 1P run's getter",
        "only All-Star's round setup (gm_801B5324, behind the All-Star gate) asks a getter, its costumes' (gm_8018A25C)",
    ),
    (
        "reading a freed coin's block the heap gave out again",
        "L, R and Start free the coins and the exit reads them in the same tick, with no allocation between (docs/melee-bugs.md, the character select's exit)",
    ),
    (
        "KO mark {other",
        "a display passes one of if_2F72's four KO mark callbacks, each ported",
    ),
    (
        "a tag frame past the table",
        "a player's port is 0 to 3 and its team 0 to 2 in a team battle (the character select's three colors), within the table's rows and columns",
    ),
    (
        "'s name with a % as the text's format",
        "the name entry's keyboards have no %, so only an edited save holds a name with one (the player imports saves the game loads whole, which doesn't test names)",
    ),
    (
        "a text on canvas {context} of font {font}, past the last",
        "each scene makes its texts on canvases it made first",
    ),
    (
        "two canvases of font {font} in a row",
        "no scene the menus reach makes two canvases of one font in a row (the sessions from power-on free every font's canvases with no stop)",
    ),
    (
        "gm_801647F8 reading past lbl_803D5168",
        "the one-door selects offer only the 25 characters the table lists (the Ice Climbers as Popo), which every caller passes",
    ),
    (
        "the VS scene's enter data in game mode",
        "a VS scene runs in VS mode, the Special Melee modes, Camera Mode, Tournament Melee and the attract match, each ported; the other modes with one are the single-player modes behind GATES",
    ),
    (
        "the mode override",
        "the game installs one override, the boot's card check (`bootOnLeave`'s `lbCardGame_DecideGameMode`), ported",
    ),
    (
        "game mode {m:#x}'s states",
        "every mode the menus reach is in the scene machine; the rest are the single-player modes behind GATES (their game overs and endings with them) and the debug modes (ledger/deferred.txt)",
    ),
    (
        "game mode {m:#x}'s loading",
        "every mode the menus reach is in the scene machine; the rest are the single-player modes behind GATES and the debug modes (ledger/deferred.txt)",
    ),
    (
        "game scene {k:#x}",
        "every scene the menus' modes run is in the scene machine; the rest are the single-player modes' and the debug modes'",
    ),
    (
        "unported_call(\"Toy_SetUnlockState\")",
        "the player's card holds Melee's files alone (melee-player's saves::read_dir), so the boot finds no Pikmin save",
    ),
    (
        "controller map hook",
        "the map's hook is the repeat (gm_801A3A74), which its reset installs before any evaluation; nothing installs another",
    ),
    (
        "a match's exit data in game mode",
        "a match exits in VS mode, the Special Melee modes, Camera Mode, Tournament Melee and the attract match, each ported; the rest are the single-player modes'",
    ),
    (
        "gm_SetupHumanResultsScreen for match kind",
        "a VS match's kind is its rules' mode, 0 to 3 (mn_803EC7DC's bounds)",
    ),
    (
        "fn_8016588C's bonus match score outside a tick",
        "only the Slippi recorder's copy of the results scores outside a tick, and the player records no Bonus match (melee-player's Platform::replay drops its recorder at the match's first frame)",
    ),
    (
        "after a flush that didn't wait",
        "the results screen's scene ends after its rounds, each of which copies a frame, so the flush at its loop's end always waits for the copy to show (only a scene that ends at once, `unk_C` 2, doesn't)",
    ),
    (
        "from Camera Mode's match exit, whose places all differ",
        "Camera Mode has no Sudden Death (gm_Mode_Camera_States), and gm_80166378's places break every tie by x30, whose low nibble is 6 - port, so every place is taken (results_tie's notes)",
    ),
    (
        "fn_80169000 moving handicaps",
        "only the last place's entry can be unset (place 0 is Sudden Death's one winner, so place 1 is taken), it moves only up and only from 8 or less, and every live cell's bytes are above 8 (results_tie's no_live_byte_can_move); main's kept registers move as modelled (MainRegs)",
    ),
    (
        "with no boot's report",
        "every session and the player boot from power-on, which reads the report's clock (game::session's boot); a replay's match has no results' exit",
    ),
    (
        "a name tag's name running past its block",
        "the name entry ends a four-letter name with x1A0's zero, so a typed name ends in its nine bytes; only an edited save runs on",
    ),
    (
        "a results label printed from a null string",
        "a statistics page lists no row for the player itself or an empty slot, the only ones whose label is null (every results session turns every page)",
    ),
    (
        "the results' statistic",
        "every getter the statistics pages list is handled (results_panel's pages)",
    ),
    (
        "gm_80167320's joiners' leaving",
        "a horde's first player falling with joiners in play: the single-player modes' hordes and the debug menu's arenas (the console halts first there), which only the entries behind GATES and the debug level lead to",
    ),
    (
        "a single-player route's callback",
        "the single-player modes' end callbacks but the arenas' two: the entries behind GATES lead to them",
    ),
    (
        "a 1P run's ratio getter",
        "Classic's and Adventure's runs' tables (All-Star's are ported): the single-player modes', which only the entries behind GATES lead to",
    ),
    (
        "gm_8017D7AC's records after a no contest",
        "a 1P run's match left by the pause's no contest or a retry: the single-player modes', which only the entries behind GATES lead to",
    ),
    (
        "HUD colour {c} past the table",
        "no caller passes a colour past the table's nine",
    ),
    (
        "gmMainLib_8015CE44 (the automatic handicap without a save)",
        "only the matches melee-sim sets up itself (replays, the player's command-line matches) enter without the save's handicaps, and their rules have none",
    ),
    (
        "GetPersistentNameData past the name tags",
        "a player's name tag is one of the save's 120 or none (0x78)",
    ),
    (
        "fn_80199AF0 with no winner's seat",
        "a box whose result shows has a winner's seat (the bracket's results)",
    ),
    (
        "fn_8019A158 with no winner's seat",
        "a box whose result shows has a winner's seat (the bracket's results)",
    ),
    (
        "fn_8018E85C past the entrants",
        "Tournament Melee seats every entrant, so each seat a box holds has one",
    ),
    (
        "fn_8018F00C on an empty name tag",
        "an entrant's name tag is one the selector showed, and nothing erases a name during a tournament",
    ),
    (
        "fn_8018F508 past the bracket",
        "the box being played is one of the bracket's 64",
    ),
    (
        "the character select's KO counts through a null pointer",
        "only the Hanyu select, a debug mode (ledger/deferred.txt), leaves the pointer null, and the player raises no debug level",
    ),
    (
        "the debug menu's Global Data Edit past the card data",
        "debug code, reached only at a raised debug level (milestone 5's task 19, the developer setting), which the player doesn't raise",
    ),
    (
        "All-Star's intro's place of character",
        "the debug menu's AllStar Enemy past its 28th choice (the Female Wire Frame on), whose intro reads past its place table: debug code, reached only at a raised debug level (milestone 5's task 19, the developer setting), which the player doesn't raise",
    ),
    (
        "the debug menu entry callback",
        "debug code, reached only at a raised debug level (milestone 5's task 19, the developer setting), which the player doesn't raise",
    ),
    (
        "unported(\"debug menu callback\"",
        "debug code, reached only at a raised debug level (milestone 5's task 19, the developer setting), which the player doesn't raise",
    ),
    (
        "gm_80164A0C (the Hanyu select's odd match types)",
        "debug code, reached only at a raised debug level (milestone 5's task 19, the developer setting), which the player doesn't raise",
    ),
    (
        "unported(\"debug pause predicate\"",
        "debug code, reached only at a raised debug level (milestone 5's task 19, the developer setting), which the player doesn't raise",
    ),
    (
        "the trophy unlock editor's count past its last class (_Toy_80310B48)",
        "debug code, reached only at a raised debug level (milestone 5's task 19, the developer setting), which the player doesn't raise; the original reads past the editor's block there (docs/melee-bugs.md)",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::roster::tests::{game_modules, ledger, stops};
    use std::collections::{BTreeMap, BTreeSet};

    /// The units and modules the other audits hold: the stages', the items', the CPU
    /// players', the characters' own items' and Special Melee's.
    fn held_elsewhere() -> (BTreeSet<String>, BTreeSet<String>) {
        let (mut units, mut modules) = crate::game::fighter_audit::tests::held();
        for s in crate::game::roster::STAGES {
            if let Some(a) = &s.audit {
                units.extend(a.units.iter().map(|u| (*u).to_owned()));
                modules.extend(a.modules.iter().map(|m| (*m).to_owned()));
            }
        }
        modules.extend(
            crate::game::roster::tests::SHARED_MODULES
                .iter()
                .map(|m| (*m).to_owned()),
        );
        for (u, m) in [
            (
                crate::game::items_audit::UNITS,
                crate::game::items_audit::MODULES,
            ),
            (
                crate::game::cpu_audit::UNITS,
                crate::game::cpu_audit::MODULES,
            ),
            (
                crate::game::special_audit::UNITS,
                crate::game::special_audit::MODULES,
            ),
        ] {
            units.extend(u.iter().map(|u| (*u).to_owned()));
            modules.extend(m.iter().map(|m| (*m).to_owned()));
        }
        (units, modules)
    }

    const TAB: char = '\t';

    /// `ledger/` lines of `NAME<TAB>...`: the names, with the rest of each line.
    fn ledger_file(name: &str) -> Vec<(String, String)> {
        let path = format!("{}/../../ledger/{name}", env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
        text.lines()
            .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
            .map(|l| {
                let (n, rest) = l.split_once(TAB).unwrap_or((l, ""));
                (n.to_owned(), rest.to_owned())
            })
            .collect()
    }

    /// Every path from power-on is held to the ledger and the code: every unaccounted function
    /// of a unit the audit holds (and no other audit does) is debug code
    /// (`ledger/deferred.txt`) or named, by itself or its unit, in `ledger/menus.txt` as gated,
    /// unreached with why, or ported awaiting a check; every stop in a module no other audit
    /// holds is in [`STOPS`] with why; and every name still names something unaccounted or a
    /// stop. So a port that lands, or a new stop, has to update the audit.
    #[test]
    fn menus_reach_nothing_unaccounted() {
        let ledger = ledger();
        let (units, modules) = held_elsewhere();
        let deferred: BTreeSet<String> = ledger_file("deferred.txt")
            .into_iter()
            .map(|e| e.0)
            .collect();
        let entries = ledger_file("menus.txt");
        let mut used = vec![false; entries.len()];
        let mut out = Vec::new();
        for e in &entries {
            let class = e.1.split(TAB).next().unwrap_or("");
            assert!(
                ["gated", "unreached", "unchecked"].contains(&class),
                "menus.txt: {} is {class:?}",
                e.0
            );
            assert!(
                e.1.split(TAB).nth(1).is_some_and(|w| !w.is_empty()),
                "menus.txt: {} says no why",
                e.0
            );
        }
        let mut missing: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (name, unit, status) in &ledger {
            if status != "unaccounted" || units.contains(unit) || !holds(unit) {
                continue;
            }
            if deferred.contains(name) {
                continue;
            }
            let mut hits: Vec<usize> = (0..entries.len())
                .filter(|&i| entries[i].0 == *name)
                .collect();
            if hits.is_empty() {
                hits = (0..entries.len())
                    .filter(|&i| entries[i].0 == *unit)
                    .collect();
            }
            if hits.is_empty() {
                missing.entry(unit.clone()).or_default().push(name.clone());
            }
            for i in hits {
                used[i] = true;
            }
        }
        for (u, names) in &missing {
            out.push(format!(
                "{u}: {} isn't ported, and ledger/menus.txt doesn't say why no path from power-on reaches it",
                names.join(" ")
            ));
        }
        let mut stops_used = vec![false; STOPS.len()];
        for m in game_modules()
            .iter()
            .filter(|m| !modules.contains(*m) && MODULES.iter().any(|p| m.starts_with(p)))
        {
            for stop in stops(m) {
                let hits: Vec<usize> = (0..STOPS.len())
                    .filter(|&i| stop.contains(STOPS[i].0))
                    .collect();
                if hits.is_empty() {
                    out.push(format!(
                        "game/{m}.rs stops, and the menus audit doesn't say why no path from power-on gets there: {stop}"
                    ));
                }
                for i in hits {
                    stops_used[i] = true;
                }
            }
        }
        for (i, e) in entries.iter().enumerate() {
            if !used[i] {
                out.push(format!(
                    "ledger/menus.txt names {}, which is no longer unaccounted",
                    e.0
                ));
            }
        }
        for ((k, _), used) in STOPS.iter().zip(stops_used) {
            if !used {
                out.push(format!("STOPS names {k:?}, which no longer stops"));
            }
        }
        assert!(out.is_empty(), "{}\n{} problems", out.join("\n"), out.len());
    }

    /// Each gate is an entry of its list that the menu shows (`mn_80229938`), and no two gates
    /// name the same entry.
    #[test]
    fn gates_are_shown_entries() {
        for (i, g) in GATES.iter().enumerate() {
            // All-Star shows once unlocked; the rest always.
            assert!(
                crate::game::mn::main::shown_with(0xFF, g.list, i32::from(g.entry)),
                "{}",
                g.what
            );
            for h in &GATES[..i] {
                assert!((h.list, h.entry) != (g.list, g.entry), "{}", g.what);
            }
        }
    }
}
