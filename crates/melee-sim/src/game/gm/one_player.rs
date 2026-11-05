//! The single-player modes' entries (the scene table's single-player rows): each mode's load
//! and its character select's mode states (`gmclassic.c`, `gmadventure.c`, `gmallstar.c`,
//! `gmmultiman.c`'s Target Test, `gmhomerun.c`, `gmtrainingmode.c`), with the 1P matches'
//! shared data the loads set up (`gm_17C0`, `gm_17DB`, `gm_17E4`, `gm_17EB`). The modes'
//! other states (their intros, matches, game overs) are their own tasks', and stop by name.

// The stage list's and the players' loops run by index, as the original's do, over tables
// indexed alongside.
#![allow(clippy::needless_range_loop)]

use super::super::handles::CodeRef;
use super::super::state::Running;
use super::super::structs::{StartMeleeData, VsModeData};
use super::machine::GM_MENU;
use super::vs_melee::{CssData, SssData};
use gekko_fp as fp;

/// The single-player `GameModeKind`s with a select.
pub const GM_CLASSIC: u8 = 0x03;
pub const GM_ADVENTURE: u8 = 0x04;
pub const GM_ALLSTAR: u8 = 0x05;
pub const GM_TARGET_TEST: u8 = 0x0F;
pub const GM_TRAINING: u8 = 0x1C;
pub const GM_HOME_RUN: u8 = 0x20;

/// The select's match types (`CSSMatchType`) the modes enter it with.
pub const REG_CLASSIC: u8 = 0xB;
pub const REG_ADVENTURE: u8 = 0xC;
pub const REG_ALLSTAR: u8 = 0xD;
pub const STADIUM_TARGET: u8 = 0xF;
pub const STADIUM_HOMERUN: u8 = 0x10;
pub const TRAINING_MODE: u8 = 0x17;

/// `ChKind_None`, `ChKind_Sandbag`, `CKind_Boy`, `CKind_Girl`, `CKind_GameWatch`.
const CHKIND_NONE: i8 = 0x21;

/// `lbl_803D5168`: each character's target stage (its stage kind, the character), ending in
/// a row of zeros.
const TARGET_STAGES: [(u8, u8); 28] = [
    (0x22, 0x00),
    (0x24, 0x01),
    (0x27, 0x02),
    (0x38, 0x03),
    (0x29, 0x04),
    (0x2A, 0x05),
    (0x2B, 0x06),
    (0x2C, 0x07),
    (0x21, 0x08),
    (0x2D, 0x09),
    (0x2E, 0x0A),
    (0x2F, 0x0B),
    (0x30, 0x0C),
    (0x32, 0x0D),
    (0x28, 0x0E),
    (0x33, 0x0F),
    (0x34, 0x10),
    (0x36, 0x11),
    (0x37, 0x12),
    (0x35, 0x13),
    (0x26, 0x14),
    (0x23, 0x15),
    (0x25, 0x16),
    (0x39, 0x17),
    (0x31, 0x18),
    (0x3A, 0x19),
    (0x48, 0x00),
    (0x00, 0x00),
];

/// A character's target stage, `gm_801647F8`: Popo's as the Ice Climbers'. The table's
/// stage kinds are bytes, so its end test (0x148) never holds: a character the table lacks
/// would read past it, which no select allows.
#[doc(alias = "gm_801647F8")]
pub fn target_stage(ckind: u8) -> u16 {
    let ckind = if ckind == 0x20 { 0xE } else { ckind };
    match TARGET_STAGES.iter().find(|e| e.1 == ckind) {
        Some(e) => u16::from(e.0),
        None => crate::game::code::unported_call(&format!(
            "gm_801647F8 reading past lbl_803D5168 for character {ckind:#x}"
        )),
    }
}

/// A character's best time in the session's records (`gmMainLib_8015D450`, `x4C[selkind]`),
/// whose last 21 entries the decomp's type calls padding.
#[doc(alias = "gmMainLib_8015D450")]
fn target_best(rec: &crate::game::structs::gmm_retval_EDBC, sel: u8) -> i32 {
    let i = usize::from(sel);
    if i < 4 {
        return rec.x4C[i];
    }
    let o = 4 * (i - 4);
    i32::from_be_bytes(rec.padding_x4C[o..o + 4].try_into().expect("four bytes"))
}

fn set_target_best(rec: &mut crate::game::structs::gmm_retval_EDBC, sel: u8, v: i32) {
    let i = usize::from(sel);
    if i < 4 {
        rec.x4C[i] = v;
        return;
    }
    let o = 4 * (i - 4);
    rec.padding_x4C[o..o + 4].copy_from_slice(&v.to_be_bytes());
}
const CHKIND_SANDBAG: i32 = 0x1F;

/// `gmPlayerData`: a 1P run's player.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct GmPlayerData {
    pub ckind: i8,
    pub color: u8,
    pub cpu_level: u8,
    pub slot: u8,
    pub nametag: u8,
    pub stocks: u8,
    pub x6: u8,
    pub mode: u8,
}

/// `Unk1PData_x24`: one of a 1P match's opponents.
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Opponent {
    pub ckind: i8,
    pub color: u8,
    pub cpu_level: u8,
    pub cpu_kind: u8,
    pub attack_ratio: f32,
    pub defense_ratio: f32,
}

/// `Unk1PData`: a 1P run's state (`gm_17C0`'s): its player, its counters (`xC` to `x23`),
/// the next match's opponents, and the mode's tables' getters (`x48` to `x70`, function
/// pointers: their addresses).
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Unk1PData {
    pub player: GmPlayerData,
    pub x8: [u8; 4],
    /// `xC` to `x13`.
    pub xc: [u8; 8],
    pub x14: u16,
    pub x18: i32,
    pub x1c: i32,
    pub x20: u32,
    pub opponents: [Opponent; 3],
    /// `x48`, `x4C`, `x50`, `x54`, `x58`, `x5C`, `x60`, `x64`, `x68`, `x6C`, `x70`.
    pub getters: [Option<CodeRef>; 11],
}

/// `UnkAllstarData` (`lbl_80472CB0`, Classic's; `gm_80473A18`, All-Star's).
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct AllStarData {
    pub base: Unk1PData,
    pub x74: u16,
    pub x76: [u8; 26],
    pub x90: [u8; 4],
    pub x94: [u8; 2],
    pub x96: [u8; 6],
    pub x9c: u32,
}

/// `UnkAdventureData` (`lbl_80472C30`).
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct AdventureData {
    pub base: Unk1PData,
    /// `x74` to `x78`.
    pub x74: [u8; 5],
    pub x79: [u8; 3],
    pub x7c: u8,
    pub x7d: [u8; 3],
}

/// Where a Classic stage list entry's matchup (`gm_803DDEC8Struct::xC`) points: one of the
/// matchup tables after `gm_Mode_Classic_States` (`gmClassic_803DDEC8`), or one of the
/// bonus stages' (`gm_804D4318`, `gm_804D4320`, `gm_804D4328`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub enum MatchupRef {
    #[default]
    None,
    /// A table (its offset in `gmClassic_803DDEC8Data`: 0xC0, 0xCC, 0x1B8, 0x26C, 0x2B0)
    /// and an entry.
    Table(u16, u8),
    /// A bonus stage's: 1 (`gm_804D4318`), 2, 3.
    Bonus(u8),
}

/// The single-player modes' statics.
#[derive(Clone, Debug, PartialEq, melee_derive::StateEq)]
pub struct OnePlayer {
    /// `gmClassic_80470708`: Classic's, Adventure's and All-Star's select data.
    pub css: CssData,
    /// `gm_80490960`: Target Test's select data, its match's start (`gm_80490AA8`) and its
    /// match's exit data (`gm_80490BE0`).
    pub target_css: CssData,
    pub target_start: StartMeleeData,
    pub target_exit: super::match_exit::MatchExitInfo,
    /// `css_data`, `sss_data` (`gmtrainingmode.c`): Training's selects' data.
    pub training_css: CssData,
    pub training_sss: SssData,
    /// The modes' 1P port and selects entered (`gm_804D68E8`/`E9`, Target Test's;
    /// `gm_804D68F8`/`F9`, Home-Run's; `gm_804D68C0`/`C1`, Training's).
    pub target_slot: u8,
    pub target_entries: u8,
    pub homerun_slot: u8,
    pub homerun_entries: u8,
    pub training_slot: u8,
    pub training_entries: u8,
    /// `lbl_80472CB0` (`gm_GetAllStarData`), Classic's run.
    pub classic: AllStarData,
    /// `gmClassicIntroDataBuffer` and `gm_804908A0`: Classic's intro data and its matchup
    /// orders (`x0C`, `x34`, `x54`, `x60`).
    pub classic_intro: [u8; 0x20],
    pub classic_order: [u8; 0x70],
    /// `gmClassic_803DDEC8.x00[i].xC`: each Classic stage's matchup.
    pub classic_picks: [MatchupRef; 12],
    /// `lbl_80472C30` (`gm_GetAdventureData`) and `gm_80490910`.
    pub adventure: AdventureData,
    pub adventure_x910: [u8; ADVENTURE_STAGES],
    /// `gm_80473A18`, All-Star's run, `gm_80490940`, and `gm_803DEBE8`'s opponents as the
    /// load shuffles them (stages, stage, character).
    pub allstar: AllStarData,
    pub allstar_x940: [u8; 25],
    pub allstar_opponents: [[u8; 4]; 25],
    /// `gmClassic_80472AF8` and `gmClassic_8047086C`: Classic's and All-Star's matches'
    /// enter and exit data.
    pub match_start: StartMeleeData,
    pub match_exit: super::match_exit::MatchExitInfo,
    /// `lbl_804706D8`: the match's won trophies (a trophy, -1 for none, and its count).
    pub match_trophies: [(i16, u8, u8); 12],
    /// `lbl_80472D28` and `lbl_804D65C0`: the stage clear screen's state (`gmregclear.c`;
    /// its `x104`, the new score, a 1P run's match hands back as its exit value).
    pub clear: super::clear::Clear,
    /// `gmClassic_80470850`: Classic's and All-Star's game over's enter and exit data.
    pub game_over: super::endings::game_over::GameOverData,
}

/// `ARRAY_SIZE(gm_803DE650)`, Adventure's stages.
pub const ADVENTURE_STAGES: usize = 0x2E;

impl Default for OnePlayer {
    /// The executable's data at power-on.
    fn default() -> Self {
        let css = CssData {
            ko_counts_null: true,
            ..Default::default()
        };
        OnePlayer {
            css: css.clone(),
            target_css: css.clone(),
            target_start: StartMeleeData::default(),
            target_exit: Default::default(),
            training_css: css,
            training_sss: SssData::default(),
            target_slot: 0,
            target_entries: 0,
            homerun_slot: 0,
            homerun_entries: 0,
            training_slot: 0,
            training_entries: 0,
            classic: AllStarData::default(),
            classic_intro: [0; 0x20],
            classic_order: [0; 0x70],
            classic_picks: [MatchupRef::None; 12],
            adventure: AdventureData::default(),
            adventure_x910: [0; ADVENTURE_STAGES],
            allstar: AllStarData::default(),
            allstar_x940: [0; 25],
            allstar_opponents: ALLSTAR_OPPONENTS,
            match_start: StartMeleeData::default(),
            match_exit: Default::default(),
            match_trophies: [(0, 0, 0); 12],
            clear: Default::default(),
            game_over: Default::default(),
        }
    }
}

/// `gm_803DEBE8`: All-Star's opponents in the executable's data (two stages, the stage
/// drawn, the character).
const ALLSTAR_OPPONENTS: [[u8; 4]; 25] = [
    [0xB1, 0xB1, 0, 8],
    [0xB2, 0xB2, 0, 1],
    [0xB3, 0xB3, 0, 6],
    [0xB4, 0xB4, 0, 0x10],
    [0xB5, 0xB5, 0, 0x11],
    [0xB6, 0xB6, 0, 4],
    [0xB7, 0xB7, 0, 2],
    [0xB8, 0xB8, 0, 0xD],
    [0xB9, 0xB9, 0, 7],
    [0xBA, 0xBA, 0, 0],
    [0xBB, 0xBB, 0, 0xB],
    [0xBC, 0xBC, 0, 0xF],
    [0xBD, 0xBD, 0, 0xE],
    [0xBE, 0xBE, 0, 0xC],
    [0xBF, 0xBF, 0, 0x12],
    [0xC0, 0xC0, 0, 9],
    [0xC1, 0xC1, 0, 0xA],
    [0xC2, 0xC2, 0, 5],
    [0xC3, 0xC3, 0, 0x16],
    [0xC4, 0xC4, 0, 0x15],
    [0xC5, 0xC5, 0, 0x14],
    [0xC6, 0xC6, 0, 0x18],
    [0xC7, 0xC7, 0, 0x17],
    [0xC9, 0xC9, 0, 0x19],
    [0xC8, 0xC8, 0, 3],
];

/// A Classic stage list entry (`gm_803DDEC8Struct`): its stage index, kind flags, bonus
/// kind, time and the rest, in the executable's data.
#[derive(Clone, Copy, Debug)]
pub struct ClassicStage {
    pub x0: u8,
    pub x1: u8,
    pub x2: u8,
    pub x4: u16,
    pub x6: u16,
    pub x8: u8,
}

const fn cs(x0: u8, x1: u8, x2: u8, x4: u16, x6: u16, x8: u8) -> ClassicStage {
    ClassicStage {
        x0,
        x1,
        x2,
        x4,
        x6,
        x8,
    }
}

/// `gmClassic_803DDEC8.x00`: Classic's twelve stages, the last the list's end (0xD).
pub const CLASSIC_STAGES: [ClassicStage; 12] = [
    cs(0x00, 0x00, 0, 300, 0, 2),
    cs(0x01, 0x10, 0, 300, 0, 4),
    cs(0x02, 0x80, 1, 120, 0, 4),
    cs(0x03, 0x00, 0, 300, 0, 2),
    cs(0x04, 0x02, 0, 300, 0, 4),
    cs(0x05, 0x80, 3, 120, 0, 4),
    cs(0x06, 0x00, 0, 300, 0, 2),
    cs(0x07, 0x08, 0, 300, 10, 4),
    cs(0x08, 0x80, 2, 180, 0, 4),
    cs(0x09, 0x04, 0, 300, 0, 2),
    cs(0x0a, 0x20, 0, 300, 0, 2),
    cs(0x0d, 0x00, 0, 0, 0, 0),
];

/// A Classic matchup (`gmClassicMatchup`): its stage kind and up to three opponents
/// (`ChKind_None` for none).
pub type Matchup = (u16, [i8; 3]);

const END: u16 = 0x148;

/// `gmClassic_803DDEC8Data.x0C0`: the last stage's (Master Hand's) matchup.
pub const CLASSIC_FINAL: [Matchup; 2] = [(176, [30, 26, 33]), (END, [0, 0, 0])];
/// `x0CC`: the one-opponent stages.
pub const CLASSIC_SINGLES: [Matchup; 39] = [
    (86, [8, 33, 33]),
    (87, [8, 33, 33]),
    (88, [1, 33, 33]),
    (89, [1, 33, 33]),
    (90, [6, 33, 33]),
    (91, [6, 33, 33]),
    (92, [16, 33, 33]),
    (93, [16, 33, 33]),
    (94, [17, 33, 33]),
    (95, [17, 33, 33]),
    (96, [4, 33, 33]),
    (97, [4, 33, 33]),
    (98, [2, 33, 33]),
    (99, [2, 33, 33]),
    (100, [13, 33, 33]),
    (101, [7, 33, 33]),
    (102, [7, 33, 33]),
    (103, [0, 33, 33]),
    (104, [0, 33, 33]),
    (105, [11, 33, 33]),
    (106, [11, 33, 33]),
    (107, [15, 33, 33]),
    (108, [5, 33, 33]),
    (109, [5, 33, 33]),
    (110, [12, 33, 33]),
    (111, [12, 33, 33]),
    (112, [18, 33, 33]),
    (113, [9, 33, 33]),
    (114, [10, 33, 33]),
    (115, [10, 33, 33]),
    (116, [14, 33, 33]),
    (117, [14, 33, 33]),
    (118, [22, 33, 33]),
    (119, [21, 33, 33]),
    (120, [21, 33, 33]),
    (121, [20, 33, 33]),
    (122, [20, 33, 33]),
    (124, [24, 33, 33]),
    (END, [0, 0, 0]),
];
/// `x1B8`: the team stages (an ally and an opponent pair).
pub const CLASSIC_TEAMS: [Matchup; 30] = [
    (125, [8, 5, 33]),
    (126, [8, 12, 33]),
    (127, [1, 2, 33]),
    (128, [6, 18, 33]),
    (129, [6, 21, 33]),
    (130, [6, 7, 33]),
    (131, [9, 6, 33]),
    (132, [16, 0, 33]),
    (133, [16, 2, 33]),
    (134, [17, 7, 33]),
    (135, [17, 11, 33]),
    (136, [4, 13, 33]),
    (137, [4, 24, 33]),
    (138, [4, 15, 33]),
    (139, [4, 14, 33]),
    (140, [2, 20, 33]),
    (141, [2, 0, 33]),
    (142, [13, 24, 33]),
    (143, [13, 15, 33]),
    (144, [7, 22, 33]),
    (145, [11, 12, 33]),
    (146, [11, 10, 33]),
    (147, [0, 20, 33]),
    (148, [5, 10, 33]),
    (149, [5, 12, 33]),
    (150, [5, 18, 33]),
    (151, [12, 18, 33]),
    (152, [18, 21, 33]),
    (153, [18, 9, 33]),
    (END, [0, 0, 0]),
];
/// `x26C`: the giant stages.
pub const CLASSIC_GIANTS: [Matchup; 11] = [
    (155, [8, 33, 33]),
    (156, [1, 33, 33]),
    (157, [6, 33, 33]),
    (158, [17, 33, 33]),
    (159, [7, 33, 33]),
    (160, [0, 33, 33]),
    (161, [15, 33, 33]),
    (162, [5, 33, 33]),
    (163, [22, 33, 33]),
    (164, [21, 33, 33]),
    (END, [0, 0, 0]),
];
/// `x2B0`: the horde stages (three of one character).
pub const CLASSIC_HORDES: [Matchup; 10] = [
    (165, [8, 8, 8]),
    (166, [1, 1, 1]),
    (167, [4, 4, 4]),
    (168, [7, 7, 7]),
    (169, [11, 11, 11]),
    (170, [15, 15, 15]),
    (174, [0, 0, 0]),
    (172, [24, 24, 24]),
    (173, [3, 3, 3]),
    (END, [0, 0, 0]),
];

/// The matchup tables by their offsets in `gmClassic_803DDEC8Data`, with their orders'
/// offsets in `gm_804908A0`.
pub const CLASSIC_TABLES: [(u16, &[Matchup], usize); 5] = [
    (0xC0, &CLASSIC_FINAL, 0),
    (0xCC, &CLASSIC_SINGLES, 0x0C),
    (0x1B8, &CLASSIC_TEAMS, 0x34),
    (0x26C, &CLASSIC_GIANTS, 0x54),
    (0x2B0, &CLASSIC_HORDES, 0x60),
];

/// The bonus stages' matchups: `gm_804D4318` (0x148: Target Test's, the character's own),
/// `gm_804D4320` (0x52), `gm_804D4328` (0x53).
pub const CLASSIC_BONUS: [u16; 3] = [0x148, 0x52, 0x53];

/// A matchup reference's stage kind and opponents.
pub fn matchup(r: MatchupRef) -> Option<Matchup> {
    match r {
        MatchupRef::None => None,
        MatchupRef::Table(t, i) => CLASSIC_TABLES
            .iter()
            .find(|e| e.0 == t)
            .map(|e| e.1[usize::from(i)]),
        MatchupRef::Bonus(k) => Some((CLASSIC_BONUS[usize::from(k) - 1], [CHKIND_NONE; 3])),
    }
}

/// The count of a matchup table's entries before its end (`gmClassic_GetMatchupCount`).
fn matchup_count(t: &[Matchup]) -> usize {
    t.iter().position(|m| m.0 == END).expect("a table's end")
}

/// Classic's 1P run's getters (`x48` to `x70`): `gm_8017EB3C` and the rest (`gm_17EB`).
const CLASSIC_GETTERS: [Option<CodeRef>; 11] = [
    Some(CodeRef(0x8017_EB3C)),
    Some(CodeRef(0x8017_EB64)),
    Some(CodeRef(0x8017_EBCC)),
    Some(CodeRef(0x8017_EB98)),
    Some(CodeRef(0x8017_ECA0)),
    Some(CodeRef(0x8017_ED08)),
    Some(CodeRef(0x8017_ECD4)),
    Some(CodeRef(0x8017_ED3C)),
    Some(CodeRef(0x8017_ED8C)),
    Some(CodeRef(0x8017_EC00)),
    Some(CodeRef(0x8017_EC50)),
];
/// Adventure's (`gm_17E4`).
const ADVENTURE_GETTERS: [Option<CodeRef>; 11] = [
    Some(CodeRef(0x8017_E500)),
    Some(CodeRef(0x8017_E5C8)),
    Some(CodeRef(0x8017_E630)),
    Some(CodeRef(0x8017_E5FC)),
    Some(CodeRef(0x8017_E704)),
    Some(CodeRef(0x8017_E76C)),
    Some(CodeRef(0x8017_E738)),
    Some(CodeRef(0x8017_E528)),
    Some(CodeRef(0x8017_E578)),
    Some(CodeRef(0x8017_E664)),
    Some(CodeRef(0x8017_E6B4)),
];
/// All-Star's (`gm_18A1`): `x58` null, `x5C`, `x60`, `x6C` and `x70` as the load left them.
const ALLSTAR_GETTERS: [(usize, Option<CodeRef>); 7] = [
    (0, Some(CodeRef(0x8018_A160))),
    (1, Some(CodeRef(0x8018_A228))),
    (2, Some(CodeRef(0x8018_A290))),
    (3, Some(CodeRef(0x8018_A25C))),
    (4, None),
    (7, Some(CodeRef(0x8018_A2C4))),
    (8, Some(CodeRef(0x8018_A314))),
];

impl Running<'_> {
    /// `gm_801677F0`: the port that confirmed the 1P list's entry.
    #[doc(alias = "gm_801677F0")]
    pub fn one_player_port(&self) -> u8 {
        self.game.menu.confirm_port as u8
    }

    /// The select data a single-player mode's select edits (its state's enter data).
    pub fn one_player_css(&self, mode: u8) -> Option<&CssData> {
        let o = &self.game.one_player;
        match mode {
            GM_CLASSIC | GM_ADVENTURE | GM_ALLSTAR => Some(&o.css),
            GM_TARGET_TEST => Some(&o.target_css),
            GM_TRAINING => Some(&o.training_css),
            GM_HOME_RUN => Some(&self.game.vs_melee.css),
            super::event::GM_EVENT => Some(&self.game.event.css),
            m if super::multi_man::is_multi_man(m) => Some(&self.game.multi_man.css),
            _ => None,
        }
    }

    pub fn one_player_css_mut(&mut self, mode: u8) -> Option<&mut CssData> {
        match mode {
            GM_CLASSIC | GM_ADVENTURE | GM_ALLSTAR => Some(&mut self.game.one_player.css),
            GM_TARGET_TEST => Some(&mut self.game.one_player.target_css),
            GM_TRAINING => Some(&mut self.game.one_player.training_css),
            GM_HOME_RUN => Some(&mut self.game.vs_melee.css),
            super::event::GM_EVENT => Some(&mut self.game.event.css),
            m if super::multi_man::is_multi_man(m) => Some(&mut self.game.multi_man.css),
            _ => None,
        }
    }

    /// The common part of the 1P runs' loads: the run cleared (`gm_8017C984`), its
    /// opponents' characters none (`gm_8017DB58`), the 1P port and the getters.
    #[doc(alias = "gm_8017C984")]
    #[doc(alias = "gm_8017DB58")]
    fn one_player_run_start(run: &mut Unk1PData, slot: u8) {
        *run = Unk1PData::default();
        for o in &mut run.opponents {
            o.ckind = CHKIND_NONE;
        }
        run.player.slot = slot;
    }

    /// `gm_Mode_Classic_OnLoad`: the matchup tables' picks cleared and their orders
    /// shuffled (the horde, giant, team and one-opponent tables'), the run started, the
    /// select first, and the trophies' save flags (`Ground_801C5A28`).
    #[doc(alias = "gm_Mode_Classic_OnLoad")]
    #[doc(alias = "gm_GetAllStarData")]
    #[doc(alias = "gmMainLib_8015CDC8")]
    pub fn classic_on_load(&mut self) {
        let o = &mut self.game.one_player;
        o.classic_picks = [MatchupRef::None; 12];
        // `gmClassic_InitMatchupOrder`: each order's entries in turn swapped with a random
        // one of the table's.
        for (_, t, at) in CLASSIC_TABLES[1..].iter().rev() {
            let count = matchup_count(t);
            for i in 0..count {
                self.game.one_player.classic_order[at + i] = i as u8;
            }
            for i in 0..count {
                let j = self.game.objs.rng.below(count as i32) as usize;
                let order = &mut self.game.one_player.classic_order;
                order.swap(at + i, at + j);
            }
        }
        let slot = self.one_player_port();
        let o = &mut self.game.one_player;
        Self::one_player_run_start(&mut o.classic.base, slot);
        o.classic_order[..12].fill(0);
        o.classic.base.getters = CLASSIC_GETTERS;
        self.set_game_mode_state_id(0x70);
        self.game.clear_trophy_notices();
        self.ground_load_trophies();
    }

    /// `Ground_801C5A28`: the stage load's trophy flags in the save, and the trophies'
    /// table synced (a 1P mode's load).
    #[doc(alias = "Ground_801C5A28")]
    fn ground_load_trophies(&mut self) {
        self.mark_class_two_trophies();
        self.toy_table_sync(false);
        self.toy_mark_class(0, false, true);
    }

    /// `gmClassic_801B3DD8`: Classic's select from its saved choices.
    #[doc(alias = "gmClassic_801B3DD8")]
    pub fn classic_enter_css(&mut self) {
        let s = self.game.game_data.vs.unk_51C.clone();
        let slot = self.game.one_player.classic.base.player.slot;
        super::setup_players::css_one_player(
            &mut self.game.one_player.css,
            REG_CLASSIC,
            s.c_kind,
            s.stocks as i8,
            s.color as i8,
            s.nametag,
            s.cpu_level,
            slot,
        );
        self.setup_vs_preload_cache();
    }

    /// `gmClassic_801B3E44`: back to the menu, or the choices saved and given to the run, the
    /// stages' matchups picked (`gmClassic_801B2D54`), the first stage next, and the
    /// announcer's bank (`gm_80168F88`).
    // Ports `gmClassic_801B3E44`; aliased once a check reaches it.
    pub fn classic_exit_css(&mut self) {
        if self.game.one_player.css.pending_scene_change == 2 {
            self.change_game_mode_after_scene(GM_MENU);
            return;
        }
        let x5 = self.save_one_player_choice(GM_CLASSIC);
        self.classic_pick_matchups();
        self.set_next_game_mode_state_id(x5 << 3);
        self.load_announcer_bank();
    }

    /// The select's choices saved in the mode's entry of the game's data
    /// (`gmMainLib_8015CDC8`, `8015CDD4`, `8015CDE0`) and given to its run; the entry's
    /// `x5`, the stage to start from.
    fn save_one_player_choice(&mut self, mode: u8) -> u8 {
        let (ckind, stocks, color, nametag, level) =
            super::setup_players::css_one_player_choice(&self.game.one_player.css);
        let d = &mut self.game.game_data.vs;
        let s = match mode {
            GM_CLASSIC => &mut d.unk_51C,
            GM_ADVENTURE => &mut d.unk_522,
            _ => &mut d.unk_528,
        };
        s.c_kind = ckind;
        s.stocks = stocks;
        s.color = color;
        s.nametag = nametag;
        s.cpu_level = level;
        let s = s.clone();
        let o = &mut self.game.one_player;
        let p = &mut match mode {
            GM_CLASSIC => &mut o.classic.base,
            GM_ADVENTURE => &mut o.adventure.base,
            _ => &mut o.allstar.base,
        }
        .player;
        p.ckind = s.c_kind;
        p.color = s.color;
        p.cpu_level = s.cpu_level;
        p.stocks = s.stocks;
        p.nametag = s.nametag;
        s.x5
    }

    /// `gm_80168F88`: the announcer's bank (bank 2's 0x10) wanted and waited for.
    #[doc(alias = "gm_80168F88")]
    pub fn load_announcer_bank(&mut self) {
        self.game.audio.lb.unwant_banks(0x12);
        self.game.audio.lb.want_banks(2, 0x10);
        self.game.audio.lb_start_bank_loads(self.data);
        self.game.audio.lb_wait_for_banks(self.data);
    }

    /// `gmClassic_801B2D54`: each Classic stage's matchup, picked from its table's order
    /// (`gmClassic_801B2BA4`): the hordes', the giants' and the teams' (but the last stage's),
    /// the one-opponent stages', then the bonus stages' and the last stage's own.
    // Ports `gmClassic_801B2D54`; aliased once a check reaches it.
    fn classic_pick_matchups(&mut self) {
        let n = CLASSIC_STAGES
            .iter()
            .position(|s| s.x0 == 0xD)
            .expect("the list's end");
        // Which stages each pass picks for, and from which table.
        type Pass = (fn(u8) -> bool, usize);
        let passes: [Pass; 4] = [
            (|f| f & 8 != 0, 4),
            (|f| f & 2 != 0 && f & 0x20 == 0, 3),
            (|f| f & 0x10 != 0 && f & 0x20 == 0, 2),
            (|f| f == 0 || f == 4, 1),
        ];
        for (wants, table) in passes {
            for i in 0..n {
                if wants(CLASSIC_STAGES[i].x1) {
                    let Some(r) = self.classic_pick(table) else {
                        // `for (;;) {}`: a console hangs.
                        panic!("gmClassic_801B2D54 found no matchup: a console hangs here");
                    };
                    self.game.one_player.classic_picks[i] = r;
                }
            }
        }
        for i in 0..n {
            let s = CLASSIC_STAGES[i];
            if s.x1 == 0x80 && (1..=3).contains(&s.x2) {
                self.game.one_player.classic_picks[i] = MatchupRef::Bonus(s.x2);
            }
        }
        if let Some(i) = (0..n).find(|&i| CLASSIC_STAGES[i].x1 & 0x20 != 0) {
            self.game.one_player.classic_picks[i] = MatchupRef::Table(0xC0, 0);
        }
    }

    /// `gmClassic_801B2BA4`: the first matchup in the table's order whose opponents the
    /// stage allows (its stage unlocked, each opponent unlocked, not the player's character,
    /// none already picked for another stage) and whose stage's ground no other pick shares;
    /// else the first that only shares a ground.
    // Ports `gmClassic_801B2BA4`; aliased once a check reaches it.
    fn classic_pick(&self, table: usize) -> Option<MatchupRef> {
        let (off, t, at) = CLASSIC_TABLES[table];
        let me = i32::from(self.game.game_data.vs.unk_51C.c_kind);
        let picks: Vec<Matchup> = self
            .game
            .one_player
            .classic_picks
            .iter()
            .take(11)
            .filter_map(|&r| matchup(r))
            .collect();
        let mut fallback = None;
        'outer: for outer in 0..matchup_count(t) {
            let idx = self.game.one_player.classic_order[at + outer];
            let entry = t[usize::from(idx)];
            for j in 0..3 {
                let c = i32::from(entry.1[j]);
                if c == i32::from(CHKIND_NONE) {
                    continue;
                }
                if !self.stage_unlocked(entry.0)
                    || !self.game.gm.unlocks.ckind_unlocked(c as u8)
                    || c == me
                    || picks.iter().any(|p| p.1.iter().any(|&k| i32::from(k) == c))
                {
                    continue 'outer;
                }
                let ground = crate::lb::audio::ground_kind(i32::from(entry.0));
                if picks
                    .iter()
                    .any(|p| crate::lb::audio::ground_kind(i32::from(p.0)) == ground)
                {
                    fallback = Some(MatchupRef::Table(off, idx));
                    continue 'outer;
                }
            }
            return Some(MatchupRef::Table(off, idx));
        }
        fallback
    }

    /// `gm_Mode_Adventure_OnLoad`: the run cleared and started, its stages' tries cleared,
    /// the select first, and the trophies' save flags.
    #[doc(alias = "gm_Mode_Adventure_OnLoad")]
    #[doc(alias = "gm_GetAdventureData")]
    #[doc(alias = "gmMainLib_8015CDD4")]
    pub fn adventure_on_load(&mut self) {
        let slot = self.one_player_port();
        let o = &mut self.game.one_player;
        o.adventure = AdventureData::default();
        Self::one_player_run_start(&mut o.adventure.base, slot);
        o.adventure_x910 = [0; ADVENTURE_STAGES];
        o.adventure.base.getters = ADVENTURE_GETTERS;
        // `ADVENTURE_BACK_TO_CSS`.
        self.set_game_mode_state_id(ADVENTURE_CSS);
        self.game.clear_trophy_notices();
        self.ground_load_trophies();
    }

    /// `gm_801B42E8`: Adventure's select from its saved choices.
    #[doc(alias = "gm_801B42E8")]
    pub fn adventure_enter_css(&mut self) {
        let s = self.game.game_data.vs.unk_522.clone();
        let slot = self.game.one_player.adventure.base.player.slot;
        super::setup_players::css_one_player(
            &mut self.game.one_player.css,
            REG_ADVENTURE,
            s.c_kind,
            s.stocks as i8,
            s.color as i8,
            s.nametag,
            s.cpu_level,
            slot,
        );
    }

    /// `gm_801B4350`: back to the menu, or the choices saved and given to the run, the first
    /// stage next and the announcer's bank.
    // Ports `gm_801B4350`; aliased once a check reaches it.
    pub fn adventure_exit_css(&mut self) {
        if self.game.one_player.css.pending_scene_change == 2 {
            self.change_game_mode_after_scene(GM_MENU);
            return;
        }
        let x5 = self.save_one_player_choice(GM_ADVENTURE);
        self.set_next_game_mode_state_id(x5 << 3);
        self.load_announcer_bank();
    }

    /// `gm_Mode_AllStar_OnLoad`: the run cleared and started, its opponents' stages drawn and
    /// their order shuffled, no characters met yet, the select first, and the trophies'
    /// save flags.
    #[doc(alias = "gm_Mode_AllStar_OnLoad")]
    #[doc(alias = "gmMainLib_8015CDE0")]
    pub fn allstar_on_load(&mut self) {
        let slot = self.one_player_port();
        let o = &mut self.game.one_player;
        Self::one_player_run_start(&mut o.allstar.base, slot);
        o.allstar_x940 = [0; 25];
        for (i, g) in ALLSTAR_GETTERS {
            o.allstar.base.getters[i] = g;
        }
        for i in 0..25 {
            let k = self.game.objs.rng.below(2) as usize;
            let op = &mut self.game.one_player.allstar_opponents[i];
            op[2] = op[k];
        }
        for i in 0..0x17 {
            let j = i + self.game.objs.rng.below(0x18 - i as i32) as usize;
            self.game.one_player.allstar_opponents.swap(i, j);
        }
        let a = &mut self.game.one_player.allstar;
        a.x74 = 0;
        a.x9c = 0;
        a.x76 = [CHKIND_NONE as u8; 26];
        a.x90 = [1; 4];
        self.set_game_mode_state_id(0x70);
        self.game.clear_trophy_notices();
        self.ground_load_trophies();
    }

    /// `gm_801B5F50`: All-Star's select from its saved choices.
    #[doc(alias = "gm_801B5F50")]
    pub fn allstar_enter_css(&mut self) {
        let s = self.game.game_data.vs.unk_528.clone();
        let slot = self.game.one_player.allstar.base.player.slot;
        super::setup_players::css_one_player(
            &mut self.game.one_player.css,
            REG_ALLSTAR,
            s.c_kind,
            s.stocks as i8,
            s.color as i8,
            s.nametag,
            s.cpu_level,
            slot,
        );
        self.setup_vs_preload_cache();
    }

    /// `gm_801B5FB4`: back to the menu, or the choices saved and given to the run, the first
    /// round next, the announcer's bank and the round's setup (`gm_801B5324`).
    #[doc(alias = "gm_801B5FB4")]
    pub fn allstar_exit_css(&mut self) {
        if self.game.one_player.css.pending_scene_change == 2 {
            self.change_game_mode_after_scene(GM_MENU);
            return;
        }
        let x5 = self.save_one_player_choice(GM_ALLSTAR);
        self.set_next_game_mode_state_id(x5.wrapping_mul(8) & 0xF8);
        self.load_announcer_bank();
        let round = self.game.game_data.vs.unk_528.x5;
        self.allstar_round_setup(usize::from(round));
    }

    /// A 1P run's `u8` getter (`Unk1PData::x48` to `x60`) for a round, level and index:
    /// All-Star's opponents' costumes (`gm_8018A25C`).
    fn one_player_getter_u8(&self, getter: Option<CodeRef>, round: u8, level: u8, i: u8) -> u8 {
        match getter {
            Some(CodeRef(0x8018_A25C)) => allstar_level(round, level).3[usize::from(i) * 3 + 1],
            other => crate::game::code::unported_call(&format!(
                "a 1P run's getter {:#010x}",
                other.map_or(0, |c| c.0)
            )),
        }
    }

    /// `gm_801B5324`: All-Star's round: its opponents (from the shuffled table), their
    /// costumes kept apart from the player's (`gmRegSetupEnemyColorTable`; the last round's
    /// Mr. Game & Watches in the table's), the game cache's player first and waited for, then
    /// the opponents and the stage, and their sound banks.
    #[doc(alias = "gm_801B5324")]
    pub fn allstar_round_setup(&mut self, round: usize) {
        let (start, count) = ALLSTAR_ROUNDS[round];
        let opponents = self.game.one_player.allstar_opponents;
        let mut chars = [CHKIND_NONE; 3];
        for i in 0..count {
            chars[i] = opponents[start + i][3] as i8;
        }
        let run = self.game.one_player.allstar.base;
        let mut colors = [0u8; 3];
        for (i, c) in colors.iter_mut().enumerate() {
            *c = self.one_player_getter_u8(
                run.getters[3],
                round as u8,
                run.player.cpu_level,
                i as u8,
            );
        }
        enemy_color_table(run.player.ckind, run.player.color, &chars, &mut colors);
        let last = round == 0xC;
        if last {
            chars = [CKIND_GAMEWATCH; 3];
            colors = [0; 3];
        }
        self.dvd_mut().keep_class();
        let gc = &mut self.dvd_mut().cache.scene.game_cache;
        gc.entries[0].char_id = i32::from(run.player.ckind);
        gc.entries[0].color = run.player.color;
        self.preload_refresh();
        self.wait_preload_class(0xC7);
        self.wait_preload_stale_released(4);
        let stage = opponents[start][2];
        let gc = &mut self.dvd_mut().cache.scene.game_cache;
        let mut slot = 1;
        for i in 0..3 {
            if chars[i] != CHKIND_NONE {
                gc.entries[slot].char_id = i32::from(chars[i]);
                gc.entries[slot].color = if last { 0xFF } else { colors[i] };
                slot += 1;
            }
        }
        gc.stkind = i32::from(stage);
        self.preload_refresh();
        let mut mask = crate::lb::audio::character_banks(i32::from(run.player.ckind));
        for c in chars {
            mask |= crate::lb::audio::character_banks(i32::from(c));
        }
        mask |= crate::lb::audio::stage_banks(i32::from(stage));
        self.game.audio.lb.unwant_banks(0x1C);
        self.game.audio.lb.want_banks(0xC, mask);
        self.game.audio.lb_start_bank_loads(self.data);
    }

    /// `gm_Mode_TargetTest_OnLoad`.
    #[doc(alias = "gm_Mode_TargetTest_OnLoad")]
    pub fn target_on_load(&mut self) {
        self.game.one_player.target_slot = self.one_player_port();
        self.game.one_player.target_entries = 0;
    }

    /// `gm_801B632C`: Target Test's select (the save written again after a run), its rules
    /// at their defaults, from the mode's saved choice.
    #[doc(alias = "gm_801B632C")]
    pub fn target_enter_css(&mut self) {
        if self.game.one_player.target_entries != 0 {
            self.save_after_run();
        }
        let t = self.game.game_data.vs.unk_530.unk_584.clone();
        let slot = self.game.one_player.target_slot;
        let css = &mut self.game.one_player.target_css;
        css.vs.start.rules = super::vs_mode::rules_defaults();
        super::setup_players::css_one_player(
            css,
            STADIUM_TARGET,
            t.unk_584,
            1,
            t.unk_585 as i8,
            t.unk_586,
            0,
            slot,
        );
        self.setup_vs_preload_cache();
        let o = &mut self.game.one_player;
        o.target_entries =
            super::results_exit::add_u8_saturating(u32::from(o.target_entries), 1) as u8;
    }

    /// `gm_801B63C4`: back to the menu, or the choice saved and the announcer's bank.
    #[doc(alias = "gm_801B63C4")]
    pub fn target_exit_css(&mut self) {
        let css = &self.game.one_player.target_css;
        if css.pending_scene_change == 2 {
            self.change_game_mode_after_scene(GM_MENU);
            return;
        }
        let (ckind, _, color, nametag, _) = super::setup_players::css_one_player_choice(css);
        let t = &mut self.game.game_data.vs.unk_530.unk_584;
        t.unk_584 = ckind;
        t.unk_585 = color;
        t.unk_586 = nametag;
        self.load_announcer_bank();
    }

    /// `gm_801B6428`: Target Test's match, from the select's rules with the mode's own: a
    /// timed match without a limit whose timer counts up and shows hours, the pause's
    /// single-player handler (`gm_80165290`), no items; the player alone at the mode's port
    /// with its saved character (Popo for the Ice Climbers), costume and name tag, one stock;
    /// the character's target stage, in the game cache too; and the rumble setting.
    #[doc(alias = "gm_801B6428")]
    pub fn target_enter_match(&mut self) {
        let t = self.game.game_data.vs.unk_530.unk_584.clone();
        let slot = self.game.one_player.target_slot;
        let (_, prefs) = super::attract::saved_rules(&self.game.game_data);
        let tags = self.nametag_rumbles();
        let o = &mut self.game.one_player;
        let s = &mut o.target_start;
        s.rules.clone_from(&o.target_css.vs.start.rules);
        let r = &mut s.rules;
        r.match_kind = 1;
        r.is_teams = 0;
        r.timer_enabled = true;
        r.time_limit = 0;
        r.timer_counts_up = true;
        r.timer_shows_hours = true;
        r.x4_2 = true;
        r.x2_5 = false;
        r.x5_1 = true;
        r.x4_3 = true;
        r.x4_4 = false;
        r.x3_1 = false;
        r.x3_3 = true;
        r.x3_2 = true;
        r.sd_penalty = -1;
        r.x18 = 0;
        r.x20 = 0;
        r.x3_7 = true;
        r.x9 = 1;
        r.item_freq = -1;
        r.on_pause_override = Some(CodeRef(0x8016_5290));
        super::vs_mode::all_player_defaults(&mut s.players);
        let ckind = if t.unk_584 == 14 {
            0x20
        } else {
            t.unk_584 as u8
        };
        super::setup_players::setup_human(&mut s.players[0], ckind, t.unk_585, 1, slot);
        s.players[0].nametag = t.unk_586;
        s.players[0].xC_b1 = false;
        let stkind = target_stage(s.players[0].ckind as u8);
        s.rules.stkind = stkind;
        self.dvd_mut().cache.scene.game_cache.stkind = i32::from(stkind);
        self.preload_refresh();
        let s = &mut self.game.one_player.target_start;
        super::vs_mode::load_rumble_enabled_with(s, &prefs, &tags);
    }

    /// `gm_801B65D4`: Target Test's match over. The match's seconds to the single-player time
    /// and its coins (`gm_80162968`, `gm_8016247C`); a retry plays the stage again. A cleared
    /// stage (no target left) takes the character's best time in the session's records and
    /// marks it cleared there; the save's record is the best time once cleared, else the most
    /// targets broken, and the first clear or a better time earns what it unlocks: Mr. Game &
    /// Watch, the stage 0x1C, the record trophies, the trophies and special messages. Then the
    /// challenger approach for a new challenger, else for a notice waiting, else the select.
    #[doc(alias = "gm_801B65D4")]
    pub fn target_exit_match(&mut self) {
        let end = self.game.one_player.target_exit.match_end.clone();
        let t = self.game.game_data.vs.unk_530.unk_584.clone();
        let slot = self.game.one_player.target_slot;
        let s = &mut self.game.game_data.thing.save_data;
        s.x1A3C = (s.x1A3C as u32).wrapping_add(end.frame_count / 60) as i32;
        self.results_add_coins(i32::from(end.player_standings[0].xE));
        if end.outcome == super::match_exit::OUTCOME_RETRY {
            self.set_next_game_mode_state_id(1);
            return;
        }
        let sel = super::match_exit::selkind(t.unk_584 as u8);
        let (left, total) = (
            i32::from(self.game.stage_info.x6D4),
            i32::from(self.game.stage_info.x6D2),
        );
        let frames = end.frame_count;
        if left == 0 {
            let bit = 1i32 << sel;
            let rec = &mut self.game.game_data.thing.save_data.unk_30;
            if rec.x8 & bit == 0 {
                set_target_best(rec, sel, frames as i32);
                rec.x8 |= bit;
            } else if target_best(rec, sel) as u32 > frames {
                set_target_best(rec, sel, frames as i32);
            }
            self.game.game_data.thing.save_data.unk_8.x1C |= bit;
            self.game.game_data.thing.save_data.unk_28.x4 |= bit;
        }
        let mut improved = false;
        let x7c = &mut self.game.game_data.thing.save_data.x1F2C[usize::from(sel)].x7C;
        if !x7c.b0 {
            if left == 0 {
                x7c.b0 = true;
                improved = true;
                x7c.x94 = frames;
            } else if x7c.x94 < total.wrapping_sub(left) as u32 {
                x7c.x94 = total.wrapping_sub(left) as u32;
            }
        } else if left == 0 {
            if x7c.x94 > frames {
                x7c.x94 = frames;
            }
            improved = true;
        }
        if improved {
            let challenger = self.target_challenger();
            let stage = self.target_stage_unlock();
            if stage != 0x148 {
                self.unlock_stage(stage);
            }
            self.target_trophies();
            self.trophy_unlocks();
            self.special_unlocks(0x80);
            if challenger != CHKIND_NONE as u8 {
                self.init_challenger(
                    t.unk_584 as u8,
                    t.unk_585,
                    slot,
                    t.unk_586,
                    challenger,
                    GM_TARGET_TEST,
                );
                self.set_pending_game_mode(super::machine::GM_CHALLENGER_APPROACH);
                self.set_new_game_mode_pending();
                return;
            }
        }
        if !self.challenger_check(GM_TARGET_TEST, slot) {
            self.set_next_game_mode_state_id(0);
        }
    }

    /// `gm_801733D8`: Mr. Game & Watch, while he's locked, once every other character has
    /// cleared one of the session's records' three modes ([`Self::all_cleared_one_mode`]).
    #[doc(alias = "gm_801733D8")]
    fn target_challenger(&mut self) -> u8 {
        self.game.sync_unlocks();
        if !self.game.gm.unlocks.ckind_unlocked(CKIND_GAMEWATCH as u8)
            && self.all_cleared_one_mode()
        {
            return CKIND_GAMEWATCH as u8;
        }
        CHKIND_NONE as u8
    }

    /// `fn_80172FAC`: whether every character but Mr. Game & Watch is marked in one of the
    /// session's records' three masks (`gmMainLib_8015D4A8`'s `x1C`, then `gmMainLib_8015D0F4`'s
    /// `x10`, then `gmMainLib_8015D21C`'s `x14`).
    #[doc(alias = "fn_80172FAC")]
    #[doc(alias = "gmMainLib_8015D4A8")]
    #[doc(alias = "gmMainLib_8015D0F4")]
    #[doc(alias = "gmMainLib_8015D21C")]
    pub(super) fn all_cleared_one_mode(&self) -> bool {
        let r = &self.game.game_data.thing.save_data.unk_8;
        [r.x1C, r.x10, r.x14]
            .into_iter()
            .any(|w| (0..0x19).all(|i| i == 3 || w & (1 << i) != 0))
    }

    /// `gm_8017341C`: the stage 0x1C, once every character's stage has been cleared
    /// (`gmMainLib_8015D5DC`) and while it's locked; 0x148 for none.
    #[doc(alias = "gm_8017341C")]
    #[doc(alias = "gmMainLib_8015D5DC")]
    fn target_stage_unlock(&self) -> u16 {
        let w = self.game.game_data.thing.save_data.unk_28.x4;
        if (0..0x19).all(|i| w & (1 << i) != 0) && !self.stage_unlocked(0x1C) {
            return 0x1C;
        }
        0x148
    }

    /// `gm_80173AA4`: once every character's stage has a best time in the session's records
    /// (`gmMainLib_8015D640`), trophy 0xBC, and by their sum 0x9B (under 0x15F90 frames) and
    /// 0x122 (under 0xAFC8).
    #[doc(alias = "gm_80173AA4")]
    #[doc(alias = "gmMainLib_8015D640")]
    fn target_trophies(&mut self) {
        let rec = &self.game.game_data.thing.save_data.unk_30;
        if !(0..0x19).all(|i| rec.x8 & (1 << i) != 0) {
            return;
        }
        let sum = (0..0x19u8).fold(0u32, |a, i| a.wrapping_add(target_best(rec, i) as u32));
        self.give_record_trophy(0xBC);
        if sum < 0x15F90 {
            self.give_record_trophy(0x9B);
        }
        if sum < 0xAFC8 {
            self.give_record_trophy(0x122);
        }
    }

    /// The save written after a run, as a Stadium mode's select's entry does
    /// (`lbCardNew_AllocWorkArea`, `lbCardGame_LoadArchive(0)`, `lbCardGame_SaveChanges`).
    fn save_after_run(&mut self) {
        self.card_alloc_work_area();
        self.card_game_load_archive(0);
        self.card_game_save_changes();
    }

    /// `gm_Mode_Homerun_OnLoad`.
    #[doc(alias = "gm_Mode_Homerun_OnLoad")]
    pub fn homerun_on_load(&mut self) {
        self.game.one_player.homerun_slot = self.one_player_port();
        self.game.one_player.homerun_entries = 0;
    }

    /// `gm_801B98E8`: the Home-Run Contest's select in VS mode's select data, from its own
    /// data, with Sandbag and the contest's stage in the game cache.
    #[doc(alias = "gm_801B98E8")]
    pub fn homerun_enter_css(&mut self) {
        if self.game.one_player.homerun_entries != 0 {
            self.save_after_run();
        }
        let p = self.game.modes.homerun.start.players[0].clone();
        let slot = self.game.one_player.homerun_slot;
        super::setup_players::css_one_player(
            &mut self.game.vs_melee.css,
            STADIUM_HOMERUN,
            p.ckind,
            1,
            p.color as i8,
            p.nametag,
            0,
            slot,
        );
        self.setup_vs_preload_cache();
        let gc = &mut self.dvd_mut().cache.scene.game_cache;
        gc.entries[1].char_id = CHKIND_SANDBAG;
        gc.entries[1].color = 0;
        gc.stkind = 0x54;
        self.preload_refresh();
        let o = &mut self.game.one_player;
        o.homerun_entries =
            super::results_exit::add_u8_saturating(u32::from(o.homerun_entries), 1) as u8;
    }

    /// `gm_801B999C`: back to the menu, or the player from the select and Sandbag.
    #[doc(alias = "gm_801B999C")]
    pub fn homerun_exit_css(&mut self) {
        if self.game.vs_melee.css.pending_scene_change == 2 {
            self.change_game_mode_after_scene(GM_MENU);
            return;
        }
        let (ckind, _, color, nametag, _) =
            super::setup_players::css_one_player_choice(&self.game.vs_melee.css);
        let vs = &mut self.game.modes.homerun;
        super::vs_mode::all_player_defaults(&mut vs.start.players);
        let p = &mut vs.start.players[0];
        p.ckind = ckind;
        p.color = color;
        p.nametag = nametag;
        let s = &mut vs.start.players[1];
        s.ckind = CHKIND_SANDBAG as i8;
        s.cpu_kind = 0xF;
        s.defense_ratio = 1.0;
        s.slot_type = 1;
        s.stocks = 1;
        s.team = 1;
    }

    /// `gm_Mode_Training_OnLoad`.
    #[doc(alias = "gm_Mode_Training_OnLoad")]
    pub fn training_on_load(&mut self) {
        self.game.one_player.training_slot = self.one_player_port();
        self.game.one_player.training_entries = 0;
    }

    /// The modes table's Training entry.
    fn training_table(&mut self) -> &mut VsModeData {
        &mut self.game.game_data.modes.table[6]
    }

    /// `gm_801B1B74`: Training's select from its table entry: its player and the CPU's
    /// partner door (level 9).
    #[doc(alias = "gm_801B1B74")]
    pub fn training_enter_css(&mut self) {
        if self.game.one_player.training_entries != 0 {
            self.save_after_run();
        }
        let players = self.training_table().start.players.clone();
        let slot = self.game.one_player.training_slot;
        let css = &mut self.game.one_player.training_css;
        let (p0, p1) = (&players[0], &players[1]);
        super::setup_players::css_one_player(
            css,
            TRAINING_MODE,
            p0.ckind,
            1,
            p0.color as i8,
            p0.nametag,
            9,
            slot,
        );
        super::setup_players::css_set_partner(
            css,
            p1.ckind,
            1,
            p1.color as i8,
            p1.nametag,
            0,
            slot,
        );
        self.setup_vs_preload_cache();
        let o = &mut self.game.one_player;
        o.training_entries =
            super::results_exit::add_u8_saturating(u32::from(o.training_entries), 1) as u8;
    }

    /// `gm_801B1C24`: back to the menu, or the player and the CPU from the select, the CPU
    /// copied to the other two slots in the next costumes (out of the match), their slots
    /// around the 1P port, the game cache's and the characters' banks.
    #[doc(alias = "gm_801B1C24")]
    pub fn training_exit_css(&mut self) {
        if self.game.one_player.training_css.pending_scene_change == 2 {
            self.change_game_mode_after_scene(GM_MENU);
            return;
        }
        let css = self.game.one_player.training_css.clone();
        let (ckind, _, color, nametag, _) = super::setup_players::css_one_player_choice(&css);
        let (pk, _, pc, pn, _) = super::setup_players::css_partner_choice(&css);
        let slot = self.game.one_player.training_slot;
        let vs = self.training_table();
        let ps = &mut vs.start.players;
        super::vs_mode::all_player_defaults(ps);
        ps[0].ckind = ckind;
        ps[0].color = color;
        ps[0].nametag = nametag;
        ps[1].ckind = pk;
        ps[1].color = pc as u8;
        ps[1].nametag = pn as u8;
        ps[1].cpu_kind = 0;
        for i in 2..4 {
            ps[i] = ps[1].clone();
            let n = crate::game::playback::costume_count(ps[i].ckind as u8);
            ps[i].color = (ps[i - 1].color + 1) % n;
            if ps[i].color == ps[0].color {
                ps[i].color = (ps[i].color + 1) % n;
            }
            ps[i].slot_type = 3;
        }
        if slot == 0 {
            for p in &mut ps[1..4] {
                p.slot = 0;
            }
        } else {
            let mut k = 1;
            for i in 0..4u8 {
                if slot != i {
                    ps[k].slot = i + 1;
                    k += 1;
                }
            }
        }
        let players = ps.clone();
        let gc = &mut self.dvd_mut().cache.scene.game_cache;
        for i in 2..4 {
            gc.entries[i].char_id = i32::from(players[i].ckind);
            gc.entries[i].color = players[i].color;
        }
        self.preload_refresh();
        let mut mask = 0u64;
        for p in &players[..4] {
            mask |= crate::lb::audio::character_banks(i32::from(p.ckind));
        }
        self.game.audio.lb.unwant_banks(20);
        self.game.audio.lb.want_banks(4, mask);
        self.game.audio.lb_start_bank_loads(self.data);
    }

    /// `gm_801B1EB8`: Training's stage select, any port, no stage forced.
    #[doc(alias = "gm_801B1EB8")]
    pub fn training_enter_sss(&mut self) {
        let s = &mut self.game.one_player.training_sss;
        s.x1 = 0;
        s.force_stage_id = -1;
        s.unk_stage = 0;
    }

    /// `gm_801B1EEC`: back to the select, or the stage kept for the match and its banks.
    #[doc(alias = "gm_801B1EEC")]
    pub fn training_exit_sss(&mut self) {
        let sss = &self.game.one_player.training_sss;
        if sss.start_game == 0 {
            self.set_next_game_mode_state_id(0);
            return;
        }
        let stkind = sss.vs.start.rules.stkind;
        self.game.modes.training_stage = stkind as i16;
        self.training_table().start.rules.stkind = stkind;
        self.game.audio.lb.unwant_banks(0x18);
        self.game
            .audio
            .lb
            .want_banks(8, crate::lb::audio::stage_banks(i32::from(stkind)));
        self.game.audio.lb_start_bank_loads(self.data);
    }
}

/// `CKind_GameWatch`.
const CKIND_GAMEWATCH: i8 = 3;

/// `gm_803DEC4C`: each All-Star round's first opponent in the shuffled table and their count.
pub(super) const ALLSTAR_ROUNDS: [(usize, usize); 13] = [
    (0, 1),
    (1, 1),
    (2, 1),
    (3, 1),
    (4, 2),
    (6, 2),
    (8, 2),
    (10, 2),
    (12, 3),
    (15, 3),
    (18, 3),
    (21, 3),
    (24, 1),
];

/// `gmTraining_ClassicStages` (`gm_18A1`): All-Star's rounds at each level (round x 5 +
/// level): a stage kind, two scales in hundredths, and three opponents' bytes (their
/// costumes the second of each three).
const ALLSTAR_LEVELS: [(u8, u16, u16, [u8; 10]); 65] = [
    (4, 50, 170, [0, 1, 23, 0, 0, 0, 0, 0, 0, 0]),
    (3, 70, 140, [1, 1, 23, 0, 0, 0, 0, 0, 0, 0]),
    (3, 80, 120, [3, 1, 23, 0, 0, 0, 0, 0, 0, 0]),
    (2, 90, 100, [5, 1, 23, 0, 0, 0, 0, 0, 0, 0]),
    (1, 100, 95, [7, 1, 23, 0, 0, 0, 0, 0, 0, 0]),
    (4, 50, 170, [1, 2, 4, 0, 0, 0, 0, 0, 0, 0]),
    (3, 70, 140, [3, 2, 4, 0, 0, 0, 0, 0, 0, 0]),
    (3, 80, 115, [4, 2, 4, 0, 0, 0, 0, 0, 0, 0]),
    (2, 90, 100, [6, 2, 4, 0, 0, 0, 0, 0, 0, 0]),
    (1, 100, 95, [8, 2, 4, 0, 0, 0, 0, 0, 0, 0]),
    (4, 50, 170, [2, 3, 4, 0, 0, 0, 0, 0, 0, 0]),
    (3, 70, 140, [4, 3, 4, 0, 0, 0, 0, 0, 0, 0]),
    (3, 80, 110, [5, 3, 4, 0, 0, 0, 0, 0, 0, 0]),
    (2, 90, 100, [7, 3, 4, 0, 0, 0, 0, 0, 0, 0]),
    (1, 100, 95, [9, 3, 4, 0, 0, 0, 0, 0, 0, 0]),
    (4, 50, 170, [3, 1, 4, 0, 0, 0, 0, 0, 0, 0]),
    (3, 70, 140, [5, 1, 4, 0, 0, 0, 0, 0, 0, 0]),
    (3, 80, 120, [6, 1, 4, 0, 0, 0, 0, 0, 0, 0]),
    (2, 90, 105, [8, 1, 4, 0, 0, 0, 0, 0, 0, 0]),
    (1, 100, 95, [9, 1, 4, 0, 0, 0, 0, 0, 0, 0]),
    (4, 50, 170, [0, 2, 23, 0, 2, 23, 0, 0, 0, 0]),
    (3, 67, 140, [2, 2, 23, 2, 2, 23, 0, 0, 0, 0]),
    (3, 78, 120, [3, 2, 23, 3, 2, 23, 0, 0, 0, 0]),
    (2, 90, 105, [5, 2, 4, 4, 2, 4, 0, 0, 0, 0]),
    (1, 100, 100, [6, 2, 4, 6, 2, 4, 0, 0, 0, 0]),
    (4, 50, 170, [1, 3, 23, 1, 3, 23, 0, 0, 0, 0]),
    (3, 67, 140, [3, 3, 4, 3, 3, 23, 0, 0, 0, 0]),
    (3, 78, 120, [4, 3, 4, 3, 3, 4, 0, 0, 0, 0]),
    (2, 90, 105, [5, 3, 4, 5, 3, 4, 0, 0, 0, 0]),
    (1, 100, 100, [7, 3, 4, 7, 3, 4, 0, 0, 0, 0]),
    (4, 50, 170, [2, 1, 23, 1, 1, 23, 0, 0, 0, 0]),
    (3, 70, 140, [3, 1, 4, 3, 1, 4, 0, 0, 0, 0]),
    (3, 78, 120, [4, 1, 4, 4, 1, 4, 0, 0, 0, 0]),
    (2, 90, 105, [6, 1, 4, 5, 1, 4, 0, 0, 0, 0]),
    (1, 102, 100, [8, 1, 4, 7, 1, 4, 0, 0, 0, 0]),
    (4, 50, 170, [2, 2, 4, 2, 2, 4, 0, 0, 0, 0]),
    (3, 70, 130, [3, 2, 4, 3, 2, 4, 0, 0, 0, 0]),
    (3, 80, 110, [5, 2, 4, 4, 2, 4, 0, 0, 0, 0]),
    (2, 90, 105, [7, 2, 4, 5, 2, 4, 0, 0, 0, 0]),
    (1, 105, 100, [9, 2, 4, 8, 2, 4, 0, 0, 0, 0]),
    (4, 50, 170, [0, 3, 23, 0, 3, 23, 0, 3, 23, 0]),
    (3, 67, 130, [2, 3, 23, 2, 3, 23, 1, 3, 23, 0]),
    (3, 78, 110, [3, 3, 23, 3, 3, 23, 3, 3, 23, 0]),
    (2, 89, 100, [4, 3, 4, 4, 3, 23, 4, 3, 23, 0]),
    (1, 100, 100, [6, 3, 4, 6, 3, 4, 6, 3, 23, 0]),
    (4, 50, 170, [1, 1, 23, 1, 1, 23, 1, 1, 23, 0]),
    (3, 69, 125, [2, 1, 23, 2, 1, 23, 2, 1, 23, 0]),
    (3, 75, 110, [4, 1, 17, 4, 1, 23, 3, 1, 23, 0]),
    (2, 89, 100, [5, 1, 4, 4, 1, 17, 4, 1, 23, 0]),
    (1, 100, 100, [7, 1, 4, 7, 1, 4, 7, 1, 17, 0]),
    (4, 50, 170, [1, 2, 23, 1, 2, 23, 1, 2, 23, 0]),
    (3, 70, 125, [3, 2, 17, 3, 2, 23, 3, 2, 23, 0]),
    (3, 79, 112, [4, 2, 4, 4, 2, 17, 4, 2, 23, 0]),
    (2, 90, 100, [5, 2, 4, 5, 2, 4, 4, 2, 17, 0]),
    (1, 100, 100, [8, 2, 4, 8, 2, 4, 7, 2, 4, 0]),
    (4, 50, 170, [2, 3, 23, 2, 3, 23, 1, 3, 23, 0]),
    (3, 70, 125, [4, 3, 17, 4, 3, 4, 3, 3, 23, 0]),
    (3, 80, 110, [5, 3, 4, 4, 3, 17, 4, 3, 23, 0]),
    (2, 92, 100, [6, 3, 4, 5, 3, 4, 5, 3, 17, 0]),
    (1, 105, 96, [9, 3, 4, 8, 3, 4, 8, 3, 17, 0]),
    (4, 20, 360, [1, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
    (3, 30, 320, [2, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
    (3, 40, 300, [3, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
    (2, 48, 240, [5, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
    (1, 55, 200, [7, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
];

pub(super) fn allstar_level(round: u8, level: u8) -> (u8, u16, u16, [u8; 10]) {
    ALLSTAR_LEVELS[usize::from(level) + usize::from(round) * 5]
}

/// `gmRegSetupEnemyColorTable`: an opponent in the player's character and costume gets the
/// first other costume no other opponent wears; a console halts on an assertion if none is
/// left.
#[doc(alias = "gmRegSetupEnemyColorTable")]
#[doc(alias = "gm_8018A25C")]
pub fn enemy_color_table(ckind: i8, color: u8, ckinds: &[i8; 3], colors: &mut [u8; 3]) {
    let n = crate::game::playback::costume_count(ckind as u8);
    for i in 0..3 {
        if ckinds[i] != CHKIND_NONE && ckind == ckinds[i] && color == colors[i] {
            colors[i] = 0xFF;
        }
    }
    for i in 0..3 {
        if ckinds[i] == CHKIND_NONE || colors[i] != 0xFF {
            continue;
        }
        for j in 0..n {
            if color == j {
                continue;
            }
            let free = (0..3).all(|k| i == k || ckinds[k] == CHKIND_NONE || colors[k] != j);
            if free {
                colors[i] = j;
                break;
            }
        }
        assert!(
            colors[i] != 0xFF,
            "gmRegSetupEnemyColorTable found no costume: a console halts on its assertion"
        );
    }
}

/// Adventure's select state (`ADVENTURE_BACK_TO_CSS`).
pub const ADVENTURE_CSS: u8 = 0x70;

impl super::super::state::Game {
    /// A character's 1P records (`GetPersistentFighterData(hud)->x7C`).
    pub fn record_1p(&self, hud: u8) -> &crate::game::structs::FighterData_x7C {
        &self.game_data.thing.save_data.x1F2C[usize::from(hud)].x7C
    }

    /// A character's high score in Classic, Adventure or All-Star (`mode` 0 to 2):
    /// `gm_80162BD8`, `gm_80162DD4`, `gm_80162FD0` (`gmMainLib_8015D0C0`, `8015D1E8`,
    /// `8015D310`).
    #[doc(alias = "gm_80162BD8")]
    #[doc(alias = "gm_80162DD4")]
    #[doc(alias = "gm_80162FD0")]
    #[doc(alias = "gmMainLib_8015D0C0")]
    #[doc(alias = "gmMainLib_8015D1E8")]
    #[doc(alias = "gmMainLib_8015D310")]
    pub fn high_score_1p(&self, mode: usize, hud: u8) -> i32 {
        let r = self.record_1p(hud);
        [r.x88, r.x8C, r.x90][mode]
    }

    /// Every character's high scores summed (`gm_80162C48`, `gm_80162E44`, `gm_80163040`):
    /// the test meant to hold the sum at the most a word holds can't fire, so it wraps
    /// ([Melee's bugs](../../../../../docs/melee-bugs.md)).
    #[doc(alias = "gm_80162C48")]
    #[doc(alias = "gm_80162E44")]
    #[doc(alias = "gm_80163040")]
    pub fn high_score_1p_total(&self, mode: usize) -> i32 {
        (0..25u8).fold(0i32, |s, i| s.wrapping_add(self.high_score_1p(mode, i)))
    }

    /// Whether every character has cleared the mode (`gm_80162D1C` and its copies' tests).
    pub fn cleared_1p_all(&self, mode: usize) -> bool {
        (0..25u8).all(|i| {
            let r = self.record_1p(i);
            [r.b4, r.b5, r.b6][mode]
        })
    }

    /// The lowest level every character cleared the mode on, at most 4 (`gm_80162D6C`,
    /// `gm_80162F68`, `gm_80163164`).
    #[doc(alias = "gm_80162D6C")]
    #[doc(alias = "gm_80162F68")]
    #[doc(alias = "gm_80163164")]
    #[doc(alias = "gmMainLib_8015D3E4")]
    pub fn cleared_1p_worst(&self, mode: usize) -> u8 {
        (0..25u8).fold(4, |m, i| {
            let r = self.record_1p(i);
            m.min([r.x80, r.x81, r.x82][mode])
        })
    }

    /// Target Test's record: the targets broken until the stage is cleared, then -1
    /// (`gm_8016332C`, `gmMainLib_8015D438`, `gmMainLib_8015D48C`).
    #[doc(alias = "gm_8016332C")]
    #[doc(alias = "gmMainLib_8015D438")]
    #[doc(alias = "gmMainLib_8015D48C")]
    pub fn target_record(&self, hud: u8) -> i32 {
        let r = self.record_1p(hud);
        if !r.b0 { r.x94 as i32 } else { -1 }
    }

    /// Frames as hours, minutes, seconds and hundredths (`(99 * (frames % 60)) / 59`).
    fn frames_to_time(frames: u32) -> [u8; 4] {
        let s = (frames / 60) as i32;
        let cs = fp::fctiwz(fp::fdivs(
            fp::fmuls(99.0, fp::frsp(f64::from(frames % 60))),
            59.0,
        ));
        [
            (s / 60 / 60) as u8,
            (s / 60 % 60) as u8,
            (s % 60) as u8,
            cs as u8,
        ]
    }

    /// A cleared Target Test's best time (`gm_80163374`; called only for a cleared stage,
    /// whose outputs it writes).
    #[doc(alias = "gm_80163374")]
    pub fn target_time(&self, hud: u8) -> [u8; 4] {
        let r = self.record_1p(hud);
        assert!(
            r.b0,
            "gm_80163374 for a stage not cleared leaves its outputs unwritten"
        );
        Self::frames_to_time(r.x94)
    }

    /// Every character's best times summed, once every stage is cleared (`gm_801634D4`).
    #[doc(alias = "gm_801634D4")]
    pub fn target_time_total(&self) -> Option<[u8; 4]> {
        let mut total = 0u32;
        for i in 0..25u8 {
            let r = self.record_1p(i);
            if !r.b0 {
                return None;
            }
            total = total.wrapping_add(r.x94);
        }
        Some(Self::frames_to_time(total))
    }

    /// The Home-Run Contest's best distance (`gm_801631CC`, `gmMainLib_8015D06C`).
    #[doc(alias = "gm_801631CC")]
    #[doc(alias = "gmMainLib_8015D06C")]
    pub fn homerun_record(&self, hud: u8) -> u32 {
        self.record_1p(hud).x84 as u32
    }

    /// Every character's best distances summed, wrapping as the scores' totals do
    /// (`gm_801631F0`).
    #[doc(alias = "gm_801631F0")]
    pub fn homerun_record_total(&self) -> u32 {
        (0..25u8).fold(0u32, |s, i| s.wrapping_add(self.homerun_record(i)))
    }

    /// Whether the character has won 10-Man (or 100-Man) Melee (`gm_8016365C`,
    /// `gm_801639C0`, negated).
    #[doc(alias = "gm_8016365C")]
    #[doc(alias = "gm_801639C0")]
    pub fn multi_man_won(&self, hundred: bool, hud: u8) -> bool {
        let r = self.record_1p(hud);
        if hundred { r.b2 } else { r.b1 }
    }

    /// 10-Man's (or 100-Man's) best KOs without a win, -1 with one (`gm_80163690`,
    /// `gm_801639F4`).
    #[doc(alias = "gm_80163690")]
    #[doc(alias = "gm_801639F4")]
    pub fn multi_man_best(&self, hundred: bool, hud: u8) -> i32 {
        let r = self.record_1p(hud);
        if self.multi_man_won(hundred, hud) {
            -1
        } else if hundred {
            r.x9C
        } else {
            r.x98
        }
    }

    /// A won 10-Man's (or 100-Man's) best time (`gm_801636D8`, `gm_80163A3C`; called only
    /// for a win, whose outputs they write). `gm_80163A3C`, 100-Man's, is aliased once a check
    /// reaches it (a select after a won 100-Man Melee).
    #[doc(alias = "gm_801636D8")]
    pub fn multi_man_time(&self, hundred: bool, hud: u8) -> [u8; 4] {
        assert!(
            self.multi_man_won(hundred, hud),
            "gm_801636D8 without a win leaves its outputs unwritten"
        );
        let r = self.record_1p(hud);
        Self::frames_to_time(if hundred { r.x9C } else { r.x98 } as u32)
    }

    /// Every character's best 10-Man (or 100-Man) times summed, once every one has won
    /// (`gm_80163838`, `gm_80163B9C`).
    #[doc(alias = "gm_80163838")]
    #[doc(alias = "gm_80163B9C")]
    pub fn multi_man_time_total(&self, hundred: bool) -> Option<[u8; 4]> {
        let mut total = 0u32;
        for i in 0..25u8 {
            if !self.multi_man_won(hundred, i) {
                return None;
            }
            let r = self.record_1p(i);
            total = total.wrapping_add(if hundred { r.x9C } else { r.x98 } as u32);
        }
        Some(Self::frames_to_time(total))
    }

    /// The best KOs of 3-Minute, 15-Minute, Endless and Cruel Melee (`mode` 0 to 3).
    #[doc(alias = "gm_Get3MinMultimanHighscore")]
    #[doc(alias = "gm_Get15MinMultimanHighscore")]
    #[doc(alias = "gm_GetEndlessHighscore")]
    #[doc(alias = "gm_GetCruelHighscore")]
    pub fn multi_man_kos(&self, mode: u8, hud: u8) -> i32 {
        let r = self.record_1p(hud);
        match mode {
            0 => i32::from(r.xA0),
            1 => i32::from(r.xA2),
            2 => r.xA4,
            _ => r.xA8,
        }
    }

    /// Every character's best KOs of a mode summed, wrapping.
    #[doc(alias = "gm_Get3MinMultimanTotalHighscore")]
    #[doc(alias = "gm_Get15MinMultimanTotalHighscore")]
    #[doc(alias = "gm_GetEndlessTotalHighscore")]
    #[doc(alias = "gm_GetCruelTotalHighscore")]
    pub fn multi_man_kos_total(&self, mode: u8) -> i32 {
        (0..25u8).fold(0i32, |s, i| s.wrapping_add(self.multi_man_kos(mode, i)))
    }

    /// Training's best combo (`gm_80163274`, `gmMainLib_8015D7EC`).
    #[doc(alias = "gm_80163274")]
    #[doc(alias = "gmMainLib_8015D7EC")]
    pub fn training_record(&self, hud: u8) -> u16 {
        self.record_1p(hud).x7E
    }
}
