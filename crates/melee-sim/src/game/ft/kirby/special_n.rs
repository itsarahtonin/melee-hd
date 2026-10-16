//! Inhale (`ftkirbyspecialn.c`): the inhale's start, loop and end on the ground and in the
//! air, catching a fighter (or an item) in its grab box and pulling it into his mouth, then
//! holding it: waiting, walking, turning, jumping and falling with it, and swallowing it (a
//! fighter's power becomes his copy, the fighter pops out in a star) or spitting it out as a
//! star. The victims' states are in `ft::co::capture_kirby`; the copies' specials are in their
//! own units, reached through the entries here by his copy's kind.

use gekko_fp as fp;

use super::super::super::code::unported_call;
use super::super::super::ef::{Arg, SpawnKind};
use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::super::data::figatree_frames;
use super::{KIND_KIRBY, addr, attr, ms, u};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::math;
use crate::vec::Vec3;

/// `FtPart_YRotN`.
const PART_Y_ROT_N: i32 = 3;

/// `ftCo_MS_HammerFall`, `_HammerJump` and `_HammerLanding`, whose animations' lengths his
/// held walks use.
const HAMMER_FALL: i32 = 311;
const HAMMER_JUMP: i32 = 312;
const HAMMER_LANDING: i32 = 313;

/// `ftCo_800BD1DC`: the victim's side of the inhale's catch (`ft::co::capture_kirby`).
pub const CAPTURE_KIRBY: u32 = 0x800B_D1DC;

/// The motion flags the inhale's state changes keep.
mod flags {
    use super::super::super::motion::flags::*;
    /// `ftKb_MF_SpecialN_Coll` (`ftCommon_GroundAirColl_MF | Ft_MF_KeepGfx`).
    pub const COLL: u32 = KEEP_GFX
        | SKIP_MAT_ANIM
        | SKIP_COL_ANIM
        | UPDATE_CMD
        | SKIP_ITEM_VIS
        | UNK_19
        | SKIP_MODEL_PART_VIS
        | SKIP_MODEL_FLAGS
        | UNK_27;
    /// `ftKb_MF_SpecialN_Capture_Coll`.
    pub const CAPTURE_COLL: u32 = COLL | SKIP_MODEL;
    /// `ftKb_MF_SpecialN_EatFall_Coll`.
    pub const EAT_FALL_COLL: u32 = (COLL & !KEEP_GFX) | SKIP_MODEL;
    /// `ftKb_MF_SpecialN_Loop_Coll`.
    pub const LOOP_COLL: u32 = CAPTURE_COLL | SKIP_HIT | KEEP_SFX;
    /// `ftKb_MF_SpecialN_LoopRumble_Coll`.
    pub const LOOP_RUMBLE_COLL: u32 = LOOP_COLL | SKIP_RUMBLE;
}

/// `ftKb_MotionVars`' words the held states use: the jump's input (`x4`) and `x0`.
const MV_X0: usize = 0x0;
const MV_JUMP_INPUT: usize = 0x4;

impl Running<'_> {
    /// His cleanups once he holds something (`ftKb_SpecialN_800F9070`): when KO'd and hit.
    #[doc(alias = "ftKb_SpecialN_800F9070")]
    pub fn kb_set_death_cbs(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.death3_cb = Some(CodeRef(addr::DEATH3));
        fp.death1_cb = Some(CodeRef(addr::DEATH1));
    }

    /// The grab's catch flags (`ftCommon_8007E2F4`).
    fn kb_set_x1a6a(&mut self, gobj: GObjId, v: u16) {
        self.game.fighter_mut(gobj).x1A6A = v;
    }

    /// The inhale's catch callbacks (`ftCommon_8007E2D0` with mask 16), and `x2225_b1`.
    fn kb_n_set_catch(&mut self, gobj: GObjId, grab: u32, item: u32) {
        let fp = self.game.fighter_mut(gobj);
        fp.x221E_b6 = true;
        fp.x1A68 = 0x10;
        fp.grab_cb = Some(CodeRef(grab));
        fp.grabbed_cb = Some(CodeRef(CAPTURE_KIRBY));
        fp.x2194 = Some(CodeRef(item));
        fp.x2225_b1 = true;
    }

    /// The velocity a spat fighter's star leaves at, `ftKb_SpecialN_800F58AC`: ahead of him
    /// along the ground; the star's slowing.
    #[doc(alias = "ftKb_SpecialN_800F58AC")]
    pub fn kb_spit_vel(&self, gobj: GObjId, facing: f32) -> (Vec3, f32) {
        let f = self.game.fighter_id(gobj);
        let v = reg(self.kb_attr_f32(f, attr::N_SPIT_VEL));
        let x = mem(fp::fmuls(fp::fneg(reg(facing)), v));
        (
            Vec3::new(x, 0.0, 0.0),
            self.kb_attr_f32(f, attr::N_SPIT_DECEL),
        )
    }

    /// The velocity a swallowed fighter's star pops out at, `ftKb_SpecialN_800F58D8`: up at
    /// the attributes' angle; its gravity.
    #[doc(alias = "ftKb_SpecialN_800F58D8")]
    pub fn kb_swallow_vel(&self, gobj: GObjId, facing: f32) -> (Vec3, f32) {
        let f = self.game.fighter_id(gobj);
        let angle = self.kb_attr_f32(f, attr::N_SWALLOW_ANGLE);
        let speed = reg(self.kb_attr_f32(f, attr::N_SWALLOW_SPEED));
        let c = reg(math::cos(angle));
        let t = fp::fmuls(speed, c);
        let x = mem(fp::fmuls(reg(facing), t));
        let y = mem(fp::fmuls(speed, reg(math::sin(angle))));
        (
            Vec3::new(x, y, 0.0),
            self.kb_attr_f32(f, attr::N_SWALLOW_GRAVITY),
        )
    }

    /// Dropping through the platform he stands on, `ftKb_SpecialN_800F5954`.
    #[doc(alias = "ftKb_SpecialN_800F5954")]
    pub fn kb_skip_floor(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).coll_data.update_floor_skip();
    }

    /// Whether he stands on a platform, `ftKb_SpecialN_800F597C`.
    #[doc(alias = "ftKb_SpecialN_800F597C")]
    pub fn kb_on_platform(&self, gobj: GObjId) -> bool {
        self.game.fighter(gobj).coll_data.floor.flags & 0x100 != 0
    }

    /// The victim's mash hopping him up (`dir` 1) or down (-1) on the ground,
    /// `ftKb_SpecialN_800F598C`: into the held fall.
    #[doc(alias = "ftKb_SpecialN_800F598C")]
    pub fn kb_victim_hop(&mut self, gobj: GObjId, dir: i32) {
        let f = self.game.fighter_id(gobj);
        let stop = reg(self.kb_attr_f32(f, attr::N_STOP_MOMENTUM));
        if self.game.fighters[f.0].ground_or_air == 0 {
            let d = fp::frsp(f64::from(dir));
            self.kb_n_eat_wait_to_air(gobj);
            self.game.fighters[f.0].self_vel.y = mem(fp::fmuls(stop, d));
        }
    }

    /// The victim's mash pushing him its way, `ftKb_SpecialN_800F5A04`.
    #[doc(alias = "ftKb_SpecialN_800F5A04")]
    pub fn kb_victim_push(&mut self, gobj: GObjId, dir: f32) {
        let f = self.game.fighter_id(gobj);
        let stop = reg(self.kb_attr_f32(f, attr::N_STOP_MOMENTUM));
        let fp = &mut self.game.fighters[f.0];
        let v = mem(fp::fmuls(reg(dir), stop));
        if fp.ground_or_air == 0 {
            fp.gr_vel = v;
        } else {
            fp.self_vel.x = v;
        }
    }

    /// Whether his victim can move him (only while he waits or falls with it),
    /// `ftKb_SpecialN_800F5A38`.
    #[doc(alias = "ftKb_SpecialN_800F5A38")]
    pub fn kb_victim_can_move(&self, gobj: GObjId) -> bool {
        matches!(
            self.game.fighter(gobj).motion_id,
            ms::EAT_WAIT | ms::EAT_FALL
        )
    }

    /// His victim's facing when it breaks out (his opposite), `ftKb_SpecialN_800F5A60`.
    #[doc(alias = "ftKb_SpecialN_800F5A60")]
    pub fn kb_victim_facing(&self, gobj: GObjId) -> f32 {
        mem(fp::fneg(reg(self.game.fighter(gobj).facing_dir)))
    }

    /// A float of Kirby's attributes in his data (`gFtDataList[Ft_Kind_Kirby]->ext_attr`).
    fn kb_ext_attr_f32(&self, off: usize) -> f32 {
        let d =
            self.game.ft.data.data_list[KIND_KIRBY as usize].expect("Kirby's data without Kirby");
        let ext = self.data.ptr(d.at(4)).expect("ftData::ext_attr");
        self.data.f32(ext.at(off as u32))
    }

    /// The thrown victim's shrink (`ftKb_SpecialN_800F5A70`).
    #[doc(alias = "ftKb_SpecialN_800F5A70")]
    pub fn kb_thrown_shrink(&self) -> f32 {
        self.kb_ext_attr_f32(attr::N_SHRINK)
    }

    /// A spat star's time (`ftKb_SpecialN_800F5A88`).
    #[doc(alias = "ftKb_SpecialN_800F5A88")]
    pub fn kb_spit_star_time(&self, gobj: GObjId) -> f32 {
        let f = self.game.fighter_id(gobj);
        self.kb_attr_f32(f, attr::N_STAR_TIME)
    }

    /// The thrown victim's spin (`ftKb_SpecialN_800F5A98`).
    #[doc(alias = "ftKb_SpecialN_800F5A98")]
    pub fn kb_thrown_spin(&self) -> f32 {
        self.kb_ext_attr_f32(attr::N_SPIT_SPIN)
    }

    /// A swallowed fighter's star's time (`ftKb_SpecialN_800F5AB0`).
    #[doc(alias = "ftKb_SpecialN_800F5AB0")]
    pub fn kb_swallow_star_time(&self, gobj: GObjId) -> f32 {
        let f = self.game.fighter_id(gobj);
        self.kb_attr_f32(f, attr::N_SWALLOW_STAR_TIME)
    }

    /// The stars' time's divisor (`ftKb_SpecialN_800F5AC0`).
    #[doc(alias = "ftKb_SpecialN_800F5AC0")]
    pub fn kb_star_divisor(&self) -> f32 {
        self.kb_ext_attr_f32(attr::N_STAR_DIVISOR)
    }

    /// The stars' slowing per mash (`ftKb_SpecialN_800F5AD8`).
    #[doc(alias = "ftKb_SpecialN_800F5AD8")]
    pub fn kb_star_decel(&self) -> f32 {
        self.kb_ext_attr_f32(attr::N_STAR_DECEL)
    }

    /// How long he holds a fighter (`ftKb_SpecialN_800F5AF0`).
    #[doc(alias = "ftKb_SpecialN_800F5AF0")]
    pub fn kb_hold_time(&self, gobj: GObjId) -> f32 {
        let f = self.game.fighter_id(gobj);
        self.kb_attr_f32(f, attr::N_HOLD_TIME)
    }

    /// The hold's time's divisor (`ftKb_SpecialN_800F5B00`).
    #[doc(alias = "ftKb_SpecialN_800F5B00")]
    pub fn kb_hold_divisor(&self, gobj: GObjId) -> f32 {
        let f = self.game.fighter_id(gobj);
        self.kb_attr_f32(f, attr::N_HOLD_DIVISOR)
    }

    /// The hold's resistance to mashing (`ftKb_SpecialN_800F5B10`).
    #[doc(alias = "ftKb_SpecialN_800F5B10")]
    pub fn kb_hold_resistance(&self, gobj: GObjId) -> f32 {
        let f = self.game.fighter_id(gobj);
        self.kb_attr_f32(f, attr::N_HOLD_RESISTANCE)
    }

    /// The pull's speeds, outer and inner (`ftKb_SpecialN_800F5B20`).
    #[doc(alias = "ftKb_SpecialN_800F5B20")]
    pub fn kb_pull_speeds(&self, gobj: GObjId) -> (f32, f32) {
        let f = self.game.fighter_id(gobj);
        (
            self.kb_attr_f32(f, attr::N_PULL_OUTER),
            self.kb_attr_f32(f, attr::N_PULL_INNER),
        )
    }

    /// The pulled victim's shrink (`ftKb_SpecialN_800F5B3C`).
    #[doc(alias = "ftKb_SpecialN_800F5B3C")]
    pub fn kb_pull_shrink(&self, gobj: GObjId) -> f32 {
        let f = self.game.fighter_id(gobj);
        self.kb_attr_f32(f, attr::N_SHRINK)
    }

    /// The range the pulled victim shrinks within (`ftKb_SpecialN_800F5B4C`).
    #[doc(alias = "ftKb_SpecialN_800F5B4C")]
    pub fn kb_pull_range(&self, gobj: GObjId) -> f32 {
        let f = self.game.fighter_id(gobj);
        self.kb_attr_f32(f, attr::N_PULL_RANGE)
    }

    /// His mouth, where the pull ends, `ftKb_SpecialN_800F5B5C`: offset ahead and up from
    /// his position.
    #[doc(alias = "ftKb_SpecialN_800F5B5C")]
    pub fn kb_mouth(&self, gobj: GObjId) -> Vec3 {
        let f = self.game.fighter_id(gobj);
        let ox = reg(self.kb_attr_f32(f, attr::N_MOUTH_X));
        let oy = reg(self.kb_attr_f32(f, attr::N_MOUTH_Y));
        let fp = &self.game.fighters[f.0];
        let mut v = fp.cur_pos;
        v.x = mem(fp::fmadds(ox, reg(fp.facing_dir), reg(v.x)));
        v.y = mem(fp::fadds(reg(v.y), oy));
        v
    }

    /// An item pulled in close enough becomes his, `ftKb_SpecialN_800F5DE8`.
    // Not ported: `ftKb_SpecialN_800F5DE8` (no replay at hand has items).
    pub fn kb_pull_item(&mut self, _gobj: GObjId) {
        unported_call("ftKb_SpecialN_800F5DE8 (an item inhaled)");
    }

    /// A fighter pulled in close enough is swallowed into his mouth, `ftKb_SpecialN_800F5EA8`:
    /// its held state, and his.
    #[doc(alias = "ftKb_SpecialN_800F5EA8")]
    pub fn kb_pull_fighter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let v = reg(self.kb_attr_f32(f, attr::N_INHALE_VEL));
        let mouth = self.kb_mouth(gobj);
        let victim = self.game.fighters[f.0]
            .victim_gobj
            .expect("pulling no fighter");
        let d2 = self.capture_kirby_dist2(victim, mouth);
        if d2 < fp::fmuls(v, v) {
            self.enter_capture_wait_kirby(victim);
            if self.game.fighters[f.0].ground_or_air == 1 {
                self.kb_n_eat_air(gobj);
            } else {
                self.kb_n_eat(gobj);
            }
        }
    }

    /// The inhale's start's shared part.
    fn kb_n_start(&mut self, gobj: GObjId, msid: i32, grab: u32, item: u32) {
        let f = self.game.fighter_id(gobj);
        let unk = self.kb_attr_i16(f, attr::JUMP_UNK);
        let fp = &mut self.game.fighters[f.0];
        fp.x2210 = [0; 4];
        fp.cmd_vars[0] = 0;
        fp.cmd_vars[1] = 0;
        fp.set_u_i32(u::N_XE0, 4);
        fp.u[u::N_XE4..u::N_XE4 + 2].copy_from_slice(&unk.to_be_bytes());
        for (msid, off) in [
            (HAMMER_FALL, u::N_XE8),
            (HAMMER_JUMP, u::N_XEC),
            (HAMMER_LANDING, u::N_XF0),
        ] {
            let tree = self.second_figatree(f, msid);
            let frames = figatree_frames(self.data, tree);
            self.game.fighters[f.0].set_u_f32(off, frames);
        }
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.game.fighters[f.0].x2222_b2 = true;
        self.anim_update(gobj);
        self.kb_n_set_catch(gobj, grab, item);
    }

    /// Inhale on the ground, `ftKb_SpecialN_800F5F68` (without a copy).
    #[doc(alias = "ftKb_SpecialN_800F5F68")]
    pub fn kb_n_enter(&mut self, gobj: GObjId) {
        self.kb_n_start(gobj, ms::SPECIAL_N, addr::N_CATCH, addr::N_CATCH_ITEM);
    }

    /// In the air, `ftKb_SpecialN_800F6070`.
    #[doc(alias = "ftKb_SpecialN_800F6070")]
    pub fn kb_n_air_enter(&mut self, gobj: GObjId) {
        self.kb_n_start(
            gobj,
            ms::SPECIAL_AIR_N,
            addr::N_AIR_CATCH,
            addr::N_AIR_CATCH_ITEM,
        );
    }

    /// An item caught on the ground, `fn_800F6178`.
    // Not ported: `fn_800F6178`.
    pub fn kb_n_catch_item(&mut self, _gobj: GObjId) {
        unported_call("fn_800F6178 (an item caught by the inhale)");
    }

    /// A fighter caught on the ground, `fn_800F6210`: pulling it in.
    #[doc(alias = "fn_800F6210")]
    pub fn kb_n_catch(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::SPECIAL_N_CAPTURE0, 0x212, 0.0, 1.0, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        fp.x2222_b2 = true;
        fp.u[u::N_XF4] &= 0x7F;
        self.kb_set_x1a6a(gobj, 0x1FF);
    }

    /// An item caught in the air, `fn_800F6280`.
    // Not ported: `fn_800F6280`.
    pub fn kb_n_air_catch_item(&mut self, _gobj: GObjId) {
        unported_call("fn_800F6280 (an item caught by the inhale)");
    }

    /// A fighter caught in the air, `fn_800F6318`.
    #[doc(alias = "fn_800F6318")]
    pub fn kb_n_air_catch(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::SPECIAL_AIR_N_CAPTURE0, 0x212, 0.0, 1.0, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        fp.x2222_b2 = true;
        fp.u[u::N_XF4] &= 0x7F;
        self.kb_set_x1a6a(gobj, 0x1FF);
    }

    /// Into a held state (`enterCaptureState`).
    fn kb_n_enter_capture(&mut self, gobj: GObjId, msid: i32) {
        self.change_motion_state(gobj, msid, 0x10, 0.0, 1.0, 0.0, None);
        self.kb_set_death_cbs(gobj);
        self.kb_set_x1a6a(gobj, 0x1FF);
    }

    /// The victim in his mouth on the ground, `ftKb_SpecialN_800F6388`.
    #[doc(alias = "ftKb_SpecialN_800F6388")]
    pub fn kb_n_eat(&mut self, gobj: GObjId) {
        self.kb_n_enter_capture(gobj, ms::EAT);
    }

    /// In the air, `ftKb_SpecialN_800F63EC`.
    #[doc(alias = "ftKb_SpecialN_800F63EC")]
    pub fn kb_n_eat_air(&mut self, gobj: GObjId) {
        self.kb_n_enter_capture(gobj, ms::EAT_AIR);
    }

    /// The held fall, off the ground, `fn_800F6450`.
    #[doc(alias = "fn_800F6450")]
    pub fn kb_n_eat_fall(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.go_airborne(f);
        self.change_motion_state(gobj, ms::EAT_FALL, 0x90, 0.0, 1.0, 0.0, None);
        self.kb_set_death_cbs(gobj);
        self.anim_update(gobj);
        self.kb_set_x1a6a(gobj, 0x1FF);
    }

    /// The held walk at the speed the ground speed calls for, `fn_800F64C8`.
    #[doc(alias = "fn_800F64C8")]
    pub fn kb_n_eat_walk(&mut self, gobj: GObjId, anim_start: f32) {
        self.kb_n_walk(gobj, 0x10, anim_start);
    }

    fn kb_n_walk(&mut self, gobj: GObjId, flags: u32, anim_start: f32) {
        let f = self.game.fighter_id(gobj);
        let speed = self.kb_attr_f32(f, attr::N_WALK_SPEED);
        let fp = &self.game.fighters[f.0];
        let frames = [fp.u_f32(u::N_XE8), fp.u_f32(u::N_XEC), fp.u_f32(u::N_XF0)];
        let rates = [
            fp.co_attrs.slow_walk_max,
            fp.co_attrs.mid_walk_point,
            fp.co_attrs.fast_walk_min,
        ];
        self.enter_walk_common(
            gobj,
            ms::EAT_WALK_SLOW,
            flags,
            anim_start,
            frames,
            rates,
            speed,
        );
        self.kb_set_x1a6a(gobj, 0x1FF);
    }

    /// The held landing, `fn_800F6528`.
    #[doc(alias = "fn_800F6528")]
    pub fn kb_n_eat_land(&mut self, gobj: GObjId) {
        self.land(gobj);
        self.change_motion_state(gobj, ms::EAT_LANDING, 0x12, 0.0, 1.0, 0.0, None);
        self.kb_set_death_cbs(gobj);
        self.anim_update(gobj);
    }

    /// Off a ledge in the inhale's start, `fn_800F6588`.
    #[doc(alias = "fn_800F6588")]
    pub fn kb_n_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_N, flags::LOOP_COLL);
        self.set_effect_hitlag_cbs(gobj);
        self.kb_n_set_catch(gobj, addr::N_AIR_CATCH, addr::N_AIR_CATCH_ITEM);
    }

    /// Landing in the inhale's start, `fn_800F6638`.
    #[doc(alias = "fn_800F6638")]
    pub fn kb_n_to_ground(&mut self, gobj: GObjId) {
        self.land(gobj);
        let frame = self.game.fighter(gobj).cur_anim_frame;
        self.change_motion_state(gobj, ms::SPECIAL_N, 0x0C4C_529A, frame, 1.0, 0.0, None);
        self.set_effect_hitlag_cbs(gobj);
        self.kb_n_set_catch(gobj, addr::N_CATCH, addr::N_CATCH_ITEM);
    }

    /// Off a ledge in the loop, `fn_800F66E8`.
    #[doc(alias = "fn_800F66E8")]
    pub fn kb_n_loop_to_air(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.go_airborne(f);
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.change_motion_state(
            gobj,
            ms::SPECIAL_AIR_N_LOOP,
            0x0C4C_5A9A,
            frame,
            1.0,
            0.0,
            None,
        );
        self.set_effect_hitlag_cbs(gobj);
        self.kb_n_set_catch(gobj, addr::N_AIR_CATCH, addr::N_AIR_CATCH_ITEM);
    }

    /// Landing in the loop, `fn_800F6798`.
    #[doc(alias = "fn_800F6798")]
    pub fn kb_n_loop_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_N_LOOP, flags::LOOP_RUMBLE_COLL);
        self.set_effect_hitlag_cbs(gobj);
        self.kb_n_set_catch(gobj, addr::N_CATCH, addr::N_CATCH_ITEM);
    }

    /// Off a ledge in the end, `fn_800F6848`.
    #[doc(alias = "fn_800F6848")]
    pub fn kb_n_end_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_N_END, flags::COLL);
    }

    /// Landing in the end, `fn_800F68A8`.
    #[doc(alias = "fn_800F68A8")]
    pub fn kb_n_end_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_N_END, flags::COLL);
    }

    /// A ground switch to `msid` from the same frame, his cleanups after (`death`) and the
    /// catch flags.
    fn kb_n_switch_air(&mut self, gobj: GObjId, msid: i32, flags: u32, death: bool, x1a6a: bool) {
        self.fox_ground_to_air(gobj, msid, flags);
        if death {
            self.kb_set_death_cbs(gobj);
        }
        if x1a6a {
            self.kb_set_x1a6a(gobj, 0x1FF);
        }
    }

    fn kb_n_switch_ground(
        &mut self,
        gobj: GObjId,
        msid: i32,
        flags: u32,
        death: bool,
        x1a6a: bool,
    ) {
        self.fox_air_to_ground(gobj, msid, flags);
        if death {
            self.kb_set_death_cbs(gobj);
        }
        if x1a6a {
            self.kb_set_x1a6a(gobj, 0x1FF);
        }
    }

    /// The item pull off a ledge, `fn_800F6908`.
    #[doc(alias = "fn_800F6908")]
    pub fn kb_n_capture_item_to_air(&mut self, gobj: GObjId) {
        self.kb_n_switch_air(
            gobj,
            ms::SPECIAL_AIR_N_CAPTURE1,
            flags::CAPTURE_COLL,
            true,
            true,
        );
    }

    /// The fighter pull off a ledge, `fn_800F697C`.
    #[doc(alias = "fn_800F697C")]
    pub fn kb_n_capture_to_air(&mut self, gobj: GObjId) {
        self.kb_n_switch_air(
            gobj,
            ms::SPECIAL_AIR_N_CAPTURE0,
            flags::CAPTURE_COLL,
            false,
            true,
        );
    }

    /// The item pull landing, `fn_800F69E8`.
    #[doc(alias = "fn_800F69E8")]
    pub fn kb_n_capture_item_to_ground(&mut self, gobj: GObjId) {
        self.kb_n_switch_ground(
            gobj,
            ms::SPECIAL_N_CAPTURE1,
            flags::CAPTURE_COLL,
            true,
            true,
        );
    }

    /// The fighter pull landing, `fn_800F6A5C`.
    #[doc(alias = "fn_800F6A5C")]
    pub fn kb_n_capture_to_ground(&mut self, gobj: GObjId) {
        self.kb_n_switch_ground(
            gobj,
            ms::SPECIAL_N_CAPTURE0,
            flags::CAPTURE_COLL,
            false,
            true,
        );
    }

    /// The swallow into his mouth off a ledge, `fn_800F6AC8`.
    #[doc(alias = "fn_800F6AC8")]
    pub fn kb_n_eat_to_air(&mut self, gobj: GObjId) {
        self.kb_n_switch_air(gobj, ms::EAT_AIR, flags::CAPTURE_COLL, true, true);
    }

    /// Landing in it, `fn_800F6B3C`.
    #[doc(alias = "fn_800F6B3C")]
    pub fn kb_n_eat_to_ground(&mut self, gobj: GObjId) {
        self.kb_n_switch_ground(gobj, ms::EAT, flags::CAPTURE_COLL, true, true);
    }

    /// The held wait off a ledge into the held fall, `ftKb_SpecialS_800F6BB0`.
    #[doc(alias = "ftKb_SpecialS_800F6BB0")]
    pub fn kb_n_eat_wait_to_air(&mut self, gobj: GObjId) {
        self.kb_n_switch_air(gobj, ms::EAT_FALL, flags::EAT_FALL_COLL, true, true);
    }

    /// The item spit off a ledge, `fn_800F6C24`.
    #[doc(alias = "fn_800F6C24")]
    pub fn kb_n_spit_item_to_air(&mut self, gobj: GObjId) {
        self.kb_n_switch_air(
            gobj,
            ms::SPECIAL_AIR_N_SPIT1,
            flags::CAPTURE_COLL,
            true,
            true,
        );
    }

    /// The fighter spit off a ledge, `fn_800F6C98`.
    #[doc(alias = "fn_800F6C98")]
    pub fn kb_n_spit_to_air(&mut self, gobj: GObjId) {
        self.kb_n_switch_air(
            gobj,
            ms::SPECIAL_AIR_N_SPIT0,
            flags::CAPTURE_COLL,
            true,
            true,
        );
    }

    /// The item spit landing, `fn_800F6D0C`.
    #[doc(alias = "fn_800F6D0C")]
    pub fn kb_n_spit_item_to_ground(&mut self, gobj: GObjId) {
        self.kb_n_switch_ground(gobj, ms::SPECIAL_N_SPIT1, flags::CAPTURE_COLL, true, true);
    }

    /// The fighter spit landing, `fn_800F6D80`.
    #[doc(alias = "fn_800F6D80")]
    pub fn kb_n_spit_to_ground(&mut self, gobj: GObjId) {
        self.kb_n_switch_ground(gobj, ms::SPECIAL_N_SPIT0, flags::CAPTURE_COLL, true, true);
    }

    /// The item swallow off a ledge, `fn_800F6DF4`.
    #[doc(alias = "fn_800F6DF4")]
    pub fn kb_n_drink_item_to_air(&mut self, gobj: GObjId) {
        self.kb_n_switch_air(gobj, ms::SPECIAL_AIR_N_DRINK1, flags::COLL, true, true);
    }

    /// The fighter swallow off a ledge, `fn_800F6E68`.
    #[doc(alias = "fn_800F6E68")]
    pub fn kb_n_drink_to_air(&mut self, gobj: GObjId) {
        self.kb_n_switch_air(gobj, ms::SPECIAL_AIR_N_DRINK0, flags::COLL, true, true);
    }

    /// The item swallow landing, `fn_800F6EDC`.
    #[doc(alias = "fn_800F6EDC")]
    pub fn kb_n_drink_item_to_ground(&mut self, gobj: GObjId) {
        self.kb_n_switch_ground(gobj, ms::SPECIAL_N_DRINK1, flags::COLL, true, true);
    }

    /// The fighter swallow landing, `fn_800F6F50`.
    #[doc(alias = "fn_800F6F50")]
    pub fn kb_n_drink_to_ground(&mut self, gobj: GObjId) {
        self.kb_n_switch_ground(gobj, ms::SPECIAL_N_DRINK0, flags::COLL, true, true);
    }

    /// The held turn off a ledge, `fn_800F6FC4` (the catch flags untouched).
    #[doc(alias = "fn_800F6FC4")]
    pub fn kb_n_eat_turn_to_air(&mut self, gobj: GObjId) {
        self.kb_n_switch_air(gobj, ms::EAT_TURN_AIR, flags::CAPTURE_COLL, true, false);
    }

    /// The held turn landing, `fn_800F702C`.
    #[doc(alias = "fn_800F702C")]
    pub fn kb_n_eat_turn_to_ground(&mut self, gobj: GObjId) {
        self.kb_n_switch_ground(gobj, ms::EAT_TURN, flags::CAPTURE_COLL, true, true);
    }

    /// The inhale's start: its effect when the script says, then the loop with its catch.
    fn kb_n_start_anim(&mut self, gobj: GObjId, loop_msid: i32, grab: u32, item: u32) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[0] != 0 {
            let j = self.bone_joint(gobj, PART_Y_ROT_N);
            let facing = self.game.fighters[f.0].facing_dir;
            self.fighter_effect(
                gobj,
                SpawnKind::AttachParam,
                0x49A,
                j,
                &[Arg::Float(facing)],
            );
            self.game.fighters[f.0].x2219_b0 = true;
            self.set_effect_hitlag_cbs(gobj);
            self.game.fighters[f.0].cmd_vars[0] = 0;
        }
        if !self.anim_frames_remaining(gobj) {
            self.change_motion_state(gobj, loop_msid, 0x212, 0.0, 1.0, 0.0, None);
            self.game.fighters[f.0].x2222_b2 = true;
            self.set_effect_hitlag_cbs(gobj);
            self.kb_n_set_catch(gobj, grab, item);
        }
    }

    /// `ftKb_SpecialN_Anim`.
    #[doc(alias = "ftKb_SpecialN_Anim")]
    pub fn kb_special_n_anim(&mut self, gobj: GObjId) {
        self.kb_n_start_anim(gobj, ms::SPECIAL_N_LOOP, addr::N_CATCH, addr::N_CATCH_ITEM);
    }

    /// `ftKb_SpecialAirN_Anim`.
    #[doc(alias = "ftKb_SpecialAirN_Anim")]
    pub fn kb_special_air_n_anim(&mut self, gobj: GObjId) {
        self.kb_n_start_anim(
            gobj,
            ms::SPECIAL_AIR_N_LOOP,
            addr::N_AIR_CATCH,
            addr::N_AIR_CATCH_ITEM,
        );
    }

    /// The states' empty callbacks.
    #[doc(alias = "ftKb_SpecialNLoop_Anim")]
    #[doc(alias = "ftKb_SpecialAirNLoop_Anim")]
    #[doc(alias = "ftKb_EatWait_Anim")]
    #[doc(alias = "ftKb_SpecialAirNCaptureWait_Anim")]
    #[doc(alias = "ftKb_EatJump2_Anim")]
    pub fn kb_special_n_nothing(&mut self, _gobj: GObjId) {}

    /// Waiting once the state ends: `ftKb_SpecialNEnd_Anim`.
    #[doc(alias = "ftKb_SpecialNEnd_Anim")]
    pub fn kb_special_n_end_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// Falling once the state ends: `ftKb_SpecialAirNEnd_Anim`.
    #[doc(alias = "ftKb_SpecialAirNEnd_Anim")]
    pub fn kb_special_air_n_end_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// The item pull's animation (`ftKb_SpecialNCapture0_Anim`, `ftKb_SpecialNCapture1_Anim`).
    // Not ported: `ftKb_SpecialNCapture0_Anim`, `ftKb_SpecialNCapture1_Anim` (an item
    // pulled in).
    pub fn kb_n_capture_item_anim(&mut self, gobj: GObjId) {
        self.kb_pull_item(gobj);
    }

    /// The fighter pull's animation (`ftKb_SpecialNCapture_Anim`,
    /// `ftKb_SpecialAirNCapture_Anim`).
    #[doc(alias = "ftKb_SpecialNCapture_Anim")]
    #[doc(alias = "ftKb_SpecialAirNCapture_Anim")]
    pub fn kb_n_capture_anim(&mut self, gobj: GObjId) {
        self.kb_pull_fighter(gobj);
    }

    /// Into the held wait once the swallow ends: `ftKb_Eat_Anim`, `ftKb_EatLanding_Anim`.
    #[doc(alias = "ftKb_Eat_Anim")]
    #[doc(alias = "ftKb_EatLanding_Anim")]
    pub fn kb_eat_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.change_motion_state(gobj, ms::EAT_WAIT, 0x10, 0.0, 1.0, 0.0, None);
            self.kb_set_death_cbs(gobj);
            self.anim_update(gobj);
            self.kb_set_x1a6a(gobj, 0x1FF);
        }
    }

    /// Into the held fall once the aerial swallow ends, `ftKb_SpecialAirNCaptured_Anim`.
    #[doc(alias = "ftKb_SpecialAirNCaptured_Anim")]
    pub fn kb_eat_air_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.kb_n_eat_fall(gobj);
        }
    }

    /// An item spat out when the script says (`ftKb_SpecialNSpit0_Anim`,
    /// `ftKb_SpecialNSpit1_Anim`), then waiting (`air` falling).
    #[doc(alias = "ftKb_SpecialNSpit0_Anim")]
    #[doc(alias = "ftKb_SpecialNSpit1_Anim")]
    pub fn kb_n_spit_item_anim(&mut self, gobj: GObjId, air: bool) {
        let fp = self.game.fighter(gobj);
        if fp.cmd_vars[0] != 0 && fp.target_item_gobj.is_some() {
            unported_call("it_802F28C8 (an inhaled item spat out)");
        }
        self.kb_n_end_or_fall(gobj, air);
    }

    fn kb_n_end_or_fall(&mut self, gobj: GObjId, air: bool) {
        if !self.anim_frames_remaining(gobj) {
            if air {
                self.enter_fall(gobj);
            } else {
                self.enter_wait(gobj);
            }
        }
    }

    /// The fighter spat out when the script says (`ftKb_SpecialNSpit_Anim`,
    /// `ftKb_SpecialAirNSpit_Anim`): let go with the throw's hit, as a star; then waiting
    /// (`air` falling).
    #[doc(alias = "ftKb_SpecialNSpit_Anim")]
    #[doc(alias = "ftKb_SpecialAirNSpit_Anim")]
    pub fn kb_n_spit_anim(&mut self, gobj: GObjId, air: bool) {
        let fp = self.game.fighter(gobj);
        if fp.cmd_vars[0] != 0
            && let Some(victim) = fp.victim_gobj
        {
            self.kb_set_x1a6a(gobj, 0);
            self.throw_hit(gobj, victim, false);
            self.enter_thrown_kirby_star(victim, gobj);
            let v = self.game.fighter_id(victim);
            self.thrown_hitbox_owner(v, gobj);
            self.game.fighter_mut(gobj).cmd_vars[0] = 0;
        }
        self.kb_n_end_or_fall(gobj, air);
    }

    /// An item swallowed when the script says (`ftKb_SpecialNDrink0_Anim`,
    /// `ftKb_SpecialNDrink1_Anim`), then waiting (`air` falling).
    #[doc(alias = "ftKb_SpecialNDrink0_Anim")]
    #[doc(alias = "ftKb_SpecialNDrink1_Anim")]
    pub fn kb_n_drink_item_anim(&mut self, gobj: GObjId, air: bool) {
        let fp = self.game.fighter(gobj);
        if fp.cmd_vars[0] != 0 && fp.target_item_gobj.is_some() {
            unported_call("it_802F28C8 (an inhaled item swallowed)");
        }
        self.kb_n_end_or_fall(gobj, air);
    }

    /// The fighter swallowed when the script says (`ftKb_SpecialNDrink_Anim`,
    /// `ftKb_SpecialAirNDrink_Anim`): let go with the throw's hit, out in a star, and its
    /// power his copy; then waiting (`air` falling).
    #[doc(alias = "ftKb_SpecialNDrink_Anim")]
    #[doc(alias = "ftKb_SpecialAirNDrink_Anim")]
    pub fn kb_n_drink_anim(&mut self, gobj: GObjId, air: bool) {
        let fp = self.game.fighter(gobj);
        if fp.cmd_vars[0] != 0
            && let Some(victim) = fp.victim_gobj
        {
            self.kb_set_x1a6a(gobj, 0);
            self.throw_hit(gobj, victim, false);
            self.enter_thrown_copy_star(victim, gobj);
            let kind = self.copy_kind_of(gobj, victim);
            self.game.fighter_mut(gobj).set_u_i32(u::N_XE0, kind);
            self.kb_copy(gobj, kind, true);
            self.game.fighter_mut(gobj).cmd_vars[0] = 0;
        }
        self.kb_n_end_or_fall(gobj, air);
    }

    /// Turning around with the victim held (`ftKb_EatTurn_Anim`): at its end, the other way,
    /// waiting.
    #[doc(alias = "ftKb_EatTurn_Anim")]
    pub fn kb_eat_turn_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            let fp = self.game.fighter_mut(gobj);
            fp.facing_dir = mem(fp::fneg(reg(fp.facing_dir)));
            self.change_motion_state(gobj, ms::EAT_WAIT, 0x10, 0.0, 1.0, 0.0, None);
            self.kb_set_death_cbs(gobj);
            self.anim_update(gobj);
            self.kb_set_x1a6a(gobj, 0x1FF);
        }
    }

    /// In the air (`ftKb_SpecialAirNCaptureTurn_Anim`): at its end, the other way, falling.
    #[doc(alias = "ftKb_SpecialAirNCaptureTurn_Anim")]
    pub fn kb_eat_turn_air_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            let fp = self.game.fighter_mut(gobj);
            fp.facing_dir = mem(fp::fneg(reg(fp.facing_dir)));
            self.kb_n_eat_fall(gobj);
        }
    }

    /// `ftKb_EatWalk_Anim` (`ftWalkCommon_800DFDDC`).
    // Ports `ftKb_EatWalk_Anim`; aliased once a check reaches it (no replay at hand walks
    // with a fighter held).
    pub fn kb_eat_walk_anim(&mut self, gobj: GObjId) {
        self.co_walk_anim(gobj);
    }

    /// The held jump's knee bend, `ftKb_EatJump1_Anim`: at its end, off the ground at the
    /// attributes' height.
    #[doc(alias = "ftKb_EatJump1_Anim")]
    pub fn kb_eat_jump1_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            let f = self.game.fighter_id(gobj);
            let height = self.kb_attr_f32(f, attr::N_JUMP_HEIGHT);
            self.go_airborne(f);
            self.change_motion_state(gobj, ms::EAT_JUMP2, 0x92, 0.0, 1.0, 0.0, None);
            self.kb_set_death_cbs(gobj);
            self.anim_update(gobj);
            self.jump_take_off(gobj, true, height);
        }
    }

    /// The loop's release, after the attributes' shortest frames (`u.xE4`): the end once B is
    /// let go (`ftKb_SpecialNLoop_IASA`, `ftKb_SpecialAirNLoop_IASA`).
    fn kb_n_loop_iasa(&mut self, gobj: GObjId, end: i32) {
        let fp = self.game.fighter_mut(gobj);
        let left = i16::from_be_bytes([fp.u[u::N_XE4], fp.u[u::N_XE4 + 1]]);
        if left != 0 {
            fp.u[u::N_XE4..u::N_XE4 + 2].copy_from_slice(&(left - 1).to_be_bytes());
            return;
        }
        if fp.input.held_buttons[0] & 0x200 == 0 {
            self.change_motion_state(gobj, end, 0, 0.0, 1.0, 0.0, None);
        }
    }

    /// `ftKb_SpecialNLoop_IASA`.
    #[doc(alias = "ftKb_SpecialNLoop_IASA")]
    pub fn kb_special_n_loop_iasa(&mut self, gobj: GObjId) {
        self.kb_n_loop_iasa(gobj, ms::SPECIAL_N_END);
    }

    /// `ftKb_SpecialAirNLoop_IASA`.
    #[doc(alias = "ftKb_SpecialAirNLoop_IASA")]
    pub fn kb_special_air_n_loop_iasa(&mut self, gobj: GObjId) {
        self.kb_n_loop_iasa(gobj, ms::SPECIAL_AIR_N_END);
    }

    /// Into `msid` with what he holds.
    fn kb_n_held_action(&mut self, gobj: GObjId, msid: i32, flags: u32, x2222: bool) {
        self.change_motion_state(gobj, msid, flags, 0.0, 1.0, 0.0, None);
        if x2222 {
            self.game.fighter_mut(gobj).x2222_b2 = true;
        }
        self.kb_set_death_cbs(gobj);
        self.anim_update(gobj);
        self.kb_set_x1a6a(gobj, 0x1FF);
    }

    /// The held wait's choices: swallowing (B, or the stick down past the attributes' range)
    /// or spitting (A) what he holds, `swallow`/`spit` its states for an item or a fighter.
    fn kb_n_eat_or_spit(&mut self, gobj: GObjId, item: [i32; 2], fighter: [i32; 2]) -> bool {
        let f = self.game.fighter_id(gobj);
        let range = reg(self.kb_attr_f32(f, attr::N_JUMP_RANGE_Y));
        let fp = &self.game.fighters[f.0];
        let is_item = fp.u[u::N_XF4] & 0x80 != 0;
        let held = if is_item {
            fp.target_item_gobj.is_some()
        } else {
            fp.victim_gobj.is_some()
        };
        let (swallow, spit) = if is_item {
            (item[0], item[1])
        } else {
            (fighter[0], fighter[1])
        };
        let b = fp.input.pressed_buttons & 0x200 != 0;
        let down = reg(fp.input.lstick[0].y) < fp::fneg(range);
        if (b && held) || (down && held) {
            self.kb_n_held_action(gobj, swallow, super::super::motion::flags::KEEP_GFX, true);
            return true;
        }
        let fp = &self.game.fighters[f.0];
        if fp.input.pressed_buttons & 0x100 != 0 && held {
            self.kb_n_held_action(gobj, spit, 0x12, true);
            return true;
        }
        false
    }

    /// The held wait's turn: the stick back past the attributes' range turns him around.
    fn kb_n_turn_check(&mut self, gobj: GObjId, msid: i32) -> bool {
        let f = self.game.fighter_id(gobj);
        let range = reg(self.kb_attr_f32(f, attr::N_WALK_RANGE_X));
        let fp = &self.game.fighters[f.0];
        let mut x = reg(fp.input.lstick[0].x);
        let mag = if x < 0.0 { fp::fneg(x) } else { x };
        if mag < range {
            x = 0.0;
        }
        let facing = reg(fp.facing_dir);
        if (x < 0.0 && facing == 1.0) || (x > 0.0 && facing == -1.0) {
            self.kb_n_held_action(gobj, msid, 0x92, false);
            return true;
        }
        false
    }

    /// `ftKb_EatWait_IASA`: swallowing, spitting, turning, jumping or walking with what he
    /// holds.
    #[doc(alias = "ftKb_EatWait_IASA")]
    pub fn kb_eat_wait_iasa(&mut self, gobj: GObjId) {
        if self.kb_n_eat_or_spit(
            gobj,
            [ms::SPECIAL_N_DRINK1, ms::SPECIAL_N_SPIT1],
            [ms::SPECIAL_N_DRINK0, ms::SPECIAL_N_SPIT0],
        ) {
            return;
        }
        if self.kb_n_turn_check(gobj, ms::EAT_TURN) {
            return;
        }
        let jump = self.jump_input(gobj);
        if jump != 0 {
            let fp = self.game.fighter_mut(gobj);
            fp.set_mv_i32(MV_JUMP_INPUT, jump);
            fp.set_mv_i32(MV_X0, 0);
            self.kb_n_held_action(gobj, ms::EAT_JUMP1, 0x92, false);
            return;
        }
        if self.walk_input(gobj) {
            self.kb_n_walk(gobj, 0x10, 0.0);
        }
    }

    /// `ftKb_SpecialAirNCaptureWait_IASA`: swallowing, spitting or turning in the held fall.
    #[doc(alias = "ftKb_SpecialAirNCaptureWait_IASA")]
    pub fn kb_eat_fall_iasa(&mut self, gobj: GObjId) {
        if self.kb_n_eat_or_spit(
            gobj,
            [ms::SPECIAL_AIR_N_DRINK1, ms::SPECIAL_AIR_N_SPIT1],
            [ms::SPECIAL_AIR_N_DRINK0, ms::SPECIAL_AIR_N_SPIT0],
        ) {
            return;
        }
        self.kb_n_turn_check(gobj, ms::EAT_TURN_AIR);
    }

    /// `ftKb_EatWalk_IASA`: a jump, the wait when the stick lets go, else the walk's speed.
    #[doc(alias = "ftKb_EatWalk_IASA")]
    pub fn kb_eat_walk_iasa(&mut self, gobj: GObjId) {
        let jump = self.jump_input(gobj);
        if jump != 0 {
            let fp = self.game.fighter_mut(gobj);
            fp.set_mv_i32(MV_JUMP_INPUT, jump);
            fp.set_mv_i32(MV_X0, 0);
            use super::super::motion::flags::*;
            self.kb_n_held_action(
                gobj,
                ms::EAT_JUMP1,
                KEEP_GFX | SKIP_MODEL | SKIP_MAT_ANIM,
                false,
            );
            return;
        }
        if self.stick_not_forward(gobj) {
            self.kb_n_held_action(gobj, ms::EAT_WAIT, 0x10, false);
            return;
        }
        self.walk_speed_switch(gobj, |g, gobj, start| g.kb_n_eat_walk(gobj, start));
    }

    /// `ftKb_EatJump1_IASA` (`ftCo_KneeBend_Check_ShortHop`).
    #[doc(alias = "ftKb_EatJump1_IASA")]
    pub fn kb_eat_jump1_iasa(&mut self, gobj: GObjId) {
        self.knee_bend_short_hop_check(gobj);
    }

    /// Friction on the ground (`ft_80084F3C`).
    #[doc(alias = "ftKb_SpecialN_Phys")]
    #[doc(alias = "ftKb_SpecialNLoop_Phys")]
    #[doc(alias = "ftKb_SpecialNEnd_Phys")]
    #[doc(alias = "ftKb_SpecialNCapture0_Phys")]
    #[doc(alias = "ftKb_SpecialNCapture_Phys")]
    #[doc(alias = "ftKb_Eat_Phys")]
    #[doc(alias = "ftKb_EatWait_Phys")]
    #[doc(alias = "ftKb_SpecialNSpit0_Phys")]
    #[doc(alias = "ftKb_SpecialNSpit_Phys")]
    #[doc(alias = "ftKb_SpecialNDrink0_Phys")]
    #[doc(alias = "ftKb_SpecialNDrink_Phys")]
    #[doc(alias = "ftKb_EatTurn_Phys")]
    #[doc(alias = "ftKb_EatJump1_Phys")]
    #[doc(alias = "ftKb_EatLanding_Phys")]
    pub fn kb_n_ground_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// Air physics without drift (`ft_80084EEC`).
    #[doc(alias = "ftKb_SpecialAirN_Phys")]
    #[doc(alias = "ftKb_SpecialAirNLoop_Phys")]
    #[doc(alias = "ftKb_SpecialAirNEnd_Phys")]
    #[doc(alias = "ftKb_SpecialNCapture1_Phys")]
    #[doc(alias = "ftKb_SpecialAirNCapture_Phys")]
    #[doc(alias = "ftKb_SpecialAirNCaptured_Phys")]
    #[doc(alias = "ftKb_SpecialAirNCaptureWait_Phys")]
    #[doc(alias = "ftKb_SpecialNSpit1_Phys")]
    #[doc(alias = "ftKb_SpecialAirNSpit_Phys")]
    #[doc(alias = "ftKb_SpecialNDrink1_Phys")]
    #[doc(alias = "ftKb_SpecialAirNDrink_Phys")]
    #[doc(alias = "ftKb_SpecialAirNCaptureTurn_Phys")]
    pub fn kb_n_air_phys(&mut self, gobj: GObjId) {
        self.air_physics_no_drift(gobj);
    }

    /// `ftKb_EatWalk_Phys` (`ftWalkCommon_800E0060`).
    #[doc(alias = "ftKb_EatWalk_Phys")]
    pub fn kb_eat_walk_phys(&mut self, gobj: GObjId) {
        self.co_walk_phys(gobj);
    }

    /// `ftKb_EatJump2_Phys` (`ftCo_Jump_Phys_Inner`).
    #[doc(alias = "ftKb_EatJump2_Phys")]
    pub fn kb_eat_jump2_phys(&mut self, gobj: GObjId) {
        self.co_cargo_jump_phys(gobj);
    }

    /// The inhale states' collisions: the ground's fall off a ledge into the matching
    /// aerial state, the air's landing into the grounded one.
    #[doc(alias = "ftKb_SpecialN_Coll")]
    #[doc(alias = "ftKb_SpecialAirN_Coll")]
    #[doc(alias = "ftKb_SpecialNLoop_Coll")]
    #[doc(alias = "ftKb_SpecialAirNLoop_Coll")]
    #[doc(alias = "ftKb_SpecialNEnd_Coll")]
    #[doc(alias = "ftKb_SpecialAirNEnd_Coll")]
    #[doc(alias = "ftKb_SpecialNCapture0_Coll")]
    #[doc(alias = "ftKb_SpecialNCapture_Coll")]
    #[doc(alias = "ftKb_SpecialNCapture1_Coll")]
    #[doc(alias = "ftKb_SpecialAirNCapture_Coll")]
    #[doc(alias = "ftKb_Eat_Coll")]
    #[doc(alias = "ftKb_SpecialAirNCaptured_Coll")]
    #[doc(alias = "ftKb_EatWait_Coll")]
    #[doc(alias = "ftKb_SpecialAirNCaptureWait_Coll")]
    #[doc(alias = "ftKb_SpecialNSpit0_Coll")]
    #[doc(alias = "ftKb_SpecialNSpit_Coll")]
    #[doc(alias = "ftKb_SpecialNSpit1_Coll")]
    #[doc(alias = "ftKb_SpecialAirNSpit_Coll")]
    #[doc(alias = "ftKb_SpecialNDrink0_Coll")]
    #[doc(alias = "ftKb_SpecialNDrink_Coll")]
    #[doc(alias = "ftKb_SpecialNDrink1_Coll")]
    #[doc(alias = "ftKb_SpecialAirNDrink_Coll")]
    #[doc(alias = "ftKb_EatTurn_Coll")]
    #[doc(alias = "ftKb_SpecialAirNCaptureTurn_Coll")]
    #[doc(alias = "ftKb_EatWalk_Coll")]
    #[doc(alias = "ftKb_EatJump1_Coll")]
    #[doc(alias = "ftKb_EatJump2_Coll")]
    #[doc(alias = "ftKb_EatLanding_Coll")]
    pub fn kb_n_coll(&mut self, gobj: GObjId, a: u32) -> bool {
        match a {
            addr::SPECIAL_N_COLL => self.ground_coll_fall_off_then(gobj, |g, x| g.kb_n_to_air(x)),
            addr::SPECIAL_AIR_N_COLL => self.air_coll_then(gobj, |g, x| g.kb_n_to_ground(x)),
            addr::SPECIAL_N_LOOP_COLL => {
                self.ground_coll_fall_off_then(gobj, |g, x| g.kb_n_loop_to_air(x))
            }
            addr::SPECIAL_AIR_N_LOOP_COLL => {
                self.air_coll_then(gobj, |g, x| g.kb_n_loop_to_ground(x))
            }
            addr::SPECIAL_N_END_COLL => {
                self.ground_coll_fall_off_then(gobj, |g, x| g.kb_n_end_to_air(x))
            }
            addr::SPECIAL_AIR_N_END_COLL => {
                self.air_coll_then(gobj, |g, x| g.kb_n_end_to_ground(x))
            }
            addr::SPECIAL_N_CAPTURE0_COLL => {
                self.ground_coll_fall_off_then(gobj, |g, x| g.kb_n_capture_item_to_air(x))
            }
            addr::SPECIAL_N_CAPTURE_COLL => {
                self.ground_coll_fall_off_then(gobj, |g, x| g.kb_n_capture_to_air(x))
            }
            addr::SPECIAL_N_CAPTURE1_COLL => {
                self.air_coll_then(gobj, |g, x| g.kb_n_capture_item_to_ground(x))
            }
            addr::SPECIAL_AIR_N_CAPTURE_COLL => {
                self.air_coll_then(gobj, |g, x| g.kb_n_capture_to_ground(x))
            }
            addr::EAT_COLL => self.ground_coll_fall_off_then(gobj, |g, x| g.kb_n_eat_to_air(x)),
            addr::SPECIAL_AIR_N_CAPTURED_COLL => {
                self.air_coll_then(gobj, |g, x| g.kb_n_eat_to_ground(x))
            }
            addr::EAT_WAIT_COLL => {
                self.ground_coll_fall_off_then(gobj, |g, x| g.kb_n_eat_wait_to_air(x))
            }
            addr::SPECIAL_AIR_N_CAPTURE_WAIT_COLL | addr::EAT_JUMP2_COLL => {
                self.air_coll_then(gobj, |g, x| g.kb_n_eat_land(x))
            }
            addr::SPECIAL_N_SPIT0_COLL => {
                self.ground_coll_fall_off_then(gobj, |g, x| g.kb_n_spit_item_to_air(x))
            }
            addr::SPECIAL_N_SPIT_COLL => {
                self.ground_coll_fall_off_then(gobj, |g, x| g.kb_n_spit_to_air(x))
            }
            addr::SPECIAL_N_SPIT1_COLL => {
                self.air_coll_then(gobj, |g, x| g.kb_n_spit_item_to_ground(x))
            }
            addr::SPECIAL_AIR_N_SPIT_COLL => {
                self.air_coll_then(gobj, |g, x| g.kb_n_spit_to_ground(x))
            }
            addr::SPECIAL_N_DRINK0_COLL => {
                self.ground_coll_fall_off_then(gobj, |g, x| g.kb_n_drink_item_to_air(x))
            }
            addr::SPECIAL_N_DRINK_COLL => {
                self.ground_coll_fall_off_then(gobj, |g, x| g.kb_n_drink_to_air(x))
            }
            addr::SPECIAL_N_DRINK1_COLL => {
                self.air_coll_then(gobj, |g, x| g.kb_n_drink_item_to_ground(x))
            }
            addr::SPECIAL_AIR_N_DRINK_COLL => {
                self.air_coll_then(gobj, |g, x| g.kb_n_drink_to_ground(x))
            }
            addr::EAT_TURN_COLL => {
                self.ground_coll_fall_off_then(gobj, |g, x| g.kb_n_eat_turn_to_air(x))
            }
            addr::SPECIAL_AIR_N_CAPTURE_TURN_COLL => {
                self.air_coll_then(gobj, |g, x| g.kb_n_eat_turn_to_ground(x))
            }
            addr::EAT_WALK_COLL | addr::EAT_JUMP1_COLL | addr::EAT_LANDING_COLL => {
                self.ground_coll_fall_off_then(gobj, |g, x| g.kb_n_eat_fall(x))
            }
            _ => return false,
        }
        true
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::N_SKIP_FLOOR => g.kb_skip_floor(gobj),
        addr::N_PULL_ITEM => g.kb_pull_item(gobj),
        addr::N_PULL_FIGHTER => g.kb_pull_fighter(gobj),
        addr::N_ENTER => g.kb_n_enter(gobj),
        addr::N_AIR_ENTER => g.kb_n_air_enter(gobj),
        addr::N_CATCH_ITEM => g.kb_n_catch_item(gobj),
        addr::N_CATCH => g.kb_n_catch(gobj),
        addr::N_AIR_CATCH_ITEM => g.kb_n_air_catch_item(gobj),
        addr::N_AIR_CATCH => g.kb_n_air_catch(gobj),
        addr::N_EAT => g.kb_n_eat(gobj),
        addr::N_EAT_AIR => g.kb_n_eat_air(gobj),
        addr::N_EAT_FALL => g.kb_n_eat_fall(gobj),
        addr::N_EAT_LAND => g.kb_n_eat_land(gobj),
        addr::N_TO_AIR => g.kb_n_to_air(gobj),
        addr::N_TO_GROUND => g.kb_n_to_ground(gobj),
        addr::N_LOOP_TO_AIR => g.kb_n_loop_to_air(gobj),
        addr::N_LOOP_TO_GROUND => g.kb_n_loop_to_ground(gobj),
        addr::N_END_TO_AIR => g.kb_n_end_to_air(gobj),
        addr::N_END_TO_GROUND => g.kb_n_end_to_ground(gobj),
        addr::N_CAPTURE_ITEM_TO_AIR => g.kb_n_capture_item_to_air(gobj),
        addr::N_CAPTURE_TO_AIR => g.kb_n_capture_to_air(gobj),
        addr::N_CAPTURE_ITEM_TO_GROUND => g.kb_n_capture_item_to_ground(gobj),
        addr::N_CAPTURE_TO_GROUND => g.kb_n_capture_to_ground(gobj),
        addr::N_EAT_TO_AIR => g.kb_n_eat_to_air(gobj),
        addr::N_EAT_TO_GROUND => g.kb_n_eat_to_ground(gobj),
        addr::N_EAT_WAIT_TO_AIR => g.kb_n_eat_wait_to_air(gobj),
        addr::N_SPIT_ITEM_TO_AIR => g.kb_n_spit_item_to_air(gobj),
        addr::N_SPIT_TO_AIR => g.kb_n_spit_to_air(gobj),
        addr::N_SPIT_ITEM_TO_GROUND => g.kb_n_spit_item_to_ground(gobj),
        addr::N_SPIT_TO_GROUND => g.kb_n_spit_to_ground(gobj),
        addr::N_DRINK_ITEM_TO_AIR => g.kb_n_drink_item_to_air(gobj),
        addr::N_DRINK_TO_AIR => g.kb_n_drink_to_air(gobj),
        addr::N_DRINK_ITEM_TO_GROUND => g.kb_n_drink_item_to_ground(gobj),
        addr::N_DRINK_TO_GROUND => g.kb_n_drink_to_ground(gobj),
        addr::N_EAT_TURN_TO_AIR => g.kb_n_eat_turn_to_air(gobj),
        addr::N_EAT_TURN_TO_GROUND => g.kb_n_eat_turn_to_ground(gobj),
        addr::SPECIAL_N_ANIM => g.kb_special_n_anim(gobj),
        addr::SPECIAL_AIR_N_ANIM => g.kb_special_air_n_anim(gobj),
        addr::SPECIAL_N_LOOP_ANIM
        | addr::SPECIAL_AIR_N_LOOP_ANIM
        | addr::EAT_WAIT_ANIM
        | addr::SPECIAL_AIR_N_CAPTURE_WAIT_ANIM
        | addr::EAT_JUMP2_ANIM => g.kb_special_n_nothing(gobj),
        addr::SPECIAL_N_END_ANIM => g.kb_special_n_end_anim(gobj),
        addr::SPECIAL_AIR_N_END_ANIM => g.kb_special_air_n_end_anim(gobj),
        addr::SPECIAL_N_CAPTURE0_ANIM | addr::SPECIAL_N_CAPTURE1_ANIM => {
            g.kb_n_capture_item_anim(gobj)
        }
        addr::SPECIAL_N_CAPTURE_ANIM | addr::SPECIAL_AIR_N_CAPTURE_ANIM => {
            g.kb_n_capture_anim(gobj)
        }
        addr::EAT_ANIM | addr::EAT_LANDING_ANIM => g.kb_eat_anim(gobj),
        addr::SPECIAL_AIR_N_CAPTURED_ANIM => g.kb_eat_air_anim(gobj),
        addr::SPECIAL_N_SPIT0_ANIM => g.kb_n_spit_item_anim(gobj, false),
        addr::SPECIAL_N_SPIT1_ANIM => g.kb_n_spit_item_anim(gobj, true),
        addr::SPECIAL_N_SPIT_ANIM => g.kb_n_spit_anim(gobj, false),
        addr::SPECIAL_AIR_N_SPIT_ANIM => g.kb_n_spit_anim(gobj, true),
        addr::SPECIAL_N_DRINK0_ANIM => g.kb_n_drink_item_anim(gobj, false),
        addr::SPECIAL_N_DRINK1_ANIM => g.kb_n_drink_item_anim(gobj, true),
        addr::SPECIAL_N_DRINK_ANIM => g.kb_n_drink_anim(gobj, false),
        addr::SPECIAL_AIR_N_DRINK_ANIM => g.kb_n_drink_anim(gobj, true),
        addr::EAT_TURN_ANIM => g.kb_eat_turn_anim(gobj),
        addr::SPECIAL_AIR_N_CAPTURE_TURN_ANIM => g.kb_eat_turn_air_anim(gobj),
        addr::EAT_WALK_ANIM => g.kb_eat_walk_anim(gobj),
        addr::EAT_JUMP1_ANIM => g.kb_eat_jump1_anim(gobj),
        addr::SPECIAL_N_LOOP_IASA => g.kb_special_n_loop_iasa(gobj),
        addr::SPECIAL_AIR_N_LOOP_IASA => g.kb_special_air_n_loop_iasa(gobj),
        addr::EAT_WAIT_IASA => g.kb_eat_wait_iasa(gobj),
        addr::SPECIAL_AIR_N_CAPTURE_WAIT_IASA => g.kb_eat_fall_iasa(gobj),
        addr::EAT_WALK_IASA => g.kb_eat_walk_iasa(gobj),
        addr::EAT_JUMP1_IASA => g.kb_eat_jump1_iasa(gobj),
        addr::SPECIAL_N_PHYS
        | addr::SPECIAL_N_LOOP_PHYS
        | addr::SPECIAL_N_END_PHYS
        | addr::SPECIAL_N_CAPTURE0_PHYS
        | addr::SPECIAL_N_CAPTURE_PHYS
        | addr::EAT_PHYS
        | addr::EAT_WAIT_PHYS
        | addr::SPECIAL_N_SPIT0_PHYS
        | addr::SPECIAL_N_SPIT_PHYS
        | addr::SPECIAL_N_DRINK0_PHYS
        | addr::SPECIAL_N_DRINK_PHYS
        | addr::EAT_TURN_PHYS
        | addr::EAT_JUMP1_PHYS
        | addr::EAT_LANDING_PHYS => g.kb_n_ground_phys(gobj),
        addr::SPECIAL_AIR_N_PHYS
        | addr::SPECIAL_AIR_N_LOOP_PHYS
        | addr::SPECIAL_AIR_N_END_PHYS
        | addr::SPECIAL_N_CAPTURE1_PHYS
        | addr::SPECIAL_AIR_N_CAPTURE_PHYS
        | addr::SPECIAL_AIR_N_CAPTURED_PHYS
        | addr::SPECIAL_AIR_N_CAPTURE_WAIT_PHYS
        | addr::SPECIAL_N_SPIT1_PHYS
        | addr::SPECIAL_AIR_N_SPIT_PHYS
        | addr::SPECIAL_N_DRINK1_PHYS
        | addr::SPECIAL_AIR_N_DRINK_PHYS
        | addr::SPECIAL_AIR_N_CAPTURE_TURN_PHYS => g.kb_n_air_phys(gobj),
        addr::EAT_WALK_PHYS => g.kb_eat_walk_phys(gobj),
        addr::EAT_JUMP2_PHYS => g.kb_eat_jump2_phys(gobj),
        a if g.kb_n_coll(gobj, a) => {}
        addr::SET_DEATH_CBS => g.kb_set_death_cbs(gobj),
        addr::RELEASE_ITEM => g.kb_release_item(gobj),
        _ => return false,
    }
    true
}
