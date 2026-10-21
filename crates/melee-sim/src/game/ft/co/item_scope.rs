//! The Super Scope's states (`ftCo_ItemScopeStart.c`, `ftCo_ItemScopeRapid.c`,
//! `ftCo_ItemScopeFire.c`, `ftCo_ItemScope.c`) and the shooting items' entry
//! (`ftCo_Attack_800CDD14`, `ftCo_800CDDA0`, `ft_0CDD.c`): A starts charging; let go early it
//! fires a rapid burst, held long enough (or let go late) one charged shot; each on the
//! ground and in the air, and an "empty" set of the same states once it has no ammunition.
//! The Ray Gun's states are in `item_lgun`, the Fire Flower's in `item_fflower`.

use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::{F_FLOWER, L_GUN, S_SCOPE, sscope};
use super::super::super::it::lib26b1;
use super::super::super::state::Running;
use super::PAD_A;
use super::ids::ms;
use crate::fpu::{mem, reg};
use crate::game::ef::Arg;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;
use gekko_fp as fp;

/// The charge's ticks (`mv` x0, a float) and whether A was let go (`mv` x4), while starting;
/// the shot's level (x0) once firing, or the burst's shots left (x0) while rapid-firing.
const MV_TIMER: usize = 0x0;
const MV_RELEASED: usize = 0x4;

/// `ftCommonData`: the charge that fires on its own (x5B8, a float) and a burst's shots
/// (x5BC).
const CD_CHARGED: u32 = 0x5B8;
const CD_RAPID: u32 = 0x5BC;

/// The callbacks the states install: the damage hooks (`fn_800D7938`, `fn_800D8378`) and the
/// shots (`fn_800D86B8`, `fn_800D80F4`).
pub const TAKE_DMG_START: u32 = 0x800D_7938;
pub const TAKE_DMG: u32 = 0x800D_8378;
pub const FIRE_SHOT: u32 = 0x800D_86B8;
pub const RAPID_SHOT: u32 = 0x800D_80F4;

/// The ground-to-air and air-to-ground switches keep the animation and most of the state.
const SWITCH: u32 = 0x0C4C_5080;
const SWITCH_START: u32 = 0x0C4C_5280;

impl Running<'_> {
    /// The held item's kind.
    fn held_kind(&self, gobj: GObjId) -> i32 {
        let it = self.game.fighter(gobj).item_gobj.expect("no held item");
        lib26b1::kind(self.game, it)
    }

    /// A shot with the held shooting item on the ground, `ftCo_Attack_800CDD14`, unless an
    /// item in reach is picked up instead.
    #[doc(alias = "ftCo_Attack_800CDD14")]
    pub fn enter_item_shoot(&mut self, gobj: GObjId) {
        if self.pick_up_item_check(gobj) {
            return;
        }
        match self.held_kind(gobj) {
            L_GUN => self.enter_lgun_shoot(gobj),
            F_FLOWER => self.enter_fflower_shoot(gobj),
            S_SCOPE => self.enter_scope_start(gobj),
            _ => {}
        }
    }

    /// A shot with the held shooting item in the air, `ftCo_800CDDA0`.
    #[doc(alias = "ftCo_800CDDA0")]
    pub fn enter_item_shoot_air(&mut self, gobj: GObjId) {
        match self.held_kind(gobj) {
            L_GUN => self.enter_lgun_shoot_air(gobj),
            F_FLOWER => self.enter_fflower_shoot_air(gobj),
            S_SCOPE => self.enter_scope_air_start(gobj),
            _ => {}
        }
    }

    /// The shooting states' ground physics, `ftCo_800CDE54` (the scope's ground states' own,
    /// `ftCo_ItemScopeStart_Phys` and the others, call it).
    #[doc(alias = "ftCo_800CDE54")]
    #[doc(alias = "ftCo_ItemScopeStart_Phys")]
    #[doc(alias = "ftCo_ItemScopeRapid_Phys")]
    #[doc(alias = "ftCo_ItemScopeFire_Phys")]
    #[doc(alias = "ftCo_ItemScopeEnd_Phys")]
    pub fn co_item_shoot_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// The shooting states' air physics, `ftCo_800CDE74` (the scope's air states' own call
    /// it).
    #[doc(alias = "ftCo_800CDE74")]
    #[doc(alias = "ftCo_ItemScopeAirStart_Phys")]
    #[doc(alias = "ftCo_ItemScopeAirRapid_Phys")]
    #[doc(alias = "ftCo_ItemScopeAirFire_Phys")]
    #[doc(alias = "ftCo_ItemScopeAirEnd_Phys")]
    pub fn co_item_shoot_air_phys(&mut self, gobj: GObjId) {
        self.air_physics(gobj);
    }

    /// A point in the item hand's space (scaled back by the fighter's size) in the world,
    /// `ftCo_800CDE94`.
    #[doc(alias = "ftCo_800CDE94")]
    pub fn item_hand_point(&mut self, gobj: GObjId, p: Vec3) -> Vec3 {
        let fp = self.game.fighter(gobj);
        let bone = usize::from(self.data.u8(self.ft_data_x8(fp).at(0x10)));
        let parts = fp.parts.expect("a fighter without parts").0;
        let scale = reg(fp.x34_scale.y);
        let j = self.game.fighter_parts[parts][bone]
            .joint
            .expect("an item hand without a joint");
        let mut pos = p;
        if scale != 1.0 {
            let inv = fp::fdivs(1.0, scale);
            pos = Vec3::new(
                mem(fp::fmuls(reg(pos.x), inv)),
                mem(fp::fmuls(reg(pos.y), inv)),
                mem(fp::fmuls(reg(pos.z), inv)),
            );
        }
        jobj::setup_matrix(&mut self.game.objs, self.data, Some(j));
        self.game.objs.jobjs[j].mtx.transform(pos)
    }

    /// The empty set's state for `msid` once the scope ran dry (`fn_800D769C`).
    #[doc(alias = "fn_800D769C")]
    fn scope_motion(&self, gobj: GObjId, msid: i32) -> i32 {
        if self.game.fighter(gobj).motion_id >= ms::ITEM_SCOPE_START_EMPTY {
            msid + 8
        } else {
            msid
        }
    }

    /// Whether the held scope is out of ammunition, `it_8026B594`.
    fn scope_empty(&self, gobj: GObjId) -> bool {
        let it = self.game.fighter(gobj).item_gobj.expect("no held item");
        lib26b1::is_empty_shooter(self.game, it)
    }

    fn set_callbacks(&mut self, gobj: GObjId, shot: Option<u32>, dmg: u32) {
        let fp = self.game.fighter_mut(gobj);
        if let Some(s) = shot {
            fp.accessory4_cb = Some(CodeRef(s));
        }
        fp.take_dmg_cb = Some(CodeRef(dmg));
    }

    /// Starts charging on the ground, `ft_800D76B8`.
    #[doc(alias = "ft_800D76B8")]
    pub fn enter_scope_start(&mut self, gobj: GObjId) {
        let msid = if self.scope_empty(gobj) {
            ms::ITEM_SCOPE_START_EMPTY
        } else {
            ms::ITEM_SCOPE_START
        };
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_f32(MV_TIMER, 0.0);
        fp.set_mv_i32(MV_RELEASED, 0);
        self.item_drop_hook(gobj, true);
        self.set_callbacks(gobj, None, TAKE_DMG_START);
    }

    /// Starts charging in the air, `ft_800D7770`.
    #[doc(alias = "ft_800D7770")]
    pub fn enter_scope_air_start(&mut self, gobj: GObjId) {
        let msid = if self.scope_empty(gobj) {
            ms::ITEM_SCOPE_AIR_START_EMPTY
        } else {
            ms::ITEM_SCOPE_AIR_START
        };
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        let f = self.game.fighter_id(gobj);
        self.clamp_air_drift(f);
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_f32(MV_TIMER, 0.0);
        fp.set_mv_i32(MV_RELEASED, 0);
        self.item_drop_hook(gobj, true);
        self.set_callbacks(gobj, None, TAKE_DMG_START);
    }

    /// Lands while charging, `fn_800D7830`.
    #[doc(alias = "fn_800D7830")]
    pub fn scope_start_land(&mut self, gobj: GObjId) {
        self.land(gobj);
        let msid = if self.game.fighter(gobj).motion_id >= ms::ITEM_SCOPE_START_EMPTY {
            ms::ITEM_SCOPE_START_EMPTY
        } else {
            ms::ITEM_SCOPE_START
        };
        self.switch_motion(gobj, msid, SWITCH_START);
        self.set_callbacks(gobj, None, TAKE_DMG_START);
    }

    /// Leaves the ground while charging, `fn_800D78B0`.
    #[doc(alias = "fn_800D78B0")]
    pub fn scope_start_air(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.go_airborne(f);
        let msid = if self.game.fighter(gobj).motion_id >= ms::ITEM_SCOPE_START_EMPTY {
            ms::ITEM_SCOPE_AIR_START_EMPTY
        } else {
            ms::ITEM_SCOPE_AIR_START
        };
        self.switch_motion(gobj, msid, SWITCH_START);
        self.clamp_air_drift(f);
        self.set_callbacks(gobj, None, TAKE_DMG_START);
    }

    /// The state at the current frame and speed, as the ground and air switches change it.
    fn switch_motion(&mut self, gobj: GObjId, msid: i32, flags: u32) {
        let fp = self.game.fighter(gobj);
        let (frame, speed) = (fp.cur_anim_frame, fp.frame_speed_mul);
        self.change_motion_state(gobj, msid, flags, frame, speed, 0.0, None);
    }

    /// Hit while charging, `fn_800D7938`: the charge so far is spent, the item back in hand.
    /// With one shot left and a charge past level 0, the level is the scope's GObj address,
    /// whose spending leaves it more than two billion shots (`sscope::spend`).
    #[doc(alias = "fn_800D7938")]
    pub fn scope_start_hit(&mut self, gobj: GObjId) {
        let Some(it) = self.game.fighter(gobj).item_gobj else {
            return;
        };
        let charge = fp::fctiwz(reg(self.game.fighter(gobj).mv_f32(MV_TIMER)));
        let level = sscope::affordable(self, it, charge);
        if level != -1 {
            sscope::spend(self, it, level);
        }
        self.item_pickup_hook(gobj, true);
    }

    /// `ftCo_ItemScopeStart_Anim` and `ftCo_ItemScopeAirStart_Anim`: nothing.
    #[doc(alias = "ftCo_ItemScopeStart_Anim")]
    #[doc(alias = "ftCo_ItemScopeAirStart_Anim")]
    pub fn co_scope_start_anim(&mut self, _gobj: GObjId) {}

    /// The charge, `fn_800D79B4`: A held counts it up until let go; at the charge that fires
    /// on its own, once the animation ends, the shot of the charge's level (`air`); let go,
    /// once the animation ends, a burst (`ground`) for a charge below the first level, else
    /// the shot.
    #[doc(alias = "fn_800D79B4")]
    fn scope_charge(
        &mut self,
        gobj: GObjId,
        burst: fn(&mut Running, GObjId),
        shot: fn(&mut Running, GObjId, i32),
    ) {
        let Some(it) = self.game.fighter(gobj).item_gobj else {
            return;
        };
        let fp = self.game.fighter_mut(gobj);
        if fp.mv_i32(MV_RELEASED) == 0 && fp.input.held_buttons[0] & PAD_A != 0 {
            let t = fp::fadds(reg(fp.mv_f32(MV_TIMER)), 1.0);
            fp.set_mv_f32(MV_TIMER, t);
        }
        if fp.input.held_buttons[0] & PAD_A == 0 {
            fp.set_mv_i32(MV_RELEASED, 1);
        }
        let charge = fp::fctiwz(reg(fp.mv_f32(MV_TIMER)));
        let level = sscope::affordable(self, it, charge);
        let ready = reg(self.common().f32(CD_CHARGED));
        if reg(self.game.fighter(gobj).mv_f32(MV_TIMER)) >= ready
            && !self.anim_frames_remaining(gobj)
        {
            shot(self, gobj, level);
        }
        if self.game.fighter(gobj).mv_i32(MV_RELEASED) == 0 || self.anim_frames_remaining(gobj) {
            return;
        }
        let charge = fp::fctiwz(reg(self.game.fighter(gobj).mv_f32(MV_TIMER)));
        if sscope::charge_level(self, it, charge) == 0 {
            burst(self, gobj);
        } else {
            shot(self, gobj, level);
        }
    }

    /// `ftCo_ItemScopeStart_IASA`: the charge, into the ground's burst or shot.
    #[doc(alias = "ftCo_ItemScopeStart_IASA")]
    pub fn co_scope_start_iasa(&mut self, gobj: GObjId) {
        self.scope_charge(
            gobj,
            |r, g| r.enter_scope_rapid(g),
            |r, g, l| r.enter_scope_fire(g, l),
        );
    }

    /// `ftCo_ItemScopeAirStart_IASA`: the charge, into the air's burst or shot.
    #[doc(alias = "ftCo_ItemScopeAirStart_IASA")]
    pub fn co_scope_air_start_iasa(&mut self, gobj: GObjId) {
        self.scope_charge(
            gobj,
            |r, g| r.enter_scope_air_rapid(g),
            |r, g, l| r.enter_scope_air_fire(g, l),
        );
    }

    /// `ftCo_ItemScopeStart_Coll`: off the ground, the air's charge.
    #[doc(alias = "ftCo_ItemScopeStart_Coll")]
    pub fn co_scope_start_coll(&mut self, gobj: GObjId) {
        self.ground_coll_then(gobj, |r, g| r.scope_start_air(g));
    }

    /// `ftCo_ItemScopeAirStart_Coll`: landing, the ground's charge.
    #[doc(alias = "ftCo_ItemScopeAirStart_Coll")]
    pub fn co_scope_air_start_coll(&mut self, gobj: GObjId) {
        self.air_coll_then(gobj, |r, g| r.scope_start_land(g));
    }

    /// A burst on the ground, `fn_800D7BDC`.
    #[doc(alias = "fn_800D7BDC")]
    pub fn enter_scope_rapid(&mut self, gobj: GObjId) {
        let msid = self.scope_motion(gobj, ms::ITEM_SCOPE_RAPID);
        self.change_motion_state(gobj, msid, 0x0200_0000, 0.0, 1.0, 0.0, None);
        let shots = self.common().i32(CD_RAPID);
        self.game.fighter_mut(gobj).set_mv_i32(MV_TIMER, shots);
        self.set_callbacks(gobj, Some(RAPID_SHOT), TAKE_DMG);
    }

    /// A burst in the air, `fn_800D7C60`.
    #[doc(alias = "fn_800D7C60")]
    pub fn enter_scope_air_rapid(&mut self, gobj: GObjId) {
        let msid = self.scope_motion(gobj, ms::ITEM_SCOPE_AIR_RAPID);
        self.change_motion_state(gobj, msid, 0x0200_0000, 0.0, 1.0, 0.0, None);
        let f = self.game.fighter_id(gobj);
        self.clamp_air_drift(f);
        let shots = self.common().i32(CD_RAPID);
        self.game.fighter_mut(gobj).set_mv_i32(MV_TIMER, shots);
        self.set_callbacks(gobj, Some(RAPID_SHOT), TAKE_DMG);
    }

    /// Lands during a burst, `fn_800D7CEC`.
    #[doc(alias = "fn_800D7CEC")]
    pub fn scope_rapid_land(&mut self, gobj: GObjId) {
        self.land(gobj);
        let msid = self.scope_motion(gobj, ms::ITEM_SCOPE_RAPID);
        self.switch_motion(gobj, msid, SWITCH);
        self.set_callbacks(gobj, Some(RAPID_SHOT), TAKE_DMG);
    }

    /// Leaves the ground during a burst, `fn_800D7D70`.
    #[doc(alias = "fn_800D7D70")]
    pub fn scope_rapid_air(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.go_airborne(f);
        let msid = self.scope_motion(gobj, ms::ITEM_SCOPE_AIR_RAPID);
        self.switch_motion(gobj, msid, SWITCH);
        self.clamp_air_drift(f);
        self.set_callbacks(gobj, Some(RAPID_SHOT), TAKE_DMG);
    }

    /// A burst's animation, `ftCo_ItemScopeRapid_Anim` and `ftCo_ItemScopeAirRapid_Anim`:
    /// each loop is a new attack and counts a shot down, ending the burst at none; a scope run
    /// dry switches to the empty burst.
    #[doc(alias = "ftCo_ItemScopeRapid_Anim")]
    #[doc(alias = "ftCo_ItemScopeAirRapid_Anim")]
    pub fn co_scope_rapid_anim(&mut self, gobj: GObjId, air: bool) {
        let fp = self.game.fighter(gobj);
        let (frame, speed) = (reg(fp.cur_anim_frame), reg(fp.frame_speed_mul));
        if !(frame >= 0.0 && frame < speed) {
            return;
        }
        self.renew_attack_instance(gobj);
        self.renew_attack_record(gobj);
        let fp = self.game.fighter_mut(gobj);
        let left = fp.mv_i32(MV_TIMER).wrapping_sub(1);
        fp.set_mv_i32(MV_TIMER, left);
        if left == 0 {
            if air {
                self.enter_scope_air_end(gobj);
            } else {
                self.enter_scope_end(gobj);
            }
        } else if self.game.fighter(gobj).item_gobj.is_some() && self.scope_empty(gobj) {
            let msid = if self.game.fighter(gobj).ground_or_air == 0 {
                ms::ITEM_SCOPE_RAPID_EMPTY
            } else {
                ms::ITEM_SCOPE_AIR_RAPID_EMPTY
            };
            self.switch_motion(gobj, msid, SWITCH);
            self.set_callbacks(gobj, Some(RAPID_SHOT), TAKE_DMG);
        }
    }

    /// `ftCo_ItemScopeRapid_IASA` and `ftCo_ItemScopeAirRapid_IASA`: A pressed refills the
    /// burst.
    #[doc(alias = "ftCo_ItemScopeRapid_IASA")]
    #[doc(alias = "ftCo_ItemScopeAirRapid_IASA")]
    pub fn co_scope_rapid_iasa(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).input.pressed_buttons & PAD_A != 0 {
            let shots = self.common().i32(CD_RAPID);
            self.game.fighter_mut(gobj).set_mv_i32(MV_TIMER, shots);
        }
    }

    /// `ftCo_ItemScopeRapid_Coll`: off the ground, the air's burst.
    #[doc(alias = "ftCo_ItemScopeRapid_Coll")]
    pub fn co_scope_rapid_coll(&mut self, gobj: GObjId) {
        self.ground_coll_then(gobj, |r, g| r.scope_rapid_air(g));
    }

    /// `ftCo_ItemScopeAirRapid_Coll`: landing, the ground's burst.
    #[doc(alias = "ftCo_ItemScopeAirRapid_Coll")]
    pub fn co_scope_air_rapid_coll(&mut self, gobj: GObjId) {
        self.air_coll_then(gobj, |r, g| r.scope_rapid_land(g));
    }

    /// A burst's shot, `fn_800D80F4`: the smallest level the scope can afford.
    #[doc(alias = "fn_800D80F4")]
    pub fn scope_rapid_shot(&mut self, gobj: GObjId) {
        if let Some(it) = self.game.fighter(gobj).item_gobj {
            let level = sscope::affordable(self, it, 0);
            self.scope_shot(gobj, level);
        }
    }

    /// The charged shot on the ground, `fn_800D8140`.
    #[doc(alias = "fn_800D8140")]
    pub fn enter_scope_fire(&mut self, gobj: GObjId, level: i32) {
        self.game.fighter_mut(gobj).x2210 = [0; 4];
        let msid = self.scope_motion(gobj, ms::ITEM_SCOPE_FIRE);
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.game.fighter_mut(gobj).set_mv_i32(MV_TIMER, level);
        self.set_callbacks(gobj, Some(FIRE_SHOT), TAKE_DMG);
    }

    /// The charged shot in the air, `fn_800D81D0`.
    #[doc(alias = "fn_800D81D0")]
    pub fn enter_scope_air_fire(&mut self, gobj: GObjId, level: i32) {
        self.game.fighter_mut(gobj).x2210 = [0; 4];
        let msid = self.scope_motion(gobj, ms::ITEM_SCOPE_AIR_FIRE);
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        let f = self.game.fighter_id(gobj);
        self.clamp_air_drift(f);
        self.game.fighter_mut(gobj).set_mv_i32(MV_TIMER, level);
        self.set_callbacks(gobj, Some(FIRE_SHOT), TAKE_DMG);
    }

    /// Lands during the charged shot, `fn_800D8268`.
    #[doc(alias = "fn_800D8268")]
    pub fn scope_fire_land(&mut self, gobj: GObjId) {
        self.land(gobj);
        let msid = self.scope_motion(gobj, ms::ITEM_SCOPE_FIRE);
        self.switch_motion(gobj, msid, SWITCH);
        self.set_callbacks(gobj, Some(FIRE_SHOT), TAKE_DMG);
    }

    /// Leaves the ground during the charged shot, `fn_800D82EC`.
    #[doc(alias = "fn_800D82EC")]
    pub fn scope_fire_air(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.go_airborne(f);
        let msid = self.scope_motion(gobj, ms::ITEM_SCOPE_AIR_FIRE);
        self.switch_motion(gobj, msid, SWITCH);
        self.clamp_air_drift(f);
        self.set_callbacks(gobj, Some(FIRE_SHOT), TAKE_DMG);
    }

    /// Hit after the charge, `fn_800D8378`: the item back in hand.
    #[doc(alias = "fn_800D8378")]
    pub fn scope_hit(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).item_gobj.is_some() {
            self.item_pickup_hook(gobj, true);
        }
    }

    /// The shot's and the end's animation, `ftCo_ItemScopeFire_Anim` (and the air's, and the
    /// ends', `ftCo_ItemScopeEnd_Anim`): at its end, falling or waiting, the item back in hand.
    #[doc(alias = "ftCo_ItemScopeFire_Anim")]
    #[doc(alias = "ftCo_ItemScopeAirFire_Anim")]
    #[doc(alias = "ftCo_ItemScopeEnd_Anim")]
    #[doc(alias = "ftCo_ItemScopeAirEnd_Anim")]
    pub fn co_scope_fire_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.fall_or_wait(gobj);
            self.item_pickup_hook(gobj, true);
        }
    }

    /// The shot's and the end's interrupts, `ftCo_ItemScopeFire_IASA` (and the others'):
    /// none.
    #[doc(alias = "ftCo_ItemScopeFire_IASA")]
    #[doc(alias = "ftCo_ItemScopeAirFire_IASA")]
    #[doc(alias = "ftCo_ItemScopeEnd_IASA")]
    #[doc(alias = "ftCo_ItemScopeAirEnd_IASA")]
    pub fn co_scope_no_iasa(&mut self, _gobj: GObjId) {}

    /// `ftCo_ItemScopeFire_Coll`: off the ground, the air's shot.
    #[doc(alias = "ftCo_ItemScopeFire_Coll")]
    pub fn co_scope_fire_coll(&mut self, gobj: GObjId) {
        self.ground_coll_then(gobj, |r, g| r.scope_fire_air(g));
    }

    /// `ftCo_ItemScopeAirFire_Coll`: landing, the ground's shot.
    #[doc(alias = "ftCo_ItemScopeAirFire_Coll")]
    pub fn co_scope_air_fire_coll(&mut self, gobj: GObjId) {
        self.air_coll_then(gobj, |r, g| r.scope_fire_land(g));
    }

    /// A shot of `level` from the muzzle when the script's throw flag 0 says, `fn_800D84D4`:
    /// levels 0 to 9 fire a beam with their flash and sound; anything else (none left, or the
    /// scope's GObj address that `affordable` returns) only clicks.
    #[doc(alias = "fn_800D84D4")]
    pub fn scope_shot(&mut self, gobj: GObjId, level: i32) {
        let Some(it) = self.game.fighter(gobj).item_gobj else {
            return;
        };
        if !self.game.fighter_mut(gobj).take_throw_flag(0) {
            return;
        }
        let muzzle = sscope::muzzle(self, it);
        let mut pos = self.item_hand_point(gobj, muzzle);
        let f = self.game.fighter_id(gobj);
        let facing = self.game.fighter(gobj).facing_dir;
        if (0..=9).contains(&level) {
            sscope::shoot(self, it, pos, level, facing);
            pos.z = 0.0;
            crate::game::ef::spawn(
                self,
                0x430,
                Some(gobj),
                &[Arg::Vec(pos), Arg::Float(facing)],
            );
            let sfx = match level {
                0 => 0xFC,
                1..=4 => 0xFF,
                _ => 0x100,
            };
            self.fighter_play_sfx(f, sfx, 0x7F, 0x40);
        } else {
            crate::game::ef::spawn(self, 0x405, Some(gobj), &[Arg::Vec(pos)]);
            self.fighter_play_sfx(f, 0x101, 0x7F, 0x40);
        }
    }

    /// The charged shot's accessory, `fn_800D86B8`: the shot of the state's level.
    #[doc(alias = "fn_800D86B8")]
    pub fn scope_fire_shot(&mut self, gobj: GObjId) {
        let level = self.game.fighter(gobj).mv_i32(MV_TIMER);
        self.scope_shot(gobj, level);
    }

    /// The burst's end on the ground, `fn_800D86E0`.
    #[doc(alias = "fn_800D86E0")]
    pub fn enter_scope_end(&mut self, gobj: GObjId) {
        let msid = self.scope_motion(gobj, ms::ITEM_SCOPE_END);
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.set_callbacks(gobj, None, TAKE_DMG);
    }

    /// The burst's end in the air, `fn_800D874C`.
    #[doc(alias = "fn_800D874C")]
    pub fn enter_scope_air_end(&mut self, gobj: GObjId) {
        let msid = self.scope_motion(gobj, ms::ITEM_SCOPE_AIR_END);
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        let f = self.game.fighter_id(gobj);
        self.clamp_air_drift(f);
        self.set_callbacks(gobj, None, TAKE_DMG);
    }

    /// Lands during the burst's end, `fn_800D87C0`.
    #[doc(alias = "fn_800D87C0")]
    pub fn scope_end_land(&mut self, gobj: GObjId) {
        self.land(gobj);
        let msid = self.scope_motion(gobj, ms::ITEM_SCOPE_END);
        self.switch_motion(gobj, msid, SWITCH);
        self.set_callbacks(gobj, None, TAKE_DMG);
    }

    /// Leaves the ground during the burst's end, `fn_800D8838`.
    #[doc(alias = "fn_800D8838")]
    pub fn scope_end_air(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.go_airborne(f);
        let msid = self.scope_motion(gobj, ms::ITEM_SCOPE_AIR_END);
        self.switch_motion(gobj, msid, SWITCH);
        self.clamp_air_drift(f);
        self.set_callbacks(gobj, None, TAKE_DMG);
    }

    /// `ftCo_ItemScopeEnd_Coll`: off the ground, the air's end.
    #[doc(alias = "ftCo_ItemScopeEnd_Coll")]
    pub fn co_scope_end_coll(&mut self, gobj: GObjId) {
        self.ground_coll_then(gobj, |r, g| r.scope_end_air(g));
    }

    /// `ftCo_ItemScopeAirEnd_Coll`: landing, the ground's end.
    #[doc(alias = "ftCo_ItemScopeAirEnd_Coll")]
    pub fn co_scope_air_end_coll(&mut self, gobj: GObjId) {
        self.air_coll_then(gobj, |r, g| r.scope_end_land(g));
    }
}
