//! Ice Shot (`ftpopospecialn.c`): the Ice Climbers form a block of ice by the hand and send
//! it sliding ([`crate::game::it::kinds::climbersice`]); in the air the first shot gives a
//! hop, later ones none.

use gekko_fp as fp;

use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::{ICE_CLIMBER_ICE, climbersice};
use super::super::super::state::Running;
use super::super::sound::Track;
use super::{KIND_POPO, addr, var};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;

/// `ftPp_MS_SpecialN`, `ftPp_MS_SpecialAirN`.
const MS_SPECIAL_N: i32 = 0x155;
const MS_SPECIAL_AIR_N: i32 = 0x156;

/// `ftIceClimberAttributes` offsets: the air shot's hop, the landing lag, the ice's offset
/// from the hand.
mod attr {
    pub const AIR_HOP: usize = 0x4;
    pub const LANDING_LAG: usize = 0x8;
    pub const ICE_X: usize = 0xC;
    pub const ICE_Y: usize = 0x10;
}

/// The shot's sounds and the voices.
const SFX_FORM: i32 = 0x1FBE5;
const SFX_THROW: i32 = 0x1FBE8;
const VOICE_POPO: i32 = 0x1FC5D;
const VOICE_NANA: i32 = 0x1FC2A;

impl Running<'_> {
    /// The item an Ice Climber's special holds (`x222C`).
    pub fn ic_item(&self, gobj: GObjId) -> Option<GObjId> {
        self.game.fighter(gobj).u.gobj_at(var::ITEM)
    }

    /// Forgets the special's item if it's `item`, with its hooks, `ftPp_Init_8011F16C`.
    // Ports `ftPp_Init_8011F16C`; aliased once a check reaches it.
    pub fn popo_forget_item(&mut self, gobj: GObjId, item: GObjId) {
        if self.ic_item(gobj) != Some(item) {
            return;
        }
        let fp = self.game.fighter_mut(gobj);
        fp.u.set_gobj_at(var::ITEM, None);
        fp.death2_cb = None;
        fp.take_dmg_cb = None;
    }

    /// Takes the special's ice away, `ftPp_Init_8011F190`.
    // Ports `ftPp_Init_8011F190`; aliased once a check reaches it.
    fn popo_remove_ice(&mut self, gobj: GObjId) {
        if let Some(item) = self.ic_item(gobj) {
            climbersice::remove(self, item);
            self.popo_forget_item(gobj, item);
        }
    }

    /// The special's start (`ftPp_SpecialN_Enter` and the air one's shared part): no throw
    /// flags, the script's flag cleared, no ice yet.
    fn popo_special_n_start(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.x2210 = [0; 4];
        fp.cmd_vars[0] = 0;
        fp.u.set_gobj_at(var::ITEM, None);
    }

    /// Ice Shot on the ground, `ftPp_SpecialN_Enter`.
    // Ports `ftPp_SpecialN_Enter`; aliased once a check reaches it.
    pub fn popo_special_n_enter(&mut self, gobj: GObjId) {
        self.popo_special_n_start(gobj);
        self.change_motion_state(gobj, MS_SPECIAL_N, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::PP_SPECIAL_N_ICE));
    }

    /// Ice Shot in the air, `ftPp_SpecialAirN_Enter`: the first since landing hops.
    // Ports `ftPp_SpecialAirN_Enter`; aliased once a check reaches it.
    pub fn popo_special_air_n_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.popo_special_n_start(gobj);
        let hop = self.ic_attr_f32(f, attr::AIR_HOP);
        let fp = &mut self.game.fighters[f.0];
        if fp.u_i32(var::X224C) == 0 {
            fp.self_vel.y = hop;
            fp.set_u_i32(var::X224C, 1);
            fp.set_u_i32(var::X2250, 0);
        } else {
            fp.set_u_i32(var::X2250, (-10.0f32).to_bits() as i32);
        }
        self.change_motion_state(gobj, MS_SPECIAL_AIR_N, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::PP_SPECIAL_N_ICE));
    }

    /// Forms and throws the ice as the script says (`cmd_vars[0]`), `ftPp_SpecialN_8011F500`:
    /// 1 forms it by the hand (offset forward and up, lower after an air shot), 2 throws it,
    /// with the voice.
    // Ports `ftPp_SpecialN_8011F500`; aliased once a check reaches it.
    pub fn popo_special_n_ice(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        match self.game.fighters[f.0].cmd_vars[0] {
            1 => {
                let (dx, dy) = (
                    self.ic_attr_f32(f, attr::ICE_X),
                    self.ic_attr_f32(f, attr::ICE_Y),
                );
                let j = self.bone(f, 0).joint;
                let mut pos =
                    crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None);
                let fp = &self.game.fighters[f.0];
                let x2250 = f32::from_bits(fp.u_i32(var::X2250) as u32);
                pos.x = mem(fp::fmadds(reg(dx), reg(fp.facing_dir), reg(pos.x)));
                pos.y = mem(fp::fadds(reg(pos.y), fp::fadds(reg(dy), reg(x2250))));
                let facing = fp.facing_dir;
                let ice = climbersice::spawn(self, gobj, pos, ICE_CLIMBER_ICE, facing);
                self.game.fighter_mut(gobj).u.set_gobj_at(var::ITEM, ice);
                self.fighter_play_sfx(f, SFX_FORM, 127, 64);
                if ice.is_some() {
                    let fp = self.game.fighter_mut(gobj);
                    fp.death2_cb = Some(CodeRef(addr::PP_REMOVE_ITEMS));
                    fp.take_dmg_cb = Some(CodeRef(addr::PP_REMOVE_ITEMS));
                }
                self.game.fighter_mut(gobj).cmd_vars[0] = 0;
            }
            2 => {
                let Some(ice) = self.ic_item(gobj) else {
                    return;
                };
                climbersice::throw(self, ice);
                self.game.fighter_mut(gobj).cmd_vars[0] = 0;
                let voice = if self.game.fighters[f.0].kind == KIND_POPO {
                    VOICE_POPO
                } else {
                    VOICE_NANA
                };
                self.fighter_play_track(f, Track::Voice, voice, 127, 64);
                self.fighter_play_sfx(f, SFX_THROW, 127, 64);
                self.popo_forget_item(gobj, ice);
            }
            _ => {}
        }
    }

    /// The ground shot's animation, `ftPp_SpecialN_Anim`: waits once it ends.
    // Ports `ftPp_SpecialN_Anim`; aliased once a check reaches it.
    fn popo_special_n_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// The air shot's animation, `ftPp_SpecialAirN_Anim`: falls once it ends.
    // Ports `ftPp_SpecialAirN_Anim`; aliased once a check reaches it.
    fn popo_special_air_n_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// The ground shot's collision, `ftPp_SpecialN_Coll`: off a ledge, the ice goes and the
    /// shot falls.
    // Ports `ftPp_SpecialN_Coll`; aliased once a check reaches it.
    fn popo_special_n_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.popo_remove_ice(gobj);
            self.enter_fall(gobj);
        }
    }

    /// The air shot's collision, `ftPp_SpecialAirN_Coll`: landing, the ice goes, the air
    /// shot's hop is back and the landing is a special fall's.
    // Ports `ftPp_SpecialAirN_Coll`; aliased once a check reaches it.
    fn popo_special_air_n_coll(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.air_coll_land(gobj) {
            self.popo_remove_ice(gobj);
            let fp = &mut self.game.fighters[f.0];
            fp.set_u_i32(var::X224C, 0);
            fp.set_u_i32(var::X2250, 0);
            let lag = self.ic_attr_f32(f, attr::LANDING_LAG);
            self.enter_landing_fall_special(gobj, 0, lag);
        }
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::PP_SPECIAL_N_ENTER => g.popo_special_n_enter(gobj),
        addr::PP_SPECIAL_AIR_N_ENTER => g.popo_special_air_n_enter(gobj),
        addr::PP_SPECIAL_N_ANIM => g.popo_special_n_anim(gobj),
        addr::PP_SPECIAL_AIR_N_ANIM => g.popo_special_air_n_anim(gobj),
        addr::PP_SPECIAL_N_IASA | addr::PP_SPECIAL_AIR_N_IASA => {}
        addr::PP_SPECIAL_N_PHYS => g.ground_friction(gobj),
        addr::PP_SPECIAL_AIR_N_PHYS => g.air_physics_no_drift(gobj),
        addr::PP_SPECIAL_N_COLL => g.popo_special_n_coll(gobj),
        addr::PP_SPECIAL_AIR_N_COLL => g.popo_special_air_n_coll(gobj),
        addr::PP_SPECIAL_N_ICE => g.popo_special_n_ice(gobj),
        _ => return false,
    }
    true
}
