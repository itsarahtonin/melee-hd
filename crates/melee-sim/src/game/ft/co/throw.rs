//! Throws and being thrown (`ftCo_Throw.c`, `ftCo_Thrown.c`): the throw's speed by the
//! victim's weight, the victim fixed to the thrower's model through it, the release with the
//! throw's hit, and the victim's mash out of a slow throw. Kirby's, Samus's, Yoshi's and
//! Bowser's own parts, and an airborne throw's collision, stop at their seams.

use gekko_fp as fp;

use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::super::common::GA_AIR;
use super::super::parts::{HIP_N, TRANS_N2};
use super::checks::KIND_YOSHI;
use super::ids::ms;
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::hsd::robj;
use crate::lb::joint::world_position;
use crate::vec::Vec3;

/// `FtPart_XRotN`.
const X_ROT_N: i32 = 2;
/// `Ft_Kind_Kirby`, `Ft_Kind_Koopa`, `Ft_Kind_Samus`, `Ft_Kind_GKoopa`.
const KIND_KIRBY: i32 = 4;
const KIND_KOOPA: i32 = 5;
const KIND_SAMUS: i32 = 13;
const KIND_GKOOPA: i32 = 31;

/// `ftCommon_MotionVars_fighterthrow`: frozen at the release, released, and the victim (a
/// thrown fighter's thrower, for `fn_800DE798`); `throw_`'s `xC`, the motion it froze with.
const THROW_FROZEN: usize = 0x4;
const THROW_RELEASED: usize = 0x8;
const THROW_VICTIM: usize = 0xC;
const THROW_MOTION_Y: usize = 0x10;
const THROW_MOTION_Z: usize = 0x14;
/// `ftCommon_MotionVars_thrown`: frozen with the thrower, the frame it freezes at, and a jump
/// buffered for a mash out (`xC`).
const THROWN_FROZEN: usize = 0x0;
const THROWN_FREEZE_FRAME: usize = 0x4;
const THROWN_JUMP: usize = 0xC;

/// The thrown fighter's callbacks.
const THROWN_FOLLOW: u32 = 0x800D_E508;
const THROWN_ON_CHANGE: u32 = 0x800D_E798;

impl Running<'_> {
    /// A throw, `ftCo_800DD4B0` (`ftCo_800DD398`): at a speed by the victim's weight unless
    /// the character's mask makes the throw weight-independent, the victim into the matching
    /// thrown state, and the thrower's knockback hitbox.
    #[doc(alias = "ftCo_800DD4B0")]
    #[doc(alias = "ftCo_800DD398")]
    pub fn enter_throw(&mut self, gobj: GObjId, msid: i32) {
        let victim = self
            .game
            .fighter(gobj)
            .victim_gobj
            .expect("throwing no one");
        let index = msid - ms::THROW_F;
        let mut victim_msid = index + ms::THROWN_F;
        let attrs = |g: &Self, o: GObjId| {
            let d = g.game.fighter(o).ft_data.expect("a fighter without data");
            g.data.ptr(d).expect("ftData::x0")
        };
        let mine = attrs(self, gobj);
        let speed = if self.data.u8(mine.at(0x180)) & (1 << index) != 0 {
            1.0
        } else {
            let weight = self.data.f32(attrs(self, victim).at(0x88));
            let k = self.common().f32(0x37C);
            fp::fdivs(1.0, fp::fmuls(reg(weight), reg(k)))
        };
        let kind = self.game.fighter(gobj).kind;
        if msid == ms::THROW_LW && (kind == KIND_GKOOPA || kind == KIND_KOOPA) {
            let vk = self.game.fighter(victim).kind;
            if vk == 9 || vk == 19 {
                victim_msid = 243;
            }
        }
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[0] = 0;
        fp.x2210 = [0; 4];
        fp.set_mv_i32(THROW_FROZEN, 0);
        fp.set_mv_i32(THROW_RELEASED, 0);
        self.change_motion_state(gobj, msid, 0, 0.0, mem(speed), 0.0, None);
        self.anim_update(gobj);
        self.game.fighter_mut(gobj).x1A6A = 0x1FF;
        self.enter_thrown(victim, victim_msid, mem(speed));
        match kind {
            KIND_KIRBY if msid == ms::THROW_HI => {
                self.game.fighter_mut(gobj).x2219_b2 = true;
                self.game.fighter_mut(victim).x2219_b2 = true;
            }
            KIND_SAMUS => self.samus_throw_beam(gobj, msid, mem(speed)),
            _ => {}
        }
        let frames = self.common().i32(0x348);
        self.knockback_hitbox(gobj, frames);
    }

    /// The throw's progress, `ftCo_800DD724`: the script turns the thrower around, releases
    /// the victim with the throw's hit and launches it (up for the down throw), or freezes
    /// both for a moment, keeping the motion it froze with.
    #[doc(alias = "ftCo_800DD724")]
    pub fn throw_progress(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.throw_flag(4) {
            fp.set_throw_flag(4, false);
            fp.facing_dir = mem(fp::fneg(reg(fp.facing_dir)));
        }
        let fp = self.game.fighter_mut(gobj);
        if fp.throw_flag(3) {
            fp.set_throw_flag(3, false);
            fp.x1A6A = 0;
            if let Some(victim) = fp.victim_gobj {
                let (slot, timer) = (
                    i32::from(fp.player_idx),
                    self.game.fighter(victim).grab_timer,
                );
                self.record_quick_throw(slot, timer);
                self.throw_hit(gobj, victim, true);
                let down = self.game.fighter(gobj).motion_id == ms::THROW_LW;
                self.thrown_launch(victim, Some(gobj), down);
            }
        }
        let fp = self.game.fighter_mut(gobj);
        if fp.cmd_vars[0] != 0 && fp.mv_i32(THROW_FROZEN) == 0 {
            fp.set_mv_i32(THROW_FROZEN, 1);
            fp.cmd_vars[0] = 0;
            let t = fp.x6A4_transNOffset;
            fp.set_mv_f32(THROW_VICTIM, reg(t.x));
            fp.set_mv_f32(THROW_MOTION_Y, reg(t.y));
            fp.set_mv_f32(THROW_MOTION_Z, reg(t.z));
            self.set_anim_rate(gobj, 0.0);
            let fp = self.game.fighter(gobj);
            if let Some(victim) = fp.victim_gobj {
                let frame = fp.cur_anim_frame;
                self.thrown_freeze(victim, frame);
            }
            let fp = self.game.fighter(gobj);
            if fp.kind == KIND_KIRBY && fp.motion_id == ms::THROW_HI {
                let victim = fp.victim_gobj;
                self.game.fighter_mut(gobj).x2219_b2 = false;
                if let Some(v) = victim {
                    self.game.fighter_mut(v).x2219_b2 = false;
                }
            }
        }
    }

    /// `ftCo_ThrowF_Anim`: Mewtwo's shadow ball, the throw's progress, then carrying the
    /// victim for a fighter with cargo attributes (`x2222_b0`), else falling or waiting.
    #[doc(alias = "ftCo_ThrowF_Anim")]
    pub fn co_throw_f_anim(&mut self, gobj: GObjId) {
        self.mt_n_shoot(gobj);
        self.throw_progress(gobj);
        if !self.anim_frames_remaining(gobj) {
            if self.game.fighter(gobj).x2222_b0 {
                self.enter_cargo_start(gobj);
            } else {
                self.fall_or_wait(gobj);
            }
        }
    }

    /// `ftCo_ThrowB_Anim`, `ftCo_ThrowHi_Anim`, `ftCo_ThrowLw_Anim` (and the cargo throws' that
    /// call them): Fox's and Falco's blaster (`ftFx_Throw_Anim`), the throw's progress, then
    /// falling or waiting.
    #[doc(alias = "ftCo_ThrowB_Anim")]
    #[doc(alias = "ftCo_ThrowHi_Anim")]
    #[doc(alias = "ftCo_CargoThrowB_Anim")]
    #[doc(alias = "ftCo_CargoThrowHi_Anim")]
    #[doc(alias = "ftCo_CargoThrowLw_Anim")]
    #[doc(alias = "ftCo_ThrowLw_Anim")]
    pub fn co_throw_anim(&mut self, gobj: GObjId) {
        self.fox_throw_anim(gobj);
        self.throw_end(gobj);
    }

    fn throw_end(&mut self, gobj: GObjId) {
        self.throw_progress(gobj);
        if !self.anim_frames_remaining(gobj) {
            self.fall_or_wait(gobj);
        }
    }

    /// The throws' collision (`ftCo_ThrowF_Coll` and the rest): on the ground, off a ledge
    /// both let go and fall (`fn_800DD684`). In the air (Kirby's throws), frozen it lands on
    /// any floor it touches (`fn_800DD568`), before its release on any floor but a platform
    /// (`ft_80083CE4`, `fn_800DD5EC`), and after it only touches.
    #[doc(alias = "ftCo_ThrowF_Coll")]
    #[doc(alias = "ftCo_ThrowB_Coll")]
    // Ports `ftCo_ThrowHi_Coll`; aliased once a check reaches it.
    #[doc(alias = "ftCo_ThrowLw_Coll")]
    // Ports `fn_800DD684`; aliased once a check reaches it.
    pub fn co_throw_coll(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        if fp.ground_or_air == GA_AIR {
            if fp.mv_i32(THROW_FROZEN) != 0 {
                if self.air_coll_touch(gobj) {
                    self.throw_land_frozen(gobj);
                }
            } else if fp.mv_i32(THROW_RELEASED) == 0 {
                if self.air_coll_throw(gobj) {
                    self.throw_land(gobj);
                }
            } else {
                self.air_coll_touch(gobj);
            }
            return;
        }
        self.ground_coll_then(gobj, |g, gobj| {
            if let Some(victim) = g.game.fighter(gobj).victim_gobj {
                g.grab_release(gobj, victim);
                g.enter_fall(victim);
            }
            g.enter_fall(gobj);
        });
    }

    /// The throw's hit, `ftCo_800DDDE4` (`ftCo_800DE2A8` with `place`): the thrower lets go,
    /// the throw's first hitbox counts and deals its damage and knockback (unless the victim
    /// is invincible), the hit is recorded and rumbles, and the victim, freed from the
    /// thrower's model, is placed where it was held (at its throw offset with `place`).
    #[doc(alias = "ftCo_800DDDE4")]
    #[doc(alias = "ftCo_800DE2A8")]
    #[doc(alias = "ftCo_800DE2CC")]
    #[doc(alias = "ftColl_80076640")]
    #[doc(alias = "ftColl_80078710")]
    #[doc(alias = "ftCommon_8007EE0C")]
    pub(crate) fn throw_hit(&mut self, gobj: GObjId, victim: GObjId, place: bool) {
        let (holder, held) = if self.game.fighter(gobj).x221B.x221B_b7 {
            (victim, gobj)
        } else {
            (gobj, victim)
        };
        let fp = self.game.fighter_mut(gobj);
        fp.x1A5C = None;
        fp.victim_gobj = None;
        fp.x221B.x221B_b5 = false;
        fp.x221B.x221B_b7 = false;
        let hit = self.game.fighter(gobj).xDF4[0].clone();
        self.record_fighter_hit(gobj, victim, reg(hit.damage));
        let v = self.game.fighter_id(victim);
        if self.game.fighters[v.0].x1988 != 0 {
            self.set_hit_status(victim, 0);
        }
        let tj = self.part_joint(holder, TRANS_N2);
        let mut pos = world_position(&mut self.game.objs, self.data, tj, None);
        let mut dmg = if self.hit_status(v) == 0 {
            reg(hit.damage)
        } else {
            0.0
        };
        // `ftColl_80076640`: the damage taken, or the armor's.
        let count = super::super::attack::whole_damage(dmg);
        let vf = &mut self.game.fighters[v.0];
        if vf.x221C_b4 {
            vf.dmg.x1834 = mem(fp::fsubs(reg(vf.dmg.x1834), dmg));
            if vf.dmg.x1834 < 0.0 {
                dmg = fp::fneg(reg(vf.dmg.x1834));
                vf.x221C_b4 = false;
            }
        }
        if !vf.x221C_b4 {
            // NaN passes, as the original's `!(dmg > 500.0)` does.
            assert!(
                dmg.partial_cmp(&500.0) != Some(std::cmp::Ordering::Greater),
                "attack power over 500!! {dmg}"
            );
            vf.dmg.x1838_percentTemp = mem(fp::fadds(reg(vf.dmg.x1838_percentTemp), dmg));
            if count > vf.dmg.x183C_applied {
                vf.dmg.x183C_applied = count;
            }
        }
        let (aslot, vslot) = (
            i32::from(self.game.fighter(gobj).player_idx),
            i32::from(self.game.fighters[v.0].player_idx),
        );
        let ratio = self.game.vs_damage_ratio();
        let attack = self.game.player_attack_ratio(aslot);
        let defense = self.game.player_defense_ratio(vslot);
        let weight = self.common().f32(0x10C);
        let me = self.game.fighter_id(gobj);
        let kb = self.knockback(
            v,
            super::super::attack::LoggedHit::Throw {
                fighter: me,
                index: 0,
            },
            hit.unk_count,
            ratio,
            attack,
            defense,
            weight,
        );
        let facing = self.game.fighter(gobj).facing_dir;
        let vf = &mut self.game.fighters[v.0];
        vf.dmg.kb_applied = mem(kb);
        vf.dmg.x1848_kb_angle = hit.kb_angle;
        vf.dmg.facing_dir_1 = mem(fp::fneg(reg(facing)));
        vf.dmg.x184c_damaged_hurtbox = 1;
        vf.dmg.x1854_collpos = pos;
        vf.dmg.x1860_element = hit.element;
        // `ftColl_80078710`: the attack's record, the facing word as the keep flag.
        let af = self.game.fighter(gobj);
        let (akind, x2070, instance) = (af.kind, u32::from_be_bytes(af.x2070), af.x2074.x2088);
        self.attack_landed(
            Some(gobj),
            victim,
            1,
            akind,
            x2070,
            super::super::super::pl::trick::AttackRecord::Fighter(gobj),
            instance,
            true,
            false,
        );
        // `ftCommon_8007EE0C`: the thrower's rumble by the damage.
        let c = self.common();
        let t = fp::fmadds(
            fp::frsp(f64::from(fp::fctiwz(reg(hit.damage)))),
            reg(c.f32(0xEC)),
            reg(c.f32(0xF0)),
        );
        if fp::fctiwz(t) >= 1 {
            let f = self.game.fighter_id(gobj);
            self.start_rumble(f, 10, fp::fctiwz(t) as u32);
        }
        self.calc_knockback(v);
        let h = self.game.fighter_id(held);
        self.go_airborne(h);
        if self.game.fighters[h.0].x2226_b2 {
            let xj = self.part_joint(held, X_ROT_N).expect("XRotN");
            unconstrain(&mut self.game.objs, xj);
            let fp = &mut self.game.fighters[h.0];
            fp.x2226_b2 = false;
            let x2174 = fp.x2174;
            jobj::set_translate(&mut self.game.objs.jobjs, xj, x2174);
            let fp = &self.game.fighters[h.0];
            if place {
                pos.x = mem(fp::fmadds(
                    reg(fp.facing_dir),
                    fp::fmuls(reg(fp.x1A70.z), reg(fp.x34_scale.y)),
                    reg(pos.x),
                ));
                pos.y = mem(fp::fmadds(reg(fp.x1A70.y), reg(fp.x34_scale.y), reg(pos.y)));
                pos.z = 0.0;
            }
            let hp = &self.game.fighter(holder);
            let mid = fp::fmuls(
                0.5,
                fp::fadds(reg(hp.coll_data.ecb.top.y), reg(hp.coll_data.ecb.bottom.y)),
            );
            let last = Vec3::new(
                mem(fp::fadds(0.0, reg(hp.cur_pos.x))),
                mem(fp::fadds(reg(hp.cur_pos.y), mid)),
                mem(fp::fadds(0.0, reg(hp.cur_pos.z))),
            );
            let root = self.root_jobj(held);
            jobj::set_translate(&mut self.game.objs.jobjs, root, pos);
            super::super::fighter::unlock_ecb(&mut self.game.fighters[h.0]);
            self.fighter_coll_placed(h, last, pos, |c, cd| c.air_frame(cd, 6, 0));
            let p = self.game.fighters[h.0].cur_pos;
            jobj::set_translate(&mut self.game.objs.jobjs, root, p);
        }
        let vf = &mut self.game.fighters[v.0];
        vf.x1A5C = None;
        vf.victim_gobj = None;
        vf.x221B.x221B_b7 = false;
    }

    /// A slow throw's quick release counts toward a bonus, `pl_80040614`: a victim the
    /// thrower held for no more than `pl_804D6470` x4C frames' worth of its timer.
    #[doc(alias = "pl_80040614")]
    fn record_quick_throw(&mut self, slot: i32, timer: f32) {
        if timer == 0.0 {
            return;
        }
        self.record_throw_timer(slot, timer);
    }

    /// Thrown, `ftCo_800DE3FC`: fixed to the thrower's model (`ftCo_800DB368`), facing as
    /// it does, into `msid` at the throw's speed, its mash timer spent (counted toward the
    /// thrower's bonus), following its throw point.
    #[doc(alias = "ftCo_800DE3FC")]
    #[doc(alias = "ftCommon_8007E358")]
    pub fn enter_thrown(&mut self, gobj: GObjId, msid: i32, speed: f32) {
        let thrower = self
            .game
            .fighter(gobj)
            .victim_gobj
            .expect("thrown by no one");
        self.attach_to_thrower(thrower, gobj);
        let tf = self.game.fighter(thrower);
        let (facing, tkind, tslot) = (tf.facing_dir, tf.kind, i32::from(tf.player_idx));
        let fp = self.game.fighter_mut(gobj);
        fp.facing_dir = facing;
        fp.set_mv_i32(THROWN_FROZEN, 0);
        self.change_motion_state(gobj, msid, 0, 0.0, speed, 0.0, Some(thrower));
        if tkind == KIND_YOSHI {
            self.set_hit_status(gobj, 2);
        }
        if tkind != KIND_KIRBY || (msid - ms::THROWN_F) as u32 > 1 {
            let timer = self.game.fighter(gobj).grab_timer;
            self.record_quick_throw(tslot, timer);
            self.game.fighter_mut(gobj).grab_timer = 0.0;
        }
        let fp = self.game.fighter_mut(gobj);
        fp.accessory1_cb = Some(CodeRef(THROWN_FOLLOW));
        fp.x1A6A = 0x1FF;
        // `ftCommon_8007E358`: the hip's translation as it is now.
        let hj = self.part_joint(gobj, HIP_N).expect("HipN");
        let t = self.game.objs.jobjs[hj].translate;
        self.game.fighter_mut(gobj).x1A7C = t;
        self.anim_update(gobj);
    }

    /// The victim fixed to the thrower's throw bone, `ftCo_800DB368`: its XRotN turned
    /// straight, its translation kept, and constrained to the thrower's TransN2.
    #[doc(alias = "ftCo_800DB368")]
    #[doc(alias = "ftParts_JObjSetRotation")]
    pub(crate) fn attach_to_thrower(&mut self, thrower: GObjId, gobj: GObjId) {
        let target = self.part_joint(thrower, TRANS_N2).expect("TransN2");
        if self.game.fighter(gobj).x2226_b2 {
            return;
        }
        let j = self.part_joint(gobj, X_ROT_N).expect("XRotN");
        jobj::set_rotation(
            &mut self.game.objs.jobjs,
            j,
            crate::quat::Quat {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 0.0,
            },
        );
        self.game.objs.jobjs[j].flags &= !jobj::USE_QUATERNION;
        let t = self.game.objs.jobjs[j].translate;
        self.game.fighter_mut(gobj).x2174 = t;
        super::super::super::ef::constrain(&mut self.game.objs, j, target, 0x9000_0001);
        self.game.fighter_mut(gobj).x2226_b2 = true;
    }

    /// The thrown fighter at its throw point, `ftCo_800DE508`: its XRotN's place offset by
    /// its throw offset (`x1A70`).
    #[doc(alias = "ftCo_800DE508")]
    pub fn thrown_follow(&mut self, gobj: GObjId) {
        let j = self.part_joint(gobj, X_ROT_N);
        let mut pos = world_position(&mut self.game.objs, self.data, j, None);
        let fp = self.game.fighter_mut(gobj);
        pos.x = mem(fp::fmadds(
            reg(fp.facing_dir),
            fp::fmuls(reg(fp.x1A70.z), reg(fp.x34_scale.y)),
            reg(pos.x),
        ));
        pos.y = mem(fp::fmadds(reg(fp.x1A70.y), reg(fp.x34_scale.y), reg(pos.y)));
        pos.z = 0.0;
        fp.cur_pos = pos;
    }

    /// The thrown states' animation, `ftCo_800DE5A4` (`ftCo_ThrownF_Anim` and the rest):
    /// frozen with the thrower at its frame, else the hip's motion damped
    /// (`ftCommon_8007E3EC`); a mash timer left counts down and lets it escape.
    #[doc(alias = "ftCo_800DE5A4")]
    #[doc(alias = "ftCo_ThrownF_Anim")]
    #[doc(alias = "ftCo_ThrownB_Anim")]
    // Ports `ftCo_ThrownHi_Anim`; aliased once a check reaches it.
    #[doc(alias = "ftCo_ThrownLw_Anim")]
    pub fn co_thrown_anim(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        if fp.mv_i32(THROWN_FROZEN) != 0 {
            let at = fp.mv_f32(THROWN_FREEZE_FRAME);
            if at != 0.0 && at == fp.cur_anim_frame {
                self.set_anim_rate(gobj, 0.0);
                self.game
                    .fighter_mut(gobj)
                    .set_mv_f32(THROWN_FREEZE_FRAME, 0.0);
                self.damp_hip(gobj);
            }
        } else {
            self.damp_hip(gobj);
        }
        if self.game.fighter(gobj).grab_timer != 0.0 {
            let c = self.common();
            let (dec, mash) = (c.f32(0x3A4), c.f32(0x3C8));
            let fp = self.game.fighter_mut(gobj);
            fp.grab_timer = mem(fp::fsubs(reg(fp.grab_timer), reg(dec)));
            let _ = self.grab_mash(gobj, mash);
            if self.game.fighter(gobj).grab_timer <= 0.0 {
                if self.game.fighter(gobj).mv_i32(THROWN_JUMP) != 0 || self.capture_jump_held(gobj)
                {
                    self.enter_capture_jump(gobj);
                } else {
                    self.enter_capture_cut(gobj);
                }
            }
        }
    }

    /// The hip follows its animation part way, `ftCommon_8007E3EC`: while its matrix is
    /// dirty, `x1A6C` of the way from where it was.
    #[doc(alias = "ftCommon_8007E3EC")]
    fn damp_hip(&mut self, gobj: GObjId) {
        let j = self.part_joint(gobj, HIP_N).expect("HipN");
        if !self.game.objs.jobjs[j].mtx_is_dirty() {
            return;
        }
        let t = self.game.objs.jobjs[j].translate;
        let fp = self.game.fighter_mut(gobj);
        let k = reg(fp.x1A6C);
        let p = &mut fp.x1A7C;
        let d = (
            fp::fsubs(reg(t.x), reg(p.x)),
            fp::fsubs(reg(t.y), reg(p.y)),
            fp::fsubs(reg(t.z), reg(p.z)),
        );
        p.x = mem(fp::fmadds(d.0, k, reg(p.x)));
        p.y = mem(fp::fmadds(d.1, k, reg(p.y)));
        p.z = mem(fp::fmadds(d.2, k, reg(p.z)));
        let p = *p;
        jobj::set_translate(&mut self.game.objs.jobjs, j, p);
    }

    /// The thrown fighter launched, `ftCo_800DE7C0`: hitstun (the down throw's into a
    /// tumble upward), facing back when the angle sends it behind, its thrower remembered
    /// for its thrown hitbox on its next action (`fn_800DE798`), and DI.
    #[doc(alias = "ftCo_800DE7C0")]
    pub(crate) fn thrown_launch(&mut self, gobj: GObjId, thrower: Option<GObjId>, upward: bool) {
        if let Some(t) = thrower {
            let fp = self.game.fighter_mut(gobj);
            fp.x21EC = Some(CodeRef(THROWN_ON_CHANGE));
            fp.mv.set_gobj_at(THROW_VICTIM, Some(t));
        }
        let fp = self.game.fighter(gobj);
        let angle = fp.dmg.x1848_kb_angle;
        let facing = if angle > 90 && angle < 270 {
            mem(fp::fneg(reg(fp.dmg.facing_dir_1)))
        } else {
            0.0
        };
        self.enter_damage(gobj, if upward { 90 } else { -1 }, facing);
        self.directional_influence(gobj);
    }

    /// The thrown fighter's hitbox takes its thrower, `fn_800DE798` (`ftColl_8007B8CC`).
    #[doc(alias = "fn_800DE798")]
    pub fn thrown_on_change(&mut self, gobj: GObjId) {
        let thrower = self
            .game
            .fighter(gobj)
            .mv
            .gobj_at(THROW_VICTIM)
            .expect("no thrower");
        let f = self.game.fighter_id(gobj);
        self.thrown_hitbox_owner(f, thrower);
    }

    /// The thrown fighter freezes with the thrower, `ftCo_800DE920`: now if it's at the
    /// frame, else when it reaches it.
    // Ports `ftCo_800DE920`; aliased once a check reaches it.
    fn thrown_freeze(&mut self, gobj: GObjId, frame: f32) {
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(THROWN_FROZEN, 1);
        if fp.cur_anim_frame == frame {
            self.set_anim_rate(gobj, 0.0);
            self.game
                .fighter_mut(gobj)
                .set_mv_f32(THROWN_FREEZE_FRAME, 0.0);
        } else {
            fp.set_mv_f32(THROWN_FREEZE_FRAME, reg(frame));
        }
    }

    /// Landing frozen in an aerial throw, `fn_800DD568`: its animation on again, the throw
    /// released, the victim unfrozen, on the ground.
    #[doc(alias = "fn_800DD568")]
    pub fn throw_land_frozen(&mut self, gobj: GObjId) {
        self.set_anim_rate(gobj, 1.0);
        self.anim_update(gobj);
        self.throw_land_common(gobj, false);
    }

    /// Landing before an aerial throw's freeze, `fn_800DD5EC`: its animation run on to the
    /// script's freeze, then as [`Self::throw_land_frozen`].
    // Ports `fn_800DD5EC`; aliased once a check reaches it.
    pub fn throw_land(&mut self, gobj: GObjId) {
        while self.game.fighter(gobj).cmd_vars[0] == 0 {
            self.anim_update(gobj);
        }
        self.throw_land_common(gobj, true);
    }

    fn throw_land_common(&mut self, gobj: GObjId, rate: bool) {
        if let Some(cb) = self.game.fighter(gobj).anim_cb {
            super::super::super::code::run_fighter_cb(self, gobj, cb);
        }
        if rate {
            self.set_anim_rate(gobj, 1.0);
        }
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(THROW_FROZEN, 0);
        fp.set_mv_i32(THROW_RELEASED, 1);
        if let Some(v) = fp.victim_gobj {
            self.thrown_unfreeze(v);
        }
        self.land(gobj);
    }

    /// The thrown fighter's animation on again, `ftCo_800DE974`.
    #[doc(alias = "ftCo_800DE974")]
    pub fn thrown_unfreeze(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).set_mv_i32(THROWN_FROZEN, 0);
        self.set_anim_rate(gobj, 1.0);
        self.anim_update(gobj);
    }

    /// `ftCo_ThrowHi_Cam`: during Kirby's up throw, only the camera bone's point follows.
    #[doc(alias = "ftCo_ThrowHi_Cam")]
    pub fn co_throw_hi_cam(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        if fp.kind == KIND_KIRBY && fp.motion_id == ms::THROW_HI {
            self.update_camera_bone(gobj);
        } else {
            self.update_camera_box(gobj);
        }
    }

    /// `ftCo_ThrownHi_Cam`: thrown up by Kirby, only the camera bone's point follows.
    #[doc(alias = "ftCo_ThrownHi_Cam")]
    pub fn co_thrown_hi_cam(&mut self, gobj: GObjId) {
        let t = self
            .game
            .fighter(gobj)
            .victim_gobj
            .expect("thrown by no one");
        let tf = self.game.fighter(t);
        if tf.kind == KIND_KIRBY && tf.motion_id == ms::THROW_HI {
            self.update_camera_bone(gobj);
        } else {
            self.update_camera_box(gobj);
        }
    }

    /// The thrown states' empty callbacks.
    #[doc(alias = "ftCo_ThrowF_IASA")]
    #[doc(alias = "ftCo_CargoThrowF_IASA")]
    #[doc(alias = "ftCo_CargoThrowB_IASA")]
    #[doc(alias = "ftCo_CargoThrowHi_IASA")]
    #[doc(alias = "ftCo_CargoThrowLw_IASA")]
    #[doc(alias = "ftCo_ThrowB_IASA")]
    // Ports `ftCo_ThrowHi_IASA`; aliased once a check reaches it.
    #[doc(alias = "ftCo_ThrowLw_IASA")]
    #[doc(alias = "ftCo_ThrownF_IASA")]
    #[doc(alias = "ftCo_ThrownF_Phys")]
    #[doc(alias = "ftCo_ThrownF_Coll")]
    #[doc(alias = "ftCo_ThrownB_IASA")]
    #[doc(alias = "ftCo_ThrownB_Phys")]
    #[doc(alias = "ftCo_ThrownB_Coll")]
    // Ports `ftCo_ThrownHi_IASA`; aliased once a check reaches it.
    // Ports `ftCo_ThrownHi_Phys`; aliased once a check reaches it.
    // Ports `ftCo_ThrownHi_Coll`; aliased once a check reaches it.
    #[doc(alias = "ftCo_ThrownLw_IASA")]
    #[doc(alias = "ftCo_ThrownLw_Coll")]
    pub fn co_throw_nothing(&mut self, _gobj: GObjId) {}
}

/// Removes a joint's joint constraints, `lb_8000C390`.
#[doc(alias = "lb_8000C390")]
pub(crate) fn unconstrain(objs: &mut crate::hsd::Objects, j: crate::hsd::jobj::JObjId) {
    let mut cur = objs.jobjs[j].robj;
    loop {
        cur = robj::get_by_type(&objs.robjs, cur, 0x1000_0000, 0);
        let Some(c) = cur else { break };
        let next = objs.robjs[c].next;
        jobj::delete_robj(objs, Some(j), Some(c));
        robj::remove(objs, Some(c));
        cur = next;
    }
}
