//! Peach's neutral special, Toad (`ftpeachspecialn.c`): she holds Toad out when her script
//! says; an attack that meets him (a shield bubble, `ftColl_8007B1B8`) turns the move into
//! its counter, where Toad releases a spore. In the air, the first Toad of a jump slows her
//! fall with a small lift.

use gekko_fp as fp;

use super::super::super::it::kinds::{peachtoad, peachtoadspore};
use super::super::super::state::Running;
use super::super::co::guard::set_shield_hit;
use super::super::common::GA_GROUND;
use super::super::motion::flags::*;
use super::{addr, attr, ms, mv, u};
use crate::fpu::{mem, reg};
use crate::game::handles::CodeRef;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// `doColl`, `doAirColl`, `doHitColl`, `doAirHitColl` and `setupHitColl`.
const DO_COLL: u32 = 0x8011_E7D4;
const DO_AIR_COLL: u32 = 0x8011_E8C0;
const DO_HIT_COLL: u32 = 0x8011_EB58;
const DO_AIR_HIT_COLL: u32 = 0x8011_EBC0;
const SETUP_HIT_COLL: u32 = 0x8011_EC30;

/// The bone Toad is held on (`FtPart_109`, her hand).
const PART_TOAD: i32 = 109;

/// The script's variables (`cmd_vars`): the air lift's, the bubble's, Toad's and the
/// counter's spore's.
const CMD_PHYS: usize = 0;
const CMD_ANIM: usize = 1;
const CMD_ACCESSORY4: usize = 2;
const CMD_HIT_ANIM: usize = 3;

/// The flags the move keeps changing between the ground and the air (`coll_mf`).
const COLL_MF: u32 = KEEP_COL_ANIM_HIT_STATUS
    | SKIP_HIT
    | SKIP_MAT_ANIM
    | SKIP_COL_ANIM
    | UPDATE_CMD
    | SKIP_ITEM_VIS
    | UNK_19
    | SKIP_MODEL_PART_VIS
    | SKIP_MODEL_FLAGS
    | UNK_27;
const _: () = assert!(COLL_MF == 0x0C4C_508C);

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    use super::addr::*;
    match a {
        N_ON_ACCESSORY4 => g.peach_toad_on_accessory4(gobj),
        N_ON_HIT_ACCESSORY4 => g.peach_toad_on_hit_accessory4(gobj),
        SPECIAL_N_ON_DEATH2 => g.peach_toad_on_death2(gobj),
        SPECIAL_N_DO_DEATH2 => g.peach_toad_gone(gobj),
        N_ON_ENTER_HITLAG => g.peach_toad_enter_hitlag(gobj),
        N_ON_EXIT_HITLAG => g.peach_toad_exit_hitlag(gobj),
        SPECIAL_N_ENTER => g.peach_special_n_enter(gobj),
        SPECIAL_AIR_N_ENTER => g.peach_special_air_n_enter(gobj),
        SPECIAL_N_ANIM => g.peach_special_n_anim(gobj),
        SPECIAL_AIR_N_ANIM => g.peach_special_air_n_anim(gobj),
        SPECIAL_N_IASA | SPECIAL_AIR_N_IASA | SPECIAL_N_HIT_IASA | SPECIAL_AIR_N_HIT_IASA => {
            g.peach_special_n_iasa(gobj)
        }
        SPECIAL_N_PHYS => g.peach_special_n_phys(gobj),
        SPECIAL_AIR_N_PHYS => g.peach_special_air_n_phys(gobj),
        SPECIAL_N_COLL => g.peach_special_n_coll(gobj),
        SPECIAL_AIR_N_COLL => g.peach_special_air_n_coll(gobj),
        DO_COLL => g.peach_special_n_to_air(gobj),
        DO_AIR_COLL => g.peach_special_n_to_ground(gobj),
        SPECIAL_N_HIT_ANIM => g.peach_special_n_hit_anim(gobj),
        SPECIAL_AIR_N_HIT_ANIM => g.peach_special_air_n_hit_anim(gobj),
        SPECIAL_N_HIT_PHYS => g.peach_special_n_hit_phys(gobj),
        SPECIAL_AIR_N_HIT_PHYS => g.peach_special_air_n_hit_phys(gobj),
        SPECIAL_N_HIT_COLL => g.peach_special_n_hit_coll(gobj),
        SPECIAL_AIR_N_HIT_COLL => g.peach_special_air_n_hit_coll(gobj),
        DO_HIT_COLL => g.peach_special_n_hit_to_air(gobj),
        DO_AIR_HIT_COLL => g.peach_special_n_hit_to_ground(gobj),
        SETUP_HIT_COLL => g.peach_special_n_hit_hooks(gobj),
        N_ON_UNK_HIT => g.peach_toad_hit(gobj),
        _ => return false,
    }
    true
}

impl Running<'_> {
    /// Toad made in her hand (`onAccessory4`, once): she holds him as her special item, with
    /// her hooks for her hit and hitlag.
    #[doc(alias = "onAccessory4")]
    pub fn peach_toad_on_accessory4(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[CMD_ACCESSORY4] != 0 {
            return;
        }
        self.game.fighters[f.0].cmd_vars[CMD_ACCESSORY4] = 1;
        let hand = self.bone_joint(gobj, PART_TOAD);
        let pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, hand, None);
        let facing = self.game.fighters[f.0].facing_dir;
        let toad = peachtoad::spawn(self, gobj, pos, PART_TOAD, peachtoad::KIND, facing);
        let fp = &mut self.game.fighters[f.0];
        fp.u.set_gobj_at(u::TOAD, toad);
        fp.x1984_heldItemSpec = toad;
        if toad.is_some() {
            fp.death2_cb = Some(CodeRef(addr::ON_DEATH2));
            fp.take_dmg_cb = Some(CodeRef(addr::ON_DEATH2));
        }
        fp.accessory4_cb = None;
        fp.pre_hitlag_cb = Some(CodeRef(addr::N_ON_ENTER_HITLAG));
        fp.post_hitlag_cb = Some(CodeRef(addr::N_ON_EXIT_HITLAG));
    }

    /// The counter's spore (`onHitAccessory4`, with `doHitAccessory4`): released from her
    /// hand, a little above it and on the stage's plane.
    #[doc(alias = "onHitAccessory4")]
    #[doc(alias = "doHitAccessory4")]
    pub fn peach_toad_on_hit_accessory4(&mut self, gobj: GObjId) {
        let hand = self.bone_joint(gobj, PART_TOAD);
        let mut pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, hand, None);
        pos.y = mem(fp::fadds(reg(pos.y), 2.5));
        pos.z = 0.0;
        let facing = self.game.fighter(gobj).facing_dir;
        peachtoadspore::spawn(self, gobj, pos, peachtoadspore::KIND, facing);
        self.game.fighter_mut(gobj).accessory4_cb = None;
    }

    /// Toad taken away when she's hit or KO'd, `ftPe_SpecialN_OnDeath2`: if out, he goes and
    /// she forgets him.
    #[doc(alias = "ftPe_SpecialN_OnDeath2")]
    pub fn peach_toad_on_death2(&mut self, gobj: GObjId) {
        if let Some(toad) = self.game.fighter(gobj).u.gobj_at(u::TOAD) {
            peachtoad::remove(self, toad);
            self.peach_toad_gone(gobj);
        }
    }

    /// She forgets Toad and its hooks, `ftPe_SpecialN_DoDeath2`, Toad going on after her
    /// hitlag first (`onExitHitlag`).
    #[doc(alias = "ftPe_SpecialN_DoDeath2")]
    pub fn peach_toad_gone(&mut self, gobj: GObjId) {
        self.peach_toad_exit_hitlag(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.u.set_gobj_at(u::TOAD, None);
        fp.death2_cb = None;
        fp.take_dmg_cb = None;
    }

    /// Toad pauses in her hitlag (`onEnterHitlag`).
    #[doc(alias = "onEnterHitlag")]
    pub fn peach_toad_enter_hitlag(&mut self, gobj: GObjId) {
        if let Some(toad) = self.game.fighter(gobj).u.gobj_at(u::TOAD) {
            peachtoad::hitlag_start(self, toad);
        }
    }

    /// Toad goes on after her hitlag (`onExitHitlag`).
    #[doc(alias = "onExitHitlag")]
    pub fn peach_toad_exit_hitlag(&mut self, gobj: GObjId) {
        if let Some(toad) = self.game.fighter(gobj).u.gobj_at(u::TOAD) {
            peachtoad::hitlag_end(self, toad);
        }
    }

    /// Whether she has left the move, `ftPe_SpecialN_IsActive` (true outside its four states,
    /// despite its name).
    #[doc(alias = "ftPe_SpecialN_IsActive")]
    pub fn peach_toad_done(&self, gobj: GObjId) -> bool {
        let msid = self.game.fighter(gobj).motion_id;
        !(ms::SPECIAL_N..=ms::SPECIAL_AIR_N_HIT).contains(&msid)
    }

    /// The move's start (`reset`): the script's variables cleared, her facing kept, and Toad
    /// made when the script says.
    fn peach_special_n_reset(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[CMD_HIT_ANIM] = 0;
        fp.cmd_vars[CMD_ACCESSORY4] = 0;
        fp.cmd_vars[CMD_ANIM] = 0;
        fp.cmd_vars[CMD_PHYS] = 0;
        let facing = fp::fctiwz(reg(fp.facing_dir));
        fp.set_mv_i32(mv::N_FACING_DIR, facing);
        fp.accessory4_cb = Some(CodeRef(addr::N_ON_ACCESSORY4));
    }

    /// `ftPe_SpecialN_Enter`: on the ground, no vertical speed.
    #[doc(alias = "ftPe_SpecialN_Enter")]
    pub fn peach_special_n_enter(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).self_vel.y = 0.0;
        self.change_motion_state(gobj, ms::SPECIAL_N, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.peach_special_n_reset(gobj);
    }

    /// `ftPe_SpecialAirN_Enter`: in the air, her horizontal speed divided by the attributes'
    /// x94.
    #[doc(alias = "ftPe_SpecialAirN_Enter")]
    pub fn peach_special_air_n_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let div = reg(self.peach_attr_f32(f, attr::N_AIR_VEL_X_DIV));
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.x = mem(fp::fdivs(reg(fp.self_vel.x), div));
        self.change_motion_state(gobj, ms::SPECIAL_AIR_N, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.peach_special_n_reset(gobj);
    }

    /// Toad's bubble, `ftColl_8007B1B8` with the attributes' absorb shape: an attack meeting
    /// it calls [`addr::N_ON_UNK_HIT`].
    fn peach_toad_bubble(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let bone = self.peach_attr_i32(f, attr::N_BUBBLE_BONE);
        let offset = Vec3::new(
            self.peach_attr_f32(f, attr::N_BUBBLE_OFFSET),
            self.peach_attr_f32(f, attr::N_BUBBLE_OFFSET + 4),
            self.peach_attr_f32(f, attr::N_BUBBLE_OFFSET + 8),
        );
        let size = self.peach_attr_f32(f, attr::N_BUBBLE_SIZE);
        let joint = self.bone_joint(gobj, bone);
        let fp = &mut self.game.fighters[f.0];
        set_shield_hit(fp, CodeRef(addr::N_ON_UNK_HIT), joint, size, offset);
    }

    /// The stance (`doAnim`): the bubble once the script opens it (`cmd_vars[1]` 1), with its
    /// push; none before; then `done` when the animation ends.
    fn peach_special_n_stance(&mut self, gobj: GObjId, air: bool) {
        let f = self.game.fighter_id(gobj);
        match self.game.fighters[f.0].cmd_vars[CMD_ANIM] {
            1 => {
                self.game.fighters[f.0].cmd_vars[CMD_ANIM] = 2;
                self.peach_toad_bubble(gobj);
                let push = self.peach_attr_f32(f, attr::N_XA8);
                let fp = &mut self.game.fighters[f.0];
                fp.x221B.x221B_b3 = true;
                fp.shield_unk0 = push;
                fp.shield_unk1 = push;
            }
            0 => self.game.fighters[f.0].x221B.x221B_b0 = false,
            _ => {}
        }
        if !self.anim_frames_remaining(gobj) {
            if air {
                self.enter_fall(gobj);
            } else {
                self.enter_wait(gobj);
            }
        }
    }

    /// `ftPe_SpecialN_Anim`.
    #[doc(alias = "ftPe_SpecialN_Anim")]
    pub fn peach_special_n_anim(&mut self, gobj: GObjId) {
        self.peach_special_n_stance(gobj, false);
    }

    /// `ftPe_SpecialAirN_Anim`.
    #[doc(alias = "ftPe_SpecialAirN_Anim")]
    pub fn peach_special_air_n_anim(&mut self, gobj: GObjId) {
        self.peach_special_n_stance(gobj, true);
    }

    /// The move's interrupts: none.
    #[doc(alias = "ftPe_SpecialN_IASA")]
    #[doc(alias = "ftPe_SpecialAirN_IASA")]
    #[doc(alias = "ftPe_SpecialNHit_IASA")]
    #[doc(alias = "ftPe_SpecialAirNHit_IASA")]
    pub fn peach_special_n_iasa(&mut self, _gobj: GObjId) {}

    /// `ftPe_SpecialN_Phys`: friction, and the bubble follows.
    #[doc(alias = "ftPe_SpecialN_Phys")]
    pub fn peach_special_n_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
        self.shield_follow(gobj);
    }

    /// `ftPe_SpecialAirN_Phys`: once the script says (`cmd_vars[0]`), the first Toad of a
    /// jump lifts her (the attributes' x9C) and later ones stop her fall, then the move's
    /// gravity; before, plain gravity. Air friction, and the bubble follows.
    #[doc(alias = "ftPe_SpecialAirN_Phys")]
    pub fn peach_special_air_n_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let lift = self.peach_attr_f32(f, attr::N_AIR_VEL_Y);
        let (g, t) = (
            reg(self.peach_attr_f32(f, attr::N_XA0)),
            reg(self.peach_attr_f32(f, attr::N_XA4)),
        );
        let friction = reg(self.peach_attr_f32(f, attr::N_X98));
        let fp = &mut self.game.fighters[f.0];
        if fp.cmd_vars[CMD_PHYS] >= 1 {
            if fp.cmd_vars[CMD_PHYS] == 1 {
                fp.cmd_vars[CMD_PHYS] = 2;
                if fp.u_i32(u::SPECIAL_AIR_N_USED) == 0 {
                    fp.set_u_i32(u::SPECIAL_AIR_N_USED, 1);
                    fp.self_vel.y = lift;
                } else {
                    fp.self_vel.y = 0.0;
                }
            }
            Self::fall(fp, g, t);
        } else {
            Self::fall_basic(fp);
        }
        Self::self_accel_deaccel(fp, friction);
        self.shield_follow(gobj);
    }

    /// `ftPe_SpecialN_Coll`: into the air off an edge.
    #[doc(alias = "ftPe_SpecialN_Coll")]
    pub fn peach_special_n_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_slide(gobj) {
            self.peach_special_n_to_air(gobj);
        }
    }

    /// `ftPe_SpecialAirN_Coll`: onto the ground on landing.
    #[doc(alias = "ftPe_SpecialAirN_Coll")]
    pub fn peach_special_air_n_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.peach_special_n_to_ground(gobj);
        }
    }

    /// The move's hooks again after a change between the ground and the air (`setupColl`),
    /// with the bubble if it was open.
    fn peach_special_n_hooks(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        if fp.u.gobj_at(u::TOAD).is_some() {
            fp.death2_cb = Some(CodeRef(addr::ON_DEATH2));
            fp.take_dmg_cb = Some(CodeRef(addr::ON_DEATH2));
        }
        fp.accessory4_cb = Some(CodeRef(addr::N_ON_ACCESSORY4));
        fp.pre_hitlag_cb = Some(CodeRef(addr::N_ON_ENTER_HITLAG));
        fp.post_hitlag_cb = Some(CodeRef(addr::N_ON_EXIT_HITLAG));
        if fp.cmd_vars[CMD_ANIM] == 2 {
            self.peach_toad_bubble(gobj);
            self.game.fighters[f.0].x221B.x221B_b3 = true;
        }
    }

    /// Off an edge (`doColl`): the air state from the same frame, the lift spent if the
    /// script had just asked for it.
    #[doc(alias = "doColl")]
    pub fn peach_special_n_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_N, COLL_MF);
        let fp = self.game.fighter_mut(gobj);
        if fp.cmd_vars[CMD_PHYS] == 1 {
            fp.cmd_vars[CMD_PHYS] = 2;
        }
        self.peach_special_n_hooks(gobj);
    }

    /// Landing (`doAirColl`): her air Toad's lift back, the ground state from the same frame.
    #[doc(alias = "doAirColl")]
    pub fn peach_special_n_to_ground(&mut self, gobj: GObjId) {
        self.game
            .fighter_mut(gobj)
            .set_u_i32(u::SPECIAL_AIR_N_USED, 0);
        self.fox_air_to_ground(gobj, ms::SPECIAL_N, COLL_MF);
        self.peach_special_n_hooks(gobj);
    }

    /// The counter (`doHitAnim`): the spore when the script says (`cmd_vars[3]`), then `done`
    /// when the animation ends.
    fn peach_special_n_counter(&mut self, gobj: GObjId, air: bool) {
        let fp = self.game.fighter_mut(gobj);
        if fp.cmd_vars[CMD_HIT_ANIM] != 0 {
            fp.cmd_vars[CMD_HIT_ANIM] = 0;
            fp.accessory4_cb = Some(CodeRef(addr::N_ON_HIT_ACCESSORY4));
        }
        if !self.anim_frames_remaining(gobj) {
            if air {
                self.enter_fall(gobj);
            } else {
                self.enter_wait(gobj);
            }
        }
    }

    /// `ftPe_SpecialNHit_Anim`.
    #[doc(alias = "ftPe_SpecialNHit_Anim")]
    pub fn peach_special_n_hit_anim(&mut self, gobj: GObjId) {
        self.peach_special_n_counter(gobj, false);
    }

    /// `ftPe_SpecialAirNHit_Anim`.
    #[doc(alias = "ftPe_SpecialAirNHit_Anim")]
    pub fn peach_special_air_n_hit_anim(&mut self, gobj: GObjId) {
        self.peach_special_n_counter(gobj, true);
    }

    /// `ftPe_SpecialNHit_Phys`: friction.
    #[doc(alias = "ftPe_SpecialNHit_Phys")]
    pub fn peach_special_n_hit_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// `ftPe_SpecialAirNHit_Phys`: the move's gravity and air friction.
    #[doc(alias = "ftPe_SpecialAirNHit_Phys")]
    pub fn peach_special_air_n_hit_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let (g, t) = (
            reg(self.peach_attr_f32(f, attr::N_XA0)),
            reg(self.peach_attr_f32(f, attr::N_XA4)),
        );
        let friction = reg(self.peach_attr_f32(f, attr::N_X98));
        let fp = &mut self.game.fighters[f.0];
        Self::fall(fp, g, t);
        Self::self_accel_deaccel(fp, friction);
    }

    /// `ftPe_SpecialNHit_Coll`: into the air off an edge.
    #[doc(alias = "ftPe_SpecialNHit_Coll")]
    pub fn peach_special_n_hit_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_slide(gobj) {
            self.peach_special_n_hit_to_air(gobj);
        }
    }

    /// `ftPe_SpecialAirNHit_Coll`: onto the ground on landing.
    #[doc(alias = "ftPe_SpecialAirNHit_Coll")]
    pub fn peach_special_air_n_hit_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.peach_special_n_hit_to_ground(gobj);
        }
    }

    /// Off an edge in the counter (`doHitColl`).
    #[doc(alias = "doHitColl")]
    pub fn peach_special_n_hit_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_N_HIT, COLL_MF);
        self.peach_special_n_hit_hooks(gobj);
    }

    /// Landing in the counter (`doAirHitColl`): her air Toad's lift back.
    #[doc(alias = "doAirHitColl")]
    pub fn peach_special_n_hit_to_ground(&mut self, gobj: GObjId) {
        self.game
            .fighter_mut(gobj)
            .set_u_i32(u::SPECIAL_AIR_N_USED, 0);
        self.fox_air_to_ground(gobj, ms::SPECIAL_N_HIT, COLL_MF);
        self.peach_special_n_hit_hooks(gobj);
    }

    /// The counter's hooks (`setupHitColl`): for her hit while Toad is out, and her hitlag.
    #[doc(alias = "setupHitColl")]
    pub fn peach_special_n_hit_hooks(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.u.gobj_at(u::TOAD).is_some() {
            fp.death2_cb = Some(CodeRef(addr::ON_DEATH2));
            fp.take_dmg_cb = Some(CodeRef(addr::ON_DEATH2));
        }
        fp.pre_hitlag_cb = Some(CodeRef(addr::N_ON_ENTER_HITLAG));
        fp.post_hitlag_cb = Some(CodeRef(addr::N_ON_EXIT_HITLAG));
    }

    /// An attack met Toad's bubble (`onUnkHit`): the counter from frame 9, facing where the
    /// hit came from, and Toad's own reaction.
    #[doc(alias = "onUnkHit")]
    pub fn peach_toad_hit(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let facing = fp::fctiwz(reg(fp.specialn_facing_dir));
        fp.set_mv_i32(mv::N_FACING_DIR, facing);
        let msid = if fp.ground_or_air == GA_GROUND {
            ms::SPECIAL_N_HIT
        } else {
            ms::SPECIAL_AIR_N_HIT
        };
        self.change_motion_state(gobj, msid, 0, 9.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        if let Some(toad) = self.game.fighter(gobj).u.gobj_at(u::TOAD) {
            peachtoad::hit(self, toad);
        }
        self.peach_special_n_hit_hooks(gobj);
    }
}
