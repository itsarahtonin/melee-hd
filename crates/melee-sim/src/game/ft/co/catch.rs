//! Grabbing (`ftCo_Catch.c`): the standing and dashing grabs, which pull a fighter their grab
//! box catches (`fn_800D9CE8`, the grab callback) or end back in waiting.

use gekko_fp as fp;

use super::super::super::code::unported_call;
use super::super::super::handles::{CodeRef, FighterId};
use super::super::super::state::Running;
use super::super::super::structs::Fighter;
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;

/// `ftCommon_MotionVars_catch`'s `x0`: Link's and Samus's tether frames.
const CATCH_X0: usize = 0x0;

/// The grab's callbacks: a fighter caught (`fn_800D9CE8`), the item the grabber had in sight
/// (`fn_800D8BFC`), and the caught fighter's side (`fn_800DAADC`).
const ON_GRAB: u32 = 0x800D_9CE8;
const ON_GRAB_ITEM: u32 = 0x800D_8BFC;
const ON_GRABBED: u32 = 0x800D_AADC;

impl Running<'_> {
    /// A grab, `ftCo_800D8C54`: still, the tether's frames cleared, into `msid` (a standing
    /// or a dashing grab) with the grab's callbacks (`ftCommon_8007E2D0`).
    #[doc(alias = "ftCo_800D8C54")]
    #[doc(alias = "ftCommon_8007E2D0")]
    pub fn enter_catch(&mut self, gobj: GObjId, msid: i32) {
        let fp = self.game.fighter_mut(gobj);
        fp.x74_self_accel.z = 0.0;
        fp.x74_self_accel.y = 0.0;
        fp.x74_self_accel.x = 0.0;
        fp.set_mv_f32(CATCH_X0, 0.0);
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        fp.x221E_b6 = true;
        fp.x1A68 = 1;
        fp.grab_cb = Some(CodeRef(ON_GRAB));
        fp.grabbed_cb = Some(CodeRef(ON_GRABBED));
        fp.x2194 = Some(CodeRef(ON_GRAB_ITEM));
    }

    /// Link's hookshot (`fn_800D8EC8`, `fn_800D9228`) and Samus's grapple beam
    /// (`fn_800D9558`, `fn_800D9930`) on their grab and dash grab: whether it ended the grab.
    fn catch_tether(&mut self, gobj: GObjId, dash: bool) -> bool {
        if self.link_catch_tether(gobj, dash) {
            return true;
        }
        self.samus_catch_tether(gobj, dash)
    }

    /// `ftCo_Catch_Anim` and `ftCo_CatchDash_Anim`: the tether's, then back to waiting when
    /// the animation ends.
    #[doc(alias = "ftCo_Catch_Anim")]
    #[doc(alias = "ftCo_CatchDash_Anim")]
    pub fn co_catch_anim(&mut self, gobj: GObjId, dash: bool) {
        if self.catch_tether(gobj, dash) {
            return;
        }
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftCo_Catch_Phys`: friction (`ftCommonData` x64), along the floor.
    #[doc(alias = "ftCo_Catch_Phys")]
    pub fn co_catch_phys(&mut self, gobj: GObjId) {
        let friction = fp_mul(
            self.common().f32(0x64),
            self.game.fighter(gobj).co_attrs.ground_friction,
        );
        Self::ground_accel_deaccel(self.game.fighter_mut(gobj), friction);
        self.self_movement_from_ground(gobj);
    }

    /// `ftCo_CatchDash_Phys`: the animation's motion or friction (`ft_80085030`).
    // Ports `ftCo_CatchDash_Phys`; aliased once a check reaches it.
    pub fn co_catch_dash_phys(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        let friction = fp_mul(self.common().f32(0x64), fp.co_attrs.ground_friction);
        let facing = reg(fp.facing_dir);
        self.ground_anim_motion(gobj, friction, facing);
    }

    /// `ftCo_Catch_Coll` and `ftCo_CatchDash_Coll`: on the ground, or off it the tether
    /// put away and falling (`fn_800D8E30`).
    #[doc(alias = "ftCo_Catch_Coll")]
    #[doc(alias = "ftCo_CatchDash_Coll")]
    pub fn co_catch_coll(&mut self, gobj: GObjId) {
        self.ground_coll_then(gobj, |g, gobj| g.catch_off_ground(gobj));
    }

    /// `fn_800D8E30`: Link's hookshot and Samus's grapple put away (`fn_800D949C`,
    /// `fn_800D9C64`), then falling.
    // Ports `fn_800D8E30`; aliased once a check reaches it.
    fn catch_off_ground(&mut self, gobj: GObjId) {
        self.link_catch_off_ground(gobj);
        self.samus_catch_off_ground(gobj);
        self.enter_fall(gobj);
    }
}

/// `ftCommonData` x64 times the ground friction, as the original multiplies them.
fn fp_mul(a: f32, b: f32) -> f64 {
    gekko_fp::fmuls(reg(a), reg(b))
}

impl Running<'_> {
    /// The fighter's grab boxes against other fighters, `ftColl_80078A2C`: of the fighters
    /// its grab boxes can catch (not its own player's, teammates aside, not intangible or
    /// already caught by this kind of grab), the nearest whose grabbable hurtbox a box
    /// touches without a wall between them becomes its victim.
    #[doc(alias = "ftColl_80078A2C")]
    pub fn grab_fighters(&mut self, gobj: GObjId) {
        use crate::lb::hitcapsule::{self, Victim};
        let me = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[me.0];
        fp.victim_gobj = None;
        fp.unk_grab_val = f32::MAX;
        let mut cur = self
            .game
            .world
            .first_in(super::super::common::FIGHTER_P_LINK);
        while let Some(v) = cur {
            cur = self.game.world.gobjs[v.0].next;
            if self.is_same_player(Some(gobj), Some(v)) {
                continue;
            }
            let vf_id = self.game.fighter_id(v);
            let (mf, vf) = (&self.game.fighters[me.0], &self.game.fighters[vf_id.0]);
            let teams = self.game.vs_is_teams() && !self.game.vs_friendly_fire();
            if vf.x2219_b1
                || vf.x222A_b0
                || (teams && vf.team == mf.team)
                || vf.x1988 != 0
                || vf.x198C != 0
                || vf.x221D_b6
                || vf.x1A6A & mf.x1A68 != 0
                || vf.stamina_dead
            {
                continue;
            }
            'boxes: for i in 0..4 {
                let (mf, vf) = (&self.game.fighters[me.0], &self.game.fighters[vf_id.0]);
                let h = &mf.x914[i];
                if h.state == hitcapsule::DISABLED
                    || h.element != ELEMENT_CATCH
                    || !((h.x40_b2 && vf.ground_or_air == super::super::common::GA_AIR)
                        || (h.x40_b3 && vf.ground_or_air == super::super::common::GA_GROUND))
                {
                    continue;
                }
                if hitcapsule::has_victim(h, Victim::Fighter(vf_id)) {
                    continue;
                }
                for j in 0..usize::from(vf.hurt_capsules_len) {
                    if self.game.fighters[vf_id.0].hurt_capsules[j].is_grabbable == 0 {
                        continue;
                    }
                    if self.grab_box_touches(me, i, vf_id, j) {
                        if !self.wall_between(me, vf_id) {
                            self.set_grab_target(me, i, vf_id);
                        }
                        break 'boxes;
                    }
                }
            }
        }
    }

    /// Whether the fighter's grab box `i` touches the victim's hurtbox `j`
    /// (`lbColl_80007ECC`).
    fn grab_box_touches(&mut self, me: FighterId, i: usize, v: FighterId, j: usize) -> bool {
        let mtx = self.own_matrix(v);
        let mut hit = self.game.fighters[me.0].x914[i].clone();
        let hit_scale = self.game.fighters[me.0].x34_scale.y;
        let vf = &mut self.game.fighters[v.0];
        let (scale, z) = (vf.x34_scale.y, vf.cur_pos.z);
        let touch = crate::lb::hitcapsule::grabs_hurtbox(
            &mut self.game.objs,
            self.data,
            &mut hit,
            &mut vf.hurt_capsules[j].capsule,
            mtx.as_ref(),
            hit_scale,
            scale,
            z,
        );
        let h = &mut self.game.fighters[me.0].x914[i];
        h.hurt_coll_pos = hit.hurt_coll_pos;
        h.coll_distance = hit.coll_distance;
        touch
    }

    /// Whether a wall stands between two fighters' middles, `ft_80084CE4`.
    // Ports `ft_80084CE4`; aliased once a check reaches it.
    fn wall_between(&mut self, a: FighterId, v: FighterId) -> bool {
        let mid = |f: &Fighter| {
            let e = &f.coll_data.ecb;
            let y = fp::fmuls(fp::fadds(reg(e.top.y), reg(e.bottom.y)), 0.5);
            (
                mem(fp::fadds(0.0, reg(f.cur_pos.x))),
                mem(fp::fadds(y, reg(f.cur_pos.y))),
            )
        };
        let (ax, ay) = mid(&self.game.fighters[a.0]);
        let (vx, vy) = mid(&self.game.fighters[v.0]);
        let g = self.game.geometry.as_mut().expect("no collision geometry");
        if ax > vx {
            g.check_right_wall(ax, ay, vx, vy, -1, -1).is_some()
        } else {
            g.check_left_wall(ax, ay, vx, vy, -1, -1).is_some()
        }
    }

    /// The victim becomes the grab's target if it's the nearest yet
    /// (`updateFighterGrabTarget`); the box's group remembers it either way
    /// (`ftColl_80076808`).
    fn set_grab_target(&mut self, me: FighterId, i: usize, v: FighterId) {
        self.hit_group(me, i, 0, crate::lb::hitcapsule::Victim::Fighter(v), false);
        let (mx, vx) = (
            reg(self.game.fighters[me.0].cur_pos.x),
            reg(self.game.fighters[v.0].cur_pos.x),
        );
        let d = fp::fsubs(vx, mx);
        let d = if d < 0.0 { fp::fneg(d) } else { d };
        let vg = self.game.fighters[v.0].gobj;
        let fp = &mut self.game.fighters[me.0];
        if d < reg(fp.unk_grab_val) {
            fp.x1A5C = vg;
            fp.victim_gobj = vg;
            fp.x221B.x221B_b5 = true;
            fp.unk_grab_val = mem(d);
        }
    }

    /// The fighter's grab boxes against items, `ftColl_8007BC90`: a box touching a catchable
    /// item's hurtbox stops at the seam (`updateItemGrabTarget`).
    #[doc(alias = "ftColl_8007BC90")]
    pub fn grab_items(&mut self, gobj: GObjId) {
        use super::super::super::it::{self, P_LINK_ITEM};
        use crate::lb::hitcapsule::{self, Victim};
        let me = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[me.0];
        fp.target_item_gobj = None;
        fp.unk_grab_val = f32::MAX;
        let mut cur = self.game.world.first_in(P_LINK_ITEM);
        while let Some(g) = cur {
            cur = self.game.world.gobjs[g.0].next;
            let ip = it::item(self.game, g);
            if ip.xD0C != 0 || ip.xDD0_flag[0] & 0x08 == 0 || ip.xD09 != 0 {
                continue;
            }
            let ii = it::item_of(self.game, g);
            for i in 0..4 {
                let ip = it::item(self.game, g);
                let h = &self.game.fighters[me.0].x914[i];
                if h.state == hitcapsule::DISABLED || h.element != ELEMENT_CATCH {
                    continue;
                }
                if !((h.x40_b2 && ip.ground_or_air == super::super::common::GA_AIR)
                    || (h.x40_b3 && ip.ground_or_air == super::super::common::GA_GROUND))
                {
                    continue;
                }
                if hitcapsule::has_victim(h, Victim::Item(ii)) {
                    continue;
                }
                for j in 0..usize::from(ip.xAC8_hurtboxNum) {
                    let mut hit = self.game.fighters[me.0].x914[i].clone();
                    let scale = self.game.fighters[me.0].x34_scale.y;
                    let ip = it::item(self.game, g);
                    let (iscl, mut hurt) = (ip.scl, ip.xACC_itemHurtbox[j].clone());
                    let touch = hitcapsule::grabs_hurtbox(
                        &mut self.game.objs,
                        self.data,
                        &mut hit,
                        &mut hurt,
                        None,
                        scale,
                        iscl,
                        0.0,
                    );
                    it::item_mut(self.game, g).xACC_itemHurtbox[j] = hurt;
                    let h = &mut self.game.fighters[me.0].x914[i];
                    h.hurt_coll_pos = hit.hurt_coll_pos;
                    h.coll_distance = hit.coll_distance;
                    if touch {
                        unported_call("updateItemGrabTarget (a grab box on an item)");
                    }
                }
            }
        }
    }
}

/// `HitElement_Catch`.
const ELEMENT_CATCH: u32 = 8;

impl Running<'_> {
    /// A sound of the character's own (`ftData::x4C`'s word at `off`).
    pub fn ft_data_sfx(&self, f: FighterId, off: u32) -> i32 {
        let ft_data = self.game.fighters[f.0]
            .ft_data
            .expect("a fighter without ftData");
        let sfx = self
            .data
            .ptr(ft_data.at(0x4C))
            .expect("ftData without sounds");
        self.data.u32(sfx.at(off)) as i32
    }

    /// A grab counts as the grabber's attack on its victim, `ftColl_80078754`, which then
    /// forgets who last hit it.
    // Ports `ftColl_80078754`; aliased once a check reaches it.
    pub fn grab_landed(&mut self, gobj: GObjId, victim: GObjId) {
        let fp = self.game.fighter(gobj);
        let (kind, x2070, instance) = (fp.kind, u32::from_be_bytes(fp.x2070), fp.x2074.x2088);
        self.attack_landed(
            Some(gobj),
            victim,
            1,
            kind,
            x2070,
            crate::game::pl::trick::AttackRecord::Fighter(gobj),
            instance,
            false,
            false,
        );
        let vf = self.game.fighter_mut(victim);
        vf.dmg.x18c4_source_ply = 6;
        vf.dmg.x18C8 = -1;
    }

    /// Runs a grab's victim callback (`Fighter::grabbed_cb`, taking the victim and the
    /// grabber).
    pub fn run_grabbed_cb(&mut self, cb: CodeRef, victim: GObjId, grabber: GObjId) {
        match cb.0 {
            ON_GRABBED => self.enter_capture_pulled(victim, grabber),
            super::super::captain::addr::CAPTURE_CAPTAIN => {
                self.enter_capture_captain(victim, grabber)
            }
            0x800B_BB8C => self.enter_capture_yoshi(victim, grabber),
            super::capture_mewtwo::CAPTURE => self.enter_capture_mewtwo(victim, grabber, false),
            super::capture_mewtwo::CAPTURE_AIR => self.enter_capture_mewtwo(victim, grabber, true),
            cb if super::capture_koopa::run_grabbed(self, victim, grabber, cb) => {}
            cb if super::capture_kirby::run_grabbed(self, victim, grabber, cb) => {}
            other => unported_call(&format!("grabbed callback {other:#010x}")),
        }
    }
}
