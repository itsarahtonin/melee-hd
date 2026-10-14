//! Samus's character callbacks (`ftsamus.c`): creation, spawn and attributes, the item hooks,
//! the charge's cleanup, its glow, and the grapple beam her throws show.

use gekko_fp as fp;

use super::super::super::code::Val;
use super::super::super::state::Running;
use super::super::fox::ext;
use super::{ITEMS, PART_THROW_N, addr, attr, u};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::{aobj, jobj};
use crate::vec::Vec3;

/// `ftCo_MS_ThrowF`, the first throw.
const MS_THROW_F: i32 = 0xDB;
/// The object types the throw's beam sets its animations' rate on: all but materials and
/// textures.
const ANIM_MASK: u32 = 0xFB7F;
/// `ftCo_800BFFD0`'s color animation for a full charge.
const COLANIM_CHARGED: i32 = 53;

impl Running<'_> {
    /// Samus's creation, `ftSs_Init_OnLoad`: she can wall jump, has her attributes, and
    /// registers her bomb, Charge Shot, missile and grapple beam with the items.
    #[doc(alias = "ftSs_Init_OnLoad")]
    pub fn samus_on_load(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.game.fighters[f.0].can_walljump = true;
        self.push_attrs(f, attr::SIZE);
        let d = self.game.fighters[f.0]
            .ft_data
            .expect("a fighter without data");
        let items = self.data.ptr(d.at(0x48)).expect("ftData::x48_items");
        for (i, kind) in ITEMS.into_iter().enumerate() {
            let article = self.data.ptr(items.at(4 * i as u32));
            super::super::super::it::item::set_char_article(self.game, article, kind);
        }
    }

    /// Samus's spawn, `ftSs_Init_OnDeath`: her first model variant, no charge, missiles,
    /// Screw Attack effects or grapple beam.
    #[doc(alias = "ftSs_Init_OnDeath")]
    pub fn samus_on_death(&mut self, gobj: GObjId) {
        self.game.set_part_vis(gobj, 0, 0);
        let fp = self.game.fighter_mut(gobj);
        fp.u.set_gobj_at(u::CHARGE_SHOT, None);
        fp.set_u_i32(u::CHARGE, 0);
        fp.set_u_i32(u::MISSILES, 0);
        fp.set_u_i32(u::SCREW_EFFECTS, 0);
        fp.u.set_gobj_at(u::GRAPPLE, None);
        fp.u[u::GRAPPLE_SEQUENCE] = 0;
    }

    /// `ftSs_Init_LoadSpecialAttrs`: her attributes, the bomb jump's speed, the bomb's height,
    /// the bombs' jumps and the morph ball's box scaled to her height.
    #[doc(alias = "ftSs_Init_LoadSpecialAttrs")]
    pub fn samus_load_special_attrs(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.copy_attrs(f, attr::SIZE);
        let scale = self.game.fighters[f.0].x34_scale.y;
        if scale == 1.0 {
            return;
        }
        let a = self.game.fighters[f.0]
            .dat_attrs
            .expect("a fighter without attributes");
        let b = &mut self.game.fighter_attrs[a.0];
        let offs = [
            attr::BOMB_JUMP_SPEED,
            attr::LW_BOMB_OFFSET + 4,
            attr::LW_VEL_Y,
            attr::LW_AIR_VEL_Y,
        ]
        .into_iter()
        .chain((0..6).map(|i| attr::LW_BOX + 4 * i));
        for off in offs {
            let v = f32::from_bits(u32::from_be_bytes(b[off..off + 4].try_into().unwrap()));
            let v = mem(fp::fmuls(reg(v), reg(scale)));
            b[off..off + 4].copy_from_slice(&v.to_bits().to_be_bytes());
        }
    }

    /// The charge's cleanup when she's hit or KO'd, `ftSs_Init_80128428`: the Charge Shot
    /// gone, the Screw Attack's effects, and the grapple beam put away (`ftCo_800D9C98`).
    #[doc(alias = "ftSs_Init_80128428")]
    pub fn samus_damage_cleanup(&mut self, gobj: GObjId) {
        self.samus_drop_charge(gobj);
        self.samus_screw_effects_off(gobj);
        self.samus_grapple_put_away(gobj);
    }

    /// Picking up an item, `ftSs_Init_OnItemPickup` (`Fighter_OnItemPickup(gobj, flag, 0,
    /// 0)`): the hand's hold pose for the item, unless it is heavy.
    // Ports `ftSs_Init_OnItemPickup`; aliased once a check reaches it.
    pub fn samus_on_item_pickup(&mut self, gobj: GObjId, flag: bool) {
        let item = self
            .game
            .fighter(gobj)
            .item_gobj
            .expect("picking up no item");
        if super::super::super::it::lib26b1::is_heavy(self.game, self.data, item) {
            return;
        }
        let kind = self
            .ext(ext::IT_GET_HOLD_KIND, &[Val::GObj(Some(item))])
            .int();
        let pose = match kind {
            1 => Some(1),
            2 => Some(0),
            3 => Some(2),
            4 => Some(3),
            _ => None,
        };
        if let Some(p) = pose {
            self.ext(
                ext::FTANIM_80070FB4,
                &[Val::GObj(Some(gobj)), Val::Int(0), Val::Int(p)],
            );
        }
        if flag {
            self.ext(ext::FTANIM_80070C48, &[Val::GObj(Some(gobj)), Val::Int(0)]);
        }
    }

    /// Hiding the held item, `ftSs_Init_OnItemInvisible` (`Fighter_OnItemInvisible(gobj,
    /// 0)`).
    // Ports `ftSs_Init_OnItemInvisible`; aliased once a check reaches it.
    pub fn samus_on_item_invisible(&mut self, gobj: GObjId) {
        let item = self.game.fighter(gobj).item_gobj.expect("hiding no item");
        if !super::super::super::it::lib26b1::is_heavy(self.game, self.data, item) {
            self.ext(ext::FTANIM_80070CC4, &[Val::GObj(Some(gobj)), Val::Int(0)]);
        }
    }

    /// Showing the held item again, `ftSs_Init_OnItemVisible` (`Fighter_OnItemVisible(gobj,
    /// 0)`).
    // Ports `ftSs_Init_OnItemVisible`; aliased once a check reaches it.
    pub fn samus_on_item_visible(&mut self, gobj: GObjId) {
        let item = self.game.fighter(gobj).item_gobj.expect("showing no item");
        if !super::super::super::it::lib26b1::is_heavy(self.game, self.data, item) {
            self.ext(ext::FTANIM_80070C48, &[Val::GObj(Some(gobj)), Val::Int(0)]);
        }
    }

    /// Dropping the item, `ftSs_Init_OnItemDrop` (`Fighter_OnItemDrop(gobj, flag, 0, 0)`).
    // Ports `ftSs_Init_OnItemDrop`; aliased once a check reaches it.
    pub fn samus_on_item_drop(&mut self, gobj: GObjId, flag: bool) {
        self.ext(
            ext::FTANIM_80070FB4,
            &[Val::GObj(Some(gobj)), Val::Int(0), Val::Int(-1)],
        );
        if flag {
            self.ext(ext::FTANIM_80070CC4, &[Val::GObj(Some(gobj)), Val::Int(0)]);
        }
    }

    /// The fully charged glow, `ftSs_Init_UnkMotionStates4` (the color animations' hook).
    #[doc(alias = "ftSs_Init_UnkMotionStates4")]
    pub fn samus_charged_glow(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let charge = self.game.fighters[f.0].u_i32(u::CHARGE);
        if fp::frsp(f64::from(charge)) == reg(self.samus_attr_f32(f, attr::CHARGE_MAX)) {
            self.start_color_anim(gobj, COLANIM_CHARGED, 0);
        }
    }

    /// The grapple beam her throws show, `ftSs_Init_CreateThrowGrappleBeam`: the accessory
    /// model (`ftData::x48_items[4]`) at her height, its own animations and the throw's played
    /// at `speed` (`ftSs_Init_80128770` on each that doesn't loop), and following her throw
    /// joint.
    #[doc(alias = "ftSs_Init_CreateThrowGrappleBeam")]
    #[doc(alias = "ftSs_Init_80128770")]
    pub fn samus_throw_beam(&mut self, gobj: GObjId, msid: i32, speed: f32) {
        let f = self.game.fighter_id(gobj);
        let d = self.game.fighters[f.0]
            .ft_data
            .expect("a fighter without data");
        let items = self.data.ptr(d.at(0x48)).expect("ftData::x48_items");
        let beam = self
            .data
            .ptr(items.at(16))
            .expect("the grapple beam's model");
        let joint = self.data.ptr(beam).expect("the beam's joint");
        self.set_accessory(gobj, joint);
        let s = self.game.fighters[f.0].x34_scale.y;
        let acc = self.game.fighters[f.0]
            .x20A0_accessory
            .expect("fp->x20A0_accessory");
        jobj::set_scale(&mut self.game.objs.jobjs, acc, Vec3::new(s, s, s));
        let anim = self.data.ptr(beam.at(8));
        let matanim = self.data.ptr(beam.at(0xC));
        jobj::add_anim_all(
            &mut self.game.objs,
            self.data,
            Some(acc),
            anim,
            matanim,
            None,
        );
        let anims = self.data.ptr(beam.at(4)).expect("the throws' animations");
        let throw = self.data.ptr(anims.at(4 * (msid - MS_THROW_F) as u32));
        jobj::add_anim_all(&mut self.game.objs, self.data, Some(acc), throw, None, None);
        jobj::foreach_anim(&mut self.game.objs, acc, ANIM_MASK, &mut |objs, a| {
            if aobj::flags(&objs.aobjs, Some(a)) & aobj::LOOP == 0 {
                aobj::set_rate(&mut objs.aobjs, Some(a), speed);
            }
        });
        jobj::req_anim_all(&mut self.game.objs, Some(acc), 0.0);
        jobj::anim_all(
            &mut self.game.objs,
            self.data,
            &mut crate::game::code::ParticleHooks,
            Some(acc),
        );
        let throw_n = self.bone_joint(gobj, PART_THROW_N);
        crate::lb::anim::attach(&mut self.game.objs, Some(acc), throw_n);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::DAMAGE_CB => g.samus_damage_cleanup(gobj),
        addr::ON_ITEM_INVISIBLE => g.samus_on_item_invisible(gobj),
        addr::ON_ITEM_VISIBLE => g.samus_on_item_visible(gobj),
        addr::UNK_MOTION_STATES4 => g.samus_charged_glow(gobj),
        _ => return false,
    }
    true
}

pub(super) fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    let gobj = |i: usize| args[i].gobj().expect("a null fighter");
    Some(match f {
        addr::ON_ITEM_PICKUP => {
            r.samus_on_item_pickup(gobj(0), args[1].int() != 0);
            Val::None
        }
        addr::ON_ITEM_DROP => {
            r.samus_on_item_drop(gobj(0), args[1].int() != 0);
            Val::None
        }
        _ => return None,
    })
}
