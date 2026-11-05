//! The single-player modes' computer kinds (`ftCo_0A01.c`, `ftCo_800B2AFC`'s other cases):
//! Classic's and the events' opponents and allies. Most start as the VS decision does
//! ([`Running::cpu_decide_start`]) and differ in the fear flags they set, the items they go
//! for and whom they head for.

use gekko_fp as fp;

use super::super::handles::FighterId;
use super::super::state::Running;
use super::common::{GA_AIR, GA_GROUND};
use super::cpu_ai::{CPU_FRAME, CPU_FRAME_SP, DECIDE_FRAME, Leftover, P_LINK_FIGHTER};
use crate::fpu::{mem, reg};
use crate::math;

/// The stack pointer in a decision by kind whose frame is `frame` bytes, under the computer
/// player's frame and `ftCo_800B2AFC`'s.
const fn kind_sp(frame: u32) -> u32 {
    DECIDE_SP - frame
}

/// The stack pointer in `ftCo_800B2AFC`, the decision by kind.
const DECIDE_SP: u32 = CPU_FRAME_SP - CPU_FRAME - DECIDE_FRAME;

/// `ftCo_800AECF0`'s frame size: computer kind 8's decision, which kinds 7, 10 and 18 call
/// for a fighter with a Starman or a hammer.
const NO_HUMAN_FRAME: u32 = 0x48;

impl Running<'_> {
    /// The items a fighter goes for that heals: a healing item in reach unless it holds an
    /// item that doesn't heal or is entering (`ftCo_800A61D8`), and a stage enemy
    /// (`ftCo_CpuUpdateSpecialItemTarget`), as computer kinds 2 and 9 look.
    pub(super) fn cpu_healing_item_targets(&mut self, f: FighterId) {
        let fpp = &self.game.fighters[f.0];
        let holds_other = fpp.item_gobj.is_some_and(|g| {
            let it = super::super::it::item_of(self.game, g);
            !self.cpu_item_heals(it)
        });
        let item = if holds_other || fpp.x2168 != 0 {
            None
        } else {
            self.cpu_healing_item_target(f)
        };
        self.game.fighters[f.0].cpu.x4C = item;
        let x50 = self.cpu_enemy_item_target(f);
        self.game.fighters[f.0].cpu.x50 = x50;
    }

    /// Computer kind 9, `ftCo_800B00F8` (Classic's Very Easy opponents; Events 4, 15, 25, 27
    /// and 39): the VS decision's start; else the nearest enemy its target with the fear
    /// flags as they were, a healing item to go for and a stage enemy; while it may act, for
    /// the item, or once a second a coin flip for another island and a coin flip for each of
    /// its fear flags; then the behavior pick, whose r5 it leaves. Its r31 holds its
    /// `CpuFighter` pointer throughout, the pick's DI switch.
    #[doc(alias = "ftCo_800B00F8")]
    pub(super) fn cpu_decide_wanderer(&mut self, f: FighterId) -> Leftover {
        let sp = kind_sp(0x48);
        if let Some(r5) = self.cpu_decide_start(f, sp) {
            return r5;
        }
        let target = self.cpu_find_target(f);
        self.game.fighters[f.0].cpu.x44 = target;
        self.cpu_healing_item_targets(f);
        if self.cpu_should_act(f) {
            if self.game.fighters[f.0].cpu.x4C.is_some() {
                self.cpu_toward_item(f);
            } else if self.game.fighters[f.0]
                .cpu
                .x7C
                .wrapping_rem(60)
                .wrapping_mul(5)
                == 0
            {
                if self.randf() < 0.5 {
                    self.cpu_to_other_island(f);
                }
                let b0 = self.randf() > 0.5;
                self.game.fighters[f.0].cpu.xF8_b0 = b0;
                let b2 = self.randf() > 0.5;
                self.game.fighters[f.0].cpu.xF9_b2 = b2;
                let b4 = self.randf() > 0.5;
                self.game.fighters[f.0].cpu.xF9_b4 = b4;
                let b3 = self.randf() > 0.5;
                self.game.fighters[f.0].cpu.xF9_b3 = b3;
                let b5 = self.randf() > 0.5;
                self.game.fighters[f.0].cpu.xF9_b5 = b5;
                let b6 = self.randf() > 0.5;
                self.game.fighters[f.0].cpu.xF9_b6 = b6;
                let b7 = self.randf() > 0.5;
                let c = &mut self.game.fighters[f.0].cpu;
                c.xF9_b7 = b7;
                c.xF9_b1 = false;
            }
        }
        self.cpu_choose_behavior_r5(f, true, Some(sp)).1
    }

    /// The fear flags a decision sets when no enemy enters: `xF8_b0`, then `xF9`'s bits 2,
    /// 4, 3, 5, 6 and 7 as `flags` lists them, and `xF9_b1` cleared
    /// (`ftCo_CpuInitNoTarget` and its copies).
    fn cpu_fear_flags(&mut self, f: FighterId, b0: bool, flags: [bool; 6]) {
        let c = &mut self.game.fighters[f.0].cpu;
        c.xF8_b0 = b0;
        c.xF9_b2 = flags[0];
        c.xF9_b4 = flags[1];
        c.xF9_b3 = flags[2];
        c.xF9_b5 = flags[3];
        c.xF9_b6 = flags[4];
        c.xF9_b7 = flags[5];
        c.xF9_b1 = false;
    }

    /// Heads for `target` (`ftCo_800A75DC`) from a decision whose stack pointer is `sp` and
    /// whose target search left `lr` in its frame's link slot: the vector `ftCo_800A75DC`
    /// keeps at `r1 + 0x18` lies where `ftCo_800A4BEC`'s frame was (0x18 smaller, from the
    /// same caller), its back chain (`sp`) and that slot, as in the VS decision: none of the
    /// item searches, `ftCo_800A53DC`, `ftCo_800A5294` and `ftCo_800A1AB4` between them
    /// writes either word.
    fn cpu_toward_from(
        &mut self,
        f: FighterId,
        target: Option<FighterId>,
        sp: u32,
        lr: Option<u32>,
    ) {
        let unset = lr.map(|y| (f32::from_bits(sp), f32::from_bits(y)));
        self.cpu_toward(f, target, unset);
    }

    /// Computer kind 8, `ftCo_800AECF0` (Events 11, 14, 27, 44 and 46): the VS decision
    /// without its turn to the human enemy standing best: the VS decision's start; else the
    /// nearest enemy its target with every fear flag set, the items in reach, and while it
    /// may act, for the item it goes for or toward its target; then the behavior pick, whose
    /// r5 it leaves. Kinds 7, 10 and 18 call it from their own frames: `caller` is the stack
    /// pointer it's called with.
    #[doc(alias = "ftCo_800AECF0")]
    pub(super) fn cpu_decide_no_human(&mut self, f: FighterId, caller: u32) -> Leftover {
        let sp = caller - NO_HUMAN_FRAME;
        if let Some(r5) = self.cpu_decide_start(f, sp) {
            return r5;
        }
        self.cpu_fear_flags(f, false, [true; 6]);
        let (target, lr) = self.cpu_find_target_lr(f);
        self.game.fighters[f.0].cpu.x44 = target;
        self.cpu_item_targets(f);
        if self.cpu_should_act(f) {
            if self.game.fighters[f.0].cpu.x4C.is_some() {
                self.cpu_toward_item(f);
            } else {
                let target = self.game.fighters[f.0].cpu.x44;
                self.cpu_toward_from(f, target, sp, lr);
            }
        }
        self.cpu_choose_behavior_r5(f, true, Some(sp)).1
    }

    /// Whether the fighter stands near its target (`xF8_b6`), `ftCo_CpuUpdateTargetDistance`:
    /// never while it can't be targeted itself (`ftCo_800A1C44`); else on the ground within
    /// its character's distance threshold.
    fn cpu_near_target_kind(&mut self, f: FighterId) {
        let near = if self.cpu_untargetable(f) {
            false
        } else {
            let fpp = &self.game.fighters[f.0];
            match fpp.cpu.x44 {
                Some(t) if fpp.ground_or_air == GA_GROUND => {
                    let limit = reg(self.cpu_distance_threshold(fpp.kind));
                    self.cpu_distance(f, t) < limit
                }
                _ => false,
            }
        };
        self.game.fighters[f.0].cpu.xF8_b6 = near;
    }

    /// Whether the fighter has a Starman (`x221D_b6`) or a hammer out of a ride (`x2168`
    /// with no `x2338.x`): computer kinds 7, 10 and 18 then decide as kind 8.
    fn cpu_powered_up(&self, f: FighterId) -> bool {
        let fpp = &self.game.fighters[f.0];
        fpp.x221D_b6 || (fpp.x2168 != 0 && fpp.x2338.x == 0)
    }

    /// The first fighter of player `player` in the fighters' list other than this one, in
    /// reach of the blast zones and not asleep, out of stamina, invisible or dead,
    /// `ftCo_800A5294`.
    #[doc(alias = "ftCo_800A5294")]
    fn cpu_player_fighter(&self, f: FighterId, player: i32) -> Option<FighterId> {
        let me = self.game.fighters[f.0].gobj;
        let mut cur = self.game.world.first_in(P_LINK_FIGHTER);
        while let Some(g) = cur {
            cur = self.game.world.gobjs[g.0].next;
            if me == Some(g) {
                continue;
            }
            let o = self.game.fighter_id(g);
            let other = &self.game.fighters[o.0];
            if self.cpu_out_of_bounds(f, other.cur_pos.x, other.cur_pos.y) {
                continue;
            }
            if other.is_sleeping
                || other.stamina_dead
                || self.cpu_cloaked(o)
                || self.game.ft_is_dead(g)
            {
                continue;
            }
            if i32::from(other.player_idx) == player {
                return Some(o);
            }
        }
        None
    }

    /// Computer kinds 19 to 22, `ftCo_800AFE3C` with player 0 to 3 (Events 17, 19 and 25):
    /// the VS decision's start; else the nearest enemy its target with every fear flag set
    /// and the items in reach, and while it may act, toward that player's fighter
    /// ([`Running::cpu_player_fighter`]) or, with none, its target, never for an item; then
    /// the behavior pick, whose r5 it leaves.
    #[doc(alias = "ftCo_800AFE3C")]
    pub(super) fn cpu_decide_player(&mut self, f: FighterId, player: i32) -> Leftover {
        let sp = kind_sp(0x50);
        if let Some(r5) = self.cpu_decide_start(f, sp) {
            return r5;
        }
        self.cpu_fear_flags(f, false, [true; 6]);
        let (target, lr) = self.cpu_find_target_lr(f);
        self.game.fighters[f.0].cpu.x44 = target;
        self.cpu_item_targets(f);
        if self.cpu_should_act(f) {
            let to = self
                .cpu_player_fighter(f, player)
                .or(self.game.fighters[f.0].cpu.x44);
            self.cpu_toward_from(f, to, sp, lr);
        }
        self.cpu_choose_behavior_r5(f, true, Some(sp)).1
    }

    /// Computer kind 18, `ftCo_800AF78C` (Events 6 and 26), and kind 7, `ftCo_800AF290`,
    /// which stands its ground instead ([`Running::cpu_stand_ground`]) once its target is
    /// farther than `x40`: the VS decision's start; with a Starman or a hammer, kind 8's
    /// decision; else the nearest enemy its target with the flags set but `xF9_b3`, the
    /// items in reach, whether it's near its target, and while it may act, for the item it
    /// goes for, or on the ground within 50 of its target, the island end farthest from it;
    /// then the behavior pick, whose r5 it leaves.
    #[doc(alias = "ftCo_800AF78C")]
    // Ports `ftCo_800AF290` too; aliased once a check reaches computer kind 7.
    pub(super) fn cpu_decide_keep_away(&mut self, f: FighterId, stand: bool) -> Leftover {
        let sp = kind_sp(if stand { 0x78 } else { 0x70 });
        if let Some(r5) = self.cpu_decide_start(f, sp) {
            return r5;
        }
        if self.cpu_powered_up(f) {
            return self.cpu_decide_no_human(f, sp);
        }
        self.cpu_fear_flags(f, true, [true, true, false, true, true, true]);
        let target = self.cpu_find_target(f);
        self.game.fighters[f.0].cpu.x44 = target;
        self.cpu_item_targets(f);
        self.cpu_near_target_kind(f);
        if stand
            && let Some(t) = self.game.fighters[f.0].cpu.x44
            && self.cpu_distance(f, t) > reg(self.game.fighters[f.0].cpu.x40)
        {
            return self.cpu_stand_ground(f, sp);
        }
        if self.cpu_should_act(f) {
            if self.game.fighters[f.0].cpu.x4C.is_some() {
                self.cpu_toward_item(f);
            } else if let Some(t) = self.game.fighters[f.0].cpu.x44
                && self.game.fighters[f.0].ground_or_air != GA_AIR
            {
                let (me, them) = (
                    self.game.fighters[f.0].cur_pos,
                    self.game.fighters[t.0].cur_pos,
                );
                let dy = fp::fsubs(reg(me.y), reg(them.y));
                let dx = fp::fsubs(reg(me.x), reg(them.x));
                let dist = math::sqrt(mem(fp::fmadds(dx, dx, fp::fmuls(dy, dy))));
                let far = reg(dist) > 50.0;
                if !far && let Some((x, y)) = self.cpu_far_island_end(f, them.x, them.y) {
                    self.cpu_set_destination(f, x, y, 5.0);
                }
            }
        }
        self.cpu_choose_behavior_r5(f, true, Some(sp)).1
    }

    /// Computer kind 7's stand, `ftCo_800AEA8C`: the nearest enemy its target with the flags
    /// set but `xF8_b0` and `xF9_b3`, the items in reach, and while it may act, the floor
    /// under it as its destination; then the behavior pick, whose r5 it leaves. `caller` is
    /// kind 7's stack pointer.
    // Ports `ftCo_800AEA8C`; aliased once a check reaches computer kind 7.
    fn cpu_stand_ground(&mut self, f: FighterId, caller: u32) -> Leftover {
        let sp = caller - 0x68;
        self.cpu_fear_flags(f, false, [true, true, false, true, true, true]);
        let target = self.cpu_find_target(f);
        self.game.fighters[f.0].cpu.x44 = target;
        self.cpu_item_targets(f);
        if self.cpu_should_act(f) {
            let at = self.game.fighters[f.0].cur_pos;
            let below = mem(fp::fsubs(reg(at.y), 1000.0));
            let above = mem(fp::fadds(10.0, reg(at.y)));
            if let Some(p) = self.cpu_floor(at.x, above, at.x, below) {
                self.cpu_set_destination(f, p.x, p.y, 5.0);
            }
        }
        self.cpu_choose_behavior_r5(f, true, Some(sp)).1
    }

    /// Computer kind 13, `ftCo_800B126C` (Events 7 and 37): without the VS decision's start,
    /// the nearest enemy its target with `xF8_b0` and the fear flags 5 to 7 set, any item
    /// worth going for (`x2C` 1), the items in reach, and while it may act, for the item it
    /// goes for when its hands are free, else once a second a coin flip for another island;
    /// then the behavior pick, whose r5 it leaves.
    #[doc(alias = "ftCo_800B126C")]
    pub(super) fn cpu_decide_roamer(&mut self, f: FighterId) -> Leftover {
        let sp = kind_sp(0x38);
        self.cpu_fear_flags(f, true, [false, false, false, true, true, true]);
        self.game.fighters[f.0].cpu.x2C = 1;
        let target = self.cpu_find_target(f);
        self.game.fighters[f.0].cpu.x44 = target;
        self.cpu_item_targets(f);
        if self.cpu_should_act(f) {
            let fpp = &self.game.fighters[f.0];
            if fpp.cpu.x4C.is_some() && fpp.item_gobj.is_none() {
                self.cpu_toward_item(f);
            } else if fpp.cpu.x7C.wrapping_rem(60).wrapping_mul(5) == 0 && self.randf() < 0.5 {
                self.cpu_to_other_island(f);
            }
        }
        self.cpu_choose_behavior_r5(f, true, Some(sp)).1
    }

    /// While a VS-like decision may act: for the item it goes for (with `hands_free`, only
    /// while it holds none), else toward the human enemy standing best (`ftCo_800A53DC`) or,
    /// with none, its target ([`Running::cpu_toward_from`]).
    fn cpu_go_for_enemy(&mut self, f: FighterId, sp: u32, lr: Option<u32>, hands_free: bool) {
        if !self.cpu_should_act(f) {
            return;
        }
        let fpp = &self.game.fighters[f.0];
        if fpp.cpu.x4C.is_some() && !(hands_free && fpp.item_gobj.is_some()) {
            self.cpu_toward_item(f);
            return;
        }
        let to = self
            .cpu_best_human_enemy(f)
            .or(self.game.fighters[f.0].cpu.x44);
        self.cpu_toward_from(f, to, sp, lr);
    }

    /// The nearest ally of the fighter's in reach of the blast zones and not asleep, out of
    /// stamina, invisible or dead, `ftCo_800A50D4`, with the two words its calls leave where
    /// `ftCo_800A4BEC`'s frame was ([`Running::cpu_toward_from`]), when any call wrote them:
    /// `ftCo_IsAlly` and `ftCo_800A0F00` save r30 and r31 there (r31 the fighter's
    /// `CpuFighter` pointer; r30 at first the caller's, the fighter's address, then the last
    /// ally's GObj address, or the last candidate's `Fighter*` after a distance), and
    /// `sqrtf` stores each distance over the second. An address the allocator model doesn't
    /// give is [`Leftover::Unknown`].
    #[doc(alias = "ftCo_800A50D4")]
    fn cpu_nearest_ally(&self, f: FighterId) -> (Option<FighterId>, Option<(Leftover, Leftover)>) {
        let fighter_addr = |id: FighterId| match self.game.ft.addresses.get(&id.0) {
            Some(&a) if a != 0 => Leftover::Word(a),
            _ => Leftover::Unknown("ftCo_800A50D4 (a fighter's address, unmodeled)"),
        };
        let gobj_addr = |g: crate::hsd::gobj::GObjId| match self.game.world.gobjs.addr(g.0) {
            0 => Leftover::Unknown("ftCo_800A50D4 (a GObj's address, unmodeled)"),
            a => Leftover::Word(a),
        };
        let cpu_ptr = match fighter_addr(f) {
            Leftover::Word(a) => Leftover::Word(a + 0x1A88),
            u => u,
        };
        let mut r30 = fighter_addr(f);
        let mut slots = None;
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
            if self.cpu_out_of_bounds(f, other.cur_pos.x, other.cur_pos.y) {
                continue;
            }
            slots = Some((r30, cpu_ptr));
            if !self.cpu_is_ally(f, o) {
                continue;
            }
            r30 = gobj_addr(g);
            if other.is_sleeping || other.stamina_dead {
                continue;
            }
            slots = Some((r30, cpu_ptr));
            if self.cpu_cloaked(o) || self.game.ft_is_dead(g) {
                continue;
            }
            if closest.is_some() {
                r30 = fighter_addr(o);
            }
            let d = self.cpu_distance(f, o);
            slots = slots.map(|(x, _)| (x, Leftover::Word(mem(d).to_bits())));
            match closest {
                None => closest = Some((o, d)),
                Some((_, best)) if best > d => closest = Some((o, d)),
                _ => {}
            }
        }
        (closest.map(|(o, _)| o), slots)
    }

    /// Computer kind 10, `ftCo_800AFC40` (Events 15 and 48): without the VS decision's start;
    /// with a Starman or a hammer, kind 8's decision; else the nearest enemy its target with
    /// `xF8_b0` and the fear flags set but `xF9_b7`, the items in reach, and while it may act,
    /// toward its nearest ally ([`Running::cpu_nearest_ally`]) or, with none, its target;
    /// then the behavior pick, whose r5 it leaves.
    #[doc(alias = "ftCo_800AFC40")]
    pub(super) fn cpu_decide_follower(&mut self, f: FighterId) -> Leftover {
        let sp = kind_sp(0x40);
        if self.cpu_powered_up(f) {
            return self.cpu_decide_no_human(f, sp);
        }
        self.cpu_fear_flags(f, true, [true, true, true, true, true, false]);
        let (target, lr) = self.cpu_find_target_lr(f);
        self.game.fighters[f.0].cpu.x44 = target;
        self.cpu_item_targets(f);
        if self.cpu_should_act(f) {
            let (ally, slots) = self.cpu_nearest_ally(f);
            let to = ally.or(self.game.fighters[f.0].cpu.x44);
            match slots {
                None => self.cpu_toward_from(f, to, sp, lr),
                Some((Leftover::Word(x), Leftover::Word(y))) => {
                    self.cpu_toward(f, to, Some((f32::from_bits(x), f32::from_bits(y))));
                }
                Some(_) => self.cpu_toward(f, to, None),
            }
        }
        self.cpu_choose_behavior_r5(f, true, Some(sp)).1
    }

    /// Computer kind 14, `ftCo_800B1478` (Event 9): the VS decision without its way back on
    /// camera, `xF8_b0` set with the fear flags, and whether it's near its target: an enemy
    /// with a Starman or a hammer to close in on; else the nearest enemy its target, the items
    /// in reach, and while it may act, for the item it goes for or toward the human enemy
    /// standing best; then the behavior pick, whose r5 it leaves.
    #[doc(alias = "ftCo_800B1478")]
    pub(super) fn cpu_decide_bold(&mut self, f: FighterId) -> Leftover {
        let sp = kind_sp(0x50);
        if let Some(r5) = self.cpu_decide_entering(f, sp) {
            return r5;
        }
        self.cpu_fear_flags(f, true, [true; 6]);
        let (target, lr) = self.cpu_find_target_lr(f);
        self.game.fighters[f.0].cpu.x44 = target;
        self.cpu_item_targets(f);
        self.cpu_near_target_kind(f);
        self.cpu_go_for_enemy(f, sp, lr, false);
        self.cpu_choose_behavior_r5(f, true, Some(sp)).1
    }

    /// Computer kind 17, `ftCo_800B17D0` (Event 13): the VS decision with any item worth going
    /// for (`x2C` 1), and only while its hands are free; then the behavior pick, whose r5 it
    /// leaves.
    #[doc(alias = "ftCo_800B17D0")]
    pub(super) fn cpu_decide_collector(&mut self, f: FighterId) -> Leftover {
        let sp = kind_sp(0x48);
        if let Some(r5) = self.cpu_decide_start(f, sp) {
            return r5;
        }
        self.cpu_fear_flags(f, false, [true; 6]);
        self.game.fighters[f.0].cpu.x2C = 1;
        let (target, lr) = self.cpu_find_target_lr(f);
        self.game.fighters[f.0].cpu.x44 = target;
        self.cpu_item_targets(f);
        self.cpu_go_for_enemy(f, sp, lr, true);
        self.cpu_choose_behavior_r5(f, true, Some(sp)).1
    }

    /// Computer kind 28, `ftCo_800B21C8` (Event 37): the VS decision's start; else the nearest
    /// enemy its target with the fear flags set but `xF9_b7`, a stage enemy but no common
    /// item (`x4C` kept as it was), whether it's near its target, and while it may act,
    /// toward the human enemy standing best or its target; then the behavior pick, whose r5
    /// it leaves, with 0 in r31 (its DI switch off).
    #[doc(alias = "ftCo_800B21C8")]
    pub(super) fn cpu_decide_fighter(&mut self, f: FighterId) -> Leftover {
        let sp = kind_sp(0x40);
        if let Some(r5) = self.cpu_decide_start(f, sp) {
            return r5;
        }
        self.cpu_fear_flags(f, false, [true, true, true, true, true, false]);
        let (target, lr) = self.cpu_find_target_lr(f);
        self.game.fighters[f.0].cpu.x44 = target;
        let x50 = self.cpu_enemy_item_target(f);
        self.game.fighters[f.0].cpu.x50 = x50;
        self.cpu_near_target_kind(f);
        if self.cpu_should_act(f) {
            let to = self
                .cpu_best_human_enemy(f)
                .or(self.game.fighters[f.0].cpu.x44);
            self.cpu_toward_from(f, to, sp, lr);
        }
        self.cpu_choose_behavior_r5(f, false, Some(sp)).1
    }
}
