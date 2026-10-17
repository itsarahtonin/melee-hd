//! Mr. Game & Watch's character callbacks (`ftgamewatch.c`): creation (his flat model, its
//! color and outline), spawn, his items let go of when he's hit, KO'd or leaves the ground,
//! the item and absorb hooks, the full bucket's glow and his items' colors.

use super::super::super::code::Val;
use super::super::super::handles::FighterId;
use super::super::super::state::Running;
use super::super::super::structs::_GXColor;
use super::super::common::GA_AIR;
use super::{ITEMS, attr, panic, u};
use crate::hsd::Objects;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::hsd::tobj::Color;

/// Sets every material's diffuse color in a model, `ftMaterial_800BFB4C` (and the items'
/// `it_80278574`, the same walk): each joint's display objects, then its children unless it's
/// an instance, then its next sibling, up the parents until one has a next.
#[doc(alias = "ftMaterial_800BFB4C")]
#[doc(alias = "it_80278574")]
pub(crate) fn set_diffuse_all(objs: &mut Objects, root: Option<JObjId>, c: Color) {
    // The walk reads the joints; the colors are the materials'.
    let (jobjs, dobjs, mobjs) = (&objs.jobjs, &objs.dobjs, &mut objs.mobjs);
    jobj::walk_tree(jobjs, root, &mut |j, _| {
        let mut d = jobjs[j].dobj();
        while let Some(di) = d {
            if let Some(m) = dobjs[di].mobj
                && let Some(mat) = mobjs[m].mat.as_mut()
            {
                mat.diffuse = c;
            }
            d = dobjs[di].next;
        }
    });
}

impl Running<'_> {
    /// A color of his attributes, `off` bytes in.
    fn gw_attr_color(&self, f: FighterId, off: usize) -> _GXColor {
        let w = self.gw_attr_i32(f, off) as u32;
        let [r, g, b, a] = w.to_be_bytes();
        _GXColor { r, g, b, a }
    }

    /// His costume's color, `ftGw_Init_8014A7F4` (`ftLib_GetGameWatchColor`), which his
    /// items take.
    #[doc(alias = "ftGw_Init_8014A7F4")]
    pub fn gw_color(&self, gobj: GObjId) -> _GXColor {
        let f = self.game.fighter_id(gobj);
        let costume = usize::from(self.game.fighters[f.0].costume_id);
        self.gw_attr_color(f, attr::COLOR + 4 * costume)
    }

    /// His outline's color, `ftGw_Init_8014A814` (`ftLib_GetGameWatchOutlineColor`).
    #[doc(alias = "ftGw_Init_8014A814")]
    pub fn gw_outline_color(&self, gobj: GObjId) -> _GXColor {
        let f = self.game.fighter_id(gobj);
        self.gw_attr_color(f, attr::OUTLINE)
    }

    /// His spawn, `ftGw_Init_OnDeath`: his model variants for no item (the bucket empty),
    /// Judgement and Chef from their first numbers, and no items out.
    #[doc(alias = "ftGw_Init_OnDeath")]
    pub fn gw_on_death(&mut self, gobj: GObjId) {
        for (model, v) in [
            (0, 0),
            (1, -1),
            (2, 0),
            (3, 0),
            (4, -1),
            (5, -1),
            (6, -1),
            (7, -1),
            (8, -1),
            (9, -1),
            (10, -1),
        ] {
            self.game.set_part_vis(gobj, model, v);
        }
        let fp = self.game.fighter_mut(gobj);
        fp.set_u_i32(u::JUDGE_VAR1, 1);
        fp.set_u_i32(u::JUDGE_VAR2, 0);
        fp.set_u_i32(u::X2234, 0);
        fp.set_u_i32(u::PANIC_DAMAGE, 0);
        fp.set_u_i32(u::CHEF_VAR1, 1);
        fp.set_u_i32(u::CHEF_VAR2, 3);
        for off in u::ITEMS {
            fp.u.set_gobj_at(off, None);
        }
    }

    /// His creation, `ftGw_Init_OnLoad`: his attributes, an empty bucket, his model's depth,
    /// outline and costume color, the bucket's model lookup, and his ten items registered.
    #[doc(alias = "ftGw_Init_OnLoad")]
    pub fn gw_on_load(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let d = self.game.fighters[f.0]
            .ft_data
            .expect("a fighter without data");
        let items = self.data.ptr(d.at(0x48)).expect("ftData::x48_items");
        let fp = &mut self.game.fighters[f.0];
        fp.x2222_b6 = false;
        fp.x2223_b1 = true;
        fp.x2224_b0 = true;
        self.push_attrs(f, attr::SIZE);
        self.game.fighters[f.0].set_u_i32(u::PANIC_CHARGE, panic::EMPTY);
        let width = self.gw_attr_f32(f, attr::WIDTH);
        let outline = self.gw_attr_color(f, attr::OUTLINE);
        let fp = &mut self.game.fighters[f.0];
        fp.x34_scale.z = width;
        fp.x610_color_rgba[1] = outline;
        let c = self.gw_color(gobj);
        let root = self.root_jobj(gobj);
        set_diffuse_all(
            &mut self.game.objs,
            Some(root),
            Color {
                r: c.r,
                g: c.g,
                b: c.b,
                a: c.a,
            },
        );
        let bucket = self.data.ptr(items.at(4 * 10));
        self.game.fighters[f.0].x5AC.xC[4] = bucket;
        for (i, kind) in ITEMS.into_iter().enumerate() {
            let article = self.data.ptr(items.at(4 * i as u32));
            super::super::super::it::item::set_char_article(self.game, article, kind);
        }
    }

    /// His items let go of, `ftGw_Init_OnDamage`: the hook his moves set for being hit and
    /// KO'd.
    #[doc(alias = "ftGw_Init_OnDamage")]
    pub fn gw_on_damage(&mut self, gobj: GObjId) {
        self.gw_greenhouse_remove(gobj);
        self.gw_manhole_on_damage(gobj);
        self.gw_torch_on_damage(gobj);
        self.gw_parachute_remove(gobj);
        self.gw_turtle_remove(gobj);
        self.gw_sparky_remove(gobj);
        self.gw_judgement_remove(gobj);
        self.gw_panic_remove(gobj);
        self.gw_rescue_remove(gobj);
    }

    /// His items let go of once he's in the air, `ftGw_Init_8014A538`: his grounded moves'
    /// collision.
    #[doc(alias = "ftGw_Init_8014A538")]
    pub fn gw_drop_items_in_air(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).ground_or_air == GA_AIR {
            self.gw_on_damage(gobj);
        }
    }

    /// Picking up an item, `ftGw_Init_OnItemPickup` (as Fox's).
    // Ports `ftGw_Init_OnItemPickup`; aliased once a check reaches it.
    pub fn gw_on_item_pickup(&mut self, gobj: GObjId, flag: bool) {
        self.fox_on_item_pickup(gobj, flag);
    }

    /// Hiding the held item, `ftGw_Init_OnItemInvisible` (as Fox's).
    // Ports `ftGw_Init_OnItemInvisible`; aliased once a check reaches it.
    pub fn gw_on_item_invisible(&mut self, gobj: GObjId) {
        self.fox_on_item_invisible(gobj);
    }

    /// Showing the held item again, `ftGw_Init_OnItemVisible` (as Fox's).
    // Ports `ftGw_Init_OnItemVisible`; aliased once a check reaches it.
    pub fn gw_on_item_visible(&mut self, gobj: GObjId) {
        self.fox_on_item_visible(gobj);
    }

    /// Dropping the item, `ftGw_Init_OnItemDrop` (as Fox's).
    // Ports `ftGw_Init_OnItemDrop`; aliased once a check reaches it.
    pub fn gw_on_item_drop(&mut self, gobj: GObjId, flag: bool) {
        self.fox_on_item_drop(gobj, flag);
    }

    /// The full bucket's glow, `ftGw_Init_UnkMotionStates4` (`ftData_UnkMotionStates4`).
    #[doc(alias = "ftGw_Init_UnkMotionStates4")]
    pub fn gw_unk_motion_states4(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).u_i32(u::PANIC_CHARGE) >= panic::FULL {
            self.start_color_anim(gobj, 5, 0);
        }
    }

    /// `ftGw_Init_LoadSpecialAttrs`.
    #[doc(alias = "ftGw_Init_LoadSpecialAttrs")]
    pub fn gw_load_special_attrs(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.copy_attrs(f, attr::SIZE);
    }

    /// Absorbing an item, `ftGw_Init_OnAbsorb`: Oil Panic's catch.
    #[doc(alias = "ftGw_Init_OnAbsorb")]
    pub fn gw_on_absorb(&mut self, gobj: GObjId) {
        self.gw_absorb_decide_action(gobj);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    use super::addr;
    match a {
        addr::INIT_ON_DAMAGE => g.gw_on_damage(gobj),
        addr::INIT_8014A538 => g.gw_drop_items_in_air(gobj),
        addr::INIT_ON_ITEM_INVISIBLE => g.gw_on_item_invisible(gobj),
        addr::INIT_ON_ITEM_VISIBLE => g.gw_on_item_visible(gobj),
        addr::INIT_UNK_MOTION_STATES4 => g.gw_unk_motion_states4(gobj),
        addr::INIT_ON_ABSORB => g.gw_on_absorb(gobj),
        _ => return false,
    }
    true
}

pub(super) fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    use super::addr;
    let gobj = |i: usize| args[i].gobj().expect("a null fighter");
    Some(match f {
        addr::INIT_ON_ITEM_PICKUP => {
            r.gw_on_item_pickup(gobj(0), args[1].int() != 0);
            Val::None
        }
        addr::INIT_ON_ITEM_DROP => {
            r.gw_on_item_drop(gobj(0), args[1].int() != 0);
            Val::None
        }
        _ => return None,
    })
}
