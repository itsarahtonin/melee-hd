//! Oil Panic (`ftgamewatchspeciallw.c`): the bucket absorbs energy projectiles (the
//! attributes' `AbsorbDesc`) while B is held, filling a third for each catch; full, the next
//! Oil Panic throws the oil (`it::kinds::gamewatchpanic`), its damage from what it caught.
//! The bucket's fill shows on his model variants.

use gekko_fp as fp;

use super::super::super::it::kinds::gamewatchpanic as panic_item;
use super::super::super::state::Running;
use super::super::common::GA_GROUND;
use super::super::motion::flags::*;
use super::{addr, attr, ms, mv, panic, part, u};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::ppc::cvt_fp2unsigned;
use crate::vec::Vec3;

/// `HSD_PAD_B`.
const PAD_B: u32 = 1 << 9;

/// What Oil Panic keeps when it changes between the ground and the air (`transition_flags0`)
/// and when it loops back (`transition_flags1`).
const KEEP_FLAGS: u32 = KEEP_GFX
    | KEEP_COL_ANIM_HIT_STATUS
    | SKIP_HIT
    | SKIP_MAT_ANIM
    | SKIP_COL_ANIM
    | UPDATE_CMD
    | SKIP_ITEM_VIS
    | UNK_19
    | SKIP_MODEL_PART_VIS
    | SKIP_MODEL_FLAGS
    | UNK_27;
const LOOP_FLAGS: u32 = KEEP_GFX
    | SKIP_HIT
    | SKIP_MAT_ANIM
    | SKIP_COL_ANIM
    | UPDATE_CMD
    | SKIP_ITEM_VIS
    | UNK_19
    | SKIP_MODEL_PART_VIS
    | SKIP_MODEL_FLAGS
    | UNK_27;

impl Running<'_> {
    /// The oil out, `ftGw_SpecialLw_ItemPanicSetup`.
    #[doc(alias = "ftGw_SpecialLw_ItemPanicSetup")]
    pub fn gw_panic_setup(&mut self, gobj: GObjId) {
        if self.gw_item(gobj, u::PANIC).is_none() {
            let pos = self.gw_part_pos(gobj, part::TOP_N, None);
            let facing = self.game.fighter(gobj).facing_dir;
            let item = panic_item::spawn(self, gobj, pos, 0, facing);
            self.gw_set_item(gobj, u::PANIC, item);
        }
        if self.gw_item(gobj, u::PANIC).is_some() {
            self.gw_damage_cbs(gobj);
        }
        self.gw_hitlag_cbs(
            gobj,
            addr::SPECIAL_LW_ITEM_PANIC_ENTER_HITLAG,
            addr::SPECIAL_LW_ITEM_PANIC_EXIT_HITLAG,
        );
        self.gw_set_accessory(gobj, None);
    }

    /// The oil forgotten, `ftGw_SpecialLw_ItemPanicSetFlag`.
    #[doc(alias = "ftGw_SpecialLw_ItemPanicSetFlag")]
    pub fn gw_panic_set_flag(&mut self, gobj: GObjId) {
        self.gw_panic_exit_hitlag(gobj);
        self.gw_set_item(gobj, u::PANIC, None);
        self.gw_clear_damage_cbs(gobj);
    }

    /// The oil taken away, `ftGw_SpecialLw_ItemPanicRemove`.
    #[doc(alias = "ftGw_SpecialLw_ItemPanicRemove")]
    pub fn gw_panic_remove(&mut self, gobj: GObjId) {
        if let Some(item) = self.gw_item(gobj, u::PANIC) {
            panic_item::remove(self, item);
            self.gw_panic_set_flag(gobj);
        }
    }

    /// `ftGw_SpecialLw_ItemPanicEnterHitlag`.
    #[doc(alias = "ftGw_SpecialLw_ItemPanicEnterHitlag")]
    pub fn gw_panic_enter_hitlag(&mut self, gobj: GObjId) {
        if let Some(item) = self.gw_item(gobj, u::PANIC) {
            panic_item::hitlag_start(self, item);
        }
    }

    /// `ftGw_SpecialLw_ItemPanicExitHitlag`.
    #[doc(alias = "ftGw_SpecialLw_ItemPanicExitHitlag")]
    pub fn gw_panic_exit_hitlag(&mut self, gobj: GObjId) {
        if let Some(item) = self.gw_item(gobj, u::PANIC) {
            panic_item::hitlag_end(self, item);
        }
    }

    /// Whether the oil should go, `ftGw_SpecialLw_ItemCheckPanicRemove`: out of the throw's
    /// states (and the air catch's between them).
    #[doc(alias = "ftGw_SpecialLw_ItemCheckPanicRemove")]
    pub fn gw_panic_should_go(&self, gobj: GObjId) -> bool {
        let msid = self.game.fighter(gobj).motion_id;
        !(ms::SPECIAL_LW_SHOOT..=ms::SPECIAL_AIR_LW_SHOOT).contains(&msid)
    }

    /// The bucket's model and its fill, `ftGw_SpecialLw_UpdateBucketModel`.
    #[doc(alias = "ftGw_SpecialLw_UpdateBucketModel")]
    pub fn gw_update_bucket_model(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.set_model_variant(f, 5, 2);
        let fill: [i32; 3] = match self.game.fighters[f.0].u_i32(u::PANIC_CHARGE) {
            panic::EMPTY => [-1, -1, -1],
            panic::LOW => [0, -1, -1],
            panic::MID => [0, 0, -1],
            _ => [0, 0, 0],
        };
        for (i, v) in fill.into_iter().enumerate() {
            self.set_model_variant(f, 6 + i, v);
        }
    }

    /// Oil Panic's start (`ftGameWatch_SpecialLw_SetVars`).
    fn gw_special_lw_start(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[1] = 0;
        fp.cmd_vars[0] = 0;
        fp.set_mv_i32(mv::LW_RELEASE, 0);
        fp.set_mv_i32(mv::LW_TURN_FRAMES, 0);
    }

    /// `ftGw_SpecialLw_Enter`: a full bucket throws its oil; else the catch.
    #[doc(alias = "ftGw_SpecialLw_Enter")]
    pub fn gw_special_lw_enter(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).u_i32(u::PANIC_CHARGE) >= panic::FULL {
            self.gw_special_lw_shoot_release(gobj);
            return;
        }
        self.game.fighter_mut(gobj).self_vel.y = 0.0;
        self.change_motion_state(gobj, ms::SPECIAL_LW, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.gw_special_lw_start(gobj);
    }

    /// `ftGw_SpecialAirLw_Enter`: the air speed cut by the attributes.
    #[doc(alias = "ftGw_SpecialAirLw_Enter")]
    pub fn gw_special_air_lw_enter(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).u_i32(u::PANIC_CHARGE) >= panic::FULL {
            self.gw_special_air_lw_shoot_release(gobj);
            return;
        }
        let f = self.game.fighter_id(gobj);
        let keep = reg(self.gw_attr_f32(f, attr::PANIC_MOMENTUM_PRESERVE));
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.x = mem(fp::fdivs(reg(fp.self_vel.x), keep));
        fp.self_vel.y = 0.0;
        self.change_motion_state(gobj, ms::SPECIAL_AIR_LW, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.gw_special_lw_start(gobj);
    }

    /// The bucket's absorber from the attributes' `AbsorbDesc`.
    fn gw_bucket_absorber(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let a = |o: usize| self.gw_attr_f32(f, attr::PANIC_ABSORPTION + o);
        let bone = self.gw_attr_i32(f, attr::PANIC_ABSORPTION);
        let offset = Vec3::new(a(4), a(8), a(0xC));
        let size = a(0x10);
        self.create_absorb_hit(gobj, bone, offset, size);
    }

    /// The bucket out (`ftGameWatch_SpecialLw_UpdateVars`): at the script's mark the
    /// absorber; before it none.
    fn gw_special_lw_update_vars(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        match fp.cmd_vars[0] {
            1 => {
                fp.cmd_vars[0] = 2;
                self.gw_bucket_absorber(gobj);
            }
            0 => fp.x2218_b6 = false,
            _ => {}
        }
        if self.game.fighter(gobj).cmd_vars[0] != 0 {
            self.gw_update_bucket_model(gobj);
        }
    }

    /// `ftGw_SpecialLw_Anim`: at frame 38 with B held the catch loops; waiting once its
    /// frames run out.
    #[doc(alias = "ftGw_SpecialLw_Anim")]
    pub fn gw_special_lw_anim(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        if fp.cur_anim_frame == 38.0 && fp.mv_i32(mv::LW_RELEASE) == 0 {
            self.gw_special_lw_update_action(gobj, 5.0);
        }
        self.gw_special_lw_update_vars(gobj);
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftGw_SpecialAirLw_Anim`: falling once its frames run out.
    #[doc(alias = "ftGw_SpecialAirLw_Anim")]
    pub fn gw_special_air_lw_anim(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        if fp.cur_anim_frame == 38.0 && fp.mv_i32(mv::LW_RELEASE) == 0 {
            self.gw_special_air_lw_update_action(gobj, 5.0);
        }
        self.gw_special_lw_update_vars(gobj);
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// Oil Panic's interrupts (`ftGw_SpecialLw_IASA`, `ftGw_SpecialAirLw_IASA`): the stick past
    /// the dead zone turns him, then not again for the attributes' frames; B let go of ends
    /// the loop.
    #[doc(alias = "ftGw_SpecialLw_IASA")]
    #[doc(alias = "ftGw_SpecialAirLw_IASA")]
    pub fn gw_special_lw_iasa(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let turn = self.game.fighters[f.0].mv_i32(mv::LW_TURN_FRAMES);
        if turn > 0 {
            self.game.fighters[f.0].set_mv_i32(mv::LW_TURN_FRAMES, turn - 1);
        } else {
            let mut stick = reg(self.game.fighters[f.0].input.lstick[0].x);
            if stick < 0.0 {
                stick = fp::fneg(stick);
            }
            if stick > reg(self.common().f32(0x0)) {
                let facing = self.game.fighters[f.0].facing_dir;
                self.face_stick(gobj);
                if facing != self.game.fighters[f.0].facing_dir {
                    let frames = fp::fctiwz(reg(self.gw_attr_f32(f, attr::PANIC_TURN_FRAMES)));
                    self.game.fighters[f.0].set_mv_i32(mv::LW_TURN_FRAMES, frames);
                }
            }
        }
        let fp = &mut self.game.fighters[f.0];
        if fp.input.held_buttons[0] & PAD_B == 0 {
            fp.set_mv_i32(mv::LW_RELEASE, 1);
        }
    }

    /// `ftGw_SpecialLw_Phys`: friction, and the absorber follows.
    #[doc(alias = "ftGw_SpecialLw_Phys")]
    pub fn gw_special_lw_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
        self.absorb_follow(gobj);
    }

    /// `ftGw_SpecialAirLw_Phys`: its own fall and air slowing, and the absorber follows.
    #[doc(alias = "ftGw_SpecialAirLw_Phys")]
    pub fn gw_special_air_lw_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let a = |o: usize| reg(self.gw_attr_f32(f, o));
        let (g, tv, slow) = (
            a(attr::PANIC_FALL_ACCEL),
            a(attr::PANIC_VEL_Y_MAX),
            a(attr::PANIC_MOMENTUM_MUL),
        );
        let fp = &mut self.game.fighters[f.0];
        Self::fall(fp, g, tv);
        Self::self_accel_deaccel(fp, slow);
        self.absorb_follow(gobj);
    }

    /// `ftGw_SpecialLw_Coll`: off a ledge, into the air.
    #[doc(alias = "ftGw_SpecialLw_Coll")]
    pub fn gw_special_lw_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_slide(gobj) {
            self.gw_special_lw_to_air(gobj);
        }
    }

    /// `ftGw_SpecialAirLw_Coll`: landing, onto the ground.
    #[doc(alias = "ftGw_SpecialAirLw_Coll")]
    pub fn gw_special_air_lw_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.gw_special_air_lw_to_ground(gobj);
        }
    }

    /// The bucket again after a change (`ftGameWatch_SpecialLw_UpdateVarsColl`): the
    /// absorber once the mark has passed.
    fn gw_special_lw_update_vars_coll(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        match fp.cmd_vars[0] {
            2 => self.gw_bucket_absorber(gobj),
            0 => fp.x2218_b6 = false,
            _ => {}
        }
        if self.game.fighter(gobj).cmd_vars[0] != 0 {
            self.gw_update_bucket_model(gobj);
        }
    }

    /// `ftGw_SpecialLw_GroundToAir`.
    #[doc(alias = "ftGw_SpecialLw_GroundToAir")]
    pub fn gw_special_lw_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_LW, KEEP_FLAGS);
        self.gw_special_lw_update_vars_coll(gobj);
    }

    /// `ftGw_SpecialAirLw_AirToGround`.
    #[doc(alias = "ftGw_SpecialAirLw_AirToGround")]
    pub fn gw_special_air_lw_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_LW, KEEP_FLAGS);
        self.gw_special_lw_update_vars_coll(gobj);
    }

    /// The bucket after a loop (`ftGameWatch_SpecialLw_UpdateVarsAction`): past the mark,
    /// the absorber again.
    fn gw_special_lw_update_vars_action(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.cmd_vars[0] >= 1 {
            fp.cmd_vars[0] = 2;
            self.gw_bucket_absorber(gobj);
        }
        if self.game.fighter(gobj).cmd_vars[0] != 0 {
            self.gw_update_bucket_model(gobj);
        }
    }

    /// The catch from `frame` again in state `msid`.
    fn gw_special_lw_loop(&mut self, gobj: GObjId, msid: i32, frame: f32) {
        let start = mem(fp::fsubs(reg(frame), 1.0));
        self.change_motion_state(gobj, msid, LOOP_FLAGS, start, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.gw_special_lw_update_vars_action(gobj);
    }

    /// `ftGw_SpecialLw_UpdateAction`.
    #[doc(alias = "ftGw_SpecialLw_UpdateAction")]
    pub fn gw_special_lw_update_action(&mut self, gobj: GObjId, frame: f32) {
        self.gw_special_lw_loop(gobj, ms::SPECIAL_LW, frame);
    }

    /// `ftGw_SpecialAirLw_UpdateAction`.
    #[doc(alias = "ftGw_SpecialAirLw_UpdateAction")]
    pub fn gw_special_air_lw_update_action(&mut self, gobj: GObjId, frame: f32) {
        self.gw_special_lw_loop(gobj, ms::SPECIAL_AIR_LW, frame);
    }

    /// `ftGw_SpecialLwCatch_Anim`: once its frames run out, waiting with a full bucket, else
    /// the catch again from frame 4.
    #[doc(alias = "ftGw_SpecialLwCatch_Anim")]
    pub fn gw_special_lw_catch_anim(&mut self, gobj: GObjId) {
        if self.anim_frames_remaining(gobj) {
            return;
        }
        if self.game.fighter(gobj).u_i32(u::PANIC_CHARGE) >= panic::FULL {
            self.enter_wait(gobj);
            return;
        }
        self.change_motion_state(gobj, ms::SPECIAL_LW, LOOP_FLAGS, 4.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.gw_special_lw_update_vars_action(gobj);
    }

    /// `ftGw_SpecialAirLwCatch_Anim`: falling with a full bucket.
    #[doc(alias = "ftGw_SpecialAirLwCatch_Anim")]
    pub fn gw_special_air_lw_catch_anim(&mut self, gobj: GObjId) {
        if self.anim_frames_remaining(gobj) {
            return;
        }
        if self.game.fighter(gobj).u_i32(u::PANIC_CHARGE) >= panic::FULL {
            self.enter_fall(gobj);
            return;
        }
        self.change_motion_state(gobj, ms::SPECIAL_AIR_LW, LOOP_FLAGS, 4.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.gw_special_lw_update_vars_action(gobj);
    }

    /// The catch's and throw's interrupts: nothing.
    #[doc(alias = "ftGw_SpecialLwCatch_IASA")]
    #[doc(alias = "ftGw_SpecialAirLwCatch_IASA")]
    #[doc(alias = "ftGw_SpecialLwShoot_IASA")]
    #[doc(alias = "ftGw_SpecialAirLwShoot_IASA")]
    pub fn gw_special_lw_no_iasa(&mut self, _gobj: GObjId) {}

    /// The catch's and throw's ground physics: friction (`ft_80084F3C`).
    #[doc(alias = "ftGw_SpecialLwCatch_Phys")]
    #[doc(alias = "ftGw_SpecialLwShoot_Phys")]
    pub fn gw_special_lw_catch_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// The catch's and throw's air physics: the air's, without drift (`ft_80084EEC`).
    #[doc(alias = "ftGw_SpecialAirLwCatch_Phys")]
    #[doc(alias = "ftGw_SpecialAirLwShoot_Phys")]
    pub fn gw_special_air_lw_catch_phys(&mut self, gobj: GObjId) {
        self.air_physics_no_drift(gobj);
    }

    /// `ftGw_SpecialLwCatch_Coll`: off a ledge, into the air.
    #[doc(alias = "ftGw_SpecialLwCatch_Coll")]
    pub fn gw_special_lw_catch_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.gw_special_lw_catch_to_air(gobj);
        }
    }

    /// `ftGw_SpecialAirLwCatch_Coll`.
    #[doc(alias = "ftGw_SpecialAirLwCatch_Coll")]
    pub fn gw_special_air_lw_catch_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.gw_special_air_lw_catch_to_ground(gobj);
        }
    }

    /// `ftGw_SpecialLwCatch_GroundToAir`.
    #[doc(alias = "ftGw_SpecialLwCatch_GroundToAir")]
    pub fn gw_special_lw_catch_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_LW_CATCH, KEEP_FLAGS);
    }

    /// `ftGw_SpecialAirLwCatch_AirToGround`.
    #[doc(alias = "ftGw_SpecialAirLwCatch_AirToGround")]
    pub fn gw_special_air_lw_catch_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_LW_CATCH, KEEP_FLAGS);
    }

    /// A catch, `ftGw_SpecialLw_AbsorbThink_DecideAction`: the hits and damage the absorber
    /// took into the bucket, its glow once full, and the catch's state.
    #[doc(alias = "ftGw_SpecialLw_AbsorbThink_DecideAction")]
    pub fn gw_absorb_decide_action(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let charge = fp
            .u_i32(u::PANIC_CHARGE)
            .wrapping_add(fp.AbsorbAttr.x1A48_hitsTaken);
        fp.set_u_i32(u::PANIC_CHARGE, charge);
        let damage = fp
            .u_i32(u::PANIC_DAMAGE)
            .wrapping_add(fp.AbsorbAttr.x1A44_damageTaken);
        fp.set_u_i32(u::PANIC_DAMAGE, damage);
        if charge >= panic::FULL {
            self.start_color_anim(gobj, 5, 0);
        }
        let msid = if self.game.fighter(gobj).ground_or_air == GA_GROUND {
            ms::SPECIAL_LW_CATCH
        } else {
            ms::SPECIAL_AIR_LW_CATCH
        };
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.gw_update_bucket_model(gobj);
    }

    /// The oil's damage on each live hitbox (`ftGameWatch_SpecialLwShoot_ApplyDamage`).
    fn gw_shoot_damage(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        for i in 0..4 {
            let fp = &self.game.fighters[f.0];
            if fp.x914[i].state == crate::lb::hitcapsule::ENABLED {
                let damage = fp.cmd_vars[1];
                self.hitbox_damage(f, i, false, damage);
            }
        }
    }

    /// `ftGw_SpecialLwShoot_Anim`: the oil's damage; waiting once its frames run out.
    #[doc(alias = "ftGw_SpecialLwShoot_Anim")]
    pub fn gw_special_lw_shoot_anim(&mut self, gobj: GObjId) {
        self.gw_shoot_damage(gobj);
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftGw_SpecialAirLwShoot_Anim`: falling once its frames run out.
    #[doc(alias = "ftGw_SpecialAirLwShoot_Anim")]
    pub fn gw_special_air_lw_shoot_anim(&mut self, gobj: GObjId) {
        self.gw_shoot_damage(gobj);
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// `ftGw_SpecialLwShoot_Coll`.
    #[doc(alias = "ftGw_SpecialLwShoot_Coll")]
    pub fn gw_special_lw_shoot_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.gw_special_lw_shoot_to_air(gobj);
        }
    }

    /// `ftGw_SpecialAirLwShoot_Coll`.
    #[doc(alias = "ftGw_SpecialAirLwShoot_Coll")]
    pub fn gw_special_air_lw_shoot_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.gw_special_air_lw_shoot_to_ground(gobj);
        }
    }

    /// `ftGw_SpecialLwShoot_GroundToAir`.
    #[doc(alias = "ftGw_SpecialLwShoot_GroundToAir")]
    pub fn gw_special_lw_shoot_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_LW_SHOOT, KEEP_FLAGS);
        self.gw_update_bucket_model(gobj);
        self.gw_set_accessory(gobj, Some(addr::SPECIAL_LW_ITEM_PANIC_SETUP));
    }

    /// `ftGw_SpecialAirLwShoot_AirToGround`.
    #[doc(alias = "ftGw_SpecialAirLwShoot_AirToGround")]
    pub fn gw_special_air_lw_shoot_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_LW_SHOOT, KEEP_FLAGS);
        self.gw_update_bucket_model(gobj);
        self.gw_set_accessory(gobj, Some(addr::SPECIAL_LW_ITEM_PANIC_SETUP));
    }

    /// The oil thrown in state `msid` (`ftGw_SpecialLwShoot_ReleaseOil`,
    /// `ftGw_SpecialAirLwShoot_ReleaseOil`): its damage the caught damage times the attributes'
    /// multiplier plus their bonus, in whole points; the bucket emptied.
    fn gw_release_oil(&mut self, gobj: GObjId, msid: i32) {
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        let f = self.game.fighter_id(gobj);
        let mul = reg(self.gw_attr_f32(f, attr::PANIC_DAMAGE_MUL));
        let add = reg(self.gw_attr_f32(f, attr::PANIC_DAMAGE_ADD));
        let fp = &mut self.game.fighters[f.0];
        let caught = fp::frsp(f64::from(fp.u_i32(u::PANIC_DAMAGE)));
        fp.cmd_vars[1] = cvt_fp2unsigned(fp::fmuls(caught, mul));
        let damage = fp::fadds(fp::frsp(f64::from(fp.cmd_vars[1])), add);
        fp.cmd_vars[1] = cvt_fp2unsigned(damage);
        fp.set_u_i32(u::PANIC_CHARGE, panic::EMPTY);
        fp.set_u_i32(u::PANIC_DAMAGE, 0);
        self.gw_update_bucket_model(gobj);
        self.gw_set_accessory(gobj, Some(addr::SPECIAL_LW_ITEM_PANIC_SETUP));
    }

    /// `ftGw_SpecialLwShoot_ReleaseOil`.
    #[doc(alias = "ftGw_SpecialLwShoot_ReleaseOil")]
    pub fn gw_special_lw_shoot_release(&mut self, gobj: GObjId) {
        self.gw_release_oil(gobj, ms::SPECIAL_LW_SHOOT);
    }

    /// `ftGw_SpecialAirLwShoot_ReleaseOil`.
    #[doc(alias = "ftGw_SpecialAirLwShoot_ReleaseOil")]
    pub fn gw_special_air_lw_shoot_release(&mut self, gobj: GObjId) {
        self.gw_release_oil(gobj, ms::SPECIAL_AIR_LW_SHOOT);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::SPECIAL_LW_ITEM_PANIC_SETUP => g.gw_panic_setup(gobj),
        addr::SPECIAL_LW_ITEM_PANIC_SET_FLAG => g.gw_panic_set_flag(gobj),
        addr::SPECIAL_LW_ITEM_PANIC_REMOVE => g.gw_panic_remove(gobj),
        addr::SPECIAL_LW_ITEM_PANIC_ENTER_HITLAG => g.gw_panic_enter_hitlag(gobj),
        addr::SPECIAL_LW_ITEM_PANIC_EXIT_HITLAG => g.gw_panic_exit_hitlag(gobj),
        addr::SPECIAL_LW_UPDATE_BUCKET_MODEL => g.gw_update_bucket_model(gobj),
        addr::SPECIAL_LW_ENTER => g.gw_special_lw_enter(gobj),
        addr::SPECIAL_AIR_LW_ENTER => g.gw_special_air_lw_enter(gobj),
        addr::SPECIAL_LW_ANIM => g.gw_special_lw_anim(gobj),
        addr::SPECIAL_AIR_LW_ANIM => g.gw_special_air_lw_anim(gobj),
        addr::SPECIAL_LW_IASA | addr::SPECIAL_AIR_LW_IASA => g.gw_special_lw_iasa(gobj),
        addr::SPECIAL_LW_PHYS => g.gw_special_lw_phys(gobj),
        addr::SPECIAL_AIR_LW_PHYS => g.gw_special_air_lw_phys(gobj),
        addr::SPECIAL_LW_COLL => g.gw_special_lw_coll(gobj),
        addr::SPECIAL_AIR_LW_COLL => g.gw_special_air_lw_coll(gobj),
        addr::SPECIAL_LW_GROUND_TO_AIR => g.gw_special_lw_to_air(gobj),
        addr::SPECIAL_AIR_LW_AIR_TO_GROUND => g.gw_special_air_lw_to_ground(gobj),
        addr::SPECIAL_LW_CATCH_ANIM => g.gw_special_lw_catch_anim(gobj),
        addr::SPECIAL_AIR_LW_CATCH_ANIM => g.gw_special_air_lw_catch_anim(gobj),
        addr::SPECIAL_LW_CATCH_IASA
        | addr::SPECIAL_AIR_LW_CATCH_IASA
        | addr::SPECIAL_LW_SHOOT_IASA
        | addr::SPECIAL_AIR_LW_SHOOT_IASA => g.gw_special_lw_no_iasa(gobj),
        addr::SPECIAL_LW_CATCH_PHYS | addr::SPECIAL_LW_SHOOT_PHYS => {
            g.gw_special_lw_catch_phys(gobj)
        }
        addr::SPECIAL_AIR_LW_CATCH_PHYS | addr::SPECIAL_AIR_LW_SHOOT_PHYS => {
            g.gw_special_air_lw_catch_phys(gobj)
        }
        addr::SPECIAL_LW_CATCH_COLL => g.gw_special_lw_catch_coll(gobj),
        addr::SPECIAL_AIR_LW_CATCH_COLL => g.gw_special_air_lw_catch_coll(gobj),
        addr::SPECIAL_LW_CATCH_GROUND_TO_AIR => g.gw_special_lw_catch_to_air(gobj),
        addr::SPECIAL_AIR_LW_CATCH_AIR_TO_GROUND => g.gw_special_air_lw_catch_to_ground(gobj),
        addr::SPECIAL_LW_ABSORB_THINK_DECIDE_ACTION => g.gw_absorb_decide_action(gobj),
        addr::SPECIAL_LW_SHOOT_ANIM => g.gw_special_lw_shoot_anim(gobj),
        addr::SPECIAL_AIR_LW_SHOOT_ANIM => g.gw_special_air_lw_shoot_anim(gobj),
        addr::SPECIAL_LW_SHOOT_COLL => g.gw_special_lw_shoot_coll(gobj),
        addr::SPECIAL_AIR_LW_SHOOT_COLL => g.gw_special_air_lw_shoot_coll(gobj),
        addr::SPECIAL_LW_SHOOT_GROUND_TO_AIR => g.gw_special_lw_shoot_to_air(gobj),
        addr::SPECIAL_AIR_LW_SHOOT_AIR_TO_GROUND => g.gw_special_air_lw_shoot_to_ground(gobj),
        addr::SPECIAL_LW_SHOOT_RELEASE_OIL => g.gw_special_lw_shoot_release(gobj),
        addr::SPECIAL_AIR_LW_SHOOT_RELEASE_OIL => g.gw_special_air_lw_shoot_release(gobj),
        _ => return false,
    }
    true
}
