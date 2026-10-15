//! Yoshi's character callbacks (`ftyoshi.c`): creation, spawn and attributes, the egg shield's
//! texture animation, his eggs' cleanup when he's hit or KO'd, and the item and knockback hooks.

use gekko_fp as fp;
use melee_data::archive::DataRef;

use super::super::super::code::Val;
use super::super::super::handles::FighterId;
use super::super::super::state::Running;
use super::{ITEMS, addr, attr, u};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::{aobj, mobj};

impl Running<'_> {
    /// The DObjs one of the fighter's model lookups (`x5AC.xC[i]`, `S_UNK_YOSHI1`) names for
    /// the egg shield's texture animation: its second variant's indices into the DObj list.
    fn ys_shield_mobjs(&self, f: FighterId, lookup: DataRef) -> Vec<crate::hsd::mobj::MObjId> {
        let unk = self
            .data
            .ptr(lookup.at(4))
            .expect("S_UNK_YOSHI1::unk_struct");
        let end = self.data.u32(unk.at(8)) as i32;
        let index = self.data.ptr(unk.at(0xC));
        let list = self.game.fighters[f.0]
            .dobj_list
            .data
            .expect("a fighter without a DObj list");
        (0..end)
            .map(|i| {
                let b = index.expect("S_UNK_YOSHI2::xC_start_index").at(i as u32);
                let d = self.game.fighter_dobj_lists[list.0][usize::from(self.data.u8(b))];
                // `dobj ? dobj->mobj : NULL`, then read through.
                let d = d.expect("the egg shield's DObj is null");
                self.game.objs.dobjs[d]
                    .mobj
                    .expect("the egg shield's DObj has no material")
            })
            .collect()
    }

    /// `ftYs_Init_8012B6E8`: stops the egg shield's texture animation in each DObj the lookup
    /// names, and returns its length (`HSD_AObjGetEndFrame`), which every one of them must
    /// share. The original keeps the length in his loaded attributes' `xC`, reset to 0 before
    /// the first lookup; `length` stands for it.
    #[doc(alias = "ftYs_Init_8012B6E8")]
    fn ys_stop_shield_anim(&mut self, f: FighterId, lookup: Option<DataRef>, length: &mut f32) {
        let lookup = lookup.expect("the egg shield's model lookup is null");
        for m in self.ys_shield_mobjs(f, lookup) {
            let a = self.game.objs.mobjs[m].aobj;
            aobj::set_rate(&mut self.game.objs.aobjs, a, 0.0);
            let end =
                self.game.objs.aobjs[a.expect("the egg shield's material has no AObj")].end_frame;
            if *length == 0.0 {
                *length = end;
            } else {
                assert!(
                    *length == end,
                    "the original's assertion failed: yoshi matanim frame not same"
                );
            }
        }
    }

    /// `ftYs_Init_8012B804`: the egg shield's texture animation at `frame` in each DObj the
    /// lookup names.
    #[doc(alias = "ftYs_Init_8012B804")]
    fn ys_shield_anim_frame(&mut self, f: FighterId, lookup: Option<DataRef>, frame: f32) {
        let Some(lookup) = lookup else { return };
        for m in self.ys_shield_mobjs(f, lookup) {
            mobj::req_anim(&mut self.game.objs, Some(m), frame);
            mobj::anim(&mut self.game.objs, self.data, Some(m));
        }
    }

    /// The egg shield's crack for the shield's health, `ftYs_Init_8012B8A4`.
    #[doc(alias = "ftYs_Init_8012B8A4")]
    pub fn ys_shield_crack(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let start = reg(self.common().f32(0x260));
        let fp = &self.game.fighters[f.0];
        let health = reg(fp.shield_health);
        let length = reg(self.ys_attr_f32(f, attr::XC));
        let frame = mem(fp::fmuls(length, fp::fsubs(1.0, fp::fdivs(health, start))));
        let xc = self.game.fighters[f.0].x5AC.xC;
        self.ys_shield_anim_frame(f, xc[0], frame);
        self.ys_shield_anim_frame(f, xc[1], frame);
    }

    /// The egg shield whole again, `ftYs_Init_8012B918`.
    #[doc(alias = "ftYs_Init_8012B918")]
    pub fn ys_shield_mend(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let xc = self.game.fighters[f.0].x5AC.xC;
        self.ys_shield_anim_frame(f, xc[0], 0.0);
        self.ys_shield_anim_frame(f, xc[1], 0.0);
    }

    /// Yoshi's spawn, `ftYs_Init_OnDeath`: his first model variant and no egg in hand.
    #[doc(alias = "ftYs_Init_OnDeath")]
    pub fn ys_on_death(&mut self, gobj: GObjId) {
        self.game.set_part_vis(gobj, 0, 0);
        self.game.fighter_mut(gobj).u.set_gobj_at(u::EGG, None);
    }

    /// Yoshi's creation, `ftYs_Init_OnLoad`: the egg shield's texture animation stopped and its
    /// length worked out (which the original writes into his loaded attributes' `xC` before
    /// copying them, so the fighter's copy holds it; docs/engine.md), his attributes, his three
    /// items registered, and `x2226_b1`.
    #[doc(alias = "ftYs_Init_OnLoad")]
    pub fn ys_on_load(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let xc = self.game.fighters[f.0].x5AC.xC;
        assert!(
            xc[0].is_some(),
            "the original's assertion failed: yoshi parts_model NULL!!"
        );
        let mut length = 0.0;
        self.ys_stop_shield_anim(f, xc[0], &mut length);
        self.ys_stop_shield_anim(f, xc[1], &mut length);
        self.push_attrs(f, attr::SIZE);
        self.ys_patch_shield_length(f, length);
        let d = self.game.fighters[f.0]
            .ft_data
            .expect("a fighter without data");
        let items = self.data.ptr(d.at(0x48)).expect("ftData::x48_items");
        for (i, kind) in ITEMS.into_iter().enumerate() {
            let article = self.data.ptr(items.at(4 * i as u32));
            super::super::super::it::item::set_char_article(self.game, article, kind);
        }
        self.game.fighters[f.0].x2226_b1 = true;
    }

    /// The egg shield's texture length in the fighter's attributes' `xC`, where the original's
    /// copies of his loaded attributes put it.
    fn ys_patch_shield_length(&mut self, f: FighterId, length: f32) {
        let a = self.game.fighters[f.0]
            .dat_attrs
            .expect("a fighter without attributes");
        self.game.fighter_attrs[a.0][attr::XC..attr::XC + 4]
            .copy_from_slice(&length.to_bits().to_be_bytes());
    }

    /// The egg shield's texture length his creation found (`ext_attr->xC`), from his loaded
    /// attributes' first DObj named for the shield.
    fn ys_shield_length(&self, f: FighterId) -> f32 {
        let lookup = self.game.fighters[f.0].x5AC.xC[0].expect("yoshi parts_model NULL!!");
        let m = self.ys_shield_mobjs(f, lookup)[0];
        let a = self.game.objs.mobjs[m].aobj.expect("an AObj");
        self.game.objs.aobjs[a].end_frame
    }

    /// His eggs' cleanup when he's hit or KO'd, `ftYs_Init_8012BA8C`: the egg in hand
    /// dropped, the item on his tongue let go.
    #[doc(alias = "ftYs_Init_8012BA8C")]
    pub fn ys_damage_cleanup(&mut self, gobj: GObjId) {
        self.ys_drop_egg(gobj);
        self.ys_release_tongue_item(gobj);
    }

    /// `ftYs_Init_8012BAC0`: `x120` of his attributes.
    #[doc(alias = "ftYs_Init_8012BAC0")]
    pub fn ys_attr_x120(&self, f: FighterId) -> f32 {
        self.ys_attr_f32(f, attr::X120)
    }

    /// `ftYs_Init_LoadSpecialAttrs`: his attributes copied again, the shield's length with
    /// them.
    #[doc(alias = "ftYs_Init_LoadSpecialAttrs")]
    pub fn ys_load_special_attrs(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.copy_attrs(f, attr::SIZE);
        let length = self.ys_shield_length(f);
        self.ys_patch_shield_length(f, length);
    }

    /// Picking up an item, `ftYs_Init_OnItemPickup` (as Fox's).
    // Ports `ftYs_Init_OnItemPickup`; aliased once a check reaches it.
    pub fn ys_on_item_pickup(&mut self, gobj: GObjId, flag: bool) {
        self.fox_on_item_pickup(gobj, flag);
    }

    /// Hiding the held item, `ftYs_Init_OnItemInvisible` (as Fox's).
    // Ports `ftYs_Init_OnItemInvisible`; aliased once a check reaches it.
    pub fn ys_on_item_invisible(&mut self, gobj: GObjId) {
        self.fox_on_item_invisible(gobj);
    }

    /// Showing the held item again, `ftYs_Init_OnItemVisible` (as Fox's).
    // Ports `ftYs_Init_OnItemVisible`; aliased once a check reaches it.
    pub fn ys_on_item_visible(&mut self, gobj: GObjId) {
        self.fox_on_item_visible(gobj);
    }

    /// Dropping the item, `ftYs_Init_OnItemDrop` (as Fox's).
    // Ports `ftYs_Init_OnItemDrop`; aliased once a check reaches it.
    pub fn ys_on_item_drop(&mut self, gobj: GObjId, flag: bool) {
        self.fox_on_item_drop(gobj, flag);
    }

    /// Entering knockback, `ftYs_Init_OnKnockbackEnter` (as Fox's).
    #[doc(alias = "ftYs_Init_OnKnockbackEnter")]
    pub fn ys_on_knockback_enter(&mut self, gobj: GObjId) {
        self.fox_on_knockback_enter(gobj);
    }

    /// Leaving knockback, `ftYs_Init_OnKnockbackExit` (as Fox's).
    #[doc(alias = "ftYs_Init_OnKnockbackExit")]
    pub fn ys_on_knockback_exit(&mut self, gobj: GObjId) {
        self.fox_on_knockback_exit(gobj);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::DAMAGE_CB => g.ys_damage_cleanup(gobj),
        addr::ON_ITEM_INVISIBLE => g.ys_on_item_invisible(gobj),
        addr::ON_ITEM_VISIBLE => g.ys_on_item_visible(gobj),
        addr::ON_KNOCKBACK_ENTER => g.ys_on_knockback_enter(gobj),
        addr::ON_KNOCKBACK_EXIT => g.ys_on_knockback_exit(gobj),
        _ => return false,
    }
    true
}

pub(super) fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    let gobj = |i: usize| args[i].gobj().expect("a null fighter");
    Some(match f {
        addr::ON_ITEM_PICKUP => {
            r.ys_on_item_pickup(gobj(0), args[1].int() != 0);
            Val::None
        }
        addr::ON_ITEM_DROP => {
            r.ys_on_item_drop(gobj(0), args[1].int() != 0);
            Val::None
        }
        _ => return None,
    })
}
