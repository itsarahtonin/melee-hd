//! Dr. Mario's own code (`ft/kinds/ftDrMario`). His moves are Mario's (`ft::mario`, whose code
//! branches on the kind for the pill); what is his are his creation, spawn and attribute
//! callbacks and his taunt, which holds up a pill (`ftdrmario.c`, `ftdrmarioappeals.c`).

use super::super::code::Val;
use super::super::handles::CodeRef;
use super::super::it::kinds::drmariopill;
use super::super::state::Running;
use super::mario::{attr, ms, u};
use crate::game::ft::vars::{MARIO_CAPE, MARIO_X2240};
use crate::hsd::gobj::GObjId;

pub use super::mario::KIND_DRMARIO;

/// `It_Kind_DrMario_Vitamin`.
const IT_KIND_DRMARIO_VITAMIN: i32 = 49;

/// Dr. Mario's functions other code reaches by address.
pub mod addr {
    pub const ON_DEATH: u32 = 0x8014_949C;
    pub const ON_LOAD: u32 = 0x8014_94E4;
    pub const ON_TAKE_DAMAGE: u32 = 0x8014_9540;
    pub const ON_ITEM_PICKUP: u32 = 0x8014_9560;
    pub const ON_ITEM_INVISIBLE: u32 = 0x8014_9640;
    pub const ON_ITEM_VISIBLE: u32 = 0x8014_9688;
    pub const ON_ITEM_DROP: u32 = 0x8014_96D0;
    pub const LOAD_SPECIAL_ATTRS: u32 = 0x8014_9724;
    pub const ON_KNOCKBACK_ENTER: u32 = 0x8014_9744;
    pub const ON_KNOCKBACK_EXIT: u32 = 0x8014_9788;
    pub const REMOVE_APPEAL_PILL: u32 = 0x8014_97CC;
    pub const FORGET_APPEAL_PILL: u32 = 0x8014_98A0;
    pub const APPEAL_ENTER: u32 = 0x8014_9910;
    pub const APPEAL_S_ANIM: u32 = 0x8014_9954;
    pub const APPEAL_S_IASA: u32 = 0x8014_9A6C;
    pub const APPEAL_S_PHYS: u32 = 0x8014_9A8C;
    pub const APPEAL_S_COLL: u32 = 0x8014_9AAC;
}

impl Running<'_> {
    /// Dr. Mario's creation, `ftDr_Init_OnLoad`: Mario's attribute layout, and his pill and
    /// sheet registered with the items.
    #[doc(alias = "ftDr_Init_OnLoad")]
    pub fn dr_on_load(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.mr_on_load_for_drmario(f);
        let d = self.game.fighters[f.0]
            .ft_data
            .expect("a fighter without data");
        let items = self.data.ptr(d.at(0x48)).expect("ftData::x48_items");
        let sheet = self.char_attr_word(f, attr::SPECIALS_CAPE_KIND) as i32;
        let pill = self.data.ptr(items.at(4));
        super::super::it::item::set_char_article(self.game, pill, IT_KIND_DRMARIO_VITAMIN);
        let sheet_article = self.data.ptr(items.at(12));
        super::super::it::item::set_char_article(self.game, sheet_article, sheet);
    }

    /// Dr. Mario's spawn, `ftDr_Init_OnDeath`: his first model variant, the Tornado's rise and
    /// the sheet's boost ready, no sheet or taunt pill.
    #[doc(alias = "ftDr_Init_OnDeath")]
    pub fn dr_on_death(&mut self, gobj: GObjId) {
        self.game.set_part_vis(gobj, 0, 0);
        let fp = self.game.fighter_mut(gobj);
        fp.set_u_i32(u::TORNADO_CHARGE, 0);
        fp.set_u_i32(u::IS_CAPE_BOOST, 0);
        fp.u.set_gobj_at(MARIO_CAPE, None);
        fp.u.set_gobj_at(MARIO_X2240, None);
    }

    /// `ftDr_Init_LoadSpecialAttrs`: Mario's.
    #[doc(alias = "ftDr_Init_LoadSpecialAttrs")]
    pub fn dr_load_special_attrs(&mut self, gobj: GObjId) {
        self.mr_load_special_attrs(gobj);
    }

    /// `ftDr_Init_80149540` (the taunt's hit hook): the taunt's pill goes.
    #[doc(alias = "ftDr_Init_80149540")]
    pub fn dr_on_take_damage(&mut self, gobj: GObjId) {
        self.dr_remove_appeal_pill(gobj);
    }

    /// `ftDr_Init_801497CC`: the taunt's pill destroyed, if out, and the hit hooks cleared.
    #[doc(alias = "ftDr_Init_801497CC")]
    pub fn dr_remove_appeal_pill(&mut self, gobj: GObjId) {
        if let Some(pill) = self.game.fighter(gobj).u.gobj_at(MARIO_X2240) {
            drmariopill::remove(self, pill);
            self.game.fighter_mut(gobj).u.set_gobj_at(MARIO_X2240, None);
        }
        let fp = self.game.fighter_mut(gobj);
        fp.take_dmg_cb = None;
        fp.death2_cb = None;
    }

    /// Whether the taunt is over for its pill, `ftDr_Init_80149844`: out of the side taunt, or
    /// without the pill.
    #[doc(alias = "ftDr_Init_80149844")]
    pub fn dr_appeal_pill_done(&self, gobj: GObjId) -> bool {
        let fp = self.game.fighter(gobj);
        if fp.motion_id != ms::APPEAL_SR && fp.motion_id != ms::APPEAL_SL {
            return true;
        }
        fp.u.gobj_at(MARIO_X2240).is_none()
    }

    /// `ftDr_Init_801498A0`: the fighter forgets the taunt's pill, and the hit hooks.
    #[doc(alias = "ftDr_Init_801498A0")]
    pub fn dr_forget_appeal_pill(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.u.gobj_at(MARIO_X2240).is_some() {
            fp.u.set_gobj_at(MARIO_X2240, None);
        }
        fp.take_dmg_cb = None;
        fp.death2_cb = None;
    }

    /// `ftDr_Init_801498EC`: the taunt script's second variable (whether the pill shows).
    #[doc(alias = "ftDr_Init_801498EC")]
    pub fn dr_appeal_cmd_var1(&self, gobj: GObjId) -> u32 {
        self.game.fighter(gobj).cmd_vars[1]
    }

    /// Dr. Mario's taunt, `ftDr_Init_80149910`: the side taunt (Mario's states), its pill to
    /// come.
    #[doc(alias = "ftDr_Init_80149910")]
    pub fn dr_appeal_enter(&mut self, gobj: GObjId) {
        self.enter_facing_state(gobj, ms::APPEAL_SR, ms::APPEAL_SL);
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[0] = 1;
        fp.cmd_vars[1] = 0;
    }

    /// `ftDr_AppealS_Anim`: on the script's word the pill (a random color, at his top joint),
    /// on its second word it goes; when the taunt ends, it goes and he waits.
    #[doc(alias = "ftDr_AppealS_Anim")]
    pub fn dr_appeal_s_anim(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        if fp.cmd_vars[0] == 1 && fp.u.gobj_at(MARIO_X2240).is_none() {
            let top = self.bone_joint(gobj, 0);
            let pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, top, None);
            let color = self.mr_special_n_vitamin_random(gobj);
            let facing = self.game.fighter(gobj).facing_dir;
            let pill =
                drmariopill::appeal_spawn(self, gobj, pos, color, IT_KIND_DRMARIO_VITAMIN, facing);
            let fp = self.game.fighter_mut(gobj);
            fp.u.set_gobj_at(MARIO_X2240, pill);
            if pill.is_some() {
                fp.death2_cb = Some(CodeRef(addr::ON_TAKE_DAMAGE));
                fp.take_dmg_cb = Some(CodeRef(addr::ON_TAKE_DAMAGE));
            }
        } else if fp.cmd_vars[0] == 2 {
            self.dr_remove_appeal_pill(gobj);
        }
        if !self.anim_frames_remaining(gobj) {
            self.dr_remove_appeal_pill(gobj);
            self.enter_wait(gobj);
        }
    }

    /// `ftDr_AppealS_IASA`: the side taunt's.
    #[doc(alias = "ftDr_AppealS_IASA")]
    pub fn dr_appeal_s_iasa(&mut self, gobj: GObjId) {
        self.co_appeal_s_iasa(gobj);
    }

    /// `ftDr_AppealS_Phys`: friction.
    #[doc(alias = "ftDr_AppealS_Phys")]
    pub fn dr_appeal_s_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// `ftDr_AppealS_Coll`: the ground's, stopping at ledges.
    #[doc(alias = "ftDr_AppealS_Coll")]
    pub fn dr_appeal_s_coll(&mut self, gobj: GObjId) {
        self.ground_coll_teeter(gobj);
    }

    /// `ftDr_Init_OnKnockbackEnter`: the hurt textures, as Sheik's.
    #[doc(alias = "ftDr_Init_OnKnockbackEnter")]
    pub fn dr_on_knockback_enter(&mut self, gobj: GObjId) {
        self.seak_on_knockback_enter(gobj);
    }

    /// `ftDr_Init_OnKnockbackExit`: the hurt textures, as Sheik's.
    #[doc(alias = "ftDr_Init_OnKnockbackExit")]
    pub fn dr_on_knockback_exit(&mut self, gobj: GObjId) {
        self.seak_on_knockback_exit(gobj);
    }
}

/// Runs one of Dr. Mario's callbacks a fighter holds (`void (*)(HSD_GObj*)`), if `addr` is one.
pub fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::ON_TAKE_DAMAGE => g.dr_on_take_damage(gobj),
        addr::ON_ITEM_INVISIBLE => g.fox_on_item_invisible(gobj),
        addr::ON_ITEM_VISIBLE => g.fox_on_item_visible(gobj),
        addr::ON_KNOCKBACK_ENTER => g.dr_on_knockback_enter(gobj),
        addr::ON_KNOCKBACK_EXIT => g.dr_on_knockback_exit(gobj),
        addr::REMOVE_APPEAL_PILL => g.dr_remove_appeal_pill(gobj),
        addr::FORGET_APPEAL_PILL => g.dr_forget_appeal_pill(gobj),
        addr::APPEAL_ENTER => g.dr_appeal_enter(gobj),
        addr::APPEAL_S_ANIM => g.dr_appeal_s_anim(gobj),
        addr::APPEAL_S_IASA => g.dr_appeal_s_iasa(gobj),
        addr::APPEAL_S_PHYS => g.dr_appeal_s_phys(gobj),
        addr::APPEAL_S_COLL => g.dr_appeal_s_coll(gobj),
        _ => return false,
    }
    true
}

/// Dr. Mario's functions other code calls by address (`code::call`).
pub fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    let gobj = |i: usize| args[i].gobj().expect("a null fighter");
    match f {
        addr::ON_ITEM_PICKUP => {
            r.fox_on_item_pickup(gobj(0), args[1].int() != 0);
            Some(Val::None)
        }
        addr::ON_ITEM_DROP => {
            r.fox_on_item_drop(gobj(0), args[1].int() != 0);
            Some(Val::None)
        }
        _ => match args.first() {
            Some(&Val::GObj(Some(g))) if args.len() == 1 && run(r, g, f) => Some(Val::None),
            _ => None,
        },
    }
}
