//! Computer players getting back to the stage (`ftCo_0A01.c`, behavior 4): the ledge they aim
//! for, a jump off stage pieces that don't count as ground, and the recovery in the air.

use gekko_fp as fp;

use super::super::gr::stage;
use super::super::handles::FighterId;
use super::super::state::Running;
use super::common::GA_GROUND;
use super::cpu_script::cmd;
use crate::fpu::{mem, reg};
use crate::math;

/// `Collide_WallMask` in `CollData::env_flags`.
const WALL_MASK: i32 = 0xFFF;

/// `Ft_Kind_Luigi`.
const KIND_LUIGI: i32 = 17;

/// Which end of an island a ledge target sits at.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Edge {
    /// The left end (`x8`), approached from the left: `ftCo_800A4038`.
    Left,
    /// The right end (`x14`), approached from the right: `ftCo_800A3908`.
    Right,
}

/// `ftCo_IsNearlyZero`.
fn nearly_zero(x: f64) -> bool {
    let tiny = reg(1e-5);
    x < tiny && x > fp::fneg(tiny)
}

impl Running<'_> {
    /// Getting back to the stage, behavior 4 of `ftCo_800B2790`: it picks a ledge to aim for
    /// (`ftCo_800A8DE4`); on the ground it jumps off a stage piece that doesn't count as
    /// ground (`ftCo_800A0148`) or goes back to its usual behavior, and in the air recovers
    /// (`ftCo_800A9904`).
    pub(super) fn cpu_recover_behavior(&mut self, f: FighterId) {
        self.cpu_pick_ledge(f);
        let fpp = &self.game.fighters[f.0];
        if fpp.ground_or_air == GA_GROUND {
            if self.excluded_floor(fpp.coll_data.floor.index) {
                self.cpu_jump_off_piece(f);
                return;
            }
            let c = &mut self.game.fighters[f.0].cpu;
            c.x18 = c.x1C;
            self.cpu_cmd(f, cmd::DONE);
            self.game.fighters[f.0].cpu.xFA_b2 = false;
            return;
        }
        self.cpu_recover_air(f);
    }

    /// Picks the ledge to recover to once per recovery (`xFA_b2`), `ftCo_800A8DE4`: the one
    /// ahead within reach of its fall, else the one behind, else the nearest one ahead.
    #[doc(alias = "ftCo_800A8DE4")]
    fn cpu_pick_ledge(&mut self, f: FighterId) {
        let c = &mut self.game.fighters[f.0].cpu;
        if c.xFA_b2 {
            return;
        }
        c.xFA_b2 = true;
        c.x5C = 10000.0;
        if self.game.fighters[f.0].facing_dir > 0.0 {
            if !self.cpu_ledge_target(f, Edge::Left, true)
                && !self.cpu_ledge_target(f, Edge::Right, true)
            {
                self.cpu_ledge_target(f, Edge::Left, false);
            }
        } else if !self.cpu_ledge_target(f, Edge::Right, true)
            && !self.cpu_ledge_target(f, Edge::Left, true)
        {
            self.cpu_ledge_target(f, Edge::Right, false);
        }
    }

    /// Aims for an island's end on the side the fighter is past, `ftCo_800A4038` (left ends,
    /// approached from the left) and `ftCo_800A3908` (right ends, from the right), over every
    /// live floor island inside the blast zones (Corneria's ceilinged one aside). Its fall is
    /// predicted over the frames its air speed takes to get there; with `reachable` it takes
    /// the first end it doesn't fall below, else the nearest. The target is 5 inside the end,
    /// where a floor must be that computer players count.
    // Ports `ftCo_800A4038`; aliased once a check reaches it.
    #[doc(alias = "ftCo_800A3908")]
    fn cpu_ledge_target(&mut self, f: FighterId, edge: Edge, reachable: bool) -> bool {
        let fpp = &self.game.fighters[f.0];
        let grav = reg(fpp.co_attrs.gravity);
        let frames = if nearly_zero(grav) {
            0x3E8
        } else {
            fp::fctiwz(fp::fdivs(
                fp::fneg(fp::fsubs(
                    fp::fneg(reg(fpp.co_attrs.terminal_velocity)),
                    reg(fpp.pos_delta.y),
                )),
                grav,
            ))
        };
        let mut island = self
            .game
            .geometry
            .as_ref()
            .expect("no collision geometry")
            .islands
            .floors;
        while let Some(id) = island {
            let geo = self.game.geometry.as_ref().expect("no collision geometry");
            let isl = *geo.islands.get(id);
            island = isl.next;
            let end = match edge {
                Edge::Left => isl.left,
                Edge::Right => isl.right,
            };
            let (ex, ey) = (end.x, end.y);
            if self.cpu_out_of_bounds(f, ex, ey) {
                continue;
            }
            if self.corneria_ceiling_floor(i32::from(isl.left_vtx)) {
                continue;
            }
            let fpp = &self.game.fighters[f.0];
            let dx = match edge {
                Edge::Left => fp::fsubs(reg(ex), reg(fpp.cur_pos.x)),
                Edge::Right => fp::fsubs(reg(fpp.cur_pos.x), reg(ex)),
            };
            if dx <= 0.0 || dx.is_nan() {
                continue;
            }
            let t = fp::fctiwz(fp::fdivs(dx, reg(fpp.co_attrs.air_drift_max)));
            let (vy, y, g) = (
                reg(fpp.pos_delta.y),
                reg(fpp.cur_pos.y),
                reg(fpp.co_attrs.gravity),
            );
            let tf = fp::frsp(f64::from(t));
            let dist = if frames <= 0 {
                fp::fmadds(vy, tf, y)
            } else if t < frames {
                let s = math::sqrt_reg(tf);
                fp::frsp(fp::fadd(
                    y,
                    fp::fnmsub(0.5, fp::fmuls(g, s), fp::fmuls(vy, tf)),
                ))
            } else {
                let ff = fp::frsp(f64::from(frames));
                let s = math::sqrt_reg(ff);
                let term = fp::fmuls(
                    reg(fpp.co_attrs.terminal_velocity),
                    fp::frsp(f64::from(t.wrapping_sub(frames))),
                );
                fp::frsp(fp::fadd(
                    y,
                    fp::fsub(fp::fnmsub(0.5, fp::fmuls(g, s), fp::fmuls(vy, ff)), term),
                ))
            };
            let px = mem(match edge {
                Edge::Left => fp::frsp(fp::fadd(5.0, reg(ex))),
                Edge::Right => fp::frsp(fp::fsub(reg(ex), 5.0)),
            });
            if reachable {
                if fp::fadds(dist, reg(fpp.cpu.x558)) < reg(ey) {
                    continue;
                }
                if self.cpu_ledge_floor(px, ey) && !self.cpu_out_of_bounds(f, px, ey) {
                    self.cpu_set_destination(f, px, ey, 5.0);
                    self.cpu_destination_distance(f);
                    return true;
                }
            } else {
                let fpp = &self.game.fighters[f.0];
                let ddx = fp::fsubs(reg(px), reg(fpp.cur_pos.x));
                let ddy = fp::fsubs(reg(ey), reg(fpp.cur_pos.y));
                if self.cpu_ledge_floor(px, ey) && !self.cpu_out_of_bounds(f, px, ey) {
                    let dist = math::sqrt_reg(fp::fmadds(ddx, ddx, fp::fmuls(ddy, ddy)));
                    if reg(self.game.fighters[f.0].cpu.x5C) > dist {
                        self.cpu_set_destination(f, px, ey, 5.0);
                        self.cpu_destination_distance(f);
                    }
                }
            }
        }
        false
    }

    /// Whether a floor computer players count is within 5 above or below `(x, y)`
    /// (`ftCo_800A3908_inline1`, `ftCo_800A4038_inline1`).
    fn cpu_ledge_floor(&mut self, x: f32, y: f32) -> bool {
        let above = mem(fp::fadds(5.0, reg(y)));
        let below = mem(fp::fsubs(reg(y), 5.0));
        let hit = self
            .game
            .geometry
            .as_mut()
            .expect("no collision geometry")
            .check_floor(x, above, x, below, 0.0, -1, -1, -1, None);
        hit.is_some_and(|h| !self.excluded_floor(h.line))
    }

    /// The distance to the destination, `ftCo_800A49B4`, which the nearest-ledge search
    /// compares against (`x5C`).
    #[doc(alias = "ftCo_800A49B4")]
    fn cpu_destination_distance(&mut self, f: FighterId) {
        let fpp = &self.game.fighters[f.0];
        let dx = fp::fsubs(reg(fpp.cur_pos.x), reg(fpp.cpu.x54.x));
        let dy = fp::fsubs(reg(fpp.cur_pos.y), reg(fpp.cpu.x54.y));
        let d = math::sqrt(mem(fp::fmadds(dx, dx, fp::fmuls(dy, dy))));
        self.game.fighters[f.0].cpu.x5C = d;
    }

    /// Jumping off a stage piece that doesn't count as ground, `ftCo_800A0148`: nothing above
    /// the top blast zone; a double jump after a while when `x2168` says so; else a jump,
    /// toward the destination when it's more than 30 away.
    // Ports `ftCo_800A0148`; aliased once a check reaches it.
    // Ports `ftCo_800A1CA8`; aliased once a check reaches it.
    fn cpu_jump_off_piece(&mut self, f: FighterId) {
        let fpp = &self.game.fighters[f.0];
        let top = reg(stage::blast_top(self.game));
        if fp::fadds(reg(fpp.cur_pos.y), reg(fpp.cpu.x558)) > top {
            self.cpu_finish_neutral(f);
            return;
        }
        if fpp.x2168 != 0 {
            // `ftCo_CpuDoubleJumpAfterDelay`.
            self.cpu_neutral_stick(f);
            self.cpu_cmd1(f, cmd::WAIT_FOR, 10);
            self.cpu_jump_tap(f);
            return;
        }
        let d = fp::fsubs(reg(fpp.cpu.x54.x), reg(fpp.cur_pos.x));
        let d = if d < 0.0 { fp::fneg(d) } else { d };
        self.cpu_cmd(f, cmd::RELEASE_Y);
        if d > 30.0 {
            self.cpu_cmd1(f, cmd::WAIT_FOR, 1);
            self.cpu_cmd1(f, cmd::LSTICK_TOWARD_DESTINATION, 0x7F);
        } else {
            self.cpu_neutral_stick(f);
            self.cpu_cmd1(f, cmd::WAIT_FOR, 1);
        }
        self.cpu_cmd(f, cmd::PRESS_Y);
        self.cpu_cmd1(f, cmd::WAIT_FOR, 10);
        self.cpu_cmd(f, cmd::RELEASE_Y);
        self.cpu_cmd1(f, cmd::WAIT_FOR, 1);
        self.cpu_cmd(f, cmd::DONE);
    }

    /// Y let go, a frame, Y held for 10 frames, let go, a frame, done.
    fn cpu_jump_tap(&mut self, f: FighterId) {
        self.cpu_cmd(f, cmd::RELEASE_Y);
        self.cpu_cmd1(f, cmd::WAIT_FOR, 1);
        self.cpu_cmd(f, cmd::PRESS_Y);
        self.cpu_cmd1(f, cmd::WAIT_FOR, 10);
        self.cpu_cmd(f, cmd::RELEASE_Y);
        self.cpu_cmd1(f, cmd::WAIT_FOR, 1);
        self.cpu_cmd(f, cmd::DONE);
    }

    /// The recovery in the air, `ftCo_800A9904`: a jump toward the destination, or the
    /// character's recovery move with its jumps used, once it should (`ftCo_800A3498`);
    /// while drifting back (`xFA_b5`) the stick toward the destination unless its fall
    /// reaches it anyway; else toward the destination, away from it under a ceiling.
    #[doc(alias = "ftCo_800A9904")]
    fn cpu_recover_air(&mut self, f: FighterId) {
        if self.cpu_should_recover(f) {
            let fpp = &self.game.fighters[f.0];
            if fpp.co_attrs.max_jumps > i32::from(fpp.x1968_jumpsUsed) {
                self.cpu_cmd1(f, cmd::LSTICK_TOWARD_DESTINATION, 0x7F);
                self.cpu_cmd(f, cmd::PRESS_Y);
                self.cpu_cmd1(f, cmd::WAIT_FOR, 1);
                self.cpu_cmd(f, cmd::RELEASE_Y);
                self.cpu_cmd(f, cmd::DONE);
            } else {
                self.cpu_recover(f);
            }
            return;
        }
        let fpp = &self.game.fighters[f.0];
        let c = &fpp.cpu;
        if c.xFA_b5 {
            let dx = fp::fsubs(reg(c.x54.x), reg(fpp.cur_pos.x));
            let x_time = if nearly_zero(reg(fpp.pos_delta.x)) {
                1000.0
            } else {
                fp::fdivs(dx, reg(fpp.pos_delta.x))
            };
            let g = reg(fpp.co_attrs.gravity);
            let terminal_time = if nearly_zero(g) {
                1000.0
            } else {
                fp::fdivs(
                    fp::fneg(fp::fsubs(
                        fp::fneg(reg(fpp.co_attrs.terminal_velocity)),
                        reg(fpp.pos_delta.y),
                    )),
                    g,
                )
            };
            let (vy, y) = (reg(fpp.pos_delta.y), reg(fpp.cur_pos.y));
            let predicted_y = if terminal_time <= 0.0 {
                fp::fmadds(vy, x_time, y)
            } else if x_time < terminal_time {
                let s = math::sqrt_reg(x_time);
                fp::frsp(fp::fadd(
                    y,
                    fp::fnmsub(0.5, fp::fmuls(g, s), fp::fmuls(vy, x_time)),
                ))
            } else {
                let s = math::sqrt_reg(terminal_time);
                let term = fp::fmuls(
                    reg(fpp.co_attrs.terminal_velocity),
                    fp::fsubs(x_time, terminal_time),
                );
                fp::frsp(fp::fadd(
                    y,
                    fp::fsub(
                        fp::fnmsub(0.5, fp::fmuls(g, s), fp::fmuls(vy, terminal_time)),
                        term,
                    ),
                ))
            };
            let stick = fp::fctiwz(fp::fmadds(
                reg(4.700_000_3_f32),
                fp::frsp(f64::from(c.level.wrapping_add(1))),
                80.0,
            ));
            if x_time < 0.0 || predicted_y < reg(c.x54.y) {
                self.cpu_cmd1(f, cmd::LSTICK_X_TOWARD_DESTINATION, stick as u8);
            } else {
                self.cpu_cmd1(f, cmd::SET_LSTICK_X, 0);
            }
            self.cpu_finish_neutral_y(f);
            return;
        }
        let (pos, dest) = (fpp.cur_pos, (c.x54.x, c.x54.y));
        let ceiling = self
            .game
            .geometry
            .as_mut()
            .expect("no collision geometry")
            .check_ceiling(pos.x, pos.y, dest.0, dest.1, -1, -1)
            .is_some();
        let stick = if ceiling { 0x81 } else { 0x7F };
        self.cpu_cmd1(f, cmd::LSTICK_X_TOWARD_DESTINATION, stick);
        self.cpu_finish_neutral_y(f);
    }

    /// Whether the fighter should jump or use its recovery move now, `ftCo_800A3498`: in the
    /// air, not rising and not drifting back (`xFA_b5`), against a wall, or (Luigi) falling,
    /// or with its destination above its feet by more than its margin (`x568`).
    #[doc(alias = "ftCo_800A3498")]
    fn cpu_should_recover(&self, f: FighterId) -> bool {
        let fpp = &self.game.fighters[f.0];
        if fpp.cpu.xFA_b5 || fpp.ground_or_air != super::common::GA_AIR {
            return false;
        }
        if fpp.pos_delta.y > 0.0 {
            return false;
        }
        if fpp.coll_data.env_flags & WALL_MASK != 0 {
            return true;
        }
        if fpp.kind == KIND_LUIGI {
            return fpp.pos_delta.y < 0.0;
        }
        let feet = fp::fadds(
            reg(fpp.coll_data.cur_pos.y),
            reg(fpp.coll_data.ecb.bottom.y),
        );
        reg(fpp.cpu.x54.y) > fp::fadds(reg(fpp.cpu.x568), feet)
    }

    /// Hanging from a ledge, `ftCo_800ACB44` (behavior 6): still catching it, the stick
    /// neutral; off it, back to the usual behavior; kinds 0 and 15, or with no target, get up
    /// a way at random (`ftCo_800A0AF4`); a target within 30 while acting alone (`xF9_b2`) is
    /// met with a roll more often the higher its level, else an attack, and a farther one with
    /// a way up at random.
    #[doc(alias = "ftCo_800ACB44")]
    pub(super) fn cpu_ledge_behavior(&mut self, f: FighterId) {
        match self.cpu_on_ledge(f) {
            1 => {
                self.cpu_finish_neutral(f);
                return;
            }
            0 => {
                self.cpu_back_to_usual(f);
                return;
            }
            _ => {}
        }
        let kind = self.game.fighters[f.0].cpu.kind;
        if kind == 0xF || kind == 0 {
            self.cpu_ledge_getup(f);
            return;
        }
        let Some(target) = self.cpu_find_target(f) else {
            self.cpu_ledge_getup(f);
            return;
        };
        let d = self.cpu_distance(f, target);
        if self.game.fighters[f.0].cpu.xF9_b2 && d < 30.0 {
            let r = reg(self.game.objs.rng.draw_f32());
            let level = self.game.fighters[f.0].cpu.level;
            if fp::fmuls(reg(0.1), fp::frsp(f64::from(level))) > r {
                self.cpu_retap_r(f);
            } else {
                self.cpu_retap_a(f);
            }
        } else {
            self.cpu_ledge_getup(f);
        }
    }

    /// A way up from a ledge at random, `ftCo_800A0AF4`: up (60%), a roll (20%), an attack
    /// (10%) or a jump.
    #[doc(alias = "ftCo_800A0AF4")]
    fn cpu_ledge_getup(&mut self, f: FighterId) {
        let r = reg(self.game.objs.rng.draw_f32());
        if r < reg(0.6) {
            self.cpu_cmd1(f, cmd::SET_LSTICK_X, 0);
            self.cpu_cmd1(f, cmd::SET_LSTICK_Y, 0x50);
            self.cpu_cmd1(f, cmd::WAIT_FOR, 1);
            self.cpu_finish_neutral_y(f);
        } else if r < reg(0.8) {
            self.cpu_retap_r(f);
        } else if r < reg(0.9) {
            self.cpu_retap_a(f);
        } else {
            self.cpu_cmd(f, cmd::RELEASE_Y);
            self.cpu_cmd1(f, cmd::WAIT_FOR, 1);
            self.cpu_cmd(f, cmd::PRESS_Y);
            self.cpu_cmd1(f, cmd::WAIT_FOR, 1);
            self.cpu_cmd(f, cmd::RELEASE_Y);
            self.cpu_cmd(f, cmd::DONE);
        }
    }

    /// `ftCo_CpuRetapR`.
    fn cpu_retap_r(&mut self, f: FighterId) {
        self.cpu_cmd(f, cmd::RELEASE_R);
        self.cpu_cmd1(f, cmd::WAIT_FOR, 1);
        self.cpu_cmd(f, cmd::PRESS_R);
        self.cpu_cmd1(f, cmd::WAIT_FOR, 1);
        self.cpu_cmd(f, cmd::RELEASE_R);
        self.cpu_cmd(f, cmd::DONE);
    }
}
