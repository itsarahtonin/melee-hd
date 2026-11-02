//! Giga Bowser's own callbacks (`ft/kinds/ftGigaKoopa/ftgkoopa.c`). He runs Bowser's code
//! (`ft::koopa`): his state table holds Bowser's specials with his own animations, his
//! attributes have Bowser's layout (`ftKoopaAttributes`) and his variables Bowser's
//! (`ftKoopa_FighterVars`). What's his: he can't be grabbed (`x222A_b0`, which every grab box
//! skips, Kirby's inhale among them), and his item and knockback hooks. His demo pose and its
//! motion file are `ft::demo`'s.

use super::super::code::Val;
use super::super::it::kinds::KOOPA_FLAME;
use super::super::state::Running;
use super::koopa::{attr, u};
use crate::hsd::gobj::GObjId;

/// Giga Bowser's functions, by address.
pub mod addr {
    pub const ON_DEATH: u32 = 0x8014_F640;
    pub const UNK_MOTION_STATES3: u32 = 0x8014_F698;
    pub const ON_LOAD: u32 = 0x8014_F6B8;
    pub const ON_ITEM_PICKUP: u32 = 0x8014_F720;
    pub const ON_ITEM_INVISIBLE: u32 = 0x8014_F800;
    pub const ON_ITEM_VISIBLE: u32 = 0x8014_F848;
    pub const ON_ITEM_DROP: u32 = 0x8014_F890;
    pub const LOAD_SPECIAL_ATTRS: u32 = 0x8014_F8E4;
    pub const ON_KNOCKBACK_ENTER: u32 = 0x8014_F904;
    pub const ON_KNOCKBACK_EXIT: u32 = 0x8014_F948;
}

impl Running<'_> {
    /// His spawn, `ftGk_Init_OnDeath`: his first model variant, Bowser's super armor, and
    /// the flame at full speed and size.
    #[doc(alias = "ftGk_Init_OnDeath")]
    pub fn gk_on_death(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.game.set_part_vis(gobj, 0, 0);
        let armor = self.kp_attr_f32(f, attr::ARMOR);
        let speed = self.kp_attr_f32(f, attr::X10);
        let size = self.kp_attr_f32(f, attr::X18);
        let fp = &mut self.game.fighters[f.0];
        fp.dmg.armor0 = armor;
        fp.set_u_f32(u::FLAME_SPEED, speed);
        fp.set_u_f32(u::FLAME_SIZE, size);
    }

    /// `ftGk_Init_UnkMotionStates3`, each frame before his state's callbacks: Bowser's
    /// (the flame's refill).
    #[doc(alias = "ftGk_Init_UnkMotionStates3")]
    pub fn gk_unk_motion_states3(&mut self, gobj: GObjId) {
        self.kp_unk_motion_states3(gobj);
    }

    /// His creation, `ftGk_Init_OnLoad`: Bowser's attribute layout pushed
    /// (`ftKp_Init_OnLoadForGKoopa`), his flame registered, `x2226_b1`, and no grabbing him
    /// (`x222A_b0`).
    #[doc(alias = "ftGk_Init_OnLoad")]
    #[doc(alias = "ftKp_Init_OnLoadForGKoopa")]
    pub fn gk_on_load(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let d = self.game.fighters[f.0]
            .ft_data
            .expect("a fighter without data");
        let items = self.data.ptr(d.at(0x48)).expect("ftData::x48_items");
        self.push_attrs(f, attr::SIZE);
        let article = self.data.ptr(items.at(0));
        super::super::it::item::set_char_article(self.game, article, KOOPA_FLAME);
        let fp = &mut self.game.fighters[f.0];
        fp.x2226_b1 = true;
        fp.x222A_b0 = true;
    }

    /// Picking up an item, `ftGk_Init_OnItemPickup` (`Fighter_OnItemPickup(gobj, flag, 1,
    /// 1)`, as Bowser's).
    #[doc(alias = "ftGk_Init_OnItemPickup")]
    pub fn gk_on_item_pickup(&mut self, gobj: GObjId, flag: bool) {
        self.item_pickup_hand(gobj, flag, 1);
    }

    /// Hiding the held item, `ftGk_Init_OnItemInvisible` (as Bowser's).
    #[doc(alias = "ftGk_Init_OnItemInvisible")]
    pub fn gk_on_item_invisible(&mut self, gobj: GObjId) {
        self.item_invisible_hand(gobj, 1);
    }

    /// Showing the held item again, `ftGk_Init_OnItemVisible` (as Bowser's).
    #[doc(alias = "ftGk_Init_OnItemVisible")]
    pub fn gk_on_item_visible(&mut self, gobj: GObjId) {
        self.item_visible_hand(gobj, 1);
    }

    /// Dropping the item, `ftGk_Init_OnItemDrop` (as Bowser's).
    #[doc(alias = "ftGk_Init_OnItemDrop")]
    pub fn gk_on_item_drop(&mut self, gobj: GObjId, flag: bool) {
        self.item_drop_hand(gobj, flag, 1);
    }

    /// `ftGk_Init_LoadSpecialAttrs`: Bowser's (his attributes copied again).
    #[doc(alias = "ftGk_Init_LoadSpecialAttrs")]
    pub fn gk_load_special_attrs(&mut self, gobj: GObjId) {
        self.kp_load_special_attrs(gobj);
    }

    /// Entering knockback, `ftGk_Init_OnKnockbackEnter`: the hurt textures, as Bowser's.
    #[doc(alias = "ftGk_Init_OnKnockbackEnter")]
    pub fn gk_on_knockback_enter(&mut self, gobj: GObjId) {
        self.seak_on_knockback_enter(gobj);
    }

    /// Leaving knockback, `ftGk_Init_OnKnockbackExit`: as Bowser's.
    #[doc(alias = "ftGk_Init_OnKnockbackExit")]
    pub fn gk_on_knockback_exit(&mut self, gobj: GObjId) {
        self.seak_on_knockback_exit(gobj);
    }
}

/// Runs one of Giga Bowser's callbacks a fighter holds (`void (*)(HSD_GObj*)`), if `a` is one.
pub fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::UNK_MOTION_STATES3 => g.gk_unk_motion_states3(gobj),
        addr::ON_ITEM_INVISIBLE => g.gk_on_item_invisible(gobj),
        addr::ON_ITEM_VISIBLE => g.gk_on_item_visible(gobj),
        addr::ON_KNOCKBACK_ENTER => g.gk_on_knockback_enter(gobj),
        addr::ON_KNOCKBACK_EXIT => g.gk_on_knockback_exit(gobj),
        _ => return false,
    }
    true
}

/// Giga Bowser's functions other code calls by address (`code::call`).
pub fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    let gobj = |i: usize| args[i].gobj().expect("a null fighter");
    match f {
        addr::ON_ITEM_PICKUP => r.gk_on_item_pickup(gobj(0), args[1].int() != 0),
        addr::ON_ITEM_DROP => r.gk_on_item_drop(gobj(0), args[1].int() != 0),
        _ => match args.first() {
            Some(&Val::GObj(Some(g))) if args.len() == 1 && run(r, g, f) => {}
            _ => return None,
        },
    }
    Some(Val::None)
}
