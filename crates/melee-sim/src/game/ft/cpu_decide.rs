//! What a computer player does next (`ftCo_0A01.c`): whom it targets and which items it goes
//! for (`ftCo_800A4BEC`, `ftCo_800A5F4C`, `ftCo_800A648C`), and the behavior it picks
//! (`ftCo_800ADE48`) from what it and its target are doing, kept in `CpuFighter::x18`, which
//! [`super::cpu_ai`]'s script writer acts on.

use crate::lb::hitcapsule::HitCapsuleState;
use crate::mp::coll::GroundOrAir;
use gekko_fp as fp;

use super::super::gr::{GrKind, ground, stage};
use super::super::handles::{FighterId, ItemId};
use super::super::state::Running;
use super::cpu_ai::{Leftover, P_LINK_FIGHTER};
use crate::fpu::{mem, reg};
use crate::game::ft::FighterKind;
use crate::game::ft::{CpuBehavior, CpuKind};
use crate::game::it::ItKind;
use crate::math;
use crate::mp::FloorR5;

/// `HSD_GOBJ_PLINK_ITEM`.
const P_LINK_ITEM: u8 = 9;

/// The return addresses `ftCo_800A4BEC`'s calls to `ftCo_IsAlly` and `ftCo_800A0F00` leave
/// in its frame's link register slot.
const FIND_TARGET_ALLY_RET: u32 = 0x800A_4CB4;
const FIND_TARGET_CLOAKED_RET: u32 = 0x800A_4D48;

/// What each common item is worth to a computer player, `ftCo_803C5A68`.
const ITEM_WORTH: [i32; ItKind::COMMON_END.id() as usize] = [
    1, 0, 0, 1, 0, 0, 0, 1, 7, 6, 5, 4, 4, 3, 1, 1, 2, 1, 5, 2, 2, 2, 3, 3, 3, 4, 0, 0, 8, 4, 1, 4,
    4, 5, 5,
];

/// A computer player's route on a stage, `ftCo_803C6594_t`: from a point's island (and
/// between two x's, if given), the point it heads for when its destination lies above
/// (`up_angle` or more), below (`down_angle` or less), or else ahead to the right or the left;
/// a point whose third number is negative isn't taken.
struct Route {
    from: (f32, f32),
    up: (f32, f32, f32),
    down: (f32, f32, f32),
    right: (f32, f32, f32),
    left: (f32, f32, f32),
    up_angle: f32,
    down_angle: f32,
    span: Option<(f32, f32)>,
}

const QUARTER_PI: f32 = std::f32::consts::FRAC_PI_4;
const TWELFTH_PI: f32 = 0.261_799_4;
const NO_POINT: (f32, f32, f32) = (0.0, 0.0, -1.0);

/// Hyrule Temple's routes (`list0_0` to `list0_4`).
const SHRINE_ROUTES: [Route; 5] = [
    Route {
        from: (37.0, -143.5),
        up: (5.5, -111.0, 0.0),
        down: (5.5, -111.0, 0.0),
        right: NO_POINT,
        left: (5.5, -111.0, 0.0),
        up_angle: QUARTER_PI,
        down_angle: -QUARTER_PI,
        span: None,
    },
    Route {
        from: (30.0, -16.0),
        up: (-3.5, 24.0, 0.0),
        down: (-60.0, -86.0, 0.0),
        right: NO_POINT,
        left: (-3.5, 24.0, 0.0),
        up_angle: QUARTER_PI,
        down_angle: -TWELFTH_PI,
        span: None,
    },
    Route {
        from: (-60.0, -86.0),
        up: (-131.0, -26.8, 0.0),
        down: NO_POINT,
        right: NO_POINT,
        left: NO_POINT,
        up_angle: QUARTER_PI,
        down_angle: -QUARTER_PI,
        span: Some((-1000.0, -95.0)),
    },
    Route {
        from: (-60.0, -86.0),
        up: (-16.0, -46.0, 0.0),
        down: NO_POINT,
        right: NO_POINT,
        left: NO_POINT,
        up_angle: TWELFTH_PI,
        down_angle: -QUARTER_PI,
        span: Some((-95.0, -25.0)),
    },
    Route {
        from: (-60.0, -86.0),
        up: (-60.0, -86.0, 0.0),
        down: NO_POINT,
        right: NO_POINT,
        left: (-60.0, -86.0, 0.0),
        up_angle: QUARTER_PI,
        down_angle: -QUARTER_PI,
        span: Some((-25.0, 1000.0)),
    },
];

/// Great Bay's routes (`list1_0` to `list1_5`).
const GREAT_BAY_ROUTES: [Route; 6] = [
    Route {
        from: (-130.0, 6.0),
        up: (-130.0, 6.0, 0.0),
        down: NO_POINT,
        right: (-130.0, 6.0, 0.0),
        left: NO_POINT,
        up_angle: QUARTER_PI,
        down_angle: -QUARTER_PI,
        span: Some((-120.0, 1000.0)),
    },
    Route {
        from: (-130.0, 6.0),
        up: (-100.0, 34.0, 0.0),
        down: NO_POINT,
        right: (-100.0, 34.0, 0.0),
        left: NO_POINT,
        up_angle: QUARTER_PI,
        down_angle: -QUARTER_PI,
        span: None,
    },
    Route {
        from: (-10.0, 6.0),
        up: (-10.0, 6.0, 0.0),
        down: NO_POINT,
        right: NO_POINT,
        left: (-10.0, 6.0, 0.0),
        up_angle: QUARTER_PI,
        down_angle: -QUARTER_PI,
        span: Some((-1000.0, -20.0)),
    },
    Route {
        from: (-10.0, 6.0),
        up: (-30.0, 34.0, 0.0),
        down: NO_POINT,
        right: NO_POINT,
        left: (-30.0, 34.0, 0.0),
        up_angle: QUARTER_PI,
        down_angle: -QUARTER_PI,
        span: None,
    },
    Route {
        from: (-100.0, 34.0),
        up: NO_POINT,
        down: (-10.0, 6.0, 0.0),
        right: NO_POINT,
        left: NO_POINT,
        up_angle: QUARTER_PI,
        down_angle: -QUARTER_PI,
        span: Some((-50.0, 0.0)),
    },
    Route {
        from: (-100.0, 34.0),
        up: NO_POINT,
        down: (-130.0, 6.0, 0.0),
        right: NO_POINT,
        left: NO_POINT,
        up_angle: QUARTER_PI,
        down_angle: -QUARTER_PI,
        span: Some((-200.0, -80.0)),
    },
];

/// What `HSD_Randf` leaves in r5: `HSD_RandSeedPtr`, the address of its seed (`seed`).
const SEED_PTR: u32 = 0x804D_5F90;

/// What `gm_80166378` leaves in r5: its `MatchEnd` (`controller + 0x24C`, gmvs.c) plus 0x30.
const MATCH_END_X30: u32 = 0x8046_B6A0 + 0x24C + 0x30;

/// The frame sizes of `ftCo_800ADE48` (the behavior pick) and `mpCheckFloor`, and the
/// slots of theirs the pick's floor checks leave r5 pointing at: the flags pointer the pick
/// passes (`r1 + 0x58`) and the line's ends `mpCheckFloor` passes `mpLib_8004ED5C`
/// (`r1 + 0x44`).
const BEHAVIOR_FRAME: u32 = 0x90;
const BEHAVIOR_FLAGS_SLOT: u32 = 0x58;
const CHECK_FLOOR_FRAME: u32 = 0xF0;
const CHECK_FLOOR_SLOT: u32 = 0x44;

impl Running<'_> {
    /// Whether `(x, y)` is out of reach of the blast zones, padded by the fighter's half size
    /// (`inlineD0`).
    pub(super) fn cpu_out_of_bounds(&self, f: FighterId, x: f32, y: f32) -> bool {
        let c = &self.game.fighters[f.0].cpu;
        let (hw, hh) = (reg(c.half_width), reg(c.half_height));
        let (x, y) = (reg(x), reg(y));
        x < fp::fadds(hw, reg(stage::blast_left(self.game)))
            || x > fp::fsubs(reg(stage::blast_right(self.game)), hw)
            || y < fp::fadds(hh, reg(stage::blast_bottom(self.game)))
            || y > fp::fsubs(reg(stage::blast_top(self.game)), hh)
    }

    /// Whether two fighters are on the same side, `ftCo_IsAlly`: the same player, else the
    /// same team in a team match or outside free-for-all.
    #[doc(alias = "ftCo_IsAlly")]
    pub(super) fn cpu_is_ally(&self, a: FighterId, b: FighterId) -> bool {
        let (fa, fb) = (&self.game.fighters[a.0], &self.game.fighters[b.0]);
        if fa.player_idx == fb.player_idx {
            return true;
        }
        if self.game.vs_is_teams() {
            return fa.team == fb.team;
        }
        if self.game.vs_is_ffa() {
            return false;
        }
        fa.team == fb.team
    }

    /// Whether a fighter can't be targeted, `ftCo_800A1C44`: it's out of the match
    /// (`x2219_b1`), held up (`x2164`), entering without its ride (`x2168` with no
    /// `x2338.x`) or asleep.
    #[doc(alias = "ftCo_800A1C44")]
    pub(super) fn cpu_untargetable(&self, f: FighterId) -> bool {
        let fp = &self.game.fighters[f.0];
        fp.out_of_play
            || fp.star_cheer != 0
            || (fp.hammer_music != 0 && fp.hammer_timers.x == 0)
            || fp.is_sleeping
    }

    /// Whether a fighter is invisible to computer players for now, `ftCo_800A0F00`: under a
    /// cloaking item's color animation (`ft_80087A18`), always or, flickering, half the time
    /// by its own frame count.
    #[doc(alias = "ft_80087A80")]
    #[doc(alias = "ftCo_800A0F00")]
    pub(super) fn cpu_cloaked(&self, f: FighterId) -> bool {
        let fp = &self.game.fighters[f.0];
        if !self.cloaking_colors(f) {
            return false;
        }
        // `ft_80087A80`: the cloak's vibrate frame, 2 while it holds and 1 as it flickers.
        match fp.smash_attrs.vibrate_frame {
            2 => true,
            1 => fp.cpu.frame_counter % 240 > 120,
            _ => false,
        }
    }

    /// Whether a fighter shows a power-up's colors that hide it, `ft_80087A18`: with a
    /// power-up's color animation (`x2226_b4`) and its refraction (`x2226_b5`), the Spycloak
    /// animation playing (`ftCo_800C06B4`: `x508`'s, 0x7B past the common ones) 0x7B or 0x80,
    /// the cloak's.
    #[doc(alias = "ft_80087A18")]
    #[doc(alias = "ftCo_800C06B4")]
    fn cloaking_colors(&self, f: FighterId) -> bool {
        let fp = &self.game.fighters[f.0];
        let id = i32::from_be_bytes(fp.spycloak_overlay.colanim)
            .wrapping_add(super::colanim::SPYCLOAK_START);
        fp.cloak && (id == 0x7B || id == 0x80) && fp.refracted
    }

    /// The distance between two fighters, `ftCo_800A1AB4`.
    #[doc(alias = "ftCo_800A1AB4")]
    pub(super) fn cpu_distance(&self, a: FighterId, b: FighterId) -> f64 {
        let (pa, pb) = (
            self.game.fighters[a.0].cur_pos,
            self.game.fighters[b.0].cur_pos,
        );
        let dx = fp::fsubs(reg(pa.x), reg(pb.x));
        let dy = fp::fsubs(reg(pa.y), reg(pb.y));
        reg(math::sqrt(mem(fp::fmadds(dx, dx, fp::fmuls(dy, dy)))))
    }

    /// The nearest other fighter, ally or not, in reach of the blast zones (padded by `f`'s
    /// half size) that isn't asleep, out of stamina, invisible or dead, `ftCo_800A4A40` (the
    /// grapple beam's homing): the first such on the fighters' list, then any strictly nearer.
    #[doc(alias = "ftCo_800A4A40")]
    pub fn nearest_fighter(&self, f: FighterId) -> Option<FighterId> {
        let me = self.game.fighters[f.0].gobj;
        let mut closest: Option<(FighterId, f64)> = None;
        let mut cur = self.game.world.first_in(P_LINK_FIGHTER);
        while let Some(g) = cur {
            cur = self.game.world.gobjs[g.0].next;
            if me == Some(g) {
                continue;
            }
            let o = self.game.fighter_id(g);
            let pos = self.game.fighters[o.0].cur_pos;
            if self.cpu_out_of_bounds(f, pos.x, pos.y) {
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
            let d = self.cpu_distance(f, o);
            match closest {
                None => closest = Some((o, d)),
                Some((_, best)) if best > d => closest = Some((o, d)),
                _ => {}
            }
        }
        closest.map(|(o, _)| o)
    }

    /// The fighter's target, `ftCo_800A4BEC`: its last one while that one lasts (`xF9_b0`),
    /// else the nearest enemy in reach that isn't out of the match, invisible or dead; a new
    /// target gets a fresh timer.
    #[doc(alias = "ftCo_800A4BEC")]
    pub(super) fn cpu_find_target(&mut self, f: FighterId) -> Option<FighterId> {
        self.cpu_find_target_lr(f).0
    }

    /// [`Running::cpu_find_target`], with the word its frame's link register slot is left
    /// holding (`r1 + 4`, where its callees save their return address): the return address
    /// of the last of `ftCo_IsAlly` and `ftCo_800A0F00` it called, the only callees that
    /// save one; none if it called neither.
    pub(super) fn cpu_find_target_lr(&mut self, f: FighterId) -> (Option<FighterId>, Option<u32>) {
        let mut lr = None;
        let me = self.game.fighters[f.0].gobj;
        let mut closest: Option<(FighterId, f64)> = None;
        let mut cur = self.game.world.first_in(P_LINK_FIGHTER);
        while let Some(g) = cur {
            cur = self.game.world.gobjs[g.0].next;
            if me == Some(g) {
                continue;
            }
            let o = self.game.fighter_id(g);
            let pos = self.game.fighters[o.0].cur_pos;
            if self.cpu_out_of_bounds(f, pos.x, pos.y) {
                continue;
            }
            lr = Some(FIND_TARGET_ALLY_RET);
            if self.cpu_is_ally(f, o) || self.cpu_untargetable(o) {
                continue;
            }
            let other = &self.game.fighters[o.0];
            if other.is_sleeping || other.stamina_dead {
                continue;
            }
            lr = Some(FIND_TARGET_CLOAKED_RET);
            if self.cpu_cloaked(o) || self.game.ft_is_dead(g) {
                continue;
            }
            let c = &self.game.fighters[f.0].cpu;
            if c.keeps_target {
                if c.last_target == Some(o) {
                    return (Some(o), lr);
                }
                continue;
            }
            let d = self.cpu_distance(f, o);
            match closest {
                None => closest = Some((o, d)),
                Some((_, best)) if best > d => closest = Some((o, d)),
                _ => {}
            }
        }
        let target = match closest {
            None => {
                self.game.fighters[f.0].cpu.keeps_target = false;
                None
            }
            Some((o, _)) => {
                self.game.fighters[f.0].cpu.keeps_target = true;
                let r = reg(self.game.objs.rng.draw_f32());
                let c = &mut self.game.fighters[f.0].cpu;
                c.retarget_timer = fp::fctiwz(fp::fmuls(120.0, fp::fmuls(0.5, fp::fmuls(0.5, r))));
                c.last_target = Some(o);
                Some(o)
            }
        };
        (target, lr)
    }

    /// Whether an item heals, `ftCo_800A5908`: a heart, a tomato or food.
    #[doc(alias = "ftCo_800A5908")]
    pub(super) fn cpu_item_heals(&self, item: ItemId) -> bool {
        // `It_Kind_Heart`, `_Tomato`, `_Foods`.
        matches!(
            ItKind::from_id(self.game.items[item.0].kind),
            Some(ItKind::HeartContainer | ItKind::MaximTomato | ItKind::Food)
        )
    }

    /// The common item of `kind` (any for `It_Kind_L_Gun_Ray`) the fighter goes for,
    /// `ftCo_800A5F4C`: grabbable, in reach and worth at least its threshold (`x2C`), the
    /// nearest of the most worth (`ftCo_803C5A68`); the first found keeps a tie.
    #[doc(alias = "ftCo_800A5F4C")]
    pub(super) fn cpu_common_item_target(&mut self, f: FighterId, kind: i32) -> Option<ItemId> {
        let mut closest: Option<(ItemId, f64)> = None;
        let mut cur = self.game.world.first_in(P_LINK_ITEM);
        while let Some(g) = cur {
            cur = self.game.world.gobjs[g.0].next;
            if !super::super::it::item::is_grabbable(self.game, g) {
                continue;
            }
            let it = super::super::it::item_of(self.game, g);
            let ip = &self.game.items[it.0];
            if kind != ItKind::COMMON_END && ip.kind != kind {
                continue;
            }
            if self.cpu_out_of_bounds(f, ip.pos.x, ip.pos.y) || ip.kind >= ItKind::COMMON_END {
                continue;
            }
            let worth = ITEM_WORTH[ip.kind as usize];
            if worth < self.game.fighters[f.0].cpu.item_worth_min {
                continue;
            }
            match closest {
                None => closest = Some((it, self.cpu_item_distance(f, it))),
                Some((best, d)) => {
                    let kind = self.game.items[best.0].kind;
                    if worth < ITEM_WORTH[kind as usize] {
                        continue;
                    }
                    let dist = self.cpu_item_distance(f, it);
                    if d > dist {
                        closest = Some((it, dist));
                    }
                }
            }
        }
        closest.map(|(it, _)| it)
    }

    /// The healing item the fighter goes for, `ftCo_800A61D8`: a grabbable heart, tomato or
    /// food in reach and worth at least its threshold (`x2C`), the nearest of the most worth
    /// (`ftCo_803C5A68`); the first found keeps a tie.
    #[doc(alias = "ftCo_800A61D8")]
    pub(super) fn cpu_healing_item_target(&mut self, f: FighterId) -> Option<ItemId> {
        let mut closest: Option<(ItemId, f64)> = None;
        let mut cur = self.game.world.first_in(P_LINK_ITEM);
        while let Some(g) = cur {
            cur = self.game.world.gobjs[g.0].next;
            if !super::super::it::item::is_grabbable(self.game, g) {
                continue;
            }
            let it = super::super::it::item_of(self.game, g);
            if !self.cpu_item_heals(it) {
                continue;
            }
            let ip = &self.game.items[it.0];
            if self.cpu_out_of_bounds(f, ip.pos.x, ip.pos.y) || ip.kind >= ItKind::COMMON_END {
                continue;
            }
            let worth = ITEM_WORTH[ip.kind as usize];
            if worth < self.game.fighters[f.0].cpu.item_worth_min {
                continue;
            }
            match closest {
                None => closest = Some((it, self.cpu_item_distance(f, it))),
                Some((best, d)) => {
                    let kind = self.game.items[best.0].kind;
                    if worth < ITEM_WORTH[kind as usize] {
                        continue;
                    }
                    let dist = self.cpu_item_distance(f, it);
                    if d > dist {
                        closest = Some((it, dist));
                    }
                }
            }
        }
        closest.map(|(it, _)| it)
    }

    /// The distance from the fighter to an item, `ftCo_GetItemDistance` (inlined), left in a
    /// register.
    fn cpu_item_distance(&self, f: FighterId, it: ItemId) -> f64 {
        let (a, b) = (self.game.fighters[f.0].cur_pos, self.game.items[it.0].pos);
        let dy = fp::fsubs(reg(a.y), reg(b.y));
        let dx = fp::fsubs(reg(a.x), reg(b.x));
        reg(math::sqrt(mem(fp::fmadds(dx, dx, fp::fmuls(dy, dy)))))
    }

    /// The enemy item the fighter goes for, `ftCo_800A648C`: the nearest of the containers'
    /// monsters and the adventure's Koopas, Like Likes and Topis in reach of the blast zones.
    #[doc(alias = "ftCo_800A648C")]
    pub(super) fn cpu_enemy_item_target(&mut self, f: FighterId) -> Option<ItemId> {
        let mut closest: Option<(ItemId, f64)> = None;
        let mut cur = self.game.world.first_in(P_LINK_ITEM);
        while let Some(g) = cur {
            cur = self.game.world.gobjs[g.0].next;
            let it = super::super::it::item_of(self.game, g);
            let ip = &self.game.items[it.0];
            // `ftCo_800A648C_inline1`: `It_Kind_Kuriboh` up to `_Octarock_Stone` (the
            // containers' monsters), `_Nokonoko`, `_Patapata` and the next, `_Whitebea`.
            let kind = ip.kind;
            let enemy = (ItKind::Goomba.id()..ItKind::OctorokStone.id()).contains(&kind)
                || kind == ItKind::KoopaTroopa
                || (kind.wrapping_sub(ItKind::KoopaParatroopa.id()) as u32) <= 1
                || kind == ItKind::PolarBear;
            if !enemy || self.cpu_out_of_bounds(f, ip.pos.x, ip.pos.y) {
                continue;
            }
            let d = self.cpu_item_distance(f, it);
            match closest {
                None => closest = Some((it, d)),
                Some((_, best)) if best > d => closest = Some((it, d)),
                _ => {}
            }
        }
        closest.map(|(it, _)| it)
    }

    /// Nana making her way back to Popo, `ftCo_800B0760`: she targets the nearest enemy and
    /// the items in reach, acts toward her destination (or recovers, `xFB_b0`), and picks a
    /// behavior.
    #[doc(alias = "ftCo_800B0760")]
    pub(super) fn nana_regroup(&mut self, f: FighterId) {
        let c = &mut self.game.fighters[f.0].cpu;
        c.fear_target = true;
        c.acts_alone = true;
        c.may_air_attack = true;
        c.may_jump_attack = false;
        c.may_ranged_attack = false;
        c.may_shield = false;
        c.may_pick_up = false;
        c.fleeing = false;
        let target = self.cpu_find_target(f);
        self.game.fighters[f.0].cpu.target = target;
        self.cpu_item_targets(f);
        // `inlineI3`.
        if self.cpu_should_act(f) {
            if self.game.fighters[f.0].cpu.nana_recovering {
                self.cpu_pick_ledge(f);
            } else {
                self.cpu_toward_partner(f);
            }
        }
        // Every caller keeps its `CpuFighter` pointer in r31, which the decision reads as
        // its DI switch (see [`Running::cpu_choose_behavior`]). Nana's decision overwrites
        // the r5 it leaves.
        self.cpu_choose_behavior(f, true);
    }

    /// The fighter's behavior for now, `ftCo_800ADE48`: first its destination is checked (over
    /// ground and in reach, else where it stands, or the floor below it in the air) and its
    /// distance to it measured, its timers and coin flips stepped; then the first that
    /// applies of DI in hitstun, leaving a barrel, a Warp Star, a ledge or a grab, waiting out
    /// a fall, a crouch's shield drop, escaping a hold, a ranged attack, an attack, a jump
    /// over its target, an item to pick up or throw, and walking to its destination. True if
    /// it switched (or stays) in one of those.
    ///
    /// The original never sets the DI switch's flag in the case it means to take it, using
    /// what r31 held in its caller; `di_switch` is that register's truth.
    #[doc(alias = "ftCo_800ADE48")]
    pub(super) fn cpu_choose_behavior(&mut self, f: FighterId, di_switch: bool) -> bool {
        self.cpu_choose_behavior_r5(f, di_switch, None).0
    }

    /// [`Running::cpu_choose_behavior`] with the r5 it leaves when it keeps or starts the DI
    /// (the stale-register rule, docs/engine.md), the only behavior that reads it: the seed
    /// pointer `HSD_Randf` loads (`&seed`) when a coin flip was its last call, else
    /// `MatchEnd + 0x30` when the standing's query compiled the results this tick (the last
    /// team `fn_801661E0` ranks leaves it there, and every match has one), else what the
    /// destination's last floor check left ([`Running::cpu_floor_r5`]). `frame` is the stack
    /// pointer at its entry, which its caller's depth decides; without it the floor checks'
    /// stack addresses aren't followed.
    pub(super) fn cpu_choose_behavior_r5(
        &mut self,
        f: FighterId,
        di_switch: bool,
        frame: Option<u32>,
    ) -> (bool, Leftover) {
        let sp = frame.map(|frame| frame - BEHAVIOR_FRAME);
        // The destination: kept if over ground in reach, else reset.
        let c = &self.game.fighters[f.0].cpu;
        let (dx, dy) = (c.destination.x, c.destination.y);
        let below = mem(fp::frsp(fp::fsub(reg(dy), 5.0)));
        let above = mem(fp::frsp(fp::fadd(5.0, reg(dy))));
        let (floor, mut r5) = self.cpu_floor_r5(dx, above, dx, below, sp);
        let keep = floor.is_some() && !self.cpu_out_of_bounds(f, dx, dy);
        if !keep {
            let fpp = &mut self.game.fighters[f.0];
            fpp.cpu.ledge_picked = false;
            let pos = fpp.cur_pos;
            let at = if fpp.ground_or_air == GroundOrAir::Air {
                let to = mem(fp::fsubs(reg(pos.y), 1000.0));
                let (floor, left) = self.cpu_floor_r5(pos.x, pos.y, pos.x, to, sp);
                r5 = left;
                floor.map_or((pos.x, pos.y), |p| (p.x, p.y))
            } else {
                (pos.x, pos.y)
            };
            let c = &mut self.game.fighters[f.0].cpu;
            c.destination.x = at.0;
            c.destination.y = at.1;
        }
        let fpp = &self.game.fighters[f.0];
        let ddy = fp::fsubs(reg(fpp.cur_pos.y), reg(fpp.cpu.destination.y));
        let ddx = fp::fsubs(reg(fpp.cur_pos.x), reg(fpp.cpu.destination.x));
        let dist = math::sqrt_reg(fp::fmadds(ddx, ddx, fp::fmuls(ddy, ddy)));
        let slot = i32::from(fpp.player_idx);
        let c = &mut self.game.fighters[f.0].cpu;
        c.destination_distance = mem(dist);
        if c.stand_timer > 0 {
            c.stand_timer -= 1;
        }
        // `gm_8016C75C`: the player's standing.
        if self.game.vs.state.match_end.standings_tick != self.game.scene_ticks() {
            r5 = Leftover::Word(MATCH_END_X30);
        }
        let standing = self.cpu_standing(slot);
        if self.game.fighters[f.0].cpu.stand_kind != standing as i32 {
            self.game.fighters[f.0].cpu.stand_timer = 0x12C;
            let again = self.cpu_standing(slot);
            self.game.fighters[f.0].cpu.stand_kind = again as i32;
        }
        let c = &self.game.fighters[f.0].cpu;
        let period = if c.kind == CpuKind::EvZelda { 120 } else { 400 };
        if c.frame_counter % period == 0 {
            r5 = Leftover::Word(SEED_PTR);
            let r = reg(self.game.objs.rng.draw_f32());
            self.game.fighters[f.0].cpu.coin_flip = r > 0.5;
        }
        let fpp = &mut self.game.fighters[f.0];
        if !fpp.state_flags.holds_grab {
            fpp.cpu.hold_choice = 0;
        }
        // DI in hitstun.
        if fpp.cpu.behavior == CpuBehavior::Di {
            return (true, r5);
        }
        let switch = if fpp.kind == FighterKind::GKoopa
            || fpp.cpu.kind == CpuKind::NoAct
            || fpp.cpu.kind == CpuKind::Stay
            || !fpp.in_hitstun_sdi
        {
            false
        } else {
            fpp.cpu.keeps_target = false;
            r5 = Leftover::Word(SEED_PTR);
            let r = reg(self.game.objs.rng.draw_f32());
            let c = &mut self.game.fighters[f.0].cpu;
            let level = fp::fsubs(f64::from(c.level), 0.0);
            c.tech_shield = fp::fmuls(0.100_000_001_490_116_12, level) > r;
            di_switch
        };
        if switch {
            // `ftCo_800B4A78` leaves r5 alone.
            self.cpu_script_clear(f);
            self.game.fighters[f.0].cpu.behavior = CpuBehavior::Di;
            return (true, r5);
        }
        let r5 = Leftover::Unknown("ftCo_800ADE48");
        let behavior = self.game.fighters[f.0].cpu.behavior;
        let ms = self.game.fighters[f.0].motion_id;
        // Each check: the behavior, whether the fighter is in it already, and whether it
        // switches to it.
        macro_rules! switch_to {
            ($b:expr) => {{
                self.cpu_script_clear(f);
                self.game.fighters[f.0].cpu.behavior = $b;
                return (true, r5);
            }};
        }
        // Leaving a barrel (`ftCo_MS_BarrelWait`, `ftCo_MS_Barrel`).
        if behavior == CpuBehavior::Barrel {
            return (true, r5);
        }
        if ms == 0x125 || ms == 0x154 {
            switch_to!(CpuBehavior::Barrel);
        }
        // Riding a Warp Star.
        if behavior == CpuBehavior::WarpStar {
            return (true, r5);
        }
        if ms == 0x131 || ms == 0x132 {
            switch_to!(CpuBehavior::WarpStar);
        }
        // Lying down.
        if behavior == CpuBehavior::GetUp {
            return (true, r5);
        }
        if matches!(ms, 0xB7 | 0xB8 | 0xBF | 0xC0) {
            switch_to!(CpuBehavior::GetUp);
        }
        // On a ledge.
        if behavior == CpuBehavior::Ledge {
            return (true, r5);
        }
        if matches!(ms, 0xFC | 0xFD) {
            switch_to!(CpuBehavior::Ledge);
        }
        // Holding a fighter.
        if behavior == CpuBehavior::Hold {
            return (true, r5);
        }
        if self.cpu_is_grabbing(f) {
            switch_to!(CpuBehavior::Hold);
        }
        if self.game.fighters[f.0].cpu.behavior == CpuBehavior::Recover {
            return (true, r5);
        }
        if self.cpu_falling_away(f) {
            switch_to!(CpuBehavior::Recover);
        }
        let behavior = self.game.fighters[f.0].cpu.behavior;
        let fpp = &self.game.fighters[f.0];
        // Tumbling (`ftCo_MS_DamageFall`) at level 5 and up.
        if behavior == CpuBehavior::Tumble {
            return (true, r5);
        }
        if fpp.cpu.level >= 5 && fpp.motion_id == 0x26 {
            switch_to!(CpuBehavior::Tumble);
        }
        // Held.
        if behavior == CpuBehavior::Mash {
            return (true, r5);
        }
        let ms = fpp.motion_id;
        if ms == 0xE0 || ms == 0xE3 || (0x10A..=0x10E).contains(&ms) {
            switch_to!(CpuBehavior::Mash);
        }
        // Shielding or dodging.
        if self.game.fighters[f.0].cpu.behavior == CpuBehavior::Shield {
            self.cpu_shield_threat(f);
            return (true, r5);
        }
        if self.cpu_shield_chance(f) && self.cpu_shield_threat(f) != 0 {
            switch_to!(CpuBehavior::Shield);
        }
        if self.game.fighters[f.0].cpu.behavior == CpuBehavior::Ranged {
            return (true, r5);
        }
        if self.cpu_ranged_attack(f) {
            switch_to!(CpuBehavior::Ranged);
        }
        if self.game.fighters[f.0].cpu.behavior == CpuBehavior::Attack {
            return (true, r5);
        }
        if self.cpu_attack(f) {
            switch_to!(CpuBehavior::Attack);
        }
        if self.game.fighters[f.0].cpu.behavior == CpuBehavior::JumpAtTarget {
            return (true, r5);
        }
        let jump = self.cpu_jump_attack(f);
        let c = &mut self.game.fighters[f.0].cpu;
        c.jump_kind = (jump & 3) as u8;
        if c.jump_kind != 0 {
            c.behavior = CpuBehavior::JumpAtTarget;
            return (true, r5);
        }
        if self.game.fighters[f.0].cpu.behavior == CpuBehavior::PickUp {
            return (true, r5);
        }
        if self.cpu_item_pickup(f) {
            self.game.fighters[f.0].cpu.behavior = CpuBehavior::PickUp;
            return (true, r5);
        }
        if self.game.fighters[f.0].cpu.behavior == CpuBehavior::ThrowItem {
            return (true, r5);
        }
        if self.cpu_throws_held_item(f) {
            self.game.fighters[f.0].cpu.behavior = CpuBehavior::ThrowItem;
            return (true, r5);
        }
        let c = &self.game.fighters[f.0].cpu;
        if c.behavior == CpuBehavior::Stand {
            return (true, r5);
        }
        if c.alt_behavior != CpuBehavior::Idle && self.cpu_at_destination(f, 0.0) {
            let c = &mut self.game.fighters[f.0].cpu;
            c.behavior = c.alt_behavior;
            return (true, r5);
        }
        (false, r5)
    }

    /// Whether the fighter gets rid of the item it holds (behavior 14, in
    /// `ftCo_800ADE48`): always for computer kind 0x1D; else a capsule, crate, barrel, egg,
    /// party ball, barrel cannon, bob-omb, Poké Ball or Yoshi's egg, or an item that passes
    /// `ftCo_800A5980`, which takes the `Item*` as a `Fighter*`: its state test reads the
    /// item's kind, the word at the same offset.
    fn cpu_throws_held_item(&self, f: FighterId) -> bool {
        let fpp = &self.game.fighters[f.0];
        let Some(g) = fpp.item_gobj else {
            return false;
        };
        if fpp.cpu.kind == CpuKind::ItemLover {
            return true;
        }
        let kind = self.game.items[super::super::it::item_of(self.game, g).0].kind;
        // `It_Kind_Capsule` to `_BombHei`, `_M_Ball`, `_EvYoshiEgg`.
        if (ItKind::Capsule.id()..=ItKind::BobOmb.id()).contains(&kind)
            || kind == ItKind::PokeBall
            || kind == ItKind::EventYoshiEgg
        {
            return true;
        }
        // `ftCo_800A5980`'s `ftCo_MS_LandingFallSpecial` up to `_Attack100Start`, and
        // `_Furafura`, `_Catch`, `_CatchPull` and `_CatchAttack`.
        (43..47).contains(&kind) || matches!(kind, 211 | 212 | 213 | 217)
    }

    /// Sets the fighter's destination and how near it must get, unless a destination timer
    /// runs (`ftCo_800A1F3C`), then lets the stage move it (`ftCo_800A1CC4`).
    #[doc(alias = "ftCo_800A1F3C")]
    pub(super) fn cpu_set_destination(&mut self, f: FighterId, x: f32, y: f32, reach: f32) {
        let c = &mut self.game.fighters[f.0].cpu;
        if c.destination_timer == 0 {
            c.destination.x = x;
            c.destination.y = y;
            c.destination_reach = reach;
            self.cpu_stage_route(f);
        }
    }

    /// The stage's routes for computer players, `ftCo_800A1CC4` (with `ftCo_803C6594`, by
    /// stage kind; only Great Bay and the Temple have any): on the ground, apart from its
    /// destination's island, a computer player at a route's start (on its island, within its
    /// span) heads for the route's next point by the destination's angle from it (above,
    /// below, or ahead by its facing), for 300 frames.
    #[doc(alias = "ftCo_800A1CC4")]
    fn cpu_stage_route(&mut self, f: FighterId) {
        let routes: &[Route] = match self.game.stage_info.ground_kind() {
            GrKind::GreatBay => &GREAT_BAY_ROUTES,
            GrKind::Shrine => &SHRINE_ROUTES,
            _ => return,
        };
        let fpp = &self.game.fighters[f.0];
        let c = &fpp.cpu;
        if c.destination_timer != 0
            || c.kind == CpuKind::Stay
            || fpp.ground_or_air == GroundOrAir::Air
            || self.cpu_destination_here(f)
        {
            return;
        }
        let fpp = &self.game.fighters[f.0];
        let c = &fpp.cpu;
        let geo = self.game.geometry.as_ref().expect("no collision geometry");
        let Some(island) = geo.island_of_line(fpp.coll_data.floor.index) else {
            return;
        };
        let dy = fp::fsubs(reg(c.destination.y), reg(fpp.cur_pos.y));
        let dx = fp::fsubs(reg(c.destination.x), reg(fpp.cur_pos.x));
        let dx = if dx < 0.0 { fp::fneg(dx) } else { dx };
        let angle = super::co::guard::stick_angle(dy, dx);
        for r in routes {
            // `ftCo_800A1CC4_inline0`.
            let from = crate::vec::Vec3::new(r.from.0, r.from.1, 0.0);
            let geo = self.game.geometry.as_mut().expect("no collision geometry");
            if geo.island_below(from, -10.0) != Some(island) {
                continue;
            }
            let fpp = &self.game.fighters[f.0];
            if let Some((lo, hi)) = r.span
                && !(fpp.cur_pos.x > lo && fpp.cur_pos.x < hi)
            {
                continue;
            }
            let to = if angle > reg(r.up_angle) {
                r.up
            } else if angle < reg(r.down_angle) {
                r.down
            } else if fpp.facing_dir > 0.0 {
                r.right
            } else {
                r.left
            };
            if to.2 >= 0.0 {
                // `ftCo_800A1CC4_inline1`.
                let c = &mut self.game.fighters[f.0].cpu;
                c.destination_timer = 300;
                c.home.x = c.destination.x;
                c.home.y = c.destination.y;
                c.destination.x = to.0;
                c.destination.y = to.1;
                c.destination_reach = 5.0;
                return;
            }
        }
    }

    /// Whether the fighter stands on its destination's island (the island under 5 above the
    /// destination), `ftCo_800A21FC`.
    #[doc(alias = "ftCo_800A21FC")]
    fn cpu_destination_here(&mut self, f: FighterId) -> bool {
        let fpp = &self.game.fighters[f.0];
        if fpp.ground_or_air == GroundOrAir::Air {
            return false;
        }
        let line = fpp.coll_data.floor.index;
        let dest = crate::vec::Vec3::new(
            fpp.cpu.destination.x,
            mem(fp::frsp(fp::fadd(5.0, reg(fpp.cpu.destination.y)))),
            0.0,
        );
        let geo = self.game.geometry.as_mut().expect("no collision geometry");
        let Some(island) = geo.island_of_line(line) else {
            return false;
        };
        geo.island_below(dest, -10.0) == Some(island)
    }

    /// Whether an island is unsafe to stand on, `ftCo_800A2718`: a live bob-omb or shell on
    /// it, or the stage's own hazard: Yoshi's Story's moving cloud, Brinstar's acid, Onett's
    /// cars.
    #[doc(alias = "it_80275870")]
    #[doc(alias = "it_8026C1B4")]
    #[doc(alias = "ftCo_800A5944")]
    #[doc(alias = "ftCo_800A2718")]
    pub(super) fn cpu_island_unsafe(&self, island: Option<crate::mp::IslandId>) -> bool {
        let Some(island) = island else { return false };
        let geo = self.game.geometry.as_ref().expect("no collision geometry");
        let mut cur = self.game.world.first_in(P_LINK_ITEM);
        while let Some(g) = cur {
            cur = self.game.world.gobjs[g.0].next;
            let it = super::super::it::item_of(self.game, g);
            let ip = &self.game.items[it.0];
            // `it_8026C1B4`: a hitbox on.
            let live = ip
                .hitboxes
                .iter()
                .take(4)
                .any(|h| h.hit.state != HitCapsuleState::Disabled);
            // `ftCo_800A5944`: `It_Kind_BombHei`, `_G_Shell`, `_R_Shell`.
            if !live
                || !matches!(
                    ip.item_kind(),
                    ItKind::BobOmb | ItKind::GreenShell | ItKind::RedShell
                )
            {
                continue;
            }
            if geo.island_of_line(ip.coll.floor.index) == Some(island) {
                return true;
            }
        }
        let isl = *geo.island(island);
        match self.game.stage_info.ground_kind() {
            GrKind::Story => geo.island_moving(island),
            // `checkZebesIsland`: either end under the acid, or under where it rises to in 90
            // frames at its last step.
            GrKind::Zebes => {
                self.cpu_under_acid(90, isl.right.y) || self.cpu_under_acid(90, isl.left.y)
            }
            // `checkOnettY`: either end at the street's height while a car is coming.
            GrKind::Onett => {
                let car = |y: f32| reg(y) <= 5.0 && ground::cars_state(self.game) != 0;
                car(isl.right.y) || car(isl.left.y)
            }
            _ => false,
        }
    }

    /// Whether a height is under Brinstar's acid now or will be in `frames` frames at its last
    /// step, `ftCo_800A1F98`; never on another stage.
    #[doc(alias = "ftCo_800A1F98")]
    fn cpu_under_acid(&self, frames: i32, y: f32) -> bool {
        if self.game.stage_info.ground_kind_id() != GrKind::Zebes {
            return false;
        }
        let (level, step) = ground::acid_level_and_step(self.game);
        let y = reg(y);
        y < reg(level) || y < fp::fmadds(reg(step), fp::frsp(f64::from(frames)), reg(level))
    }

    /// The fighter heads for its partner (Nana for Popo), `ftCo_800A7AAC`: below him on the
    /// ground when he's in the air, at his feet when on the ground, unless his island is
    /// unsafe; off a ledge's edge when they're on different islands. Its reach is its own
    /// plus half his size.
    #[doc(alias = "ftCo_800A7AAC")]
    pub(super) fn cpu_toward_partner(&mut self, f: FighterId) {
        let Some(partner) = self.cpu_partner(f) else {
            return;
        };
        let p = &self.game.fighters[partner.0];
        let ppos = p.cur_pos;
        let reach = mem(fp::fadds(
            reg(self.game.fighters[f.0].cpu.reach),
            reg(p.cpu.hurt_middle),
        ));
        if p.ground_or_air == GroundOrAir::Air {
            let below = mem(fp::fsubs(reg(ppos.y), 1000.0));
            let above = mem(fp::fadds(10.0, reg(ppos.y)));
            let hit = self
                .game
                .geometry
                .as_mut()
                .expect("no collision geometry")
                .check_floor(ppos.x, above, ppos.x, below, 0.0, -1, -1, -1, None);
            let Some(hit) = hit.filter(|h| !self.excluded_floor(h.line)) else {
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
                .island(island);
            let dx = reg(self.game.fighters[f.0].cpu.destination.x);
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
        let below = mem(fp::frsp(fp::fsub(reg(ppos.y), 2.0)));
        let above = mem(fp::frsp(fp::fadd(2.0, reg(ppos.y))));
        // Either way his position: the floor check only decides which copy.
        let _ = self.cpu_floor(ppos.x, above, ppos.x, below);
        self.cpu_set_destination(f, ppos.x, ppos.y, reach);
        let me = &self.game.fighters[f.0];
        let p = &self.game.fighters[partner.0];
        let geo = self.game.geometry.as_ref().expect("no collision geometry");
        let same_island = me.ground_or_air != GroundOrAir::Air
            && p.ground_or_air != GroundOrAir::Air
            && geo
                .island_of_line(me.coll_data.floor.index)
                .is_some_and(|i| geo.island_of_line(p.coll_data.floor.index) == Some(i));
        if same_island || matches!(me.motion_id, 252 | 253) {
            return;
        }
        let Some(island) = geo.island_of_line(p.coll_data.floor.index) else {
            return;
        };
        let isl = *geo.island(island);
        let c = &me.cpu;
        if fp::fsubs(reg(c.destination.y), reg(me.cur_pos.y)) <= 0.0 {
            return;
        }
        if fp::fsubs(reg(c.destination.x), reg(me.cur_pos.x)) > 0.0 {
            if me.cur_pos.x < isl.left.x {
                let x = mem(fp::frsp(fp::fadd(5.0, reg(isl.left.x))));
                self.cpu_set_destination(f, x, isl.left.y, reach);
            }
        } else if me.cur_pos.x > isl.right.x {
            let x = mem(fp::frsp(fp::fsub(reg(isl.right.x), 5.0)));
            self.cpu_set_destination(f, x, isl.right.y, reach);
        }
    }

    /// `mpCheckFloor` from `(ax, ay)` to `(bx, by)` unless the floor is one computer players
    /// don't count (`ftCo_800A1B38`): its point.
    pub(super) fn cpu_floor(
        &mut self,
        ax: f32,
        ay: f32,
        bx: f32,
        by: f32,
    ) -> Option<crate::vec::Vec3> {
        let hit = self
            .game
            .geometry
            .as_mut()
            .expect("no collision geometry")
            .check_floor(ax, ay, bx, by, 0.0, -1, -1, -1, None)?;
        (!self.excluded_floor(hit.line)).then_some(hit.pos)
    }

    /// [`Running::cpu_floor`] as the behavior pick (`ftCo_800ADE48`) runs it, with the r5 it
    /// leaves ([`FloorR5`]): `sp` is the pick's stack pointer, which the stack addresses
    /// left there are relative to (unfollowed without it). A floor found is checked against
    /// the stage's excluded floors (`ftCo_800A1B38`), which on the four stages that have them
    /// is a call that leaves r5 its own way ([`Running::excluded_floor_r5`]).
    pub(super) fn cpu_floor_r5(
        &mut self,
        ax: f32,
        ay: f32,
        bx: f32,
        by: f32,
        sp: Option<u32>,
    ) -> (Option<crate::vec::Vec3>, Leftover) {
        let (hit, left) = self
            .game
            .geometry
            .as_mut()
            .expect("no collision geometry")
            .check_floor_r5(ax, ay, bx, by, 0.0, -1, -1, -1);
        let r5 = match (left, sp) {
            (FloorR5::Flag(flag), _) => Leftover::Word(u32::from(flag)),
            (FloorR5::Entry, Some(sp)) => Leftover::Word(sp + BEHAVIOR_FLAGS_SLOT),
            (FloorR5::OwnSlot, Some(sp)) => {
                Leftover::Word(sp - CHECK_FLOOR_FRAME + CHECK_FLOOR_SLOT)
            }
            (_, None) => Leftover::Unknown("mpCheckFloor (a stack address, its frame unmodeled)"),
        };
        let Some(hit) = hit else {
            return (None, r5);
        };
        if self.excluded_floor_asks_stage() {
            // The r5 the stage's check leaves, followed only where the DI can read it
            // (`Rules::follows_r5`).
            let r5 = if self.game.rules.follows_r5() {
                self.excluded_floor_r5(hit.line)
            } else {
                Leftover::Unknown("ftCo_800A1B38 (not followed: NanaDeterminism clears r5)")
            };
            return ((!self.excluded_floor(hit.line)).then_some(hit.pos), r5);
        }
        (Some(hit.pos), r5)
    }

    /// The player's standing word (`MatchPlayerData::x20`) in the results as they stand,
    /// `gm_8016C75C`: the results brought up to date once a tick.
    #[doc(alias = "gm_8016C75C")]
    pub(super) fn cpu_standing(&mut self, slot: i32) -> u32 {
        let tick = self.game.scene_ticks();
        if self.game.vs.state.match_end.standings_tick != tick {
            self.game.vs.state.match_end.standings_tick = tick;
            self.compile_results();
        }
        self.game.vs.state.match_end.player_standings[slot as usize].kos
    }

    /// Whether the fighter holds someone, `ftCo_IsGrabbing`: its catch wait, Donkey Kong's
    /// carry, Bowser's Koopa Klaw hold, Kirby's inhale and swallow.
    #[doc(alias = "ftCo_IsGrabbing")]
    pub(super) fn cpu_is_grabbing(&self, f: FighterId) -> bool {
        let fp = &self.game.fighters[f.0];
        // `ftCo_MS_CatchWait`.
        if fp.motion_id == 216 {
            return true;
        }
        let ms = fp.motion_id;
        match fp.fighter_kind() {
            // Ft_Kind_Donkey: `ftDk_MS_ThrowFWait0` to `_ThrowFKneebend`, carrying a fighter.
            FighterKind::Donkey => (350..=355).contains(&ms),
            // Ft_Kind_Koopa, Ft_Kind_GKoops: `ftKp_MS_SpecialSHit0_1`.
            FighterKind::Koopa | FighterKind::GKoopa => ms == super::koopa::ms::SPECIAL_S_HIT0_1,
            // Ft_Kind_Kirby: `ftKb_MS_SpecialNCapture0` to `_EatTurnAir`.
            FighterKind::Kirby => (super::kirby::ms::SPECIAL_N_CAPTURE0
                ..=super::kirby::ms::EAT_TURN_AIR)
                .contains(&ms),
            _ => false,
        }
    }

    /// Whether the fighter falls with nothing to land on, `ftCo_800A2C80`: on the ground
    /// over a floor computer players don't count; in the air falling steeply (not past
    /// Peach's Castle's and Fourside's floors), outside its stage's reach, with neither a
    /// floor nor a wall in reach along its fall.
    #[doc(alias = "ftCo_800A2C80")]
    fn cpu_falling_away(&mut self, f: FighterId) -> bool {
        let fpp = &self.game.fighters[f.0];
        if fpp.cpu.off_stage {
            return false;
        }
        if fpp.ground_or_air == GroundOrAir::Ground {
            return self.excluded_floor(fpp.coll_data.floor.index);
        }
        // `ftCo_MS_Pass`.
        if fpp.motion_id == 244 {
            return false;
        }
        let mut dir = fpp.pos_delta;
        let v = reg(dir.x);
        let vy = reg(dir.y);
        let mag = fp::fadds(fp::fmuls(v, v), fp::fmuls(vy, vy));
        let tiny = reg(1e-5);
        if mag < tiny && mag > fp::fneg(tiny) {
            return false;
        }
        if vy > 0.0 {
            return false;
        }
        let v = if v < 0.0 { fp::fneg(v) } else { v };
        if super::co::guard::stick_angle(vy, v) > fp::fneg(1.047_197_543_084_621_4) {
            return false;
        }
        if fpp.cpu.drifting_back {
            return false;
        }
        match self.game.stage_info.ground_kind() {
            GrKind::Inishie1 => {
                if fpp.cur_pos.y < 5.0 {
                    return true;
                }
            }
            GrKind::Fourside if fpp.cur_pos.y < -35.0 => return true,
            _ => {}
        }
        dir.normalize();
        let coll = &fpp.coll_data;
        let ax = mem(fp::fadds(reg(coll.cur_pos.x), reg(coll.ecb.bottom.x)));
        let ay = mem(fp::fadds(reg(coll.cur_pos.y), reg(coll.ecb.bottom.y)));
        let ex = mem(fp::fmadds(1000.0, reg(dir.x), reg(ax)));
        let ey = mem(fp::fmadds(1000.0, reg(dir.y), reg(ay)));
        if let Some(p) = self.cpu_floor(ax, ay, ex, ey)
            && !self.cpu_out_of_bounds(f, p.x, p.y)
        {
            return false;
        }
        let geo = self.game.geometry.as_mut().expect("no collision geometry");
        if let Some(h) = geo.check_left_wall(ax, ay, ex, ey, -1, -1)
            && !self.cpu_out_of_bounds(f, h.pos.x, h.pos.y)
        {
            return false;
        }
        let geo = self.game.geometry.as_mut().expect("no collision geometry");
        if let Some(h) = geo.check_right_wall(ax, ay, ex, ey, -1, -1)
            && !self.cpu_out_of_bounds(f, h.pos.x, h.pos.y)
        {
            return false;
        }
        true
    }

    /// Whether the fighter shields now, `ftCo_800A5ACC`: if it may (`xF9_b6`), more often the
    /// higher its level.
    #[doc(alias = "ftCo_800A5ACC")]
    fn cpu_shield_chance(&self, f: FighterId) -> bool {
        let c = &self.game.fighters[f.0].cpu;
        if !c.may_shield {
            return false;
        }
        let t = c.frame_counter % 120;
        match c.level {
            2 => t > 0x64,
            3 => t > 0x50,
            4 => t > 0x3C,
            5 => t > 0x28,
            6 => t > 0x1E,
            7 => t > 0x14,
            8 => t > 0xA,
            9 => true,
            _ => false,
        }
    }

    /// Whether the fighter picks up the item it goes for, `ftCo_800A3710`: if it may
    /// (`xF9_b7`), the item is still grabbable (else it forgets it), and both stand on the
    /// same ground with the fighter's hands free.
    #[doc(alias = "ftCo_800A3710")]
    fn cpu_item_pickup(&mut self, f: FighterId) -> bool {
        let c = &self.game.fighters[f.0].cpu;
        if !c.may_pick_up {
            return false;
        }
        let Some(item) = c.item_target else {
            return false;
        };
        let ig = self.game.items[item.0]
            .entity
            .expect("the item the fighter goes for has its GObj");
        if !super::super::it::item::is_grabbable(self.game, ig) {
            self.game.fighters[f.0].cpu.item_target = None;
            return false;
        }
        let fpp = &self.game.fighters[f.0];
        if fpp.ground_or_air == GroundOrAir::Air || fpp.item_gobj.is_some() {
            return false;
        }
        // `ftCo_800A2170_it`: both on the same island.
        if self.game.items[item.0].ground_or_air == GroundOrAir::Air {
            return false;
        }
        let geo = self.game.geometry.as_ref().expect("no collision geometry");
        let Some(island) = geo.island_of_line(fpp.coll_data.floor.index) else {
            return false;
        };
        if geo.island_of_line(self.game.items[item.0].coll.floor.index) != Some(island) {
            return false;
        }
        let g = fpp.gobj.expect("the fighter's GObj");
        self.item_pickup_in_reach(g, ig)
    }

    /// Whether the fighter has reached its destination, `ftCo_800A3554`: on the ground on
    /// the destination's island, within its reach (`x38`) plus `margin`. A destination timer
    /// still running then moves it to the next destination instead.
    #[doc(alias = "ftCo_800A3554")]
    pub(super) fn cpu_at_destination(&mut self, f: FighterId, margin: f32) -> bool {
        let fpp = &self.game.fighters[f.0];
        if fpp.ground_or_air == GroundOrAir::Air {
            return false;
        }
        // `ftCo_800A21FC`: the destination's island is the fighter's.
        let line = fpp.coll_data.floor.index;
        let dest = crate::vec::Vec3::new(
            fpp.cpu.destination.x,
            mem(fp::frsp(fp::fadd(5.0, reg(fpp.cpu.destination.y)))),
            0.0,
        );
        let geo = self.game.geometry.as_mut().expect("no collision geometry");
        let Some(island) = geo.island_of_line(line) else {
            return false;
        };
        if geo.island_below(dest, -10.0) != Some(island) {
            return false;
        }
        let fpp = &self.game.fighters[f.0];
        let c = &fpp.cpu;
        let x = fp::fsubs(reg(c.destination.x), reg(fpp.cur_pos.x));
        let y = fp::fsubs(reg(c.destination.y), reg(fpp.cur_pos.y));
        if math::sqrt_reg(fp::fmadds(x, x, fp::fmuls(y, y)))
            < fp::fadds(reg(c.destination_reach), reg(margin))
        {
            if c.destination_timer != 0 {
                let c = &mut self.game.fighters[f.0].cpu;
                c.destination_timer = 0;
                c.destination.x = c.home.x;
                c.destination.y = c.home.y;
                self.cpu_stage_route(f);
                return false;
            }
            return true;
        }
        false
    }
}
