//! Ceiling techs (`ftCo_PassiveCeil.c`): a tumbling fighter that hugs a ceiling with the
//! tech's input clings to it for a moment (`PassiveCeil`), then falls.

use gekko_fp as fp;

use super::super::super::ef;
use super::super::super::state::Running;
use super::super::coll::PLATFORM_FILTER;
use super::ids::ms;
use super::jump::FT_800881D8;
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::mp::coll::CEILING_HUG;
use crate::vec::Vec3;

/// The tech's rumble, color animation and sound.
const RUMBLE: u32 = 12;
const COLANIM: i32 = 120;
const SFX_CLING: i32 = 3;

impl Running<'_> {
    /// A ceiling tech, `ftCo_800C23A0`: hugging a ceiling with the tech's input
    /// (`ftCo_800986B0`).
    #[doc(alias = "ftCo_800C23A0")]
    pub fn passive_ceil_check(&mut self, gobj: GObjId) -> bool {
        if self.game.fighter(gobj).coll_data.env_flags & CEILING_HUG != 0 && self.tech_input(gobj) {
            self.passive_ceil(gobj);
            return true;
        }
        false
    }

    /// Clinging to the ceiling, `ftCo_800C23FC`: stopped, an effect where the ECB's top meets
    /// it, the state, the fighter set under the ceiling by its TransN, the landing on walls'
    /// pass, sounds, a rumble and a flash.
    #[doc(alias = "ftCo_800C23FC")]
    pub fn passive_ceil(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.stop_dead(gobj);
        let fp = &mut self.game.fighters[f.0];
        fp.x2210 = [0; 4];
        let offset = Vec3::new(0.0, fp.coll_data.ecb.top.y, 0.0);
        self.kirby_bump_effect(gobj, offset);
        self.change_motion_state(gobj, ms::PASSIVE_CEIL, 0, 0.0, 1.0, 0.0, None);
        let fp = &self.game.fighters[f.0];
        let y = fp::fadds(reg(fp.cur_pos.y), reg(offset.y));
        let pos = Vec3::new(fp.cur_pos.x, mem(y), fp.cur_pos.z);
        self.fighter_effect(
            gobj,
            ef::SpawnKind::PosOffset,
            0x41D,
            None,
            &[ef::Arg::Vec(pos)],
        );
        let fp = &mut self.game.fighters[f.0];
        fp.cur_pos.y = mem(fp::fadds(y, reg(fp.x68C_transNPos.y)));
        // `ftCo_80090574`.
        self.air_coll_ledges_snap(gobj);
        let sfx = self.character_sfx(gobj, 0x24);
        self.play_track(gobj, FT_800881D8, sfx);
        self.fighter_play_sfx(f, SFX_CLING, 0x7F, 0x40);
        self.start_rumble(f, RUMBLE, 0);
        self.start_color_anim(gobj, COLANIM, 0);
    }

    /// `ftCo_PassiveCeil_Anim`: drifting with the stick while the script allows it, the fall
    /// once the animation ends.
    #[doc(alias = "ftCo_PassiveCeil_Anim")]
    pub fn co_passive_ceil_anim(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.throw_flag(3) {
            fp.self_vel.x = mem(fp::fmuls(
                reg(fp.input.lstick[0].x),
                reg(fp.co_attrs.passiveceil_vel_x),
            ));
        }
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// `ftCo_PassiveCeil_Phys`: gravity or a fast fall, and air drift (`ft_80084DB0`).
    #[doc(alias = "ftCo_PassiveCeil_Phys")]
    pub fn co_passive_ceil_phys(&mut self, gobj: GObjId) {
        self.air_physics(gobj);
    }

    /// `ftCo_PassiveCeil_Coll`: the filtered airborne pass, landing normally.
    #[doc(alias = "ftCo_PassiveCeil_Coll")]
    pub fn co_passive_ceil_coll(&mut self, gobj: GObjId) {
        self.air_coll_filtered_full(gobj, PLATFORM_FILTER, |g, gobj| g.land_normally(gobj));
    }
}
