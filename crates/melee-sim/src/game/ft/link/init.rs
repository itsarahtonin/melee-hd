//! Link's character callbacks (`ftlink.c`): creation, spawn and attributes, his items' cleanup
//! when he's hit or KO'd, the item hooks, the knockback hooks and his shield's absorb push. Young
//! Link's creation is here too, and his callbacks that do what Link's do carry his aliases.

use gekko_fp as fp;

use super::super::super::code::Val;
use super::super::super::handles::FighterId;
use super::super::super::state::Running;
use super::super::bind::{JointStack, next_joint};
use super::super::create_model::Creator;
use super::super::parts::{bone_from_joint, set_bone_bit, set_bone_xd};
use super::{addr, attr, u};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;

/// The down aerial's hit animation (`ftData_80085E50(fp, 72)`), whose length his creation
/// writes into his loaded attributes (`attackairlw_hit_anim_frame_end`).
const ATTACK_AIR_LW_HIT_ANIM: i32 = 72;
/// `ft_PlaySFX`'s sound for the shield's absorb push.
const SFX_ABSORB: i32 = 0x2716A;

impl Running<'_> {
    /// Link's creation, `ftLk_Init_OnLoad`: the down aerial's hit animation's length, which the
    /// original writes into his loaded attributes (docs/engine.md), his attributes, his five
    /// items registered, and the optional part his `ftData` names hung on his model
    /// (`ftParts_800753D4`).
    #[doc(alias = "ftLk_Init_OnLoad")]
    pub fn link_on_load(&mut self, gobj: GObjId, by: Creator) {
        self.link_load(gobj, false, by);
    }

    /// Young Link's creation, `ftCl_Init_OnLoad`: he can wall jump, then Link's (his attributes
    /// pushed by `ftLk_Init_OnLoadForCLink`), with the milk his taunt makes registered too.
    #[doc(alias = "ftCl_Init_OnLoad")]
    #[doc(alias = "ftLk_Init_OnLoadForCLink")]
    pub fn clink_on_load(&mut self, gobj: GObjId, by: Creator) {
        self.game.fighter_mut(gobj).can_walljump = true;
        self.link_load(gobj, true, by);
    }

    /// Link's creation, or Young Link's with `clink`, from `by`.
    fn link_load(&mut self, gobj: GObjId, clink: bool, by: Creator) {
        let f = self.game.fighter_id(gobj);
        // `lbAnim_8001E8F8(ftData_80085E50(fp, 72))`: the animation goes through the second
        // buffer, which keeps it.
        let tree = self.second_figatree(f, ATTACK_AIR_LW_HIT_ANIM);
        let _ = super::super::data::figatree_frames(self.data, tree);
        self.push_attrs(f, attr::SIZE);
        self.link_patch_attack_air_lw_end(f);
        let d = self.game.fighters[f.0]
            .ft_data
            .expect("a fighter without data");
        let items = self.data.ptr(d.at(0x48)).expect("ftData::x48_items");
        let kinds = [attr::X48, attr::X2C, attr::XBC, attr::XC, attr::X10];
        for (i, off) in kinds.into_iter().enumerate() {
            let kind = self.link_attr_i32(f, off);
            let article = self.data.ptr(items.at(4 * i as u32));
            super::super::super::it::item::set_char_article(self.game, article, kind);
        }
        if clink {
            let article = self.data.ptr(items.at(20));
            let kind = super::super::super::it::kinds::CLINK_MILK;
            super::super::super::it::item::set_char_article(self.game, article, kind);
        }
        let joint = self.data.ptr(items.at(24)).expect("ftData::x48_items[6]");
        let kind = self.game.fighters[f.0].kind;
        let table = self.load_common(0x14);
        let entry = self
            .data
            .ptr(table.at(4 * kind as u32))
            .expect("Fighter_804D6540[kind]");
        let desc = self.data.ptr(entry).expect("Fighter_804D6540[kind]->x0");
        // `ftLk_Init_OnLoad` and `ftCl_Init_OnLoad` (0x28 each) call it from the creator's.
        let sp = match by {
            Creator::FighterCreate => FIGHTER_CREATE_SP - 0x48,
            Creator::Demo => DEMO_CREATE_SP - 0x30,
        };
        self.attach_part_model(f, desc, joint, sp - 0x28 - PARTS_COPY_BELOW_SP);
    }

    /// The down aerial's hit animation's length his creation writes over his loaded
    /// attributes' `attackairlw_hit_anim_frame_end`, put in the fighter's: the copies of those
    /// attributes (creation's and every later one) read it from there. It follows from his
    /// animation, so it's worked out here rather than kept.
    fn link_patch_attack_air_lw_end(&mut self, f: FighterId) {
        let frames = self.link_anim_frames(f, ATTACK_AIR_LW_HIT_ANIM);
        let a = self.game.fighters[f.0]
            .dat_attrs
            .expect("a fighter without attributes");
        let off = attr::ATTACK_AIR_LW_HIT_FRAME_END;
        self.game.fighter_attrs[a.0][off..off + 4].copy_from_slice(&frames.to_bits().to_be_bytes());
    }

    /// The frame count of the character's animation `anim`, as `ftData_80085E50` and
    /// `lbAnim_8001E8F8` find it, without loading it.
    fn link_anim_frames(&self, f: FighterId, anim: i32) -> f32 {
        let fp = &self.game.fighters[f.0];
        if anim as u32 >= fp.x58C {
            return 0.0;
        }
        let entry = self.anim_entry(f, anim);
        if self.anim_aram(fp, entry) == 0 {
            return 0.0;
        }
        let tree = self.anim_tree(fp.kind, entry);
        super::super::data::figatree_frames(self.data, Some(tree))
    }

    /// Hangs an optional part's model on the fighter, `ftParts_800753D4` (Link's and Young
    /// Link's creation, and Kirby's copied hats): `desc` (a `Fighter_804D6540` entry) names
    /// the part, the part it hangs from, how (`ftParts_80075304`) and how far into `joint`'s
    /// tree its joint is. The joint's descriptor is copied onto the stack with no child or
    /// sibling and loaded twice in the blend skeleton's class (`ftParts_8007482C`), so neither
    /// copy has display objects; the first becomes the part's joint, the second its blend
    /// joint. The ID table files both under the copy's stack address, `stack`, which depends on
    /// the calls that led here: a key that names another call path's live entry would take it
    /// over, where the original files a new one.
    #[doc(alias = "ftParts_800753D4")]
    #[doc(alias = "ftParts_80075304")]
    pub fn attach_part_model(
        &mut self,
        f: FighterId,
        desc: melee_data::archive::DataRef,
        joint: melee_data::archive::DataRef,
        stack: u32,
    ) {
        let part = usize::from(self.data.u8(desc));
        let parent = usize::from(self.data.u8(desc.at(1)));
        let how = self.data.u8(desc.at(2));
        let depth = self.data.u8(desc.at(3));
        let mut j = joint;
        if depth != 0xFF {
            let mut stack = JointStack::new();
            for _ in 0..depth {
                j = next_joint(self.data, j, &mut stack).expect("a joint past the tree's end");
            }
        }
        let key = crate::hsd::id::stack_key(stack);
        let objs = &mut self.game.objs;
        jobj::set_default_class(objs, jobj::JObjClass::FighterIntp);
        let model = jobj::load_joint_copy(objs, self.data, j, key);
        let blend = jobj::load_joint_copy(objs, self.data, j, key);
        jobj::set_default_class(objs, jobj::JObjClass::Hsd);
        let parts = self.game.fighters[f.0]
            .parts
            .expect("a fighter without parts")
            .0;
        let (root, root2, root_depth) = {
            let b = &self.game.fighter_parts[parts][parent];
            (b.joint, b.x4_jobj2, b.xC_u[0])
        };
        hang(&mut self.game.objs.jobjs, how, root, model);
        hang(&mut self.game.objs.jobjs, how, root2, blend);
        let mut tree_depth = u32::from(root_depth);
        if how == 0 || how == 1 {
            tree_depth += 1;
        }
        // `ftParts_80074194` with the DObj index at 0: the copy has no display objects.
        let flags = self.game.objs.jobjs[model].flags;
        assert!(
            self.game.objs.jobjs[model].dobj().is_none(),
            "a part's model from the blend class with display objects"
        );
        let bone = &mut self.game.fighter_parts[parts][part];
        bone.joint = Some(model);
        bone_from_joint(bone, flags, tree_depth);
        set_bone_xd(bone, 0);
        set_bone_bit(bone, 1, 6, false);
        bone.x4_jobj2 = Some(blend);
        set_bone_bit(bone, 0, 2, true);
    }

    /// Link's spawn, `ftLk_Init_OnDeath` (and Young Link's, `ftCl_Init_OnDeath`): his three models' first variants, and no boomerang,
    /// hookshot, arrow, bow or `x18`.
    #[doc(alias = "ftLk_Init_OnDeath")]
    #[doc(alias = "ftCl_Init_OnDeath")]
    pub fn link_on_death(&mut self, gobj: GObjId) {
        self.game.set_part_vis(gobj, 0, 0);
        self.game.set_part_vis(gobj, 1, 0);
        self.game.set_part_vis(gobj, 2, 0);
        let fp = self.game.fighter_mut(gobj);
        fp.set_u_i32(u::USED_BOOMERANG, 0);
        fp.u.set_gobj_at(u::BOOMERANG, None);
        fp.u.set_gobj_at(u::HOOKSHOT, None);
        fp.u.set_gobj_at(u::ARROW, None);
        fp.u.set_gobj_at(u::BOW, None);
        fp.u.set_gobj_at(u::HOOKSHOT, None);
        fp.u.set_gobj_at(u::X18, None);
    }

    /// `ftLk_Init_LoadSpecialAttrs` (Young Link's calls it): his attributes copied again (the down aerial's length
    /// with them), the Spin Attack's height offset scaled to his height.
    #[doc(alias = "ftLk_Init_LoadSpecialAttrs")]
    #[doc(alias = "ftCl_Init_LoadSpecialAttrs")]
    pub fn link_load_special_attrs(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.copy_attrs(f, attr::SIZE);
        self.link_patch_attack_air_lw_end(f);
        let scale = self.game.fighters[f.0].x34_scale.y;
        if scale != 1.0 {
            let off = attr::SPECIAL_HI_POS_Y_OFFSET;
            let v = self.link_attr_f32(f, off);
            let v = mem(fp::fmuls(reg(v), reg(scale)));
            let a = self.game.fighters[f.0]
                .dat_attrs
                .expect("a fighter without attributes");
            self.game.fighter_attrs[a.0][off..off + 4].copy_from_slice(&v.to_bits().to_be_bytes());
        }
    }

    /// Whether his boomerang is out, `ftLk_Init_BoomerangExists`.
    #[doc(alias = "ftLk_Init_BoomerangExists")]
    pub fn link_boomerang_exists(&self, gobj: GObjId) -> bool {
        self.game.fighter(gobj).u.gobj_at(u::BOOMERANG).is_some()
    }

    /// `ftLk_800EAF38`: the boomerang's catch undone (`ftLk_SpecialS_RemoveBoomerang1`).
    #[doc(alias = "ftLk_800EAF38")]
    pub fn link_remove_boomerang_cb(&mut self, gobj: GObjId) {
        self.link_remove_boomerang1(gobj);
    }

    /// His items' cleanup when he's hit or KO'd, `ftLk_800EAF58`: the boomerang's catch undone,
    /// the hookshot put away (`ftCo_800D94D8`), the arrow and the bow let go, and Young Link's
    /// milk taken away (`ftCl_Init_80149268`, which Link's `x18` never holds).
    #[doc(alias = "ftLk_800EAF58")]
    pub fn link_damage_cleanup(&mut self, gobj: GObjId) {
        self.link_remove_boomerang1(gobj);
        self.link_hookshot_put_away(gobj);
        self.link_process_arrow(gobj);
        self.link_process_bow(gobj);
        self.clink_remove_milk(gobj);
    }

    /// Picking up an item, `ftLk_Init_OnItemPickupExt`: his sheathed sword's and shield's
    /// models swapped (the second only for a heavy item), then `ftLk_Init_OnItemPickup`.
    #[doc(alias = "ftLk_Init_OnItemPickupExt")]
    #[doc(alias = "ftCl_Init_OnItemPickupExt")]
    pub fn link_on_item_pickup(&mut self, gobj: GObjId, flag: bool) {
        let item = self
            .game
            .fighter(gobj)
            .item_gobj
            .expect("picking up no item");
        if super::super::super::it::lib26b1::is_heavy(self.game, self.data, item) {
            self.game.set_part_vis(gobj, 1, 1);
        }
        self.game.set_part_vis(gobj, 2, 1);
        self.link_item_pickup(gobj, flag);
    }

    /// The item back in hand, `ftLk_Init_OnItemPickup` (`Fighter_OnItemPickup(gobj, flag, 1,
    /// 1)`): his models left alone (a swing's end calls this).
    #[doc(alias = "ftLk_Init_OnItemPickup")]
    #[doc(alias = "ftCl_Init_OnItemPickup")]
    pub fn link_item_pickup(&mut self, gobj: GObjId, flag: bool) {
        self.item_pickup_hand(gobj, flag, 1);
    }

    /// Dropping the item, `ftLk_Init_OnItemDropExt`: the models swapped back, then
    /// `ftLk_Init_OnItemDrop`.
    #[doc(alias = "ftLk_Init_OnItemDropExt")]
    #[doc(alias = "ftCl_Init_OnItemDropExt")]
    pub fn link_on_item_drop(&mut self, gobj: GObjId, flag: bool) {
        let item = self.game.fighter(gobj).item_gobj.expect("dropping no item");
        if super::super::super::it::lib26b1::is_heavy(self.game, self.data, item) {
            self.game.set_part_vis(gobj, 1, 0);
        }
        self.game.set_part_vis(gobj, 2, 0);
        self.link_item_drop(gobj, flag);
    }

    /// The item out of the hand, `ftLk_Init_OnItemDrop` (`Fighter_OnItemDrop(gobj, flag, 1,
    /// 1)`): his models left alone (a swing's start calls this).
    #[doc(alias = "ftLk_Init_OnItemDrop")]
    #[doc(alias = "ftCl_Init_OnItemDrop")]
    pub fn link_item_drop(&mut self, gobj: GObjId, flag: bool) {
        self.item_drop_hand(gobj, flag, 1);
    }

    /// Hiding the held item, `ftLk_Init_OnItemInvisible` (`Fighter_OnItemInvisible(gobj, 1)`).
    #[doc(alias = "ftLk_Init_OnItemInvisible")]
    #[doc(alias = "ftCl_Init_OnItemInvisible")]
    pub fn link_on_item_invisible(&mut self, gobj: GObjId) {
        self.item_invisible_hand(gobj, 1);
    }

    /// Showing the held item again, `ftLk_Init_OnItemVisible` (`Fighter_OnItemVisible(gobj,
    /// 1)`).
    #[doc(alias = "ftLk_Init_OnItemVisible")]
    #[doc(alias = "ftCl_Init_OnItemVisible")]
    pub fn link_on_item_visible(&mut self, gobj: GObjId) {
        self.item_visible_hand(gobj, 1);
    }

    /// Entering knockback, `ftLk_Init_OnKnockbackEnter` (`Fighter_OnKnockbackEnter(gobj,
    /// 1)`): the hurt textures.
    #[doc(alias = "ftLk_Init_OnKnockbackEnter")]
    #[doc(alias = "ftCl_Init_OnKnockbackEnter")]
    pub fn link_on_knockback_enter(&mut self, gobj: GObjId) {
        self.texture_anim(gobj, 1, 3.0);
        self.texture_anim(gobj, 0, 3.0);
    }

    /// Leaving knockback, `ftLk_Init_OnKnockbackExit`.
    #[doc(alias = "ftLk_Init_OnKnockbackExit")]
    #[doc(alias = "ftCl_Init_OnKnockbackExit")]
    pub fn link_on_knockback_exit(&mut self, gobj: GObjId) {
        self.texture_anim(gobj, 1, 0.0);
        self.texture_anim(gobj, 0, 0.0);
    }

    /// His shield's absorb push, `ftLk_800EB334`: pushed back along the ground by the shield
    /// stun of the absorbed hit (`ftCo_80092ED8`, with his loaded attributes' `xD8`), away
    /// from the hit's side, with a sound.
    #[doc(alias = "ftLk_800EB334")]
    pub fn link_absorb_push(&mut self, gobj: GObjId) {
        self.absorb_push_with(gobj, SFX_ABSORB);
    }

    /// The shield's absorb push, Link's or Young Link's, with his sound `sfx`.
    pub(in super::super) fn absorb_push_with(&mut self, gobj: GObjId, sfx: i32) {
        let f = self.game.fighter_id(gobj);
        let xd8 = {
            let d = self.game.fighters[f.0]
                .ft_data
                .expect("a fighter without data");
            let ext = self.data.ptr(d.at(4)).expect("ftData::ext_attr");
            self.data.f32(ext.at(attr::XD8 as u32))
        };
        let x19a4 = self.game.fighters[f.0].x19A4;
        let stun = self.shield_stun_frames(x19a4, xd8);
        let mul = self.common().f32(0x294);
        let fp = &mut self.game.fighters[f.0];
        fp.gr_vel = mem(fp::fmuls(stun, reg(mul)));
        // Away from the hit's side: kept for a hit from the left (`specialn_facing_dir`
        // negative), else turned.
        if fp.specialn_facing_dir >= 0.0 || fp.specialn_facing_dir.is_nan() {
            fp.gr_vel = mem(fp::fneg(reg(fp.gr_vel)));
        }
        self.play_sfx(gobj, sfx, 127, 64);
    }
}

/// How far below the stack pointer it's entered with `ftParts_800753D4` keeps its descriptor
/// copy (its frame 0x80, the copy 0x2C into it): the part's ID key, which ssbm-rs's probe of
/// 2026-10-10_12-54-40 logs 0x54 below that pointer on every path
/// (`$SSBM_RS_LOCAL/research/card-chain/vs-12-54-40-link-key.log`, `vs-12-54-40-8579.log`).
pub(crate) const PARTS_COPY_BELOW_SP: u32 = 0x54;
/// The stack pointer `Fighter_Create` (frame 0x48) is entered with from VS mode's match setup
/// (`fn_8016E730`, `fn_8016E2BC`, `Player_80031AD0`, from `gm_Scene_Vs_OnEnter` or
/// `gm_Scene_SuddenDeath_OnEnter`, both 0x18), and `ftDemo_CreateFighter` (0x30) from the
/// results screen's entry (`gm_Scene_Results_OnEnter`, `fn_8017A67C`, `Player_80036F34`):
/// the probe logs `ftParts_800753D4` entered at 0x804EE9F8 under Link's match creation (tick
/// 2442, key 0x804EE9A4) and at 0x804EE9E8 under his results pose (tick 9880, key 0x804EE994).
const FIGHTER_CREATE_SP: u32 = 0x804E_E9F8 + 0x28 + 0x48;
const DEMO_CREATE_SP: u32 = 0x804E_E9E8 + 0x28 + 0x30;

/// Hangs `new` on `root` the way `how` says, `ftParts_80075304`: 0 as its only child (taking
/// its children), 1 as its first child, 2 as its next sibling (taking its later siblings as
/// children, whose parents stay), 3 as its next sibling.
fn hang(jobjs: &mut jobj::JObjs, how: u8, root: Option<jobj::JObjId>, new: jobj::JObjId) {
    match how {
        0 => {
            let child = root.and_then(|r| jobjs[r].child);
            jobjs[new].child = child;
            jobjs[new].parent = root;
            jobjs[root.expect("root")].child = Some(new);
            if let Some(c) = child {
                jobjs[c].parent = Some(new);
            }
        }
        1 => {
            let child = root.and_then(|r| jobjs[r].child);
            jobjs[new].next = child;
            jobjs[new].parent = root;
            jobjs[root.expect("root")].child = Some(new);
        }
        2 => {
            let next = root.and_then(|r| jobjs[r].next);
            let r = root.expect("root");
            jobjs[new].child = next;
            jobjs[new].parent = jobjs[r].parent;
            jobjs[r].next = Some(new);
        }
        3 => {
            let next = root.and_then(|r| jobjs[r].next);
            let r = root.expect("root");
            jobjs[new].next = next;
            jobjs[new].parent = jobjs[r].parent;
            jobjs[r].next = Some(new);
        }
        _ => {}
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::REMOVE_BOOMERANG_CB => g.link_remove_boomerang_cb(gobj),
        addr::DAMAGE_CB => g.link_damage_cleanup(gobj),
        addr::ON_ITEM_INVISIBLE => g.link_on_item_invisible(gobj),
        addr::ON_ITEM_VISIBLE => g.link_on_item_visible(gobj),
        addr::LOAD_SPECIAL_ATTRS => g.link_load_special_attrs(gobj),
        addr::ON_KNOCKBACK_ENTER => g.link_on_knockback_enter(gobj),
        addr::ON_KNOCKBACK_EXIT => g.link_on_knockback_exit(gobj),
        addr::ABSORB_CB => g.link_absorb_push(gobj),
        _ => return false,
    }
    true
}

pub(super) fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    let gobj = |i: usize| args[i].gobj().expect("a null fighter");
    Some(match f {
        addr::ON_ITEM_PICKUP_EXT => {
            r.link_on_item_pickup(gobj(0), args[1].int() != 0);
            Val::None
        }
        addr::ON_ITEM_DROP_EXT => {
            r.link_on_item_drop(gobj(0), args[1].int() != 0);
            Val::None
        }
        addr::ON_ITEM_PICKUP => {
            r.link_item_pickup(gobj(0), args[1].int() != 0);
            Val::None
        }
        addr::ON_ITEM_DROP => {
            r.link_item_drop(gobj(0), args[1].int() != 0);
            Val::None
        }
        addr::BOOMERANG_EXISTS => Val::Int(i32::from(r.link_boomerang_exists(gobj(0)))),
        _ => return None,
    })
}
