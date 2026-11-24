//! The HUD's structs (`melee/if`), hand-written: each field's `#[at]` is its place in the
//! original's, which the comparer's mirror reads (`#[derive(Mirror)]`) and the layout check
//! holds to the decomp's (`game::mirror`). The fields keep the original's order, so the sync
//! test's hash reads them as the generated structs' did. Each one's state lives in
//! [`Hud`](super::Hud), but the magnifier's (`Game::magnify`, which the display reads).

use melee_data::archive::DataRef;

use super::magnify::MagnifyImage;
use super::stock::StockJObj;
use crate::game::handles::{CodeRef, RawPtr};
use crate::game::structs::{_GXColor, HSD_ImageDesc};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::JObjId;
use crate::hsd::sis::TextId;
use crate::vec::Vec3;

/// The decomp's `ifMagnifyPlayer_state`.
#[doc(alias("ifMagnifyPlayer_state"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "ifMagnifyPlayer_state", size = 0x1)]
pub struct BubbleState {
    #[at(0x0, bits(0, 1))]
    pub is_offscreen: bool,
    #[at(0x0, bits(1, 1))]
    pub ignore_offscreen: bool,
    #[at(0x0, bits(2, 6))]
    pub edge: u8,
}

/// The decomp's `ifMagnifyPlayer`.
#[doc(alias("ifMagnifyPlayer"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "ifMagnifyPlayer", size = 0x10, pad(0xD, 3))]
pub struct MagnifierBubble {
    #[at(0x0)]
    pub gobj: Option<GObjId>,
    #[at(0x4)]
    pub jobj: Option<JObjId>,
    #[at(0x8)]
    pub idesc: MagnifyImage,
    #[at(0xC)]
    pub state: BubbleState,
}

/// The decomp's `ifMagnify`.
#[doc(alias("ifMagnify"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "ifMagnify", size = 0xF0)]
pub struct Magnifier {
    #[at(0x0)]
    pub model_desc: Option<DataRef>,
    /// The stage's bubble colors (`GroundParam`'s), for the magnifier.
    #[at(0x4)]
    #[doc(alias("x4"))]
    pub bubble_color_0: _GXColor,
    #[at(0x8)]
    #[doc(alias("x8"))]
    pub bubble_color_2: _GXColor,
    #[at(0xC)]
    #[doc(alias("xC"))]
    pub bubble_color_6: _GXColor,
    #[at(0x10)]
    #[doc(alias("x10"))]
    pub bubble_color_8: _GXColor,
    #[at(0x14)]
    pub player: [MagnifierBubble; 6],
    #[at(0x74)]
    pub image_descs: [HSD_ImageDesc; 5],
    #[at(0xEC)]
    #[doc(alias("pad"))]
    pub pad_ec: [u8; 4],
}

/// The decomp's `IfDamageFlags`.
#[doc(alias("IfDamageFlags"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "IfDamageFlags", size = 0x1)]
pub struct DamageDisplayFlags {
    #[at(0x0, bits(0, 1))]
    pub explode_animation: bool,
    #[at(0x0, bits(1, 1))]
    pub randomize_velocity: bool,
    #[at(0x0, bits(2, 1))]
    pub force_digit_shake: bool,
    /// Anim 1 (the damage going the good way: down, or up in stamina) plays.
    #[at(0x0, bits(3, 1))]
    #[doc(alias("unk10"))]
    pub heal_anim: bool,
    #[at(0x0, bits(4, 1))]
    pub hide_all_digits: bool,
    #[at(0x0, bits(5, 2))]
    pub animation_status_id: u8,
    #[at(0x0, bits(7, 1))]
    #[doc(alias("unk1"))]
    pub unused_b7: bool,
}

/// The decomp's `IfDamageState`.
#[doc(alias("IfDamageState"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "IfDamageState", size = 0x64, pad(0x11, 3))]
pub struct DamageDisplay {
    /// The display's GObj.
    #[at(0x0)]
    #[doc(alias("HUD_parent_entity"))]
    pub parent: Option<GObjId>,
    #[at(0x4)]
    pub next: Option<GObjId>,
    #[at(0x8)]
    pub player_slot: u8,
    /// Frames since the display blew apart (to 0xC8, when it's freed).
    #[at(0x9)]
    #[doc(alias("unk9"))]
    pub fade_frames: u8,
    #[at(0xA)]
    pub damage_percent: i16,
    #[at(0xC)]
    pub old_damage: i16,
    #[at(0xE)]
    pub damage_from_last_attack: u8,
    #[at(0xF)]
    pub frames_of_shake_remaining: u8,
    #[at(0x10)]
    pub flags: DamageDisplayFlags,
    #[at(0x14)]
    pub translation_x: [f32; 4],
    #[at(0x24)]
    pub translation_y: [f32; 4],
    #[at(0x34)]
    pub velocity_x: [f32; 4],
    #[at(0x44)]
    pub velocity_y: [f32; 4],
    #[at(0x54)]
    pub jobjs: [Option<JObjId>; 4],
}

/// The decomp's `HudIndex`.
#[doc(alias("HudIndex"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "HudIndex", size = 0x278)]
pub struct DamageDisplays {
    #[at(0x0)]
    pub players: [DamageDisplay; 6],
    #[at(0x258)]
    pub damage_num_joint: Option<DataRef>,
    #[at(0x25C)]
    pub damage_num_anims: Option<DataRef>,
    #[at(0x260)]
    pub damage_num_matanims: Option<DataRef>,
    #[at(0x264)]
    pub damage_num_shapeanims: Option<DataRef>,
    #[at(0x268)]
    pub damage_mark_joint: Option<DataRef>,
    #[at(0x26C)]
    pub damage_mark_anims: Option<DataRef>,
    #[at(0x270)]
    pub damage_mark_matanims: Option<DataRef>,
    #[at(0x274)]
    pub damage_mark_shapeanims: Option<DataRef>,
}

/// The decomp's `Element_803F9628_x12`.
#[doc(alias("Element_803F9628_x12"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "Element_803F9628_x12", size = 0x1)]
pub struct AnnouncementFlags {
    #[at(0x0, bits(0, 1))]
    #[doc(alias("x0"))]
    pub sfx_1_played: bool,
    #[at(0x0, bits(1, 1))]
    #[doc(alias("x1"))]
    pub sfx_2_played: bool,
    /// The start callback ran.
    #[at(0x0, bits(2, 1))]
    #[doc(alias("x2"))]
    pub started: bool,
    #[at(0x0, bits(3, 1))]
    #[doc(alias("x3"))]
    pub unused_3: bool,
    #[at(0x0, bits(4, 1))]
    #[doc(alias("x4"))]
    pub unused_4: bool,
    #[at(0x0, bits(5, 1))]
    #[doc(alias("x5"))]
    pub unused_5: bool,
    #[at(0x0, bits(6, 1))]
    #[doc(alias("x6"))]
    pub unused_6: bool,
    #[at(0x0, bits(7, 1))]
    #[doc(alias("x7"))]
    pub unused_7: bool,
}

/// The decomp's `Element_803F9628`.
#[doc(alias("Element_803F9628"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "Element_803F9628", size = 0x28)]
pub struct Announcement {
    #[at(0x0)]
    #[doc(alias("x0"))]
    pub gobj: Option<GObjId>,
    #[at(0x4)]
    #[doc(alias("x4"))]
    pub unread_4: u32,
    #[at(0x8)]
    #[doc(alias("x8"))]
    pub proc: Option<CodeRef>,
    /// The announcement's own first line (the table's), played at `sfx_frame` unless a caller
    /// gave one; the caller's second line at `sfx_2_frame`.
    #[at(0xC)]
    #[doc(alias("xC"))]
    pub default_sfx: i32,
    #[at(0x10)]
    #[doc(alias("x10"))]
    pub sfx_frame: u8,
    #[at(0x11)]
    #[doc(alias("x11"))]
    pub sfx_2_frame: u8,
    #[at(0x12)]
    #[doc(alias("x12"))]
    pub flags: AnnouncementFlags,
    #[at(0x13)]
    #[doc(alias("x13"))]
    pub unused_13: u8,
    #[at(0x14)]
    #[doc(alias("x14"))]
    pub model: Option<DataRef>,
    #[at(0x18)]
    #[doc(alias("x18"))]
    pub on_start: Option<CodeRef>,
    #[at(0x1C)]
    #[doc(alias("x1C"))]
    pub on_end: Option<CodeRef>,
    #[at(0x20)]
    #[doc(alias("x20"))]
    pub sfx_1: i32,
    #[at(0x24)]
    #[doc(alias("x24"))]
    pub sfx_2: i32,
}

/// The decomp's `ScInfStcSlot`.
#[doc(alias("ScInfStcSlot"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "ScInfStcSlot", size = 0x8)]
pub struct KoMarkSlot {
    #[at(0x0)]
    pub gobj: Option<GObjId>,
    #[at(0x4)]
    pub gobj2: Option<GObjId>,
}

/// The decomp's `ScInfStcModels`.
#[doc(alias("ScInfStcModels"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "ScInfStcModels", size = 0x34)]
pub struct KoMarks {
    #[at(0x0)]
    pub scene_models: Option<DataRef>,
    #[at(0x4)]
    pub slots: [KoMarkSlot; 6],
}

/// The decomp's `ifTime_data`.
#[doc(alias("ifTime_data"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "ifTime_data", size = 0x3C)]
pub struct HudTimer {
    #[at(0x0)]
    pub match_timer: Option<GObjId>,
    #[at(0x4)]
    pub countdown_timer: Option<GObjId>,
    #[at(0x8)]
    pub digits: [Option<JObjId>; 10],
    #[at(0x30)]
    pub countdown_seconds: u8,
    #[at(0x31)]
    pub pad31: [i8; 7],
    #[at(0x38)]
    pub countdown_timer_models: Option<DataRef>,
}

/// The decomp's `ifAll_804A0FD8_t`.
#[doc(alias("ifAll_804A0FD8_t"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "ifAll_804A0FD8_t", size = 0x9C)]
pub struct HudLayout {
    #[at(0x0)]
    pub gobj: Option<GObjId>,
    #[at(0x4)]
    pub gobj_2: Option<GObjId>,
    #[at(0x8)]
    #[doc(alias("x8"))]
    pub root: Option<JObjId>,
    #[at(0xC)]
    #[doc(alias("xC"))]
    pub timer_pos: Vec3,
    /// Each player's display's place, and the places for three and for two players.
    #[at(0x18)]
    #[doc(alias("x18"))]
    pub player_pos: [Vec3; 6],
    #[at(0x60)]
    #[doc(alias("x60"))]
    pub player_pos_3: [Vec3; 3],
    #[at(0x84)]
    #[doc(alias("x84"))]
    pub player_pos_2: [Vec3; 2],
}

/// The decomp's `ifStock_804A1378_per_player`: a player's stock icons' model and its joints by
/// index: 0 to 7 the icons; past them, by mode, the coin counter's digits (9 to 13, 13 also
/// its 0), the stock count's group (14), its ones (15) and tens or only digit (16).
#[doc(alias("ifStock_804A1378_per_player"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "ifStock_804A1378_per_player", size = 0x50)]
pub struct PlayerStocks {
    #[at(0x0)]
    #[doc(alias("x0"))]
    pub gobj: Option<GObjId>,
    #[at(0x4)]
    #[doc(alias("x4"))]
    pub joints: [Option<JObjId>; 8],
    #[at(0x24)]
    #[doc(alias("x24"))]
    pub joint_8: Option<JObjId>,
    #[at(0x28)]
    #[doc(alias("x28"))]
    pub joint_9: Option<JObjId>,
    #[at(0x2C)]
    #[doc(alias("x2C"))]
    pub joint_10: Option<JObjId>,
    #[at(0x30)]
    #[doc(alias("x30"))]
    pub joint_11: Option<JObjId>,
    #[at(0x34)]
    #[doc(alias("x34"))]
    pub joint_12: Option<JObjId>,
    #[at(0x38)]
    #[doc(alias("x38"))]
    pub joint_13: Option<JObjId>,
    #[at(0x3C)]
    #[doc(alias("x3C"))]
    pub joint_14: Option<JObjId>,
    #[at(0x40)]
    #[doc(alias("x40"))]
    pub joint_15: Option<JObjId>,
    #[at(0x44)]
    #[doc(alias("x44"))]
    pub joint_16: Option<JObjId>,
    #[at(0x48)]
    pub coins: i32,
    #[at(0x4C)]
    pub stocks: i32,
}

/// The decomp's `ifStock_804A1378`.
#[doc(alias("ifStock_804A1378"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "ifStock_804A1378", size = 0x204)]
pub struct HudStocks {
    #[at(0x0)]
    #[doc(alias("x0"))]
    pub models: Option<DataRef>,
    #[at(0x4)]
    #[doc(alias("x4"))]
    pub counter_model: Option<DataRef>,
    #[at(0x8)]
    pub player: [PlayerStocks; 6],
    #[at(0x1E8)]
    pub gobj: Option<GObjId>,
    #[at(0x1EC)]
    pub jobj: Option<JObjId>,
    #[at(0x1F0)]
    pub jobj_1: StockJObj,
    #[at(0x1F4)]
    pub jobj_a: Option<JObjId>,
    #[at(0x1F8)]
    pub jobj_b: Option<JObjId>,
    #[at(0x1FC)]
    pub jobj_c: Option<JObjId>,
    #[at(0x200)]
    pub jobj_d: Option<JObjId>,
}

/// The decomp's `IfStockStealAnim`.
#[doc(alias("IfStockStealAnim"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "IfStockStealAnim", size = 0x24)]
pub struct StockSteal {
    #[at(0x0)]
    pub start: Vec3,
    #[at(0xC)]
    pub mid: Vec3,
    #[at(0x18)]
    pub end: Vec3,
}

/// The decomp's `IfStockUserData`.
#[doc(alias("IfStockUserData"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "IfStockUserData", size = 0x54)]
pub struct StockUserData {
    #[at(0x0)]
    pub player: u8,
    #[at(0x1)]
    pub mode: u8,
    #[at(0x2)]
    pub flag: u8,
    /// The players the steal slots' stocks went to (slots 5 and 6).
    #[at(0x3)]
    #[doc(alias("x3"))]
    pub steal_takers: [u8; 2],
    #[at(0x5)]
    pub anim: [u8; 7],
    #[at(0xC)]
    pub steal: [StockSteal; 2],
}

/// The decomp's `ifStock_804A1774`, the joiners' tally: 0 to make, 1 made; the joiners'
/// costumes and characters (-1 beaten, -2 the end), the crowd's count shown, the icons.
#[doc(alias("ifStock_804A1774"))]
#[derive(
    melee_derive::CloneFrom, Debug, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "ifStock_804A1774", size = 0x318)]
pub struct JoinerTally {
    #[at(0x0)]
    #[doc(alias("x0"))]
    pub state: i8,
    #[at(0x1)]
    #[doc(alias("x1"))]
    pub costumes: [i8; 130],
    #[at(0x83)]
    #[doc(alias("x83"))]
    pub chars: [i8; 133],
    #[at(0x108)]
    #[doc(alias("x108"))]
    pub crowd_count: i32,
    #[at(0x10C)]
    #[doc(alias("x10C"))]
    pub icons: [Option<GObjId>; 131],
}

impl Default for JoinerTally {
    fn default() -> Self {
        JoinerTally {
            state: 0,
            costumes: [0; 130],
            chars: [0; 133],
            crowd_count: 0,
            icons: [None; 131],
        }
    }
}

/// The decomp's `ifStock_804A1ACC`, the characters-left tally (All-Star's rest area).
#[doc(alias("ifStock_804A1ACC"))]
#[derive(
    melee_derive::CloneFrom, Debug, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "ifStock_804A1ACC", size = 0x314)]
pub struct CharsLeftTally {
    #[at(0x0)]
    #[doc(alias("x0"))]
    pub state: i8,
    #[at(0x1)]
    #[doc(alias("x1"))]
    pub chars: [i8; 130],
    #[at(0x83)]
    #[doc(alias("x83"))]
    pub costumes: [i8; 133],
    #[at(0x108)]
    #[doc(alias("x108"))]
    pub unread_108: Option<GObjId>,
    #[at(0x10C)]
    #[doc(alias("x10C"))]
    pub icons: [Option<GObjId>; 130],
}

impl Default for CharsLeftTally {
    fn default() -> Self {
        CharsLeftTally {
            state: 0,
            chars: [0; 130],
            costumes: [0; 133],
            unread_108: None,
            icons: [None; 130],
        }
    }
}

/// The decomp's `un_804A1F10_t`.
#[doc(alias("un_804A1F10_t"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "un_804A1F10_t", size = 0x38, pad(0x33, 1))]
pub struct OffscreenArrows {
    #[at(0x0)]
    #[doc(alias("x0"))]
    pub models: Option<DataRef>,
    #[at(0x4)]
    #[doc(alias("x4"))]
    pub gobjs: [Option<GObjId>; 4],
    #[at(0x14)]
    #[doc(alias("x14"))]
    pub colors: [_GXColor; 4],
    /// By slot: the player's arrow is out.
    #[at(0x24)]
    #[doc(alias("x24"))]
    pub shown: [u8; 4],
    /// By arrow: its side (0 none, 1 left, 2 right) and its player.
    #[at(0x28)]
    #[doc(alias("x28"))]
    pub sides: [u8; 4],
    #[at(0x2C)]
    #[doc(alias("x2C"))]
    pub players: [u8; 4],
    /// The rules' `arrows_flag`: no sound.
    #[at(0x30)]
    #[doc(alias("x30"))]
    pub silent: u8,
    /// The arrow whose turn it is to sound, and the sound's timer (to 51).
    #[at(0x31)]
    #[doc(alias("x31"))]
    pub sounding: i8,
    #[at(0x32)]
    #[doc(alias("x32"))]
    pub sound_timer: u8,
    #[at(0x34)]
    pub pad_34: RawPtr,
}

/// The decomp's `un_803F9E08_t`.
#[doc(alias("un_803F9E08_t"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "un_803F9E08_t", size = 0x10)]
pub struct CoinCounter {
    #[at(0x0, bits(0, 1))]
    #[doc(alias("x0_b0"))]
    pub unread_b0: bool,
    #[at(0x0, bits(1, 1))]
    #[doc(alias("x0_b1"))]
    pub counting: bool,
    /// A coin every `step` frames.
    #[at(0x0, bits(2, 3))]
    #[doc(alias("x0_b234"))]
    pub step: u8,
    #[at(0x0, bits(5, 3))]
    #[doc(alias("x0_b567"))]
    pub step_timer: u8,
    #[at(0x1)]
    #[doc(alias("x1"))]
    pub count: u8,
    #[at(0x2)]
    #[doc(alias("x2"))]
    pub target: u8,
    #[at(0x3)]
    #[doc(alias("x3"))]
    pub unused_3: u8,
    #[at(0x4)]
    #[doc(alias("x4"))]
    pub pos_x: i32,
    #[at(0x8)]
    #[doc(alias("x8"))]
    pub pos_y: i32,
    #[at(0xC)]
    #[doc(alias("xC"))]
    pub gobj: Option<GObjId>,
}

/// The decomp's `un_804A1F58_x8_t`.
#[doc(alias("un_804A1F58_x8_t"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "un_804A1F58_x8_t", size = 0x14, pad(0x11, 3))]
pub struct ScoreDisplay {
    #[at(0x0)]
    #[doc(alias("x0"))]
    pub gobj: Option<GObjId>,
    #[at(0x4)]
    #[doc(alias("x4"))]
    pub text: Option<TextId>,
    #[at(0x8)]
    #[doc(alias("x8"))]
    pub string: u32,
    #[at(0xC)]
    #[doc(alias("xC"))]
    pub score: u32,
    /// "-" shown in place of the score.
    #[at(0x10)]
    #[doc(alias("x10"))]
    pub hidden: u8,
}

/// The decomp's `un_804A1F58_t`.
#[doc(alias("un_804A1F58_t"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "un_804A1F58_t", size = 0x80)]
pub struct ScoreDisplays {
    #[at(0x0)]
    #[doc(alias("x0"))]
    pub canvas: u32,
    #[at(0x4)]
    pub pad_x4: [i8; 4],
    #[at(0x8)]
    #[doc(alias("x8"))]
    pub entries: [ScoreDisplay; 6],
}
