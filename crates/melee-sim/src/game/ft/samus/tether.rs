//! The common states' Samus parts (`ftCo_0D95.c`, `ftCo_AirCatch.c`, `ftCo_Escape.c`,
//! `ftCo_CliffJump.c`): her grab and dash grab shooting the grapple beam on their frames, her
//! air grab and its latch, her roll as the morph ball, and the beam's end in a ledge jump.

// NaN takes the original's branches, which the negated comparisons spell out.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::super::ef::{self, Arg};
use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::samusgrapple::GRAPPLE;
use super::super::super::it::kinds::tether::{self, Tether};
use super::super::super::state::Running;
use super::super::common::{GA_AIR, GA_GROUND};
use super::{attr, ms};
use crate::fpu::{mem, reg};
use crate::game::it::ItKind;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::mp::coll::{LEFT_LEDGE_GRAB, RIGHT_LEDGE_GRAB};
use crate::vec::Vec3;

/// The grab's frame counter (`mv.co.catch_.x0`, the air grab's `aircatch.x0`, a float).
const MV_FRAME: usize = 0x0;
/// The roll's ball (`mv.co.escape.x4`).
const MV_ESCAPE_BALL: usize = 0x4;
/// `mv.co.aircatchhit`'s `x0` and `x4`.
const MV_HIT_X0: usize = 0x0;
const MV_HIT_X4: usize = 0x4;
/// The beam's sparks while it's shot.
const GFX_SPARK: i32 = 0x3F3;
/// `Ft_MF_KeepFastFall`.
const KEEP_FAST_FALL: u32 = 1;
/// The roll's own animation and collision callbacks.
pub const ROLL_ANIM: u32 = 0x8009_9564;
pub const ROLL_COLL: u32 = 0x8009_9754;
/// `ftCo_AirCatch` and `ftCo_AirCatchHit`'s callbacks.
pub const AIR_CATCH_ANIM: u32 = 0x800C_3D6C;
pub const AIR_CATCH_HIT_ANIM: u32 = 0x800C_4380;
pub const AIR_CATCH_IASA: u32 = 0x800C_4384;
pub const AIR_CATCH_HIT_IASA: u32 = 0x800C_4388;
pub const AIR_CATCH_PHYS: u32 = 0x800C_438C;
pub const AIR_CATCH_HIT_PHYS: u32 = 0x800C_4438;
pub const AIR_CATCH_COLL: u32 = 0x800C_447C;
/// `ftCo_MS_Landing` and the ledge jumps' second parts.
const MS_LANDING: i32 = 42;
use super::super::co::ids::ms::{
    CLIFF_JUMP_QUICK1 as CO_CLIFF_JUMP_QUICK1, CLIFF_JUMP_QUICK2 as CO_CLIFF_JUMP_QUICK2,
    CLIFF_JUMP_SLOW2 as CO_CLIFF_JUMP_SLOW2,
};
use crate::game::ft::FighterKind;
/// A character's tether grabs (Link's and Young Link's hookshot, Samus's grapple beam): the
/// tether, and the frames and settings its grab, dash grab and air grab throw it with.
pub struct TetherGrab {
    /// The tether.
    pub item: &'static Tether,
    /// Its item kind.
    pub kind: TetherKind,
    /// The grab's, the dash grab's and the air grab's frames in the fighter's attributes: the
    /// tether out, shot, drawn in and put away.
    pub frames: [[usize; 4]; 3],
    /// How far past the hand the stage blocks the shot, times his height.
    pub reach: f64,
    /// The beam's sparks on the grab's and the dash grab's frames 20, 23, ... (6 and 4).
    pub sparks: [i32; 2],
    /// The shot's and the draw-in's sounds (Link's and Young Link's own).
    pub sfx: Option<[i32; 2]>,
}

/// A tether's item kind.
pub enum TetherKind {
    /// The fighter's attributes' word at this offset (Link's `xBC`: his or Young Link's).
    Attr(usize),
    /// This kind.
    Item(ItKind),
}

/// The grab's tether frames (`x9C` to `xA8`), the dash grab's (`xAC` to `xB8`) and the air
/// grab's (`xBC` to `xC8`).
const FRAMES: [[usize; 4]; 3] = [
    [
        attr::CATCH_SPAWN,
        attr::CATCH_ATTACH,
        attr::CATCH_RETRACT,
        attr::CATCH_END,
    ],
    [
        attr::CATCH_DASH_SPAWN,
        attr::CATCH_DASH_ATTACH,
        attr::CATCH_DASH_RETRACT,
        attr::CATCH_DASH_END,
    ],
    [0xBC, attr::AIR_CATCH_ATTACH, 0xC4, 0xC8],
];

/// Samus's grapple beam grabs: two units of reach, the beam's sparks, no sounds.
pub static SAMUS: TetherGrab = TetherGrab {
    item: &GRAPPLE,
    kind: TetherKind::Item(ItKind::SamusGrappleBeam),
    frames: FRAMES,
    reach: 2.0,
    sparks: [6, 4],
    sfx: None,
};

impl Running<'_> {
    /// The fighter's hitboxes cleared (`ftColl_8007AFF8`), for the item code.
    pub fn fighter_clear_hitboxes(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.clear_hitboxes(f);
    }

    /// The fighter's tether grabs, if his character has them.
    pub fn tether_grab(&self, gobj: GObjId) -> Option<&'static TetherGrab> {
        match self.game.fighter(gobj).fighter_kind() {
            FighterKind::Link => Some(&super::super::link::catch::LINK),
            FighterKind::Clink => Some(&super::super::link::catch::CLINK),
            FighterKind::Samus => Some(&SAMUS),
            _ => None,
        }
    }

    /// The tether's hooks on the fighter: each frame, on death, after his animation.
    fn tether_hooks(&mut self, gobj: GObjId, t: &Tether) {
        let fp = self.game.fighter_mut(gobj);
        fp.accessory2_cb = Some(CodeRef(t.hooks[0]));
        fp.death1_cb = Some(CodeRef(t.hooks[1]));
        fp.accessory3_cb = Some(CodeRef(t.hooks[2]));
    }

    /// The tether out at his hand (`lb_8000B1CC` on the hand's joint), kept in his word.
    fn tether_out(&mut self, gobj: GObjId, g: &TetherGrab) -> Option<GObjId> {
        let t = g.item;
        let bone = tether::hand_bone(self, gobj, t);
        let hand = self.bone_joint(gobj, bone);
        let pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, hand, None);
        let facing = self.game.fighter(gobj).facing_dir;
        let kind = match g.kind {
            TetherKind::Attr(off) => {
                let f = self.game.fighter_id(gobj);
                self.char_attr_word(f, off) as i32
            }
            TetherKind::Item(k) => k.id(),
        };
        let hs = tether::spawn(self, gobj, pos, facing, kind, t);
        self.game.fighter_mut(gobj).u.set_gobj_at(t.slot, hs);
        hs
    }

    /// The beam's sparks on its frames 20, 23, ... (`count` of them), each at the tip's joint
    /// with a random spread of 4.
    fn tether_sparks(&mut self, gobj: GObjId, beam: GObjId, t: &Tether, count: i32) {
        let frame = reg(self.game.fighter(gobj).mv_f32(MV_FRAME));
        for i in 0..count {
            if frame != fp::frsp(f64::from(i * 3 + 0x14)) {
                continue;
            }
            let seg = tether::tip_gobj(self, beam, t);
            let j = tether::model_of(self, seg);
            jobj::setup_matrix(&mut self.game.objs, self.data, j);
            let m = self.game.objs.jobjs[j.expect("the tip's model")].mtx;
            let mut at = Vec3::new(m.m[0][3], m.m[1][3], m.m[2][3]);
            let spread = |r: &mut Running, v: f32| {
                let d = fp::fsubs(reg(r.game.objs.rng.draw_f32()), fp::frsp(0.5));
                mem(fp::frsp(fp::fmadd(4.0, d, reg(v))))
            };
            at.x = spread(self, at.x);
            at.y = spread(self, at.y);
            at.z = spread(self, at.z);
            ef::spawn(self, GFX_SPARK, Some(seg), &[Arg::Vec(at)]);
        }
    }

    /// Whether the stage cuts the line from him to [`TetherGrab::reach`] times his height past
    /// his hand (`mpCheckAllRemap`).
    fn tether_blocked(&mut self, gobj: GObjId, g: &TetherGrab) -> bool {
        let bone = tether::hand_bone(self, gobj, g.item);
        let hand = self.bone_joint(gobj, bone);
        jobj::setup_matrix(&mut self.game.objs, self.data, hand);
        let m = self.game.objs.jobjs[hand.expect("his hand's joint")].mtx;
        let fp = self.game.fighter(gobj);
        let reach = fp::fmul(g.reach, reg(fp.facing_dir));
        let x_reach = mem(fp::frsp(fp::fmadd(reach, reg(fp.scale.y), reg(m.m[0][3]))));
        let x_from = fp.coll_data.cur_pos.x;
        let y = m.m[1][3];
        let g = self.game.geometry.as_mut().expect("no collision geometry");
        g.check_all_remap(-1, -1, x_from, y, x_reach, y).is_some()
    }

    /// The tether's sound on its frame `i` (shot, drawn in), if his character has one.
    fn tether_sfx(&mut self, gobj: GObjId, g: &TetherGrab, i: usize) {
        if let Some(sfx) = g.sfx {
            self.play_sfx(gobj, sfx[i], 0x7F, 0x40);
        }
    }

    /// His grab's tether (`fn_800D8EC8`, `fn_800D9558`) or his dash grab's (`fn_800D9228`,
    /// `fn_800D9930`), Link's and Young Link's hookshot and Samus's grapple beam: on its frame
    /// it comes out at his hand (none, and he waits); then the beam's sparks, and on their
    /// frames it's shot (unless, for the grab, the stage is in the way, then he waits), drawn
    /// in, and put away. True when he stopped grabbing.
    #[doc(alias = "fn_800D8EC8")]
    #[doc(alias = "fn_800D9228")]
    #[doc(alias = "fn_800D9558")]
    #[doc(alias = "fn_800D9930")]
    #[doc(alias("link_catch_tether", "samus_catch_tether"))]
    pub fn catch_tether(&mut self, gobj: GObjId, dash: bool) -> bool {
        let Some(g) = self.tether_grab(gobj) else {
            return false;
        };
        let t = g.item;
        let f = self.game.fighter_id(gobj);
        let [out, shoot, draw_in, end] = g.frames[usize::from(dash)];
        let at = |r: &Running, off| fp::frsp(f64::from(r.char_attr_word(f, off) as i32));
        let fp_ = &mut self.game.fighters[f.0];
        let frame = mem(fp::frsp(fp::fadd(reg(fp_.mv_f32(MV_FRAME)), 1.0)));
        fp_.set_mv_f32(MV_FRAME, reg(frame));
        let frame = reg(frame);
        if frame == at(self, out) {
            if self.tether_out(gobj, g).is_none() {
                self.enter_wait(gobj);
                return true;
            }
            self.tether_hooks(gobj, t);
            return false;
        }
        if !(frame > at(self, out)) || !(frame <= at(self, end)) {
            return false;
        }
        let hs = self.game.fighters[f.0].u.gobj_at(t.slot);
        if let Some(b) = hs {
            self.tether_sparks(gobj, b, t, g.sparks[usize::from(dash)]);
        }
        let hs = hs.expect("a tether's frames without the tether");
        if frame == at(self, shoot) {
            if !dash && self.tether_blocked(gobj, g) {
                tether::put_away(self, hs, t);
                self.enter_wait(gobj);
                return true;
            }
            let speed = tether::shot_speed(self, hs, t);
            let facing = reg(self.game.fighters[f.0].facing_dir);
            let vel = Vec3::new(mem(fp::fmuls(reg(speed), facing)), 0.0, 0.0);
            tether::shoot(self, hs, vel, t);
            self.tether_sfx(gobj, g, 0);
        } else if frame == at(self, draw_in) {
            tether::to_retract(self, hs);
            self.tether_sfx(gobj, g, 1);
        } else if frame == at(self, end) {
            tether::put_away(self, hs, t);
        }
        false
    }

    /// His air grab's tether, the Link and Samus parts of `ftCo_AirCatch_Anim`: on its frame
    /// it comes out (none, and he falls helpless); then on their frames it's shot with his own
    /// motion (unless the stage is in the way, then it's put away and he falls helpless),
    /// drawn in, and put away. True when he stopped.
    fn air_catch_tether(&mut self, gobj: GObjId) -> bool {
        let Some(g) = self.tether_grab(gobj) else {
            return false;
        };
        let t = g.item;
        let f = self.game.fighter_id(gobj);
        let [out, shoot, draw_in, end] = g.frames[2];
        let at = |r: &Running, off| fp::frsp(f64::from(r.char_attr_word(f, off) as i32));
        let fp_ = &mut self.game.fighters[f.0];
        let frame = mem(fp::frsp(fp::fadd(reg(fp_.mv_f32(MV_FRAME)), 1.0)));
        fp_.set_mv_f32(MV_FRAME, reg(frame));
        let frame = reg(frame);
        if frame == at(self, out) {
            if self.tether_out(gobj, g).is_none() {
                self.enter_fall_special_basic(gobj);
            } else {
                self.tether_hooks(gobj, t);
            }
            return false;
        }
        if !(frame > at(self, out)) || !(frame <= at(self, end)) {
            return false;
        }
        let hs = self.game.fighters[f.0]
            .u
            .gobj_at(t.slot)
            .expect("a tether's frames without the tether");
        if frame == at(self, shoot) {
            if self.tether_blocked(gobj, g) {
                tether::put_away(self, hs, t);
                self.enter_fall_special_basic(gobj);
                return true;
            }
            let speed = tether::shot_speed(self, hs, t);
            let fp_ = &self.game.fighters[f.0];
            let x = mem(fp::fmuls(reg(speed), reg(fp_.facing_dir)));
            let x = mem(fp::fadds(reg(x), reg(fp_.pos_delta.x)));
            tether::shoot(self, hs, Vec3::new(x, 0.0, 0.0), t);
            self.tether_sfx(gobj, g, 0);
        } else if frame == at(self, draw_in) {
            tether::to_retract(self, hs);
            self.tether_sfx(gobj, g, 1);
        } else if frame == at(self, end) {
            tether::put_away(self, hs, t);
        }
        false
    }

    /// His grab ended off the ground, `fn_800D949C` (Link's and Young Link's) and
    /// `fn_800D9C64` (Samus's): the tether put away. A grab that leaves the ground before the
    /// tether comes out passes none, which the put-away ignores.
    #[doc(alias = "fn_800D949C")]
    #[doc(alias = "fn_800D9C64")]
    #[doc(alias("link_catch_off_ground", "samus_catch_off_ground"))]
    pub fn tether_off_ground(&mut self, gobj: GObjId) {
        if let Some(g) = self.tether_grab(gobj)
            && let Some(hs) = self.game.fighter(gobj).u.gobj_at(g.item.slot)
        {
            tether::put_away(self, hs, g.item);
        }
    }

    /// The tether put away and forgotten (`ftCo_800D94D8`'s and `ftCo_800D9C98`'s start).
    pub fn tether_forget(&mut self, gobj: GObjId, t: &Tether) {
        if let Some(hs) = self.game.fighter(gobj).u.gobj_at(t.slot) {
            tether::put_away(self, hs, t);
            self.game.fighter_mut(gobj).u.set_gobj_at(t.slot, None);
        }
    }

    /// The beam put away when she's hit, `ftCo_800D9C98`: and the damage hooks cleared.
    #[doc(alias = "ftCo_800D9C98")]
    pub fn samus_grapple_put_away(&mut self, gobj: GObjId) {
        self.tether_forget(gobj, &GRAPPLE);
        let fp = self.game.fighter_mut(gobj);
        fp.death2_cb = None;
        fp.take_dmg_2_cb = None;
        fp.take_dmg_cb = None;
    }

    /// Her air grab, `ftCo_800C3BE8`: in the air, the frame counter at 0, no acceleration, the
    /// air grab, and grab immunity from every kind (`ftCommon_8007E2F4`).
    #[doc(alias = "ftCo_800C3BE8")]
    pub fn enter_air_catch(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.ground_or_air != GA_AIR {
            return;
        }
        fp.set_mv_f32(MV_FRAME, 0.0);
        fp.anim_vel = Vec3::new(0.0, 0.0, 0.0);
        match fp.fighter_kind() {
            FighterKind::Samus => {
                self.change_motion_state(gobj, ms::AIR_CATCH, 0, 0.0, 1.0, 0.0, None);
            }
            _ => {
                let msid = super::super::link::ms::AIR_CATCH;
                self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
            }
        }
        self.game.fighter_mut(gobj).grab_immune = 0x1FF;
    }

    /// The air grab latched on, `ftCo_800C3CC0`: the latch state keeping a fast fall, her drift
    /// within the air's, 20 frames, and airborne from the ground.
    #[doc(alias = "ftCo_800C3CC0")]
    pub fn enter_air_catch_hit(&mut self, gobj: GObjId) {
        match self.game.fighter(gobj).fighter_kind() {
            FighterKind::Samus => {
                self.change_motion_state(
                    gobj,
                    ms::AIR_CATCH_HIT,
                    KEEP_FAST_FALL,
                    0.0,
                    1.0,
                    0.0,
                    None,
                );
            }
            _ => {
                let msid = super::super::link::ms::AIR_CATCH_HIT;
                self.change_motion_state(gobj, msid, KEEP_FAST_FALL, 0.0, 1.0, 0.0, None);
            }
        }
        let fp = self.game.fighter_mut(gobj);
        let drift = reg(fp.co_attrs.air_drift_max);
        Self::clamp_self_vel_x(fp, drift);
        fp.set_mv_i32(MV_HIT_X0, 20);
        fp.set_mv_f32(MV_HIT_X4, 0.0);
        if fp.ground_or_air == GA_GROUND {
            let f = self.game.fighter_id(gobj);
            self.go_airborne(f);
        }
    }

    /// Whether a ledge is in reach with a little more snap (5 more in height and reach) on
    /// her facing side, `ftCo_800C3A14`: then she grabs it (the ledge's line and flag) and
    /// stops.
    #[doc(alias = "ftCo_800C3A14")]
    pub fn ledge_in_reach(&mut self, gobj: GObjId) -> bool {
        let fp = self.game.fighter(gobj);
        let mut cd = fp.coll_data.clone();
        cd.ledge_snap_y = mem(fp::frsp(fp::fadd(reg(cd.ledge_snap_y), 5.0)));
        cd.ledge_snap_height = mem(fp::frsp(fp::fadd(reg(cd.ledge_snap_height), 5.0)));
        let right = f64::from(fp.facing_dir) > 0.0;
        let found = crate::game::it::maplib::with_stage_coll(self.game, self.data, |c| {
            if right {
                c.ledge_left_end(&mut cd)
            } else {
                c.ledge_right_end(&mut cd)
            }
        });
        let Some(line) = found else {
            return false;
        };
        let fp = self.game.fighter_mut(gobj);
        if right {
            fp.coll_data.ledge_id_left = line;
            fp.coll_data.env_flags |= LEFT_LEDGE_GRAB;
        } else {
            fp.coll_data.ledge_id_right = line;
            fp.coll_data.env_flags |= RIGHT_LEDGE_GRAB;
        }
        fp.self_vel.x = 0.0;
        fp.self_vel.y = 0.0;
        true
    }

    /// Whether another fighter hangs from the ledge she'd catch (`ft_80082E3C`).
    pub fn ledge_taken(&self, gobj: GObjId) -> bool {
        self.ledge_taken_by(gobj).is_some()
    }

    /// A tether's end in a ledge jump's rise, `ftCo_8009B390`: the quick or slow second part,
    /// rising at the fighter's ledge jump speed times `mul`.
    #[doc(alias = "ftCo_8009B390")]
    pub fn tether_landing(&mut self, gobj: GObjId, mul: f32) {
        let f = self.game.fighter_id(gobj);
        let mul = reg(mul);
        let quick = self.game.fighters[f.0].motion_id == CO_CLIFF_JUMP_QUICK1;
        let msid = if quick {
            CO_CLIFF_JUMP_QUICK2
        } else {
            CO_CLIFF_JUMP_SLOW2
        };
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(0, 0);
        fp.self_vel.y = mem(fp::fmuls(
            reg(fp.co_attrs.ledge_jump_vertical_velocity),
            mul,
        ));
    }

    /// `ftCo_AirCatch_Anim`: the tether's frames ([`Running::catch_tether`]'s air grab); at
    /// the animation's end he falls helpless.
    #[doc(alias = "ftCo_AirCatch_Anim")]
    pub fn co_air_catch_anim(&mut self, gobj: GObjId) {
        if self.air_catch_tether(gobj) {
            return;
        }
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall_special_basic(gobj);
        }
    }

    /// `ftCo_AirCatch_Phys`: a fast fall, or gravity (a fifth of it for the first 20 frames
    /// while falling), and drift.
    #[doc(alias = "ftCo_AirCatch_Phys")]
    pub fn co_air_catch_phys(&mut self, gobj: GObjId) {
        self.check_fall_fast(gobj);
        let fp = self.game.fighter_mut(gobj);
        let (g, t) = (reg(fp.co_attrs.gravity), reg(fp.co_attrs.terminal_velocity));
        if fp.fall_fast {
            // `ftCommon_FallFast`.
            fp.self_vel.y = mem(fp::fneg(reg(fp.co_attrs.fast_fall_velocity)));
        } else if reg(fp.mv_f32(MV_FRAME)) < fp::frsp(20.0) && f64::from(fp.pos_delta.y) < 0.0 {
            Self::fall(fp, fp::frsp(fp::fmul(0.2, g)), t);
        } else {
            Self::fall(fp, g, t);
        }
        self.air_drift(gobj);
    }

    /// `ftCo_AirCatchHit_Phys`: her motion's own speed, and gravity.
    #[doc(alias = "ftCo_AirCatchHit_Phys")]
    pub fn co_air_catch_hit_phys(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.self_vel = fp.pos_delta;
        let (g, t) = (reg(fp.co_attrs.gravity), reg(fp.co_attrs.terminal_velocity));
        Self::fall(fp, g, t);
    }

    /// `ftCo_AirCatch_Coll`: landing.
    #[doc(alias = "ftCo_AirCatch_Coll")]
    pub fn co_air_catch_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.enter_landing(gobj, MS_LANDING, 0, 0, 0.0, 1.0);
        }
    }

    /// Her roll, `ftCo_80099390`: the script's flag and the ball off, the roll, and her own
    /// animation and collision callbacks.
    #[doc(alias = "ftCo_80099390")]
    pub fn samus_enter_roll(&mut self, gobj: GObjId, msid: i32, item_window: i32) {
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[0] = 0;
        fp.set_mv_i32(MV_ESCAPE_BALL, 0);
        self.enter_escape_common(gobj, msid, item_window);
        let fp = self.game.fighter_mut(gobj);
        fp.anim_cb = Some(CodeRef(ROLL_ANIM));
        fp.coll_cb = Some(CodeRef(ROLL_COLL));
    }

    /// Her roll's animation, `ftCo_80099564`: the ball's intangibility as the script says,
    /// then the roll's.
    #[doc(alias = "ftCo_80099564")]
    pub fn samus_roll_anim(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        if fp.cmd_vars[0] != 0 && fp.mv_i32(MV_ESCAPE_BALL) == 0 {
            self.samus_ball_intangible(gobj);
            self.game.fighter_mut(gobj).set_mv_i32(MV_ESCAPE_BALL, 1);
        }
        let fp = self.game.fighter(gobj);
        if fp.cmd_vars[0] == 0 && fp.mv_i32(MV_ESCAPE_BALL) != 0 {
            self.samus_ball_tangible(gobj);
            self.game.fighter_mut(gobj).set_mv_i32(MV_ESCAPE_BALL, 0);
        }
        self.co_escape_anim(gobj);
    }

    /// Her roll's collision, `ftCo_80099754`: the ball's box while the script says, else the
    /// ground's.
    #[doc(alias = "ftCo_80099754")]
    pub fn samus_roll_coll(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).cmd_vars[0] != 0 {
            let b = self.samus_ball_box(gobj);
            self.ground_coll_box_fall(gobj, &b);
        } else {
            self.ground_coll(gobj);
        }
    }
}

#[doc(alias = "ftCo_AirCatch_IASA")]
#[doc(alias = "ftCo_AirCatchHit_Anim")]
#[doc(alias = "ftCo_AirCatchHit_IASA")]
#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        ROLL_ANIM => g.samus_roll_anim(gobj),
        ROLL_COLL => g.samus_roll_coll(gobj),
        AIR_CATCH_ANIM => g.co_air_catch_anim(gobj),
        AIR_CATCH_HIT_ANIM | AIR_CATCH_IASA | AIR_CATCH_HIT_IASA => {}
        AIR_CATCH_PHYS => g.co_air_catch_phys(gobj),
        AIR_CATCH_HIT_PHYS => g.co_air_catch_hit_phys(gobj),
        AIR_CATCH_COLL => g.co_air_catch_coll(gobj),
        _ => return false,
    }
    true
}
