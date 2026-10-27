//! The in-match HUD and the text objects a match creates (`melee/if`): the damage
//! percentages and stock icons, the timer, the Ready/GO!/GAME! announcements, the name tags
//! and the rest of what a VS match puts on screen, and the debug text screens every scene
//! sets up. Their GObjs, joints, animations and timers live in the game's state and run in
//! the scheduler's lists, so they are simulation; only their drawing is presentation, and it
//! becomes draw commands ([`Draw`]).

pub mod all;
pub mod announce;
pub mod devtext;
pub mod magnify;
pub mod nametag;
pub mod prize;
pub mod small;
pub mod status;
pub mod stock;
pub mod text;
pub mod time;

use melee_data::archive::{ArchiveId, DataRef};

use super::handles::CodeRef;
use super::structs::{
    Element_803F9628, HudIndex, IfStockUserData, ScInfStcModels, ifAll_804A0FD8_t,
    ifStock_804A1ACC, ifStock_804A1378, ifStock_804A1774, ifTime_data, un_804A1F10_t,
    un_804A1F58_t,
};
use crate::hsd::gobj::GObjId;

/// `JOBJ_HIDDEN`.
pub const HIDDEN: u32 = 1 << 4;
/// `HSD_GObj_JObjCallback`, the render callback that draws a GObj's joints.
pub const JOBJ_RENDER: u32 = 0x8039_1070;
/// `HSD_GObj_803910D8`, a camera GObj's render callback.
pub const CAMERA_RENDER: u32 = 0x8039_10D8;
/// `HSD_GObj_LObjCallback`, a light GObj's render callback.
pub const LIGHT_RENDER: u32 = 0x8039_1044;

/// What a HUD render callback asks the display pass to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub enum Draw {
    /// `HSD_GObj_JObjCallback`: draw the GObj's joints in this pass.
    JObjs(GObjId, i32),
    /// `HSD_CObjSetCurrent` on the GObj's camera, then `HSD_GObj_80390ED0` with this render
    /// mask, then `HSD_CObjEndCurrent`.
    Camera(GObjId, u32),
}

/// A `DynamicModelDesc` copied into a static (`ifTime_match_timer_models`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct ModelDesc {
    pub joint: Option<DataRef>,
    pub anims: Option<DataRef>,
    pub matanims: Option<DataRef>,
    pub shapeanims: Option<DataRef>,
}

/// The HUD's globals, one field per unit's statics.
#[derive(melee_derive::CloneFrom, Debug, melee_derive::StateEq)]
pub struct Hud {
    /// The debug text screens (`textdraw.c`, `textlib.c`).
    pub devtext: devtext::DevTexts,
    /// `ifAll_804A0FD8`: the HUD's camera and light and the places its parts go.
    pub all: ifAll_804A0FD8_t,
    /// `ifall.c`'s `hidden`.
    pub hidden: bool,
    /// `IfAll.dat` (`ifAll_GetArchive`'s static).
    pub archive: Option<ArchiveId>,
    /// `ifStatus_HudInfo`: each player's damage display.
    pub status: HudIndex,
    /// `ifStatus_804D6D60`: how many players the displays are laid out for.
    pub status_count: u8,
    /// `ifStatus_804D6D61`: which way the next exploding digit flies.
    pub status_flip: i8,
    /// `ifStatus_803F9628`: the announcements (GAME!, TIME!, Ready GO! and the rest).
    pub announcements: [Element_803F9628; 8],
    /// `scinfstc_models`: the marks a KO puts by a player's display.
    pub ko_marks: ScInfStcModels,
    /// `ifTime_data` and `ifTime_match_timer_models`.
    pub time: ifTime_data,
    pub timer_models: ModelDesc,
    /// `ifStock_804A1378` and `ifStock_804A157C`: each player's stock icons.
    pub stock: ifStock_804A1378,
    pub stock_users: [IfStockUserData; 6],
    /// `ifStock_804A1774`, `ifStock_804A1A8C` and `ifStock_804A1ACC`: the single-player
    /// tallies.
    pub stock_1774: ifStock_804A1774,
    pub stock_1a8c: [Option<GObjId>; 16],
    pub stock_1acc: ifStock_804A1ACC,
    /// The players' tags.
    pub nametags: nametag::NameTags,
    /// The hazard sign.
    pub hazard: small::Hazard,
    /// The coin counter.
    pub coins: small::Coins,
    /// `un_804A1F10` and `un_804D6D90`: the off-screen arrows, and whether they're hidden.
    pub arrows: un_804A1F10_t,
    pub arrows_hidden: i32,
    /// `un_804A1F58`: the scores.
    pub scores: un_804A1F58_t,
}

impl Default for Hud {
    fn default() -> Self {
        let e = |proc: u32, sfx: i32, frame: u8| Element_803F9628 {
            x8: Some(CodeRef(proc)),
            xC: sfx,
            x10: frame,
            ..Element_803F9628::default()
        };
        let (wait, once) = (announce::WAIT, announce::ONCE);
        Hud {
            devtext: Default::default(),
            all: Default::default(),
            hidden: false,
            archive: None,
            status: Default::default(),
            status_count: 0,
            status_flip: 0,
            announcements: [
                e(wait, 0x7C860, 8),
                e(once, 0xC351, 0),
                e(wait, 0x7C85E, 0),
                e(once, 0x7C85D, 0),
                e(once, 0x7C857, 10),
                e(wait, 0x7C855, 8),
                e(wait, 0x9C48, 0),
                e(wait, 0x9C46, 0),
            ],
            ko_marks: Default::default(),
            time: Default::default(),
            timer_models: Default::default(),
            stock: Default::default(),
            stock_users: Default::default(),
            stock_1774: Default::default(),
            stock_1a8c: [None; 16],
            stock_1acc: Default::default(),
            nametags: Default::default(),
            hazard: Default::default(),
            coins: Default::default(),
            arrows: Default::default(),
            arrows_hidden: 0,
            scores: Default::default(),
        }
    }
}

/// Runs a HUD GObj's process, if `proc` is one; whether it was.
pub fn run_proc(r: &mut super::state::Running, gobj: GObjId, proc: u32) -> bool {
    match proc {
        status::PROC_DAMAGE => r.damage_proc(gobj),
        status::PROC_DIGITS => r.damage_digits_proc(gobj),
        announce::ONCE => r.announcement_once(gobj),
        announce::WAIT => r.announcement_wait(gobj),
        announce::MARK_FADE => r.ko_mark_proc(gobj),
        announce::MARK_FADE_2 => r.ko_mark_proc_2(gobj),
        announce::MARK_SPAWN_2 => r.ko_mark_proc_spawning(gobj),
        time::PROC_TIMER => r.timer_proc(gobj),
        time::PROC_COUNTDOWN => r.countdown_proc(gobj),
        stock::PROC_ICONS => r.stock_proc(gobj),
        stock::PROC_TALLY => r.stock_tally_proc(gobj),
        stock::PROC_TALLY_2 => r.stock_tally_proc_2(gobj),
        stock::PROC_TARGETS => r.target_icons_proc(gobj),
        nametag::PROC => r.nametag_proc(gobj),
        small::HAZARD_PROC => r.hazard_proc(gobj),
        small::COIN_PROC => r.coin_proc(gobj),
        small::ARROW_PROC => r.arrow_proc(gobj),
        small::SCORE_PROC => r.score_proc(gobj),
        _ => return false,
    }
    true
}
