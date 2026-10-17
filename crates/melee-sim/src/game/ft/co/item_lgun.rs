//! The Ray Gun's shooting states (`ft_0CDF.c`): one shot on the ground or in the air, an
//! "empty" set once it has none left, the gun's recoil, and the ray (or the empty click) on
//! the script's throw flag.

use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::lgun;
use super::super::super::it::lib26b1;
use super::super::super::state::Running;
use super::ids::ms;
use crate::game::ef::Arg;
use crate::hsd::gobj::GObjId;
use gekko_fp as fp;

/// The shot's accessory callback (`ftCo_800CE1D4`).
pub const SHOT: u32 = 0x800C_E1D4;

/// The ground-to-air and air-to-ground switches keep the animation and most of the state.
const SWITCH: u32 = 0x0C4C_5080;

/// The ray's muzzle flash and sounds, and the empty gun's.
const GFX_RAY: i32 = 0x431;
const GFX_EMPTY: i32 = 0x405;
const SFX_RAY_RIGHT: i32 = 228;
const SFX_RAY_LEFT: i32 = 229;
const SFX_EMPTY: i32 = 230;

impl Running<'_> {
    fn lgun_empty(&self, gobj: GObjId) -> bool {
        let it = self.game.fighter(gobj).item_gobj.expect("no held item");
        lib26b1::is_empty_shooter(self.game, it)
    }

    /// A shot on the ground, `ftCo_800CDF5C`.
    #[doc(alias = "ftCo_800CDF5C")]
    pub fn enter_lgun_shoot(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).x2210 = [0; 4];
        let msid = if self.lgun_empty(gobj) {
            ms::L_GUN_SHOOT_EMPTY
        } else {
            ms::L_GUN_SHOOT
        };
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.lgun_start(gobj);
    }

    /// A shot in the air, `ftCo_800CE010`.
    #[doc(alias = "ftCo_800CE010")]
    pub fn enter_lgun_shoot_air(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).x2210 = [0; 4];
        let msid = if self.lgun_empty(gobj) {
            ms::L_GUN_SHOOT_AIR_EMPTY
        } else {
            ms::L_GUN_SHOOT_AIR
        };
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        let f = self.game.fighter_id(gobj);
        self.clamp_air_drift(f);
        self.lgun_start(gobj);
    }

    /// The shot's callback, and the gun's recoil.
    fn lgun_start(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(SHOT));
        if let Some(it) = self.game.fighter(gobj).item_gobj {
            lgun::shot(self, it);
        }
    }

    /// The state at the current frame and speed, as the ground and air switches change it.
    fn lgun_switch(&mut self, gobj: GObjId, msid: i32) {
        let fp = self.game.fighter(gobj);
        let (frame, speed) = (fp.cur_anim_frame, fp.frame_speed_mul);
        self.change_motion_state(gobj, msid, SWITCH, frame, speed, 0.0, None);
    }

    /// Lands while shooting, `ftCo_800CE0CC`. (The air shot lands as the ground's; the empty
    /// air shot as the empty ground one.)
    #[doc(alias = "ftCo_800CE0CC")]
    pub fn lgun_land(&mut self, gobj: GObjId) {
        self.land(gobj);
        let msid = if self.game.fighter(gobj).motion_id == ms::L_GUN_SHOOT_AIR {
            ms::L_GUN_SHOOT
        } else {
            ms::L_GUN_SHOOT_EMPTY
        };
        self.lgun_switch(gobj, msid);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(SHOT));
    }

    /// Leaves the ground while shooting, `ftCo_800CE14C`.
    #[doc(alias = "ftCo_800CE14C")]
    pub fn lgun_air(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.go_airborne(f);
        let msid = if self.game.fighter(gobj).motion_id == ms::L_GUN_SHOOT {
            ms::L_GUN_SHOOT_AIR
        } else {
            ms::L_GUN_SHOOT_AIR_EMPTY
        };
        self.lgun_switch(gobj, msid);
        self.clamp_air_drift(f);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(SHOT));
    }

    /// The shot, on the script's throw flag, `ftCo_800CE1D4`: a ray from the muzzle with its
    /// flash and sound (by the way the fighter faces), or the empty gun's click.
    #[doc(alias = "ftCo_800CE1D4")]
    pub fn lgun_shot(&mut self, gobj: GObjId) {
        let Some(it) = self.game.fighter(gobj).item_gobj else {
            return;
        };
        if !self.game.fighter_mut(gobj).take_throw_flag(0) {
            return;
        }
        let muzzle = lgun::muzzle(self, it);
        let mut pos = self.item_hand_point(gobj, muzzle);
        let f = self.game.fighter_id(gobj);
        let facing = self.game.fighter(gobj).facing_dir;
        if !lib26b1::is_empty_shooter(self.game, it) {
            lgun::fire(self, it, pos, facing);
            pos.z = 0.0;
            crate::game::ef::spawn(
                self,
                GFX_RAY,
                Some(gobj),
                &[Arg::Vec(pos), Arg::Float(facing)],
            );
            let sfx = if crate::fpu::reg(facing) == fp::frsp(1.0) {
                SFX_RAY_RIGHT
            } else {
                SFX_RAY_LEFT
            };
            self.fighter_play_sfx(f, sfx, 127, 64);
        } else {
            crate::game::ef::spawn(self, GFX_EMPTY, Some(gobj), &[Arg::Vec(pos)]);
            self.fighter_play_sfx(f, SFX_EMPTY, 127, 64);
        }
    }

    /// The shooting states' animation, `ftCo_800CDE18`: at its end, falling or standing
    /// (`ftCo_LGunShoot_Anim`, `ftCo_LGunShootAir_Anim` and the Fire Flower's end).
    #[doc(alias = "ftCo_800CDE18")]
    #[doc(alias = "ftCo_LGunShoot_Anim")]
    #[doc(alias = "ftCo_LGunShootAir_Anim")]
    pub fn co_item_shoot_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.fall_or_wait(gobj);
        }
    }

    /// `ftCo_LGunShoot_IASA` and `ftCo_LGunShootAir_IASA`: none.
    #[doc(alias = "ftCo_LGunShoot_IASA")]
    #[doc(alias = "ftCo_LGunShootAir_IASA")]
    pub fn co_lgun_iasa(&mut self, _gobj: GObjId) {}

    /// `ftCo_LGunShoot_Phys`: the shooting states' ground physics.
    #[doc(alias = "ftCo_LGunShoot_Phys")]
    pub fn co_lgun_phys(&mut self, gobj: GObjId) {
        self.co_item_shoot_phys(gobj);
    }

    /// `ftCo_LGunShootAir_Phys`: the shooting states' air physics.
    #[doc(alias = "ftCo_LGunShootAir_Phys")]
    pub fn co_lgun_air_phys(&mut self, gobj: GObjId) {
        self.co_item_shoot_air_phys(gobj);
    }

    /// `ftCo_LGunShoot_Coll`: off the ground, the air's shot.
    #[doc(alias = "ftCo_LGunShoot_Coll")]
    pub fn co_lgun_coll(&mut self, gobj: GObjId) {
        self.ground_coll_then(gobj, |r, g| r.lgun_air(g));
    }

    /// `ftCo_LGunShootAir_Coll`: landing, the ground's shot.
    #[doc(alias = "ftCo_LGunShootAir_Coll")]
    pub fn co_lgun_air_coll(&mut self, gobj: GObjId) {
        self.air_coll_then(gobj, |r, g| r.lgun_land(g));
    }
}
