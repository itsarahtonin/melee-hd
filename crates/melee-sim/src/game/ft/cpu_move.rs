//! Computer players getting where they're going (`ftCo_0A01.c`): the scripts that walk, run
//! and turn toward the destination on the ground (`ftCo_800AB224`), drift, jump and recover
//! toward it in the air (`ftCo_800A9CB4`), and leave the respawn platform.

use gekko_fp as fp;

use super::super::code::unported_call;
use super::super::handles::FighterId;
use super::super::state::Running;
use super::common::{GA_AIR, GA_GROUND};
use super::cpu_ai::P_LINK_FIGHTER;
use super::cpu_script::cmd;
use crate::fpu::{mem, reg};
use crate::math;

/// `ftLoadCommonData::x58`: the computer players' data, whose first word is its scripts.
const CPU_DATA: u32 = 0x58;

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
    fn cpu_finish_neutral_y(&mut self, f: FighterId) {
        self.cpu_cmd1(f, cmd::SET_LSTICK_Y, 0);
        self.cpu_cmd(f, cmd::DONE);
    }

    /// `ftCo_CpuTapR`.
    fn cpu_tap_r(&mut self, f: FighterId) {
        self.cpu_cmd(f, cmd::PRESS_R);
        self.cpu_cmd1(f, cmd::WAIT_FOR, 1);
        self.cpu_cmd(f, cmd::RELEASE_R);
    }

    /// Turns around with a flick back (`ftCo_CpuTurnAround`).
    fn cpu_turn_around(&mut self, f: FighterId) {
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
    #[doc(alias = "ftCo_800B4880")]
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
    #[doc(alias = "ftCo_800A5CE0")]
    fn cpu_entering_enemy(&self, f: FighterId) -> Option<FighterId> {
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
    #[doc(alias = "ftCo_800ABA34")]
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
                unported_call("ftCo_800A05F4")
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
    #[doc(alias = "ftCo_800A3234")]
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

    /// A recovery toward the destination by character, `ftCo_800A96B8`; the default (and
    /// Nana's) is up and toward it, then the up special.
    #[doc(alias = "ftCo_800A96B8")]
    fn cpu_recover(&mut self, f: FighterId) {
        match self.game.fighters[f.0].kind {
            12 | 23 | 1 | 22 | 14 | 8 | 17 | 19 | 13 => {
                unported_call("ftCo_800A96B8 for a character with its own recovery")
            }
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

    /// Moving in the air, `ftCo_800A9CB4`: falling short, it jumps while it has jumps, air-
    /// dodges on the Temple and Fourside, and recovers when small or on Icicle Mountain and
    /// Rainbow Cruise; under a ceiling it drifts back; else it steers toward the destination
    /// by where its fall (under gravity to terminal speed) will bring it when level with it.
    #[doc(alias = "ftCo_800A9CB4")]
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
    fn cpu_near_edge(&self, f: FighterId, scale: f64) -> bool {
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
    #[doc(alias = "ftCo_800A2BD4")]
    fn cpu_facing_destination(&self, f: FighterId) -> bool {
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
    #[doc(alias = "ftCo_800AB224")]
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
        let _ = angle;
        unported_call("ftCo_800AB224 off the fighter's island");
    }

    /// Walks or runs to a destination on the fighter's island, `ftCo_800AA42C`: off a teeter
    /// first, turning to face it, at full speed while far (running if it runs, else a dash
    /// start), slowing in on it, standing once within reach.
    #[doc(alias = "ftCo_800AA42C")]
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
    #[doc(alias = "ftCo_800A2C08")]
    pub(super) fn cpu_target_ahead(&self, f: FighterId) -> bool {
        let fpp = &self.game.fighters[f.0];
        let Some(t) = fpp.cpu.x44 else { return true };
        let dx = fp::fsubs(reg(self.game.fighters[t.0].cur_pos.x), reg(fpp.cur_pos.x));
        if abs(dx) < 1.0 {
            return true;
        }
        fp::fmuls(dx, reg(fpp.facing_dir)) >= 0.0
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
        // Ft_Kind_Donkey, Ft_Kind_Samus, Ft_Kind_Mewtwo, Ft_Kind_Kirby.
        if matches!(fpp.kind, 3 | 13 | 16 | 4) {
            unported_call("ftCo_800ACD5C for a character that charges");
        }
        self.cpu_finish_neutral(f);
    }

    /// DI in hitstun, `ftCo_800AC5A0` (behavior 18): out of hitstun it goes back to its usual
    /// behavior; else, more often the higher its level, it pushes the stick along its
    /// knockback's angle (turned a quarter: up for a knockback right), or holds it neutral;
    /// it shields to tech when its coin flip said so (`xFA_b1`).
    ///
    /// With no knockback the original pushes the stick by two registers it never set: r5 for
    /// x (its caller's) and r30 for y (its caller's `Fighter*`, so the low byte of Nana's
    /// address). Slippi's NanaDeterminism code clears r30, and 3.19's r5 too; 3.16's leaves r5
    /// as the caller left it. Such a frame stops the port (`DI with no knockback`) until the
    /// value ssbm-rs's ports leave there agrees with the original's.
    #[doc(alias = "ftCo_800AC5A0")]
    pub(super) fn cpu_di(&mut self, f: FighterId) {
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
                unported_call("ftCo_800AC5A0: DI with no knockback (its stick registers unset)");
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
        if self.game.fighters[f.0].cpu.xFA_b1 {
            self.cpu_cmd(f, cmd::PRESS_R);
        }
        self.cpu_cmd1(f, cmd::WAIT_FOR, 1);
        self.cpu_cmd(f, cmd::DONE);
    }

    /// How fast the fighter walks (`ftCo_800AA320`): the stick's step and clamp by level;
    /// Nana's are fixed.
    #[doc(alias = "ftCo_800AA320")]
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
