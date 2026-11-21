//! Tournament mode (mode 0x1B: `gmtoumode.c`, `gmtoulib.c`, `gmtou_0.c` to `gmtou_2.c`).
//!
//! The setup screen (`gmtou_0.c`) takes the match type (a bracket, winner-out or loser-out),
//! the players a match, the entrants, the humans among them, the stage selection and the
//! computers' level, then the entrants' list: each one's character, color, name and handicap.
//! A bracket tournament goes on to the bracket (`gmtou_1.c`): up to 64 entrants in matches of
//! two to four, the next match's players ready up, play it, and its winner moves on along the
//! bracket's lines (`gmtoulib.c` draws them). Winner-out and loser-out go to the match-up
//! screen (`gmtou_2.c`), where the winner (or the loser) stays for the next match. The mode's
//! states (`gmtoumode.c`) run the stage select when the stage is chosen, the match, Sudden
//! Death and the results between.

pub mod alt;
pub mod bracket;
pub mod lib;
pub mod setup;

use melee_data::archive::{ArchiveId, DataRef};

use super::super::handles::CodeRef;
use super::super::state::Running;
use super::super::structs::{MatchEnd, StartMeleeData};
use crate::hsd::cobj::CObjId;
use crate::hsd::gobj::GObjId;
use crate::hsd::sis::TextId;
use crate::vec::Vec3;

/// `GM_TOURNAMENT`.
pub const GM_TOURNAMENT: u8 = 0x1B;

/// `lbl_80473AB8`, the bracket's boxes' address (their GObjs' user data).
pub const BRACKET_ADDR: u32 = 0x8047_3AB8;

/// `GS_TOU_SETUP`, `GS_TOU_BRACKET`, `GS_TOU_ALT` (36 to 38 in `GameSceneKind`; 0x18 to 0x1B
/// are the last four cutscenes').
pub const GS_TOU_SETUP: u8 = 0x24;
pub const GS_TOU_BRACKET: u8 = 0x25;
pub const GS_TOU_ALT: u8 = 0x26;

/// The three scenes' frame functions and entries.
pub const SETUP_ON_FRAME: CodeRef = CodeRef(0x8019_628C);
pub const SETUP_ON_ENTER: CodeRef = CodeRef(0x8019_63B4);
pub const BRACKET_ON_FRAME: CodeRef = CodeRef(0x8019_B2DC);
pub const BRACKET_ON_ENTER: CodeRef = CodeRef(0x8019_B8C4);
pub const ALT_ON_FRAME: CodeRef = CodeRef(0x8019_DF8C);
pub const ALT_ON_ENTER: CodeRef = CodeRef(0x8019_ECAC);

/// The mode's states' ids (`gm_Mode_Tournament_States`).
pub const STATE_SETUP: u8 = 0;
pub const STATE_BRACKET: u8 = 1;
pub const STATE_ALT: u8 = 2;
pub const STATE_SSS: u8 = 3;
pub const STATE_VS: u8 = 4;
pub const STATE_SUDDEN_DEATH: u8 = 5;
pub const STATE_RESULTS: u8 = 6;

/// One entrant of the setup's list (`TmData.x37[i]`, 0x12 bytes, packed): its slot type, its
/// computer's level, handicap, character icon (`CSSIconHud`), the icon, random flag, color
/// and name kept while it's edited, its random flag, color, name (a name tag, `0x320` up for
/// "HMN n", `0x384` up for "CPU n", `0x3E7` for none), its bracket seat, and the match-up
/// screen's seats.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Entrant {
    #[doc(alias("x0"))]
    pub slot_type: u8,
    #[doc(alias("x1"))]
    pub level: u8,
    #[doc(alias("x2"))]
    pub handicap: u8,
    #[doc(alias("x3"))]
    pub icon: u8,
    #[doc(alias("x4"))]
    pub saved_icon: u8,
    #[doc(alias("x5"))]
    pub random: u8,
    #[doc(alias("x6"))]
    pub saved_random: u8,
    #[doc(alias("x7"))]
    pub color: u8,
    #[doc(alias("x8"))]
    pub saved_color: u8,
    #[doc(alias("x9"))]
    pub name: u16,
    /// `xB` as `TmData_80194F30` sees it, a halfword over `xB` and `xC`.
    #[doc(alias("xb"))]
    pub saved_name: u16,
    #[doc(alias("xd"))]
    pub seat: u8,
    #[doc(alias("xe"))]
    pub matchup_seat: u8,
    #[doc(alias("xf"))]
    pub shown_seat: u8,
}

/// A player of the next match (`TmData.x4B8[i]`, 0xA bytes): slot type, character icon,
/// random flag, color, computer level, handicap, name; and the two bytes after, which
/// `fn_8018FA24` writes for the next player (`x8`: the bracket slot's presence, `x9`: its
/// entrant).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct MatchPlayer {
    #[doc(alias("x0"))]
    pub slot_type: u8,
    #[doc(alias("x1"))]
    pub icon: u8,
    #[doc(alias("x2"))]
    pub random: u8,
    #[doc(alias("x3"))]
    pub color: u8,
    #[doc(alias("x4"))]
    pub level: u8,
    #[doc(alias("x5"))]
    pub handicap: u8,
    #[doc(alias("x6"))]
    pub name: u16,
    #[doc(alias("x8"))]
    pub next_present: u8,
    #[doc(alias("x9"))]
    pub next_entrant: u8,
}

/// The tournament's data (`gm_804771C4`, `TmData`, 0x574 bytes of `.bss`): the setup's
/// cursor and options, the round's counters, the entrants and the next match's players, and
/// the texts the scenes make.
#[derive(melee_derive::CloneFrom, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct TmData {
    /// `cur_option`: the setup's option, then the scenes' steps.
    pub cur_option: i32,
    /// `match_type` to `cpu_level`: the setup's six options, which its code indexes as the
    /// words after `cur_option`.
    pub opts: [i32; 6],
    #[doc(alias("x1c"))]
    pub with_camera: i32,
    #[doc(alias("x20"))]
    pub in_name_entry: i32,
    #[doc(alias("x24"))]
    pub rounds: i32,
    #[doc(alias("x28"))]
    pub stage: u32,
    #[doc(alias("x2c"))]
    pub new_bracket: u8,
    #[doc(alias("x2d"))]
    pub to_match: u8,
    #[doc(alias("x2e"))]
    pub entrant_count: u8,
    #[doc(alias("x2f"))]
    pub human_count: u8,
    #[doc(alias("x30"))]
    pub players_per_match: u8,
    #[doc(alias("x31"))]
    pub handicaps: u8,
    #[doc(alias("x32"))]
    pub stage_picked: u8,
    #[doc(alias("x33"))]
    pub rank: u8,
    /// `pad_x34[0]`: the rank shown before the last match's.
    #[doc(alias("x34"))]
    pub last_rank: u8,
    /// The byte before each entrant (`((u8*)&x37[i])[-1]`: `pad_x34[2]` for the first, the
    /// last pad byte of the one before for the rest): whether it's in the tournament.
    pub present: [u8; 64],
    #[doc(alias("x37"))]
    pub roster: [Entrant; 64],
    /// The two bytes before `x4B8` (the last entrant's pad and `pad_x4B7`), which
    /// `fn_8018FA24` writes for the first player.
    #[doc(alias("x4b6"))]
    pub first_present: u8,
    #[doc(alias("x4b7"))]
    pub first_entrant: u8,
    #[doc(alias("x4b8"))]
    pub match_players: [MatchPlayer; 4],
    #[doc(alias("x4e0"))]
    pub title_text: Option<TextId>,
    #[doc(alias("x4e4"))]
    pub count_text: Option<TextId>,
    #[doc(alias("x4e8"))]
    pub description_texts: [Option<TextId>; 6],
    #[doc(alias("x500"))]
    pub value_texts: [Option<TextId>; 6],
    /// The texts from `x518` on (`x518[3]`, `x524[4]`, `x534[16]` overlap as one array).
    #[doc(alias("x518"))]
    pub name_texts: [Option<TextId>; 23],
}

impl Default for TmData {
    fn default() -> TmData {
        TmData {
            cur_option: 0,
            opts: [0; 6],
            with_camera: 0,
            in_name_entry: 0,
            rounds: 0,
            stage: 0,
            new_bracket: 0,
            to_match: 0,
            entrant_count: 0,
            human_count: 0,
            players_per_match: 0,
            handicaps: 0,
            stage_picked: 0,
            rank: 0,
            last_rank: 0,
            present: [0; 64],
            roster: [Entrant::default(); 64],
            first_present: 0,
            first_entrant: 0,
            match_players: [MatchPlayer::default(); 4],
            title_text: None,
            count_text: None,
            description_texts: [None; 6],
            value_texts: [None; 6],
            name_texts: [None; 23],
        }
    }
}

/// The options' places in [`TmData::opts`].
pub const MATCH_TYPE: usize = 0;
pub const COMBATANTS: usize = 1;
pub const ENTRANTS: usize = 2;
pub const HMN_CPU: usize = 3;
pub const STAGE_SEL: usize = 4;
pub const CPU_LEVEL: usize = 5;

impl TmData {
    pub fn match_type(&self) -> i32 {
        self.opts[MATCH_TYPE]
    }
    pub fn combatants(&self) -> i32 {
        self.opts[COMBATANTS]
    }
    pub fn entrants(&self) -> i32 {
        self.opts[ENTRANTS]
    }
    pub fn hmn_cpu_count(&self) -> i32 {
        self.opts[HMN_CPU]
    }
    pub fn stage_selection_type(&self) -> i32 {
        self.opts[STAGE_SEL]
    }
    pub fn cpu_level(&self) -> i32 {
        self.opts[CPU_LEVEL]
    }

    /// The setup's word `k` as its code indexes them (`((s32*) tm)[k]`): the option, the six
    /// options, then `x1C` to `x24`.
    pub fn word(&self, k: usize) -> i32 {
        match k {
            0 => self.cur_option,
            1..=6 => self.opts[k - 1],
            7 => self.with_camera,
            8 => self.in_name_entry,
            9 => self.rounds,
            _ => panic!("TmData's word {k}"),
        }
    }

    pub fn word_mut(&mut self, k: usize) -> &mut i32 {
        match k {
            0 => &mut self.cur_option,
            1..=6 => &mut self.opts[k - 1],
            7 => &mut self.with_camera,
            8 => &mut self.in_name_entry,
            9 => &mut self.rounds,
            _ => panic!("TmData's word {k}"),
        }
    }

    /// The scenes' texts from `x524` on (`x524[k]`: the players' names 2, the winner's 3).
    #[doc(alias("x524"))]
    pub fn screen_text(&self, k: usize) -> Option<TextId> {
        self.name_texts[3 + k]
    }

    /// Each entrant's name's text on the match-up screen (`x534[k]`).
    #[doc(alias("x534"))]
    pub fn entrant_text(&self, k: usize) -> Option<TextId> {
        self.name_texts[7 + k]
    }
}

/// One player's place in a bracket box (`BracketEntrySlot`, 0x2C bytes from `0x2C`): its
/// icon's GObj, whether it's there, a bye's line, its icon's corners and place, its rank in
/// the match, character icon, slot type, color, entrant, level and handicap, and name.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct BracketSlot {
    #[doc(alias("x2c"))]
    pub icon_gobj: Option<GObjId>,
    #[doc(alias("x30"))]
    pub present: u8,
    #[doc(alias("x32"))]
    pub bye_line: u8,
    #[doc(alias("x34"))]
    pub start_x: i32,
    #[doc(alias("x38"))]
    pub start_y: i32,
    #[doc(alias("x3c"))]
    pub target_x: i32,
    #[doc(alias("x40"))]
    pub target_y: i32,
    #[doc(alias("x44"))]
    pub x: i32,
    #[doc(alias("x48"))]
    pub y: i32,
    #[doc(alias("x4c"))]
    pub rank: u8,
    #[doc(alias("x4d"))]
    pub icon: u8,
    #[doc(alias("x4e"))]
    pub slot_type: u8,
    #[doc(alias("x4f"))]
    pub color: u8,
    #[doc(alias("x50"))]
    pub entrant: u8,
    #[doc(alias("x51"))]
    pub level: u8,
    #[doc(alias("x52"))]
    pub handicap: u8,
    #[doc(alias("x54"))]
    pub name: u16,
}

/// A bracket box (`BracketEntry`, 0xDC bytes, `lbl_80473AB8[64]`): whether it's in the
/// bracket, whether it's the match being played, which way it opens, its kind, its seats,
/// where its winner goes (box and seat), its rectangle, line thickness and color, and its four
/// players.
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct BracketEntry {
    #[doc(alias("x0"))]
    pub in_bracket: u8,
    #[doc(alias("x1"))]
    pub playing: u8,
    #[doc(alias("x2"))]
    pub opens: u8,
    #[doc(alias("x3"))]
    pub kind: u8,
    #[doc(alias("x4"))]
    pub seats: u8,
    #[doc(alias("x5"))]
    pub winner_box: u8,
    #[doc(alias("x6"))]
    pub winner_seat: u8,
    #[doc(alias("xc"))]
    pub x: i32,
    #[doc(alias("x10"))]
    pub y: i32,
    #[doc(alias("x14"))]
    pub width: i32,
    #[doc(alias("x18"))]
    pub height: i32,
    #[doc(alias("x1c"))]
    pub thickness: f32,
    #[doc(alias("x20"))]
    pub color: [u8; 4],
    #[doc(alias("x24"))]
    pub unread_a: u8,
    #[doc(alias("x25"))]
    pub unread_b: u8,
    #[doc(alias("x26"))]
    pub unread_c: u8,
    #[doc(alias("x27"))]
    pub unread_d: u8,
    #[doc(alias("x28"))]
    pub unread_e: u8,
    pub slots: [BracketSlot; 4],
}

/// The bracket camera's move (`lbl_803D9DAC`): where it is, where it's going, and its steps.
#[derive(Clone, Copy, Debug, PartialEq, melee_derive::StateEq)]
pub struct CameraMove {
    pub current: Vec3,
    pub target: Vec3,
    pub step: Vec3,
}

/// `415.6922f`, the bracket camera's eye distance.
pub const EYE_Z: f32 = f32::from_bits(0x43CF_D89A);

impl Default for CameraMove {
    fn default() -> CameraMove {
        CameraMove {
            current: Vec3::new(320.0, -240.0, EYE_Z),
            target: Vec3::new(320.0, -240.0, EYE_Z),
            step: Vec3::new(0.0, 0.0, 0.0),
        }
    }
}

/// An archive a scene loaded into its heap (`lbArchive_80016DBC`): the archive and its two
/// blocks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Loaded {
    pub id: Option<ArchiveId>,
    pub file: u32,
    pub archive: u32,
}

/// The setup screen's statics (`gmtou_0.c`): `lbl_804799B8` (the list's cursor and scroll,
/// the names' cursor and page, the arrows' flashes and the models' counters), the archives
/// and their scenes, the option to go back to after the confirmation, the panel's counter and
/// the controllers counted.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Setup {
    #[doc(alias("x0"))]
    pub handicap: u8,
    #[doc(alias("x1"))]
    pub editing_handicap: u8,
    #[doc(alias("x2"))]
    pub cursor: u8,
    #[doc(alias("x3"))]
    pub scroll: u8,
    #[doc(alias("x4"))]
    pub scroll_end: u8,
    #[doc(alias("x5"))]
    pub name_cursor: u8,
    #[doc(alias("x6"))]
    pub name_page: u8,
    #[doc(alias("x7"))]
    pub left_flash: u8,
    #[doc(alias("x8"))]
    pub right_flash: u8,
    #[doc(alias("x9"))]
    pub unused_a: u8,
    #[doc(alias("xa"))]
    pub cursor_flash: u8,
    #[doc(alias("xb"))]
    pub unused_b: u8,
    #[doc(alias("xc"))]
    pub bg_frame: u16,
    #[doc(alias("xe"))]
    pub panel_frame: u8,
    #[doc(alias("xf"))]
    pub marks_frame: u8,
    #[doc(alias("pad2"))]
    pub model_frames: [u8; 16],
    /// `lbl_804D6640`, `6644`, `6648`: `GmTou1p`, `GmTou2p`, `MnExtAll`.
    pub tou1: Loaded,
    pub tou2: Loaded,
    pub ext: Loaded,
    /// `lbl_804D664C`, `6650`: the scenes of `GmTou1p` and `GmTou2p`.
    pub scene1: Option<DataRef>,
    pub scene2: Option<DataRef>,
    /// `lbl_804D6654`, `6658`, `665C`.
    pub back_to: i32,
    pub panel: i32,
    pub pads: i32,
}

/// One player's model state on the bracket's and the match-up's screens (`x2A[i]`: start,
/// current, end frames, state, done, loop).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct PlayerAnim {
    pub start: u8,
    pub cur: u8,
    pub end: u8,
    pub state: u8,
    pub done: u8,
    pub loop_: u8,
}

/// The bracket screen's statics (`gmtou_1.c`): `lbl_804799D8` (its counters, the players'
/// models' states, the winner's name and the result overlay's place), the archives and
/// scenes, and the song's steps.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Bracket {
    #[doc(alias("x0"))]
    pub step_frames: u32,
    #[doc(alias("x4"))]
    pub banner_frame: u32,
    #[doc(alias("x8"))]
    pub banner_start: i32,
    #[doc(alias("xc"))]
    pub banner_end: i32,
    #[doc(alias("x10"))]
    pub bg: u16,
    #[doc(alias("x12"))]
    pub unplugged: [u16; 4],
    #[doc(alias("x1a"))]
    pub panel: u8,
    #[doc(alias("x1b"))]
    pub rank_frames: u8,
    #[doc(alias("x1c"))]
    pub title: u8,
    #[doc(alias("x1d"))]
    pub bounce: [u8; 4],
    #[doc(alias("x21"))]
    pub name_flash: [u8; 4],
    #[doc(alias("x25"))]
    pub cleared: [u8; 4],
    #[doc(alias("x29"))]
    pub bar: u8,
    #[doc(alias("x2a"))]
    pub anims: [PlayerAnim; 4],
    #[doc(alias("x44"))]
    pub choice: [u8; 4],
    /// `x48`: `&gm_80477738` once set.
    #[doc(alias("x48"))]
    pub settled: bool,
    #[doc(alias("x4c"))]
    pub winner_seat: u8,
    #[doc(alias("x4d"))]
    pub winner_type: u8,
    #[doc(alias("x4e"))]
    pub winner_name: [u8; 20],
    #[doc(alias("x64"))]
    pub banner_translate: Vec3,
    #[doc(alias("x70"))]
    pub banner_scale: Vec3,
    /// `lbl_804D6660`, `6664`, `6668`: `GmTou1p`, `GmTou3p`, `GmTou4p`.
    pub tou1: Loaded,
    pub tou3: Loaded,
    pub tou4: Loaded,
    /// `lbl_804D666C`, `6670`, `6674`.
    pub scene1: Option<DataRef>,
    pub scene3: Option<DataRef>,
    pub scene4: Option<DataRef>,
    /// `lbl_804D6678`, `667C`, `6680[3]`.
    #[doc(alias("x6678"))]
    pub back_to_players: i32,
    #[doc(alias("x667c"))]
    pub banner_hidden: i32,
    pub song: [u8; 3],
}

/// The match-up screen's statics (`gmtou_2.c`): `lbl_80479A58` (its counters and the
/// players' models' states), the archives and scenes.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Alt {
    #[doc(alias("x0"))]
    pub confirm_frames: u32,
    #[doc(alias("x4"))]
    pub bg: u16,
    #[doc(alias("x6"))]
    pub unplugged: [u16; 4],
    #[doc(alias("xe"))]
    pub panel: u8,
    #[doc(alias("xf"))]
    pub title: u8,
    #[doc(alias("x10"))]
    pub bounce: [u8; 4],
    #[doc(alias("x18"))]
    pub b_held: [u8; 4],
    #[doc(alias("x1c"))]
    pub bar: u8,
    /// `x1D[i]`: `a` (start), `b` (current), `c` (end), `x0` (state), `x1` (done), `x2`
    /// (loop).
    #[doc(alias("x1d"))]
    pub anims: [PlayerAnim; 4],
    #[doc(alias("x38"))]
    pub choice: [u8; 4],
    /// `lbl_804D6688`, `668C`: `GmTou1p`, `GmTou4p`; `6690`, `6694` their scenes.
    pub tou1: Loaded,
    pub tou4: Loaded,
    pub scene1: Option<DataRef>,
    pub scene4: Option<DataRef>,
}

/// The tournament's statics.
#[derive(melee_derive::CloneFrom, Debug, PartialEq, melee_derive::StateEq)]
pub struct Tournament {
    /// `gm_804771C4` (`gm_GetTournamentData`).
    #[doc(alias = "gm_GetTournamentData")]
    pub tm: TmData,
    /// `lbl_80473AB8`.
    pub bracket: [BracketEntry; 64],
    /// `lbl_804771B8`: `TmBox.dat`'s three tables of boxes, and the archive
    /// (`lbl_804D6638`).
    pub boxes: [Option<DataRef>; 3],
    pub box_file: Loaded,
    /// `gm_80477738`: the last match's end.
    pub match_end: Box<MatchEnd>,
    /// `lbl_804D663C`: the scenes' text canvas.
    pub canvas: i32,
    /// `lbl_804D6630`, `lbl_804D6634`: the bracket's frame counter and the winner's seat.
    pub slide: i32,
    pub seat: i32,
    /// `lbl_803D9DAC` and `lbl_803D9DD0`: the bracket camera's move, its camera and
    /// `xD0` (0, the field of view `fn_80190480(0)` would set).
    pub cam: CameraMove,
    pub cobj: Option<CObjId>,
    /// `lbl_803D9F0C`: the last three random characters.
    pub recent: [i32; 3],
    /// `lbl_803D9D20.x72`: which of the 25 icons' characters are unlocked.
    pub unlocked: [u8; 26],
    /// `lbl_803DA0D0.bounce_y` and `lbl_803DA2E0.x20`: the name tags' bounce, read from the
    /// bracket's and the match-up's models.
    pub bounce: [f32; 41],
    pub alt_bounce: [f32; 41],
    /// `lbl_804D4190`, `lbl_804D4194`: the random stage (which `fn_8019655C` returns) and
    /// the last one.
    #[doc(alias = "fn_8019655C")]
    pub stage: i32,
    pub last_stage: i32,
    pub setup: Setup,
    pub bracket_scene: Bracket,
    pub alt: Alt,
    /// `gm_804876D8`: the match's (and Sudden Death's) enter data.
    pub start: Box<StartMeleeData>,
    /// `gm_80487810`, `gm_8048BE68`: the match's and Sudden Death's exit data.
    pub exit: Box<super::match_exit::MatchExitInfo>,
    pub sd_exit: Box<super::match_exit::MatchExitInfo>,
    /// `gm_8048BD20`: the stage select's enter and exit data.
    pub sss: Box<super::vs_melee::SssData>,
    /// `gm_80489A98`: the results screen's enter data.
    pub results_enter: Box<super::results_scene::ResultsMatchInfo>,
}

impl Default for Tournament {
    fn default() -> Tournament {
        Tournament {
            tm: TmData::default(),
            bracket: [BracketEntry::default(); 64],
            boxes: [None; 3],
            box_file: Loaded::default(),
            match_end: Box::default(),
            canvas: 0,
            slide: 0,
            seat: 0,
            cam: CameraMove::default(),
            cobj: None,
            recent: [-1; 3],
            unlocked: [0; 26],
            bounce: [0.0; 41],
            alt_bounce: [0.0; 41],
            stage: -1,
            last_stage: -1,
            setup: Setup::default(),
            bracket_scene: Bracket::default(),
            alt: Alt::default(),
            start: Box::default(),
            exit: Box::default(),
            sd_exit: Box::default(),
            sss: Box::default(),
            results_enter: Box::default(),
        }
    }
}

impl Running<'_> {
    /// `gm_801B1724`, the setup's entry: the card's work areas and the save's archive.
    #[doc(alias = "gm_801B1724")]
    pub fn tou_enter_setup(&mut self) {
        self.card_alloc_work_area();
        self.card_game_load_archive(0);
    }

    /// `gm_801B174C`, the stage select's entry: no L, R and Start, no forced stage.
    #[doc(alias = "gm_801B174C")]
    pub fn tou_enter_sss(&mut self) {
        let s = &mut self.game.tou.sss;
        s.no_b = 0;
        s.no_lras = 1;
        s.force_stage_id = -1;
        s.cursor_port = 0;
    }

    /// `gm_801B1788`, the stage select's exit: the match with the stage picked, else back to
    /// the bracket (its players' step again) or the match-up screen.
    #[doc(alias = "gm_801B1788")]
    pub fn tou_exit_sss(&mut self) {
        let s = &self.game.tou.sss;
        if s.start_game != 0 {
            let stage = s.vs.start.rules.stkind;
            self.set_next_game_mode_state_id(STATE_VS);
            self.game.tou.tm.stage = u32::from(stage);
            return;
        }
        if self.game.tou.tm.match_type() == 0 {
            self.tou_back_to_players();
            self.set_next_game_mode_state_id(STATE_BRACKET);
            return;
        }
        self.game.tou.tm.stage_picked = 0;
        self.set_next_game_mode_state_id(STATE_ALT);
    }

    /// `gm_801B1810`, the match's entry.
    #[doc(alias = "gm_801B1810")]
    pub fn tou_enter_vs(&mut self) {
        self.tou_setup_match();
    }

    /// `gm_801B1834`, the match's exit: the results screen's setup when a human played, then
    /// Sudden Death for a tie the tournament can't settle, else the results.
    #[doc(alias = "gm_801B1834")]
    pub fn tou_exit_vs(&mut self) {
        let me = self.game.tou.exit.match_end.clone();
        if super::records::any_human(&me) {
            self.game.count_match_kind(me.match_kind, me.outcome);
            let humans = self.game.count_contestants(&me);
            self.game.add_play_time(me.frame_count / 60, humans);
        }
        if super::records::several_winners(&me) && self.tou_tie_goes_on(&me) {
            self.set_next_game_mode_state_id(STATE_SUDDEN_DEATH);
        } else {
            self.set_next_game_mode_state_id(STATE_RESULTS);
        }
    }

    /// `gm_801B18D4`, Sudden Death's entry: the match's players, set up for it.
    #[doc(alias = "gm_801B18D4")]
    pub fn tou_enter_sudden_death(&mut self) {
        // Its enter data is the match's (`gm_804876D8`): the players copied onto themselves.
        let end = self.game.tou.exit.match_end.clone();
        super::setup_players::setup_sudden_death(&mut self.game.tou.start, &end);
    }

    /// `gm_801B1A2C`, Sudden Death's exit.
    #[doc(alias = "gmMainLib_8015CD80")]
    #[doc(alias = "gm_801629B4")]
    #[doc(alias = "gm_801B1A2C")]
    pub fn tou_exit_sudden_death(&mut self) {
        // `gm_801629B4`: the Sudden Death's seconds added to the save's (`x1A44`).
        let secs = self.game.tou.sd_exit.match_end.frame_count / 60;
        let s = &mut self.game.game_data.thing.save_data;
        s.x1A44 = (s.x1A44 as u32).wrapping_add(secs) as i32;
        let mut sd = self.game.tou.sd_exit.match_end.clone();
        super::records::merge_sudden_death(&mut self.game.tou.exit.match_end, &mut sd);
        self.game.tou.sd_exit.match_end.clone_from(&sd);
    }

    /// `gm_801B1A84`, the results' entry: the match's end.
    #[doc(alias = "gm_801B1A84")]
    pub fn tou_enter_results(&mut self) {
        let me = self.game.tou.exit.match_end.clone();
        self.game.tou.results_enter.match_end = me;
    }

    /// `gm_801B1AD4`, the results' exit.
    #[doc(alias = "gm_801B1AD4")]
    pub fn tou_exit_results(&mut self) {
        let me = self.game.tou.results_enter.match_end.clone();
        (*self.game.tou.match_end).clone_from(&me);
        let kos = super::results_exit::human_kos(&me);
        self.results_add_coins(kos);
        if self.game.tou.tm.match_type() == 0 {
            self.set_next_game_mode_state_id(STATE_BRACKET);
            self.game.tou.tm.cur_option = 0x1F;
            return;
        }
        self.tou_next_match_up();
        self.set_next_game_mode_state_id(STATE_ALT);
    }
}
