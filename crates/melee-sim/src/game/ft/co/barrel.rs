//! Riding in a barrel cannon (`ftCo_Barrel.c`): caught by the barrel's grab, the fighter goes
//! inside, invisible and intangible, following the barrel, until it fires them out: on a
//! button once its lock-out frames pass, when its ride frames run out, or when the fighter
//! is hit. The launch puts them at the barrel's mouth with the barrel's knockback.

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::super::handles::CodeRef;
use super::super::super::it::{self, kinds::tarucann};
use super::super::super::state::Running;
use super::ids::ms;
use super::{PAD_A, PAD_B};
use crate::fpu::{mem, reg};
use crate::game::structs::HitCapsule;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// `barrel.x0` (the ride's frames left), `barrel.x4` (the lock-out's) and `barrel.x8` (the
/// barrel) in `mv`.
const MV_RIDE: usize = 0x0;
const MV_LOCK: usize = 0x4;
const MV_BARREL: usize = 0x8;

/// The barrel states' callbacks and hooks.
pub const DEATH2: u32 = 0x800C_9058;
pub const TAKE_DMG: u32 = 0x800C_9198;
const ANIM: u32 = 0x800C_91C0;
const IASA: u32 = 0x800C_9208;
const PHYS: u32 = 0x800C_925C;
const COLL: u32 = 0x800C_9260;
pub const ACCESSORY1: u32 = 0x800C_9264;
/// The launched fighter's state change hook, `fn_800C9290`.
pub const LAUNCHED: u32 = 0x800C_9290;

/// Runs one of the barrel states' callbacks, if `addr` is one.
pub fn run(g: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        DEATH2 | TAKE_DMG => g.barrel_fire(gobj),
        ANIM => g.co_barrel_anim(gobj),
        IASA => g.co_barrel_iasa(gobj),
        PHYS | COLL => g.co_barrel_nothing(gobj),
        ACCESSORY1 => g.barrel_follow(gobj),
        LAUNCHED => g.barrel_launched(gobj),
        _ => return false,
    }
    true
}

impl Running<'_> {
    /// The fighter's barrel.
    fn barrel_of(&self, gobj: GObjId) -> GObjId {
        self.game
            .fighter(gobj)
            .mv
            .gobj_at(MV_BARREL)
            .expect("a fighter in a barrel without one")
    }

    /// Caught by a barrel cannon, `ftCo_Barrel_Enter`: lets go of what it holds, goes inside,
    /// still, invisible and intangible, the barrel's hit on it recorded.
    #[doc(alias = "ftCo_Barrel_Enter")]
    pub fn enter_barrel(&mut self, gobj: GObjId, barrel: GObjId) {
        // `ftCo_ReleaseItemAndVictim`.
        self.damage_hooks(gobj);
        self.drop_heavy_item(gobj);
        self.release_victim(gobj);
        self.change_motion_state(gobj, ms::BARREL, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        let (ride, lock) = (
            tarucann::ride_frames(self, barrel),
            tarucann::lock_frames(self, barrel),
        );
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        fp.death2_cb = Some(CodeRef(DEATH2));
        fp.accessory1_cb = Some(CodeRef(ACCESSORY1));
        fp.take_dmg_cb = Some(CodeRef(TAKE_DMG));
        fp.x221D_b5 = true;
        fp.set_mv_i32(MV_RIDE, ride);
        fp.set_mv_i32(MV_LOCK, lock);
        fp.mv.set_gobj_at(MV_BARREL, Some(barrel));
        self.go_airborne(f);
        self.stop_dead(gobj);
        let fp = &mut self.game.fighters[f.0];
        fp.x1988 = 2;
        fp.invisible = true;
        fp.x1A6A = 0x1FF;
        fp.x209A = 1;
        self.item_attack_landed(barrel, gobj, false);
    }

    /// The barrel fires the fighter out, its hit and death hook, `ftCo_Barrel_Take_Dmg_Cb`
    /// (and `ftCo_Barrel_Death2_Cb`).
    // Ports `ftCo_Barrel_Take_Dmg_Cb`; aliased once a check reaches it.
    // Ports `ftCo_Barrel_Death2_Cb`; aliased once a check reaches it.
    pub fn barrel_fire(&mut self, gobj: GObjId) {
        let barrel = self.barrel_of(gobj);
        tarucann::fire(self, barrel);
    }

    /// `ftCo_Barrel_Anim`: when the ride's frames run out the barrel fires.
    // Ports `ftCo_Barrel_Anim`; aliased once a check reaches it.
    pub fn co_barrel_anim(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let ride = fp.mv_i32(MV_RIDE);
        if ride != 0 {
            fp.set_mv_i32(MV_RIDE, ride - 1);
            if ride - 1 == 0 {
                let barrel = self.barrel_of(gobj);
                tarucann::fire(self, barrel);
            }
        }
    }

    /// `ftCo_Barrel_IASA`: once the lock-out's frames pass, A or B fires the barrel.
    // Ports `ftCo_Barrel_IASA`; aliased once a check reaches it.
    pub fn co_barrel_iasa(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let lock = fp.mv_i32(MV_LOCK);
        if lock != 0 {
            fp.set_mv_i32(MV_LOCK, lock - 1);
        }
        let fp = self.game.fighter(gobj);
        if fp.mv_i32(MV_LOCK) == 0 && fp.input.pressed_buttons & (PAD_A | PAD_B) != 0 {
            let barrel = self.barrel_of(gobj);
            tarucann::fire(self, barrel);
        }
    }

    /// `ftCo_Barrel_Phys` and `ftCo_Barrel_Coll`: nothing.
    // Ports `ftCo_Barrel_Phys`; aliased once a check reaches it.
    // Ports `ftCo_Barrel_Coll`; aliased once a check reaches it.
    pub fn co_barrel_nothing(&mut self, _gobj: GObjId) {}

    /// Inside the barrel, the fighter is where it is, `ftCo_Barrel_Accessory1_Cb`.
    // Ports `ftCo_Barrel_Accessory1_Cb`; aliased once a check reaches it.
    pub fn barrel_follow(&mut self, gobj: GObjId) {
        let barrel = self.barrel_of(gobj);
        let pos = it::lib26b1::bottom_pos(self.game, barrel);
        self.game.fighter_mut(gobj).cur_pos = pos;
    }

    /// The launched fighter's state change hook, `fn_800C9290`: its thrown hitbox takes the
    /// barrel's owner, if a fighter.
    // Ports `fn_800C9290`; aliased once a check reaches it.
    pub fn barrel_launched(&mut self, gobj: GObjId) {
        let barrel = self.barrel_of(gobj);
        let owner = it::item(self.game, barrel).owner;
        if let Some(o) = owner.filter(|_| it::item::is_fighter(self.game, owner)) {
            let f = self.game.fighter_id(gobj);
            self.thrown_hitbox_owner(f, o);
        }
    }

    /// Fired out of the barrel at `mouth`, `ftCo_800C92E4`: its camera bone put at the mouth,
    /// its collision at the barrel (`at`), the barrel's knockback at `kb_angle` (degrees, the
    /// way the barrel points, mirrored for a fighter facing right), the hit recorded, and
    /// tumbling off.
    // Ports `ftCo_800C92E4`; aliased once a check reaches it.
    pub fn barrel_launch(
        &mut self,
        gobj: GObjId,
        mouth: Vec3,
        at: Vec3,
        hit: &HitCapsule,
        kb_angle: f32,
    ) {
        let f = self.game.fighter_id(gobj);
        let root = self.root_jobj(gobj);
        let t = self.game.objs.jobjs[root].translate;
        let bone = self.camera_bone_pos(f);
        let fp = &mut self.game.fighters[f.0];
        fp.cur_pos.x = mem(fp::fsubs(fp::fadds(reg(mouth.x), reg(t.x)), reg(bone.x)));
        fp.cur_pos.y = mem(fp::fsubs(fp::fadds(reg(mouth.y), reg(t.y)), reg(bone.y)));
        fp.cur_pos.z = 0.0;
        fp.coll_data.set_position(at);
        // `ftCo_Barrel_ApplyKnockback`.
        let mut angle = reg(kb_angle);
        if angle < 0.0 {
            angle = fp::fadds(angle, 360.0);
        }
        let angle = if reg(self.game.fighters[f.0].facing_dir) < 0.0 {
            angle
        } else {
            fp::fsubs(180.0, angle)
        };
        let weight = self.game.fighters[f.0].co_attrs.weight;
        // `ftColl_80079EA8`: knockback at no ratio.
        let kb = self.knockback_of(f, hit, hit.unk_count, 1.0, 1.0, 1.0, weight);
        let fp = &mut self.game.fighters[f.0];
        fp.dmg.kb_applied = mem(kb);
        fp.dmg.x1848_kb_angle = fp::fctiwz(angle);
        fp.dmg.facing_dir_1 = fp.facing_dir;
        fp.dmg.x184c_damaged_hurtbox = 0;
        fp.dmg.x1854_collpos = fp.cur_pos;
        fp.dmg.x1860_element = hit.element;
        let barrel = self.barrel_of(gobj);
        self.item_attack_landed(barrel, gobj, false);
        self.game.fighter_mut(gobj).x21EC = Some(CodeRef(LAUNCHED));
        self.enter_damage(gobj, ms::DAMAGE_FLY_ROLL, 0.0);
    }
}
