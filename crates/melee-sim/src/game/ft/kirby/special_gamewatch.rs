//! Mr. Game & Watch's copy (`ftkirbyspecialgamewatch.c`): Chef as Kirby flips it, a pan of
//! his own in hand (`it::kinds::kirbygamewatchchefpan`, kept in `u` as `xDC`) and each flip
//! throwing a sausage. Mr. Game & Watch's own code
//! ([`ft::gamewatch::special_n`](super::super::gamewatch::special_n)) over his states, the
//! attributes 0x3CC bytes on in his, the last two sausages in his `u` (`xD4`, `xD8`), the
//! copy's sausage kind and his callbacks ([`KIRBY`]); the pan, which Mr. Game & Watch's has
//! no item for, is his own and here. The items take their colors from the copy's data
//! (`ftKb_Init_800EEB00`, `_800EEB1C`).

use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::kirbygamewatchchefpan as pan;
use super::super::super::state::Running;
use super::super::super::structs::_GXColor;
use super::super::gamewatch::part;
use super::super::gamewatch::special_n::{Chef, KEEP_FLAGS};
use super::super::motion::flags::SKIP_MODEL;
use super::{addr as kb, ms};
use crate::game::ft::FighterKind;
use crate::game::it::ItKind;
use crate::hsd::gobj::GObjId;

/// The copy's function addresses.
pub(super) mod addr {
    pub const SAUSAGE: u32 = 0x8010_CE5C;
    pub const PAN: u32 = 0x8010_CFB0;
    pub const PAN_GONE: u32 = 0x8010_D074;
    pub const CLEANUP: u32 = 0x8010_D0A8;
    pub const ON_ENTER_HITLAG: u32 = 0x8010_D100;
    pub const ON_EXIT_HITLAG: u32 = 0x8010_D130;
    pub const ENTER: u32 = 0x8010_D188;
    pub const AIR_ENTER: u32 = 0x8010_D204;
    pub const N_ANIM: u32 = 0x8010_D280;
    pub const AIR_N_ANIM: u32 = 0x8010_D320;
    pub const N_IASA: u32 = 0x8010_D3C0;
    pub const AIR_N_IASA: u32 = 0x8010_D444;
    pub const N_PHYS: u32 = 0x8010_D4C8;
    pub const AIR_N_PHYS: u32 = 0x8010_D4E8;
    pub const N_COLL: u32 = 0x8010_D508;
    pub const AIR_N_COLL: u32 = 0x8010_D544;
    pub const TO_AIR: u32 = 0x8010_D580;
    pub const TO_GROUND: u32 = 0x8010_D5F0;
}

/// Kirby's Chef: his states, the attributes and `u` words at his offsets, the copy's sausage
/// kind, his own callbacks (the pan first), and his model not kept through the changes
/// (`ftKb_MF_GwSpecialN_Coll`).
pub const KIRBY: Chef = Chef {
    states: [ms::GW_SPECIAL_N, ms::GW_SPECIAL_AIR_N],
    attrs: 0x3CC,
    u: 0xC0,
    sausage_kind: ItKind::KirbyGameWatchChef.id(),
    sausage_cb: addr::SAUSAGE,
    start_cb: addr::PAN,
    keep: KEEP_FLAGS & !SKIP_MODEL,
};

/// `u` offset of the pan (`xDC`).
const PAN: usize = 0xDC;

impl Running<'_> {
    /// A color of his copy of Mr. Game & Watch, the word `off` bytes into the copy's last
    /// dynamics (`hat_dynamics[4]`): the items' and the costume's materials' (4,
    /// `ftKb_Init_800EEB00`), and theirs and his outline's (8, `ftKb_Init_800EEB1C`).
    #[doc(alias = "ftKb_Init_800EEB00")]
    #[doc(alias = "ftKb_Init_800EEB1C")]
    pub fn kb_gw_color(&self, off: u32) -> _GXColor {
        let copy = self.game.ft.kirby.copies[FighterKind::GameWatch.index()]
            .expect("a copy without its data");
        let colors = self
            .data
            .ptr(copy.at(0x1C))
            .expect("the copy's colors (hat_dynamics[4])");
        let [r, g, b, a] = self.data.u32(colors.at(off)).to_be_bytes();
        _GXColor { r, g, b, a }
    }

    /// The pan in hand at the move's start, `fn_8010CFB0`: with his damage hooks while he
    /// holds it, his hitlag's, and the sausages' accessory next.
    #[doc(alias = "fn_8010CFB0")]
    pub fn kb_gw_pan(&mut self, gobj: GObjId) {
        let bone = self.bone_index(self.game.fighter(gobj), part::L_THUMB_NB);
        let pos = self.gw_part_pos(gobj, bone, None);
        let facing = self.game.fighter(gobj).facing_dir;
        let p = pan::spawn(self, gobj, pos, bone, facing);
        let fp = self.game.fighter_mut(gobj);
        fp.u.set_gobj_at(PAN, p);
        if p.is_some() {
            fp.death2_cb = Some(CodeRef(kb::DEATH3));
            fp.take_dmg_cb = Some(CodeRef(kb::DEATH1));
        }
        fp.pre_hitlag_cb = Some(CodeRef(addr::ON_ENTER_HITLAG));
        fp.post_hitlag_cb = Some(CodeRef(addr::ON_EXIT_HITLAG));
        fp.accessory4_cb = Some(CodeRef(addr::SAUSAGE));
    }

    /// He forgets the pan, `ftKb_SpecialNGw_8010D074`, the pan going on after his hitlag first.
    #[doc(alias = "ftKb_SpecialNGw_8010D074")]
    pub fn kb_gw_pan_gone(&mut self, gobj: GObjId) {
        self.kb_gw_exit_hitlag(gobj);
        self.game.fighter_mut(gobj).u.set_gobj_at(PAN, None);
    }

    /// The pan taken away when he's hit or KO'd, `ftKb_SpecialNGw_8010D0A8`.
    #[doc(alias = "ftKb_SpecialNGw_8010D0A8")]
    pub fn kb_gw_cleanup(&mut self, gobj: GObjId) {
        if let Some(p) = self.game.fighter(gobj).u.gobj_at(PAN) {
            pan::remove(self, p);
            self.kb_gw_pan_gone(gobj);
        }
    }

    /// The pan pauses in his hitlag, `fn_8010D100`.
    #[doc(alias = "fn_8010D100")]
    pub fn kb_gw_enter_hitlag(&mut self, gobj: GObjId) {
        if let Some(p) = self.game.fighter(gobj).u.gobj_at(PAN) {
            pan::hitlag_start(self, p);
        }
    }

    /// The pan goes on after his hitlag, `ftKb_SpecialNGw_8010D130`.
    #[doc(alias = "ftKb_SpecialNGw_8010D130")]
    pub fn kb_gw_exit_hitlag(&mut self, gobj: GObjId) {
        if let Some(p) = self.game.fighter(gobj).u.gobj_at(PAN) {
            pan::hitlag_end(self, p);
        }
    }

    /// Whether he has left the move, `ftKb_SpecialNGw_8010D160` (true outside its two
    /// states).
    #[doc(alias = "ftKb_SpecialNGw_8010D160")]
    pub fn kb_gw_done(&self, gobj: GObjId) -> bool {
        let msid = self.game.fighter(gobj).motion_id;
        !(ms::GW_SPECIAL_N..=ms::GW_SPECIAL_AIR_N).contains(&msid)
    }
}

#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    let p = &KIRBY;
    match a {
        addr::SAUSAGE => g.gw_create_sausage(gobj, p),
        addr::PAN => g.kb_gw_pan(gobj),
        addr::PAN_GONE => g.kb_gw_pan_gone(gobj),
        addr::CLEANUP => g.kb_gw_cleanup(gobj),
        addr::ON_ENTER_HITLAG => g.kb_gw_enter_hitlag(gobj),
        addr::ON_EXIT_HITLAG => g.kb_gw_exit_hitlag(gobj),
        addr::ENTER => g.gw_special_n_enter(gobj, p, false),
        addr::AIR_ENTER => g.gw_special_n_enter(gobj, p, true),
        addr::N_ANIM => g.gw_special_n_anim(gobj, p, false),
        addr::AIR_N_ANIM => g.gw_special_n_anim(gobj, p, true),
        addr::N_IASA => g.gw_special_n_iasa(gobj, p, false),
        addr::AIR_N_IASA => g.gw_special_n_iasa(gobj, p, true),
        addr::N_PHYS => g.gw_special_n_phys(gobj),
        addr::AIR_N_PHYS => g.gw_special_air_n_phys(gobj),
        addr::N_COLL => g.gw_special_n_coll(gobj, p),
        addr::AIR_N_COLL => g.gw_special_air_n_coll(gobj, p),
        addr::TO_AIR => g.gw_special_n_to_air(gobj, p),
        addr::TO_GROUND => g.gw_special_air_n_to_ground(gobj, p),
        _ => return false,
    }
    true
}
