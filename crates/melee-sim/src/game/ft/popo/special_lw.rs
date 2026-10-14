//! Blizzard (`ftpopospeciallw.c`): the Ice Climbers blow freezing wind from the hand, a puff
//! every few frames ([`crate::game::it::kinds::climbersblizzard`]) while the script says, the
//! body leaning with the floor's slope on the ground.

use gekko_fp as fp;

use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::climbersblizzard;
use super::super::super::state::Running;
use super::super::sound::Track;
use super::{KIND_POPO, addr, var};
use crate::fpu::{mem, reg};
use crate::game::ef::Arg;
use crate::hsd::gobj::GObjId;
use crate::math;

/// `ftPp_MS_SpecialLw`, `ftPp_MS_SpecialAirLw`.
pub const MS_SPECIAL_LW: i32 = 357;
pub const MS_SPECIAL_AIR_LW: i32 = 358;

/// The state change's flags between the ground and air Blizzards.
const SWITCH_FLAGS: u32 = 0x0C4C_5282;

/// `FtPart_L3rdNa`, `FtPart_L4thNb`: the bones (indexing `parts` directly) the puffs and
/// the wind's effect come from.
const PART_L3RD_NA: i32 = 26;
const PART_L4TH_NB: i32 = 29;

/// `ftIceClimberAttributes` offsets: the frames between puffs, and the puffs' offset.
mod attr {
    pub const PUFF_FRAMES: usize = 0xB8;
    pub const PUFF_X: usize = 0xBC;
    pub const PUFF_Y: usize = 0xC0;
}

/// The state's variables (`mv.pp.speciallw`): the frames to the next puff, and whether it
/// blows (`x4_b0`).
mod mv {
    pub const PUFF_TIMER: usize = 0x0;
    pub const BLOWING: usize = 0x4;
}

/// The wind's effect, voices and sounds, Popo's then Nana's.
const GFX_WIND: i32 = 0x4EC;
const VOICE: [i32; 2] = [0x1FC42, 0x1FC0F];
const SFX: [i32; 2] = [0x1FBEB, 0x1FBEE];

impl Running<'_> {
    /// The Blizzard's start (shared by both entries): no throw flags, the script's flags and
    /// the state's variables cleared.
    fn popo_special_lw_start(&mut self, gobj: GObjId, msid: i32) {
        let fp = self.game.fighter_mut(gobj);
        fp.x2210 = [0; 4];
        fp.cmd_vars[0] = 0;
        fp.cmd_vars[3] = 0;
        fp.set_mv_i32(mv::PUFF_TIMER, 0);
        fp.mv[mv::BLOWING] &= 0x7F;
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::PP_SPECIAL_LW_BLOW));
    }

    /// Blizzard on the ground, `ftPp_SpecialLw_Enter`.
    #[doc(alias = "ftPp_SpecialLw_Enter")]
    pub fn popo_special_lw_enter(&mut self, gobj: GObjId) {
        self.popo_special_lw_start(gobj, MS_SPECIAL_LW);
    }

    /// Blizzard in the air, `ftPp_SpecialAirLw_Enter`.
    // Ports `ftPp_SpecialAirLw_Enter`; aliased once a check reaches it.
    pub fn popo_special_air_lw_enter(&mut self, gobj: GObjId) {
        self.popo_special_lw_start(gobj, MS_SPECIAL_AIR_LW);
    }

    /// Stops the wind (`ftPp_SpecialHi_80122898`, inline in the animations): its effects
    /// gone, the hooks cleared, the lean undone.
    fn popo_blizzard_stop(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].u[var::X2230] & 0x80 != 0 {
            crate::game::ef::eflib::destroy_all(self, gobj);
            let fp = &mut self.game.fighters[f.0];
            fp.u[var::X2230] &= 0x7F;
            fp.death2_cb = None;
            fp.take_dmg_cb = None;
            self.game.part_set_rot_x(f, 0, 0.0);
        }
    }

    /// The ground Blizzard's animation, `ftPp_SpecialLw_Anim`: at its end the wind stops and
    /// the fighter waits.
    // Ports `ftPp_SpecialLw_Anim`; aliased once a check reaches it.
    fn popo_special_lw_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.popo_blizzard_stop(gobj);
            self.enter_wait(gobj);
        }
    }

    /// The air Blizzard's animation, `ftPp_SpecialAirLw_Anim`: at its end it falls.
    // Ports `ftPp_SpecialAirLw_Anim`; aliased once a check reaches it.
    fn popo_special_air_lw_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.popo_blizzard_stop(gobj);
            self.enter_fall(gobj);
        }
    }

    /// The death and damage hooks the wind sets (`ftPp_set_cbs`).
    fn popo_set_remove_hooks(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.death2_cb = Some(CodeRef(addr::PP_REMOVE_ITEMS));
        fp.take_dmg_cb = Some(CodeRef(addr::PP_REMOVE_ITEMS));
    }

    /// Leans with the floor while the script says (`cmd_vars[3]`), else upright
    /// (`ftPp_SpecialLw_Coll_inline`).
    fn popo_blizzard_lean(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        let v = if fp.cmd_vars[3] != 0 {
            let n = fp.coll_data.floor.normal;
            mem(fp::fmuls(reg(fp.facing_dir), reg(math::atan2(n.x, n.y))))
        } else {
            0.0
        };
        self.game.part_set_rot_x(f, 0, v);
    }

    /// Landing in the air Blizzard, `fn_80122B54`: the ground one from the same frame.
    // Ports `fn_80122B54`; aliased once a check reaches it.
    fn popo_special_lw_land(&mut self, gobj: GObjId) {
        self.land(gobj);
        let frame = self.game.fighter(gobj).cur_anim_frame;
        self.change_motion_state(gobj, MS_SPECIAL_LW, SWITCH_FLAGS, frame, 1.0, 0.0, None);
        self.popo_set_remove_hooks(gobj);
        self.popo_blizzard_lean(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::PP_SPECIAL_LW_BLOW));
    }

    /// The ground Blizzard's collision, `ftPp_SpecialLw_Coll`: off a ledge it carries on in
    /// the air, upright; else it leans.
    // Ports `ftPp_SpecialLw_Coll`; aliased once a check reaches it.
    fn popo_special_lw_coll(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if !self.ground_coll_off_ledges(gobj) {
            self.game.part_set_rot_x(f, 0, 0.0);
            // `ftCommon_GroundToAirStateChange`.
            self.go_airborne(f);
            let frame = self.game.fighters[f.0].cur_anim_frame;
            self.change_motion_state(gobj, MS_SPECIAL_AIR_LW, SWITCH_FLAGS, frame, 1.0, 0.0, None);
            self.popo_set_remove_hooks(gobj);
            self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::PP_SPECIAL_LW_BLOW));
            self.clamp_air_drift(f);
        } else {
            self.popo_blizzard_lean(gobj);
        }
    }

    /// Blows the wind, `fn_80122D2C`: while blowing, a puff from the hand every few frames;
    /// the script starts the wind (its effect, the hooks, the voice) or stops it.
    // Ports `fn_80122D2C`; aliased once a check reaches it.
    pub fn popo_special_lw_blow(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].mv[mv::BLOWING] & 0x80 != 0 {
            if self.game.fighters[f.0].mv_i32(mv::PUFF_TIMER) == 0 {
                let (dx, dy) = (
                    self.ic_attr_f32(f, attr::PUFF_X),
                    self.ic_attr_f32(f, attr::PUFF_Y),
                );
                let frames = self.ic_attr_f32(f, attr::PUFF_FRAMES);
                let j = self.bone_joint(gobj, PART_L3RD_NA);
                let mut pos =
                    crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None);
                let facing = self.game.fighters[f.0].facing_dir;
                pos.x = mem(fp::fmadds(reg(dx), reg(facing), reg(pos.x)));
                pos.y = mem(fp::fadds(reg(pos.y), reg(dy)));
                let _ = climbersblizzard::spawn(self, gobj, pos, facing);
                self.game.fighters[f.0].set_mv_i32(mv::PUFF_TIMER, fp::fctiwz(reg(frames)));
            }
            let fp = &mut self.game.fighters[f.0];
            let t = fp.mv_i32(mv::PUFF_TIMER);
            fp.set_mv_i32(mv::PUFF_TIMER, t.wrapping_sub(1));
        }
        match self.game.fighters[f.0].cmd_vars[0] {
            1 => {
                let j = self.bone_joint(gobj, PART_L4TH_NB);
                crate::game::ef::spawn(self, GFX_WIND, Some(gobj), &[Arg::JObj(j)]);
                let fp = &mut self.game.fighters[f.0];
                fp.mv[mv::BLOWING] |= 0x80;
                fp.u[var::X2230] |= 0x80;
                self.popo_set_remove_hooks(gobj);
                self.game.fighters[f.0].cmd_vars[0] = 0;
                let i = usize::from(self.game.fighters[f.0].kind != KIND_POPO);
                self.fighter_play_track(f, Track::Voice, VOICE[i], 127, 64);
                self.fighter_play_track(f, Track::Effect2, SFX[i], 127, 64);
            }
            2 => {
                let fp = &mut self.game.fighters[f.0];
                fp.mv[mv::BLOWING] &= 0x7F;
                fp.cmd_vars[0] = 0;
            }
            _ => {}
        }
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::PP_SPECIAL_LW_ENTER => g.popo_special_lw_enter(gobj),
        addr::PP_SPECIAL_AIR_LW_ENTER => g.popo_special_air_lw_enter(gobj),
        addr::PP_SPECIAL_LW_ANIM => g.popo_special_lw_anim(gobj),
        addr::PP_SPECIAL_AIR_LW_ANIM => g.popo_special_air_lw_anim(gobj),
        addr::PP_SPECIAL_LW_IASA | addr::PP_SPECIAL_AIR_LW_IASA => {}
        addr::PP_SPECIAL_LW_PHYS => g.ground_friction(gobj),
        addr::PP_SPECIAL_AIR_LW_PHYS => g.air_physics_no_drift(gobj),
        addr::PP_SPECIAL_LW_COLL => g.popo_special_lw_coll(gobj),
        addr::PP_SPECIAL_AIR_LW_COLL => {
            if g.air_coll_land(gobj) {
                g.popo_special_lw_land(gobj);
            }
        }
        addr::PP_SPECIAL_LW_BLOW => g.popo_special_lw_blow(gobj),
        _ => return false,
    }
    true
}
