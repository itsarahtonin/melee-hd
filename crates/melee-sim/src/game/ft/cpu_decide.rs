//! What a computer player does next (`ftCo_0A01.c`): whom it targets and which items it goes
//! for (`ftCo_800A4BEC`, `ftCo_800A5F4C`, `ftCo_800A648C`), and the behavior it picks
//! (`ftCo_800ADE48`) from what it and its target are doing, kept in `CpuFighter::x18`, which
//! [`super::cpu_ai`]'s script writer acts on.

use gekko_fp as fp;

use super::super::code::unported_call;
use super::super::gr::stage;
use super::super::handles::{FighterId, ItemId};
use super::super::state::Running;
use super::common::{GA_AIR, GA_GROUND};
use super::cpu_ai::P_LINK_FIGHTER;
use crate::fpu::{mem, reg};
use crate::math;

/// `HSD_GOBJ_PLINK_ITEM`.
const P_LINK_ITEM: u8 = 9;

/// `It_Kind_L_Gun_Ray`: the first item kind past the common items.
const IT_KIND_L_GUN_RAY: i32 = 0x23;

/// Behaviors (`CpuFighter::x18`) the decision switches to.
pub mod behavior {
    pub const WAIT_OUT: i32 = 4;
    pub const DI: i32 = 0x12;
}

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
        fp.x2219_b1 || fp.x2164 != 0 || (fp.x2168 != 0 && fp.x2338.x == 0) || fp.is_sleeping
    }

    /// Whether a fighter is invisible to computer players for now, `ftCo_800A0F00`: under a
    /// cloaking item's color animation (`ft_80087A18`), always or, flickering, half the time
    /// by its own frame count.
    #[doc(alias = "ftCo_800A0F00")]
    fn cpu_cloaked(&self, f: FighterId) -> bool {
        // `ft_80087A18`: only with a powered-up color animation (`x2226_b4`).
        if self.game.fighters[f.0].x2226_b4 {
            unported_call("ft_80087A18 with a power-up's color animation");
        }
        false
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

    /// The fighter's target, `ftCo_800A4BEC`: its last one while that one lasts (`xF9_b0`),
    /// else the nearest enemy in reach that isn't out of the match, invisible or dead; a new
    /// target gets a fresh timer.
    #[doc(alias = "ftCo_800A4BEC")]
    pub(super) fn cpu_find_target(&mut self, f: FighterId) -> Option<FighterId> {
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
            if self.cpu_out_of_bounds(f, pos.x, pos.y) || self.cpu_is_ally(f, o) {
                continue;
            }
            if self.cpu_untargetable(o) {
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
            let c = &self.game.fighters[f.0].cpu;
            if c.xF9_b0 {
                if c.x48 == Some(o) {
                    return Some(o);
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
        match closest {
            None => {
                self.game.fighters[f.0].cpu.xF9_b0 = false;
                None
            }
            Some((o, _)) => {
                self.game.fighters[f.0].cpu.xF9_b0 = true;
                let r = reg(self.game.objs.rng.draw_f32());
                let c = &mut self.game.fighters[f.0].cpu;
                c.x30 = fp::fctiwz(fp::fmuls(120.0, fp::fmuls(0.5, fp::fmuls(0.5, r))));
                c.x48 = Some(o);
                Some(o)
            }
        }
    }

    /// Whether an item heals, `ftCo_800A5908`: a heart, a tomato or food.
    #[doc(alias = "ftCo_800A5908")]
    fn cpu_item_heals(&self, item: ItemId) -> bool {
        // `It_Kind_Heart`, `_Tomato`, `_Foods`.
        matches!(self.game.items[item.0].kind, 8 | 9 | 0x12)
    }

    /// The common item of `kind` (any for `It_Kind_L_Gun_Ray`) the fighter goes for,
    /// `ftCo_800A5F4C`: grabbable, in reach and worth at least its threshold (`x2C`). The
    /// original measures each candidate's distance with an inline `sqrtf` whose zero test
    /// reads a register its caller left (`f29`), so a candidate stops the port.
    #[doc(alias = "ftCo_800A5F4C")]
    fn cpu_common_item_target(&mut self, f: FighterId, kind: i32) -> Option<ItemId> {
        let mut cur = self.game.world.first_in(P_LINK_ITEM);
        while let Some(g) = cur {
            cur = self.game.world.gobjs[g.0].next;
            if !super::super::it::item::is_grabbable(self.game, g) {
                continue;
            }
            let it = super::super::it::item_of(self.game, g);
            let ip = &self.game.items[it.0];
            if kind != IT_KIND_L_GUN_RAY && ip.kind != kind {
                continue;
            }
            if self.cpu_out_of_bounds(f, ip.pos.x, ip.pos.y) || ip.kind >= IT_KIND_L_GUN_RAY {
                continue;
            }
            unported_call("ftCo_800A5F4C with a candidate item (its sqrtf reads the caller's f29)");
        }
        None
    }

    /// The enemy item the fighter goes for, `ftCo_800A648C`: Goombas, Redeads and the other
    /// stage enemies in reach. Its distance test reads a caller's register as
    /// [`Running::cpu_common_item_target`]'s does, so a candidate stops the port.
    #[doc(alias = "ftCo_800A648C")]
    fn cpu_enemy_item_target(&mut self, f: FighterId) -> Option<ItemId> {
        let mut cur = self.game.world.first_in(P_LINK_ITEM);
        while let Some(g) = cur {
            cur = self.game.world.gobjs[g.0].next;
            let it = super::super::it::item_of(self.game, g);
            let ip = &self.game.items[it.0];
            // `ftCo_800A648C_inline1`: `It_Kind_Kuriboh` up to `_Octarock_Stone`,
            // `_Nokonoko`, `_Patapata` and the next, `_Whitebea`.
            let k = ip.kind;
            let enemy = (0xA0..0xAE).contains(&k)
                || k == 0xB2
                || (k.wrapping_sub(0xB4) as u32) <= 1
                || k == 0xB8;
            if enemy && !self.cpu_out_of_bounds(f, ip.pos.x, ip.pos.y) {
                unported_call(
                    "ftCo_800A648C with a candidate item (its sqrtf reads the caller's f29)",
                );
            }
        }
        None
    }

    /// Nana making her way back to Popo, `ftCo_800B0760`: she targets the nearest enemy and
    /// the items in reach, acts toward her destination (or recovers, `xFB_b0`), and picks a
    /// behavior.
    #[doc(alias = "ftCo_800B0760")]
    pub(super) fn nana_regroup(&mut self, f: FighterId) {
        let c = &mut self.game.fighters[f.0].cpu;
        c.xF8_b0 = true;
        c.xF9_b2 = true;
        c.xF9_b4 = true;
        c.xF9_b3 = false;
        c.xF9_b5 = false;
        c.xF9_b6 = false;
        c.xF9_b7 = false;
        c.xF9_b1 = false;
        let target = self.cpu_find_target(f);
        self.game.fighters[f.0].cpu.x44 = target;
        // `ftCo_CpuUpdateCommonItemTarget`.
        let fpp = &self.game.fighters[f.0];
        let holds_other = fpp.item_gobj.is_some_and(|g| {
            let it = super::super::it::item_of(self.game, g);
            !self.cpu_item_heals(it)
        });
        let item = if holds_other || fpp.x2168 != 0 {
            None
        } else {
            self.cpu_common_item_target(f, IT_KIND_L_GUN_RAY)
        };
        self.game.fighters[f.0].cpu.x4C = item;
        // `ftCo_CpuUpdateSpecialItemTarget`.
        let x50 = self.cpu_enemy_item_target(f);
        self.game.fighters[f.0].cpu.x50 = x50;
        // `inlineI3`.
        if self.cpu_should_act(f) {
            if self.game.fighters[f.0].cpu.xFB_b0 {
                unported_call("ftCo_800A8DE4");
            } else {
                self.cpu_toward_partner(f);
            }
        }
        // Every caller keeps its `CpuFighter` pointer in r31, which the decision reads as
        // its DI switch (see [`Running::cpu_choose_behavior`]).
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
        // The destination: kept if over ground in reach, else reset.
        let c = &self.game.fighters[f.0].cpu;
        let (dx, dy) = (c.x54.x, c.x54.y);
        let below = mem(fp::frsp(fp::fsub(reg(dy), 5.0)));
        let above = mem(fp::frsp(fp::fadd(5.0, reg(dy))));
        let floor = self.cpu_floor(dx, above, dx, below);
        let keep = floor.is_some() && !self.cpu_out_of_bounds(f, dx, dy);
        if !keep {
            let fpp = &mut self.game.fighters[f.0];
            fpp.cpu.xFA_b2 = false;
            let pos = fpp.cur_pos;
            let at = if fpp.ground_or_air == GA_AIR {
                let to = mem(fp::fsubs(reg(pos.y), 1000.0));
                self.cpu_floor(pos.x, pos.y, pos.x, to)
                    .map_or((pos.x, pos.y), |p| (p.x, p.y))
            } else {
                (pos.x, pos.y)
            };
            let c = &mut self.game.fighters[f.0].cpu;
            c.x54.x = at.0;
            c.x54.y = at.1;
        }
        let fpp = &self.game.fighters[f.0];
        let ddy = fp::fsubs(reg(fpp.cur_pos.y), reg(fpp.cpu.x54.y));
        let ddx = fp::fsubs(reg(fpp.cur_pos.x), reg(fpp.cpu.x54.x));
        let dist = math::sqrt_reg(fp::fmadds(ddx, ddx, fp::fmuls(ddy, ddy)));
        let slot = i32::from(fpp.player_idx);
        let c = &mut self.game.fighters[f.0].cpu;
        c.x5C = mem(dist);
        if c.x88 > 0 {
            c.x88 -= 1;
        }
        // `gm_8016C75C`: the player's standing.
        let standing = self.standing_x20(slot);
        if self.game.fighters[f.0].cpu.x8C != standing as i32 {
            self.game.fighters[f.0].cpu.x88 = 0x12C;
            let again = self.standing_x20(slot);
            self.game.fighters[f.0].cpu.x8C = again as i32;
        }
        let c = &self.game.fighters[f.0].cpu;
        let period = if c.kind == 0xE { 120 } else { 400 };
        if c.x7C % period == 0 {
            let r = reg(self.game.objs.rng.draw_f32());
            self.game.fighters[f.0].cpu.xF8_b5 = r > 0.5;
        }
        let fpp = &mut self.game.fighters[f.0];
        if !fpp.x221B.x221B_b5 {
            fpp.cpu.x94 = 0;
        }
        // DI in hitstun.
        if fpp.cpu.x18 == behavior::DI {
            return true;
        }
        let switch =
            if fpp.kind == 0x1F || fpp.cpu.kind == 0xF || fpp.cpu.kind == 0 || !fpp.x221A_b3 {
                false
            } else {
                fpp.cpu.xF9_b0 = false;
                let r = reg(self.game.objs.rng.draw_f32());
                let c = &mut self.game.fighters[f.0].cpu;
                let level = fp::fsubs(f64::from(c.level), 0.0);
                c.xFA_b1 = fp::fmuls(0.100_000_001_490_116_12, level) > r;
                di_switch
            };
        if switch {
            self.cpu_script_clear(f);
            self.game.fighters[f.0].cpu.x18 = behavior::DI;
            return true;
        }
        let x18 = self.game.fighters[f.0].cpu.x18;
        let ms = self.game.fighters[f.0].motion_id;
        // Each check: the behavior, whether the fighter is in it already, and whether it
        // switches to it.
        macro_rules! switch_to {
            ($b:expr) => {{
                self.cpu_script_clear(f);
                self.game.fighters[f.0].cpu.x18 = $b;
                return true;
            }};
        }
        // Leaving a barrel (`ftCo_MS_BarrelWait`, `ftCo_MS_Barrel`).
        if x18 == 0x11 {
            return true;
        }
        if ms == 0x125 || ms == 0x154 {
            switch_to!(0x11);
        }
        // Riding a Warp Star.
        if x18 == 0x13 {
            return true;
        }
        if ms == 0x131 || ms == 0x132 {
            switch_to!(0x13);
        }
        // Lying down.
        if x18 == 5 {
            return true;
        }
        if matches!(ms, 0xB7 | 0xB8 | 0xBF | 0xC0) {
            switch_to!(5);
        }
        // On a ledge.
        if x18 == 6 {
            return true;
        }
        if matches!(ms, 0xFC | 0xFD) {
            switch_to!(6);
        }
        // Holding a fighter.
        if x18 == 9 {
            return true;
        }
        if self.cpu_is_grabbing(f) {
            switch_to!(9);
        }
        if self.game.fighters[f.0].cpu.x18 == behavior::WAIT_OUT {
            return true;
        }
        if self.cpu_falling_away(f) {
            switch_to!(behavior::WAIT_OUT);
        }
        let x18 = self.game.fighters[f.0].cpu.x18;
        let fpp = &self.game.fighters[f.0];
        // A crouch at level 5 and up.
        if x18 == 0xF {
            return true;
        }
        if fpp.cpu.level >= 5 && fpp.motion_id == 0x26 {
            switch_to!(0xF);
        }
        // Held.
        if x18 == 0x10 {
            return true;
        }
        let ms = fpp.motion_id;
        if ms == 0xE0 || ms == 0xE3 || (0x10A..=0x10E).contains(&ms) {
            switch_to!(0x10);
        }
        // Shielding.
        if x18 == 7 {
            unported_call("ftCo_800BB9B4");
        }
        if self.cpu_shield_chance(f) {
            unported_call("ftCo_800BB9B4");
        }
        if self.game.fighters[f.0].cpu.x18 == 3 {
            return true;
        }
        if self.cpu_ranged_attack(f) {
            switch_to!(3);
        }
        if self.game.fighters[f.0].cpu.x18 == 2 {
            return true;
        }
        if self.cpu_attack(f) {
            switch_to!(2);
        }
        if self.game.fighters[f.0].cpu.x18 == 8 {
            return true;
        }
        let jump = self.cpu_jump_attack(f);
        let c = &mut self.game.fighters[f.0].cpu;
        c.xF8_b34 = (jump & 3) as u8;
        if c.xF8_b34 != 0 {
            c.x18 = 8;
            return true;
        }
        if self.game.fighters[f.0].cpu.x18 == 0xD {
            return true;
        }
        if self.cpu_item_pickup(f) {
            self.game.fighters[f.0].cpu.x18 = 0xD;
            return true;
        }
        if self.game.fighters[f.0].cpu.x18 == 0xE {
            return true;
        }
        if let Some(item) = self.game.fighters[f.0].item_gobj {
            let _ = item;
            unported_call("ftCo_800ADE48 with an item held");
        }
        let c = &self.game.fighters[f.0].cpu;
        if c.x18 == 0xA {
            return true;
        }
        if c.x20 != 0 && self.cpu_at_destination(f, 0.0) {
            let c = &mut self.game.fighters[f.0].cpu;
            c.x18 = c.x20;
            return true;
        }
        false
    }

    /// Sets the fighter's destination and how near it must get, unless a destination timer
    /// runs (`ftCo_800A1F3C`), then lets the stage move it (`ftCo_800A1CC4`).
    #[doc(alias = "ftCo_800A1F3C")]
    fn cpu_set_destination(&mut self, f: FighterId, x: f32, y: f32, reach: f32) {
        let c = &mut self.game.fighters[f.0].cpu;
        if c.x60 == 0 {
            c.x54.x = x;
            c.x54.y = y;
            c.x38 = reach;
            self.cpu_stage_route(f);
        }
    }

    /// The stage's routes for computer players (`ftCo_800A1CC4` with `ftCo_803C6594`, by
    /// stage kind): only Great Bay and the Temple have any.
    #[doc(alias = "ftCo_800A1CC4")]
    fn cpu_stage_route(&mut self, f: FighterId) {
        // Gr_Kind_GreatBay, Gr_Kind_Shrine.
        if matches!(self.game.stage_info.grkind, 6 | 7) {
            let _ = f;
            unported_call("ftCo_800A1CC4 on a stage with routes");
        }
    }

    /// Whether an island is unsafe to stand on, `ftCo_800A2718`: a live bob-omb or shell on
    /// it, or the stage's own hazard (Mushroom Kingdom's moving islands; Zebes' and Onett's
    /// checks aren't ported).
    #[doc(alias = "ftCo_800A2718")]
    fn cpu_island_unsafe(&self, island: Option<crate::mp::IslandId>) -> bool {
        let Some(island) = island else { return false };
        let geo = self.game.geometry.as_ref().expect("no collision geometry");
        let mut cur = self.game.world.first_in(P_LINK_ITEM);
        while let Some(g) = cur {
            cur = self.game.world.gobjs[g.0].next;
            let it = super::super::it::item_of(self.game, g);
            let ip = &self.game.items[it.0];
            // `it_8026C1B4`: a hitbox on.
            let live = ip.x5D4_hitboxes.iter().take(4).any(|h| h.hit.state != 0);
            // `ftCo_800A5944`: `It_Kind_BombHei`, `_G_Shell`, `_R_Shell`.
            if !live || !matches!(ip.kind, 6 | 14 | 15) {
                continue;
            }
            if geo.island_of_line(ip.x378_itemColl.floor.index) == Some(island) {
                return true;
            }
        }
        match self.game.stage_info.grkind {
            // Gr_Kind_Story.
            10 => geo.island_moving(island),
            // Gr_Kind_Zebes, Gr_Kind_Onett.
            8 | 20 => unported_call("ftCo_800A2718 on Zebes or Onett"),
            _ => false,
        }
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
        let below = mem(fp::frsp(fp::fsub(reg(ppos.y), 2.0)));
        let above = mem(fp::frsp(fp::fadd(2.0, reg(ppos.y))));
        // Either way his position: the floor check only decides which copy.
        let _ = self.cpu_floor(ppos.x, above, ppos.x, below);
        self.cpu_set_destination(f, ppos.x, ppos.y, reach);
        let me = &self.game.fighters[f.0];
        let p = &self.game.fighters[partner.0];
        let geo = self.game.geometry.as_ref().expect("no collision geometry");
        let same_island = me.ground_or_air != GA_AIR
            && p.ground_or_air != GA_AIR
            && geo
                .island_of_line(me.coll_data.floor.index)
                .is_some_and(|i| geo.island_of_line(p.coll_data.floor.index) == Some(i));
        if same_island || matches!(me.motion_id, 252 | 253) {
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

    /// The player's standing word (`MatchPlayerData::x20`) in the results as they stand,
    /// `gm_8016C75C`: the results brought up to date once a tick.
    #[doc(alias = "gm_8016C75C")]
    fn standing_x20(&mut self, slot: i32) -> u32 {
        let tick = self.game.scene_ticks();
        if self.game.vs.state.x24C.x0 != tick {
            self.game.vs.state.x24C.x0 = tick;
            self.game.compile_results();
        }
        self.game.vs.state.x24C.player_standings[slot as usize].x20
    }

    /// Whether the fighter holds someone, `ftCo_IsGrabbing`: its catch wait, Donkey Kong's
    /// carry, Bowser's Koopa Klaw hold, Kirby's inhale.
    #[doc(alias = "ftCo_IsGrabbing")]
    pub(super) fn cpu_is_grabbing(&self, f: FighterId) -> bool {
        let fp = &self.game.fighters[f.0];
        // `ftCo_MS_CatchWait`.
        if fp.motion_id == 216 {
            return true;
        }
        match fp.kind {
            3 | 4 | 5 | 31 => unported_call("ftCo_IsGrabbing for Donkey Kong, Kirby or Bowser"),
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
        if fpp.cpu.xFA_b6 {
            return false;
        }
        if fpp.ground_or_air == GA_GROUND {
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
        if fpp.cpu.xFA_b5 {
            return false;
        }
        match self.game.stage_info.grkind {
            // Gr_Kind_Inishie1, Gr_Kind_Fourside.
            24 => {
                if fpp.cur_pos.y < 5.0 {
                    return true;
                }
            }
            21 if fpp.cur_pos.y < -35.0 => return true,
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
        if !c.xF9_b6 {
            return false;
        }
        let t = c.x7C % 120;
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
        if !c.xF9_b7 {
            return false;
        }
        let Some(_item) = c.x4C else { return false };
        unported_call("ftCo_800A3710 with an item to pick up");
    }

    /// Whether the fighter has reached its destination, `ftCo_800A3554`: on the ground on
    /// the destination's island, within its reach (`x38`) plus `margin`. A destination timer
    /// still running then moves it to the next destination instead.
    #[doc(alias = "ftCo_800A3554")]
    pub(super) fn cpu_at_destination(&mut self, f: FighterId, margin: f32) -> bool {
        let fpp = &self.game.fighters[f.0];
        if fpp.ground_or_air == GA_AIR {
            return false;
        }
        // `ftCo_800A21FC`: the destination's island is the fighter's.
        let line = fpp.coll_data.floor.index;
        let dest = crate::vec::Vec3::new(
            fpp.cpu.x54.x,
            mem(fp::frsp(fp::fadd(5.0, reg(fpp.cpu.x54.y)))),
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
        let x = fp::fsubs(reg(c.x54.x), reg(fpp.cur_pos.x));
        let y = fp::fsubs(reg(c.x54.y), reg(fpp.cur_pos.y));
        if math::sqrt_reg(fp::fmadds(x, x, fp::fmuls(y, y))) < fp::fadds(reg(c.x38), reg(margin)) {
            if c.x60 != 0 {
                let c = &mut self.game.fighters[f.0].cpu;
                c.x60 = 0;
                c.x54.x = c.x64.x;
                c.x54.y = c.x64.y;
                unported_call("ftCo_800A1CC4");
            }
            return true;
        }
        false
    }
}
