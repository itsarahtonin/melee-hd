//! Pichu's own code (`ft/kinds/ftPichu`): only the character's callbacks. His moves are
//! Pikachu's ([`super::pikachu`]), which tell the two apart where they differ.

use super::super::code::Val;
use super::super::state::Running;
use crate::hsd::gobj::GObjId;

/// Pichu's functions, by address.
pub mod addr {
    pub const ON_LOAD: u32 = 0x8014_9E34;
    pub const ON_DEATH: u32 = 0x8014_9EAC;
    pub const ON_ITEM_PICKUP: u32 = 0x8014_9FE4;
    pub const ON_ITEM_INVISIBLE: u32 = 0x8014_A0C4;
    pub const ON_ITEM_VISIBLE: u32 = 0x8014_A10C;
    pub const ON_ITEM_DROP: u32 = 0x8014_A154;
    pub const LOAD_SPECIAL_ATTRS: u32 = 0x8014_A1A8;
    pub const ON_KNOCKBACK_ENTER: u32 = 0x8014_A1C8;
    pub const ON_KNOCKBACK_EXIT: u32 = 0x8014_A20C;
}

impl Running<'_> {
    /// Pichu's creation, `ftPc_Init_OnLoad`: he can wall jump, his attributes in Pikachu's
    /// layout, and his articles.
    #[doc(alias = "ftPc_Init_OnLoad")]
    pub fn pc_on_load(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.game.fighters[f.0].can_walljump = true;
        self.pk_on_load_for_pichu(f);
        self.pk_set_articles(f);
    }

    /// Pichu's spawn, `ftPc_Init_OnDeath`: his first model variant, and the costume's
    /// accessory (goggles, a bow or a hat) for the costumes that have one.
    #[doc(alias = "ftPc_Init_OnDeath")]
    pub fn pc_on_death(&mut self, gobj: GObjId) {
        self.game.set_part_vis(gobj, 0, 0);
        let shown = match self.game.fighter(gobj).costume_id {
            0 => None,
            1 => Some(1),
            2 => Some(2),
            3 => Some(3),
            _ => return,
        };
        for model in 1..=3 {
            let v = if shown == Some(model) { 0 } else { -1 };
            self.game.set_part_vis(gobj, model, v);
        }
    }

    /// `ftPc_Init_OnKnockbackEnter`: the hurt textures, as Sheik's.
    #[doc(alias = "ftPc_Init_OnKnockbackEnter")]
    pub fn pc_on_knockback_enter(&mut self, gobj: GObjId) {
        self.seak_on_knockback_enter(gobj);
    }

    /// `ftPc_Init_OnKnockbackExit`.
    #[doc(alias = "ftPc_Init_OnKnockbackExit")]
    pub fn pc_on_knockback_exit(&mut self, gobj: GObjId) {
        self.seak_on_knockback_exit(gobj);
    }
}

/// Runs one of Pichu's callbacks a fighter holds, if `addr` is one.
pub fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        // `ftPc_Init_OnItemInvisible` and `_OnItemVisible`: as Pikachu's.
        addr::ON_ITEM_INVISIBLE => g.pk_on_item_invisible(gobj),
        addr::ON_ITEM_VISIBLE => g.pk_on_item_visible(gobj),
        // `ftPc_Init_LoadSpecialAttrs`: Pikachu's.
        addr::LOAD_SPECIAL_ATTRS => g.pk_load_special_attrs(gobj),
        addr::ON_KNOCKBACK_ENTER => g.pc_on_knockback_enter(gobj),
        addr::ON_KNOCKBACK_EXIT => g.pc_on_knockback_exit(gobj),
        _ => return false,
    }
    true
}

/// Pichu's functions other code calls by address (`code::call`).
pub fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    let gobj = |i: usize| args[i].gobj().expect("a null fighter");
    match f {
        // `ftPc_Init_OnItemPickup` and `_OnItemDrop`: as Pikachu's.
        addr::ON_ITEM_PICKUP => {
            r.pk_on_item_pickup(gobj(0), args[1].int() != 0);
            Some(Val::None)
        }
        addr::ON_ITEM_DROP => {
            r.pk_on_item_drop(gobj(0), args[1].int() != 0);
            Some(Val::None)
        }
        _ => match args.first() {
            Some(&Val::GObj(Some(g))) if args.len() == 1 && run(r, g, f) => Some(Val::None),
            _ => None,
        },
    }
}
