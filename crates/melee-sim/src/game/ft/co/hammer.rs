//! The hammer's states (`ftCo_HammerWait.c`, `ftCo_HammerWalk.c`, `ftCo_HammerTurn.c`,
//! `ftCo_HammerKneeBend.c`, `ftCo_HammerJump.c`, `ftCo_HammerFall.c`, `ftCo_HammerLanding.c`,
//! `ftCo_0C60.c`): the fighter swings the hammer (`it::kinds::hammer`) on every loop of its
//! animation, can walk, turn, jump and drop through platforms but do nothing else, and lets it
//! go when its time runs out; a hammer that loses its head swings headless.

use gekko_fp as fp;

use super::super::super::it::{self, item_mut, kinds, lib2725};
use super::super::super::state::Running;
use super::super::motion::flags::*;
use super::ids::ms;
use super::{PAD_A, PAD_B, PAD_LR};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::lb::hitcapsule;

/// The hammer's color animation.
const HAMMER_COLANIM: i32 = 106;
/// `ftCommonData`'s hammer fields: its frames held (x6AC), the turn's length (x6B0), the
/// landing's (x6B4) and the drop's window (x6B8).
const HELD_FRAMES: u32 = 0x6AC;
const TURN_FRAMES: u32 = 0x6B0;
const LANDING_FRAMES: u32 = 0x6B4;
const DROP_WINDOW: u32 = 0x6B8;
/// `mv`'s turn frames left (`walk.fast_anim_frame`), the walk's speed and acceleration
/// (`walk.x0`, `walk.accel_mul`), the knee bend's (`hammerkneebend`) and the landing's frames
/// left (`hammerlanding.x4`).
const MV_TURN_LEFT: usize = 0x10;
const MV_WALK_X0: usize = 0x0;
const MV_WALK_ACCEL_MUL: usize = 0x20;
const MV_KNEE_SHORT_HOP: usize = 0x0;
const MV_KNEE_INPUT: usize = 0x4;
const MV_KNEE_LEFT: usize = 0x8;
const MV_LANDING_LEFT: usize = 0x4;

/// The hammer states' callbacks, by address.
pub mod addr {
    pub const WAIT_ANIM: u32 = 0x800C_4FEC;
    pub const WAIT_IASA: u32 = 0x800C_5074;
    pub const WAIT_PHYS: u32 = 0x800C_50D4;
    pub const WAIT_COLL: u32 = 0x800C_50F4;
    pub const WALK_ANIM: u32 = 0x800C_56BC;
    pub const WALK_IASA: u32 = 0x800C_56DC;
    pub const WALK_PHYS: u32 = 0x800C_572C;
    pub const WALK_COLL: u32 = 0x800C_574C;
    pub const TURN_ANIM: u32 = 0x800C_5848;
    pub const TURN_IASA: u32 = 0x800C_59EC;
    pub const TURN_PHYS: u32 = 0x800C_5A10;
    pub const TURN_COLL: u32 = 0x800C_5A30;
    pub const JUMP_ANIM: u32 = 0x800C_5B24;
    pub const JUMP_IASA: u32 = 0x800C_5B44;
    pub const JUMP_PHYS: u32 = 0x800C_5B48;
    pub const JUMP_COLL: u32 = 0x800C_5B68;
    pub const KNEE_BEND_ANIM: u32 = 0x800C_5C10;
    pub const KNEE_BEND_IASA: u32 = 0x800C_5C74;
    pub const KNEE_BEND_PHYS: u32 = 0x800C_5C94;
    pub const KNEE_BEND_COLL: u32 = 0x800C_5CB4;
    pub const FALL_ANIM: u32 = 0x800C_5DBC;
    pub const FALL_IASA: u32 = 0x800C_5E94;
    pub const FALL_PHYS: u32 = 0x800C_5F40;
    pub const FALL_COLL: u32 = 0x800C_5F60;
    pub const LANDING_ANIM: u32 = 0x800C_6020;
    pub const LANDING_IASA: u32 = 0x800C_6084;
    pub const LANDING_PHYS: u32 = 0x800C_6088;
    pub const LANDING_COLL: u32 = 0x800C_60A8;
}

/// Runs one of the hammer states' callbacks, if `cb` is one.
pub fn run(g: &mut Running, gobj: GObjId, cb: u32) -> bool {
    use addr::*;
    match cb {
        WAIT_ANIM | WALK_ANIM | JUMP_ANIM | FALL_ANIM => g.hammer_swing(gobj),
        WAIT_IASA => g.co_hammer_wait_iasa(gobj),
        WAIT_PHYS | TURN_PHYS | KNEE_BEND_PHYS | LANDING_PHYS => g.co_hammer_ground_phys(gobj),
        WAIT_COLL | WALK_COLL | TURN_COLL | KNEE_BEND_COLL | LANDING_COLL => {
            g.co_hammer_ground_coll(gobj);
        }
        WALK_IASA => g.co_hammer_walk_iasa(gobj),
        WALK_PHYS => g.co_walk_phys(gobj),
        TURN_ANIM => g.co_hammer_turn_anim(gobj),
        TURN_IASA => g.co_hammer_turn_iasa(gobj),
        JUMP_IASA | LANDING_IASA => {}
        JUMP_PHYS => g.co_cargo_jump_phys(gobj),
        JUMP_COLL | FALL_COLL => g.co_hammer_air_coll(gobj),
        KNEE_BEND_ANIM => g.co_hammer_knee_bend_anim(gobj),
        KNEE_BEND_IASA => g.knee_bend_short_hop_check(gobj),
        FALL_IASA => g.co_hammer_fall_iasa(gobj),
        FALL_PHYS => g.air_physics(gobj),
        LANDING_ANIM => g.co_hammer_landing_anim(gobj),
        _ => return false,
    }
    true
}

impl Running<'_> {
    /// Whether the fighter is in one of the hammer's states, `ftCo_800C53E4`.
    #[doc(alias = "ftCo_800C53E4")]
    pub fn in_hammer_state(&self, gobj: GObjId) -> bool {
        (ms::HAMMER_WAIT..=ms::HAMMER_LANDING).contains(&self.game.fighter(gobj).motion_id)
    }

    /// The hammer state's animation picks up where the last one's was, `ftCo_800C548C`.
    #[doc(alias = "ftCo_800C548C")]
    pub fn hammer_anim_start(&self, gobj: GObjId) -> f32 {
        if self.in_hammer_state(gobj) {
            self.game.fighter(gobj).cur_anim_frame
        } else {
            0.0
        }
    }

    /// From one hammer state to another the fighter keeps its hits, model, materials, rumble
    /// and color animation, `ftCo_800C54C4`.
    #[doc(alias = "ftCo_800C54C4")]
    pub fn hammer_flags(&self, gobj: GObjId) -> u32 {
        if self.in_hammer_state(gobj) {
            SKIP_HIT
                | SKIP_MODEL
                | SKIP_MAT_ANIM
                | SKIP_RUMBLE
                | SKIP_COL_ANIM
                | UPDATE_CMD
                | UNK_19
        } else {
            0
        }
    }

    /// Into hammer state `msid` with its animation picked up, `Fighter_ChangeMotionState` as
    /// each state's entry calls it.
    fn change_hammer_state(&mut self, gobj: GObjId, msid: i32) {
        let flags = self.hammer_flags(gobj);
        let start = self.hammer_anim_start(gobj);
        self.change_motion_state(gobj, msid, flags, start, 1.0, 0.0, None);
    }

    /// The hammer's color animation unless it's on, `ftCo_800C5408`.
    #[doc(alias = "ftCo_800C5408")]
    pub fn hammer_colanim_on(&mut self, gobj: GObjId) {
        if self.color_anim_id(gobj) != HAMMER_COLANIM {
            self.start_color_anim(gobj, HAMMER_COLANIM, 0);
        }
    }

    /// The hammer's color animation off, `ftCo_800C544C`.
    #[doc(alias = "ftCo_800C544C")]
    pub fn hammer_colanim_off(&mut self, gobj: GObjId) {
        if self.color_anim_id(gobj) == HAMMER_COLANIM {
            self.end_color_anim(gobj, HAMMER_COLANIM);
        }
    }

    /// Every hammer state's entry ends with `ftCo_800C4E94`: the color animation, the hammer's
    /// hitboxes held on, and a rumble.
    #[doc(alias = "ftCo_800C4E94")]
    pub fn hammer_state_entered(&mut self, gobj: GObjId) {
        self.hammer_colanim_on(gobj);
        self.hammer_hits_held(gobj);
        let f = self.game.fighter_id(gobj);
        self.start_rumble(f, 1, 0);
    }

    /// The hammer's hitboxes on (with its head), held through the fighter's state changes
    /// (`x2219_b4`), `ftCo_800C5588`.
    #[doc(alias = "ftCo_800C5588")]
    pub fn hammer_hits_held(&mut self, gobj: GObjId) {
        let it = self.game.fighter(gobj).item_gobj.expect("no hammer held");
        hammer_hits_on(self, it, true);
        self.game.fighter_mut(gobj).x2219_b4 = true;
    }

    /// The hammer's hitboxes off as a state change takes the hits, `ftCo_800C555C`.
    #[doc(alias = "ftCo_800C555C")]
    pub fn hammer_hits_off(&mut self, gobj: GObjId) {
        let it = self.game.fighter(gobj).item_gobj.expect("no hammer held");
        hammer_hits_on(self, it, false);
    }

    /// Standing with the hammer, `ftCo_800C4ED8`.
    #[doc(alias = "ftCo_800C4ED8")]
    pub fn enter_hammer_wait(&mut self, gobj: GObjId) {
        self.change_hammer_state(gobj, ms::HAMMER_WAIT);
        self.hammer_state_entered(gobj);
    }

    /// Standing again once the stick isn't held forward, `ftCo_800C4E4C`.
    #[doc(alias = "ftCo_800C4E4C")]
    pub fn hammer_wait_check(&mut self, gobj: GObjId) -> bool {
        if self.stick_not_forward(gobj) {
            self.enter_hammer_wait(gobj);
            return true;
        }
        false
    }

    /// The swing, `ftCo_800C4F64` (and the states' animations, `ftCo_HammerWait_Anim`): a
    /// headless hammer's hitboxes stay off; else each loop of the animation starts a new
    /// attack, which the hammer takes from the fighter.
    #[doc(alias = "ftCo_800C4F64")]
    #[doc(alias = "ftCo_HammerWait_Anim")]
    #[doc(alias = "ftCo_HammerWalk_Anim")]
    #[doc(alias = "ftCo_HammerJump_Anim")]
    #[doc(alias = "ftCo_HammerFall_Anim")]
    pub fn hammer_swing(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        if fp.x2338.x != 0 {
            self.clear_hitboxes(f);
        } else if fp.cur_anim_frame >= 0.0 && fp.cur_anim_frame < fp.frame_speed_mul {
            self.renew_attack_instance(gobj);
            self.renew_attack_record(gobj);
            let it = self.game.fighters[f.0].item_gobj.expect("no hammer held");
            it::lib279c::copy_fighter_attack(self.game, it, gobj);
        }
    }

    /// `ftCo_HammerWait_IASA`: a jump, a platform drop, a turn or a walk.
    #[doc(alias = "ftCo_HammerWait_IASA")]
    pub fn co_hammer_wait_iasa(&mut self, gobj: GObjId) {
        if self.hammer_jump_check(gobj)
            || self.hammer_pass_check(gobj)
            || self.hammer_turn_check(gobj)
        {
            return;
        }
        self.hammer_walk_check(gobj);
    }

    /// Ground friction, `ftCo_HammerWait_Phys` (and the turn's, the knee bend's and the
    /// landing's).
    #[doc(alias = "ftCo_HammerWait_Phys")]
    #[doc(alias = "ftCo_HammerTurn_Phys")]
    #[doc(alias = "ftCo_HammerKneeBend_Phys")]
    #[doc(alias = "ftCo_HammerLanding_Phys")]
    pub fn co_hammer_ground_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// `ftCo_HammerWait_Coll` (and the other grounded states'): off a ledge into the hammer's
    /// fall.
    #[doc(alias = "ftCo_HammerWait_Coll")]
    #[doc(alias = "ftCo_HammerWalk_Coll")]
    #[doc(alias = "ftCo_HammerTurn_Coll")]
    #[doc(alias = "ftCo_HammerKneeBend_Coll")]
    #[doc(alias = "ftCo_HammerLanding_Coll")]
    pub fn co_hammer_ground_coll(&mut self, gobj: GObjId) {
        self.ground_coll_fall_off_then(gobj, |g, gobj| g.enter_hammer_fall(gobj));
    }

    /// The hammer's frame, `ftCo_800C511C` (from `Fighter_procAnim` while it's held), except
    /// while picking it up: its time counts down (`x2330.x`); in its states, the swings until
    /// it loses its head (`x2330.y`), when it does (`it_80293E34`); out of time it goes, and the
    /// fighter falls or stands if it was swinging; and the drop's window (`x2338.y`) counts
    /// down.
    #[doc(alias = "ftCo_800C511C")]
    pub fn hammer_tick(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].motion_id == ms::LIGHT_GET {
            return;
        }
        let fp = &mut self.game.fighters[f.0];
        if fp.x2330.x != 0 {
            fp.x2330.x -= 1;
        }
        if self.game.fighters[f.0].x2330.y != 0 && self.in_hammer_state(gobj) {
            let fp = &mut self.game.fighters[f.0];
            fp.x2330.y -= 1;
            if fp.x2330.y == 0 {
                fp.x2338.x = 1;
                let it = fp.item_gobj.expect("no hammer held");
                kinds::hammer::lose_head(self, it);
                self.stop_rumble(f, 1);
            }
        }
        if self.game.fighters[f.0].x2330.x == 0 {
            let it = self.game.fighters[f.0].item_gobj.expect("no hammer held");
            it::item::destroy(self, it);
            self.hammer_crowd_less(f);
            let colanim = self.color_anim_id(gobj) == HAMMER_COLANIM;
            if self.in_hammer_state(gobj) {
                self.fall_or_wait(gobj);
            }
            if colanim {
                self.end_color_anim(gobj, HAMMER_COLANIM);
            }
        }
        let fp = &mut self.game.fighters[f.0];
        if fp.x2338.y != 0 {
            fp.x2338.y -= 1;
        }
    }

    /// The crowd's second loop wanted as often fewer as the fighter asked, `ft_80088110`.
    #[doc(alias = "ft_80088110")]
    pub fn hammer_crowd_less(&mut self, f: super::super::FighterId) {
        let fp = &mut self.game.fighters[f.0];
        let n = fp.x2168;
        fp.x2168 = 0;
        self.lb_audio_crowd_b_less(n);
    }

    /// The hammer's timers set as it's taken, `ftCo_800C5284`: its frames, the swings until it
    /// loses its head (`it_80293DCC`), the statistic for a hammer that will, and the crowd's
    /// second loop (`ft_800880AC`).
    #[doc(alias = "ftCo_800C5284")]
    pub fn start_hammer_timers(&mut self, gobj: GObjId) {
        let held = self.common().i32(HELD_FRAMES);
        let f = self.game.fighter_id(gobj);
        let it = self.game.fighters[f.0].item_gobj.expect("no hammer held");
        let swings = kinds::hammer::swings_to_lose_head(self, it);
        let fp = &mut self.game.fighters[f.0];
        fp.x2330.x = held;
        fp.x2330.y = swings;
        fp.x2338.x = 0;
        fp.x2338.y = 0;
        if fp.x2330.y != 0 {
            let slot = i32::from(fp.player_idx);
            self.count_headless_hammer(slot);
        }
        self.hammer_crowd(f);
    }

    /// The crowd's second loop wanted once more for the fighter, `ft_800880AC`.
    #[doc(alias = "ft_800880AC")]
    pub fn hammer_crowd(&mut self, f: super::super::FighterId) {
        self.game.fighters[f.0].x2168 += 1;
        self.lb_audio_crowd_b();
    }

    /// A hammer that will lose its head counted for the player, `pl_800403C0` (point 0xC5).
    #[doc(alias = "pl_800403C0")]
    pub fn count_headless_hammer(&mut self, slot: i32) {
        self.set_point(slot, 0xC5, 1);
    }

    /// A dropped hammer counted for the player, `pl_8003FDF4` (point 0xC4).
    #[doc(alias = "pl_8003FDF4")]
    pub fn count_dropped_hammer(&mut self, slot: i32) {
        self.set_point(slot, 0xC4, 1);
    }

    /// The hammer taken in hand, `ftCo_800C52F4`: its timers, then standing or falling with
    /// it.
    #[doc(alias = "ftCo_800C52F4")]
    pub fn take_hammer(&mut self, gobj: GObjId) {
        self.start_hammer_timers(gobj);
        if self.game.fighter(gobj).ground_or_air == 0 {
            self.enter_hammer_wait(gobj);
        } else {
            self.enter_hammer_fall(gobj);
        }
    }

    /// The hammer let go of, `ftCo_800C5500`: its color animation off and the crowd's loop.
    #[doc(alias = "ftCo_800C5500")]
    pub fn let_go_hammer(&mut self, gobj: GObjId) {
        self.hammer_colanim_off(gobj);
        let f = self.game.fighter_id(gobj);
        self.hammer_crowd_less(f);
    }

    /// The drop's window opened, `ftCo_800C554C` (as hitstun ends with the hammer held).
    #[doc(alias = "ftCo_800C554C")]
    pub fn open_hammer_drop(&mut self, gobj: GObjId) {
        let w = self.common().i32(DROP_WINDOW);
        self.game.fighter_mut(gobj).x2338.y = w;
    }

    /// A walk, `ftCo_800C55CC`.
    #[doc(alias = "ftCo_800C55CC")]
    pub fn hammer_walk_check(&mut self, gobj: GObjId) -> bool {
        if self.walk_input(gobj) {
            self.enter_hammer_walk(gobj);
            return true;
        }
        false
    }

    /// Walking with the hammer, `ftCo_800C5614`: its acceleration scaled by the fighter's
    /// height, from its ground speed, the animation run once.
    #[doc(alias = "ftCo_800C5614")]
    pub fn enter_hammer_walk(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        let mut accel = 1.0;
        let scale = reg(fp.x34_scale.y);
        if scale != 1.0 {
            let t = super::super::common_ptr(self.game, self.data, super::super::SCALE_PARAMS)
                .expect("no fighter scale parameters");
            accel = it::owner::y_scaled(1.0, scale, reg(self.data.f32(t.at(0xC))));
        }
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_f32(MV_WALK_ACCEL_MUL, accel);
        let v = fp.gr_vel;
        fp.set_mv_f32(MV_WALK_X0, reg(v));
        self.change_hammer_state(gobj, ms::HAMMER_WALK);
        self.anim_update(gobj);
        self.hammer_state_entered(gobj);
    }

    /// `ftCo_HammerWalk_IASA`: a jump, a platform drop, or standing.
    #[doc(alias = "ftCo_HammerWalk_IASA")]
    pub fn co_hammer_walk_iasa(&mut self, gobj: GObjId) {
        if self.hammer_jump_check(gobj) || self.hammer_pass_check(gobj) {
            return;
        }
        self.hammer_wait_check(gobj);
    }

    /// A turn, `ftCo_800C576C`.
    #[doc(alias = "ftCo_800C576C")]
    pub fn hammer_turn_check(&mut self, gobj: GObjId) -> bool {
        if self.turn_input(gobj) {
            self.enter_hammer_turn(gobj);
            return true;
        }
        false
    }

    /// Turning with the hammer, over `ftCommonData` x6B0 frames, `ftCo_800C57B4`.
    #[doc(alias = "ftCo_800C57B4")]
    pub fn enter_hammer_turn(&mut self, gobj: GObjId) {
        let flags = self.hammer_flags(gobj);
        let frames = fp::frsp(f64::from(self.common().i32(TURN_FRAMES)));
        let start = self.hammer_anim_start(gobj);
        self.enter_turn(gobj, ms::HAMMER_TURN, flags, 0.0, mem(frames), start);
        self.hammer_state_entered(gobj);
    }

    /// `ftCo_HammerTurn_Anim`: the swing, and the turn's frames counting down, the body turned
    /// a half turn over them (unless `x2222_b6`), then facing the other way, standing.
    #[doc(alias = "ftCo_HammerTurn_Anim")]
    pub fn co_hammer_turn_anim(&mut self, gobj: GObjId) {
        self.hammer_swing(gobj);
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].mv_f32(MV_TURN_LEFT) == 0.0 {
            return;
        }
        let part = self.bone_index(&self.game.fighters[f.0], super::super::parts::TRANS_N);
        let j = self.bone(f, part).x4_jobj2;
        let fp = &mut self.game.fighters[f.0];
        let left = fp::fsubs(reg(fp.mv_f32(MV_TURN_LEFT)), 1.0);
        fp.set_mv_f32(MV_TURN_LEFT, left);
        if !fp.x2222_b6 {
            let frames = fp::frsp(f64::from(self.common().i32(TURN_FRAMES)));
            let left = reg(self.game.fighters[f.0].mv_f32(MV_TURN_LEFT));
            let rot = mem(fp::frsp(fp::fmul(
                std::f64::consts::PI,
                fp::fdivs(fp::fsubs(frames, left), frames),
            )));
            self.game.part_set_rot_y(f, part, rot);
            jobj::set_rotation_y(&mut self.game.objs.jobjs, j.expect("no TransN joint"), rot);
        }
        if self.game.fighters[f.0].mv_f32(MV_TURN_LEFT) <= 0.0 {
            let fp = &mut self.game.fighters[f.0];
            fp.facing_dir = mem(fp::fneg(reg(fp.facing_dir)));
            self.enter_hammer_wait(gobj);
        }
    }

    /// `ftCo_HammerTurn_IASA`: a jump.
    #[doc(alias = "ftCo_HammerTurn_IASA")]
    pub fn co_hammer_turn_iasa(&mut self, gobj: GObjId) {
        self.hammer_jump_check(gobj);
    }

    /// A jump's squat, `ftCo_800C5A50`.
    #[doc(alias = "ftCo_800C5A50")]
    pub fn hammer_jump_check(&mut self, gobj: GObjId) -> bool {
        let input = self.jump_input(gobj);
        if input != 0 {
            self.enter_hammer_knee_bend(gobj, input);
            return true;
        }
        false
    }

    /// Leaving the ground with the hammer, `ftCo_800C5A98`.
    #[doc(alias = "ftCo_800C5A98")]
    pub fn enter_hammer_jump(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.go_airborne(f);
        self.change_hammer_state(gobj, ms::HAMMER_JUMP);
        self.jump_take_off(gobj, false, 1.0);
        self.hammer_state_entered(gobj);
    }

    /// The jump's squat with the hammer, `ftCo_800C5B88`, for the character's jump squat.
    #[doc(alias = "ftCo_800C5B88")]
    pub fn enter_hammer_knee_bend(&mut self, gobj: GObjId, input: i32) {
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(MV_KNEE_INPUT, input);
        fp.set_mv_i32(MV_KNEE_SHORT_HOP, 0);
        let t = fp.co_attrs.jump_startup_time;
        fp.set_mv_f32(MV_KNEE_LEFT, reg(t));
        self.change_hammer_state(gobj, ms::HAMMER_KNEE_BEND);
        self.hammer_state_entered(gobj);
    }

    /// `ftCo_HammerKneeBend_Anim`: the swing, and the jump once the squat's frames are out.
    #[doc(alias = "ftCo_HammerKneeBend_Anim")]
    pub fn co_hammer_knee_bend_anim(&mut self, gobj: GObjId) {
        self.hammer_swing(gobj);
        if self.game.fighter(gobj).mv_f32(MV_KNEE_LEFT) <= 0.0 {
            self.enter_hammer_jump(gobj);
        }
        let fp = self.game.fighter_mut(gobj);
        let left = fp::fsubs(reg(fp.mv_f32(MV_KNEE_LEFT)), 1.0);
        fp.set_mv_f32(MV_KNEE_LEFT, left);
    }

    /// Falling with the hammer once A or B is pressed, `ftCo_800C5CD4`.
    #[doc(alias = "ftCo_800C5CD4")]
    pub fn hammer_fall_check(&mut self, gobj: GObjId) -> bool {
        if self.holding_hammer(gobj)
            && self.game.fighter(gobj).input.pressed_buttons & (PAD_A | PAD_B) != 0
        {
            self.enter_hammer_fall(gobj);
            return true;
        }
        false
    }

    /// Falling with the hammer, `ftCo_800C5D34` (off the ground if it was on it).
    #[doc(alias = "ftCo_800C5D34")]
    pub fn enter_hammer_fall(&mut self, gobj: GObjId) {
        self.change_hammer_state(gobj, ms::HAMMER_FALL);
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].ground_or_air == 0 {
            self.go_airborne(f);
        }
        self.hammer_state_entered(gobj);
    }

    /// Dropping the hammer, `ftCo_800C5DDC`: in the drop's window, A pressed with a shoulder
    /// held past `ftCommonData` x1C frames, the hammer drops, the fighter falls, its color
    /// animation off, and the statistic counts it (`pl_8003FDF4`).
    #[doc(alias = "ftCo_800C5DDC")]
    pub fn hammer_drop_check(&mut self, gobj: GObjId) -> bool {
        let window = self.common().u32(0x1C);
        let fp = self.game.fighter(gobj);
        if !(fp.x2338.y != 0
            && fp.input.pressed_buttons & PAD_A != 0
            && fp.input.held_buttons[0] & PAD_LR != 0
            && u32::from(fp.x683) >= window)
        {
            return false;
        }
        let it = fp.item_gobj.expect("no hammer held");
        it::release::drop(self, it, crate::vec::Vec3::new(0.0, 0.0, 0.0), 1.0);
        self.enter_fall(gobj);
        self.hammer_colanim_off(gobj);
        let slot = i32::from(self.game.fighter(gobj).player_idx);
        self.count_dropped_hammer(slot);
        true
    }

    /// `ftCo_HammerFall_IASA`: the drop.
    #[doc(alias = "ftCo_HammerFall_IASA")]
    pub fn co_hammer_fall_iasa(&mut self, gobj: GObjId) {
        self.hammer_drop_check(gobj);
    }

    /// `ftCo_HammerFall_Coll` (and the jump's): landing with the hammer.
    #[doc(alias = "ftCo_HammerFall_Coll")]
    #[doc(alias = "ftCo_HammerJump_Coll")]
    pub fn co_hammer_air_coll(&mut self, gobj: GObjId) {
        self.air_coll_then(gobj, |g, gobj| g.enter_hammer_landing(gobj));
    }

    /// Landing with the hammer, `ftCommonData` x6B4 frames, `ftCo_HammerLanding_Enter`.
    #[doc(alias = "ftCo_HammerLanding_Enter")]
    pub fn enter_hammer_landing(&mut self, gobj: GObjId) {
        let frames = fp::frsp(f64::from(self.common().i32(LANDING_FRAMES)));
        self.game
            .fighter_mut(gobj)
            .set_mv_f32(MV_LANDING_LEFT, frames);
        let flags = self.hammer_flags(gobj);
        let start = self.hammer_anim_start(gobj);
        self.enter_landing(
            gobj,
            ms::HAMMER_LANDING,
            KEEP_FAST_FALL as i32,
            flags,
            start,
            1.0,
        );
        self.hammer_state_entered(gobj);
    }

    /// `ftCo_HammerLanding_Anim`: the swing, and standing once its frames are out.
    #[doc(alias = "ftCo_HammerLanding_Anim")]
    pub fn co_hammer_landing_anim(&mut self, gobj: GObjId) {
        self.hammer_swing(gobj);
        if self.game.fighter(gobj).mv_f32(MV_LANDING_LEFT) <= 0.0 {
            self.enter_hammer_wait(gobj);
        }
        let fp = self.game.fighter_mut(gobj);
        let left = fp::fsubs(reg(fp.mv_f32(MV_LANDING_LEFT)), 1.0);
        fp.set_mv_f32(MV_LANDING_LEFT, left);
    }

    /// A platform drop with the hammer, `ftCo_800C60C8`.
    #[doc(alias = "ftCo_800C60C8")]
    pub fn hammer_pass_check(&mut self, gobj: GObjId) -> bool {
        if self.pass_input(gobj) {
            self.enter_hammer_pass(gobj);
            return true;
        }
        false
    }

    /// Through the platform into the hammer's fall, `ftCo_800C6110`.
    #[doc(alias = "ftCo_800C6110")]
    pub fn enter_hammer_pass(&mut self, gobj: GObjId) {
        self.enter_pass_as(gobj, ms::HAMMER_FALL, 0, 0.0);
        self.hammer_state_entered(gobj);
    }
}

/// The hammer's hitboxes, `it_8026C16C`: with a head left and `on`, all on (`it_80272784`);
/// else off (`it_802725D4`).
#[doc(alias = "it_8026C16C")]
pub fn hammer_hits_on(r: &mut Running, gobj: GObjId, on: bool) {
    if it::item(r.game, gobj).xD4C <= 0 || !on {
        lib2725::clear_hitboxes(r.game, r.data, gobj);
    } else {
        all_hitboxes_on(r, gobj);
    }
}

/// Every hitbox on, `it_80272784`.
#[doc(alias = "it_80272784")]
pub fn all_hitboxes_on(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    for h in &mut ip.x5D4_hitboxes {
        hitcapsule::enable(&mut h.hit);
    }
    ip.xDC8_word.flags.x16 = true;
    for i in 0..4 {
        it::coll::update_hitbox(r.game, r.data, gobj, i);
    }
}
