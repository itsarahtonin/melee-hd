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
use crate::game::ft::FighterKind;
use crate::game::gr::GrKind;
use crate::game::it::ItKind;
use crate::hsd::gobj::GObjId;
use crate::math;

/// `HSD_GOBJ_PLINK_FIGHTER`.
pub(super) const P_LINK_FIGHTER: u8 = 8;

super::registry::kind_enum! {
    /// A computer player's kind (`CpuKind`, the players' `cpu_type`, `CpuFighter::kind`), which
    /// picks its decision (`ftCo_800B2AFC`'s switch). The game's own numbers; the names are the
    /// debug menu's for 0 to 17 (`CpTp_*`) and the decisions' for the single-player modes'.
    CpuKind:
    /// `CpTp_Stay`: Training's, standing where it is.
    Stay = 0,
    /// `CpTp_Walk`: Training's, walking to its spot.
    Walk = 1,
    /// `CpTp_Escape`: Training's evading one (`ftCo_800B04DC`).
    Escape = 2,
    /// `CpTp_Jump`: Training's, jumping.
    Jump = 3,
    /// `CpTp_Normal`: the VS kind.
    Normal = 4,
    /// `CpTp_Manual`.
    Manual = 5,
    /// `CpTp_Nana`, `CpuKind_Nana`: Nana following Popo.
    Nana = 6,
    /// `CpTp_Defensive` (`ftCo_800AF290`).
    Defensive = 7,
    /// `CpTp_Struggle` (`ftCo_800AECF0`).
    Struggle = 8,
    /// `CpTp_Freak` (`ftCo_800B00F8`): Classic's Very Easy opponents.
    Freak = 9,
    /// `CpTp_Cooperate` (`ftCo_800AFC40`): toward its nearest ally.
    Cooperate = 10,
    /// `CpTp_SpLwLink`.
    SpLwLink = 11,
    /// `CpTp_SpLwSamus`.
    SpLwSamus = 12,
    /// `CpTp_OnlyItem` (`ftCo_800B126C`).
    OnlyItem = 13,
    /// `CpTp_EvZelda` (`ftCo_800B1478`).
    EvZelda = 14,
    /// `CpTp_NoAct`: the Home-Run Contest's.
    NoAct = 15,
    /// `CpTp_Air`: jumping at its target.
    Air = 16,
    /// `CpTp_Item` (`ftCo_800B17D0`): any item worth going for.
    Item = 17,
    /// Kind 7's decision standing its ground (`ftCo_800AF78C`, Events 6 and 26).
    KeepAway = 18,
    /// Toward player 1's to player 4's fighter (`ftCo_800AFE3C`, Events 17, 19 and 25).
    TowardP1 = 19,
    TowardP2 = 20,
    TowardP3 = 21,
    TowardP4 = 22,
    /// The single-player opponents': toward the nearest enemy.
    Enemy = 23,
    /// Multi-Man's Wire Frames' (`ftCo_800B1DA0`): to another island now and then.
    Wander = 24,
    /// Kind 23's decision again (Multi-Man's Wire Frames).
    EnemyAgain = 25,
    /// Kind 23's target, Training's floor under it: nothing gives it (`cpu_audit`'s `KIND_26`).
    Unused26 = 26,
    /// A metal or giant single-player opponent's (`ftCo_800B1EF0`, Adventure's giant Kirby).
    Unafraid = 27,
    /// Toward the human enemy standing best (`ftCo_800B21C8`, Event 37).
    Fighter = 28,
    /// Items worth going for (`ftCo_800B1AB8`, Adventure's Nesses on Onett).
    ItemLover = 29,
}

impl CpuKind {
    /// Its number as a byte, as the players and a match's start data hold it.
    pub const fn byte(self) -> u8 {
        self as u8
    }
}

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

/// The stack pointer at the computer player's frame's entry (`ftCo_800B3900`, which only
/// `Fighter_8006ABA0` calls, from the fighters' input process): the same in every saved call,
/// level 1 replays' and level 9 oracle matches'. With the frame sizes of the functions it
/// calls, it gives the stack addresses a register can be left holding ([`Leftover`]).
pub(super) const CPU_FRAME_SP: u32 = 0x804E_EAB0;

/// The frame sizes of `ftCo_800B3900` and `ftCo_800B2AFC`, the decision by computer kind.
pub(super) const CPU_FRAME: u32 = 0x20;
pub(super) const DECIDE_FRAME: u32 = 0xC8;
/// The frame size of `ftCo_800B04DC`, training's computer kind 2.
const EVADE_FRAME: u32 = 0x58;

/// `Fighter_x1A88_xFC_t`'s size and how many the history holds.
const INPUT_SIZE: u32 = 0x1C;
const INPUTS: u32 = 30;

/// What a register holds when code reads it without its caller passing it: the stale-register
/// rule for the CPU AI (docs/engine.md). The computer player's frame follows one such value,
/// r5, which Nana's DI takes as her stick's x when her knockback is zero (`ftCo_800AC5A0`):
/// each function on the way returns the r5 it leaves, as the original's machine code leaves
/// it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub(super) enum Leftover {
    /// The register's word.
    Word(u32),
    /// A word melee-hd doesn't follow, named by the function that left it: reading it stops.
    Unknown(&'static str),
}

impl Running<'_> {
    /// Whether the rules run Slippi 3.19's NanaDeterminism (`800AC5B8`), which clears r5 as
    /// well as r30 before the DI with no knockback reads them; 3.16's clears r30 alone.
    pub(super) fn nana_determinism_clears_r5(&self) -> bool {
        !self.game.rules.follows_r5()
    }

    /// The other fighter of the same player (Nana's Popo, Popo's Nana), `ftCo_800A589C`: the
    /// first in the fighters' list, or none if that one is asleep or there is none.
    #[doc(alias = "ftCo_800A589C")]
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
    pub(super) fn randf(&mut self) -> f64 {
        reg(self.game.objs.rng.draw_f32())
    }

    /// A computer-controlled fighter's frame, `ftCo_800B3900`: it reads the stage, decides by
    /// its kind, writes a new script when the last one is done, runs its script, and Nana
    /// replays Popo's inputs while she follows him; its frame count goes up.
    ///
    /// Under Slippi 3.19's NanaDeterminism the DI clears r5 before it reads it
    /// ([`Running::nana_determinism_clears_r5`]), so the r5 the decision leaves is never
    /// consulted: it goes on as a word nothing may read, and anything that reads it stops by
    /// name (the Slippi ruleset's tripwire, docs/slippi-ruleset.md).
    #[doc(alias = "ftCo_800B3900")]
    pub fn cpu_frame(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.cpu_read_stage(f);
        let r5 = self.cpu_decide(f);
        let r5 = if self.nana_determinism_clears_r5() {
            Leftover::Unknown("ftCo_800B3900 under 3.19's NanaDeterminism")
        } else {
            r5
        };
        self.cpu_behave(f, r5);
        self.cpu_run_script(f);
        self.nana_follow_inputs(f);
        let c = &mut self.game.fighters[f.0].cpu;
        c.frame_counter = c.frame_counter.wrapping_add(1);
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
    #[doc(alias = "ftCo_800B33B0")]
    fn cpu_read_stage(&mut self, f: FighterId) {
        let timer = self.game.fighters[f.0].cpu.frame_counter;
        if timer % 300 == 0 {
            let r = self.randf();
            let c = &mut self.game.fighters[f.0].cpu;
            let p = fp::fmadds(
                0.039_999_999_105_930_33,
                fp::frsp(f64::from(c.level)),
                0.300_000_011_920_928_96,
            );
            c.human_target_rule = u8::from(r < p);
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
        fpp.cpu.drifting_back = in_bounds;
        let coll = &fpp.coll_data;
        let vy = fp::fsubs(reg(coll.cur_pos.y), reg(coll.last_pos.y));
        let vx = fp::fsubs(reg(coll.cur_pos.x), reg(coll.last_pos.x));
        let vmag = math::sqrt_reg(fp::fmadds(vx, vx, fp::fmuls(vy, vy)));
        let c = &mut fpp.cpu;
        if vmag < 0.01 {
            c.stage_read_count = c.stage_read_count.wrapping_add(1);
        } else {
            c.stage_read_count = 0;
        }
        if c.keeps_target {
            if c.retarget_timer != 0 {
                c.retarget_timer -= 1;
            } else {
                c.keeps_target = false;
            }
        } else {
            let r = self.randf();
            self.game.fighters[f.0].cpu.retarget_timer =
                fp::fctiwz(fp::fmuls(120.0, fp::fmuls(0.5, fp::fmuls(0.5, r))));
        }
        let c = &mut self.game.fighters[f.0].cpu;
        if c.destination_timer != 0 {
            c.destination_timer -= 1;
            if c.destination_timer == 0 {
                c.destination.x = c.home.x;
                c.destination.y = c.home.y;
            }
        }
        // The destination rides its floor (or the stage's wind).
        let (sx, sy) = (c.destination.x, c.destination.y);
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
            None => {
                let mut wind = crate::vec::Vec3::new(0.0, 0.0, 0.0);
                super::super::gr::grlib::item_wind(self.game, &mut wind).then_some(wind)
            }
        };
        if let Some(v) = vel {
            let c = &mut self.game.fighters[f.0].cpu;
            c.destination.x = mem(fp::fadds(reg(c.destination.x), reg(v.x)));
            c.destination.y = mem(fp::fadds(reg(c.destination.y), reg(v.y)));
        }
        // `ftCo_CpuUpdateRecoveryScale`.
        if timer % 30 == 0 {
            let r = self.randf();
            let c = &mut self.game.fighters[f.0].cpu;
            c.reach_scale = mem(fp::frsp(fp::fmadd(
                0.05,
                f64::from(c.level.wrapping_add(1)),
                fp::fmul(0.05, r),
            )));
        }
        self.cpu_reach_scale(f);
        let fpp = &self.game.fighters[f.0];
        if fpp.cpu.off_stage {
            let (x, y) = (fpp.cur_pos.x, fpp.cur_pos.y);
            let top = mem(fp::fadds(1000.0, reg(y)));
            let ceiling = self
                .game
                .geometry
                .as_mut()
                .expect("no collision geometry")
                .check_ceiling(x, y, x, top, -1, -1);
            if ceiling.is_none() {
                self.game.fighters[f.0].cpu.off_stage = false;
            }
        } else if fpp.ground_or_air == GA_GROUND
            && self.corneria_ceiling_floor(fpp.coll_data.floor.index)
        {
            self.game.fighters[f.0].cpu.off_stage = true;
        }
    }

    /// Whether a floor is Corneria's ceilinged one (joint 4), `grCorneria_801E2E50`.
    #[doc(alias = "grCorneria_801E2E50")]
    pub(super) fn corneria_ceiling_floor(&self, line: i32) -> bool {
        // Gr_Kind_Corneria.
        self.game.stage_info.grkind == GrKind::Corneria
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
    #[doc(alias = "ftCo_800A0CB0")]
    fn cpu_reach_scale(&mut self, f: FighterId) {
        if self.game.fighters[f.0].cpu.frame_counter % 600 != 0 {
            return;
        }
        let x = self.randf();
        let x = fp::fmuls(x, fp::fmuls(x, x));
        let r = fp::fsubs(1.0, x);
        let fpp = &mut self.game.fighters[f.0];
        fpp.cpu.reach = mem(match fpp.fighter_kind() {
            FighterKind::Donkey | FighterKind::Koopa => fp::frsp(fp::fmul(9.0, r)),
            FighterKind::GKoopa => fp::frsp(fp::fmul(18.0, r)),
            FighterKind::Nana => fp::frsp(r),
            _ => fp::frsp(fp::fmul(4.0, r)),
        });
    }

    /// The fighter's decision by its computer kind, `ftCo_800B2AFC`: Nana's, the VS kind's
    /// ([`Running::cpu_decide_vs`]), training's, the single-player modes'
    /// ([`super::cpu_kinds`]) and kind 15's are ported. It returns the r5 the decision
    /// leaves, which `ftCo_800B2AFC` passes on unchanged.
    #[doc(alias = "ftCo_800B2AFC")]
    fn cpu_decide(&mut self, f: FighterId) -> Leftover {
        use CpuKind as K;
        let Some(kind) = CpuKind::from_id(self.game.fighters[f.0].cpu.kind) else {
            // Every kind the switch doesn't list.
            return self.cpu_decide_vs(f);
        };
        match kind {
            K::Nana => self.nana_decide(f),
            // The VS kind, two others and the two the switch doesn't list.
            K::Normal | K::SpLwLink | K::SpLwSamus | K::Manual | K::Air => self.cpu_decide_vs(f),
            K::NoAct => {
                self.game.fighters[f.0].cpu.behavior = 0;
                Leftover::Unknown("ftCo_800B2AFC")
            }
            // Training's: stop where it stands, walk to its spot or jump, the jump and the
            // walk the behavior pick's.
            K::Stay | K::Walk | K::Jump => self.cpu_decide_training(f, kind == K::Walk),
            K::Escape => self.cpu_decide_evade(f),
            // The single-player modes' kinds (`super::cpu_kinds`).
            K::Defensive => self.cpu_decide_keep_away(f, true),
            K::Struggle => self.cpu_decide_no_human(f, CPU_FRAME_SP - CPU_FRAME - DECIDE_FRAME),
            K::Freak => self.cpu_decide_wanderer(f),
            K::Cooperate => self.cpu_decide_follower(f),
            K::OnlyItem => self.cpu_decide_roamer(f),
            K::EvZelda => self.cpu_decide_bold(f),
            K::Item => self.cpu_decide_collector(f),
            K::KeepAway => self.cpu_decide_keep_away(f, false),
            K::TowardP1 | K::TowardP2 | K::TowardP3 | K::TowardP4 => {
                self.cpu_decide_player(f, kind.id() - K::TowardP1.id())
            }
            K::Fighter => self.cpu_decide_fighter(f),
            // Kind 25 is kind 23's code again (Multi-Man's Wire Frames).
            K::Enemy | K::EnemyAgain => self.cpu_decide_enemy(f),
            K::Wander => self.cpu_decide_wander(f),
            K::Unafraid => self.cpu_decide_unafraid(f),
            K::ItemLover => self.cpu_decide_item_lover(f),
            // Kind 26 (kind 23's target, training's floor under it): no table, setter or
            // debug choice gives it (`cpu_audit`'s `KIND_26`).
            K::Unused26 => unported_call("ftCo_800B2AFC for computer kind 26"),
        }
    }

    /// The single-player opponents' computer kind 23 (`ftCo_800B2AFC`'s case): its target the
    /// nearest enemy (`ftCo_CpuInitEnemyTarget`, the fear flags but two off), the destination
    /// timer stopped when the behavior changed, and unless it's being hit (behavior 4), toward
    /// the target; then the behavior pick, whose r5 it leaves.
    fn cpu_decide_enemy(&mut self, f: FighterId) -> Leftover {
        let c = &mut self.game.fighters[f.0].cpu;
        c.fear_target = false;
        c.acts_alone = true;
        c.may_air_attack = true;
        c.may_jump_attack = false;
        c.may_ranged_attack = false;
        c.may_shield = false;
        c.may_pick_up = false;
        c.fleeing = false;
        let (target, lr) = self.cpu_find_target_lr(f);
        let c = &mut self.game.fighters[f.0].cpu;
        c.target = target;
        if c.behavior != c.alt_behavior && c.behavior != c.default_behavior {
            c.destination_timer = 0;
        }
        let sp = CPU_FRAME_SP - CPU_FRAME - DECIDE_FRAME;
        if c.behavior != 4 {
            c.ledge_picked = false;
            // `ftCo_800A75DC`'s vector at `r1 + 0x18` as `ftCo_800A4BEC`'s frame left it, as
            // in the VS decision: the back chain (this frame's stack pointer) and the word its
            // link register slot holds.
            let unset = lr.map(|y| (f32::from_bits(sp), f32::from_bits(y)));
            let t = self.game.fighters[f.0].cpu.target;
            self.cpu_toward(f, t, unset);
        }
        self.cpu_choose_behavior_r5(f, true, Some(sp)).1
    }

    /// Computer kind 24 (`ftCo_800B1DA0`, Multi-Man's Wire Frames): its target the nearest
    /// enemy as kind 23's, and once a second while it may act, on a coin flip, a destination
    /// on another island (`ftCo_800A8940`); then the behavior pick, whose r5 it leaves.
    #[doc(alias = "ftCo_800B1DA0")]
    fn cpu_decide_wander(&mut self, f: FighterId) -> Leftover {
        /// `ftCo_800B1DA0`'s frame, below the decision's.
        const WANDER_FRAME: u32 = 0x38;
        let c = &mut self.game.fighters[f.0].cpu;
        c.fear_target = false;
        c.acts_alone = true;
        c.may_air_attack = true;
        c.may_jump_attack = false;
        c.may_ranged_attack = false;
        c.may_shield = false;
        c.may_pick_up = false;
        c.fleeing = false;
        let (target, _) = self.cpu_find_target_lr(f);
        self.game.fighters[f.0].cpu.target = target;
        if self.cpu_should_act(f)
            && self.game.fighters[f.0]
                .cpu
                .frame_counter
                .wrapping_rem(60)
                .wrapping_mul(5)
                == 0
            && reg(self.game.objs.rng.draw_f32()) < 0.5
        {
            self.cpu_to_other_island(f);
        }
        let sp = CPU_FRAME_SP - CPU_FRAME - DECIDE_FRAME - WANDER_FRAME;
        self.cpu_choose_behavior_r5(f, true, Some(sp)).1
    }

    /// `ftCo_800A8940`: a destination on another live island picked at random, weighted by
    /// width: of the islands other than the fighter's whose ends are inside the blast zones,
    /// the safe ones' widths summed (none: no destination); then a draw over the same
    /// islands but the safety test, a second draw for the spot along the island picked (5 in
    /// from its ends when it's wider than 10), and the floor under it, if inside the blast
    /// zones, the destination with a reach of 5.
    ///
    /// The second pass skips `ftCo_800A2718`'s test, so its islands are the first pass's and
    /// maybe more, and its sum over the first's reaches 1 before its last island: a draw (under
    /// 1 - 2^-16) always picks one, so the island the original keeps in r31 (its caller's r31
    /// when none is picked) is always written ([Melee's bugs](docs/melee-bugs.md)).
    #[doc(alias = "ftCo_800A8940")]
    pub(super) fn cpu_to_other_island(&mut self, f: FighterId) {
        let rnd = reg(self.game.objs.rng.draw_f32());
        let floor = self.game.fighters[f.0].coll_data.floor.index;
        let geo = self.game.geometry.as_ref().expect("no collision geometry");
        let cur = geo.island_of_line(floor);
        let first = geo.first_floor_island();
        let island = |r: &Self, id| *r.game.geometry.as_ref().expect("geometry").island(id);
        let inside = |r: &Self, isl: &crate::mp::Island| {
            !r.cpu_out_of_bounds(f, isl.right.x, isl.right.y)
                && !r.cpu_out_of_bounds(f, isl.left.x, isl.left.y)
        };
        let mut total = reg(0.0);
        let mut it = first;
        while let Some(id) = it {
            let isl = island(self, id);
            it = isl.next;
            if cur != Some(id) && !self.cpu_island_unsafe(Some(id)) && inside(self, &isl) {
                total = fp::fadds(total, fp::fsubs(reg(isl.right.x), reg(isl.left.x)));
            }
        }
        if total < reg(0.00001) && total > reg(-0.00001) {
            return;
        }
        let total = fp::frsp(fp::fdiv(1.0, total));
        let mut accum = reg(0.0);
        let mut chosen = None;
        let mut it = first;
        while let Some(id) = it {
            let isl = island(self, id);
            it = isl.next;
            if cur != Some(id) && inside(self, &isl) {
                accum = fp::fadds(accum, fp::fsubs(reg(isl.right.x), reg(isl.left.x)));
                if rnd < fp::fmuls(accum, total) {
                    chosen = Some(isl);
                    break;
                }
            }
        }
        let Some(isl) = chosen else {
            unported_call("ftCo_800A8940 with no island picked (its r31 and r27 unwritten)")
        };
        let rnd2 = reg(self.game.objs.rng.draw_f32());
        let width = fp::fsubs(reg(isl.right.x), reg(isl.left.x));
        let px = if width > 10.0 {
            fp::frsp(fp::fmadd(
                fp::fsub(width, 10.0),
                rnd2,
                fp::fadd(5.0, reg(isl.left.x)),
            ))
        } else {
            fp::fmadds(width, rnd2, reg(isl.left.x))
        };
        let py = fp::fmadds(
            rnd2,
            fp::fsubs(reg(isl.right.y), reg(isl.left.y)),
            reg(isl.left.y),
        );
        let top = mem(fp::fadds(5.0, py));
        let bottom = mem(fp::fsubs(py, reg(100.0)));
        let px = mem(px);
        let Some(pos) = self.cpu_floor(px, top, px, bottom) else {
            return;
        };
        if !self.cpu_out_of_bounds(f, pos.x, pos.y) {
            self.cpu_set_destination(f, pos.x, pos.y, 5.0);
        }
    }

    /// Training's computer kinds 0, 1 and 3 (`ftCo_800B2AFC`'s cases): no target, the
    /// destination timer stopped when the behavior changed, and unless it's being hit
    /// (behavior 4), its destination the floor under it (kind 1: under its spot, `x98`, or
    /// where it is when there's none), then the behavior pick, whose r5 it leaves.
    fn cpu_decide_training(&mut self, f: FighterId, walk: bool) -> Leftover {
        let c = &mut self.game.fighters[f.0].cpu;
        c.fear_target = false;
        // `ftCo_CpuClearTargetModes`.
        c.acts_alone = false;
        c.may_air_attack = false;
        c.may_jump_attack = false;
        c.may_ranged_attack = false;
        c.may_shield = false;
        c.may_pick_up = false;
        c.fleeing = false;
        if c.behavior != c.alt_behavior && c.behavior != c.default_behavior {
            c.destination_timer = 0;
        }
        if c.behavior != 4 {
            c.ledge_picked = false;
            let fpp = &self.game.fighters[f.0];
            let at = if walk { fpp.cpu.spawn_pos } else { fpp.cur_pos };
            let below = mem(fp::fsubs(reg(at.y), 1000.0));
            let above = mem(fp::fadds(10.0, reg(at.y)));
            match self.cpu_floor(at.x, above, at.x, below) {
                Some(p) => self.cpu_set_destination(f, p.x, p.y, 5.0),
                None if walk => {
                    let pos = self.game.fighters[f.0].cur_pos;
                    self.cpu_set_destination(f, pos.x, pos.y, 5.0);
                }
                None => {}
            }
        }
        // The pick reads r31, here the fighter's pointer, as its DI switch.
        let sp = CPU_FRAME_SP - CPU_FRAME - DECIDE_FRAME;
        self.cpu_choose_behavior_r5(f, true, Some(sp)).1
    }

    /// Training's computer kind 2, `ftCo_800B04DC`: its target the nearest enemy, a
    /// healing item to go for unless it holds another or is entering, a stage enemy, and
    /// when it may act on the ground within 50 of its target, the island end farthest from
    /// it as its destination; then the behavior pick, whose r5 it leaves.
    #[doc(alias = "ftCo_800B04DC")]
    fn cpu_decide_evade(&mut self, f: FighterId) -> Leftover {
        let c = &mut self.game.fighters[f.0].cpu;
        c.fear_target = true;
        c.acts_alone = false;
        c.may_air_attack = false;
        c.may_jump_attack = false;
        c.may_ranged_attack = false;
        c.may_shield = false;
        c.may_pick_up = false;
        c.fleeing = true;
        let target = self.cpu_find_target(f);
        self.game.fighters[f.0].cpu.target = target;
        self.cpu_healing_item_targets(f);
        if self.cpu_should_act(f)
            && let Some(t) = self.game.fighters[f.0].cpu.target
            && self.game.fighters[f.0].ground_or_air != GA_AIR
        {
            let (me, them) = (
                self.game.fighters[f.0].cur_pos,
                self.game.fighters[t.0].cur_pos,
            );
            let dy = fp::fsubs(reg(me.y), reg(them.y));
            let dx = fp::fsubs(reg(me.x), reg(them.x));
            let dist = crate::math::sqrt(mem(fp::fmadds(dx, dx, fp::fmuls(dy, dy))));
            let far = reg(dist) > 50.0;
            if !far && let Some((x, y)) = self.cpu_far_island_end(f, them.x, them.y) {
                self.cpu_set_destination(f, x, y, 5.0);
            }
        }
        // The pick reads r31, here the computer data's pointer, as its DI switch.
        let sp = CPU_FRAME_SP - CPU_FRAME - DECIDE_FRAME - EVADE_FRAME;
        self.cpu_choose_behavior_r5(f, true, Some(sp)).1
    }

    /// Nana's decision, `ftCo_800B101C`: with Popo gone she acts alone; else she records his
    /// inputs, takes her level from his percent, and either follows him (until he's out of
    /// reach or doing his up special) or makes her way back to him. It returns the r5 its
    /// last call leaves: on her way back, always [`Running::nana_keeps_up`]'s.
    #[doc(alias = "ftCo_800B101C")]
    fn nana_decide(&mut self, f: FighterId) -> Leftover {
        let c = &mut self.game.fighters[f.0].cpu;
        c.acts_alone = true;
        c.may_air_attack = true;
        let Some(popo) = self.cpu_partner(f) else {
            return self.nana_decide_alone(f);
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
        if self.game.fighters[f.0].cpu.follows_popo {
            self.cpu_toward_partner(f);
            // What `ftCo_800A7AAC` leaves (or passes on from `ftCo_800B0918`, Popo's x) depends
            // on the stack and the collision code's registers, which aren't followed: the
            // partner search leaves a fighter's address, the floor checks their own words.
            let (lost, left) = self.nana_lost_popo(f, popo, Leftover::Unknown("ftCo_800A7AAC"));
            r5 = left;
            if lost {
                let c = &mut self.game.fighters[f.0].cpu;
                c.follows_popo = false;
                c.behavior = c.default_behavior;
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
                c.behavior = 0;
                c.follows_popo = true;
                c.acts_alone = false;
                c.may_air_attack = false;
            }
        }
        let fpp = &mut self.game.fighters[f.0];
        if fpp.ground_or_air == GA_GROUND {
            fpp.cpu.nana_recovering = false;
        }
        r5
    }

    /// Whether Nana stops following Popo (`ftCo_800B101C_inline0`): when she's in her up
    /// special (which marks her `xFB_b0`), more than 25 from him, or not where she can keep
    /// up with him ([`Running::nana_keeps_up`]); with the r5 left, `r5` unless that check ran.
    fn nana_lost_popo(&mut self, f: FighterId, popo: FighterId, r5: Leftover) -> (bool, Leftover) {
        let ms = self.game.fighters[f.0].motion_id;
        if (MS_PP_SPECIAL_HI_0..=MS_PP_SPECIAL_HI_5).contains(&ms) {
            self.game.fighters[f.0].cpu.nana_recovering = true;
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
    #[doc(alias = "ftCo_800B0E98")]
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
        if me.cpu.nana_recovering
            || (MS_PP_SPECIAL_HI_0..=MS_PP_SPECIAL_HI_5).contains(&me.motion_id)
        {
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
        if c.behavior != c.alt_behavior && c.behavior != c.default_behavior {
            c.destination_timer = 0;
        }
        if c.behavior == 4 {
            false
        } else {
            c.ledge_picked = false;
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
            if matches!(
                ItKind::from_id(kind),
                Some(ItKind::Crate | ItKind::Barrel | ItKind::PartyBall | ItKind::BarrelCannon)
            ) {
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
        if me.nana_follows {
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
    #[doc(alias = "ftCo_800B0918")]
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
        c.partner_input_write.0 = Some(step(c.partner_input_write.0));
        c.partner_input_read.0 = Some(step(c.partner_input_read.0));
        let i = &mut c.partner_inputs[(c.partner_input_write.0.unwrap() / INPUT_SIZE) as usize];
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
    #[doc(alias = "ftCo_800B0AF4")]
    fn nana_follow_inputs(&mut self, f: FighterId) {
        let Some(popo) = self.cpu_partner(f) else {
            return;
        };
        if !self.game.fighters[f.0].cpu.follows_popo {
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
            .partner_input_read
            .0
            .expect("Nana's input history without a read position");
        let rec = fpp.cpu.partner_inputs[(at / INPUT_SIZE) as usize].clone();
        let c = &mut fpp.cpu;
        c.lstick.x = rec.lstick.x;
        c.lstick.y = rec.lstick.y;
        c.cstick.x = rec.cstick.x;
        c.cstick.y = rec.cstick.y;
        c.buttons = rec.x0;
        c.ltrigger = rec.x4;
        c.rtrigger = rec.x5;
        if fpp.nana_follows {
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
    /// ([`Leftover`]); before the DI's call only a charge's release writes it.
    #[doc(alias = "ftCo_800B2790")]
    fn cpu_behave(&mut self, f: FighterId, r5: Leftover) {
        let c = &self.game.fighters[f.0].cpu;
        if c.script_cursor.0.is_some() || c.command_duration != 0 {
            return;
        }
        let c = &mut self.game.fighters[f.0].cpu;
        c.hold_back_counter = c.hold_back_counter.wrapping_add(1);
        c.attacking = false;
        self.cpu_script_reset(f);
        let r5 = self.cpu_charge_release(f).unwrap_or(r5);
        use super::cpu_script::cmd;
        match self.game.fighters[f.0].cpu.behavior {
            0 => {
                self.cpu_script_clear(f);
                self.cpu_cmd(f, cmd::DONE);
            }
            1 => self.cpu_walk(f),
            2 => self.cpu_attack_behavior(f),
            3 => self.cpu_ranged(f),
            4 => self.cpu_recover_behavior(f),
            5 => self.cpu_get_up(f),
            6 => self.cpu_ledge_behavior(f),
            7 => self.cpu_shield(f),
            8 => self.cpu_jump_at_target(f),
            9 => self.cpu_hold(f),
            10 => self.cpu_stand(f),
            11 => self.cpu_hop(f),
            12 => self.cpu_pace(f),
            13 => {
                if self.game.fighters[f.0].item_gobj.is_some() {
                    let c = &mut self.game.fighters[f.0].cpu;
                    c.behavior = c.default_behavior;
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
                    c.behavior = c.default_behavior;
                }
            }
            14 => self.cpu_throw_item(f),
            15 => self.cpu_tumble(f),
            16 => self.cpu_mash(f),
            17 => self.cpu_barrel(f),
            18 => self.cpu_di(f, r5),
            19 => {
                let ms = self.game.fighters[f.0].motion_id;
                if ms != 0x131 && ms != 0x132 {
                    let c = &mut self.game.fighters[f.0].cpu;
                    c.behavior = c.default_behavior;
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

    /// In a barrel (behavior 17), `ftCo_800AC434`: out of it, back to the usual behavior; in
    /// one of Kongo Jungle's barrels (`ftCo_MS_BarrelWait`), a tap of A to be shot while the
    /// barrel points up to 15 degrees right of straight up (its angle, `Ground_801C4DA0`); in
    /// a barrel cannon (`ftCo_MS_Barrel`), a coin flip every (10 - level) * 5 frames for one.
    #[doc(alias = "ftCo_800AC434")]
    fn cpu_barrel(&mut self, f: FighterId) {
        use super::co::ids::ms;
        use super::cpu_script::cmd;
        let fpp = &self.game.fighters[f.0];
        let tap = match fpp.motion_id {
            ms::BARREL_WAIT => {
                let angle = reg(super::super::gr::ground::barrel(self.game).1);
                angle >= fp::fneg(0.261_799_385_771_155_36) && angle <= 0.0
            }
            ms::BARREL => {
                let c = &fpp.cpu;
                let period = 10i32.wrapping_sub(c.level).wrapping_mul(5);
                if period == 0 {
                    unported_call("ftCo_800AC434 at level 10");
                }
                c.frame_counter.wrapping_rem(period) == 0
                    && reg(self.game.objs.rng.draw_f32()) < 0.5
            }
            _ => {
                self.cpu_back_to_usual(f);
                return;
            }
        };
        if tap {
            // `ftCo_CpuTapA`.
            self.cpu_cmd(f, cmd::PRESS_A);
            self.cpu_cmd1(f, cmd::WAIT_FOR, 1);
            self.cpu_cmd(f, cmd::RELEASE_A);
        }
        self.cpu_cmd(f, cmd::DONE);
    }

    /// Lets go of a charging special when it's charged, `ftCo_800ADC28`: Donkey Kong's Giant
    /// Punch (a tap of R), Samus's Charge Shot, Mewtwo's Shadow Ball and Kirby's copies of
    /// them (a tap of R and a frame's wait); not while it's taking a hit (`x18` 10).
    /// A release leaves r5 1, its last argument (`ftCo_CpuTapR`'s wait; the command writers
    /// leave r5 alone), which a DI with no knockback in the same frame reads: a charging state
    /// can still carry the hitstun flag. The r5 it leaves, if it released.
    #[doc(alias = "ftCo_800ADC28")]
    fn cpu_charge_release(&mut self, f: FighterId) -> Option<Leftover> {
        use super::cpu_script::cmd;
        let fpp = &self.game.fighters[f.0];
        if fpp.cpu.behavior == 10 {
            return None;
        }
        let ms = fpp.motion_id;
        let tap_and_wait = match fpp.fighter_kind() {
            FighterKind::Donkey => {
                if ms == super::donkey::ms::SPECIAL_N_LOOP {
                    self.cpu_tap_r(f);
                    return Some(Leftover::Word(1));
                }
                return None;
            }
            FighterKind::Samus => ms == super::samus::ms::SPECIAL_N_HOLD,
            FighterKind::Mewtwo => {
                ms == super::mewtwo::ms::SPECIAL_N_LOOP
                    || ms == super::mewtwo::ms::SPECIAL_N_LOOP_FULL
            }
            FighterKind::Kirby => {
                use super::kirby::ms as kb;
                match FighterKind::from_id(fpp.u_i32(super::kirby::u::HAT_KIND)) {
                    Some(FighterKind::Donkey) => ms == kb::DK_SPECIAL_N_LOOP,
                    Some(FighterKind::Samus) => ms == kb::SS_SPECIAL_N_HOLD,
                    Some(FighterKind::Mewtwo) => {
                        ms == kb::MT_SPECIAL_N_LOOP || ms == kb::MT_SPECIAL_N_LOOP_FULL
                    }
                    _ => false,
                }
            }
            _ => false,
        };
        if !tap_and_wait {
            return None;
        }
        // `ftCo_CpuTapRAndWait`.
        self.cpu_tap_r(f);
        self.cpu_cmd1(f, cmd::WAIT_FOR, 1);
        Some(Leftover::Word(1))
    }
}
