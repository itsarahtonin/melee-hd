//! Bowser's character callbacks (`ftkoopa.c`): creation, spawn and attributes, the fire
//! breath's refill each frame, and the item and knockback hooks.

use super::super::super::code::Val;
use super::super::super::it::kinds::KOOPA_FLAME;
use super::super::super::state::Running;
use super::{addr, attr, u};
use crate::hsd::gobj::GObjId;

impl Running<'_> {
    /// Bowser's spawn, `ftKp_Init_OnDeath`: his first model variant, his super armor, and the
    /// flame at full speed and size.
    #[doc(alias = "ftKp_Init_OnDeath")]
    pub fn kp_on_death(&mut self, gobj: GObjId) {
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

    /// `ftKp_Init_UnkMotionStates3` (`ftData_UnkMotionStates3`, each frame before the state's
    /// callbacks): the flame's refill.
    #[doc(alias = "ftKp_Init_UnkMotionStates3")]
    pub fn kp_unk_motion_states3(&mut self, gobj: GObjId) {
        self.kp_refuel(gobj);
    }

    /// Bowser's creation, `ftKp_Init_OnLoad`: his attributes, his flame registered, and
    /// `x2226_b1`.
    #[doc(alias = "ftKp_Init_OnLoad")]
    pub fn kp_on_load(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let d = self.game.fighters[f.0]
            .ft_data
            .expect("a fighter without data");
        let items = self.data.ptr(d.at(0x48)).expect("ftData::x48_items");
        self.push_attrs(f, attr::SIZE);
        let article = self.data.ptr(items.at(0));
        super::super::super::it::item::set_char_article(self.game, article, KOOPA_FLAME);
        self.game.fighters[f.0].x2226_b1 = true;
    }

    /// The up special's damage and death hook, `ftKp_Init_80132B38`: nothing.
    #[doc(alias = "ftKp_Init_80132B38")]
    pub fn kp_damage_cb(&mut self, _gobj: GObjId) {}

    /// Picking up an item, `ftKp_Init_OnItemPickup` (`Fighter_OnItemPickup(gobj, flag, 1,
    /// 1)`, as Fox's).
    // Ports `ftKp_Init_OnItemPickup`; aliased once a check reaches it.
    pub fn kp_on_item_pickup(&mut self, gobj: GObjId, flag: bool) {
        self.item_pickup_hand(gobj, flag, 1);
    }

    /// Hiding the held item, `ftKp_Init_OnItemInvisible` (as Fox's).
    // Ports `ftKp_Init_OnItemInvisible`; aliased once a check reaches it.
    pub fn kp_on_item_invisible(&mut self, gobj: GObjId) {
        self.item_invisible_hand(gobj, 1);
    }

    /// Showing the held item again, `ftKp_Init_OnItemVisible` (as Fox's).
    // Ports `ftKp_Init_OnItemVisible`; aliased once a check reaches it.
    pub fn kp_on_item_visible(&mut self, gobj: GObjId) {
        self.item_visible_hand(gobj, 1);
    }

    /// Dropping the item, `ftKp_Init_OnItemDrop` (as Fox's).
    // Ports `ftKp_Init_OnItemDrop`; aliased once a check reaches it.
    pub fn kp_on_item_drop(&mut self, gobj: GObjId, flag: bool) {
        self.item_drop_hand(gobj, flag, 1);
    }

    /// `ftKp_Init_LoadSpecialAttrs`: his attributes copied again.
    #[doc(alias = "ftKp_Init_LoadSpecialAttrs")]
    pub fn kp_load_special_attrs(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.copy_attrs(f, attr::SIZE);
    }

    /// Entering knockback, `ftKp_Init_OnKnockbackEnter`: the hurt textures, as Sheik's.
    #[doc(alias = "ftKp_Init_OnKnockbackEnter")]
    pub fn kp_on_knockback_enter(&mut self, gobj: GObjId) {
        self.seak_on_knockback_enter(gobj);
    }

    /// Leaving knockback, `ftKp_Init_OnKnockbackExit`: as Sheik's.
    #[doc(alias = "ftKp_Init_OnKnockbackExit")]
    pub fn kp_on_knockback_exit(&mut self, gobj: GObjId) {
        self.seak_on_knockback_exit(gobj);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::UNK_MOTION_STATES3 => g.kp_unk_motion_states3(gobj),
        addr::DAMAGE_CB => g.kp_damage_cb(gobj),
        addr::ON_ITEM_INVISIBLE => g.kp_on_item_invisible(gobj),
        addr::ON_ITEM_VISIBLE => g.kp_on_item_visible(gobj),
        addr::ON_KNOCKBACK_ENTER => g.kp_on_knockback_enter(gobj),
        addr::ON_KNOCKBACK_EXIT => g.kp_on_knockback_exit(gobj),
        _ => return false,
    }
    true
}

pub(super) fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    let gobj = |i: usize| args[i].gobj().expect("a null fighter");
    Some(match f {
        addr::ON_ITEM_PICKUP => {
            r.kp_on_item_pickup(gobj(0), args[1].int() != 0);
            Val::None
        }
        addr::ON_ITEM_DROP => {
            r.kp_on_item_drop(gobj(0), args[1].int() != 0);
            Val::None
        }
        _ => return None,
    })
}
