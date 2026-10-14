//! Squall Hammer, Nana's side (`ftnanaspecials.c`): when Popo starts his and she's near
//! enough and not in hitstun, Nana joins him at his place (state 359 on the ground, 360 in the
//! air), frozen with him (`x1A5C` each other), at his animation frame and speeds. Once his
//! script lets her go she spins on alone a little behind him, until he's done.
//!
//! [`Running::nana_join_special_s`] hosts the Slippi ruleset's freeze glitch fix
//! (`External/FreezeGlitchFix`), which keeps her `x1A5C` when she's in hitstun
//! ([`crate::game::state::Rules::freeze_glitch_fix`]).

use gekko_fp as fp;

use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::super::common::GA_AIR;
use super::addr;
use super::special_s::spin_box;
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;

/// Nana's states (`ftPp_MS_SpecialS_0`, `_1`).
pub const MS_GROUND: i32 = 359;
pub const MS_AIR: i32 = 360;

/// The state change's flags between her ground and air states.
const SWITCH_FLAGS: u32 = 0x0C4C_528A;

/// `ftIceClimberAttributes` offsets: how near Popo she must be, and her helpless fall's
/// landing lag (0: no helpless fall).
mod attr {
    pub const RANGE: usize = 0xD0;
    pub const LANDING_LAG: usize = 0x12C;
}

/// Her frames behind Popo since he let her go (`mv.pp.unk_80123954.x0`), up to 5.
const MV_BEHIND: usize = 0x0;

impl Running<'_> {
    /// Nana's hooks while spinning (`ftNn_Init_80123B3C_inline`): a hit or a KO lets go
    /// (`ftNn_Init_80122FAC`), and her effects pause in hitlag.
    fn nana_special_s_hooks(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.take_dmg_cb = Some(CodeRef(addr::NN_SPECIAL_S_RELEASE));
        fp.death2_cb = Some(CodeRef(addr::NN_SPECIAL_S_RELEASE));
        self.set_effect_hitlag_cbs(gobj);
    }

    /// Nana lets go, `ftNn_Init_801238E4` (and its hook `ftNn_Init_80122FAC`): upright, and
    /// she and the fighter she froze with thaw and forget each other.
    #[doc(alias = "ftNn_Init_801238E4")]
    #[doc(alias = "ftNn_Init_80122FAC")]
    pub fn nana_special_s_release(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.game.part_set_rot_x(f, 0, 0.0);
        self.ic_thaw(gobj);
        if let Some(other) = self.game.fighters[f.0].x1A5C {
            self.ic_thaw(other);
            self.game.fighter_mut(other).x1A5C = None;
        }
        self.game.fighters[f.0].x1A5C = None;
    }

    /// Whether Nana stays out of Popo's Squall Hammer, `ftNn_Init_80123954` (`pp_ga` is
    /// where he starts it): without her, in hitstun (`x2071_b0_3` 1 to 13), or out of reach,
    /// she does (true). Else she joins, into her ground or air state as he is, at his place
    /// and speeds, facing his way and frozen with him (false).
    ///
    /// In hitstun the original clears her `x1A5C`; the Slippi ruleset's freeze glitch fix
    /// (`801239A8`, a nop over that store) keeps it.
    #[doc(alias = "ftNn_Init_80123954")]
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    pub fn nana_join_special_s(&mut self, nana: Option<GObjId>, pp_ga: i32) -> bool {
        let Some(nana) = nana else {
            return true;
        };
        let n = self.game.fighter_id(nana);
        let slot = i32::from(self.game.fighters[n.0].player_idx);
        let popo_gobj = self
            .game
            .player_entity_at(slot, 0)
            .expect("Nana without a Popo");
        let p = self.game.fighter_id(popo_gobj);
        if matches!(self.game.fighters[n.0].x2070[1] >> 4, 1..=13) {
            if !self.game.rules.freeze_glitch_fix {
                self.game.fighters[n.0].x1A5C = None;
            }
            return true;
        }
        let dd = reg(self.ic_attr_f32(n, attr::RANGE));
        let (np, pp) = (
            self.game.fighters[n.0].cur_pos,
            self.game.fighters[p.0].cur_pos,
        );
        let dx = fp::fsubs(reg(np.x), reg(pp.x));
        let dy = fp::fsubs(reg(np.y), reg(pp.y));
        let d2 = fp::fadds(fp::fmuls(dx, dx), fp::fmuls(dy, dy));
        let scale = reg(self.game.fighters[p.0].x34_scale.y);
        let reach = fp::frsp(f64::from(fp::fctiwz(fp::fmuls(scale, fp::fmuls(dd, dd)))));
        if !(d2 < reach) {
            self.game.fighters[n.0].x1A5C = None;
            return true;
        }
        let ga = self.game.fighters[n.0].ground_or_air;
        if pp_ga == GA_AIR {
            if pp_ga != ga {
                self.go_airborne(n);
            }
            self.nana_enter_special_s(nana, MS_AIR);
        } else {
            if pp_ga != ga {
                self.copy_coll_data(p, n);
                self.land(nana);
            }
            self.nana_enter_special_s(nana, MS_GROUND);
        }
        let pf = &self.game.fighters[p.0];
        let (pos, vel, gr, facing) = (pf.cur_pos, pf.self_vel, pf.gr_vel, pf.facing_dir);
        let fp = &mut self.game.fighters[n.0];
        fp.set_mv_i32(MV_BEHIND, 0);
        fp.cur_pos = pos;
        fp.self_vel = vel;
        fp.gr_vel = gr;
        fp.facing_dir = facing;
        let rot = mem(fp::frsp(fp::fmul(
            std::f64::consts::FRAC_PI_2,
            reg(fp.facing_dir),
        )));
        self.game.part_set_rot_y(n, 0, rot);
        self.game.fighters[n.0].x1A5C = Some(popo_gobj);
        false
    }

    /// Whether Nana isn't in her Squall Hammer, `ftNn_Init_80123B10`.
    #[doc(alias = "ftNn_Init_80123B10")]
    pub fn nana_out_of_special_s(&self, gobj: GObjId) -> bool {
        !matches!(self.game.fighter(gobj).motion_id, MS_GROUND | MS_AIR)
    }

    /// Into her state `ms` from its start (`ftNn_Init_80123B3C`, `ftNn_Init_80123BF0`), no
    /// vertical speed.
    #[doc(alias = "ftNn_Init_80123B3C")]
    #[doc(alias = "ftNn_Init_80123BF0")]
    fn nana_enter_special_s(&mut self, gobj: GObjId, ms: i32) {
        self.game.fighter_mut(gobj).cmd_vars = [0; 4];
        self.nana_special_s_hooks(gobj);
        self.change_motion_state(gobj, ms, 0, 0.0, 1.0, 0.0, None);
        self.nana_special_s_hooks(gobj);
        self.anim_update(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.x74_self_accel.y = 0.0;
        fp.self_vel.y = 0.0;
    }

    /// Nana's animation's shared start: while Popo spins with her she keeps his frame (false);
    /// once he's alone she lets go and the hooks go (true).
    fn nana_special_s_anim(&mut self, gobj: GObjId) -> bool {
        let popo = self.nana_popo_of(gobj);
        if !self.popo_special_s_alone(popo) {
            let frame = self.game.fighter(popo).cur_anim_frame;
            self.game.fighter_mut(gobj).cur_anim_frame = frame;
            return false;
        }
        self.nana_special_s_release(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.take_dmg_cb = None;
        fp.death2_cb = None;
        fp.pre_hitlag_cb = None;
        fp.post_hitlag_cb = None;
        fp.x1A5C = None;
        true
    }

    /// Nana's Popo (`Player_GetEntityAtIndex(slot, 0)`), who's there while she spins.
    fn nana_popo_of(&self, gobj: GObjId) -> GObjId {
        let slot = i32::from(self.game.fighter(gobj).player_idx);
        self.game
            .player_entity_at(slot, 0)
            .expect("Nana's Squall Hammer without Popo")
    }

    /// Nana's physics (`ftPp_SpecialS_0_Phys`, `ftPp_SpecialS_1_Phys`): Popo's speeds and
    /// facing.
    #[doc(alias = "ftPp_SpecialS_0_Phys")]
    #[doc(alias = "ftPp_SpecialS_1_Phys")]
    fn nana_special_s_phys(&mut self, gobj: GObjId) {
        let popo = self.nana_popo_of(gobj);
        let p = self.game.fighter(popo);
        let (vel, accel, gr, gr_accel, facing) = (
            p.self_vel,
            p.x74_self_accel,
            p.gr_vel,
            p.xE4_ground_accel_1,
            p.facing_dir,
        );
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel = vel;
        fp.x74_self_accel = accel;
        fp.gr_vel = gr;
        fp.xE4_ground_accel_1 = gr_accel;
        fp.facing_dir = facing;
        let rot = mem(fp::frsp(fp::fmul(std::f64::consts::FRAC_PI_2, reg(facing))));
        self.game.part_set_rot_y(f, 0, rot);
    }

    /// Nana's collision's shared body (`ftPp_SpecialS_0_Coll_inline4` and `_1`'s): while
    /// Popo spins with her and holds her, she's at his place with his collision, switching to
    /// `same` when they part between the ground and the air; once he lets her go she drifts
    /// up to 7.5 behind him and her own collision runs (on the ground with `ground`), off a
    /// ledge or landing into her other state. Then his body's lean and her hooks.
    fn nana_special_s_coll(&mut self, gobj: GObjId, same: i32, ground: bool) {
        let popo = self.nana_popo_of(gobj);
        let (n, p) = (self.game.fighter_id(gobj), self.game.fighter_id(popo));
        if !self.popo_special_s_alone(popo) && !self.popo_special_s_lets_go(Some(popo)) {
            self.game.fighters[n.0].cur_pos = self.game.fighters[p.0].cur_pos;
            self.copy_coll_data(p, n);
            let pga = self.game.fighters[p.0].ground_or_air;
            if self.game.fighters[n.0].ground_or_air != pga {
                let frame = self.game.fighters[n.0].cur_anim_frame;
                self.change_motion_state(gobj, same, SWITCH_FLAGS, frame, 1.0, 0.0, None);
            }
            self.game.fighters[n.0].ground_or_air = pga;
        } else {
            let px = self.game.fighters[p.0].cur_pos.x;
            let fp = &mut self.game.fighters[n.0];
            let b = fp.mv_i32(MV_BEHIND).wrapping_add(1).min(5);
            fp.set_mv_i32(MV_BEHIND, b);
            fp.cur_pos.x = mem(fp::fnmsubs(
                1.5,
                fp::fmuls(fp::frsp(f64::from(b)), reg(fp.facing_dir)),
                reg(px),
            ));
            if ground {
                if !self.ground_coll_box(gobj, &spin_box()) {
                    // `ftPp_SpecialS_0_Coll_inline3`.
                    self.game.fighters[n.0].cmd_vars = [0; 4];
                    self.nana_special_s_hooks(gobj);
                    self.go_airborne(n);
                    let frame = self.game.fighters[n.0].cur_anim_frame;
                    self.change_motion_state(gobj, MS_AIR, SWITCH_FLAGS, frame, 1.0, 0.0, None);
                    self.nana_special_s_hooks(gobj);
                }
            } else if self.air_coll_box(gobj, &spin_box()) {
                // `ftPp_SpecialS_1_Coll_inline3`.
                self.game.fighters[n.0].cmd_vars = [0; 4];
                self.nana_special_s_hooks(gobj);
                self.land(gobj);
                self.game.fighters[n.0].self_vel.y = 0.0;
                let frame = self.game.fighters[n.0].cur_anim_frame;
                self.change_motion_state(gobj, MS_GROUND, SWITCH_FLAGS, frame, 1.0, 0.0, None);
                self.nana_special_s_hooks(gobj);
            }
        }
        let rot = self.game.part_rot_x(p, 0);
        self.game.part_set_rot_x(n, 0, rot);
        self.nana_special_s_hooks(gobj);
    }
}

/// The states' callbacks, one item each.
impl Running<'_> {
    /// Once Popo's alone, Nana stands.
    #[doc(alias = "ftPp_SpecialS_0_Anim")]
    fn nana_special_s_0_anim(&mut self, gobj: GObjId) {
        if self.nana_special_s_anim(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// Once Popo's alone, Nana falls (helpless with a landing lag).
    #[doc(alias = "ftPp_SpecialS_1_Anim")]
    fn nana_special_s_1_anim(&mut self, gobj: GObjId) {
        if self.nana_special_s_anim(gobj) {
            let f = self.game.fighter_id(gobj);
            let lag = self.ic_attr_f32(f, attr::LANDING_LAG);
            if 0.0 == reg(lag) {
                self.enter_fall(gobj);
            } else {
                self.go_airborne_jumped(f);
                self.enter_fall_special(gobj, 1, 0, 1, 1.0, lag, 0.0, true);
            }
        }
    }

    #[doc(alias = "ftPp_SpecialS_0_IASA")]
    #[doc(alias = "ftPp_SpecialS_1_IASA")]
    fn nana_special_s_no_iasa(&mut self, _gobj: GObjId) {}

    #[doc(alias = "ftPp_SpecialS_0_Coll")]
    fn nana_special_s_0_coll(&mut self, gobj: GObjId) {
        self.nana_special_s_coll(gobj, MS_AIR, true);
    }

    #[doc(alias = "ftPp_SpecialS_1_Coll")]
    fn nana_special_s_1_coll(&mut self, gobj: GObjId) {
        self.nana_special_s_coll(gobj, MS_GROUND, false);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    use addr::*;
    match a {
        NN_SPECIAL_S_RELEASE => g.nana_special_s_release(gobj),
        NN_SPECIAL_S_0_ANIM => g.nana_special_s_0_anim(gobj),
        NN_SPECIAL_S_1_ANIM => g.nana_special_s_1_anim(gobj),
        NN_SPECIAL_S_0_IASA | NN_SPECIAL_S_1_IASA => g.nana_special_s_no_iasa(gobj),
        NN_SPECIAL_S_0_PHYS | NN_SPECIAL_S_1_PHYS => g.nana_special_s_phys(gobj),
        NN_SPECIAL_S_0_COLL => g.nana_special_s_0_coll(gobj),
        NN_SPECIAL_S_1_COLL => g.nana_special_s_1_coll(gobj),
        _ => return false,
    }
    true
}
