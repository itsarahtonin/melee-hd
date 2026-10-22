//! A VS match's computer players' decision (`ftCo_0A01.c`, computer kind 4 and the kinds
//! that share it, `ftCo_800B24B8`): off camera it heads back on (`ftCo_800AE7AC`); an enemy
//! with a Starman or a hammer is its target to close in on; else it targets the nearest
//! enemy, goes for an item in reach or heads for the human enemy standing best, and picks its
//! behavior ([`Running::cpu_choose_behavior`]).

use gekko_fp as fp;

use super::super::code::unported_call;
use super::super::gr::{GR_KIND_RCRUISE, greatbay, grlib, ground, stage};
use super::super::handles::FighterId;
use super::super::state::Running;
use super::common::{GA_AIR, GA_GROUND};
use super::cpu_ai::{CPU_FRAME, CPU_FRAME_SP, DECIDE_FRAME, Leftover, P_LINK_FIGHTER};
use crate::fpu::{mem, reg};
use crate::hsd;
use crate::math;
use crate::vec::Vec3;

/// `ftLoadCommonData::x58`: the computer players' data (`Fighter_804D64FC`), and its table of
/// each character's distance threshold (`x20`).
const CPU_DATA: u32 = 0x58;
const DISTANCES: u32 = 0x20;

/// `Gr_Kind_Castle`, `Gr_Kind_Zebes`.
const GR_KIND_CASTLE: i32 = 2;
const GR_KIND_ZEBES: i32 = 8;

/// `Gr_Kind_BigBlue`, `Gr_Kind_Icemt`: stages not on the roster.
const GR_KIND_BIGBLUE: i32 = 0x13;
const GR_KIND_ICEMT: i32 = 0x16;

/// The stack pointer in `ftCo_800B24B8` under the computer player's frame and the decision
/// by kind, and `ftCo_800AE7AC`'s frame size: where the behavior pick's frame lies, whose
/// addresses its floor checks can leave in r5.
const VS_SP: u32 = CPU_FRAME_SP - CPU_FRAME - DECIDE_FRAME - 0x48;
const BACK_ON_SCREEN_FRAME: u32 = 0x68;

/// `Gm_PKind_Cpu`.
const PKIND_CPU: i32 = 1;

impl Running<'_> {
    /// The VS decision, `ftCo_800B24B8`. Every call it makes runs with the fighter's
    /// `CpuFighter` pointer in r31, which [`Running::cpu_choose_behavior`] reads as its DI
    /// switch, and every path ends in that call, so the r5 it leaves is that call's.
    #[doc(alias = "ftCo_800B24B8")]
    pub(super) fn cpu_decide_vs(&mut self, f: FighterId) -> Leftover {
        if let Some((cmd, at)) = self.cpu_off_screen(f) {
            return self.cpu_back_on_screen(f, at, cmd);
        }
        let target = self.cpu_entering_enemy(f);
        self.game.fighters[f.0].cpu.x44 = target;
        if target.is_some() {
            // `ftCo_CpuSetTargetFlags`, then `ftCo_CpuTargetAct`.
            let c = &mut self.game.fighters[f.0].cpu;
            c.xF8_b0 = true;
            c.xF9_b2 = false;
            c.xF9_b4 = false;
            c.xF9_b3 = false;
            c.xF9_b5 = false;
            c.xF9_b6 = false;
            c.xF9_b7 = false;
            c.xF9_b1 = true;
            self.cpu_near_target(f);
            if self.cpu_should_act(f) {
                self.cpu_beside_target(f);
            }
            return self.cpu_choose_behavior_r5(f, true, Some(VS_SP)).1;
        }
        // `ftCo_CpuInitNoTarget` with the fear flags set.
        let c = &mut self.game.fighters[f.0].cpu;
        c.xF8_b0 = false;
        c.xF9_b2 = true;
        c.xF9_b4 = true;
        c.xF9_b3 = true;
        c.xF9_b5 = true;
        c.xF9_b6 = true;
        c.xF9_b7 = true;
        c.xF9_b1 = false;
        let (target, lr) = self.cpu_find_target_lr(f);
        self.game.fighters[f.0].cpu.x44 = target;
        self.cpu_item_targets(f);
        // `ftCo_CpuActOnNoTarget`.
        if self.cpu_should_act(f) {
            if self.game.fighters[f.0].cpu.x4C.is_some() {
                self.cpu_toward_item(f);
                return self.cpu_choose_behavior_r5(f, true, Some(VS_SP)).1;
            }
            let target = self
                .cpu_best_human_enemy(f)
                .or(self.game.fighters[f.0].cpu.x44);
            // What `ftCo_800A75DC`'s vector at `r1 + 0x18` holds before anything writes it:
            // its x the back chain `ftCo_800A4BEC`'s frame left there (the VS decision's
            // stack pointer), its y the word that frame's link register slot was left. The
            // common item search stores over the y only with an item to go for, which takes
            // `ftCo_800A866C` instead; no other call since writes either.
            let unset = lr.map(|y| (f32::from_bits(VS_SP), f32::from_bits(y)));
            self.cpu_toward(f, target, unset);
        }
        self.cpu_choose_behavior_r5(f, true, Some(VS_SP)).1
    }

    /// Whether the fighter is somewhere the stage or camera sends it back from, and where it
    /// is, `ftCo_800A229C`: 2 by Great Bay's turtle; 1 at Peach's Castle's hazard, or on
    /// Brinstar while the acid is above the blast zones' bottom (always), at the bottom's
    /// middle; 2 on Rainbow Cruise near the edges ([`Running::cpu_near_moving_edge`]); -1 off
    /// camera; else none (0).
    #[doc(alias = "ftCo_800A229C")]
    fn cpu_off_screen(&mut self, f: FighterId) -> Option<(i32, Vec3)> {
        let pos = self.game.fighters[f.0].cur_pos;
        if greatbay::turtle_out(self.game) && reg(pos.x) > -5.0 {
            return Some((2, pos));
        }
        match self.game.stage_info.grkind {
            GR_KIND_CASTLE => unported_call("ftCo_800A229C at Peach's Castle (grCastle_801CDF54)"),
            GR_KIND_ZEBES => {
                let (acid, _) = ground::x724_and_step(self.game);
                let bottom = reg(stage::blast_bottom(self.game));
                if reg(acid) > bottom {
                    let right = reg(stage::blast_right(self.game));
                    let left = reg(stage::blast_left(self.game));
                    let x = mem(fp::fmuls(0.5, fp::fadds(left, right)));
                    return Some((1, Vec3::new(x, mem(bottom), 0.0)));
                }
            }
            GR_KIND_RCRUISE => {
                let mut wind = Vec3::new(0.0, 0.0, 0.0);
                grlib::item_wind(self.game, &mut wind);
                if self.cpu_near_moving_edge(pos, wind) {
                    return Some((2, pos));
                }
            }
            // `grLib_801C9E60` stops on both first.
            GR_KIND_BIGBLUE | GR_KIND_ICEMT => {
                unported_call("ftCo_800A229C on Big Blue or Icicle Mountain")
            }
            _ => {}
        }
        if !super::super::camera::in_bounds_by(&self.game.stage_info, pos, 0.0) {
            return Some((-1, pos));
        }
        None
    }

    /// `ftCo_800A229C`'s Rainbow Cruise case: whether `pos` lies in the 40% of the blast
    /// zones' width the wind blows toward, or the 40% of their height it blows toward, or
    /// within 20% of any side.
    fn cpu_near_moving_edge(&self, pos: Vec3, wind: Vec3) -> bool {
        // 0.4f and 0.2f.
        const TWO_FIFTHS: f64 = 0.400_000_005_960_464_5;
        const FIFTH: f64 = 0.200_000_002_980_232_24;
        let (x, y) = (reg(pos.x), reg(pos.y));
        let left = reg(stage::blast_left(self.game));
        let right = reg(stage::blast_right(self.game));
        let w = fp::fsubs(right, left);
        if reg(wind.x) < 0.0 {
            if x < fp::fmadds(TWO_FIFTHS, w, left) {
                return true;
            }
        } else if x > fp::fnmsubs(TWO_FIFTHS, w, right) {
            return true;
        }
        let bottom = reg(stage::blast_bottom(self.game));
        let top = reg(stage::blast_top(self.game));
        let h = fp::fsubs(top, bottom);
        if reg(wind.y) > 0.0 {
            if y > fp::fnmsubs(TWO_FIFTHS, h, top) {
                return true;
            }
        } else if y < fp::fmadds(TWO_FIFTHS, h, bottom) {
            return true;
        }
        let dx = fp::fmuls(FIFTH, w);
        let dy = fp::fmuls(FIFTH, h);
        x < fp::fadds(dx, left)
            || x > fp::fsubs(right, dx)
            || y > fp::fsubs(top, dy)
            || y < fp::fadds(dy, bottom)
    }

    /// Heading back from off camera or the stage's edge, `ftCo_800AE7AC`: the fighter targets
    /// the nearest enemy and the items in reach; from the stage's cases it heads for safe
    /// ground (`ftCo_800A8210`); off camera on the ground it heads toward the blast zones'
    /// middle, or for its target. Its call to the behavior runs with `cmd` in r31, never 0,
    /// as the DI switch.
    #[doc(alias = "ftCo_800AE7AC")]
    fn cpu_back_on_screen(&mut self, f: FighterId, at: Vec3, cmd: i32) -> Leftover {
        let c = &mut self.game.fighters[f.0].cpu;
        c.xF8_b0 = true;
        c.xF9_b3 = false;
        c.xF9_b5 = false;
        c.xF9_b7 = false;
        c.xF9_b1 = false;
        let mark = cmd <= 1;
        c.xF9_b2 = mark;
        c.xF9_b4 = mark;
        c.xF9_b6 = mark;
        let target = self.cpu_find_target(f);
        self.game.fighters[f.0].cpu.x44 = target;
        self.cpu_item_targets(f);
        if self.cpu_should_act(f) {
            if cmd >= 0 {
                self.cpu_toward_safe_ground(f, at);
            } else if self.game.fighters[f.0].ground_or_air != GA_AIR {
                let left = reg(stage::blast_left(self.game));
                let cx = fp::fmuls(0.5, fp::fadds(reg(stage::blast_right(self.game)), left));
                let bottom = reg(stage::blast_bottom(self.game));
                let cy = fp::fmuls(0.5, fp::fadds(reg(stage::blast_top(self.game)), bottom));
                let pos = self.game.fighters[f.0].cur_pos;
                let toward = Vec3::new(
                    mem(fp::fsubs(cx, reg(pos.x))),
                    mem(fp::fsubs(cy, reg(pos.y))),
                    0.0,
                );
                if let Some((x, y)) = self.cpu_island_toward(f, toward) {
                    self.cpu_set_destination(f, x, y, 5.0);
                } else {
                    // `ftCo_800A75DC`'s vector at `r1 + 0x18` lies here where
                    // `ftCo_800A6FC4`'s frame, just gone, saved f20: its high and low words.
                    let f20 = hsd::gobj::PROC_F20.to_bits();
                    let unset = (
                        f32::from_bits((f20 >> 32) as u32),
                        f32::from_bits(f20 as u32),
                    );
                    let target = self.game.fighters[f.0].cpu.x44;
                    self.cpu_toward(f, target, Some(unset));
                }
            }
        }
        self.cpu_choose_behavior_r5(f, cmd != 0, Some(VS_SP - BACK_ON_SCREEN_FRAME))
            .1
    }

    /// Heading for safe ground from the stage's cases, `ftCo_800A8210`, on the ground only:
    /// on Rainbow Cruise (and Big Blue) the nearest island toward the blast zones' middle, or
    /// failing that against the wind ([`Running::cpu_island_toward`]); elsewhere (Brinstar's
    /// acid, Great Bay's turtle) the island end farthest from `from`
    /// ([`Running::cpu_far_island_end`]). It sets the destination as `ftCo_800A1F3C` does
    /// (`ftCo_800A8210_inline0`), within 5.
    #[doc(alias = "ftCo_800A8210")]
    fn cpu_toward_safe_ground(&mut self, f: FighterId, from: Vec3) {
        if self.game.fighters[f.0].ground_or_air == GA_AIR {
            return;
        }
        let found = match self.game.stage_info.grkind {
            GR_KIND_ICEMT => unported_call("ftCo_800A8210 on Icicle Mountain"),
            GR_KIND_RCRUISE | GR_KIND_BIGBLUE => {
                let left = reg(stage::blast_left(self.game));
                let cx = fp::fmuls(0.5, fp::fadds(reg(stage::blast_right(self.game)), left));
                let bottom = reg(stage::blast_bottom(self.game));
                let cy = fp::fmuls(0.5, fp::fadds(reg(stage::blast_top(self.game)), bottom));
                let pos = self.game.fighters[f.0].cur_pos;
                let toward = Vec3::new(
                    mem(fp::fsubs(cx, reg(pos.x))),
                    mem(fp::fsubs(cy, reg(pos.y))),
                    0.0,
                );
                match self.cpu_island_toward(f, toward) {
                    Some(at) => Some(at),
                    None => {
                        let mut wind = Vec3::new(0.0, 0.0, 0.0);
                        grlib::item_wind(self.game, &mut wind);
                        let against = Vec3::new(
                            mem(fp::fneg(reg(wind.x))),
                            mem(fp::fneg(reg(wind.y))),
                            wind.z,
                        );
                        self.cpu_island_toward(f, against)
                    }
                }
            }
            _ => self.cpu_far_island_end(f, from.x, from.y),
        };
        if let Some((x, y)) = found {
            self.cpu_set_destination(f, x, y, 5.0);
        }
    }

    /// The nearest place on another island in the direction `dir` (within 45 degrees) that
    /// computer players count as ground, inside the blast zones by the fighter's half size,
    /// `ftCo_800A6FC4`: each island's middle, then 5 inside its left and right ends, found
    /// within 10 above or below. An island whose middle has no such floor, is one of Rainbow
    /// Cruise's planks or lies off the direction is passed over whole; the ends count only
    /// once some middle has. One random number is drawn for each island but the fighter's own.
    #[doc(alias = "ftCo_800A6FC4")]
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    fn cpu_island_toward(&mut self, f: FighterId, mut dir: Vec3) -> Option<(f32, f32)> {
        let geo = self.game.geometry.as_ref().expect("no collision geometry");
        let own = geo.island_of_line(self.game.fighters[f.0].coll_data.floor.index);
        dir.normalize();
        let mut best = reg(-1.0);
        let mut found = None;
        let mut island = geo.islands.floors;
        while let Some(id) = island {
            let geo = self.game.geometry.as_ref().expect("no collision geometry");
            let isl = *geo.islands.get(id);
            island = isl.next;
            if own == Some(id) {
                continue;
            }
            let (a, b) = (isl.left, isl.right);
            let _ = self.game.objs.rng.draw_f32();
            let mx = mem(fp::fmuls(0.5, fp::fadds(reg(a.x), reg(b.x))));
            let my = fp::fmuls(0.5, fp::fadds(reg(a.y), reg(b.y)));
            let above = mem(fp::frsp(fp::fadd(10.0, my)));
            let below = mem(fp::frsp(fp::fsub(my, 10.0)));
            let hit = self
                .game
                .geometry
                .as_mut()
                .expect("no collision geometry")
                .check_floor(mx, above, mx, below, 0.0, -1, -1, -1, None);
            let Some(hit) = hit.filter(|h| !self.excluded_floor(h.line)) else {
                continue;
            };
            if super::super::gr::rcruise::plank_line(self.game, hit.line) {
                continue;
            }
            let (x, y) = (hit.pos.x, hit.pos.y);
            if !self.cpu_within_45(f, dir, x, y) {
                continue;
            }
            let d = self.cpu_distance_sq(f, x, y);
            if !self.cpu_out_of_bounds(f, x, y) && (best < 0.0 || !(d >= best)) {
                best = d;
                found = Some((x, y));
            }
            let left = mem(fp::frsp(fp::fadd(5.0, reg(a.x))));
            let right = mem(fp::frsp(fp::fsub(reg(b.x), 5.0)));
            for (x, end) in [(left, a), (right, b)] {
                let above = mem(fp::frsp(fp::fadd(10.0, reg(end.y))));
                let below = mem(fp::frsp(fp::fsub(reg(end.y), 10.0)));
                let Some(p) = self.cpu_floor(x, above, x, below) else {
                    break;
                };
                if !self.cpu_within_45(f, dir, p.x, p.y) {
                    break;
                }
                let d = self.cpu_distance_sq(f, p.x, p.y);
                if !self.cpu_out_of_bounds(f, p.x, p.y) && !(d >= best) {
                    best = d;
                    found = Some((p.x, p.y));
                }
            }
        }
        if best >= 0.0 { found } else { None }
    }

    /// Whether `(x, y)` lies within 45 degrees of the unit vector `dir` from the fighter, as
    /// [`Running::cpu_island_toward`] tests it.
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    fn cpu_within_45(&self, f: FighterId, dir: Vec3, x: f32, y: f32) -> bool {
        let pos = self.game.fighters[f.0].cur_pos;
        let mut to = Vec3::new(
            mem(fp::fsubs(reg(x), reg(pos.x))),
            mem(fp::fsubs(reg(y), reg(pos.y))),
            0.0,
        );
        to.normalize();
        // The original's 0.7853982 is this f32.
        !(reg(dir.angle(to)) > reg(std::f32::consts::FRAC_PI_4))
    }

    /// The squared distance from the fighter to `(x, y)`, the squares not fused.
    fn cpu_distance_sq(&self, f: FighterId, x: f32, y: f32) -> f64 {
        let pos = self.game.fighters[f.0].cur_pos;
        let dx = fp::fsubs(reg(x), reg(pos.x));
        let dy = fp::fsubs(reg(y), reg(pos.y));
        fp::fadds(fp::fmuls(dx, dx), fp::fmuls(dy, dy))
    }

    /// The items the fighter goes for: a common one in reach unless it holds an item that
    /// doesn't heal or is entering (`ftCo_CpuUpdateCommonItemTarget`), and a stage enemy
    /// (`ftCo_CpuUpdateSpecialItemTarget`).
    pub(super) fn cpu_item_targets(&mut self, f: FighterId) {
        let fpp = &self.game.fighters[f.0];
        let holds_other = fpp.item_gobj.is_some_and(|g| {
            let it = super::super::it::item_of(self.game, g);
            !self.cpu_item_heals(it)
        });
        let item = if holds_other || fpp.x2168 != 0 {
            None
        } else {
            self.cpu_common_item_target(f, super::cpu_decide::IT_KIND_L_GUN_RAY)
        };
        self.game.fighters[f.0].cpu.x4C = item;
        let x50 = self.cpu_enemy_item_target(f);
        self.game.fighters[f.0].cpu.x50 = x50;
    }

    /// The character's distance threshold (`Fighter_804D64FC->x20[kind]`).
    pub(super) fn cpu_distance_threshold(&self, kind: i32) -> f32 {
        let data = self.common_ptr(CPU_DATA);
        let table = self
            .data
            .ptr(data.at(DISTANCES))
            .expect("the computer players' distance thresholds");
        self.data.f32(table.at(4 * kind as u32))
    }

    /// Whether the fighter stands near its target, `ftCo_800A20A0` (`xF8_b6`): on the ground
    /// within twice its character's distance threshold.
    #[doc(alias = "ftCo_800A20A0")]
    fn cpu_near_target(&mut self, f: FighterId) {
        let fpp = &self.game.fighters[f.0];
        let near = match fpp.cpu.x44 {
            Some(t) if fpp.ground_or_air == GA_GROUND => {
                let limit = fp::fmuls(2.0, reg(self.cpu_distance_threshold(fpp.kind)));
                self.cpu_distance(f, t) < limit
            }
            _ => false,
        };
        self.game.fighters[f.0].cpu.xF8_b6 = near;
    }

    /// Heads for the island end farthest from its target when that target is within 50 and
    /// the fighter on the ground, `ftCo_800A80E4`.
    #[doc(alias = "ftCo_800A80E4")]
    fn cpu_beside_target(&mut self, f: FighterId) {
        let fpp = &self.game.fighters[f.0];
        let Some(t) = fpp.cpu.x44 else { return };
        if fpp.ground_or_air == GA_AIR {
            return;
        }
        let (me, other) = (fpp.cur_pos, self.game.fighters[t.0].cur_pos);
        let dx = fp::fsubs(reg(me.x), reg(other.x));
        let dy = fp::fsubs(reg(me.y), reg(other.y));
        let d = reg(math::sqrt(mem(fp::fmadds(dx, dx, fp::fmuls(dy, dy)))));
        if d > 50.0 {
            return;
        }
        if let Some((x, y)) = self.cpu_far_island_end(f, other.x, other.y) {
            self.cpu_set_destination(f, x, y, 5.0);
        }
    }

    /// The enemy to go for, `ftCo_800A53DC`: when the fighter's coin flips allow it
    /// (`xFA_b34`), the best standing human enemy in reach, the nearer of a tie; else (or
    /// with none) the best standing enemy of any kind.
    #[doc(alias = "ftCo_800A53DC")]
    fn cpu_best_human_enemy(&mut self, f: FighterId) -> Option<FighterId> {
        if self.game.fighters[f.0].cpu.xFA_b34 != 0
            && let Some(o) = self.cpu_best_enemy(f, true)
        {
            return Some(o);
        }
        self.cpu_best_enemy(f, false)
    }

    /// One pass of [`Running::cpu_best_human_enemy`]: enemies in reach, targetable, awake,
    /// visible and alive, humans only if `humans`, by standing (`gm_8016C6C0`), then
    /// distance.
    fn cpu_best_enemy(&mut self, f: FighterId, humans: bool) -> Option<FighterId> {
        let me = self.game.fighters[f.0].gobj;
        let mut best: Option<(FighterId, i32, f64)> = None;
        let mut cur = self.game.world.first_in(P_LINK_FIGHTER);
        while let Some(g) = cur {
            cur = self.game.world.gobjs[g.0].next;
            if me == Some(g) {
                continue;
            }
            let o = self.game.fighter_id(g);
            let pos = self.game.fighters[o.0].cur_pos;
            if self.cpu_out_of_bounds(f, pos.x, pos.y)
                || self.cpu_is_ally(f, o)
                || self.cpu_untargetable(o)
            {
                continue;
            }
            let other = &self.game.fighters[o.0];
            if other.is_sleeping
                || other.stamina_dead
                || self.cpu_cloaked(o)
                || self.game.ft_is_dead(g)
            {
                continue;
            }
            if humans {
                // `ftCo_IsCpuControlled`.
                let other = &self.game.fighters[o.0];
                let pkind = self
                    .game
                    .player_kind(i32::from(other.player_idx), i32::from(other.is_sub_fighter));
                if pkind == PKIND_CPU && other.cpu.kind != 5 {
                    continue;
                }
            }
            let slot = i32::from(self.game.fighters[o.0].player_idx);
            let standing = self.game.player_rank(slot);
            let d = self.cpu_distance(f, o);
            match best {
                None => best = Some((o, standing, d)),
                Some((_, s, bd)) => {
                    if s > standing || (s == standing && bd > d) {
                        best = Some((o, standing, d));
                    }
                }
            }
        }
        best.map(|(o, _, _)| o)
    }

    /// Heads for a fighter, `ftCo_800A75DC`: in the air, for the ground below it (5 inside an
    /// island's end it would stand at), or the island end beside it with none; on the
    /// ground, at its feet, and onto its island's near end from below. Nothing while the
    /// ground is unsafe. The reach is the fighter's own plus half the other's size. `unset`
    /// is what its frame holds where the island end beside goes, which the destination takes
    /// when there is none (the caller's leftover, or none where it isn't followed).
    #[doc(alias = "ftCo_800A75DC")]
    pub(super) fn cpu_toward(
        &mut self,
        f: FighterId,
        target: Option<FighterId>,
        unset: Option<(f32, f32)>,
    ) {
        let Some(t) = target else { return };
        let p = &self.game.fighters[t.0];
        let ppos = p.cur_pos;
        let reach = mem(fp::fadds(
            reg(self.game.fighters[f.0].cpu.x56C),
            reg(p.cpu.x564),
        ));
        if p.ground_or_air == GA_AIR {
            let below = mem(fp::fsubs(reg(ppos.y), 1000.0));
            let above = mem(fp::fadds(10.0, reg(ppos.y)));
            let hit = self
                .game
                .geometry
                .as_mut()
                .expect("no collision geometry")
                .check_floor(ppos.x, above, ppos.x, below, 0.0, -1, -1, -1, None);
            let Some(hit) = hit.filter(|h| !self.excluded_floor(h.line)) else {
                let (x, y) = self.cpu_island_end_beside(t, unset);
                self.cpu_set_destination(f, x, y, reach);
                return;
            };
            let geo = self.game.geometry.as_ref().expect("no collision geometry");
            let island = geo.island_of_line(hit.line);
            if self.cpu_island_unsafe(island) {
                return;
            }
            self.cpu_set_destination(f, hit.pos.x, hit.pos.y, reach);
            let Some(island) = island else { return };
            let isl = *self
                .game
                .geometry
                .as_ref()
                .expect("no collision geometry")
                .islands
                .get(island);
            let dx = reg(self.game.fighters[f.0].cpu.x54.x);
            let mut d = fp::fsubs(reg(isl.right.x), dx);
            if d < 0.0 {
                d = fp::fneg(d);
            }
            if d < 5.0 {
                let x = mem(fp::frsp(fp::fsub(reg(isl.right.x), 5.0)));
                self.cpu_set_destination(f, x, isl.right.y, reach);
            } else {
                let mut d = fp::fsubs(reg(isl.left.x), dx);
                if d < 0.0 {
                    d = fp::fneg(d);
                }
                if d < 5.0 {
                    let x = mem(fp::frsp(fp::fadd(5.0, reg(isl.left.x))));
                    self.cpu_set_destination(f, x, isl.left.y, reach);
                }
            }
            return;
        }
        let geo = self.game.geometry.as_ref().expect("no collision geometry");
        let p_island = geo.island_of_line(p.coll_data.floor.index);
        if self.cpu_island_unsafe(p_island) {
            return;
        }
        self.cpu_set_destination(f, ppos.x, ppos.y, reach);
        let me = &self.game.fighters[f.0];
        let p = &self.game.fighters[t.0];
        let geo = self.game.geometry.as_ref().expect("no collision geometry");
        let same_island = me.ground_or_air != GA_AIR
            && p.ground_or_air != GA_AIR
            && geo
                .island_of_line(me.coll_data.floor.index)
                .is_some_and(|i| geo.island_of_line(p.coll_data.floor.index) == Some(i));
        if same_island {
            return;
        }
        let Some(island) = geo.island_of_line(p.coll_data.floor.index) else {
            return;
        };
        let isl = *geo.islands.get(island);
        let c = &me.cpu;
        if fp::fsubs(reg(c.x54.y), reg(me.cur_pos.y)) <= 0.0 {
            return;
        }
        if fp::fsubs(reg(c.x54.x), reg(me.cur_pos.x)) > 0.0 {
            if me.cur_pos.x < isl.left.x {
                let x = mem(fp::frsp(fp::fadd(5.0, reg(isl.left.x))));
                self.cpu_set_destination(f, x, isl.left.y, reach);
            }
        } else if me.cur_pos.x > isl.right.x {
            let x = mem(fp::frsp(fp::fsub(reg(isl.right.x), 5.0)));
            self.cpu_set_destination(f, x, isl.right.y, reach);
        }
    }

    /// Heads for the item it goes for (`x4C`), `ftCo_800A866C`: with both on the ground and
    /// the item's island safe, for the floor under the item, then onto that island's near end
    /// from below when the fighter stands elsewhere.
    #[doc(alias = "ftCo_800A866C")]
    fn cpu_toward_item(&mut self, f: FighterId) {
        let Some(item) = self.game.fighters[f.0].cpu.x4C else {
            return;
        };
        let ip = &self.game.items[item.0];
        if self.game.fighters[f.0].ground_or_air == GA_AIR || ip.ground_or_air == GA_AIR {
            return;
        }
        let (ix, iy, item_line) = (ip.pos.x, ip.pos.y, ip.x378_itemColl.floor.index);
        let geo = self.game.geometry.as_ref().expect("no collision geometry");
        let island = geo.island_of_line(item_line);
        if self.cpu_island_unsafe(island) {
            return;
        }
        let bottom = mem(fp::fsubs(reg(iy), 50.0));
        let top = mem(fp::fadds(1.0, reg(iy)));
        let hit = self
            .game
            .geometry
            .as_mut()
            .expect("no collision geometry")
            .check_floor(ix, top, ix, bottom, 0.0, -1, -1, -1, None);
        if let Some(hit) = hit.filter(|h| !self.excluded_floor(h.line)) {
            self.cpu_set_destination(f, hit.pos.x, hit.pos.y, 1.0);
        }
        let me = &self.game.fighters[f.0];
        let geo = self.game.geometry.as_ref().expect("no collision geometry");
        // The item is still on the ground: nothing since has moved it.
        let same_island = me.ground_or_air != GA_AIR
            && geo
                .island_of_line(me.coll_data.floor.index)
                .is_some_and(|i| geo.island_of_line(item_line) == Some(i));
        let Some(island) = island else { return };
        // Not above, NaN included.
        #[allow(clippy::neg_cmp_op_on_partial_ord)]
        let below = !(fp::fsubs(reg(me.cpu.x54.y), reg(me.cur_pos.y)) > 0.0);
        if same_island || below {
            return;
        }
        let isl = *geo.islands.get(island);
        if fp::fsubs(reg(me.cpu.x54.x), reg(me.cur_pos.x)) > 0.0 {
            if me.cur_pos.x < isl.left.x {
                let x = mem(fp::frsp(fp::fadd(5.0, reg(isl.left.x))));
                self.cpu_set_destination(f, x, isl.left.y, 1.0);
            }
        } else if me.cur_pos.x > isl.right.x {
            let x = mem(fp::frsp(fp::fsub(reg(isl.right.x), 5.0)));
            self.cpu_set_destination(f, x, isl.right.y, 1.0);
        }
    }

    /// The nearest island end beside a fighter in reach of the blast zones (by its half
    /// size), 5 inside it: a right end to its left or a left end to its right,
    /// `ftCo_800A4768`. The left end's reach is tested at the right end's point, as the
    /// original does. With none the original leaves its caller's vector as it was: `unset`,
    /// or a stop where the caller's leftover isn't followed.
    #[doc(alias = "ftCo_800A4768")]
    fn cpu_island_end_beside(&self, f: FighterId, unset: Option<(f32, f32)>) -> (f32, f32) {
        let pos = self.game.fighters[f.0].cur_pos;
        let geo = self.game.geometry.as_ref().expect("no collision geometry");
        let mut best = reg(-1.0);
        let mut found = None;
        let mut island = geo.islands.floors;
        while let Some(id) = island {
            let isl = *geo.islands.get(id);
            island = isl.next;
            let right = isl.right;
            let in_reach = !self.cpu_out_of_bounds(f, right.x, right.y);
            if in_reach {
                let dx = fp::fsubs(reg(pos.x), reg(right.x));
                if dx > 0.0 {
                    let dy = fp::fsubs(reg(pos.y), reg(right.y));
                    let d = fp::fmadds(dx, dx, fp::fmuls(dy, dy));
                    if best < 0.0 || best > d {
                        best = d;
                        found = Some((mem(fp::frsp(fp::fsub(reg(right.x), 5.0))), right.y));
                    }
                }
                let left = isl.left;
                let dx = fp::fsubs(reg(pos.x), reg(left.x));
                if dx < 0.0 {
                    let dy = fp::fsubs(reg(pos.y), reg(left.y));
                    let d = fp::fmadds(dx, dx, fp::fmuls(dy, dy));
                    if best < 0.0 || best > d {
                        best = d;
                        found = Some((mem(fp::frsp(fp::fadd(5.0, reg(left.x)))), left.y));
                    }
                }
            }
        }
        found.or(unset).unwrap_or_else(|| {
            unported_call("ftCo_800A4768 with no island end (its vector unset, not followed)")
        })
    }
}
