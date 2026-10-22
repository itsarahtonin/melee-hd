//! Computer players getting where they're going (`ftCo_0A01.c`): the scripts that walk, run
//! and turn toward the destination on the ground (`ftCo_800AB224`), drift, jump and recover
//! toward it in the air (`ftCo_800A9CB4`), and leave the respawn platform.

use gekko_fp as fp;

use super::super::code::unported_call;
use super::super::handles::FighterId;
use super::super::state::Running;
use super::common::{GA_AIR, GA_GROUND};
use super::cpu_ai::{Leftover, P_LINK_FIGHTER};
use super::cpu_script::cmd;
use crate::fpu::{mem, reg};
use crate::math;

/// `ftLoadCommonData::x58`: the computer players' data, whose first word is its scripts.
const CPU_DATA: u32 = 0x58;

/// `Gr_Kind_Shrine` (Hyrule Temple) and `Gr_Kind_Icemt` (Icicle Mountain).
const GR_KIND_SHRINE: i32 = 7;
const GR_KIND_ICEMT: i32 = 0x16;

/// `Ft_Kind_Donkey`, `_Kirby`, `_Samus`, `_Mewtwo`.
const KIND_DONKEY: i32 = 3;
const KIND_KIRBY: i32 = 4;
const KIND_SAMUS: i32 = 13;
const KIND_MEWTWO: i32 = 16;

/// `LINE_FLAG_PLATFORM`: a floor a fighter can drop through.
const LINE_FLAG_PLATFORM: u32 = 1 << 8;

/// C's `ABS` on a single difference.
fn abs(x: f64) -> f64 {
    if x < 0.0 { fp::fneg(x) } else { x }
}

/// `ftCo_IsNearlyZero`.
fn nearly_zero(x: f64) -> bool {
    let tiny = reg(1e-5);
    x < tiny && x > fp::fneg(tiny)
}

impl Running<'_> {
    /// `ftCo_CpuSetNeutralStick`.
    pub(super) fn cpu_neutral_stick(&mut self, f: FighterId) {
        self.cpu_cmd1(f, cmd::SET_LSTICK_X, 0);
        self.cpu_cmd1(f, cmd::SET_LSTICK_Y, 0);
    }

    /// `ftCo_CpuFinishWithNeutralStick`.
    pub(super) fn cpu_finish_neutral(&mut self, f: FighterId) {
        self.cpu_neutral_stick(f);
        self.cpu_cmd(f, cmd::DONE);
    }

    /// `ftCo_CpuFinishWithNeutralY`.
    pub(super) fn cpu_finish_neutral_y(&mut self, f: FighterId) {
        self.cpu_cmd1(f, cmd::SET_LSTICK_Y, 0);
        self.cpu_cmd(f, cmd::DONE);
    }

    /// `ftCo_CpuTapR`.
    pub(super) fn cpu_tap_r(&mut self, f: FighterId) {
        self.cpu_cmd(f, cmd::PRESS_R);
        self.cpu_cmd1(f, cmd::WAIT_FOR, 1);
        self.cpu_cmd(f, cmd::RELEASE_R);
    }

    /// Turns around with a flick back (`ftCo_CpuTurnAround`, and `ftCo_800A0098`, which
    /// writes the same commands).
    #[doc(alias = "ftCo_800A0098")]
    pub(super) fn cpu_turn_around(&mut self, f: FighterId) {
        self.cpu_neutral_stick(f);
        self.cpu_cmd1(f, cmd::WAIT_FOR, 1);
        self.cpu_cmd1(f, cmd::LSTICK_X_FORWARD, 176);
        self.cpu_cmd1(f, cmd::WAIT_FOR, 1);
        self.cpu_cmd1(f, cmd::SET_LSTICK_Y, 0);
        self.cpu_cmd1(f, cmd::WAIT_FOR, 10);
        self.cpu_cmd1(f, cmd::SET_LSTICK_X, 0);
        self.cpu_cmd(f, cmd::DONE);
    }

    /// Steps forward off a teeter (`ftCo_CpuTurnAwayFromLedge`).
    fn cpu_turn_away_from_ledge(&mut self, f: FighterId) {
        self.cpu_cmd1(f, cmd::SET_LSTICK_Y, 0);
        self.cpu_cmd1(f, cmd::SET_LSTICK_X, 0);
        self.cpu_cmd1(f, cmd::WAIT_FOR, 1);
        self.cpu_cmd1(f, cmd::LSTICK_X_FORWARD, 127);
        self.cpu_cmd1(f, cmd::WAIT_FOR, 1);
        self.cpu_cmd1(f, cmd::SET_LSTICK_X, 0);
        self.cpu_cmd(f, cmd::DONE);
    }

    /// Runs one of the computer players' scripts from `PlCo.dat`, `ftCo_800B4880`: its
    /// commands are written in, each with as many argument bytes as its range says (one past
    /// `CpuCmd_OneArgEnd`'s range checks the argument, as the original does).
    // Ports `ftCo_800B4880`; aliased once a check reaches it.
    pub(super) fn cpu_data_script(&mut self, f: FighterId, idx: u32) {
        let data = self.common_ptr(CPU_DATA);
        let scripts = self.data.ptr(data).expect("the computer players' scripts");
        let mut at = self
            .data
            .ptr(scripts.at(4 * idx))
            .expect("a computer script");
        loop {
            let c = self.data.u8(at);
            if c == cmd::DONE {
                break;
            }
            self.cpu_cmd(f, c);
            let mut cur = c;
            if cur > cmd::ONE_ARG_END {
                at = at.at(1);
                cur = self.data.u8(at);
                self.cpu_cmd(f, cur);
            }
            if cur > cmd::DONE {
                at = at.at(1);
                self.cpu_cmd(f, self.data.u8(at));
            }
            at = at.at(1);
        }
        self.cpu_cmd(f, self.data.u8(at));
    }

    /// The nearest enemy just entering (respawning or riding in, `x221D_b6` or `x2168` with
    /// no ride) and in reach, `ftCo_800A5CE0`; none for a fighter that can't target.
    // Ports `ftCo_800A5CE0`; aliased once a check reaches it.
    pub(super) fn cpu_entering_enemy(&self, f: FighterId) -> Option<FighterId> {
        if self.cpu_untargetable(f) {
            return None;
        }
        let me = self.game.fighters[f.0].gobj;
        let mut closest: Option<(FighterId, f64)> = None;
        let mut cur = self.game.world.first_in(P_LINK_FIGHTER);
        while let Some(g) = cur {
            cur = self.game.world.gobjs[g.0].next;
            if me == Some(g) {
                continue;
            }
            let o = self.game.fighter_id(g);
            let other = &self.game.fighters[o.0];
            if other.is_sleeping || other.stamina_dead || self.game.ft_is_dead(g) {
                continue;
            }
            if other.x2226_b4 {
                unported_call("ft_80087A18 with a power-up's color animation");
            }
            let entering = other.x221D_b6 || (other.x2168 != 0 && other.x2338.x == 0);
            if !entering || self.cpu_is_ally(f, o) {
                continue;
            }
            let pos = other.cur_pos;
            if self.cpu_out_of_bounds(f, pos.x, pos.y) {
                continue;
            }
            let d = self.cpu_distance(f, o);
            match closest {
                None => closest = Some((o, d)),
                Some((_, best)) if best > d => closest = Some((o, d)),
                _ => {}
            }
        }
        closest.map(|(o, _)| o)
    }

    /// Leaves the respawn platform, `ftCo_800A08F0`: a crouch's flick down, after a wait
    /// by level (none for Nana).
    #[doc(alias = "ftCo_800A08F0")]
    fn cpu_leave_platform(&mut self, f: FighterId) {
        self.cpu_neutral_stick(f);
        let fpp = &self.game.fighters[f.0];
        // `ftCo_MS_SquatRv`, `ftCo_MS_Squat`, `ftCo_MS_SquatWait`.
        if matches!(fpp.motion_id, 39..=41) {
            self.cpu_cmd1(f, cmd::WAIT_FOR, 1);
            self.cpu_cmd(f, cmd::DONE);
            return;
        }
        if fpp.kind != super::popo::KIND_NANA {
            let wait = match fpp.cpu.level {
                0 => Some(45),
                1 | 2 => Some(30),
                3 | 4 => Some(15),
                5 | 6 => Some(10),
                7 | 8 => Some(5),
                9 => Some(1),
                _ => None,
            };
            if let Some(w) = wait {
                self.cpu_cmd1(f, cmd::WAIT_FOR, w);
            }
        } else {
            self.cpu_cmd1(f, cmd::WAIT_FOR, 1);
        }
        self.cpu_cmd1(f, cmd::SET_LSTICK_Y, 0x81);
        self.cpu_cmd1(f, cmd::WAIT_FOR, 5);
        self.cpu_neutral_stick(f);
        self.cpu_cmd1(f, cmd::WAIT_FOR, 1);
        self.cpu_cmd(f, cmd::DONE);
    }

    /// Walking toward the destination, `ftCo_800ABA34` (behavior 1): kinds that taunt or
    /// wait do so; Zelda and Sheik transform on their coin flip; off the respawn platform
    /// once no enemy is entering; else moving in the air or on the ground.
    // Ports `ftCo_800ABA34`; aliased once a check reaches it.
    pub(super) fn cpu_walk(&mut self, f: FighterId) {
        let fpp = &self.game.fighters[f.0];
        match fpp.cpu.kind {
            11 => {
                if fpp.item_gobj.is_none() {
                    self.cpu_data_script(f, 38);
                    return;
                }
            }
            12 => {
                if fpp.cpu.x7C % 30 == 0 {
                    self.cpu_data_script(f, 38);
                    return;
                }
            }
            16 if fpp.ground_or_air == GA_GROUND && fpp.cpu.x7C % 0x12C == 0 => {
                self.cpu_jump_toward_target(f);
                return;
            }
            _ => {}
        }
        let fpp = &self.game.fighters[f.0];
        // Ft_Kind_Zelda, Ft_Kind_Seak; `ftCo_MS_RebirthWait`.
        if matches!(fpp.kind, 19 | 7) && fpp.cpu.xF8_b5 {
            self.cpu_data_script(f, 38);
            self.game.fighters[f.0].cpu.xF8_b5 = false;
        } else if self.cpu_entering_enemy(f).is_none() && self.game.fighters[f.0].motion_id == 13 {
            self.cpu_leave_platform(f);
        } else if self.game.fighters[f.0].ground_or_air == GA_AIR {
            self.cpu_air_move(f);
        } else {
            self.cpu_ground_move(f);
        }
    }

    /// Whether the fighter falls toward a destination above its landing spot, `ftCo_800A3234`:
    /// in the air falling (not passing through a platform, nor a big Yoshi early on), the
    /// destination not steeply below, touching a wall it faces, or its fall from last frame
    /// crossing no floor with the destination above its feet. Under Corneria's ceiling,
    /// whenever it barely rises.
    // Ports `ftCo_800A3234`; aliased once a check reaches it.
    fn cpu_falling_short(&mut self, f: FighterId) -> bool {
        let fpp = &self.game.fighters[f.0];
        let c = &fpp.cpu;
        if c.xFA_b6 {
            return fpp.pos_delta.y < 0.1;
        }
        if fpp.ground_or_air != GA_AIR || fpp.pos_delta.y > 0.0 || fpp.motion_id == 244 {
            return false;
        }
        // Ft_Kind_Yoshi.
        if fpp.kind == 14 && fpp.x34_scale.y > 1.0 && c.x7C < 0x384 {
            return false;
        }
        let dy = fp::fsubs(reg(c.x54.y), reg(fpp.cur_pos.y));
        let dx = abs(fp::fsubs(reg(c.x54.x), reg(fpp.cur_pos.x)));
        if super::co::guard::stick_angle(dy, dx) < fp::fneg(1.308_996_915_817_260_7) {
            return false;
        }
        let env = fpp.coll_data.env_flags;
        let wall = if f64::from(fpp.facing_dir) > 0.0 {
            env & 0x3F != 0
        } else {
            env & 0xFC0 != 0
        };
        if wall {
            return true;
        }
        let coll = &fpp.coll_data;
        let (bx, by) = (reg(coll.ecb.bottom.x), reg(coll.ecb.bottom.y));
        let feet = fp::fadds(reg(coll.last_pos.y), by);
        let ax = mem(fp::fadds(reg(coll.last_pos.x), bx));
        let ay = mem(fp::fadds(reg(coll.last_pos.y), by));
        let ex = mem(fp::fadds(reg(coll.cur_pos.x), bx));
        let ey = mem(fp::fadds(reg(coll.cur_pos.y), by));
        let dest_y = reg(c.x54.y);
        // `ftCo_800A0FB0`.
        if self.cpu_floor(ax, ay, ex, ey).is_some() {
            return false;
        }
        dest_y > feet
    }

    /// A recovery toward the destination by character, `ftCo_800A96B8`: by default (and for
    /// Mario and Nana) the up special up and toward it with full drift in; Fox's and Falco's
    /// diagonally, Luigi's side special, nothing for Yoshi, Ness's PK Thunder once below the
    /// destination, and Pikachu's, Pichu's, Zelda's and Samus's own.
    #[doc(alias = "ftCo_800A96B8")]
    #[doc(alias = "ftCo_800A963C")]
    pub(super) fn cpu_recover(&mut self, f: FighterId) {
        let fpp = &self.game.fighters[f.0];
        match fpp.kind {
            // Ft_Kind_Pikachu, Ft_Kind_Pichu.
            12 | 23 => self.cpu_recover_pikachu(f),
            // Ft_Kind_Fox, Ft_Kind_Falco.
            1 | 22 => {
                self.cpu_cmd1(f, cmd::SET_LSTICK_Y, 88);
                self.cpu_cmd1(f, cmd::LSTICK_X_TOWARD_DESTINATION, 88);
                self.cpu_cmd1(f, cmd::PRESS_B_FOR, 1);
                self.cpu_cmd(f, cmd::RELEASE_B);
                self.cpu_cmd1(f, cmd::WAIT_FOR, 40);
                self.cpu_cmd(f, cmd::DONE);
            }
            // Ft_Kind_Yoshi.
            14 => self.cpu_cmd(f, cmd::DONE),
            // Ft_Kind_Ness.
            8 => {
                let above = fp::fsub(reg(fpp.cpu.x54.y), 10.0);
                if reg(fpp.cur_pos.y) > above {
                    self.cpu_cmd(f, cmd::DONE);
                } else {
                    self.cpu_recover_ness(f);
                }
            }
            // Ft_Kind_Luigi.
            17 => {
                self.cpu_cmd1(f, cmd::SET_LSTICK_Y, 0);
                self.cpu_cmd1(f, cmd::LSTICK_X_TOWARD_DESTINATION, 127);
                self.cpu_cmd1(f, cmd::PRESS_B_FOR, 1);
                self.cpu_cmd(f, cmd::RELEASE_B);
                self.cpu_cmd1(f, cmd::WAIT_FOR, 40);
                self.cpu_cmd(f, cmd::DONE);
            }
            // Ft_Kind_Zelda.
            19 => self.cpu_recover_zelda(f),
            // Ft_Kind_Samus, `ftCo_800A963C`, then the default's own commands.
            _ => {
                // `ftCo_CpuRecoverDiagonally`.
                self.cpu_cmd1(f, cmd::SET_LSTICK_Y, 88);
                self.cpu_cmd1(f, cmd::LSTICK_X_TOWARD_DESTINATION, 88);
                self.cpu_cmd1(f, cmd::PRESS_B_FOR, 1);
                self.cpu_cmd(f, cmd::RELEASE_B);
                self.cpu_cmd1(f, cmd::LSTICK_X_TOWARD_DESTINATION, 127);
                self.cpu_cmd(f, cmd::DONE);
            }
        }
    }

    /// `ftCo_CpuStartUpSpecial`.
    fn cpu_start_up_special(&mut self, f: FighterId) {
        self.cpu_cmd1(f, cmd::SET_LSTICK_Y, 0x7F);
        self.cpu_cmd1(f, cmd::SET_LSTICK_X, 0);
        self.cpu_cmd1(f, cmd::PRESS_B_FOR, 1);
        self.cpu_cmd(f, cmd::RELEASE_B);
    }

    /// Ness's recovery, `ftCo_800A8EB0`: at level 9 only, PK Thunder steered around and back
    /// into him from below (away, down, toward, up, each held 20 frames but the 35 down).
    #[doc(alias = "ftCo_800A8EB0")]
    fn cpu_recover_ness(&mut self, f: FighterId) {
        let fpp = &self.game.fighters[f.0];
        if fpp.cpu.level < 9 {
            self.cpu_cmd(f, cmd::DONE);
            return;
        }
        let toward = fp::fsubs(reg(fpp.cpu.x54.x), reg(fpp.cur_pos.x)) > 0.0;
        self.cpu_start_up_special(f);
        let pi = mem(fp::frsp(std::f64::consts::PI));
        let half = mem(fp::frsp(fp::fdiv(std::f64::consts::PI, 2.0)));
        let down = mem(fp::frsp(fp::fneg(fp::fdiv(std::f64::consts::PI, 2.0))));
        let (first, third) = if toward { (pi, 0.0) } else { (0.0, pi) };
        for (angle, frames) in [(first, 20), (down, 35), (third, 20), (half, 20)] {
            // `ftCo_CpuAimPkThunder`.
            let x = fp::fctiwz(fp::fmuls(127.0, reg(math::cos(angle))));
            let y = fp::fctiwz(fp::fmuls(127.0, reg(math::sin(angle))));
            self.cpu_cmd1(f, cmd::SET_LSTICK_X, x as u8);
            self.cpu_cmd1(f, cmd::SET_LSTICK_Y, y as u8);
            self.cpu_cmd1(f, cmd::WAIT_FOR, frames);
        }
        self.cpu_cmd(f, cmd::DONE);
    }

    /// Pikachu's and Pichu's recovery, `ftCo_800A92CC`: far out (60), Quick Attack up and
    /// toward the destination, then across; else straight up, then across or diagonally by a
    /// coin flip.
    #[doc(alias = "ftCo_800A92CC")]
    fn cpu_recover_pikachu(&mut self, f: FighterId) {
        let fpp = &self.game.fighters[f.0];
        let far = abs(fp::fsubs(reg(fpp.cur_pos.x), reg(fpp.cpu.x54.x))) > 60.0;
        self.cpu_start_up_special(f);
        if far {
            self.cpu_cmd1(f, cmd::SET_LSTICK_Y, 88);
            self.cpu_cmd1(f, cmd::LSTICK_X_TOWARD_DESTINATION, 88);
            self.cpu_cmd1(f, cmd::WAIT_FOR, 15);
            self.cpu_cmd1(f, cmd::LSTICK_X_TOWARD_DESTINATION, 127);
            self.cpu_cmd1(f, cmd::SET_LSTICK_Y, 0);
        } else {
            self.cpu_cmd1(f, cmd::WAIT_FOR, 15);
            if reg(self.game.objs.rng.draw_f32()) > 0.5 {
                self.cpu_cmd1(f, cmd::LSTICK_X_TOWARD_DESTINATION, 88);
                self.cpu_cmd1(f, cmd::SET_LSTICK_Y, 88);
            } else {
                self.cpu_cmd1(f, cmd::LSTICK_X_TOWARD_DESTINATION, 127);
                self.cpu_cmd1(f, cmd::SET_LSTICK_Y, 0);
            }
        }
        self.cpu_cmd1(f, cmd::WAIT_FOR, 15);
        self.cpu_cmd(f, cmd::DONE);
    }

    /// Zelda's recovery, `ftCo_800A949C`: Farore's Wind held 40 frames, toward the
    /// destination when it's over 30 across, else straight up.
    #[doc(alias = "ftCo_800A949C")]
    fn cpu_recover_zelda(&mut self, f: FighterId) {
        let fpp = &self.game.fighters[f.0];
        let far = abs(fp::fsubs(reg(fpp.cpu.x54.x), reg(fpp.cur_pos.x))) > 30.0;
        self.cpu_neutral_stick(f);
        self.cpu_cmd1(f, cmd::WAIT_FOR, 1);
        self.cpu_cmd1(f, cmd::SET_LSTICK_Y, 0x7F);
        if far {
            self.cpu_cmd1(f, cmd::LSTICK_X_TOWARD_DESTINATION, 0x7F);
            self.cpu_cmd1(f, cmd::PRESS_B_FOR, 40);
            self.cpu_cmd1(f, cmd::SET_LSTICK_Y, 0);
            self.cpu_cmd1(f, cmd::SET_LSTICK_X, 0);
        } else {
            self.cpu_cmd1(f, cmd::SET_LSTICK_X, 0);
            self.cpu_cmd1(f, cmd::PRESS_B_FOR, 40);
            self.cpu_cmd1(f, cmd::SET_LSTICK_Y, 0);
        }
        self.cpu_cmd(f, cmd::RELEASE_B);
        self.cpu_cmd1(f, cmd::WAIT_FOR, 1);
        self.cpu_cmd(f, cmd::DONE);
    }

    /// Moving in the air, `ftCo_800A9CB4`: falling short, it jumps while it has jumps, air-
    /// dodges on the Temple and Fourside, and recovers when small or on Icicle Mountain and
    /// Rainbow Cruise; under a ceiling it drifts back; else it steers toward the destination
    /// by where its fall (under gravity to terminal speed) will bring it when level with it.
    // Ports `ftCo_800A9CB4`; aliased once a check reaches it.
    fn cpu_air_move(&mut self, f: FighterId) {
        if self.game.fighters[f.0].x2168 != 0 {
            self.cpu_finish_neutral(f);
            return;
        }
        if self.cpu_falling_short(f) {
            let fpp = &self.game.fighters[f.0];
            if fpp.co_attrs.max_jumps > i32::from(fpp.x1968_jumpsUsed) {
                // `ftCo_CpuUseAvailableJump`.
                let c = &fpp.cpu;
                if c.xFA_b6 {
                    self.cpu_cmd1(f, cmd::SET_LSTICK_Y, 0);
                    self.cpu_cmd1(f, cmd::LSTICK_X_FORWARD, 127);
                    self.cpu_cmd(f, cmd::PRESS_Y);
                    self.cpu_cmd1(f, cmd::WAIT_FOR, 1);
                    self.cpu_cmd(f, cmd::RELEASE_Y);
                    self.cpu_cmd(f, cmd::DONE);
                } else if fpp.kind == 14
                    && fpp.x34_scale.y < 1.0
                    && abs(fp::fsubs(reg(c.x54.x), reg(fpp.cur_pos.x))) < 20.0
                {
                    unported_call("ftCo_800A0508");
                } else {
                    self.cpu_cmd1(f, cmd::LSTICK_TOWARD_DESTINATION, 127);
                    self.cpu_cmd(f, cmd::PRESS_Y);
                    self.cpu_cmd1(f, cmd::WAIT_FOR, 1);
                    self.cpu_cmd(f, cmd::RELEASE_Y);
                    self.cpu_cmd(f, cmd::DONE);
                }
                return;
            }
            // `ftCo_800A9CB4_inline0`: Gr_Kind_Shrine's gap, Gr_Kind_Fourside.
            let x = self.game.fighters[f.0].cur_pos.x;
            let dodge = match self.game.stage_info.grkind {
                7 if x > -150.0 && x < -90.0 => 1,
                21 => 2,
                _ => 0,
            };
            if dodge != 0 {
                self.cpu_neutral_stick(f);
                self.cpu_cmd1(f, cmd::WAIT_FOR, 1);
                if dodge == 2 {
                    self.cpu_cmd1(f, cmd::LSTICK_X_FORWARD, 80);
                }
                self.cpu_cmd1(f, cmd::SET_LSTICK_Y, 127);
                self.cpu_tap_r(f);
                self.cpu_finish_neutral_y(f);
                return;
            }
            // `ftCo_800A9CB4_inline1`: Gr_Kind_Icemt, Gr_Kind_RCruise.
            let fpp = &self.game.fighters[f.0];
            let recover = if fpp.cpu.xFA_b6 {
                false
            } else if fpp.x34_scale.y < 1.0 {
                true
            } else {
                match self.game.stage_info.grkind {
                    22 => unported_call("ftCo_800A9CB4 on Icicle Mountain (grLib_801C9E60)"),
                    3 => true,
                    _ => false,
                }
            };
            if recover {
                self.cpu_recover(f);
                return;
            }
        }
        let fpp = &self.game.fighters[f.0];
        if fpp.cpu.xFA_b6 {
            self.cpu_cmd1(f, cmd::SET_LSTICK_X, 129);
            self.cpu_finish_neutral_y(f);
            return;
        }
        let c = &fpp.cpu;
        let dx = fp::fsubs(reg(c.x54.x), reg(fpp.cur_pos.x));
        let dx_abs = abs(dx);
        let vx = reg(fpp.pos_delta.x);
        let x_time = if !nearly_zero(vx) {
            fp::fdivs(dx, vx)
        } else if dx_abs > 0.0 {
            fp::fneg(1.0)
        } else {
            0.0
        };
        let gravity = reg(fpp.co_attrs.gravity);
        let term = reg(fpp.co_attrs.terminal_velocity);
        let vy = reg(fpp.pos_delta.y);
        let y = reg(fpp.cur_pos.y);
        let terminal_time = if nearly_zero(gravity) {
            1000.0
        } else {
            fp::fdivs(fp::fneg(fp::fsubs(fp::fneg(term), vy)), gravity)
        };
        let y_pos = if terminal_time <= 0.0 {
            fp::fmadds(vy, x_time, y)
        } else if x_time < terminal_time {
            let s = math::sqrt_reg(x_time);
            fp::frsp(fp::fadd(
                y,
                fp::fnmsub(0.5, fp::fmuls(gravity, s), fp::fmuls(vy, x_time)),
            ))
        } else {
            let s = math::sqrt_reg(terminal_time);
            let over = fp::fsubs(x_time, terminal_time);
            fp::frsp(fp::fadd(
                y,
                fp::fsub(
                    fp::fnmsub(0.5, fp::fmuls(gravity, s), fp::fmuls(vy, terminal_time)),
                    fp::fmuls(over, term),
                ),
            ))
        };
        let dest_y = reg(c.x54.y);
        if x_time < 0.0 {
            self.cpu_cmd1(f, cmd::LSTICK_X_TOWARD_DESTINATION, 127);
        } else if y_pos < dest_y {
            let mut clamp = fp::fctiwz(fp::fsubs(dest_y, y_pos)).wrapping_mul(2);
            if clamp > 32 {
                clamp = 32;
            }
            self.cpu_cmd2(
                f,
                cmd::LSTICK_X_TOWARD_DESTINATION_CLAMPED,
                clamp as u8,
                127,
            );
        } else {
            self.cpu_cmd1(f, cmd::SET_LSTICK_X, 0);
        }
        self.cpu_finish_neutral_y(f);
    }

    /// Whether the fighter stands within `5 * scale` of the edge it faces, `ftCo_800A28D0`.
    #[doc(alias = "ftCo_800A28D0")]
    pub(super) fn cpu_near_edge(&self, f: FighterId, scale: f64) -> bool {
        let fpp = &self.game.fighters[f.0];
        if fpp.ground_or_air == GA_AIR {
            return false;
        }
        let geo = self.game.geometry.as_ref().expect("no collision geometry");
        let Some(island) = geo.island_of_line(fpp.coll_data.floor.index) else {
            return false;
        };
        let isl = geo.islands.get(island);
        let edge = if f64::from(fpp.facing_dir) > 0.0 {
            isl.right.x
        } else {
            isl.left.x
        };
        abs(fp::fsubs(reg(edge), reg(fpp.cur_pos.x))) < fp::fmul(5.0, scale)
    }

    /// Whether the fighter faces its destination, `ftCo_800A2BD4`.
    // Ports `ftCo_800A2BD4`; aliased once a check reaches it.
    pub(super) fn cpu_facing_destination(&self, f: FighterId) -> bool {
        let fpp = &self.game.fighters[f.0];
        fp::fmuls(
            reg(fpp.facing_dir),
            fp::fsubs(reg(fpp.cpu.x54.x), reg(fpp.cur_pos.x)),
        ) >= 0.0
    }

    /// Moving on the ground, `ftCo_800AB224`: under Corneria's ceiling it turns or heads for
    /// the edge; with an approach allowed (`xF9_b1`) it may close in on its target; then, if
    /// its destination is on its island, it walks or runs there; else it heads off toward it
    /// (jumping, dropping or turning by the angle and what's in between).
    // Ports `ftCo_800AB224`; aliased once a check reaches it.
    fn cpu_ground_move(&mut self, f: FighterId) {
        let fpp = &self.game.fighters[f.0];
        if fpp.cpu.xFA_b6 {
            if f64::from(fpp.facing_dir) > 0.0 {
                self.cpu_turn_around(f);
            } else if !self.cpu_near_edge(f, 1.0) {
                self.cpu_cmd1(f, cmd::SET_LSTICK_Y, 0);
                self.cpu_cmd1(f, cmd::SET_LSTICK_X, 176);
                self.cpu_cmd1(f, cmd::WAIT_FOR, 1);
                self.cpu_cmd(f, cmd::DONE);
            } else {
                unported_call("ftCo_800A0384");
            }
            return;
        }
        if fpp.cpu.xF9_b1 {
            unported_call("ftCo_800AB224 with an approach (xF9_b1)");
        }
        // `ftCo_800A21FC`: the destination on the fighter's island.
        let fpp = &self.game.fighters[f.0];
        let on_island = if fpp.ground_or_air == GA_AIR {
            false
        } else {
            let line = fpp.coll_data.floor.index;
            let dest = crate::vec::Vec3::new(
                fpp.cpu.x54.x,
                mem(fp::frsp(fp::fadd(5.0, reg(fpp.cpu.x54.y)))),
                0.0,
            );
            let geo = self.game.geometry.as_mut().expect("no collision geometry");
            match geo.island_of_line(line) {
                None => false,
                Some(i) => geo.island_below(dest, -10.0) == Some(i),
            }
        };
        if on_island {
            self.cpu_walk_to(f);
            return;
        }
        let fpp = &self.game.fighters[f.0];
        let c = &fpp.cpu;
        let dx = abs(fp::fsubs(reg(c.x54.x), reg(fpp.cur_pos.x)));
        let dy = fp::fsubs(reg(c.x54.y), reg(fpp.cur_pos.y));
        let angle = if nearly_zero(dx) {
            if dy > 0.0 {
                fp::frsp(fp::fdiv(std::f64::consts::PI, 2.0))
            } else {
                fp::frsp(fp::fneg(fp::fdiv(std::f64::consts::PI, 2.0)))
            }
        } else {
            super::co::guard::stick_angle(dy, dx)
        };
        if angle > 0.6108652334660292 {
            // Steeply up (over 35 degrees): under a ceiling it turns at the edge, jumps toward
            // Hyrule Temple's openings or walks on; on Icicle Mountain, or facing the
            // destination, it jumps; else it turns back unless nearly straight up.
            let fpp = &self.game.fighters[f.0];
            let (cx, cy, dest_y) = (
                fpp.coll_data.cur_pos.x,
                fpp.coll_data.cur_pos.y,
                fpp.cpu.x54.y,
            );
            let ceiling = self
                .game
                .geometry
                .as_mut()
                .expect("no collision geometry")
                .check_ceiling(cx, cy, cx, dest_y, -1, -1)
                .is_some();
            if ceiling {
                if self.cpu_near_edge(f, 1.0) {
                    self.cpu_turn_around(f);
                } else if self.cpu_shrine_opening(f) {
                    self.cpu_jump_off_piece(f);
                } else {
                    self.cpu_walk_forward(f);
                }
            } else if self.game.stage_info.grkind == GR_KIND_ICEMT {
                self.cpu_jump_off_piece(f);
            } else if !self.cpu_facing_destination(f) {
                if angle < 1.3089969288557768 {
                    self.cpu_turn_around(f);
                } else {
                    self.cpu_walk_forward(f);
                }
            } else {
                self.cpu_jump_off_piece(f);
            }
        } else if angle > fp::fneg(0.7853981573134661) {
            if !self.cpu_facing_destination(f) {
                self.cpu_turn_around(f);
            } else {
                self.cpu_head_off_level(f);
            }
        } else {
            self.cpu_head_off_down(f);
        }
    }

    /// The stick toward the fighter's facing at its walk speed (`ftCo_800AA320`'s step and
    /// clamp), done.
    fn cpu_walk_forward_cmds(&mut self, f: FighterId) {
        let (stick, clamp) = self.cpu_walk_speed(f);
        self.cpu_cmd1(f, cmd::SET_LSTICK_Y, 0);
        self.cpu_cmd2(f, cmd::LSTICK_FORWARD_CLAMPED, stick as u8, clamp as u8);
        self.cpu_cmd(f, cmd::DONE);
    }

    /// Walks on the way it faces, `ftCo_800AABC8`: off a teeter first (forward, as
    /// `ftCo_CpuTurnAwayFromLedge` does).
    // Ports `ftCo_800AABC8`; aliased once a check reaches it.
    fn cpu_walk_forward(&mut self, f: FighterId) {
        if matches!(self.game.fighters[f.0].motion_id, 245 | 246) {
            self.cpu_turn_away_from_ledge(f);
        } else {
            self.cpu_walk_forward_cmds(f);
        }
    }

    /// Whether the fighter meets a wall (`inlineC0`: any left or right wall flag).
    fn cpu_against_wall(&self, f: FighterId) -> bool {
        self.game.fighters[f.0].coll_data.env_flags & 0xFFF != 0
    }

    /// Whether the fighter stands on a platform (`LINE_FLAG_PLATFORM` on its floor).
    fn cpu_on_platform(&self, f: FighterId) -> bool {
        let line = self.game.fighters[f.0].coll_data.floor.index;
        line != -1
            && self
                .game
                .geometry
                .as_ref()
                .expect("no collision geometry")
                .line_surface(line)
                & LINE_FLAG_PLATFORM
                != 0
    }

    /// Heading off the island level or slightly up or down, `ftCo_800AA844`: off a teeter
    /// first; at the edge, through a platform when `x2168` says and there's floor below
    /// (`ftCo_800A0FB0`), else a jump unless the destination is below and near and no wall's
    /// in the way, when it walks off; away from the edge a jump at a wall, else walking on.
    #[doc(alias = "ftCo_800AA844")]
    fn cpu_head_off_level(&mut self, f: FighterId) {
        if matches!(self.game.fighters[f.0].motion_id, 245 | 246) {
            self.cpu_turn_away_from_ledge(f);
            return;
        }
        if self.cpu_near_edge(f, 1.0) {
            let drop = self.game.fighters[f.0].x2168 != 0 && self.cpu_on_platform(f) && {
                let pos = self.game.fighters[f.0].cur_pos;
                let ay = mem(fp::fsubs(reg(pos.y), 10.0));
                let by = mem(fp::fsubs(reg(pos.y), 1000.0));
                let hit = self
                    .game
                    .geometry
                    .as_mut()
                    .expect("no collision geometry")
                    .check_floor(pos.x, ay, pos.x, by, 0.0, -1, -1, -1, None);
                hit.is_some_and(|h| !self.excluded_floor(h.line))
            };
            let fpp = &self.game.fighters[f.0];
            if drop {
                self.cpu_leave_platform(f);
            } else if fpp.cpu.x54.y > fpp.cur_pos.y
                || abs(fp::fsubs(reg(fpp.cpu.x54.x), reg(fpp.cur_pos.x))) > 40.0
                || self.cpu_against_wall(f)
            {
                self.cpu_jump_off_piece(f);
            } else {
                self.cpu_walk_forward_cmds(f);
            }
        } else if self.cpu_against_wall(f) {
            self.cpu_jump_off_piece(f);
        } else {
            self.cpu_walk_forward_cmds(f);
        }
    }

    /// Heading off the island steeply down, `ftCo_800AACD0`: off a teeter first; through a
    /// platform; turning back when the destination is past the island's ends the other way;
    /// a jump at a wall (but on Icicle Mountain); else walking on.
    #[doc(alias = "ftCo_800AACD0")]
    fn cpu_head_off_down(&mut self, f: FighterId) {
        if matches!(self.game.fighters[f.0].motion_id, 245 | 246) {
            self.cpu_turn_away_from_ledge(f);
            return;
        }
        if self.cpu_on_platform(f) {
            self.cpu_leave_platform(f);
            return;
        }
        let line = self.game.fighters[f.0].coll_data.floor.index;
        let geo = self.game.geometry.as_ref().expect("no collision geometry");
        if let Some(island) = geo.island_of_line(line) {
            let (mut l, mut r) = (crate::vec::Vec3::default(), crate::vec::Vec3::default());
            geo.island_ends(island, Some(&mut l), Some(&mut r));
            let x = self.game.fighters[f.0].cpu.x54.x;
            if (x < l.x || x > r.x) && !self.cpu_facing_destination(f) {
                self.cpu_turn_around(f);
                return;
            }
        }
        if self.game.stage_info.grkind != GR_KIND_ICEMT && self.cpu_against_wall(f) {
            self.cpu_jump_off_piece(f);
            return;
        }
        self.cpu_walk_forward_cmds(f);
    }

    /// Whether a computer player on Hyrule Temple heads for an island's end above it and ahead,
    /// `ftCo_800AAF48`: the first, in the islands' order, with no ceiling or wall between them
    /// and near enough (less than twice its jump's height plus its height above, five times its
    /// half width ahead), for 300 frames; never while its destination's timer runs (`x60`) or
    /// on another stage.
    #[doc(alias = "ftCo_800AAF48")]
    fn cpu_shrine_opening(&mut self, f: FighterId) -> bool {
        if self.game.fighters[f.0].cpu.x60 != 0 || self.game.stage_info.grkind != GR_KIND_SHRINE {
            return false;
        }
        let geo = self.game.geometry.as_mut().expect("no collision geometry");
        let islands: Vec<_> = geo.islands.iter(geo.islands.floors).collect();
        for island in islands {
            let fpp = &self.game.fighters[f.0];
            let (pos, right) = (fpp.cur_pos, fpp.facing_dir > 0.0);
            let geo = self.game.geometry.as_mut().expect("no collision geometry");
            let isl = *geo.islands.get(island);
            // Facing right, an island's left end ahead; facing left, its right end.
            let (end, dx, dy) = if right {
                let e = isl.left;
                (
                    e,
                    fp::fsubs(reg(e.x), reg(pos.x)),
                    fp::fsubs(reg(e.y), reg(pos.y)),
                )
            } else {
                let e = isl.right;
                (
                    e,
                    fp::fsubs(reg(pos.x), reg(e.x)),
                    fp::fsubs(reg(pos.y), reg(e.y)),
                )
            };
            if dx < 0.0 || dy < 0.0 {
                continue;
            }
            if geo
                .check_ceiling(pos.x, pos.y, end.x, end.y, -1, -1)
                .is_some()
            {
                continue;
            }
            let wall = if right {
                geo.check_right_wall(pos.x, pos.y, end.x, end.y, -1, -1)
            } else {
                geo.check_left_wall(pos.x, pos.y, end.x, end.y, -1, -1)
            };
            if wall.is_some() {
                continue;
            }
            let c = &mut self.game.fighters[f.0].cpu;
            if dy < fp::fmadds(2.0, reg(c.x558), reg(c.x568)) && dx < fp::fmuls(5.0, reg(c.x564)) {
                c.x60 = 300;
                c.x64.x = c.x54.x;
                c.x64.y = c.x54.y;
                c.x54.x = end.x;
                c.x54.y = end.y;
                c.x38 = 5.0;
                return true;
            }
        }
        false
    }

    /// Walks or runs to a destination on the fighter's island, `ftCo_800AA42C`: off a teeter
    /// first, turning to face it, at full speed while far (running if it runs, else a dash
    /// start), slowing in on it, standing once within reach.
    // Ports `ftCo_800AA42C`; aliased once a check reaches it.
    fn cpu_walk_to(&mut self, f: FighterId) {
        let (stick, clamp) = self.cpu_walk_speed(f);
        let fpp = &self.game.fighters[f.0];
        // `isInTeeter`: `ftCo_MS_Ottotto`, `ftCo_MS_OttottoWait`.
        if matches!(fpp.motion_id, 245 | 246) {
            self.cpu_turn_away_from_ledge(f);
            return;
        }
        if !self.cpu_facing_destination(f) {
            self.cpu_turn_around(f);
            return;
        }
        let fpp = &self.game.fighters[f.0];
        let c = &fpp.cpu;
        let dx = fp::fsubs(reg(c.x54.x), reg(fpp.cur_pos.x));
        let dy = fp::fsubs(reg(c.x54.y), reg(fpp.cur_pos.y));
        let dist = math::sqrt_reg(fp::fmadds(dx, dx, fp::fmuls(dy, dy)));
        let near = reg(c.x38);
        let far = reg(c.x3C);
        if fp::fsubs(dist, near) > far {
            self.cpu_cmd1(f, cmd::SET_LSTICK_Y, 0);
            let fpp = &self.game.fighters[f.0];
            if fpp.cpu.xF8_b0 {
                // `ftCo_MS_Dash`, `ftCo_MS_Run`, `ftCo_MS_TurnRun`.
                if matches!(fpp.motion_id, 19..=21) {
                    self.cpu_cmd1(f, cmd::LSTICK_X_TOWARD_DESTINATION, 127);
                } else {
                    self.cpu_cmd1(f, cmd::LSTICK_X_TOWARD_DESTINATION, 0);
                    self.cpu_cmd1(f, cmd::WAIT_FOR, 2);
                    self.cpu_cmd1(f, cmd::LSTICK_X_TOWARD_DESTINATION, 127);
                }
            } else {
                self.cpu_cmd2(
                    f,
                    cmd::LSTICK_X_TOWARD_DESTINATION_CLAMPED,
                    stick as u8,
                    clamp as u8,
                );
            }
        } else if dist > near {
            self.cpu_cmd1(f, cmd::SET_LSTICK_Y, 0);
            // `ftCo_800AA42C_inline0`: the clamp eased from the walk's slowest (at least 80,
            // the deadzone's push) at `near` to full at `far`.
            let clampf = fp::frsp(f64::from(clamp));
            let c = &self.game.fighters[f.0].cpu;
            let (near, far) = (reg(c.x38), reg(c.x3C));
            let eased = if dist > far {
                fp::fctiwz(clampf)
            } else if dist < near {
                0
            } else {
                let mut raw = fp::fmuls(127.0, reg(self.common().f32(0)));
                if raw > clampf {
                    raw = clampf;
                }
                if raw < 80.0 {
                    raw = 80.0;
                }
                let scale = fp::fdivs(fp::fsubs(dist, near), fp::fsubs(far, near));
                let mut n = fp::fctiwz(fp::fmadds(scale, fp::fsubs(clampf, raw), raw));
                if fp::frsp(f64::from(n)) > clampf {
                    n = fp::fctiwz(clampf);
                }
                n
            };
            self.cpu_cmd2(
                f,
                cmd::LSTICK_X_TOWARD_DESTINATION_CLAMPED,
                stick as u8,
                eased as u8,
            );
        } else {
            self.cpu_neutral_stick(f);
        }
        self.cpu_cmd(f, cmd::DONE);
    }

    /// Whether the fighter faces its target, or stands within 1 of it, or has none,
    /// `ftCo_800A2C08`.
    // Ports `ftCo_800A2C08`; aliased once a check reaches it.
    pub(super) fn cpu_target_ahead(&self, f: FighterId) -> bool {
        let fpp = &self.game.fighters[f.0];
        let Some(t) = fpp.cpu.x44 else { return true };
        let dx = fp::fsubs(reg(self.game.fighters[t.0].cur_pos.x), reg(fpp.cur_pos.x));
        if abs(dx) < 1.0 {
            return true;
        }
        fp::fmuls(dx, reg(fpp.facing_dir)) >= 0.0
    }

    /// Mashing out of a hold (behavior 16), `ftCo_800AC30C`: once out of the held states
    /// (`inlineG0`: 224, 227, 266 to 270) back to the usual behavior; else every third frame
    /// (`x7C`), with a chance by level, the stick flips side to side.
    #[doc(alias = "ftCo_800AC30C")]
    pub(super) fn cpu_mash(&mut self, f: FighterId) {
        let ms = self.game.fighters[f.0].motion_id;
        if !(ms == 224 || ms == 227 || (266..=270).contains(&ms)) {
            let c = &mut self.game.fighters[f.0].cpu;
            c.x18 = c.x1C;
            self.cpu_cmd(f, cmd::DONE);
            return;
        }
        let c = &self.game.fighters[f.0].cpu;
        if c.x7C % 3 != 0 {
            return;
        }
        let chance = fp::fmuls(reg(0.1), fp::frsp(f64::from(c.level)));
        if chance < reg(self.game.objs.rng.draw_f32()) {
            return;
        }
        let x = if self.game.fighters[f.0].cpu.lstick.x < 0 {
            127
        } else {
            -127i8 as u8
        };
        self.cpu_cmd1(f, cmd::SET_LSTICK_X, x);
        self.cpu_cmd(f, cmd::DONE);
    }

    /// `ftCo_CpuHoldUpForOneFrame`.
    fn cpu_hold_up(&mut self, f: FighterId) {
        self.cpu_cmd1(f, cmd::SET_LSTICK_X, 0);
        self.cpu_cmd1(f, cmd::SET_LSTICK_Y, 127);
        self.cpu_cmd1(f, cmd::WAIT_FOR, 1);
        self.cpu_finish_neutral_y(f);
    }

    /// `ftCo_CpuRetapA`.
    pub(super) fn cpu_retap_a(&mut self, f: FighterId) {
        self.cpu_cmd(f, cmd::RELEASE_A);
        self.cpu_cmd1(f, cmd::WAIT_FOR, 1);
        self.cpu_cmd(f, cmd::PRESS_A);
        self.cpu_cmd1(f, cmd::WAIT_FOR, 1);
        self.cpu_cmd(f, cmd::RELEASE_A);
        self.cpu_cmd(f, cmd::DONE);
    }

    /// Getting up, `ftCo_800AC7D4` (behavior 5): not lying, back to the usual behavior;
    /// face up, stick neutral; face down, a computer of kind 0 or 15 or with no target
    /// near gets up, else above a chance by level it attacks getting up, else rolls away
    /// from the target.
    // Ports `ftCo_800AC7D4`; aliased once a check reaches it.
    pub(super) fn cpu_get_up(&mut self, f: FighterId) {
        match self.cpu_lying(f) {
            0 => {
                self.cpu_back_to_usual(f);
                return;
            }
            1 => {
                self.cpu_finish_neutral(f);
                return;
            }
            _ => {}
        }
        let kind = self.game.fighters[f.0].cpu.kind;
        if kind == 15 || kind == 0 {
            self.cpu_hold_up(f);
            return;
        }
        let Some(target) = self.cpu_find_target(f) else {
            self.cpu_hold_up(f);
            return;
        };
        let near = self.cpu_distance(f, target) < 15.0;
        if !near {
            self.cpu_hold_up(f);
            return;
        }
        let level = self.game.fighters[f.0].cpu.level;
        if reg(self.game.objs.rng.draw_f32()) < fp::fmuls(reg(0.1), fp::frsp(f64::from(level))) {
            self.cpu_cmd1(f, cmd::SET_LSTICK_Y, 0);
            self.cpu_cmd1(f, cmd::SET_LSTICK_X, 0);
            self.cpu_retap_a(f);
            return;
        }
        let dx = fp::fsubs(
            reg(self.game.fighters[target.0].cur_pos.x),
            reg(self.game.fighters[f.0].cur_pos.x),
        );
        self.cpu_cmd1(f, cmd::SET_LSTICK_X, if dx < 0.0 { 80 } else { 176 });
        self.cpu_cmd1(f, cmd::SET_LSTICK_Y, 0);
        self.cpu_cmd1(f, cmd::WAIT_FOR, 10);
        self.cpu_cmd1(f, cmd::SET_LSTICK_X, 0);
        self.cpu_cmd(f, cmd::DONE);
    }

    /// Tumbling, `ftCo_800ABBA8` (behavior 15): below level 5 or out of the tumble, back to
    /// the usual behavior with the stick centered and R let go. Else it heads for the island
    /// end farthest from its target ([`Running::cpu_far_island_end`]); above level 7 it techs
    /// when the floor below is under 10 frames away; with its target below and close in its
    /// up tilt or up smash, it shields, waits 50 frames and presses down; else it drifts
    /// toward its destination unless its fall lands it above it.
    #[doc(alias = "ftCo_800ABBA8")]
    pub(super) fn cpu_tumble(&mut self, f: FighterId) {
        let fpp = &self.game.fighters[f.0];
        let target = fpp.cpu.x44;
        // `ftCo_MS_DamageFall`.
        if !(fpp.cpu.level >= 5 && fpp.motion_id == 38) {
            let c = &mut self.game.fighters[f.0].cpu;
            c.x18 = c.x1C;
            self.cpu_cmd1(f, cmd::SET_LSTICK_X, 0);
            self.cpu_cmd(f, cmd::RELEASE_R);
            self.cpu_cmd(f, cmd::DONE);
            return;
        }
        if let Some(t) = target {
            let from = self.game.fighters[t.0].cur_pos;
            if let Some((x, y)) = self.cpu_far_island_end(f, from.x, from.y) {
                self.cpu_set_destination(f, x, y, 5.0);
            }
        }
        if self.game.fighters[f.0].cpu.level > 7 {
            let pos = self.game.fighters[f.0].cur_pos;
            let to = mem(fp::frsp(fp::fsub(reg(pos.y), 1000.0)));
            if let Some(floor) = self.cpu_floor(pos.x, pos.y, pos.x, to) {
                let fpp = &self.game.fighters[f.0];
                let g = fp::fneg(reg(fpp.co_attrs.gravity));
                let v = reg(fpp.pos_delta.y);
                let coll = &fpp.coll_data;
                let feet = fp::fadds(reg(coll.cur_pos.y), reg(coll.ecb.bottom.y));
                let h = fp::fsubs(reg(floor.y), feet);
                // Frames until it lands.
                let t = if !nearly_zero(g) {
                    let d = fp::fmadds(fp::fmuls(2.0, g), h, fp::fmuls(v, v));
                    let root = math::sqrt_reg(abs(d));
                    fp::fctiwz(fp::fdivs(fp::fsubs(fp::fneg(root), v), g))
                } else if !nearly_zero(v) {
                    fp::fctiwz(fp::fdivs(h, v))
                } else {
                    100
                };
                if t < 10 {
                    self.cpu_cmd1(f, cmd::SET_LSTICK_Y, 0);
                    self.cpu_cmd1(f, cmd::LSTICK_X_TOWARD_DESTINATION, 0x7F);
                    self.cpu_cmd(f, cmd::PRESS_R);
                    self.cpu_cmd(f, cmd::DONE);
                    return;
                }
            }
        }
        let Some(t) = self.cpu_find_target(f) else {
            self.cpu_back_to_usual(f);
            return;
        };
        let (me, tg) = (&self.game.fighters[f.0], &self.game.fighters[t.0]);
        // The original's compares, NaN included.
        #[allow(clippy::neg_cmp_op_on_partial_ord)]
        let shield_drop = !(tg.cur_pos.y >= me.cur_pos.y)
            && !(abs(fp::fsubs(reg(tg.cur_pos.y), reg(me.cur_pos.y)))
                >= fp::fadds(reg(tg.cpu.x568), reg(me.cpu.x568)))
            && !(abs(fp::fsubs(reg(tg.cur_pos.x), reg(me.cur_pos.x)))
                >= fp::fadd(5.0, fp::fadds(reg(tg.cpu.x564), reg(me.cpu.x564))))
            // `ftCo_MS_AttackHi3`, `ftCo_MS_AttackHi4`.
            && matches!(tg.motion_id, 56 | 63);
        if shield_drop {
            self.cpu_cmd(f, cmd::RELEASE_R);
            self.cpu_cmd1(f, cmd::WAIT_FOR, 1);
            self.cpu_cmd(f, cmd::PRESS_R);
            self.cpu_cmd1(f, cmd::WAIT_FOR, 1);
            self.cpu_neutral_stick(f);
            self.cpu_cmd(f, cmd::RELEASE_R);
            self.cpu_cmd1(f, cmd::WAIT_FOR, 0x32);
            self.cpu_cmd1(f, cmd::SET_LSTICK_Y, 0x81);
            self.cpu_cmd1(f, cmd::WAIT_FOR, 1);
            self.cpu_finish_neutral_y(f);
            return;
        }
        // Where its fall puts it once it has drifted to its destination's x.
        let fpp = &self.game.fighters[f.0];
        let vx = reg(fpp.pos_delta.x);
        let dx = fp::fsubs(reg(fpp.cpu.x54.x), reg(fpp.cur_pos.x));
        let to_dest = if nearly_zero(vx) {
            reg(1000.0)
        } else {
            fp::fdivs(dx, vx)
        };
        let g = reg(fpp.co_attrs.gravity);
        let term = reg(fpp.co_attrs.terminal_velocity);
        let (y, vy) = (reg(fpp.cur_pos.y), reg(fpp.pos_delta.y));
        let to_term = if nearly_zero(g) {
            reg(1000.0)
        } else {
            fp::fdivs(fp::fneg(fp::fsubs(fp::fneg(term), vy)), g)
        };
        // The original takes the square root of the frames where the square belongs.
        #[allow(clippy::neg_cmp_op_on_partial_ord)]
        let land_y = if to_term <= 0.0 {
            fp::fmadds(vy, to_dest, y)
        } else if !(to_dest >= to_term) {
            let s = math::sqrt_reg(to_dest);
            fp::frsp(fp::fadd(
                y,
                fp::fnmsub(0.5, fp::fmuls(g, s), fp::fmuls(vy, to_dest)),
            ))
        } else {
            let s = math::sqrt_reg(to_term);
            let fall = fp::fnmsub(0.5, fp::fmuls(g, s), fp::fmuls(vy, to_term));
            let rest = fp::fmuls(fp::fsubs(to_dest, to_term), term);
            fp::frsp(fp::fadd(y, fp::fsub(fall, rest)))
        };
        #[allow(clippy::neg_cmp_op_on_partial_ord)]
        if to_dest < 0.0 || !(land_y >= reg(fpp.cpu.x54.y)) {
            self.cpu_cmd1(f, cmd::LSTICK_X_TOWARD_DESTINATION, 0x7F);
        } else {
            self.cpu_cmd1(f, cmd::SET_LSTICK_X, 0);
        }
        self.cpu_finish_neutral_y(f);
    }

    /// The island end farthest from `(x, y)` that computer players can stand at,
    /// `ftCo_800A6700`: 5 inside either end of every live floor island that isn't unsafe
    /// ([`Running::cpu_island_unsafe`]), where a floor they count is within 5 and inside the
    /// blast zones.
    #[doc(alias = "ftCo_800A6700")]
    pub(super) fn cpu_far_island_end(
        &mut self,
        f: FighterId,
        x: f32,
        y: f32,
    ) -> Option<(f32, f32)> {
        let mut best = reg(-1.0);
        let mut found = None;
        let geo = self.game.geometry.as_ref().expect("no collision geometry");
        let mut island = geo.islands.floors;
        while let Some(id) = island {
            let geo = self.game.geometry.as_ref().expect("no collision geometry");
            let isl = *geo.islands.get(id);
            island = isl.next;
            if self.cpu_island_unsafe(Some(id)) {
                continue;
            }
            for (end, left) in [(isl.left, true), (isl.right, false)] {
                let ex = mem(fp::frsp(if left {
                    fp::fadd(5.0, reg(end.x))
                } else {
                    fp::fsub(reg(end.x), 5.0)
                }));
                let above = mem(fp::frsp(fp::fadd(5.0, reg(end.y))));
                let below = mem(fp::frsp(fp::fsub(reg(end.y), 5.0)));
                if self.cpu_floor(ex, above, ex, below).is_none()
                    || self.cpu_out_of_bounds(f, ex, end.y)
                {
                    continue;
                }
                let dy = fp::fsubs(reg(end.y), reg(y));
                let dx = fp::fsubs(reg(ex), reg(x));
                let d = fp::fmadds(dx, dx, fp::fmuls(dy, dy));
                #[allow(clippy::neg_cmp_op_on_partial_ord)]
                if !(d <= best) {
                    best = d;
                    found = Some((ex, end.y));
                }
            }
        }
        if best >= 0.0 { found } else { None }
    }

    /// Back to the fighter's usual behavior (`ftCo_CpuReturnToPreviousBehavior`).
    pub(super) fn cpu_back_to_usual(&mut self, f: FighterId) {
        let c = &mut self.game.fighters[f.0].cpu;
        c.x18 = c.x1C;
        self.cpu_cmd(f, cmd::DONE);
    }

    /// Standing at the destination, `ftCo_800ACD5C` (behavior 10): an approach allowed, it
    /// may close in on its target; a new standing in the results is taunted with a tap up
    /// (outside training); kinds that wait do; off its destination, it goes back to walking;
    /// it turns to face its target, and above level 5 the charging characters charge.
    #[doc(alias = "ftCo_800ACD5C")]
    pub(super) fn cpu_stand(&mut self, f: FighterId) {
        let c = &self.game.fighters[f.0].cpu;
        if c.xF9_b1 && c.x44.is_some() && c.xF8_b6 {
            unported_call("ftCo_800BA674");
        }
        // `GM_TRAINING`.
        if c.x88 > 0 && self.game.gr.game_mode != 28 {
            self.cpu_neutral_stick(f);
            self.cpu_cmd1(f, cmd::WAIT_FOR, 1);
            self.cpu_cmd(f, cmd::PRESS_UP);
            self.cpu_cmd1(f, cmd::WAIT_FOR, 1);
            self.cpu_cmd(f, cmd::RELEASE_UP);
            self.cpu_cmd(f, cmd::DONE);
            let slot = i32::from(self.game.fighters[f.0].player_idx);
            self.game.fighters[f.0].cpu.x88 = 0;
            let standing = self.cpu_standing(slot);
            self.game.fighters[f.0].cpu.x8C = standing as i32;
            return;
        }
        let fpp = &self.game.fighters[f.0];
        if fpp.cpu.kind == 11 && fpp.item_gobj.is_none() {
            self.cpu_data_script(f, 38);
            return;
        }
        if !self.cpu_at_destination(f, 1.0) {
            self.cpu_back_to_usual(f);
            return;
        }
        let fpp = &self.game.fighters[f.0];
        if matches!(fpp.kind, 19 | 7) && fpp.cpu.xF8_b5 {
            self.cpu_data_script(f, 38);
            self.game.fighters[f.0].cpu.xF8_b5 = false;
            return;
        }
        if !self.cpu_target_ahead(f) {
            self.cpu_turn_around(f);
            return;
        }
        let fpp = &self.game.fighters[f.0];
        if fpp.cpu.level < 5 {
            self.cpu_finish_neutral(f);
            return;
        }
        if self.cpu_charge(f) {
            return;
        }
        self.cpu_finish_neutral(f);
    }

    /// The charging characters' part of [`Running::cpu_stand`]: Donkey Kong, Samus, Mewtwo and
    /// Kirby's copies of them start charging with nothing stored; Mewtwo (and Kirby's copy)
    /// let a fully charged ball go. True when it wrote a script.
    fn cpu_charge(&mut self, f: FighterId) -> bool {
        let fpp = &self.game.fighters[f.0];
        let ms = fpp.motion_id;
        match fpp.kind {
            KIND_DONKEY => {
                if ms != super::donkey::ms::SPECIAL_N_LOOP
                    && fpp.u_i32(super::donkey::U_SWINGS) == 0
                {
                    self.cpu_start_charging(f);
                    return true;
                }
                false
            }
            KIND_SAMUS => {
                if ms != super::samus::ms::SPECIAL_N_HOLD && fpp.u_i32(super::samus::u::CHARGE) == 0
                {
                    self.cpu_start_charging(f);
                    return true;
                }
                false
            }
            KIND_MEWTWO => {
                use super::mewtwo::ms as mt;
                let charge = fpp.u_i32(super::mewtwo::u::CHARGE);
                let full = self.mt_attr_f32(f, super::mewtwo::attr::N_CHARGE_CYCLES);
                if mem(fp::frsp(f64::from(charge))) == full {
                    if ms == mt::SPECIAL_N_LOOP || ms == mt::SPECIAL_N_LOOP_FULL {
                        self.cpu_tap_r(f);
                        self.cpu_cmd(f, cmd::DONE);
                        return true;
                    }
                    false
                } else if ms != mt::SPECIAL_N_LOOP && charge == 0 {
                    self.cpu_start_charging(f);
                    true
                } else {
                    false
                }
            }
            KIND_KIRBY => {
                use super::kirby::{ms as kb, u};
                match fpp.u_i32(u::HAT_KIND) {
                    KIND_DONKEY => {
                        if ms != kb::DK_SPECIAL_N_LOOP && fpp.u_i32(u::DK_SWINGS) == 0 {
                            self.cpu_start_charging(f);
                            return true;
                        }
                        false
                    }
                    KIND_SAMUS => {
                        if ms != kb::SS_SPECIAL_N_HOLD && fpp.u_i32(u::SS_CHARGE) == 0 {
                            self.cpu_start_charging(f);
                            return true;
                        }
                        false
                    }
                    KIND_MEWTWO => {
                        let charge = fpp.u_i32(u::MT_CHARGE);
                        let full = self.kb_attr_f32(f, super::kirby::attr::MT_CHARGE_TIME);
                        if mem(fp::frsp(f64::from(charge))) == full {
                            if ms == kb::MT_SPECIAL_N_LOOP || ms == kb::MT_SPECIAL_N_LOOP_FULL {
                                self.cpu_tap_r(f);
                                return true;
                            }
                            false
                        } else if ms != kb::MT_SPECIAL_N_LOOP && charge == 0 {
                            self.cpu_start_charging(f);
                            true
                        } else {
                            false
                        }
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    /// `ftCo_CpuStartCharging`: a tap of B with the stick neutral.
    fn cpu_start_charging(&mut self, f: FighterId) {
        self.cpu_neutral_stick(f);
        self.cpu_cmd(f, cmd::PRESS_B);
        self.cpu_cmd1(f, cmd::WAIT_FOR, 1);
        self.cpu_cmd(f, cmd::RELEASE_B);
        self.cpu_cmd(f, cmd::DONE);
    }

    /// DI in hitstun, `ftCo_800AC5A0` (behavior 18): out of hitstun it goes back to its usual
    /// behavior; else, more often the higher its level, it pushes the stick along its
    /// knockback's angle (turned a quarter: up for a knockback right), or holds it neutral;
    /// it shields to tech when its coin flip said so (`xFA_b1`).
    ///
    /// With no knockback the original pushes the stick by two registers it never set: r5 for
    /// x (what the computer player's frame left there, `r5`) and r30 for y (its caller's
    /// `Fighter*`, so the low byte of the fighter's address). Slippi's NanaDeterminism code
    /// clears r30, and 3.19's r5 too; 3.16's leaves r5 as the frame left it (docs/engine.md).
    #[doc(alias = "ftCo_800AC5A0")]
    pub(super) fn cpu_di(&mut self, f: FighterId, r5: Leftover) {
        let fpp = &self.game.fighters[f.0];
        if !fpp.x221A_b3 {
            let c = &mut self.game.fighters[f.0].cpu;
            c.x18 = c.x1C;
            self.cpu_cmd(f, cmd::RELEASE_R);
            self.cpu_cmd(f, cmd::DONE);
            return;
        }
        let c = &fpp.cpu;
        let push = c.x7C % 120 > 120 - c.level.wrapping_mul(12);
        if push {
            let (kx, ky) = (reg(fpp.x8c_kb_vel.x), reg(fpp.x8c_kb_vel.y));
            let mag = math::sqrt_reg(fp::fmadds(kx, kx, fp::fmuls(ky, ky)));
            if nearly_zero(mag) {
                let (x, y) = self.di_leftover_stick(f, r5);
                self.cpu_cmd1(f, cmd::SET_LSTICK_X, x);
                self.cpu_cmd1(f, cmd::SET_LSTICK_Y, y);
                return self.cpu_di_end(f);
            }
            let inv = fp::fdivs(1.0, mag);
            let (x, y) = (fp::fmuls(kx, inv), fp::fmuls(ky, inv));
            let (sx, sy) = if kx > 0.0 {
                (
                    fp::fctiwz(fp::fmuls(127.0, x)),
                    fp::fctiwz(fp::fmuls(127.0, fp::fneg(y))),
                )
            } else {
                (
                    fp::fctiwz(fp::fmuls(127.0, fp::fneg(x))),
                    fp::fctiwz(fp::fmuls(127.0, y)),
                )
            };
            // The original's swapped commands: x gets the turned y, y the x.
            self.cpu_cmd1(f, cmd::SET_LSTICK_X, sy as u8);
            self.cpu_cmd1(f, cmd::SET_LSTICK_Y, sx as u8);
        } else {
            self.cpu_neutral_stick(f);
        }
        self.cpu_di_end(f);
    }

    /// The DI's last commands: shielding to tech when its coin flip said so, a frame's wait.
    fn cpu_di_end(&mut self, f: FighterId) {
        if self.game.fighters[f.0].cpu.xFA_b1 {
            self.cpu_cmd(f, cmd::PRESS_R);
        }
        self.cpu_cmd1(f, cmd::WAIT_FOR, 1);
        self.cpu_cmd(f, cmd::DONE);
    }

    /// The stick the DI pushes with no knockback, from the low bytes of r5 and r30: what the
    /// frame left in r5, and the fighter's own address in r30, unless Slippi's NanaDeterminism
    /// code cleared them (r30 in both versions, r5 in 3.19's).
    fn di_leftover_stick(&self, f: FighterId, r5: Leftover) -> (u8, u8) {
        let rules = &self.game.rules;
        let (r5, r30) = if rules.slippi {
            let r5 = if rules.codes == super::super::slippi::CodeSet::V3_19 {
                Leftover::Word(0)
            } else {
                r5
            };
            (r5, Leftover::Word(0))
        } else {
            let addr = self.game.ft.addresses.get(&f.0).copied().unwrap_or(0);
            let r30 = if addr == 0 {
                Leftover::Unknown("ftCo_800B2790 (the fighter's address, unmodeled)")
            } else {
                Leftover::Word(addr)
            };
            (r5, r30)
        };
        let byte = |v: Leftover| match v {
            Leftover::Word(w) => w as u8,
            Leftover::Unknown(from) => unported_call(&format!(
                "ftCo_800AC5A0: DI with no knockback reads what {from} left"
            )),
        };
        (byte(r5), byte(r30))
    }

    /// How fast the fighter walks (`ftCo_800AA320`): the stick's step and clamp by level;
    /// Nana's are fixed.
    // Ports `ftCo_800AA320`; aliased once a check reaches it.
    fn cpu_walk_speed(&self, f: FighterId) -> (i32, i32) {
        let fpp = &self.game.fighters[f.0];
        if fpp.kind == super::popo::KIND_NANA {
            return (64, 127);
        }
        match fpp.cpu.level {
            0 => (1, 67),
            1 => (2, 72),
            2 => (2, 77),
            3 => (2, 82),
            4 => (2, 87),
            5 => (2, 92),
            6 => (4, 97),
            7 => (4, 107),
            8 => (8, 117),
            9 => (8, 127),
            _ => unported_call("ftCo_800AA320 at a level past 9 (its outputs unset)"),
        }
    }
}

impl Running<'_> {
    /// Getting rid of the held item (behavior 14), `ftCo_800AD7FC`: with no item back to the
    /// usual behavior; for computer kind 0x1D, facing its target, a throw up, down or forward
    /// by the target's angle (scripts 43, 44, 42), else a turn first; otherwise a throw up or
    /// down when the target is over 75° off level, forward when it's ahead within 15°, else
    /// (or with no target) a throw down for a capsule, egg or Poké Ball and script 40 for the
    /// rest. Each throw goes back to the usual behavior.
    #[doc(alias = "ftCo_800AD7FC")]
    pub(super) fn cpu_throw_item(&mut self, f: FighterId) {
        let fpp = &self.game.fighters[f.0];
        let Some(g) = fpp.item_gobj else {
            self.cpu_back_to_usual(f);
            return;
        };
        let kind = self.game.items[super::super::it::item_of(self.game, g).0].kind;
        // `It_Kind_Capsule`, `_Egg`, `_M_Ball`.
        let drop = matches!(kind, 0 | 3 | 0x22);
        let enemy = fpp.cpu.x44;
        // `lb_8000D008(enemy.y - pos.y, ABS(enemy.x - pos.x))`.
        let angle = |r: &Self, e: FighterId| {
            let (me, other) = (r.game.fighters[f.0].cur_pos, r.game.fighters[e.0].cur_pos);
            let dy = fp::fsubs(reg(other.y), reg(me.y));
            let dx = fp::fsubs(reg(other.x), reg(me.x));
            super::co::guard::stick_angle(dy, abs(dx))
        };
        const STEEP: f64 = 1.308_996_928_855_776_8;
        const LEVEL: f64 = 0.261_799_385_771_155_36;
        match enemy {
            Some(e) if fpp.cpu.kind == 0x1D => {
                if !self.cpu_target_ahead(f) {
                    self.cpu_turn_around(f);
                    return;
                }
                let a = angle(self, e);
                let script = if a > STEEP {
                    0x2B
                } else if a < fp::fneg(STEEP) {
                    0x2C
                } else {
                    0x2A
                };
                self.cpu_data_script(f, script);
            }
            Some(e) => {
                let a = angle(self, e);
                let script = if a > STEEP {
                    0xD
                } else if a < fp::fneg(STEEP) {
                    0xE
                } else if self.cpu_target_ahead(f) && a < LEVEL && a > fp::fneg(LEVEL) {
                    0xF
                } else if drop {
                    0xE
                } else {
                    0x28
                };
                self.cpu_data_script(f, script);
            }
            None => self.cpu_data_script(f, if drop { 0xE } else { 0x28 }),
        }
        let c = &mut self.game.fighters[f.0].cpu;
        c.x18 = c.x1C;
    }
}

impl Running<'_> {
    /// A jump toward the target, `ftCo_800A05F4`: nothing when a jump's height would pass the
    /// top blast zone; a double jump after a while when `x2168` says so (`ftCo_800A1CA8`);
    /// else a full jump with the stick toward the target.
    #[doc(alias = "ftCo_800A05F4")]
    pub(super) fn cpu_jump_toward_target(&mut self, f: FighterId) {
        let fpp = &self.game.fighters[f.0];
        let top = reg(super::super::gr::stage::blast_top(self.game));
        if fp::fadds(reg(fpp.cur_pos.y), reg(fpp.cpu.x558)) > top {
            self.cpu_finish_neutral(f);
            return;
        }
        self.cpu_neutral_stick(f);
        if self.game.fighters[f.0].x2168 != 0 {
            // `ftCo_CpuDoubleJumpAfterDelay`.
            self.cpu_cmd1(f, cmd::WAIT_FOR, 10);
            self.cpu_cmd(f, cmd::RELEASE_Y);
            self.cpu_cmd1(f, cmd::WAIT_FOR, 1);
        } else {
            self.cpu_cmd(f, cmd::RELEASE_Y);
            self.cpu_cmd1(f, cmd::WAIT_FOR, 1);
            self.cpu_cmd1(f, cmd::LSTICK_TOWARD_FIGHTER, 0x7F);
        }
        self.cpu_cmd(f, cmd::PRESS_Y);
        self.cpu_cmd1(f, cmd::WAIT_FOR, 10);
        self.cpu_cmd(f, cmd::RELEASE_Y);
        self.cpu_cmd1(f, cmd::WAIT_FOR, 1);
        self.cpu_cmd(f, cmd::DONE);
    }

    /// A short hop toward the target, `ftCo_800A0798`: after a while when `x2168` says so
    /// (`ftCo_800A1CA8`), else with the stick's x toward the target.
    #[doc(alias = "ftCo_800A0798")]
    fn cpu_hop_toward_target(&mut self, f: FighterId) {
        self.cpu_neutral_stick(f);
        if self.game.fighters[f.0].x2168 != 0 {
            self.cpu_cmd1(f, cmd::WAIT_FOR, 10);
            self.cpu_cmd(f, cmd::RELEASE_Y);
            self.cpu_cmd1(f, cmd::WAIT_FOR, 1);
            self.cpu_cmd(f, cmd::PRESS_Y);
            self.cpu_cmd1(f, cmd::WAIT_FOR, 1);
            self.cpu_cmd(f, cmd::RELEASE_Y);
            self.cpu_cmd1(f, cmd::WAIT_FOR, 1);
        } else {
            self.cpu_cmd(f, cmd::RELEASE_Y);
            self.cpu_cmd1(f, cmd::WAIT_FOR, 1);
            self.cpu_cmd1(f, cmd::LSTICK_X_TOWARD_FIGHTER, 0x7F);
            self.cpu_cmd(f, cmd::PRESS_Y);
            self.cpu_cmd1(f, cmd::WAIT_FOR, 1);
            self.cpu_cmd(f, cmd::RELEASE_Y);
            self.cpu_cmd1(f, cmd::WAIT_FOR, 5);
        }
        self.cpu_cmd(f, cmd::DONE);
    }

    /// Jumping at the target (behavior 8), `ftCo_800B7180`, by the kind of jump
    /// `ftCo_800B732C` picked (`xF8_b34`): a full jump (1) or a short hop (2), then (3) in the
    /// air drifting toward the target while rising, jumping again while it's above, and back
    /// to the usual behavior otherwise. With no target it goes back to the usual behavior,
    /// twice (the jump kind cleared, the switch takes its default).
    #[doc(alias = "ftCo_800B7180")]
    pub(super) fn cpu_jump_at_target(&mut self, f: FighterId) {
        let target = self.game.fighters[f.0].cpu.x44;
        let back = |r: &mut Self| {
            let c = &mut r.game.fighters[f.0].cpu;
            c.x18 = c.x1C;
            c.xF8_b34 = 0;
            r.cpu_cmd(f, cmd::DONE);
        };
        if target.is_none() {
            back(self);
        }
        match self.game.fighters[f.0].cpu.xF8_b34 {
            1 => {
                self.cpu_jump_toward_target(f);
                self.game.fighters[f.0].cpu.xF8_b34 = 3;
            }
            2 => {
                self.cpu_hop_toward_target(f);
                self.game.fighters[f.0].cpu.xF8_b34 = 3;
            }
            3 => {
                let fpp = &self.game.fighters[f.0];
                if fpp.ground_or_air != GA_AIR {
                    back(self);
                } else if f64::from(fpp.pos_delta.y) > 0.0 {
                    self.cpu_cmd1(f, cmd::SET_LSTICK_Y, 0);
                    self.cpu_cmd1(f, cmd::LSTICK_X_TOWARD_FIGHTER, 0x7F);
                    self.cpu_cmd(f, cmd::DONE);
                } else {
                    let t = target.expect("a jump at no target, rising past its start");
                    if self.game.fighters[t.0].cur_pos.y > fpp.cur_pos.y {
                        self.cpu_jump_toward_target(f);
                    } else {
                        back(self);
                    }
                }
            }
            _ => back(self),
        }
    }
}
