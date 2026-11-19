//! PK Flash (`ftnessspecialn.c`): Ness charges a flash (`it::kinds::nesspkflash`) that floats
//! up while B is held and bursts when he lets go or it reaches its peak.
//!
//! Kirby's copy of it (`ftkirbyspecialness.c`, with its item's checks at the end of
//! `ftkirbyspecialfox.c`) is the same code over his own states, attributes, `u` word, bone,
//! flash kind and hooks, with one branch of its own; a [`PkFlash`] says which, Ness's
//! [`NESS`] or Kirby's (`ft::kirby::special_ness::KIRBY`), and each function here carries
//! both originals' names.

use gekko_fp as fp;

use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::nesspkflash;
use super::super::super::state::Running;
use super::super::motion::flags::*;
use super::{PART_L2ND_NA, addr, attr, ms, mv, u};
use crate::fpu::{mem, reg};
use crate::game::it::ItKind;
use crate::hsd::gobj::GObjId;

/// What tells one PK Flash from the other.
pub struct PkFlash {
    /// Added to Ness's states (`ms::SPECIAL_N_START` on): 0, Kirby's `NS_SPECIAL_N_START`
    /// on; both keep the eight in the same order (Kirby's `Hold0` and `Hold1` are Ness's hold
    /// and release).
    pub(crate) states: i32,
    /// Added to `ftNessAttributes`' PK Flash offsets: 0, Kirby's `specialn_ns_*` 0x1A0 on.
    pub(crate) attrs: usize,
    /// Added to the `u` offset of the flash he charges: 0, Kirby's 0x68 on (`x7C`).
    pub(crate) u: usize,
    /// The bone the flash appears on (`FtPart_L2ndNa`, Kirby's `FtPart_WaistN`, bone 5).
    pub(crate) bone: i32,
    pub(crate) flash_kind: i32,
    /// The hooks while the flash is out: on a KO (`death2_cb`) and when hit (`take_dmg_cb`).
    pub(crate) on_death: u32,
    pub(crate) on_damage: u32,
    /// Ness's: the flash let go of, forgotten or reset clears his damage hooks; Kirby's copy
    /// leaves them in place.
    pub(crate) clears_hooks: bool,
}

/// Ness's own.
pub const NESS: PkFlash = PkFlash {
    states: 0,
    attrs: 0,
    u: 0,
    bone: PART_L2ND_NA,
    flash_kind: ItKind::NessPKFlush.id(),
    on_death: addr::ON_DAMAGE,
    on_damage: addr::ON_DAMAGE,
    clears_hooks: true,
};

/// What PK Flash's states keep between the ground and the air (`FTNESS_SPECIALN_COLL_FLAG`).
const COLL_FLAGS: u32 = SKIP_MAT_ANIM
    | SKIP_COL_ANIM
    | UPDATE_CMD
    | SKIP_ITEM_VIS
    | UNK_19
    | SKIP_MODEL_PART_VIS
    | SKIP_MODEL_FLAGS
    | UNK_27;

/// The aerial states are the grounded ones' four on.
const AIR: i32 = ms::SPECIAL_AIR_N_START - ms::SPECIAL_N_START;

/// `HSD_PAD_B`.
const PAD_B: u32 = 1 << 9;

impl Running<'_> {
    /// The user's damage hooks cleared, if his flash's ending clears them.
    fn ness_special_n_hooks_off(&mut self, gobj: GObjId, p: &PkFlash) {
        if p.clears_hooks {
            let fp = self.game.fighter_mut(gobj);
            fp.death2_cb = None;
            fp.take_dmg_cb = None;
        }
    }

    /// Whether Ness charges PK Flash, `ftNs_SpecialN_CheckSpecialNHold` (Kirby's
    /// `ftKb_SpecialNNs_800FEC78`): a flash out and the hold state.
    #[doc(alias = "ftNs_SpecialN_CheckSpecialNHold")]
    #[doc(alias = "ftKb_SpecialNNs_800FEC78")]
    pub fn ness_special_n_holding(&self, gobj: GObjId, p: &PkFlash) -> bool {
        let fp = self.game.fighter(gobj);
        fp.u.gobj_at(p.u + u::PKFLASH).is_some()
            && (fp.motion_id == p.states + ms::SPECIAL_N_HOLD
                || fp.motion_id == p.states + ms::SPECIAL_AIR_N_HOLD)
    }

    /// The flash let go of, `ftNs_SpecialN_SetNULL` (Kirby's `ftKb_SpecialNNs_800FECBC`, its
    /// end telling him): no flash, and no damage hooks.
    #[doc(alias = "ftNs_SpecialN_SetNULL")]
    #[doc(alias = "ftKb_SpecialNNs_800FECBC")]
    pub fn ness_special_n_set_null(&mut self, gobj: GObjId, p: &PkFlash) {
        let fp = self.game.fighter_mut(gobj);
        if fp.u.gobj_at(p.u + u::PKFLASH).is_some() {
            fp.u.set_gobj_at(p.u + u::PKFLASH, None);
        }
        self.ness_special_n_hooks_off(gobj, p);
    }

    /// Ness hit while charging, `ftNs_SpecialN_ItemPKFlushSetNULL` (Kirby's
    /// `ftKb_SpecialNNs_800FECE8`, hit or KO'd): the flash loses him.
    #[doc(alias = "ftNs_SpecialN_ItemPKFlushSetNULL")]
    #[doc(alias = "ftKb_SpecialNNs_800FECE8")]
    pub fn ness_pkflash_set_null(&mut self, gobj: GObjId, p: &PkFlash) {
        if let Some(flash) = self.game.fighter(gobj).u.gobj_at(p.u + u::PKFLASH) {
            nesspkflash::let_go(self, flash);
            self.game
                .fighter_mut(gobj)
                .u
                .set_gobj_at(p.u + u::PKFLASH, None);
        }
        self.ness_special_n_hooks_off(gobj, p);
    }

    /// The flash from the user's bone, 3 units up (scaled), unless one is out
    /// (`ftNs_PKFlash_Init`).
    fn ness_pkflash_init(&mut self, gobj: GObjId, p: &PkFlash) {
        if self
            .game
            .fighter(gobj)
            .u
            .gobj_at(p.u + u::PKFLASH)
            .is_some()
        {
            return;
        }
        let j = self.bone_joint(gobj, p.bone);
        let mut pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None);
        pos.z = 0.0;
        let fp = self.game.fighter(gobj);
        pos.y = mem(fp::fmadds(3.0, reg(fp.x34_scale.y), reg(pos.y)));
        let facing = fp.facing_dir;
        let flash = nesspkflash::spawn(self, gobj, pos, p.flash_kind, facing);
        let fp = self.game.fighter_mut(gobj);
        fp.u.set_gobj_at(p.u + u::PKFLASH, flash);
        if flash.is_some() {
            fp.death2_cb = Some(CodeRef(p.on_death));
            fp.take_dmg_cb = Some(CodeRef(p.on_damage));
        }
    }

    /// The timers from the attributes, no flash and no hooks (`SetPKFlashAttr`, Kirby's
    /// `ftKb_SetNsFlashAttr`).
    fn ness_special_n_reset(&mut self, gobj: GObjId, p: &PkFlash) {
        let f = self.game.fighter_id(gobj);
        let t1 = self.ness_attr_i32(f, p.attrs + attr::PKFLASH_TIMER1_LOOPFRAMES);
        let t2 = self.ness_attr_i32(f, p.attrs + attr::PKFLASH_TIMER2_LOOPFRAMES);
        let delay = self.ness_attr_i32(f, p.attrs + attr::PKFLASH_GRAVITY_DELAY);
        let min = self.ness_attr_i32(f, p.attrs + attr::PKFLASH_MINCHARGEFRAMES);
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_i32(mv::N_FRAMES_TO_LOOP_CHARGE_GROUND, t1);
        fp.set_mv_i32(mv::N_FRAMES_TO_LOOP_CHARGE_AIR, t2);
        fp.set_mv_i32(mv::N_FALLING_ACCELERATION_DELAY, delay);
        fp.u.set_gobj_at(p.u + u::PKFLASH, None);
        fp.set_mv_i32(mv::N_CHARGE_RELEASE_DELAY, min);
        self.ness_special_n_hooks_off(gobj, p);
    }

    /// `ftNs_SpecialNStart_Enter` (Kirby's `ftKb_SpecialNNs_800FED38`).
    #[doc(alias = "ftNs_SpecialNStart_Enter")]
    #[doc(alias = "ftKb_SpecialNNs_800FED38")]
    pub fn ness_special_n_start_enter(&mut self, gobj: GObjId, p: &PkFlash) {
        let msid = p.states + ms::SPECIAL_N_START;
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.game.fighter_mut(gobj).cmd_vars = [0; 4];
        self.ness_special_n_reset(gobj, p);
        self.anim_update(gobj);
    }

    /// `ftNs_SpecialAirNStart_Enter` (Kirby's `ftKb_SpecialNNs_800FEDD0`): no rise kept.
    #[doc(alias = "ftNs_SpecialAirNStart_Enter")]
    #[doc(alias = "ftKb_SpecialNNs_800FEDD0")]
    pub fn ness_special_air_n_start_enter(&mut self, gobj: GObjId, p: &PkFlash) {
        let msid = p.states + ms::SPECIAL_AIR_N_START;
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars = [0; 4];
        fp.self_vel.y = 0.0;
        self.ness_special_n_reset(gobj, p);
        self.anim_update(gobj);
    }

    /// The start's end (`air` the aerial one's): holding, the flash out, the jumps used.
    #[doc(alias = "ftNs_SpecialNStart_Anim")]
    #[doc(alias = "ftNs_SpecialAirNStart_Anim")]
    #[doc(alias = "ftKb_NsSpecialNStart_Anim")]
    #[doc(alias = "ftKb_NsSpecialAirNStart_Anim")]
    pub(crate) fn ness_special_n_start_anim(&mut self, gobj: GObjId, p: &PkFlash, air: bool) {
        if self.anim_frames_remaining(gobj) {
            return;
        }
        let hold = p.states + ms::SPECIAL_N_HOLD + if air { AIR } else { 0 };
        self.change_motion_state(gobj, hold, 0, 0.0, 1.0, 0.0, None);
        self.ness_pkflash_init(gobj, p);
        let fp = self.game.fighter_mut(gobj);
        fp.x1968_jumpsUsed = fp.co_attrs.max_jumps as u8;
    }

    /// The charge (the hold and release states', `air` the aerial ones'): the timers run;
    /// with the flash gone, the end once both are out, else the release state; with the
    /// flash out but no longer his, he forgets it; with it bursting (`it_802AA7F0`), the
    /// release state.
    #[doc(alias = "ftNs_SpecialNRelease_Anim")]
    #[doc(alias = "ftNs_SpecialAirNRelease_Anim")]
    #[doc(alias = "ftKb_NsSpecialNHold_Anim")]
    #[doc(alias = "ftKb_NsSpecialAirNHold_Anim")]
    pub(crate) fn ness_special_n_charge(&mut self, gobj: GObjId, p: &PkFlash, air: bool) {
        let base = p.states + if air { AIR } else { 0 };
        let (release, end) = (base + ms::SPECIAL_N_RELEASE, base + ms::SPECIAL_N_END);
        let fp = self.game.fighter_mut(gobj);
        let t = fp.mv_i32(mv::N_FRAMES_TO_LOOP_CHARGE_GROUND);
        if t != 0 {
            fp.set_mv_i32(mv::N_FRAMES_TO_LOOP_CHARGE_GROUND, t.wrapping_sub(1));
        }
        let flash = fp.u.gobj_at(p.u + u::PKFLASH);
        let t = fp.mv_i32(mv::N_FRAMES_TO_LOOP_CHARGE_AIR);
        if flash.is_none() && t != 0 {
            fp.set_mv_i32(mv::N_FRAMES_TO_LOOP_CHARGE_AIR, t.wrapping_sub(1));
        }
        let Some(flash) = flash else {
            let fp = self.game.fighter(gobj);
            if fp.mv_i32(mv::N_FRAMES_TO_LOOP_CHARGE_GROUND) <= 0
                && fp.mv_i32(mv::N_FRAMES_TO_LOOP_CHARGE_AIR) <= 0
            {
                self.change_motion_state(gobj, end, 0, 0.0, 1.0, 0.0, None);
                return;
            }
            if fp.motion_id != release {
                let frame = fp.cur_anim_frame;
                self.change_motion_state(gobj, release, 0, frame, 1.0, 0.0, None);
            }
            return;
        };
        if nesspkflash::owner(self, flash) != Some(gobj) {
            self.game
                .fighter_mut(gobj)
                .u
                .set_gobj_at(p.u + u::PKFLASH, None);
            return;
        }
        if nesspkflash::released(self, flash) && self.game.fighter(gobj).motion_id != release {
            let frame = self.game.fighter(gobj).cur_anim_frame;
            self.change_motion_state(gobj, release, 0, frame, 1.0, 0.0, None);
        }
    }

    /// `ftNs_SpecialNEnd_Anim` (Kirby's `ftKb_NsSpecialNEnd_Anim`): the timers reset each
    /// frame, no rotation; at the end, waiting.
    #[doc(alias = "ftNs_SpecialNEnd_Anim")]
    #[doc(alias = "ftKb_NsSpecialNEnd_Anim")]
    pub fn ness_special_n_end_anim(&mut self, gobj: GObjId, p: &PkFlash) {
        self.ness_special_n_reset(gobj, p);
        let f = self.game.fighter_id(gobj);
        self.game.part_set_rot_x(f, 0, 0.0);
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftNs_SpecialAirNEnd_Anim` (Kirby's `ftKb_NsSpecialAirNEnd_Anim`): at the end, falling
    /// helpless, or just falling when the landing lag is 0.
    #[doc(alias = "ftNs_SpecialAirNEnd_Anim")]
    #[doc(alias = "ftKb_NsSpecialAirNEnd_Anim")]
    pub fn ness_special_air_n_end_anim(&mut self, gobj: GObjId, p: &PkFlash) {
        self.ness_special_n_reset(gobj, p);
        let f = self.game.fighter_id(gobj);
        self.game.part_set_rot_x(f, 0, 0.0);
        if self.anim_frames_remaining(gobj) {
            return;
        }
        let lag = self.ness_attr_f32(f, p.attrs + attr::PKFLASH_LANDING_LAG);
        if lag == 0.0 {
            self.enter_fall(gobj);
            return;
        }
        self.enter_fall_special(gobj, 1, 0, 1, 1.0, lag, 0.0, true);
    }

    /// The charge's interrupt (the release states' on Ness, the hold states' on Kirby): after
    /// the least charge, letting go of B lets go of the flash.
    #[doc(alias = "ftNs_SpecialNRelease_IASA")]
    #[doc(alias = "ftNs_SpecialAirNRelease_IASA")]
    #[doc(alias = "ftKb_NsSpecialNHold_IASA")]
    #[doc(alias = "ftKb_NsSpecialAirNHold_IASA")]
    pub(crate) fn ness_special_n_charge_iasa(&mut self, gobj: GObjId, p: &PkFlash) {
        let fp = self.game.fighter_mut(gobj);
        let d = fp.mv_i32(mv::N_CHARGE_RELEASE_DELAY).wrapping_sub(1);
        fp.set_mv_i32(mv::N_CHARGE_RELEASE_DELAY, d);
        if fp.mv_i32(mv::N_CHARGE_RELEASE_DELAY) > 0 {
            return;
        }
        fp.set_mv_i32(mv::N_CHARGE_RELEASE_DELAY, 0);
        if fp.input.held_buttons[0] & PAD_B != 0 {
            return;
        }
        self.ness_special_n_set_null(gobj, p);
    }

    /// The grounded start's physics: the fall's delay counts down, and friction.
    #[doc(alias = "ftNs_SpecialNStart_Phys")]
    #[doc(alias = "ftKb_NsSpecialNStart_Phys")]
    pub fn ness_special_n_start_phys(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let d = fp.mv_i32(mv::N_FALLING_ACCELERATION_DELAY);
        if d != 0 {
            fp.set_mv_i32(mv::N_FALLING_ACCELERATION_DELAY, d.wrapping_sub(1));
        }
        self.ground_friction(gobj);
    }

    /// The air's physics (start, charge and end): after the delay, PK Flash's slow fall, and
    /// air friction.
    #[doc(alias = "ftNs_SpecialAirNStart_Phys")]
    #[doc(alias = "ftNs_SpecialAirNRelease_Phys")]
    #[doc(alias = "ftNs_SpecialAirNEnd_Phys")]
    #[doc(alias = "ftKb_NsSpecialAirNStart_Phys")]
    #[doc(alias = "ftKb_NsSpecialAirNHold_Phys")]
    #[doc(alias = "ftKb_NsSpecialAirNEnd_Phys")]
    pub fn ness_special_air_n_phys(&mut self, gobj: GObjId, p: &PkFlash) {
        let f = self.game.fighter_id(gobj);
        let g = reg(self.ness_attr_f32(f, p.attrs + attr::PKFLASH_FALL_ACCEL));
        let fp = &mut self.game.fighters[f.0];
        let d = fp.mv_i32(mv::N_FALLING_ACCELERATION_DELAY);
        if d != 0 {
            fp.set_mv_i32(mv::N_FALLING_ACCELERATION_DELAY, d.wrapping_sub(1));
        } else {
            let tv = reg(fp.co_attrs.terminal_velocity);
            Self::fall(fp, g, tv);
        }
        let friction = reg(fp.co_attrs.aerial_friction);
        Self::self_accel_deaccel(fp, friction);
    }

    /// The ground's states off a ledge: `ground` (Ness's grounded state) to its aerial one.
    #[doc(alias = "ftNs_SpecialNStart_Coll")]
    #[doc(alias = "ftNs_SpecialNRelease_Coll")]
    #[doc(alias = "ftNs_SpecialNEnd_Coll")]
    #[doc(alias = "ftKb_NsSpecialNStart_Coll")]
    #[doc(alias = "ftKb_NsSpecialNHold_Coll")]
    #[doc(alias = "ftKb_NsSpecialNEnd_Coll")]
    pub(crate) fn ness_special_n_ground_coll(&mut self, gobj: GObjId, p: &PkFlash, ground: i32) {
        if !self.ground_coll_off_ledges(gobj) {
            self.fox_ground_to_air(gobj, p.states + ground + AIR, COLL_FLAGS);
        }
    }

    /// The air's states landing: `ground` (Ness's grounded state).
    #[doc(alias = "ftNs_SpecialAirNStart_Coll")]
    #[doc(alias = "ftNs_SpecialAirNRelease_Coll")]
    #[doc(alias = "ftNs_SpecialAirNEnd_Coll")]
    #[doc(alias = "ftKb_NsSpecialAirNStart_Coll")]
    #[doc(alias = "ftKb_NsSpecialAirNHold_Coll")]
    #[doc(alias = "ftKb_NsSpecialAirNEnd_Coll")]
    pub(crate) fn ness_special_air_n_land(&mut self, gobj: GObjId, p: &PkFlash, ground: i32) {
        if self.air_coll_land(gobj) {
            self.fox_air_to_ground(gobj, p.states + ground, COLL_FLAGS);
        }
    }
}

#[doc(alias = "ftNs_SpecialNRelease_Phys")]
#[doc(alias = "ftNs_SpecialNStart_IASA")]
#[doc(alias = "ftNs_SpecialNEnd_Phys")]
#[doc(alias = "ftNs_SpecialNEnd_IASA")]
#[doc(alias = "ftNs_SpecialAirNStart_IASA")]
#[doc(alias = "ftNs_SpecialAirNEnd_IASA")]
#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    let p = &NESS;
    match a {
        addr::SPECIAL_N_SET_NULL => g.ness_special_n_set_null(gobj, p),
        addr::SPECIAL_N_ITEM_PKFLUSH_SET_NULL => g.ness_pkflash_set_null(gobj, p),
        addr::SPECIAL_N_START_ENTER => g.ness_special_n_start_enter(gobj, p),
        addr::SPECIAL_AIR_N_START_ENTER => g.ness_special_air_n_start_enter(gobj, p),
        addr::SPECIAL_N_START_ANIM => g.ness_special_n_start_anim(gobj, p, false),
        addr::SPECIAL_AIR_N_START_ANIM => g.ness_special_n_start_anim(gobj, p, true),
        addr::SPECIAL_N_RELEASE_ANIM => g.ness_special_n_charge(gobj, p, false),
        addr::SPECIAL_AIR_N_RELEASE_ANIM => g.ness_special_n_charge(gobj, p, true),
        addr::SPECIAL_N_END_ANIM => g.ness_special_n_end_anim(gobj, p),
        addr::SPECIAL_AIR_N_END_ANIM => g.ness_special_air_n_end_anim(gobj, p),
        addr::SPECIAL_N_START_IASA
        | addr::SPECIAL_N_END_IASA
        | addr::SPECIAL_AIR_N_START_IASA
        | addr::SPECIAL_AIR_N_END_IASA => {}
        addr::SPECIAL_N_RELEASE_IASA | addr::SPECIAL_AIR_N_RELEASE_IASA => {
            g.ness_special_n_charge_iasa(gobj, p)
        }
        addr::SPECIAL_N_START_PHYS => g.ness_special_n_start_phys(gobj),
        addr::SPECIAL_N_RELEASE_PHYS | addr::SPECIAL_N_END_PHYS => g.ground_friction(gobj),
        addr::SPECIAL_AIR_N_START_PHYS
        | addr::SPECIAL_AIR_N_RELEASE_PHYS
        | addr::SPECIAL_AIR_N_END_PHYS => g.ness_special_air_n_phys(gobj, p),
        addr::SPECIAL_N_START_COLL => g.ness_special_n_ground_coll(gobj, p, ms::SPECIAL_N_START),
        addr::SPECIAL_N_RELEASE_COLL => g.ness_special_n_ground_coll(gobj, p, ms::SPECIAL_N_HOLD),
        addr::SPECIAL_N_END_COLL => g.ness_special_n_ground_coll(gobj, p, ms::SPECIAL_N_END),
        addr::SPECIAL_AIR_N_START_COLL => g.ness_special_air_n_land(gobj, p, ms::SPECIAL_N_START),
        addr::SPECIAL_AIR_N_RELEASE_COLL => g.ness_special_air_n_land(gobj, p, ms::SPECIAL_N_HOLD),
        addr::SPECIAL_AIR_N_END_COLL => g.ness_special_air_n_land(gobj, p, ms::SPECIAL_N_END),
        _ => return false,
    }
    true
}
