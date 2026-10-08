//! A match's entrance (`ft_0C31.c`): the fighter waits hidden for its player's delay
//! (`Entry`), rises out of its platform while the platform grows (`EntryStart`), then the
//! platform sinks away under it (`EntryEnd`) and it falls or waits.

use gekko_fp as fp;

use super::super::super::ef::{self, Arg, SpawnKind};
use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::super::super::structs::{Fighter, Vec2, ftCollisionBox};
use super::super::motion::flags::{KEEP_ACCESSORY, SKIP_COL_ANIM};
use crate::fpu::{copy, mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::quat::Quat;
use crate::vec::Vec3;

/// `ftCo_MS_EntryStart` and `ftCo_MS_EntryEnd`.
const MS_ENTRY_START: i32 = 323;
const MS_ENTRY_END: i32 = 324;

/// `ftCommon_MotionVars_entry`: the frames left, the fighter's height when it entered, its
/// model's scale and the squashed scale, the platform's full height, its scale, its height
/// now, and the ECB box.
const TIMER: usize = 0x0;
const X4: usize = 0x4;
const X8: usize = 0x8;
const X14: usize = 0x14;
const X20: usize = 0x20;
const X24: usize = 0x24;
const X28: usize = 0x28;
const X2C: usize = 0x2C;

/// `fn_800C69F4` and `fn_800C6F34`, the platform's accessory callbacks.
const PLATFORM_FOLLOW_START: u32 = 0x800C_69F4;
const PLATFORM_FOLLOW_END: u32 = 0x800C_6F34;

/// The platform's entrance effect, and its sound.
const GFX_ENTRY: i32 = 0x43E;
const SFX_ENTRY: i32 = 0x8B;
/// The color animation the fighter rises in.
const COLANIM_ENTRY: i32 = 0x75;

/// `M_PI_2` and the platform's height for its scale.
const HALF_PI: f64 = std::f64::consts::FRAC_PI_2;
const PLATFORM_HEIGHT: f64 = 1.497345;

fn mv_vec(fp: &Fighter, off: usize) -> Vec3 {
    Vec3::new(fp.mv_f32(off), fp.mv_f32(off + 4), fp.mv_f32(off + 8))
}

fn mv_box(fp: &Fighter, off: usize) -> ftCollisionBox {
    let f = |i: usize| fp.mv_f32(off + 4 * i);
    ftCollisionBox {
        top: f(0),
        bottom: f(1),
        left: Vec2 { x: f(2), y: f(3) },
        right: Vec2 { x: f(4), y: f(5) },
    }
}

impl Running<'_> {
    /// `ftCo_Entry_Anim`: once the delay runs out the fighter starts rising; the timer counts
    /// down either way.
    #[doc(alias = "ftCo_Entry_Anim")]
    pub fn co_entry_anim(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).mv_i32(TIMER) == 0 {
            self.enter_entry_start(gobj);
        }
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(TIMER, fp.mv_i32(TIMER).wrapping_sub(1));
    }

    /// `ftCo_Entry_IASA`, `ftCo_Entry_Phys`, `ftCo_Entry_Coll`, `ftCo_EntryStart_IASA` and
    /// `ftCo_EntryEnd_IASA`: nothing.
    #[doc(alias = "ftCo_Entry_IASA")]
    #[doc(alias = "ftCo_Entry_Phys")]
    #[doc(alias = "ftCo_Entry_Coll")]
    #[doc(alias = "ftCo_EntryStart_IASA")]
    #[doc(alias = "ftCo_EntryEnd_IASA")]
    pub fn co_entry_none(&mut self, _gobj: GObjId) {}

    /// Rising out of the platform, `ftCo_800C6408`: for `ftCommonData` x6BC frames, the
    /// platform (`Fighter_804D6514`'s model) at the fighter's feet in its trophy scale, with
    /// its effect, sound and color animation. A sub-fighter (Nana, Zelda or Sheik waiting)
    /// rides its partner's.
    #[doc(alias = "ftCo_800C6408")]
    pub fn enter_entry_start(&mut self, gobj: GObjId) {
        let frames = self.common().i32(0x6BC);
        self.game.fighter_mut(gobj).set_mv_i32(TIMER, frames);
        self.change_motion_state(gobj, MS_ENTRY_START, 0, 0.0, 1.0, 0.0, None);
        let f = self.game.fighter_id(gobj);
        self.game.fighters[f.0].x2219_b1 = true;
        if !self.game.fighters[f.0].is_sub_fighter {
            let model = self.common_ptr(0x40);
            self.set_accessory(gobj, model);
            let fp = &mut self.game.fighters[f.0];
            let s = mem(fp::fmuls(
                reg(fp.x34_scale.y),
                reg(fp.co_attrs.trophy_scale),
            ));
            fp.set_mv_f32(X24, reg(s));
            let acc = fp.x20A0_accessory.expect("fp->x20A0_accessory");
            let yaw = mem(fp::frsp(fp::fmul(HALF_PI, reg(fp.facing_dir))));
            let height = fp::frsp(fp::fmul(PLATFORM_HEIGHT, reg(s)));
            fp.set_mv_f32(X28, height);
            fp.set_mv_f32(X20, height);
            let jobjs = &mut self.game.objs.jobjs;
            jobj::set_scale(jobjs, acc, Vec3::new(s, s, s));
            jobj::set_rotation(jobjs, acc, Quat::new(0.0, yaw, 0.0, 0.0));
            self.platform_at_feet(gobj);
        }
        self.game.fighters[f.0].accessory1_cb = Some(CodeRef(PLATFORM_FOLLOW_START));
        let root = self.root_jobj(gobj);
        let scale = copy(self.game.fighters[f.0].mv_f32(X8));
        let mut head = self.game.fighters[f.0].x60C;
        ef::queue(
            self,
            Some(gobj),
            &mut head,
            SpawnKind::AttachParam,
            GFX_ENTRY,
            Some(root),
            &[Arg::Float(scale)],
        );
        self.game.fighters[f.0].x60C = head;
        self.sfx_play_entry(SFX_ENTRY);
        self.start_color_anim(gobj, COLANIM_ENTRY, 0);
    }

    /// Gives the fighter an accessory model, `ftCommon_SetAccessory`; it has none yet.
    #[doc(alias = "ftCommon_SetAccessory")]
    pub fn set_accessory(&mut self, gobj: GObjId, joint: melee_data::archive::DataRef) {
        let f = self.game.fighter_id(gobj);
        assert!(
            self.game.fighters[f.0].x20A0_accessory.is_none(),
            "the original's assertion failed: ftcommon.c:1158"
        );
        let j = jobj::load_joint(&mut self.game.objs, self.data, Some(joint));
        self.game.fighters[f.0].x20A0_accessory = j;
    }

    /// The platform under the fighter's feet: at its position less `ftCommon_800804EC`
    /// toward its facing.
    fn platform_at_feet(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        let offset = fp::fmuls(reg(fp.x40), reg(fp.x34_scale.x));
        let x = mem(fp::fnmsubs(reg(fp.facing_dir), offset, reg(fp.cur_pos.x)));
        let at = Vec3::new(x, fp.cur_pos.y, fp.cur_pos.z);
        let acc = fp.x20A0_accessory.expect("jobj");
        jobj::set_translate(&mut self.game.objs.jobjs, acc, at);
    }

    /// The main fighter of a sub-fighter's player.
    fn entry_partner(&self, gobj: GObjId) -> GObjId {
        let slot = i32::from(self.game.fighter(gobj).player_idx);
        self.game
            .player_entity_at(slot, 0)
            .expect("the player's main fighter")
    }

    /// A sub-fighter keeps to its partner's height.
    fn entry_partner_height(&mut self, gobj: GObjId) {
        let y = self.game.fighter(self.entry_partner(gobj)).cur_pos.y;
        self.game.fighter_mut(gobj).cur_pos.y = y;
    }

    /// `ftCo_EntryStart_Anim`: when its frames are up, the platform sinks away.
    #[doc(alias = "ftCo_EntryStart_Anim")]
    pub fn co_entry_start_anim(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(TIMER, fp.mv_i32(TIMER).wrapping_sub(1));
        if fp.mv_i32(TIMER) == 0 {
            self.enter_entry_end(gobj);
        }
    }

    /// `ftCo_EntryStart_Phys`: the fighter unsquashes from `ftCommonData` x6C4's height to
    /// its own as the frames pass, and the platform grows under it.
    #[doc(alias = "ftCo_EntryStart_Phys")]
    pub fn co_entry_start_phys(&mut self, gobj: GObjId) {
        let frames = self.common().i32(0x6BC);
        let low = reg(self.common().f32(0x6C4));
        let fp = self.game.fighter_mut(gobj);
        let t = fp::fdivs(
            fp::frsp(f64::from(frames.wrapping_sub(fp.mv_i32(TIMER)))),
            fp::frsp(f64::from(frames)),
        );
        let y = fp::fmadds(t, fp::fsubs(reg(fp.mv_f32(X8 + 4)), low), low);
        fp.set_mv_f32(X14 + 4, y);
        let scale = mv_vec(fp, X14);
        let root = self.root_jobj(gobj);
        jobj::set_scale(&mut self.game.objs.jobjs, root, scale);
        self.platform_rise(gobj, t);
    }

    /// The platform `t` of the way up: its scale's height, and the fighter on top of it; a
    /// sub-fighter at its partner's height.
    fn platform_rise(&mut self, gobj: GObjId, t: f64) {
        let fp = self.game.fighter_mut(gobj);
        if fp.is_sub_fighter {
            self.entry_partner_height(gobj);
            return;
        }
        let s = fp.mv_f32(X24);
        let scale = Vec3::new(s, mem(fp::fmuls(reg(s), t)), s);
        let acc = fp.x20A0_accessory.expect("jobj");
        jobj::set_scale(&mut self.game.objs.jobjs, acc, scale);
        let fp = self.game.fighter_mut(gobj);
        let h = fp::fmuls(reg(fp.mv_f32(X20)), t);
        fp.set_mv_f32(X28, h);
        fp.cur_pos.y = mem(fp::fadds(reg(fp.mv_f32(X4)), reg(fp.mv_f32(X28))));
    }

    /// `ftCo_EntryStart_Coll` and `ftCo_EntryEnd_Coll`: the ECB's bottom at the platform's
    /// top (its partner's for a sub-fighter); landing in the air
    /// ([`Self::co_entry_land`]), going airborne off the ground ([`Self::co_entry_airborne`]).
    #[doc(alias = "ftCo_EntryStart_Coll")]
    #[doc(alias = "ftCo_EntryEnd_Coll")]
    pub fn co_entry_coll(&mut self, gobj: GObjId) {
        let owner = if self.game.fighter(gobj).is_sub_fighter {
            self.entry_partner(gobj)
        } else {
            gobj
        };
        let h = reg(self.game.fighter(owner).mv_f32(X28));
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_f32(X2C + 4, fp::fneg(h));
        let b = mv_box(fp, X2C);
        if self.is_airborne(gobj) {
            self.air_coll_box_then(gobj, &b, |g, gobj| g.co_entry_land(gobj));
        } else {
            self.ground_coll_box_then(gobj, &b, |g, gobj| g.co_entry_airborne(gobj));
        }
    }

    /// `fn_800C63BC`: lands (`ftCommon_8007D7FC`).
    // Ports `fn_800C63BC`; aliased once a check reaches it.
    pub fn co_entry_land(&mut self, gobj: GObjId) {
        self.land(gobj);
    }

    /// `fn_800C63E0`: goes airborne (`ftCommon_8007D5D4`).
    // Ports `fn_800C63E0`; aliased once a check reaches it.
    pub fn co_entry_airborne(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.go_airborne(f);
    }

    /// The platform follows the fighter, `fn_800C69F4` and `fn_800C6F34`: raised to the
    /// player's other fighter where that one is awake and higher, then the platform at its
    /// feet; a sub-fighter keeps to its partner's height.
    #[doc(alias = "fn_800C69F4")]
    #[doc(alias = "fn_800C6F34")]
    pub fn entry_platform_follow(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).is_sub_fighter {
            self.entry_partner_height(gobj);
            return;
        }
        let slot = i32::from(self.game.fighter(gobj).player_idx);
        if let Some(other) = self.game.player_entity_at(slot, 1) {
            let o = self.game.fighter(other);
            let y = o.cur_pos.y;
            if !o.is_sleeping && y > self.game.fighter(gobj).cur_pos.y {
                self.game.fighter_mut(gobj).cur_pos.y = y;
            }
        }
        self.platform_at_feet(gobj);
    }

    /// The platform sinks away, `ftCo_800C6B6C`: for `ftCommonData` x6C0 frames, the fighter
    /// at its own scale on top of the platform.
    #[doc(alias = "ftCo_800C6B6C")]
    pub fn enter_entry_end(&mut self, gobj: GObjId) {
        let frames = self.common().i32(0x6C0);
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(TIMER, frames);
        let scale = mv_vec(fp, X8);
        let root = self.root_jobj(gobj);
        jobj::set_scale(&mut self.game.objs.jobjs, root, scale);
        if self.game.fighter(gobj).is_sub_fighter {
            self.entry_partner_height(gobj);
        } else {
            let fp = self.game.fighter_mut(gobj);
            fp.cur_pos.y = mem(fp::fadds(reg(fp.mv_f32(X4)), reg(fp.mv_f32(X20))));
        }
        let keep = SKIP_COL_ANIM | KEEP_ACCESSORY;
        self.change_motion_state(gobj, MS_ENTRY_END, keep, 0.0, 1.0, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        fp.x2219_b1 = true;
        fp.accessory1_cb = Some(CodeRef(PLATFORM_FOLLOW_END));
    }

    /// `ftCo_EntryEnd_Anim`: when its frames are up, the fighter falls or waits, intangible
    /// for `ftCommonData` x6C8 frames if its player's flag bit 4 says so.
    #[doc(alias = "ftCo_EntryEnd_Anim")]
    pub fn co_entry_end_anim(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(TIMER, fp.mv_i32(TIMER).wrapping_sub(1));
        if fp.mv_i32(TIMER) == 0 {
            let slot = i32::from(fp.player_idx);
            if self.game.player_flags_bit4(slot) {
                let frames = self.common().i32(0x6C8);
                self.set_intangible(gobj, frames);
            }
            self.fall_or_wait(gobj);
        }
    }

    /// `ftCo_EntryEnd_Phys`: the platform shrinks over the frames left (of `ftCommonData`
    /// x6BC's), the fighter on top of it.
    #[doc(alias = "ftCo_EntryEnd_Phys")]
    pub fn co_entry_end_phys(&mut self, gobj: GObjId) {
        let frames = self.common().i32(0x6BC);
        let fp = self.game.fighter(gobj);
        if fp.is_sub_fighter {
            self.entry_partner_height(gobj);
            return;
        }
        let t = fp::fdivs(
            fp::frsp(f64::from(fp.mv_i32(TIMER))),
            fp::frsp(f64::from(frames)),
        );
        self.platform_rise(gobj, t);
    }
}
