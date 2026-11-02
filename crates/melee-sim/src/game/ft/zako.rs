//! The Fighting Wire Frames' callbacks, the male's (`ft/kinds/ftZakoBoy/ftboy.c`) and the
//! female's (`ft/kinds/ftZakoGirl/ftgirl.c`). They have no state table and no specials, only
//! the common states; their attributes are one word, and their item hooks take hand 0 where
//! Fox's and Bowser's take hand 1.

use super::super::code::Val;
use super::super::state::Running;
use crate::hsd::gobj::GObjId;

/// `_ftZakoboyAttributes` and the female's attributes (an `int`).
const ATTRS_SIZE: usize = 0x4;

/// The Wire Frames' functions, by address.
pub mod addr {
    pub const BOY_ON_DEATH: u32 = 0x8014_F1F0;
    pub const BOY_ON_ITEM_PICKUP: u32 = 0x8014_F218;
    pub const BOY_ON_ITEM_INVISIBLE: u32 = 0x8014_F2F8;
    pub const BOY_ON_ITEM_VISIBLE: u32 = 0x8014_F340;
    pub const BOY_ON_ITEM_DROP: u32 = 0x8014_F388;
    pub const BOY_ON_LOAD: u32 = 0x8014_F3DC;
    pub const BOY_LOAD_SPECIAL_ATTRS: u32 = 0x8014_F3FC;
    pub const GIRL_ON_DEATH: u32 = 0x8014_F418;
    pub const GIRL_ON_LOAD: u32 = 0x8014_F440;
    pub const GIRL_ON_ITEM_PICKUP: u32 = 0x8014_F460;
    pub const GIRL_ON_ITEM_INVISIBLE: u32 = 0x8014_F540;
    pub const GIRL_ON_ITEM_VISIBLE: u32 = 0x8014_F588;
    pub const GIRL_ON_ITEM_DROP: u32 = 0x8014_F5D0;
    pub const GIRL_LOAD_SPECIAL_ATTRS: u32 = 0x8014_F624;
}

impl Running<'_> {
    /// A Wire Frame's spawn, `ftBo_Init_OnDeath` and `ftGl_Init_OnDeath`: the first model
    /// variant.
    #[doc(alias = "ftBo_Init_OnDeath")]
    #[doc(alias = "ftGl_Init_OnDeath")]
    pub fn zako_on_death(&mut self, gobj: GObjId) {
        self.game.set_part_vis(gobj, 0, 0);
    }

    /// A Wire Frame's creation, `ftBo_Init_OnLoad` and `ftGl_Init_OnLoad`: the attributes
    /// pushed.
    #[doc(alias = "ftBo_Init_OnLoad")]
    #[doc(alias = "ftGl_Init_OnLoad")]
    pub fn zako_on_load(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.push_attrs(f, ATTRS_SIZE);
    }

    /// `ftBo_Init_LoadSpecialAttrs` and `ftGl_Init_LoadSpecialAttrs`: the attributes copied
    /// again.
    #[doc(alias = "ftBo_Init_LoadSpecialAttrs")]
    #[doc(alias = "ftGl_Init_LoadSpecialAttrs")]
    pub fn zako_load_special_attrs(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.copy_attrs(f, ATTRS_SIZE);
    }

    /// Picking up an item, `ftBo_Init_OnItemPickup` and `ftGl_Init_OnItemPickup`
    /// (`Fighter_OnItemPickup(gobj, flag, 0, 0)`).
    #[doc(alias = "ftBo_Init_OnItemPickup")]
    #[doc(alias = "ftGl_Init_OnItemPickup")]
    pub fn zako_on_item_pickup(&mut self, gobj: GObjId, flag: bool) {
        self.item_pickup_hand(gobj, flag, 0);
    }

    /// Hiding the held item, `ftBo_Init_OnItemInvisible` and `ftGl_Init_OnItemInvisible`.
    /// Only their knockdowns' scripts (`DownBoundU`, `DownBoundD`) change a held item's
    /// visibility.
    // Ports ftBo_Init_OnItemInvisible and ftGl_Init_OnItemInvisible; aliased once a check
    // reaches them.
    pub fn zako_on_item_invisible(&mut self, gobj: GObjId) {
        self.item_invisible_hand(gobj, 0);
    }

    /// Showing the held item again, `ftBo_Init_OnItemVisible` and `ftGl_Init_OnItemVisible`.
    // Ports ftBo_Init_OnItemVisible and ftGl_Init_OnItemVisible; aliased once a check reaches
    // them.
    pub fn zako_on_item_visible(&mut self, gobj: GObjId) {
        self.item_visible_hand(gobj, 0);
    }

    /// Dropping the item, `ftBo_Init_OnItemDrop` and `ftGl_Init_OnItemDrop`.
    #[doc(alias = "ftBo_Init_OnItemDrop")]
    #[doc(alias = "ftGl_Init_OnItemDrop")]
    pub fn zako_on_item_drop(&mut self, gobj: GObjId, flag: bool) {
        self.item_drop_hand(gobj, flag, 0);
    }
}

/// Runs one of the Wire Frames' callbacks a fighter holds (`void (*)(HSD_GObj*)`), if `a` is
/// one.
pub fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::BOY_ON_ITEM_INVISIBLE | addr::GIRL_ON_ITEM_INVISIBLE => {
            g.zako_on_item_invisible(gobj)
        }
        addr::BOY_ON_ITEM_VISIBLE | addr::GIRL_ON_ITEM_VISIBLE => g.zako_on_item_visible(gobj),
        _ => return false,
    }
    true
}

/// The Wire Frames' functions other code calls by address (`code::call`).
pub fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    let gobj = |i: usize| args[i].gobj().expect("a null fighter");
    match f {
        addr::BOY_ON_ITEM_PICKUP | addr::GIRL_ON_ITEM_PICKUP => {
            r.zako_on_item_pickup(gobj(0), args[1].int() != 0)
        }
        addr::BOY_ON_ITEM_DROP | addr::GIRL_ON_ITEM_DROP => {
            r.zako_on_item_drop(gobj(0), args[1].int() != 0)
        }
        _ => match args.first() {
            Some(&Val::GObj(Some(g))) if args.len() == 1 && run(r, g, f) => {}
            _ => return None,
        },
    }
    Some(Val::None)
}
