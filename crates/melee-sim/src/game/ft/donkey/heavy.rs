//! Donkey Kong carrying a heavy item (`ftdonkeyheavy*.c`, `ftdonkeyms3450.c`): a box, barrel or
//! Party Ball on his shoulders in his own states, 341 to 350, counted from the first one his
//! cargo attributes name (`x2CC`'s `motion_state`; the cargo states after his forward throw
//! count from its `x4_motion_state`): wait, three walks, turn, knee bend, fall, jump, landing
//! and the hurt wait. Unlike the common carry (`ft::co::lift`) he walks, turns, jumps and
//! falls with it, and only a hit hard enough (or a throw) makes him let go. Its states are
//! the cargo's but for the turn ([`HEAVY`], `ft::co::cargo`'s [`Carry`]).

use gekko_fp as fp;

use super::super::super::it::release;
use super::super::super::state::Running;
use super::super::co::cargo::Carry;
use crate::fpu::reg;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// The hurt wait, past the first state.
const WAIT2: i32 = 9;

/// `mv.dk`: the turn's `x0` (has turned, `ftCo_Turn_Enter`'s).
const TURN_HAS_TURNED: usize = 0x0;

/// The heavy item's carry: its first state the cargo attributes' first word, its throw the
/// heavy item's, landed first by its wait, with no rider and no drop through platforms.
pub const HEAVY: Carry = Carry {
    first: 0,
    throw: |g, gobj| g.heavy_throw_check(gobj),
    pass: false,
    rider: false,
    wait_lands: true,
    fall: |g, gobj| g.enter_carry_fall(gobj, &HEAVY),
    landing: |g, gobj| g.enter_carry_landing(gobj, &HEAVY),
};

/// The states' callbacks.
pub mod addr {
    pub const WAIT0_IASA: u32 = 0x800D_F9F0;
    pub const WAIT0_PHYS: u32 = 0x800D_FA50;
    pub const WAIT0_COLL: u32 = 0x800D_FA70;
    pub const WALK_ANIM: u32 = 0x800D_FB34;
    pub const WALK_IASA: u32 = 0x800D_FB54;
    pub const WALK_PHYS: u32 = 0x800D_FBB8;
    pub const WALK_COLL: u32 = 0x800D_FBD8;
    pub const TURN_ANIM: u32 = 0x800E_01BC;
    pub const TURN_IASA: u32 = 0x800E_0200;
    pub const TURN_PHYS: u32 = 0x800E_0254;
    pub const TURN_COLL: u32 = 0x800E_0274;
    pub const FALL_IASA: u32 = 0x800E_030C;
    pub const FALL_PHYS: u32 = 0x800E_0330;
    pub const FALL_COLL: u32 = 0x800E_0350;
    pub const JUMP_IASA: u32 = 0x800E_0440;
    pub const JUMP_PHYS: u32 = 0x800E_0464;
    pub const JUMP_COLL: u32 = 0x800E_0484;
    pub const WAIT1_ANIM: u32 = 0x800E_0518;
    pub const WAIT1_IASA: u32 = 0x800E_0568;
    pub const WAIT1_PHYS: u32 = 0x800E_05A4;
    pub const WAIT1_COLL: u32 = 0x800E_05C4;
    pub const LANDING_ANIM: u32 = 0x800E_0648;
    pub const LANDING_PHYS: u32 = 0x800E_0698;
    pub const LANDING_COLL: u32 = 0x800E_06B8;
    pub const WAIT2_ANIM: u32 = 0x800E_07E4;
    pub const WAIT2_PHYS: u32 = 0x800E_0848;
    pub const WAIT2_COLL: u32 = 0x800E_0868;
}

/// Runs one of the heavy states' callbacks, if `a` is one.
#[melee_callbacks::callbacks(Fighter)]
pub fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    let s = &HEAVY;
    match a {
        addr::WAIT0_IASA => g.carry_wait_iasa(gobj, s),
        addr::WAIT0_PHYS | addr::TURN_PHYS | addr::WAIT1_PHYS | addr::LANDING_PHYS => {
            g.carry_ground_phys(gobj)
        }
        addr::WAIT0_COLL
        | addr::WALK_COLL
        | addr::TURN_COLL
        | addr::WAIT1_COLL
        | addr::LANDING_COLL => g.carry_wait_coll(gobj, s),
        addr::WALK_ANIM => g.carry_walk_anim(gobj),
        addr::WALK_IASA => g.carry_walk_iasa(gobj, s),
        addr::WALK_PHYS => g.carry_walk_phys(gobj),
        addr::TURN_ANIM => g.dk_heavy_turn_anim(gobj),
        addr::TURN_IASA => g.dk_heavy_turn_iasa(gobj),
        addr::FALL_IASA | addr::JUMP_IASA => g.carry_air_iasa(gobj, s),
        addr::FALL_PHYS => g.carry_fall_phys(gobj),
        addr::FALL_COLL | addr::JUMP_COLL => g.carry_fall_coll(gobj, s),
        addr::JUMP_PHYS => g.co_cargo_jump_phys(gobj),
        addr::WAIT1_ANIM => g.carry_knee_bend_anim(gobj, s),
        addr::WAIT1_IASA => g.carry_knee_bend_iasa(gobj, s),
        addr::LANDING_ANIM => g.carry_landing_anim(gobj, s),
        addr::WAIT2_ANIM => g.carry_wait2_anim(gobj, s),
        addr::WAIT2_PHYS => g.carry_wait2_phys(gobj),
        addr::WAIT2_COLL => g.carry_wait2_coll(gobj),
        _ => return false,
    }
    true
}

impl Running<'_> {
    /// `ftDk_HeavyTurn_Anim`: the turn, then the wait when it ends.
    #[doc(alias = "ftDk_HeavyTurn_Anim")]
    pub fn dk_heavy_turn_anim(&mut self, gobj: GObjId) {
        self.turn_anim_inner(gobj);
        if !self.anim_frames_remaining(gobj) {
            self.enter_carry_wait(gobj, &HEAVY);
        }
    }

    /// `ftDk_HeavyTurn_IASA`: once turned, a throw or a jump.
    #[doc(alias = "ftDk_HeavyTurn_IASA")]
    pub fn dk_heavy_turn_iasa(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).mv_i32(TURN_HAS_TURNED) != 0 && !self.heavy_throw_check(gobj) {
            self.carry_jump_check(gobj, &HEAVY);
        }
    }

    /// A hit while carrying the item, `ftDk_MS_349_800E06D8`: a light one (`checkSomething`)
    /// leaves him hurt with it in the hurt wait (`doSomething`); a harder one makes him drop
    /// it and react as anyone does.
    #[doc(alias = "ftDk_MS_349_800E06D8")]
    pub fn dk_heavy_hit(&mut self, gobj: GObjId) {
        if self.dk_heavy_hit_keeps(gobj) {
            self.dk_heavy_hit_keep(gobj);
        } else {
            let item = self.game.fighter(gobj).item_gobj.expect("carrying nothing");
            release::drop(self, item, Vec3::new(0.0, 0.0, 0.0), 1.0);
            self.damage_reaction(gobj, 0.0);
        }
    }

    /// Whether a hit leaves him carrying, `checkSomething`: too light to react to, or
    /// knockback below the third level (`ftCo_8008D8E8` of `ftCo_ScaleBy154`).
    #[doc(alias = "checkSomething")]
    #[doc(alias = "ftCo_ScaleBy154")]
    #[doc(alias = "ftCo_8008D8E8")]
    pub fn dk_heavy_hit_keeps(&self, gobj: GObjId) -> bool {
        let f = self.game.fighter_id(gobj);
        if self.hit_too_light(f) {
            return true;
        }
        let c = self.common();
        let scaled = fp::fmuls(
            reg(self.game.fighters[f.0].dmg.kb_applied),
            reg(c.f32(0x154)),
        );
        // `ftCo_8008D8E8` < 3: below `ftCommonData` x160 (and so below x158 and x15C).
        let level = if scaled < reg(c.f32(0x158)) {
            0
        } else if scaled < reg(c.f32(0x15C)) {
            1
        } else if scaled < reg(c.f32(0x160)) {
            2
        } else {
            3
        };
        level < 3
    }

    /// Hurt carrying the item, `doSomething`: hitstun in the hurt wait.
    #[doc(alias = "doSomething")]
    pub fn dk_heavy_hit_keep(&mut self, gobj: GObjId) {
        let msid = self.carry_state(gobj, &HEAVY, WAIT2);
        self.enter_damage(gobj, msid, 0.0);
    }
}
