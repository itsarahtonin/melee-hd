//! Chef (`ftgamewatchspecialn.c`): each flip of the pan throws a sausage
//! (`it::kinds::gamewatchchef`), one of five other than the last two, and B pressed again (or
//! held) loops the flip back, up to the attributes' most.
//!
//! Kirby's copy of it (`ftkirbyspecialgamewatch.c`) is the same code over his own states,
//! attributes, `u` words, sausage kind and callbacks, with a pan of his own in hand
//! (`ft::kirby::special_gamewatch`); a [`Chef`] says which, Mr. Game & Watch's [`GAME_WATCH`]
//! or Kirby's (`ft::kirby::special_gamewatch::KIRBY`), and each function here carries both
//! originals' names.

use gekko_fp as fp;

use super::super::super::it::kinds::gamewatchchef as chef;
use super::super::super::state::Running;
use super::super::motion::flags::*;
use super::{addr, attr, ms, mv, part, u};
use crate::fpu::{mem, reg};
use crate::game::it::ItKind;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// What tells one Chef from the other.
pub struct Chef {
    /// The flip's states on the ground and in the air (Kirby's `GW_SPECIAL_N`,
    /// `GW_SPECIAL_AIR_N`).
    pub(crate) states: [i32; 2],
    /// Added to `ftGameWatchAttributes`' Chef offsets: 0, Kirby's `specialn_gw_*` 0x3CC on.
    pub(crate) attrs: usize,
    /// Added to the `u` offsets of the last sausage and the one before: 0, Kirby's 0xC0 on.
    pub(crate) u: usize,
    pub(crate) sausage_kind: i32,
    /// The sausage's callback (`accessory4_cb`), and the one the move starts with: the
    /// sausage's for Mr. Game & Watch; Kirby's takes his pan out first.
    pub(crate) sausage_cb: u32,
    pub(crate) start_cb: u32,
    /// What a change between the ground and the air, or a loop, keeps: Mr. Game & Watch's
    /// keeps his model (`SKIP_MODEL`), Kirby's doesn't.
    pub(crate) keep: u32,
}

/// Mr. Game & Watch's own.
pub const GAME_WATCH: Chef = Chef {
    states: [ms::SPECIAL_N, ms::SPECIAL_AIR_N],
    attrs: 0,
    u: 0,
    sausage_kind: ItKind::GameAndWatchSausage.id(),
    sausage_cb: addr::SPECIAL_N_CREATE_SAUSAGE,
    start_cb: addr::SPECIAL_N_CREATE_SAUSAGE,
    keep: KEEP_FLAGS,
};

/// `HSD_PAD_B`.
const PAD_B: u32 = 1 << 9;

/// What Chef keeps when it changes between the ground and the air, or loops.
pub(crate) const KEEP_FLAGS: u32 = KEEP_COL_ANIM_HIT_STATUS
    | SKIP_HIT
    | SKIP_MODEL
    | SKIP_MAT_ANIM
    | SKIP_COL_ANIM
    | UPDATE_CMD
    | SKIP_ITEM_VIS
    | UNK_19
    | SKIP_MODEL_PART_VIS
    | SKIP_MODEL_FLAGS
    | UNK_27;

impl Running<'_> {
    /// Whether he has thrown fewer sausages than the attributes allow.
    fn gw_chef_more(&self, gobj: GObjId, p: &Chef) -> bool {
        let f = self.game.fighter_id(gobj);
        let n = self.game.fighters[f.0].mv_i32(mv::N_SAUSAGES);
        fp::frsp(f64::from(n)) < reg(self.gw_attr_f32(f, p.attrs + attr::CHEF_MAX))
    }

    /// The frame a flip loops back to (the attributes' `CHEF_LOOPFRAME`).
    fn gw_chef_loop_frame(&self, gobj: GObjId, p: &Chef) -> f32 {
        let f = self.game.fighter_id(gobj);
        self.gw_attr_f32(f, p.attrs + attr::CHEF_LOOPFRAME)
    }

    /// A sausage at the script's mark, `ftGw_SpecialN_CreateSausage` (Kirby's `fn_8010CE5C`):
    /// one of the five but the last two at random, from his thumb.
    #[doc(alias = "ftGw_SpecialN_CreateSausage")]
    #[doc(alias = "fn_8010CE5C")]
    pub fn gw_create_sausage(&mut self, gobj: GObjId, p: &Chef) {
        if self.game.fighter(gobj).cmd_vars[0] == 0 {
            return;
        }
        self.game.fighter_mut(gobj).cmd_vars[0] = 0;
        if self.gw_chef_more(gobj, p) {
            let fp = self.game.fighter_mut(gobj);
            let n = fp.mv_i32(mv::N_SAUSAGES);
            fp.set_mv_i32(mv::N_SAUSAGES, n.wrapping_add(1));
            let bone = self.bone_index(self.game.fighter(gobj), part::L_THUMB_NB);
            let pos = self.gw_part_pos(gobj, bone, Some(Vec3::new(2.5, 6.5, 0.0)));
            let (var1, var2) = (p.u + u::CHEF_VAR1, p.u + u::CHEF_VAR2);
            let fp = self.game.fighter(gobj);
            let (last, before) = (fp.u_i32(var1), fp.u_i32(var2));
            let choices: Vec<i32> = (0..5).filter(|&i| i != last && i != before).collect();
            let pick = self.game.objs.rng.below(choices.len() as i32);
            let sausage = choices[pick as usize];
            let fp = self.game.fighter_mut(gobj);
            fp.set_u_i32(var2, last);
            fp.set_u_i32(var1, sausage);
            let facing = fp.facing_dir;
            chef::spawn(self, gobj, pos, p.sausage_kind, sausage, facing);
        }
        self.gw_set_accessory(gobj, None);
    }

    /// Chef's start, `ftGw_SpecialN_Enter` and `ftGw_SpecialAirN_Enter` (Kirby's
    /// `ftKb_SpecialNGw_8010D188`, `_8010D204`; `air` the aerial one): no vertical speed,
    /// the script's variables cleared (`ftGameWatch_SpecialN_SetVars`).
    #[doc(alias = "ftGw_SpecialN_Enter")]
    #[doc(alias = "ftGw_SpecialAirN_Enter")]
    #[doc(alias = "ftKb_SpecialNGw_8010D188")]
    #[doc(alias = "ftKb_SpecialNGw_8010D204")]
    pub fn gw_special_n_enter(&mut self, gobj: GObjId, p: &Chef, air: bool) {
        self.game.fighter_mut(gobj).self_vel.y = 0.0;
        let msid = p.states[usize::from(air)];
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[0] = 0;
        fp.cmd_vars[1] = 0;
        fp.cmd_vars[2] = 0;
        fp.set_mv_i32(mv::N_LOOP_DISABLE, 0);
        fp.set_mv_i32(mv::N_SAUSAGES, 0);
        self.gw_set_accessory(gobj, Some(p.start_cb));
    }

    /// `ftGw_SpecialN_Anim` and `ftGw_SpecialAirN_Anim` (Kirby's `ftKb_GwSpecialN_Anim`,
    /// `ftKb_GwSpecialAirN_Anim`; `air` the aerial one): at the script's mark
    /// (`ftGameWatch_SpecialN_ChefLoop`, its air copy testing in the other order), the loop
    /// with B still held and sausages left; waiting or a fall once its frames run out.
    #[doc(alias = "ftGw_SpecialN_Anim")]
    #[doc(alias = "ftGw_SpecialAirN_Anim")]
    #[doc(alias = "ftKb_GwSpecialN_Anim")]
    #[doc(alias = "ftKb_GwSpecialAirN_Anim")]
    pub fn gw_special_n_anim(&mut self, gobj: GObjId, p: &Chef, air: bool) {
        let fp = self.game.fighter_mut(gobj);
        if fp.cmd_vars[2] != 0 {
            fp.cmd_vars[2] = 0;
            let held = fp.mv_i32(mv::N_LOOP_DISABLE) == 0;
            if held && self.gw_chef_more(gobj, p) {
                let frame = self.gw_chef_loop_frame(gobj, p);
                self.gw_special_n_loop(gobj, p, air, frame);
            }
        }
        if !self.anim_frames_remaining(gobj) {
            if air {
                self.enter_fall(gobj);
            } else {
                self.enter_wait(gobj);
            }
        }
    }

    /// Chef's interrupts (`ftGw_SpecialN_IASA`, `ftGw_SpecialAirN_IASA`, Kirby's the same):
    /// B let go of ends the loop at the mark; B pressed again after the script allows it
    /// loops now.
    #[doc(alias = "ftGw_SpecialN_IASA")]
    #[doc(alias = "ftGw_SpecialAirN_IASA")]
    #[doc(alias = "ftKb_GwSpecialN_IASA")]
    #[doc(alias = "ftKb_GwSpecialAirN_IASA")]
    pub fn gw_special_n_iasa(&mut self, gobj: GObjId, p: &Chef, air: bool) {
        let fp = self.game.fighter_mut(gobj);
        if fp.input.held_buttons[0] & PAD_B == 0 {
            fp.set_mv_i32(mv::N_LOOP_DISABLE, 1);
        }
        let fp = self.game.fighter(gobj);
        if fp.cmd_vars[1] != 0
            && fp.input.pressed_buttons & PAD_B != 0
            && self.gw_chef_more(gobj, p)
        {
            let frame = self.gw_chef_loop_frame(gobj, p);
            self.gw_special_n_loop(gobj, p, air, frame);
        }
    }

    /// `ftGw_SpecialN_Phys` (Kirby's `ftKb_GwSpecialN_Phys`): friction (`ft_80084F3C`).
    #[doc(alias = "ftGw_SpecialN_Phys")]
    #[doc(alias = "ftKb_GwSpecialN_Phys")]
    pub fn gw_special_n_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// `ftGw_SpecialAirN_Phys` (Kirby's `ftKb_GwSpecialAirN_Phys`): the air's, without drift
    /// (`ft_80084EEC`).
    #[doc(alias = "ftGw_SpecialAirN_Phys")]
    #[doc(alias = "ftKb_GwSpecialAirN_Phys")]
    pub fn gw_special_air_n_phys(&mut self, gobj: GObjId) {
        self.air_physics_no_drift(gobj);
    }

    /// `ftGw_SpecialN_Coll` (Kirby's `ftKb_GwSpecialN_Coll`): off a ledge, into the air.
    #[doc(alias = "ftGw_SpecialN_Coll")]
    #[doc(alias = "ftKb_GwSpecialN_Coll")]
    pub fn gw_special_n_coll(&mut self, gobj: GObjId, p: &Chef) {
        if !self.ground_coll_slide(gobj) {
            self.gw_special_n_to_air(gobj, p);
        }
    }

    /// `ftGw_SpecialAirN_Coll` (Kirby's `ftKb_GwSpecialAirN_Coll`): landing, onto the ground.
    #[doc(alias = "ftGw_SpecialAirN_Coll")]
    #[doc(alias = "ftKb_GwSpecialAirN_Coll")]
    pub fn gw_special_air_n_coll(&mut self, gobj: GObjId, p: &Chef) {
        if self.air_coll_land(gobj) {
            self.gw_special_air_n_to_ground(gobj, p);
        }
    }

    /// `ftGw_SpecialN_GroundToAir` (Kirby's `ftKb_SpecialNGw_8010D580`).
    #[doc(alias = "ftGw_SpecialN_GroundToAir")]
    #[doc(alias = "ftKb_SpecialNGw_8010D580")]
    pub fn gw_special_n_to_air(&mut self, gobj: GObjId, p: &Chef) {
        self.fox_ground_to_air(gobj, p.states[1], p.keep);
        self.gw_set_accessory(gobj, Some(p.sausage_cb));
    }

    /// `ftGw_SpecialAirN_AirToGround` (Kirby's `ftKb_SpecialNGw_8010D5F0`).
    #[doc(alias = "ftGw_SpecialAirN_AirToGround")]
    #[doc(alias = "ftKb_SpecialNGw_8010D5F0")]
    pub fn gw_special_air_n_to_ground(&mut self, gobj: GObjId, p: &Chef) {
        self.fox_air_to_ground(gobj, p.states[0], p.keep);
        self.gw_set_accessory(gobj, Some(p.sausage_cb));
    }

    /// Chef from `frame` again, `ftGw_SpecialN_Loop` and `ftGw_SpecialAirN_Loop` (Kirby's
    /// `ftKb_SpecialNGw_8010D660`, `_8010D6D0`, which read the attributes' frame themselves;
    /// `air` the aerial one).
    #[doc(alias = "ftGw_SpecialN_Loop")]
    #[doc(alias = "ftGw_SpecialAirN_Loop")]
    #[doc(alias = "ftKb_SpecialNGw_8010D660")]
    #[doc(alias = "ftKb_SpecialNGw_8010D6D0")]
    pub fn gw_special_n_loop(&mut self, gobj: GObjId, p: &Chef, air: bool, frame: f32) {
        let start = mem(fp::fsubs(reg(frame), 1.0));
        let msid = p.states[usize::from(air)];
        self.change_motion_state(gobj, msid, p.keep, start, 1.0, 0.0, None);
        self.anim_update(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[1] = 0;
        fp.cmd_vars[2] = 0;
        fp.set_mv_i32(mv::N_LOOP_DISABLE, 0);
        self.gw_set_accessory(gobj, Some(p.sausage_cb));
    }
}

#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    let p = &GAME_WATCH;
    match a {
        addr::SPECIAL_N_CREATE_SAUSAGE => g.gw_create_sausage(gobj, p),
        addr::SPECIAL_N_ENTER => g.gw_special_n_enter(gobj, p, false),
        addr::SPECIAL_AIR_N_ENTER => g.gw_special_n_enter(gobj, p, true),
        addr::SPECIAL_N_ANIM => g.gw_special_n_anim(gobj, p, false),
        addr::SPECIAL_AIR_N_ANIM => g.gw_special_n_anim(gobj, p, true),
        addr::SPECIAL_N_IASA => g.gw_special_n_iasa(gobj, p, false),
        addr::SPECIAL_AIR_N_IASA => g.gw_special_n_iasa(gobj, p, true),
        addr::SPECIAL_N_PHYS => g.gw_special_n_phys(gobj),
        addr::SPECIAL_AIR_N_PHYS => g.gw_special_air_n_phys(gobj),
        addr::SPECIAL_N_COLL => g.gw_special_n_coll(gobj, p),
        addr::SPECIAL_AIR_N_COLL => g.gw_special_air_n_coll(gobj, p),
        addr::SPECIAL_N_GROUND_TO_AIR => g.gw_special_n_to_air(gobj, p),
        addr::SPECIAL_AIR_N_AIR_TO_GROUND => g.gw_special_air_n_to_ground(gobj, p),
        _ => return false,
    }
    true
}
