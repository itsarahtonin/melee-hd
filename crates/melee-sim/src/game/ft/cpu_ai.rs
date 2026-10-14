//! Computer players' decisions (`ftCo_0A01.c`): the per-frame process a computer-controlled
//! fighter runs (`ftCo_800B3900`), which reads the stage around it, picks what to do by its
//! kind, and writes commands its script runs into the controller ([`super::cpu_script`]).
//! Nana, the Ice Climbers' partner, is the computer player ported first: she follows Popo,
//! replaying his recorded inputs, and acts on her own when apart from him.

use gekko_fp as fp;

use super::super::code::unported_call;
use super::super::gr::stage;
use super::super::handles::FighterId;
use super::super::state::Running;
use super::common::{GA_AIR, GA_GROUND};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::math;

/// `HSD_GOBJ_PLINK_FIGHTER`.
pub(super) const P_LINK_FIGHTER: u8 = 8;

/// `CpuKind_Nana`.
const CPU_KIND_NANA: i32 = 6;

/// `Ft_Kind_Nana`.
const KIND_NANA: i32 = 11;

/// `ftPp_MS_SpecialLw`, `ftPp_MS_SpecialAirLw`, and the Ice Climbers' up special's states
/// (`ftPp_MS_SpecialHi_0` to `_5`).
const MS_PP_SPECIAL_LW: i32 = 357;
const MS_PP_SPECIAL_AIR_LW: i32 = 358;
const MS_PP_SPECIAL_HI_0: i32 = 361;
const MS_PP_SPECIAL_HI_5: i32 = 366;

/// Common action states (`ftCo_MS_*`).
const MS_REBIRTH: i32 = 12;
const MS_REBIRTH_WAIT: i32 = 13;
const MS_WAIT: i32 = 14;
const MS_SQUAT: i32 = 39;
const MS_SQUAT_WAIT: i32 = 40;
const MS_LANDING: i32 = 42;
const MS_LANDING_FALL_SPECIAL: i32 = 43;
const MS_LANDING_AIR_N: i32 = 70;
const MS_LANDING_AIR_F: i32 = 71;
const MS_LANDING_AIR_B: i32 = 72;
const MS_LANDING_AIR_HI: i32 = 73;
const MS_LANDING_AIR_LW: i32 = 74;
const MS_ITEM_PARASOL_FALL: i32 = 145;
const MS_ITEM_PARASOL_DAMAGE_FALL: i32 = 147;
const MS_FIRE_FLOWER_SHOOT: i32 = 152;
const MS_FIRE_FLOWER_SHOOT_AIR: i32 = 153;
const MS_CAPTURE_WAIT_HI: i32 = 224;
const MS_CAPTURE_WAIT_LW: i32 = 227;
const MS_OTTOTTO: i32 = 245;
const MS_OTTOTTO_WAIT: i32 = 246;
const MS_CLIFF_CATCH: i32 = 252;
const MS_CLIFF_WAIT: i32 = 253;
const MS_SHOULDERED_WAIT: i32 = 266;
const MS_SHOULDERED_TURN: i32 = 270;
const MS_CAPTURE_KIRBY: i32 = 288;
const MS_CAPTURE_WAIT_KIRBY: i32 = 289;
const MS_WARP_STAR_JUMP: i32 = 305;
const MS_WARP_STAR_FALL: i32 = 306;
const MS_COUNT: i32 = 341;

/// `Fighter_x1A88_xFC_t`'s size and how many the history holds.
const INPUT_SIZE: u32 = 0x1C;
const INPUTS: u32 = 30;

/// What a register holds when code reads it without its caller passing it: the stale-register
/// rule for the CPU AI (docs/engine.md). The computer player's frame follows one such value,
/// r5, which Nana's DI takes as her stick's x when her knockback is zero (`ftCo_800AC5A0`):
/// each function on the way returns the r5 it leaves, as the original's machine code leaves
/// it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Leftover {
    /// The register's word.
    Word(u32),
    /// A word melee-hd doesn't follow, named by the function that left it: reading it stops.
    Unknown(&'static str),
}

impl Running<'_> {
    /// The other fighter of the same player (Nana's Popo, Popo's Nana), `ftCo_800A589C`: the
    /// first in the fighters' list, or none if that one is asleep or there is none.
    // Ports `ftCo_800A589C`; aliased once a check reaches it.
    pub fn cpu_partner(&self, f: FighterId) -> Option<FighterId> {
        let fp = &self.game.fighters[f.0];
        let mut cur = self.game.world.first_in(P_LINK_FIGHTER);
        while let Some(g) = cur {
            cur = self.game.world.gobjs[g.0].next;
            if fp.gobj == Some(g) {
                continue;
            }
            let o = self.game.fighter_id(g);
            let other = &self.game.fighters[o.0];
            if other.player_idx == fp.player_idx {
                return (!other.is_sleeping).then_some(o);
            }
        }
        None
    }

    /// A random float (`HSD_Randf`) as a register holds it.
    fn randf(&mut self) -> f64 {
        reg(self.game.objs.rng.draw_f32())
    }

    /// A computer-controlled fighter's frame, `ftCo_800B3900`: it reads the stage, decides by
    /// its kind, writes a new script when the last one is done, runs its script, and Nana
    /// replays Popo's inputs while she follows him; its frame count goes up.
    #[doc(alias = "ftCo_800B3900")]
    pub fn cpu_frame(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.cpu_read_stage(f);
        let r5 = self.cpu_decide(f);
        self.cpu_behave(f, r5);
        self.cpu_run_script(f);
        self.nana_follow_inputs(f);
        let c = &mut self.game.fighters[f.0].cpu;
        c.x7C = c.x7C.wrapping_add(1);
    }

    /// The floor below `(x, y)` (from 10 above to 1000 below) unless it is one computer
    /// players don't count (`ftCo_800A1B38`): its point.
    fn cpu_floor_below(&mut self, x: f32, y: f32) -> Option<crate::vec::Vec3> {
        let below = mem(fp::frsp(fp::fsub(reg(y), 1000.0)));
        let above = mem(fp::frsp(fp::fadd(10.0, reg(y))));
        let hit = self
            .game
            .geometry
            .as_mut()
            .expect("no collision geometry")
            .check_floor(x, above, x, below, 0.0, -1, -1, -1, None)?;
        (!self.excluded_floor(hit.line)).then_some(hit.pos)
    }

    /// What the fighter knows of the stage this frame, `ftCo_800B33B0`: whether it's over
    /// ground inside the blast zones, how long it has stood still, its target's timer, its
    /// destination carried along with moving floors, its recovery scales and whether it's
    /// under Corneria's ceiling.
    // Ports `ftCo_800B33B0`; aliased once a check reaches it.
    fn cpu_read_stage(&mut self, f: FighterId) {
        let timer = self.game.fighters[f.0].cpu.x7C;
        if timer % 300 == 0 {
            let r = self.randf();
            let c = &mut self.game.fighters[f.0].cpu;
            let p = fp::fmadds(
                0.039_999_999_105_930_33,
                fp::frsp(f64::from(c.level)),
                0.300_000_011_920_928_96,
            );
            c.xFA_b34 = u8::from(r < p);
        }
        let pos = self.game.fighters[f.0].cur_pos;
        let in_bounds = match self.cpu_floor_below(pos.x, pos.y) {
            Some(floor) => {
                let (left, right) = (stage::blast_left(self.game), stage::blast_right(self.game));
                let (bottom, top) = (stage::blast_bottom(self.game), stage::blast_top(self.game));
                let c = &self.game.fighters[f.0].cpu;
                let (hw, hh) = (reg(c.half_width), reg(c.half_height));
                let (fx, fy) = (reg(floor.x), reg(floor.y));
                let oob = fx < fp::fadds(hw, reg(left))
                    || fx > fp::fsubs(reg(right), hw)
                    || fy < fp::fadds(hh, reg(bottom))
                    || fy > fp::fsubs(reg(top), hh);
                !oob
            }
            None => false,
        };
        let fpp = &mut self.game.fighters[f.0];
        fpp.cpu.xFA_b5 = in_bounds;
        let coll = &fpp.coll_data;
        let vy = fp::fsubs(reg(coll.cur_pos.y), reg(coll.last_pos.y));
        let vx = fp::fsubs(reg(coll.cur_pos.x), reg(coll.last_pos.x));
        let vmag = math::sqrt_reg(fp::fmadds(vx, vx, fp::fmuls(vy, vy)));
        let c = &mut fpp.cpu;
        if vmag < 0.01 {
            c.x84 = c.x84.wrapping_add(1);
        } else {
            c.x84 = 0;
        }
        if c.xF9_b0 {
            if c.x30 != 0 {
                c.x30 -= 1;
            } else {
                c.xF9_b0 = false;
            }
        } else {
            let r = self.randf();
            self.game.fighters[f.0].cpu.x30 =
                fp::fctiwz(fp::fmuls(120.0, fp::fmuls(0.5, fp::fmuls(0.5, r))));
        }
        let c = &mut self.game.fighters[f.0].cpu;
        if c.x60 != 0 {
            c.x60 -= 1;
            if c.x60 == 0 {
                c.x54.x = c.x64.x;
                c.x54.y = c.x64.y;
            }
        }
        // The destination rides its floor (or the stage's wind).
        let (sx, sy) = (c.x54.x, c.x54.y);
        let below = mem(fp::frsp(fp::fsub(reg(sy), 2.0)));
        let above = mem(fp::frsp(fp::fadd(2.0, reg(sy))));
        let hit = self
            .game
            .geometry
            .as_mut()
            .expect("no collision geometry")
            .check_floor(sx, above, sx, below, 0.0, -1, -1, -1, None);
        let hit = hit.filter(|h| !self.excluded_floor(h.line));
        let vel = match hit {
            Some(h) => Some(
                self.game
                    .geometry
                    .as_ref()
                    .expect("no collision geometry")
                    .line_speed(h.line, h.pos)
                    .expect("a floor mpCheckFloor found that isn't live"),
            ),
            None => self.stage_wind(),
        };
        if let Some(v) = vel {
            let c = &mut self.game.fighters[f.0].cpu;
            c.x54.x = mem(fp::fadds(reg(c.x54.x), reg(v.x)));
            c.x54.y = mem(fp::fadds(reg(c.x54.y), reg(v.y)));
        }
        // `ftCo_CpuUpdateRecoveryScale`.
        if timer % 30 == 0 {
            let r = self.randf();
            let c = &mut self.game.fighters[f.0].cpu;
            c.x570 = mem(fp::frsp(fp::fmadd(
                0.05,
                f64::from(c.level.wrapping_add(1)),
                fp::fmul(0.05, r),
            )));
        }
        self.cpu_reach_scale(f);
        let fpp = &self.game.fighters[f.0];
        if fpp.cpu.xFA_b6 {
            let (x, y) = (fpp.cur_pos.x, fpp.cur_pos.y);
            let top = mem(fp::fadds(1000.0, reg(y)));
            let ceiling = self
                .game
                .geometry
                .as_mut()
                .expect("no collision geometry")
                .check_ceiling(x, y, x, top, -1, -1);
            if ceiling.is_none() {
                self.game.fighters[f.0].cpu.xFA_b6 = false;
            }
        } else if fpp.ground_or_air == GA_GROUND
            && self.corneria_ceiling_floor(fpp.coll_data.floor.index)
        {
            self.game.fighters[f.0].cpu.xFA_b6 = true;
        }
    }

    /// The stage's wind on a point, `grLib_801C9E60`: Rainbow Cruise's, Big Blue's and Icicle
    /// Mountain's stages move it; elsewhere none.
    // Ports `grLib_801C9E60`; aliased once a check reaches it.
    fn stage_wind(&self) -> Option<crate::vec::Vec3> {
        match self.game.stage_info.grkind {
            // Gr_Kind_RCruise, Gr_Kind_BigBlue, Gr_Kind_Icemt.
            3 | 19 | 22 => unported_call("grLib_801C9E60 on a moving stage"),
            _ => None,
        }
    }

    /// Whether a floor is Corneria's ceilinged one (joint 4), `grCorneria_801E2E50`.
    // Ports `grCorneria_801E2E50`; aliased once a check reaches it.
    pub(super) fn corneria_ceiling_floor(&self, line: i32) -> bool {
        // Gr_Kind_Corneria.
        self.game.stage_info.grkind == 14
            && line != -1
            && self
                .game
                .geometry
                .as_ref()
                .expect("no collision geometry")
                .joint_from_line(line)
                == 4
    }

    /// Every 600 frames, how far the fighter reaches (`x56C`) by its kind and a random draw,
    /// `ftCo_800A0CB0`.
    // Ports `ftCo_800A0CB0`; aliased once a check reaches it.
    fn cpu_reach_scale(&mut self, f: FighterId) {
        if self.game.fighters[f.0].cpu.x7C % 600 != 0 {
            return;
        }
        let x = self.randf();
        let x = fp::fmuls(x, fp::fmuls(x, x));
        let r = fp::fsubs(1.0, x);
        let fpp = &mut self.game.fighters[f.0];
        fpp.cpu.x56C = mem(match fpp.kind {
            3 | 5 => fp::frsp(fp::fmul(9.0, r)),
            31 => fp::frsp(fp::fmul(18.0, r)),
            KIND_NANA => fp::frsp(r),
            _ => fp::frsp(fp::fmul(4.0, r)),
        });
    }

    /// The fighter's decision by its computer kind, `ftCo_800B2AFC`: only Nana's is ported.
    /// It returns the r5 the decision leaves, which `ftCo_800B2AFC` passes on unchanged.
    // Ports `ftCo_800B2AFC`; aliased once a check reaches it.
    fn cpu_decide(&mut self, f: FighterId) -> Leftover {
        match self.game.fighters[f.0].cpu.kind {
            CPU_KIND_NANA => self.nana_decide(f),
            k => unported_call(&format!("ftCo_800B2AFC for computer kind {k}")),
        }
    }

    /// Nana's decision, `ftCo_800B101C`: with Popo gone she acts alone; else she records his
    /// inputs, takes her level from his percent, and either follows him (until he's out of
    /// reach or doing his up special) or makes her way back to him. It returns the r5 its
    /// last call leaves: on her way back, always [`Running::nana_keeps_up`]'s.
    // Ports `ftCo_800B101C`; aliased once a check reaches it.
    fn nana_decide(&mut self, f: FighterId) -> Leftover {
        let c = &mut self.game.fighters[f.0].cpu;
        c.xF9_b2 = true;
        c.xF9_b4 = true;
        let Some(popo) = self.cpu_partner(f) else {
            unported_call("ftCo_800AEFB8");
        };
        self.nana_record_input(popo, f);
        let r5;
        let level = fp::fdivs(reg(self.game.fighters[popo.0].dmg.x1830_percent), 20.0);
        let level = if fp::fctiwz(level) > 9 {
            9
        } else {
            fp::fctiwz(level)
        };
        self.game.fighters[f.0].cpu.level = level;
        if self.game.fighters[f.0].cpu.xFA_b7 {
            self.cpu_toward_partner(f);
            // What `ftCo_800A7AAC` leaves (or passes on from `ftCo_800B0918`, Popo's x) depends
            // on the stack and the collision code's registers, which aren't followed: the
            // partner search leaves a fighter's address, the floor checks their own words.
            let (lost, left) = self.nana_lost_popo(f, popo, Leftover::Unknown("ftCo_800A7AAC"));
            r5 = left;
            if lost {
                let c = &mut self.game.fighters[f.0].cpu;
                c.xFA_b7 = false;
                c.x18 = c.x1C;
                c.lstick.x = 0;
                c.lstick.y = 0;
                c.buttons = 0;
                c.ltrigger = 0;
                c.rtrigger = 0;
            }
        } else {
            // `ftCo_CpuUpdateNanaFollow`.
            self.nana_regroup(f);
            let (follow, left) = self.nana_can_follow(f, popo);
            r5 = left;
            if follow {
                let c = &mut self.game.fighters[f.0].cpu;
                c.x18 = 0;
                c.xFA_b7 = true;
                c.xF9_b2 = false;
                c.xF9_b4 = false;
            }
        }
        let fpp = &mut self.game.fighters[f.0];
        if fpp.ground_or_air == GA_GROUND {
            fpp.cpu.xFB_b0 = false;
        }
        r5
    }

    /// Whether Nana stops following Popo (`ftCo_800B101C_inline0`): when she's in her up
    /// special (which marks her `xFB_b0`), more than 25 from him, or not where she can keep
    /// up with him ([`Running::nana_keeps_up`]); with the r5 left, `r5` unless that check ran.
    fn nana_lost_popo(&mut self, f: FighterId, popo: FighterId, r5: Leftover) -> (bool, Leftover) {
        let ms = self.game.fighters[f.0].motion_id;
        if (MS_PP_SPECIAL_HI_0..=MS_PP_SPECIAL_HI_5).contains(&ms) {
            self.game.fighters[f.0].cpu.xFB_b0 = true;
            return (true, r5);
        }
        let (me, other) = (
            self.game.fighters[f.0].cur_pos,
            self.game.fighters[popo.0].cur_pos,
        );
        let dx = fp::fsubs(reg(me.x), reg(other.x));
        let dy = fp::fsubs(reg(other.y), reg(me.y));
        if fp::fmadds(dx, dx, fp::fmuls(dy, dy)) > fp::fmul(25.0, 25.0) {
            return (true, r5);
        }
        let (keeps_up, r5) = self.nana_keeps_up(f, popo);
        (!keeps_up, r5)
    }

    /// Whether Nana starts following Popo again, `ftCo_800B0E98`: both on the ground, she
    /// isn't recovering (`xFB_b0`) or in her up special, she can act or stands waiting, they
    /// move alike (within his walk speed's midpoint) and she's within 25 of him. The r5 it
    /// leaves is [`Running::nana_keeps_up`]'s, which it calls first.
    // Ports `ftCo_800B0E98`; aliased once a check reaches it.
    fn nana_can_follow(&mut self, f: FighterId, popo: FighterId) -> (bool, Leftover) {
        let (keeps_up, r5) = self.nana_keeps_up(f, popo);
        (self.nana_can_follow_from(f, popo, keeps_up), r5)
    }

    /// [`Running::nana_can_follow`] once [`Running::nana_keeps_up`] has answered.
    fn nana_can_follow_from(&mut self, f: FighterId, popo: FighterId, keeps_up: bool) -> bool {
        if !keeps_up {
            return false;
        }
        let (me, other) = (&self.game.fighters[f.0], &self.game.fighters[popo.0]);
        if other.ground_or_air != GA_GROUND || me.ground_or_air != GA_GROUND {
            return false;
        }
        if me.cpu.xFB_b0 || (MS_PP_SPECIAL_HI_0..=MS_PP_SPECIAL_HI_5).contains(&me.motion_id) {
            return false;
        }
        if !self.cpu_should_act(f) && self.game.fighters[f.0].motion_id != MS_WAIT {
            return false;
        }
        let (me, other) = (&self.game.fighters[f.0], &self.game.fighters[popo.0]);
        let x = fp::fsubs(reg(me.pos_delta.x), reg(other.pos_delta.x));
        let y = fp::fsubs(reg(me.pos_delta.y), reg(other.pos_delta.y));
        let r = reg(me.co_attrs.mid_walk_point);
        if fp::fmadds(x, x, fp::fmuls(y, y)) > fp::fmuls(r, r) {
            return false;
        }
        let x = fp::fsubs(reg(me.cur_pos.x), reg(other.cur_pos.x));
        let y = fp::fsubs(reg(other.cur_pos.y), reg(me.cur_pos.y));
        fp::fmadds(x, x, fp::fmuls(y, y)) < fp::fmul(25.0, 25.0)
    }

    /// Whether the computer player may act on its decision (`ftCo_CpuShouldAct`): not while
    /// it's waiting out a state (`x18` 4). A behavior other than its current or fallback one
    /// clears its destination's timer, and acting clears `xFA_b2`.
    pub(super) fn cpu_should_act(&mut self, f: FighterId) -> bool {
        let c = &mut self.game.fighters[f.0].cpu;
        if c.x18 != c.x20 && c.x18 != c.x1C {
            c.x60 = 0;
        }
        if c.x18 == 4 {
            false
        } else {
            c.xFA_b2 = false;
            true
        }
    }

    /// Whether Nana can keep up with Popo where they are, `ftCo_800B0CA8`: not while he hangs
    /// from a ledge, is held, teeters, is swallowed, respawns, rides a Warp Star or floats on
    /// a parasol, nor while she carries a crate, barrel, capsule or barrel cannon, nor when he
    /// stands above her while she falls; she follows on the ground (`x2225_b3`) or when she
    /// crouches or lands.
    ///
    /// It leaves r5 holding Popo's state while it checks his states; then Nana's held item:
    /// none (0), or the item's `Item*`, its address in the original's heap, which the
    /// allocator model gives ([the allocators](docs/engine.md)).
    #[doc(alias = "ftCo_800B0CA8")]
    fn nana_keeps_up(&self, f: FighterId, popo: FighterId) -> (bool, Leftover) {
        let (me, other) = (&self.game.fighters[f.0], &self.game.fighters[popo.0]);
        let ms = other.motion_id;
        let r5 = Leftover::Word(ms as u32);
        // `ftCo_800A3200`: ledge catch or hang.
        if matches!(ms, MS_CLIFF_CATCH | MS_CLIFF_WAIT) {
            return (false, r5);
        }
        if matches!(ms, MS_CAPTURE_WAIT_HI | MS_CAPTURE_WAIT_LW)
            || (MS_SHOULDERED_WAIT..=MS_SHOULDERED_TURN).contains(&ms)
            || matches!(ms, MS_OTTOTTO | MS_OTTOTTO_WAIT)
        {
            return (false, r5);
        }
        if (MS_CAPTURE_KIRBY..=MS_CAPTURE_WAIT_KIRBY).contains(&ms)
            || (MS_REBIRTH..=MS_REBIRTH_WAIT).contains(&ms)
            || (MS_WARP_STAR_JUMP..=MS_WARP_STAR_FALL).contains(&ms)
            || (MS_ITEM_PARASOL_FALL..=MS_ITEM_PARASOL_DAMAGE_FALL).contains(&ms)
        {
            return (false, r5);
        }
        let r5 = match me.item_gobj {
            None => Leftover::Word(0),
            Some(item) => {
                let it = super::super::it::item_of(self.game, item);
                match self.game.items.addr(it.0) {
                    0 => Leftover::Unknown("ftCo_800B0CA8 (an item's address, unmodeled)"),
                    a => Leftover::Word(a),
                }
            }
        };
        if let Some(item) = me.item_gobj {
            let kind = self.game.items[super::super::it::item_of(self.game, item).0].kind;
            // `It_Kind_Box`, `_Taru`, `_Kusudama`, `_TaruCann`.
            if matches!(kind, 1 | 2 | 4 | 5) {
                return (false, r5);
            }
        }
        if other.ground_or_air == GA_GROUND
            && me.ground_or_air == GA_AIR
            && other.cur_pos.y > me.cur_pos.y
            && me.pos_delta.y < 0.0
        {
            return (false, r5);
        }
        if me.x2225_b3 {
            return (true, r5);
        }
        let landing = matches!(
            me.motion_id,
            MS_SQUAT
                | MS_SQUAT_WAIT
                | MS_LANDING
                | MS_LANDING_FALL_SPECIAL
                | MS_LANDING_AIR_N
                | MS_LANDING_AIR_F
                | MS_LANDING_AIR_B
                | MS_LANDING_AIR_HI
                | MS_LANDING_AIR_LW
        );
        (landing, r5)
    }

    /// Records Popo's controller and position into Nana's input history, `ftCo_800B0918`:
    /// both of its positions step on (the write one five ahead of the read one), wrapping.
    // Ports `ftCo_800B0918`; aliased once a check reaches it.
    pub fn nana_record_input(&mut self, popo: FighterId, nana: FighterId) {
        let step = |p: Option<u32>| {
            let at = p.expect("Nana's input history without a position") + INPUT_SIZE;
            if at == INPUTS * INPUT_SIZE { 0 } else { at }
        };
        let p = &self.game.fighters[popo.0];
        // `inlineM0`: a stick axis as a signed byte, 127 a full push right or up, 128 left
        // or down.
        let axis = |x: f32| {
            let r = reg(x);
            if r >= 0.0 {
                fp::fctiwz(fp::fmuls(127.0, r)) as i8
            } else {
                fp::fctiwz(fp::fmuls(128.0, r)) as i8
            }
        };
        let lx = axis(p.input.lstick[0].x);
        let ly = axis(p.input.lstick[0].y);
        let cx = axis(p.input.cstick[0].x);
        let cy = axis(p.input.cstick[0].y);
        let trigger = fp::fctiwz(reg(p.input.triggers[0])) as u8;
        let buttons = p.input.held_buttons[0];
        let (pos, facing) = (p.cur_pos, p.facing_dir);
        let c = &mut self.game.fighters[nana.0].cpu;
        c.x444.0 = Some(step(c.x444.0));
        c.x448.0 = Some(step(c.x448.0));
        let i = &mut c.xFC[(c.x444.0.unwrap() / INPUT_SIZE) as usize];
        i.lstick.x = lx;
        i.lstick.y = ly;
        i.cstick.x = cx;
        i.cstick.y = cy;
        i.x4 = trigger;
        i.x5 = trigger;
        i.x0 = buttons;
        i.cur_pos = pos;
        i.facing_dir = facing;
    }

    /// While Nana follows Popo, his recorded inputs from five frames back become hers,
    /// `ftCo_800B0AF4`, and on the ground she drifts toward where he stood then, facing his
    /// way; she goes into her down special when he's in his.
    // Ports `ftCo_800B0AF4`; aliased once a check reaches it.
    fn nana_follow_inputs(&mut self, f: FighterId) {
        let Some(popo) = self.cpu_partner(f) else {
            return;
        };
        if !self.game.fighters[f.0].cpu.xFA_b7 {
            return;
        }
        let me = self.game.fighters[f.0].motion_id;
        let popo_ms = self.game.fighters[popo.0].motion_id;
        if me != MS_FIRE_FLOWER_SHOOT
            && me != MS_FIRE_FLOWER_SHOOT_AIR
            && popo_ms == MS_PP_SPECIAL_LW
            && me < MS_COUNT
        {
            let facing = self.game.fighters[popo.0].facing_dir;
            self.game.fighters[f.0].facing_dir = mem(fp::fneg(reg(facing)));
            let gobj = self.game.fighters[f.0].gobj.expect("Nana without a GObj");
            self.popo_special_lw_enter(gobj);
            return;
        }
        let fpp = &mut self.game.fighters[f.0];
        let at = fpp
            .cpu
            .x448
            .0
            .expect("Nana's input history without a read position");
        let rec = fpp.cpu.xFC[(at / INPUT_SIZE) as usize].clone();
        let c = &mut fpp.cpu;
        c.lstick.x = rec.lstick.x;
        c.lstick.y = rec.lstick.y;
        c.cstick.x = rec.cstick.x;
        c.cstick.y = rec.cstick.y;
        c.buttons = rec.x0;
        c.ltrigger = rec.x4;
        c.rtrigger = rec.x5;
        if fpp.x2225_b3 {
            fpp.cur_pos.x = mem(fp::frsp(fp::fmadd(
                0.95,
                reg(fpp.cur_pos.x),
                fp::fmul(0.05, reg(rec.cur_pos.x)),
            )));
            fpp.cur_pos.y = mem(fp::frsp(fp::fmadd(
                0.95,
                reg(fpp.cur_pos.y),
                fp::fmul(0.05, reg(rec.cur_pos.y)),
            )));
            if fpp.motion_id != MS_PP_SPECIAL_LW && fpp.motion_id != MS_PP_SPECIAL_AIR_LW {
                fpp.facing_dir = rec.facing_dir;
            }
        }
    }

    /// A new script once the last is done, `ftCo_800B2790`, by what the fighter is doing
    /// (`x18`). `r5` is what its caller left in that register, which the DI reads
    /// ([`Leftover`]); nothing before the DI's call writes it for the characters ported.
    // Ports `ftCo_800B2790`; aliased once a check reaches it.
    fn cpu_behave(&mut self, f: FighterId, r5: Leftover) {
        let c = &self.game.fighters[f.0].cpu;
        if c.csP.0.is_some() || c.command_duration != 0 {
            return;
        }
        let c = &mut self.game.fighters[f.0].cpu;
        c.x80 = c.x80.wrapping_add(1);
        c.xF8_b7 = false;
        self.cpu_script_reset(f);
        self.cpu_charge_release(f);
        use super::cpu_script::cmd;
        match self.game.fighters[f.0].cpu.x18 {
            0 => {
                self.cpu_script_clear(f);
                self.cpu_cmd(f, cmd::DONE);
            }
            1 => self.cpu_walk(f),
            2 => self.cpu_attack_behavior(f),
            3 => unported_call("ftCo_800B9020"),
            4 => self.cpu_recover_behavior(f),
            5 => self.cpu_get_up(f),
            6 => self.cpu_ledge_behavior(f),
            7 => unported_call("ftCo_800BA9A0"),
            8 => unported_call("ftCo_800B7180"),
            9 => self.cpu_hold(f),
            10 => self.cpu_stand(f),
            11 => unported_call("ftCo_800AD42C"),
            12 => unported_call("ftCo_800AD54C"),
            13 => {
                if self.game.fighters[f.0].item_gobj.is_some() {
                    let c = &mut self.game.fighters[f.0].cpu;
                    c.x18 = c.x1C;
                    self.cpu_cmd(f, cmd::DONE);
                } else {
                    // `ftCo_CpuSetNeutralStick` and `ftCo_CpuRetapA`.
                    self.cpu_cmd1(f, cmd::SET_LSTICK_X, 0);
                    self.cpu_cmd1(f, cmd::SET_LSTICK_Y, 0);
                    self.cpu_cmd(f, cmd::RELEASE_A);
                    self.cpu_cmd1(f, cmd::WAIT_FOR, 1);
                    self.cpu_cmd(f, cmd::PRESS_A);
                    self.cpu_cmd1(f, cmd::WAIT_FOR, 1);
                    self.cpu_cmd(f, cmd::RELEASE_A);
                    self.cpu_cmd(f, cmd::DONE);
                    let c = &mut self.game.fighters[f.0].cpu;
                    c.x18 = c.x1C;
                }
            }
            14 => unported_call("ftCo_800AD7FC"),
            15 => unported_call("ftCo_800ABBA8"),
            16 => self.cpu_mash(f),
            17 => unported_call("ftCo_800AC434"),
            18 => self.cpu_di(f, r5),
            19 => {
                let ms = self.game.fighters[f.0].motion_id;
                if ms != 0x131 && ms != 0x132 {
                    let c = &mut self.game.fighters[f.0].cpu;
                    c.x18 = c.x1C;
                    self.cpu_cmd(f, cmd::DONE);
                } else {
                    // `ftCo_CpuFinishWithNeutralStick`.
                    self.cpu_cmd1(f, cmd::SET_LSTICK_X, 0);
                    self.cpu_cmd1(f, cmd::SET_LSTICK_Y, 0);
                    self.cpu_cmd(f, cmd::DONE);
                }
            }
            _ => self.cpu_cmd(f, cmd::DONE),
        }
        self.cpu_script_start(f);
    }

    /// Lets go of a charging special when it's charged, `ftCo_800ADC28`: Donkey Kong's Giant
    /// Punch, Samus's Charge Shot, Mewtwo's Shadow Ball and Kirby's copies of them; not while
    /// it's taking a hit (`x18` 10). Releasing one leaves r5 1 (its last command's argument);
    /// for the characters ported it writes nothing.
    // Ports `ftCo_800ADC28`; aliased once a check reaches it.
    fn cpu_charge_release(&mut self, f: FighterId) {
        let fpp = &self.game.fighters[f.0];
        if fpp.cpu.x18 == 10 {
            return;
        }
        // Ft_Kind_Donkey, Ft_Kind_Kirby, Ft_Kind_Samus, Ft_Kind_Mewtwo.
        if matches!(fpp.kind, 3 | 4 | 13 | 16) {
            unported_call("ftCo_800ADC28 for a character with a charging special");
        }
    }
}
