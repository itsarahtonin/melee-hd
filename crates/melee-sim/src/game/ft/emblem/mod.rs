//! Roy's own code (`ft/kinds/ftEmblem`): only the character's callbacks. His moves are
//! Marth's ([`super::mars`]), which tell the two apart where they differ (the specials'
//! effects, Flare Blade's charge flash, Counter's damage).

use super::super::code::Val;
use super::super::state::Running;
use crate::hsd::gobj::GObjId;

/// Roy's functions, by address.
pub mod addr {
    pub const ON_DEATH: u32 = 0x8014_EEF8;
    pub const ON_ITEM_PICKUP: u32 = 0x8014_EF60;
    pub const ON_ITEM_INVISIBLE: u32 = 0x8014_F040;
    pub const ON_ITEM_VISIBLE: u32 = 0x8014_F088;
    pub const ON_ITEM_DROP: u32 = 0x8014_F0D0;
    pub const ON_LOAD: u32 = 0x8014_F124;
    pub const LOAD_SPECIAL_ATTRS: u32 = 0x8014_F148;
    pub const ON_KNOCKBACK_ENTER: u32 = 0x8014_F168;
    pub const ON_KNOCKBACK_EXIT: u32 = 0x8014_F1AC;
}

impl Running<'_> {
    /// Roy's creation, `ftFe_Init_OnLoad`: Marth's attributes, through
    /// `ftMs_Init_OnLoadForRoy`.
    #[doc(alias = "ftFe_Init_OnLoad")]
    pub fn fe_on_load(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.mars_on_load_for_roy(f);
    }

    /// Roy's spawn, `ftFe_Init_OnDeath`: Marth's first two model variants, the third hidden,
    /// and `x222C` cleared.
    #[doc(alias = "ftFe_Init_OnDeath")]
    pub fn fe_on_death(&mut self, gobj: GObjId) {
        self.game.set_part_vis(gobj, 0, 0);
        self.game.set_part_vis(gobj, 1, 0);
        self.game.set_part_vis(gobj, 2, -1);
        let fp = self.game.fighter_mut(gobj);
        fp.u[..4].copy_from_slice(&[0; 4]);
    }

    /// `ftFe_Init_LoadSpecialAttrs`: Marth's.
    #[doc(alias = "ftFe_Init_LoadSpecialAttrs")]
    pub fn fe_load_special_attrs(&mut self, gobj: GObjId) {
        self.mars_load_special_attrs(gobj);
    }

    /// `ftFe_Init_OnItemPickup`: as Marth's (`Fighter_OnItemPickup(gobj, flag, 0, 1)`).
    #[doc(alias = "ftFe_Init_OnItemPickup")]
    pub fn fe_on_item_pickup(&mut self, gobj: GObjId, flag: bool) {
        self.mars_on_item_pickup(gobj, flag);
    }

    /// `ftFe_Init_OnItemDrop`: as Marth's (`Fighter_OnItemDrop(gobj, flag, 0, 1)`).
    #[doc(alias = "ftFe_Init_OnItemDrop")]
    pub fn fe_on_item_drop(&mut self, gobj: GObjId, flag: bool) {
        self.mars_on_item_drop(gobj, flag);
    }

    /// `ftFe_Init_OnItemInvisible`: as Marth's.
    #[doc(alias = "ftFe_Init_OnItemInvisible")]
    pub fn fe_on_item_invisible(&mut self, gobj: GObjId) {
        self.mars_on_item_invisible(gobj);
    }

    /// `ftFe_Init_OnItemVisible`: as Marth's.
    #[doc(alias = "ftFe_Init_OnItemVisible")]
    pub fn fe_on_item_visible(&mut self, gobj: GObjId) {
        self.mars_on_item_visible(gobj);
    }

    /// `ftFe_Init_OnKnockbackEnter`: the hurt textures, as Marth's.
    #[doc(alias = "ftFe_Init_OnKnockbackEnter")]
    pub fn fe_on_knockback_enter(&mut self, gobj: GObjId) {
        self.mars_on_knockback_enter(gobj);
    }

    /// `ftFe_Init_OnKnockbackExit`.
    #[doc(alias = "ftFe_Init_OnKnockbackExit")]
    pub fn fe_on_knockback_exit(&mut self, gobj: GObjId) {
        self.mars_on_knockback_exit(gobj);
    }
}

/// Runs one of Roy's callbacks a fighter holds, if `addr` is one.
pub fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::ON_ITEM_INVISIBLE => g.fe_on_item_invisible(gobj),
        addr::ON_ITEM_VISIBLE => g.fe_on_item_visible(gobj),
        addr::LOAD_SPECIAL_ATTRS => g.fe_load_special_attrs(gobj),
        addr::ON_KNOCKBACK_ENTER => g.fe_on_knockback_enter(gobj),
        addr::ON_KNOCKBACK_EXIT => g.fe_on_knockback_exit(gobj),
        _ => return false,
    }
    true
}

/// Roy's functions other code calls by address (`code::call`).
pub fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    let gobj = |i: usize| args[i].gobj().expect("a null fighter");
    match f {
        addr::ON_ITEM_PICKUP => {
            r.fe_on_item_pickup(gobj(0), args[1].int() != 0);
            Some(Val::None)
        }
        addr::ON_ITEM_DROP => {
            r.fe_on_item_drop(gobj(0), args[1].int() != 0);
            Some(Val::None)
        }
        _ => match args.first() {
            Some(&Val::GObj(Some(g))) if args.len() == 1 && run(r, g, f) => Some(Val::None),
            _ => None,
        },
    }
}
