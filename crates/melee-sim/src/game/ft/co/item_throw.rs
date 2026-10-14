//! Throwing the held item (`ftCo_ItemThrow.c`): the throw picked from the stick or the
//! C-stick, the throw states, and the item let go when the throw's script says, at the speed
//! and angle `Fighter_804D6550`'s table gives the state.

use gekko_fp as fp;

use super::super::super::it::{lib26b1, lib2725, release};
use super::super::super::state::Running;
use super::super::motion::flags::*;
use super::PAD_LR;
use super::ids::ms;
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::math;
use crate::vec::Vec3;

/// `itemthrow.facing_dir` (and `itemthrow4.unk_timer`), `itemthrow4.anim_spd` and
/// `itemthrow4.x8` in `mv`.
const MV_FACING_DIR: usize = 0x0;
const MV_ANIM_SPD: usize = 0x4;
const MV_X8: usize = 0x8;

/// `ftLoadCommonData`'s `x4`: the throws' speed, angle and heavy speed per state
/// (`Fighter_804D6550`, `ftCo_ItemThrowAttrs`), from `ftCo_MS_LightThrowF`.
const THROW_ATTRS: u32 = 0x4;

/// `ftCommonData`'s fields the throws read.
mod cd {
    pub const X20_RADIANS: u32 = 0x20;
    pub const DASH_SMASH_STICK_THRESHOLD: u32 = 0x3C;
    pub const DASH_SMASH_WINDOW: u32 = 0x40;
    pub const X44: u32 = 0x44;
    pub const X98: u32 = 0x98;
    pub const ATTACKHI3_STICK_THRESHOLD_Y: u32 = 0xAC;
    pub const XB0: u32 = 0xB0;
    pub const XCC: u32 = 0xCC;
    pub const XD0: u32 = 0xD0;
    pub const XD4: u32 = 0xD4;
    pub const XD8: u32 = 0xD8;
    pub const XDC: u32 = 0xDC;
    pub const XE0: u32 = 0xE0;
    pub const X3FC: u32 = 0x3FC;
    pub const X400: u32 = 0x400;
    pub const X404: u32 = 0x404;
    pub const X408: u32 = 0x408;
    pub const X40C: u32 = 0x40C;
}

/// The flags a throw changing between the ground and the air keeps (`inlineA0`).
const AIR_GROUND_MF: u32 = SKIP_MAT_ANIM
    | SKIP_COL_ANIM
    | UPDATE_CMD
    | SKIP_ITEM_VIS
    | UNK_19
    | SKIP_MODEL_PART_VIS
    | SKIP_MODEL_FLAGS
    | UNK_27;

/// `ABS`.
fn abs(v: f64) -> f64 {
    if v < 0.0 { fp::fneg(v) } else { v }
}

impl Running<'_> {
    /// What the held item's throw would be with the C-stick, `ftCo_800DF50C`: a C-stick flick,
    /// unless single-button mode is on and the item isn't one A only throws.
    #[doc(alias = "ftCo_800DF50C")]
    pub fn item_cstick_throw_input(&mut self, gobj: GObjId) -> bool {
        let may = !self.game.vs_single_button() || self.item_a_kind(gobj) == 0;
        may && self.cstick_flick(gobj)
    }

    /// Throwing the held item in the air, `ftCo_80095328`: a C-stick flick or A (with a
    /// shoulder, or an item A only throws) throws it the way the stick points, a smash throw
    /// when the stick was flicked; with the stick near the middle it's dropped. `Some` when it
    /// threw, holding whether the state changed (the original's `arg1`).
    #[doc(alias = "ftCo_80095328")]
    pub fn item_throw_air(&mut self, gobj: GObjId) -> Option<bool> {
        let fp = self.game.fighter(gobj);
        if fp.item_gobj.is_none() {
            return None;
        }
        let (x, y, sx, sy, angle);
        if self.item_cstick_throw_input(gobj) {
            let fp = self.game.fighter(gobj);
            x = reg(fp.input.cstick[0].x);
            y = reg(fp.input.cstick[0].y);
            sy = 0.0;
            sx = 0.0;
            angle = self.cstick_angle(gobj);
        } else if self.item_throw_input(gobj) {
            let fp = self.game.fighter(gobj);
            x = reg(fp.input.lstick[0].x);
            y = reg(fp.input.lstick[0].y);
            sx = fp::frsp(f64::from(fp.active_sticky.lstick.x));
            sy = fp::frsp(f64::from(fp.active_sticky.lstick.y));
            angle = self.lstick_angle(gobj);
        } else {
            return None;
        }
        let c = self.common();
        if abs(x) < reg(c.f32(cd::XDC)) && abs(y) < reg(c.f32(cd::XE0)) {
            let held = self.game.fighter(gobj).input.held_buttons[0] & PAD_LR != 0;
            if self.item_a_kind(gobj) == 0 && !held {
                self.enter_item_throw(gobj, ms::LIGHT_THROW_AIR_F);
                return Some(true);
            }
            let changed = self.item_drop_from_hand(gobj);
            self.game.fighter_mut(gobj).x2224_b1 = true;
            return Some(changed);
        }
        let c = self.common();
        let radians = reg(c.f32(cd::X20_RADIANS));
        let flick = fp::frsp(f64::from(c.i32(cd::X3FC)));
        let facing = reg(self.game.fighter(gobj).facing_dir);
        let msid = if angle > radians {
            if sy < flick {
                ms::LIGHT_THROW_AIR_HI4
            } else {
                ms::LIGHT_THROW_AIR_HI
            }
        } else if angle < fp::fneg(radians) {
            if sy < flick {
                ms::LIGHT_THROW_AIR_LW4
            } else {
                ms::LIGHT_THROW_AIR_LW
            }
        } else if fp::fmuls(x, facing) >= 0.0 {
            if sx < flick {
                ms::LIGHT_THROW_AIR_F4
            } else {
                ms::LIGHT_THROW_AIR_F
            }
        } else if sx < flick {
            ms::LIGHT_THROW_AIR_B4
        } else {
            ms::LIGHT_THROW_AIR_B
        };
        self.enter_item_throw(gobj, msid);
        Some(true)
    }

    /// The held item dropped where it is, `ftCo_80095744`: holding a parasol, the fighter
    /// falls (helplessly from a helpless parasol fall). Whether the state changed.
    #[doc(alias = "ftCo_80095744")]
    pub fn item_drop_from_hand(&mut self, gobj: GObjId) -> bool {
        let changed = if self.parasol_status(gobj) != -1 {
            if self.game.fighter(gobj).motion_id == ms::ITEM_PARASOL_FALL_SPECIAL {
                self.enter_fall_special_basic(gobj);
            } else {
                self.enter_fall(gobj);
            }
            true
        } else {
            false
        };
        let item = self.game.fighter(gobj).item_gobj.expect("dropping no item");
        release::drop(self, item, Vec3::new(0.0, 0.0, 0.0), 1.0);
        changed
    }

    /// The throw's animation speed: a smash throw's (`ftCommonData` x400), over the item's
    /// damage multiplier (`getAnimSpeed`).
    fn item_throw_anim_speed(&self, gobj: GObjId, msid: i32) -> f64 {
        let mut speed = 1.0;
        if msid >= ms::LIGHT_THROW_F4 {
            speed = fp::fmuls(speed, reg(self.common().f32(cd::X400)));
        }
        let item = self.game.fighter(gobj).item_gobj.expect("throwing no item");
        let mul = lib26b1::damage_mul(self.game, self.data, item);
        fp::fmuls(speed, fp::fdivs(1.0, reg(mul)))
    }

    /// Throwing the held item in state `msid`, `ftCo_800957F4`: the script's variables and
    /// throw flags cleared, the throw's speed, its facing (back throws face away,
    /// `ftCo_80095700`), and the hand followed until the script lets go (`ftCo_80095EFC`).
    #[doc(alias = "ftCo_800957F4")]
    #[doc(alias = "ftCo_80095700")]
    pub fn enter_item_throw(&mut self, gobj: GObjId, msid: i32) {
        let speed = self.item_throw_anim_speed(gobj, msid);
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[0] = 0;
        fp.cmd_vars[1] = 0;
        fp.x2210 = [0; 4];
        fp.set_mv_f32(MV_ANIM_SPD, speed);
        // `ftCo_80095700`.
        let facing = match msid {
            95 | 101 | 105 | 109 | 113 | 117 => fp::fneg(reg(fp.facing_dir)),
            _ => reg(fp.facing_dir),
        };
        fp.set_mv_f32(MV_FACING_DIR, facing);
        self.change_motion_state(gobj, msid, 0, 0.0, mem(speed), 0.0, None);
        self.anim_update(gobj);
        self.game.fighter_mut(gobj).accessory4_cb =
            Some(super::super::super::handles::CodeRef(ITEM_THROW_HAND));
        self.item_throw_hand(gobj);
    }

    /// Throwing the held item on the ground, `ftCo_80095A30`: a smash throw for a flicked
    /// stick, else the way the stick points, else forward (or dropped, for an item A doesn't
    /// only throw).
    #[doc(alias = "ftCo_80095A30")]
    pub fn item_throw_ground(&mut self, gobj: GObjId) {
        let c = self.common();
        let fp = self.game.fighter(gobj);
        let (x, y) = (reg(fp.input.lstick[0].x), reg(fp.input.lstick[0].y));
        let sx = fp::frsp(f64::from(fp.active_sticky.lstick.x));
        let sy = fp::frsp(f64::from(fp.active_sticky.lstick.y));
        let facing = reg(fp.facing_dir);
        let jump_startup = reg(fp.co_attrs.jump_startup_time);
        let window = fp::fadds(
            fp::frsp(f64::from(c.i32(cd::DASH_SMASH_WINDOW))),
            reg(c.f32(cd::X44)),
        );
        let msid = if abs(x) >= reg(c.f32(cd::DASH_SMASH_STICK_THRESHOLD)) && sx < window {
            if fp::fmuls(x, facing) >= 0.0 {
                ms::LIGHT_THROW_F4
            } else {
                ms::LIGHT_THROW_B4
            }
        } else if y >= reg(c.f32(cd::XCC)) && sy < fp::fadds(reg(c.f32(cd::XD0)), jump_startup) {
            ms::LIGHT_THROW_HI4
        } else if y <= reg(c.f32(cd::XD4)) && sy < reg(c.f32(cd::XD8)) {
            ms::LIGHT_THROW_LW4
        } else {
            let radians = reg(c.f32(cd::X20_RADIANS));
            let sideways = abs(x) >= reg(c.f32(cd::X98)) && {
                let a = self.lstick_angle(gobj);
                let a = if a < 0.0 {
                    fp::fneg(self.lstick_angle(gobj))
                } else {
                    a
                };
                a <= radians
            };
            if sideways {
                if fp::fmuls(x, facing) >= 0.0 {
                    ms::LIGHT_THROW_F
                } else {
                    ms::LIGHT_THROW_B
                }
            } else if y >= reg(c.f32(cd::ATTACKHI3_STICK_THRESHOLD_Y))
                && self.lstick_angle(gobj) > radians
            {
                ms::LIGHT_THROW_HI
            } else if y <= reg(c.f32(cd::XB0)) && self.lstick_angle(gobj) < fp::fneg(radians) {
                ms::LIGHT_THROW_LW
            } else if self.item_a_kind(gobj) == 0 {
                ms::LIGHT_THROW_F
            } else {
                ms::LIGHT_THROW_DROP
            }
        };
        self.enter_item_throw(gobj, msid);
    }

    /// The thrown item's velocity, `ftCo_80095D5C`: the state's speed and angle from the
    /// throws' table, unless the script's first variable gives a speed (bits 12-21, percent)
    /// and angle (its low 12 bits, signed; 361 the table's), along the throw's facing.
    #[doc(alias = "ftCo_80095D5C")]
    fn item_throw_velocity(&mut self, gobj: GObjId) -> Vec3 {
        let f = self.game.fighter_id(gobj);
        let table = self.common_ptr(THROW_ATTRS);
        let fp = &self.game.fighters[f.0];
        let cmd = fp.cmd_vars[0];
        let mut vel = 1.0;
        if cmd != 0 {
            vel = fp::fmuls(f64::from(0.01f32), fp::frsp(f64::from((cmd >> 12) & 0x3FF)));
        }
        let entry = table.at(12 * (fp.motion_id - ms::LIGHT_THROW_F) as u32);
        let mul = reg(fp.co_attrs.item_throw_velocity_multiplier);
        vel = fp::fmuls(vel, fp::fmuls(mul, reg(self.data.f32(entry))));
        let angle = if cmd != 0 {
            let int_angle = ((cmd << 20) as i32) >> 20;
            let a = if int_angle == 361 {
                reg(self.data.f32(entry.at(4)))
            } else {
                fp::fmuls(f64::from(0.017453292f32), fp::frsp(f64::from(int_angle)))
            };
            self.game.fighters[f.0].cmd_vars[0] = 0;
            a
        } else {
            reg(self.data.f32(entry.at(4)))
        };
        let facing = reg(self.game.fighters[f.0].mv_f32(MV_FACING_DIR));
        Vec3::new(
            mem(fp::fmuls(
                facing,
                fp::fmuls(vel, reg(math::cos(mem(angle)))),
            )),
            mem(fp::fmuls(vel, reg(math::sin(mem(angle))))),
            0.0,
        )
    }

    /// `ftCo_ItemThrow_Anim`: a back throw turns the fighter when its script says; at the
    /// end it falls or waits.
    #[doc(alias = "ftCo_ItemThrow_Anim")]
    pub fn co_item_throw_anim(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.take_throw_flag(4) {
            fp.facing_dir = mem(fp::fneg(reg(fp.facing_dir)));
        }
        if !self.anim_frames_remaining(gobj) {
            self.fall_or_wait(gobj);
        }
    }

    /// The hand followed through a throw, `ftCo_80095EFC`: until the script's throw flag, the
    /// item's attach point is kept; then the item is let go between where it was and where it
    /// is, by how far into the frame the flag came, at the throw's velocity and speed (the
    /// script's second variable a percent), with the throw's rumble.
    #[doc(alias = "ftCo_80095EFC")]
    pub fn item_throw_hand(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let Some(item) = self.game.fighters[f.0].item_gobj else {
            return;
        };
        let at = {
            let ip = super::super::super::it::item(self.game, item);
            let model = super::super::super::it::item::article(ip).model(self.data);
            let attach = model.bone_attach_id(self.data);
            let j =
                lib2725::bone(self.game, self.data, item, attach).expect("the item's attach bone");
            crate::lb::joint::world_position(&mut self.game.objs, self.data, Some(j), None)
        };
        if !self.game.fighters[f.0].take_throw_flag(3) {
            let fp = &mut self.game.fighters[f.0];
            fp.set_mv_f32(MV_X8, reg(at.x));
            fp.set_mv_f32(MV_X8 + 4, reg(at.y));
            fp.set_mv_f32(MV_X8 + 8, reg(at.z));
            return;
        }
        let vel = self.item_throw_velocity(gobj);
        let table = self.common_ptr(THROW_ATTRS);
        let fp = &mut self.game.fighters[f.0];
        let cmd1 = fp.cmd_vars[1];
        let mut scale = 1.0f32;
        if cmd1 != 0 {
            scale = mem(fp::fmuls(
                f64::from(0.01f32),
                fp::frsp(f64::from(cmd1 & 0x3F_FFFF)),
            ));
            fp.cmd_vars[1] = 0;
        }
        let fsm = fp::fdivs(fp::fneg(reg(fp.cmd_timer)), reg(fp.frame_speed_mul));
        let entry = table.at(12 * (fp.motion_id - ms::LIGHT_THROW_F) as u32);
        let heavy_mul = reg(fp.co_attrs.heavy_throw_velocity_multiplier);
        scale = mem(fp::fmuls(
            reg(scale),
            fp::fmuls(heavy_mul, reg(self.data.f32(entry.at(8)))),
        ));
        let (ox, oy) = (fp.mv_f32(MV_X8), fp.mv_f32(MV_X8 + 4));
        let pos = Vec3::new(
            mem(fp::fmadds(fsm, fp::fsubs(reg(ox), reg(at.x)), reg(at.x))),
            mem(fp::fmadds(fsm, fp::fsubs(reg(oy), reg(at.y)), reg(at.y))),
            0.0,
        );
        let (slot, sub) = (i32::from(fp.player_idx), i32::from(fp.is_sub_fighter));
        self.item_thrown_stat(slot, sub);
        let msid = self.game.fighters[f.0].motion_id;
        if msid == ms::LIGHT_THROW_DROP {
            release::drop_at(self, item, pos, vel, scale);
        } else {
            let heavy = lib26b1::is_heavy(self.game, self.data, item);
            let rumble = match (msid >= ms::LIGHT_THROW_F4, heavy) {
                (true, true) => 29,
                (true, false) => 27,
                (false, true) => 28,
                (false, false) => 26,
            };
            self.start_rumble(f, rumble, 0);
            release::throw_at(self, item, pos, vel, scale);
        }
    }

    /// A thrown item's statistic, `pl_8003E978` (`fn_8003E998`): a main fighter keeps who
    /// last hit it with an item (`xD6C`) in its attack record.
    #[doc(alias = "pl_8003E978")]
    #[doc(alias = "fn_8003E998")]
    pub fn item_thrown_stat(&mut self, slot: i32, sub: i32) {
        let who = self.game.player(slot).ext_u32(0xD6C) as i32;
        if sub == 0 && who != 6 {
            let g = self
                .game
                .player_entity_at(slot, sub)
                .expect("a thrower without a fighter");
            self.game.fighter_mut(g).x2074.x207C.y = who;
        }
    }

    /// `ftCo_ItemThrow_IASA`: nothing interrupts a throw.
    #[doc(alias = "ftCo_ItemThrow_IASA")]
    pub fn co_item_throw_iasa(&mut self, _gobj: GObjId) {}

    /// `ftCo_ItemThrow_Phys`: friction.
    #[doc(alias = "ftCo_ItemThrow_Phys")]
    pub fn co_item_throw_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// `ftCo_LightThrowDash_Phys`: the dash's slide, with more friction (`ftCommonData` x40C)
    /// up to frame x408.
    #[doc(alias = "ftCo_LightThrowDash_Phys")]
    pub fn co_light_throw_dash_phys(&mut self, gobj: GObjId) {
        let c = self.common();
        let fp = self.game.fighter(gobj);
        let friction = fp::fmuls(reg(c.f32(cd::X404)), reg(fp.co_attrs.ground_friction));
        let friction = if reg(fp.cur_anim_frame) <= reg(c.f32(cd::X408)) {
            fp::fmuls(reg(c.f32(cd::X40C)), friction)
        } else {
            friction
        };
        let facing = reg(fp.facing_dir);
        self.ground_anim_motion(gobj, friction, facing);
    }

    /// `ftCo_LightThrowAir_Phys`: falling.
    #[doc(alias = "ftCo_LightThrowAir_Phys")]
    pub fn co_light_throw_air_phys(&mut self, gobj: GObjId) {
        self.air_physics(gobj);
    }

    /// `ftCo_LightThrowDashDrop_Coll`: on the ground, or falling.
    #[doc(alias = "ftCo_LightThrowDashDrop_Coll")]
    pub fn co_light_throw_dash_drop_coll(&mut self, gobj: GObjId) {
        self.ground_coll(gobj);
    }

    /// `ftCo_LightThrow_Coll`: off the ground, the air's throw (`ftCo_80096250`).
    #[doc(alias = "ftCo_LightThrow_Coll")]
    pub fn co_light_throw_coll(&mut self, gobj: GObjId) {
        self.ground_coll_then(gobj, |r, g| r.item_throw_to_air(g));
    }

    /// The ground throw going on in the air, `ftCo_80096250`.
    #[doc(alias = "ftCo_80096250")]
    pub fn item_throw_to_air(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.go_airborne(f);
        let msid = self.game.fighters[f.0].motion_id;
        let to = if msid >= ms::LIGHT_THROW_F4 {
            msid + 4
        } else {
            msid + 6
        };
        self.item_throw_switch(gobj, to);
    }

    /// `ftCo_LightThrowAir_Coll`: landing, the ground's throw (`ftCo_80096374`).
    #[doc(alias = "ftCo_LightThrowAir_Coll")]
    pub fn co_light_throw_air_coll(&mut self, gobj: GObjId) {
        self.air_coll_then(gobj, |r, g| r.item_throw_to_ground(g));
    }

    /// The air throw going on on the ground, `ftCo_80096374`.
    #[doc(alias = "ftCo_80096374")]
    pub fn item_throw_to_ground(&mut self, gobj: GObjId) {
        self.land(gobj);
        let msid = self.game.fighter(gobj).motion_id;
        let to = if msid >= ms::LIGHT_THROW_F4 {
            msid - 4
        } else {
            msid - 6
        };
        self.item_throw_switch(gobj, to);
    }

    /// The throw's state swapped for its ground or air twin from the same frame, facing as
    /// it started (`inlineA0`).
    fn item_throw_switch(&mut self, gobj: GObjId, msid: i32) {
        let fp = self.game.fighter_mut(gobj);
        let facing = fp.facing_dir;
        if reg(facing) != reg(fp.facing_dir1) {
            fp.facing_dir = fp.facing_dir1;
        }
        let (frame, speed) = (fp.cur_anim_frame, fp.mv_f32(MV_ANIM_SPD));
        self.change_motion_state(gobj, msid, AIR_GROUND_MF, frame, speed, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        fp.facing_dir = facing;
        fp.accessory4_cb = Some(super::super::super::handles::CodeRef(ITEM_THROW_HAND));
        self.item_throw_hand(gobj);
    }

    /// Whether A throws the held item now, `ftCo_800951D0`: the ground throw.
    #[doc(alias = "ftCo_800951D0")]
    pub fn item_throw_ground_check(&mut self, gobj: GObjId) -> bool {
        if self.game.fighter(gobj).item_gobj.is_some() && self.item_throw_input(gobj) {
            self.item_throw_ground(gobj);
            return true;
        }
        false
    }

    /// Whether a shoulder throws the held item out of a dash, `ftCo_800952DC`.
    #[doc(alias = "ftCo_800952DC")]
    pub fn item_throw_dash_held_check(&mut self, gobj: GObjId) -> bool {
        let fp = self.game.fighter(gobj);
        if fp.item_gobj.is_some() && fp.input.held_buttons[0] & PAD_LR != 0 {
            self.enter_item_throw(gobj, ms::LIGHT_THROW_DASH);
            return true;
        }
        false
    }

    /// Whether A throws the held item out of a dash, `ftCo_80095254`.
    #[doc(alias = "ftCo_80095254")]
    pub fn item_throw_dash_check(&mut self, gobj: GObjId) -> bool {
        if self.game.fighter(gobj).item_gobj.is_some() && self.item_throw_input(gobj) {
            self.enter_item_throw(gobj, ms::LIGHT_THROW_DASH);
            return true;
        }
        false
    }
}

/// `ftCo_80095EFC`, the throw's accessory callback.
pub const ITEM_THROW_HAND: u32 = 0x8009_5EFC;
