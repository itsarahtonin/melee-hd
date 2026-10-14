//! Link's bombs (`ftlinkspeciallw.c`): pulled into his hand on the script's flag, or, held
//! already, thrown.

use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::linkbomb;
use super::super::super::it::lib26b1;
use super::super::super::state::Running;
use super::{addr, attr, ms};
use crate::hsd::gobj::GObjId;

/// `ftCo_MS_LightThrowF4` and `ftCo_MS_LightThrowAirF4`: a held item's throws the special
/// takes over.
const MS_THROW_F4: i32 = 108;
const MS_THROW_AIR_F4: i32 = 112;
/// `FtPart_LThumbNb`, where the bomb appears.
const PART_L_THUMB_NB: i32 = 31;
/// The ground and air versions' collision change: the animation and the bomb's hook carry on.
const COLL_MF: u32 = 0x80 | 0x1000 | 0x4000 | 0x4_0000 | 0x8_0000 | 0x40_0000;
const COLL_MF_HI: u32 = 0x400_0000 | 0x800_0000;

impl Running<'_> {
    /// `ftLk_SpecialLw_Enter` and `ftLk_SpecialAirLw_Enter` (`doEnter`): a held bomb is thrown
    /// (`updateBomb`, any other held item stops the special), else the pull, its bomb made on
    /// the script's flag ([`addr::SPAWN_BOMB`]).
    #[doc(alias = "ftLk_SpecialLw_Enter")]
    #[doc(alias = "ftLk_SpecialAirLw_Enter")]
    #[doc(alias = "updateBomb")]
    pub fn link_special_lw_enter(&mut self, gobj: GObjId, air: bool) {
        let (throw, msid) = if air {
            (MS_THROW_AIR_F4, ms::SPECIAL_AIR_LW)
        } else {
            (MS_THROW_F4, ms::SPECIAL_LW)
        };
        if let Some(item) = self.game.fighter(gobj).item_gobj {
            let kind = lib26b1::kind(self.game, item);
            if (kind.wrapping_sub(linkbomb::KIND) as u32) <= 1 {
                self.enter_item_throw(gobj, throw);
            }
            return;
        }
        self.game.fighter_mut(gobj).x2210 = [0; 4];
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::SPAWN_BOMB));
    }

    /// The pull, `spawnBomb`: on the script's throw flag, a bomb at his left thumb in his hand,
    /// his sword's model swapped, and his grip on it.
    #[doc(alias = "spawnBomb")]
    pub fn link_spawn_bomb(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if !self.game.fighters[f.0].take_throw_flag(0) {
            return;
        }
        let thumb = self.part_joint(gobj, PART_L_THUMB_NB);
        let pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, thumb, None);
        let kind = self.link_attr_i32(f, attr::X48);
        let hand = i32::from(
            self.data
                .u8(self.ft_data_x8(&self.game.fighters[f.0]).at(0x10)),
        );
        let facing = self.game.fighters[f.0].facing_dir;
        let bomb = linkbomb::spawn(self, gobj, pos, hand, kind, facing);
        self.game.fighters[f.0].item_gobj = bomb;
        self.game.set_part_vis(gobj, 2, 1);
        if bomb.is_some() {
            self.retake_held_item(gobj, true);
        }
    }

    /// `ftLk_SpecialLw_Anim` and `ftLk_SpecialAirLw_Anim` (`doAnim`): waiting or falling at
    /// the end.
    #[doc(alias = "ftLk_SpecialLw_Anim")]
    #[doc(alias = "ftLk_SpecialAirLw_Anim")]
    pub fn link_special_lw_anim(&mut self, gobj: GObjId, air: bool) {
        if !self.anim_frames_remaining(gobj) {
            if air {
                self.enter_fall(gobj);
            } else {
                self.enter_wait(gobj);
            }
        }
    }

    /// `ftLk_SpecialLw_Phys` (friction) and `ftLk_SpecialAirLw_Phys` (gravity, no drift).
    #[doc(alias = "ftLk_SpecialLw_Phys")]
    #[doc(alias = "ftLk_SpecialAirLw_Phys")]
    pub fn link_special_lw_phys(&mut self, gobj: GObjId, air: bool) {
        if air {
            self.air_physics_no_drift(gobj);
        } else {
            self.ground_friction(gobj);
        }
    }

    /// `ftLk_SpecialLw_Coll`: off a ledge, the air version, the bomb's hook kept.
    #[doc(alias = "ftLk_SpecialLw_Coll")]
    pub fn link_special_lw_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_LW, COLL_MF | COLL_MF_HI);
            self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::SPAWN_BOMB));
        }
    }

    /// `ftLk_SpecialAirLw_Coll`: landing, the ground version, the bomb's hook kept.
    #[doc(alias = "ftLk_SpecialAirLw_Coll")]
    pub fn link_special_air_lw_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.fox_air_to_ground(gobj, ms::SPECIAL_LW, COLL_MF | COLL_MF_HI);
            self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::SPAWN_BOMB));
        }
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::SPECIAL_LW_ENTER => g.link_special_lw_enter(gobj, false),
        addr::SPECIAL_AIR_LW_ENTER => g.link_special_lw_enter(gobj, true),
        addr::SPAWN_BOMB => g.link_spawn_bomb(gobj),
        addr::SPECIAL_LW_ANIM => g.link_special_lw_anim(gobj, false),
        addr::SPECIAL_AIR_LW_ANIM => g.link_special_lw_anim(gobj, true),
        addr::SPECIAL_LW_PHYS => g.link_special_lw_phys(gobj, false),
        addr::SPECIAL_AIR_LW_PHYS => g.link_special_lw_phys(gobj, true),
        addr::SPECIAL_LW_COLL => g.link_special_lw_coll(gobj),
        addr::SPECIAL_AIR_LW_COLL => g.link_special_air_lw_coll(gobj),
        _ => return false,
    }
    true
}
