//! Peach's neutral special, Toad (`ftpeachspecialn.c`): she holds Toad out when her script
//! says; an attack that meets him (a shield bubble, `ftColl_8007B1B8`) turns the move into
//! its counter, where Toad releases a spore. In the air, the first Toad of a jump slows her
//! fall with a small lift.
//!
//! Kirby's copy of it (`ftkirbyspecialpeach.c`) is the same code over his own states,
//! attributes, `u` words, hand, Toad and spore kinds, hooks and callbacks, with two branches
//! of its own; a [`Toad`] says which, Peach's [`PEACH`] or Kirby's
//! (`ft::kirby::special_peach::KIRBY`), and each function here carries both originals' names.

use gekko_fp as fp;

use super::super::super::it::kinds::{peachtoad, peachtoadspore};
use super::super::super::state::Running;
use super::super::co::guard::set_shield_hit;
use super::super::common::GA_GROUND;
use super::super::motion::flags::*;
use super::{addr, attr, ms, mv, u};
use crate::fpu::{mem, reg};
use crate::game::handles::CodeRef;
use crate::game::it::ItKind;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// What tells one Toad from the other.
pub struct Toad {
    /// Added to Peach's states (`ms::SPECIAL_N` on): 0, Kirby's `PE_SPECIAL_LW` on; both
    /// keep the four in the same order.
    pub(crate) states: i32,
    /// Added to `ftPeachAttributes`' Toad offsets before the bubble's (`x94` to `xA8`): 0,
    /// Kirby's 0xE4 on; and to the bubble's (`xAC` to `xBC`): 0, Kirby's 0x340 on
    /// (`specialn_pe_absorbdesc`).
    pub(crate) attrs: usize,
    pub(crate) bubble_attrs: usize,
    /// Added to the `u` offsets of the aerial Toad's use and Toad: 0, Kirby's 0xB8 on
    /// (`xCC`, `xD0`).
    pub(crate) u: usize,
    /// The bone Toad is held on (`FtPart_109`, her hand; Kirby's `FtPart_R3rdNa`, 44).
    pub(crate) bone: i32,
    pub(crate) toad_kind: i32,
    pub(crate) spore_kind: i32,
    /// The hooks while Toad is out: on a KO (`death2_cb`) and when hit (`take_dmg_cb`).
    pub(crate) on_death: u32,
    pub(crate) on_damage: u32,
    /// The user's own callbacks the move sets: Toad made, the spore, the hitlag's two, and
    /// the bubble met.
    pub(crate) on_accessory4: u32,
    pub(crate) on_hit_accessory4: u32,
    pub(crate) on_enter_hitlag: u32,
    pub(crate) on_exit_hitlag: u32,
    pub(crate) on_hit: u32,
    /// Peach's: Toad made is her special item (`x1984_heldItemSpec`); Kirby's copy leaves
    /// that alone.
    pub(crate) held_item_spec: bool,
    /// Peach's: forgetting Toad clears her damage hooks; Kirby's copy leaves them in place.
    pub(crate) clears_hooks: bool,
}

/// Peach's own.
pub const PEACH: Toad = Toad {
    states: 0,
    attrs: 0,
    bubble_attrs: 0,
    u: 0,
    bone: PART_TOAD,
    toad_kind: ItKind::PeachToad.id(),
    spore_kind: ItKind::PeachToadSpore.id(),
    on_death: addr::ON_DEATH2,
    on_damage: addr::ON_DEATH2,
    on_accessory4: addr::N_ON_ACCESSORY4,
    on_hit_accessory4: addr::N_ON_HIT_ACCESSORY4,
    on_enter_hitlag: addr::N_ON_ENTER_HITLAG,
    on_exit_hitlag: addr::N_ON_EXIT_HITLAG,
    on_hit: addr::N_ON_UNK_HIT,
    held_item_spec: true,
    clears_hooks: true,
};

/// `doColl`, `doAirColl`, `doHitColl`, `doAirHitColl` and `setupHitColl`.
const DO_COLL: u32 = 0x8011_E7D4;
const DO_AIR_COLL: u32 = 0x8011_E8C0;
const DO_HIT_COLL: u32 = 0x8011_EB58;
const DO_AIR_HIT_COLL: u32 = 0x8011_EBC0;
const SETUP_HIT_COLL: u32 = 0x8011_EC30;

/// The bone Toad is held on (`FtPart_109`, her hand).
const PART_TOAD: i32 = 109;

/// The script's variables (`cmd_vars`): the air lift's, the bubble's, Toad's and the
/// counter's spore's.
const CMD_PHYS: usize = 0;
const CMD_ANIM: usize = 1;
const CMD_ACCESSORY4: usize = 2;
const CMD_HIT_ANIM: usize = 3;

/// The flags the move keeps changing between the ground and the air (`coll_mf`).
const COLL_MF: u32 = KEEP_COL_ANIM_HIT_STATUS
    | SKIP_HIT
    | SKIP_MAT_ANIM
    | SKIP_COL_ANIM
    | UPDATE_CMD
    | SKIP_ITEM_VIS
    | UNK_19
    | SKIP_MODEL_PART_VIS
    | SKIP_MODEL_FLAGS
    | UNK_27;
const _: () = assert!(COLL_MF == 0x0C4C_508C);

#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    use super::addr::*;
    let p = &PEACH;
    match a {
        N_ON_ACCESSORY4 => g.peach_toad_on_accessory4(gobj, p),
        N_ON_HIT_ACCESSORY4 => g.peach_toad_on_hit_accessory4(gobj, p),
        SPECIAL_N_ON_DEATH2 => g.peach_toad_on_death2(gobj, p),
        SPECIAL_N_DO_DEATH2 => g.peach_toad_gone(gobj, p),
        N_ON_ENTER_HITLAG => g.peach_toad_enter_hitlag(gobj, p),
        N_ON_EXIT_HITLAG => g.peach_toad_exit_hitlag(gobj, p),
        SPECIAL_N_ENTER => g.peach_special_n_enter(gobj, p),
        SPECIAL_AIR_N_ENTER => g.peach_special_air_n_enter(gobj, p),
        SPECIAL_N_ANIM => g.peach_special_n_anim(gobj, p),
        SPECIAL_AIR_N_ANIM => g.peach_special_air_n_anim(gobj, p),
        SPECIAL_N_IASA | SPECIAL_AIR_N_IASA | SPECIAL_N_HIT_IASA | SPECIAL_AIR_N_HIT_IASA => {
            g.peach_special_n_iasa(gobj)
        }
        SPECIAL_N_PHYS => g.peach_special_n_phys(gobj),
        SPECIAL_AIR_N_PHYS => g.peach_special_air_n_phys(gobj, p),
        SPECIAL_N_COLL => g.peach_special_n_coll(gobj, p),
        SPECIAL_AIR_N_COLL => g.peach_special_air_n_coll(gobj, p),
        DO_COLL => g.peach_special_n_to_air(gobj, p),
        DO_AIR_COLL => g.peach_special_n_to_ground(gobj, p),
        SPECIAL_N_HIT_ANIM => g.peach_special_n_hit_anim(gobj, p),
        SPECIAL_AIR_N_HIT_ANIM => g.peach_special_air_n_hit_anim(gobj, p),
        SPECIAL_N_HIT_PHYS => g.peach_special_n_hit_phys(gobj),
        SPECIAL_AIR_N_HIT_PHYS => g.peach_special_air_n_hit_phys(gobj, p),
        SPECIAL_N_HIT_COLL => g.peach_special_n_hit_coll(gobj, p),
        SPECIAL_AIR_N_HIT_COLL => g.peach_special_air_n_hit_coll(gobj, p),
        DO_HIT_COLL => g.peach_special_n_hit_to_air(gobj, p),
        DO_AIR_HIT_COLL => g.peach_special_n_hit_to_ground(gobj, p),
        SETUP_HIT_COLL => g.peach_special_n_hit_hooks(gobj, p),
        N_ON_UNK_HIT => g.peach_toad_hit(gobj, p),
        _ => return false,
    }
    true
}

impl Running<'_> {
    /// A float of the user's Toad attributes before the bubble's.
    fn peach_n_attr_f32(&self, gobj: GObjId, p: &Toad, off: usize) -> f32 {
        let f = self.game.fighter_id(gobj);
        self.peach_attr_f32(f, p.attrs + off)
    }

    /// The user's damage hooks while Toad is out.
    fn peach_toad_damage_hooks(&mut self, gobj: GObjId, p: &Toad) {
        let fp = self.game.fighter_mut(gobj);
        fp.death2_cb = Some(CodeRef(p.on_death));
        fp.take_dmg_cb = Some(CodeRef(p.on_damage));
    }

    /// Toad made in her hand (`onAccessory4`, Kirby's `fn_8010C288`; once): she holds him,
    /// as her special item, with her hooks for her hit and hitlag.
    #[doc(alias = "onAccessory4")]
    #[doc(alias = "fn_8010C288")]
    pub fn peach_toad_on_accessory4(&mut self, gobj: GObjId, p: &Toad) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[CMD_ACCESSORY4] != 0 {
            return;
        }
        self.game.fighters[f.0].cmd_vars[CMD_ACCESSORY4] = 1;
        let hand = self.bone_joint(gobj, p.bone);
        let pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, hand, None);
        let facing = self.game.fighters[f.0].facing_dir;
        let toad = peachtoad::spawn(self, gobj, pos, p.bone, p.toad_kind, facing);
        let fp = &mut self.game.fighters[f.0];
        fp.u.set_gobj_at(p.u + u::TOAD, toad);
        if p.held_item_spec {
            fp.special_item = toad;
        }
        if toad.is_some() {
            self.peach_toad_damage_hooks(gobj, p);
        }
        let fp = &mut self.game.fighters[f.0];
        fp.accessory4_cb = None;
        fp.pre_hitlag_cb = Some(CodeRef(p.on_enter_hitlag));
        fp.post_hitlag_cb = Some(CodeRef(p.on_exit_hitlag));
    }

    /// The counter's spore (`onHitAccessory4`, with `doHitAccessory4`; Kirby's
    /// `fn_8010C344`): released from her hand, a little above it and on the stage's plane.
    #[doc(alias = "onHitAccessory4")]
    #[doc(alias = "fn_8010C344")]
    pub fn peach_toad_on_hit_accessory4(&mut self, gobj: GObjId, p: &Toad) {
        let hand = self.bone_joint(gobj, p.bone);
        let mut pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, hand, None);
        pos.y = mem(fp::fadds(reg(pos.y), 2.5));
        pos.z = 0.0;
        let facing = self.game.fighter(gobj).facing_dir;
        peachtoadspore::spawn(self, gobj, pos, p.spore_kind, facing);
        self.game.fighter_mut(gobj).accessory4_cb = None;
    }

    /// Toad taken away when she's hit or KO'd, `ftPe_SpecialN_OnDeath2` (Kirby's
    /// `ftKb_SpecialNPe_8010C3F4`): if out, he goes and she forgets him.
    #[doc(alias = "ftPe_SpecialN_OnDeath2")]
    #[doc(alias = "ftKb_SpecialNPe_8010C3F4")]
    pub fn peach_toad_on_death2(&mut self, gobj: GObjId, p: &Toad) {
        if let Some(toad) = self.game.fighter(gobj).u.gobj_at(p.u + u::TOAD) {
            peachtoad::remove(self, toad);
            self.peach_toad_gone(gobj, p);
        }
    }

    /// She forgets Toad and its hooks, `ftPe_SpecialN_DoDeath2` (Kirby's
    /// `ftKb_SpecialNPe_8010C3C0`, his hooks kept), Toad going on after her hitlag first
    /// (`onExitHitlag`).
    #[doc(alias = "ftPe_SpecialN_DoDeath2")]
    #[doc(alias = "ftKb_SpecialNPe_8010C3C0")]
    pub fn peach_toad_gone(&mut self, gobj: GObjId, p: &Toad) {
        self.peach_toad_exit_hitlag(gobj, p);
        let fp = self.game.fighter_mut(gobj);
        fp.u.set_gobj_at(p.u + u::TOAD, None);
        if p.clears_hooks {
            fp.death2_cb = None;
            fp.take_dmg_cb = None;
        }
    }

    /// Toad pauses in her hitlag (`onEnterHitlag`, Kirby's `fn_8010C44C`).
    #[doc(alias = "onEnterHitlag")]
    #[doc(alias = "fn_8010C44C")]
    pub fn peach_toad_enter_hitlag(&mut self, gobj: GObjId, p: &Toad) {
        if let Some(toad) = self.game.fighter(gobj).u.gobj_at(p.u + u::TOAD) {
            peachtoad::hitlag_start(self, toad);
        }
    }

    /// Toad goes on after her hitlag (`onExitHitlag`, Kirby's `ftKb_SpecialNPe_8010C47C`).
    #[doc(alias = "onExitHitlag")]
    #[doc(alias = "ftKb_SpecialNPe_8010C47C")]
    pub fn peach_toad_exit_hitlag(&mut self, gobj: GObjId, p: &Toad) {
        if let Some(toad) = self.game.fighter(gobj).u.gobj_at(p.u + u::TOAD) {
            peachtoad::hitlag_end(self, toad);
        }
    }

    /// Whether she has left the move, `ftPe_SpecialN_IsActive` (Kirby's
    /// `ftKb_SpecialNPe_8010C4AC`; true outside its four states, despite its name).
    #[doc(alias = "ftPe_SpecialN_IsActive")]
    #[doc(alias = "ftKb_SpecialNPe_8010C4AC")]
    pub fn peach_toad_done(&self, gobj: GObjId, p: &Toad) -> bool {
        let msid = self.game.fighter(gobj).motion_id;
        !(p.states + ms::SPECIAL_N..=p.states + ms::SPECIAL_AIR_N_HIT).contains(&msid)
    }

    /// The move's start (`reset`): the script's variables cleared, her facing kept, and Toad
    /// made when the script says.
    fn peach_special_n_reset(&mut self, gobj: GObjId, p: &Toad) {
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[CMD_HIT_ANIM] = 0;
        fp.cmd_vars[CMD_ACCESSORY4] = 0;
        fp.cmd_vars[CMD_ANIM] = 0;
        fp.cmd_vars[CMD_PHYS] = 0;
        let facing = fp::fctiwz(reg(fp.facing_dir));
        fp.set_mv_i32(mv::N_FACING_DIR, facing);
        fp.accessory4_cb = Some(CodeRef(p.on_accessory4));
    }

    /// `ftPe_SpecialN_Enter` (Kirby's `ftKb_SpecialNPe_8010C4D4`): on the ground, no
    /// vertical speed.
    #[doc(alias = "ftPe_SpecialN_Enter")]
    #[doc(alias = "ftKb_SpecialNPe_8010C4D4")]
    pub fn peach_special_n_enter(&mut self, gobj: GObjId, p: &Toad) {
        self.game.fighter_mut(gobj).self_vel.y = 0.0;
        let msid = p.states + ms::SPECIAL_N;
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.peach_special_n_reset(gobj, p);
    }

    /// `ftPe_SpecialAirN_Enter` (Kirby's `ftKb_SpecialNPe_8010C560`): in the air, her
    /// horizontal speed divided by the attributes' x94.
    #[doc(alias = "ftPe_SpecialAirN_Enter")]
    #[doc(alias = "ftKb_SpecialNPe_8010C560")]
    pub fn peach_special_air_n_enter(&mut self, gobj: GObjId, p: &Toad) {
        let div = reg(self.peach_n_attr_f32(gobj, p, attr::N_AIR_VEL_X_DIV));
        let fp = self.game.fighter_mut(gobj);
        fp.self_vel.x = mem(fp::fdivs(reg(fp.self_vel.x), div));
        let msid = p.states + ms::SPECIAL_AIR_N;
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.peach_special_n_reset(gobj, p);
    }

    /// Toad's bubble, `ftColl_8007B1B8` with the attributes' absorb shape: an attack meeting
    /// it calls the user's `on_hit`.
    fn peach_toad_bubble(&mut self, gobj: GObjId, p: &Toad) {
        let f = self.game.fighter_id(gobj);
        let b = p.bubble_attrs;
        let bone = self.peach_attr_i32(f, b + attr::N_BUBBLE_BONE);
        let offset = Vec3::new(
            self.peach_attr_f32(f, b + attr::N_BUBBLE_OFFSET),
            self.peach_attr_f32(f, b + attr::N_BUBBLE_OFFSET + 4),
            self.peach_attr_f32(f, b + attr::N_BUBBLE_OFFSET + 8),
        );
        let size = self.peach_attr_f32(f, b + attr::N_BUBBLE_SIZE);
        let joint = self.bone_joint(gobj, bone);
        let fp = &mut self.game.fighters[f.0];
        set_shield_hit(fp, CodeRef(p.on_hit), joint, size, offset);
    }

    /// The stance (`doAnim`): the bubble once the script opens it (`cmd_vars[1]` 1), with its
    /// push; none before; then `done` when the animation ends.
    fn peach_special_n_stance(&mut self, gobj: GObjId, p: &Toad, air: bool) {
        let f = self.game.fighter_id(gobj);
        match self.game.fighters[f.0].cmd_vars[CMD_ANIM] {
            1 => {
                self.game.fighters[f.0].cmd_vars[CMD_ANIM] = 2;
                self.peach_toad_bubble(gobj, p);
                let push = self.peach_n_attr_f32(gobj, p, attr::N_XA8);
                let fp = &mut self.game.fighters[f.0];
                fp.state_flags.bubble_faces = true;
                fp.counter_min_hitlag = push;
                fp.counter_item_min_hitlag = push;
            }
            0 => self.game.fighters[f.0].state_flags.shield_bubble = false,
            _ => {}
        }
        if !self.anim_frames_remaining(gobj) {
            if air {
                self.enter_fall(gobj);
            } else {
                self.enter_wait(gobj);
            }
        }
    }

    /// `ftPe_SpecialN_Anim` (Kirby's `ftKb_PeSpecialLw_Anim`).
    #[doc(alias = "ftPe_SpecialN_Anim")]
    #[doc(alias = "ftKb_PeSpecialLw_Anim")]
    pub fn peach_special_n_anim(&mut self, gobj: GObjId, p: &Toad) {
        self.peach_special_n_stance(gobj, p, false);
    }

    /// `ftPe_SpecialAirN_Anim` (Kirby's `ftKb_PeSpecialAirLw_Anim`).
    #[doc(alias = "ftPe_SpecialAirN_Anim")]
    #[doc(alias = "ftKb_PeSpecialAirLw_Anim")]
    pub fn peach_special_air_n_anim(&mut self, gobj: GObjId, p: &Toad) {
        self.peach_special_n_stance(gobj, p, true);
    }

    /// The move's interrupts: none.
    #[doc(alias = "ftPe_SpecialN_IASA")]
    #[doc(alias = "ftPe_SpecialAirN_IASA")]
    #[doc(alias = "ftPe_SpecialNHit_IASA")]
    #[doc(alias = "ftPe_SpecialAirNHit_IASA")]
    #[doc(alias = "ftKb_PeSpecialLw_IASA")]
    #[doc(alias = "ftKb_PeSpecialAirLw_IASA")]
    #[doc(alias = "ftKb_PeSpecialLwHit_IASA")]
    #[doc(alias = "ftKb_PeSpecialAirLwHit_IASA")]
    pub fn peach_special_n_iasa(&mut self, _gobj: GObjId) {}

    /// `ftPe_SpecialN_Phys` (Kirby's `ftKb_PeSpecialLw_Phys`): friction, and the bubble
    /// follows.
    #[doc(alias = "ftPe_SpecialN_Phys")]
    #[doc(alias = "ftKb_PeSpecialLw_Phys")]
    pub fn peach_special_n_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
        self.shield_follow(gobj);
    }

    /// `ftPe_SpecialAirN_Phys` (Kirby's `ftKb_PeSpecialAirLw_Phys`): once the script says
    /// (`cmd_vars[0]`), the first Toad of a jump lifts her (the attributes' x9C) and later
    /// ones stop her fall, then the move's gravity; before, plain gravity. Air friction, and
    /// the bubble follows.
    #[doc(alias = "ftPe_SpecialAirN_Phys")]
    #[doc(alias = "ftKb_PeSpecialAirLw_Phys")]
    pub fn peach_special_air_n_phys(&mut self, gobj: GObjId, p: &Toad) {
        let lift = self.peach_n_attr_f32(gobj, p, attr::N_AIR_VEL_Y);
        let (g, t) = (
            reg(self.peach_n_attr_f32(gobj, p, attr::N_XA0)),
            reg(self.peach_n_attr_f32(gobj, p, attr::N_XA4)),
        );
        let friction = reg(self.peach_n_attr_f32(gobj, p, attr::N_X98));
        let used = p.u + u::SPECIAL_AIR_N_USED;
        let fp = self.game.fighter_mut(gobj);
        if fp.cmd_vars[CMD_PHYS] >= 1 {
            if fp.cmd_vars[CMD_PHYS] == 1 {
                fp.cmd_vars[CMD_PHYS] = 2;
                if fp.u_i32(used) == 0 {
                    fp.set_u_i32(used, 1);
                    fp.self_vel.y = lift;
                } else {
                    fp.self_vel.y = 0.0;
                }
            }
            Self::fall(fp, g, t);
        } else {
            Self::fall_basic(fp);
        }
        Self::self_accel_deaccel(fp, friction);
        self.shield_follow(gobj);
    }

    /// `ftPe_SpecialN_Coll` (Kirby's `ftKb_PeSpecialLw_Coll`): into the air off an edge.
    #[doc(alias = "ftPe_SpecialN_Coll")]
    #[doc(alias = "ftKb_PeSpecialLw_Coll")]
    pub fn peach_special_n_coll(&mut self, gobj: GObjId, p: &Toad) {
        if !self.ground_coll_slide(gobj) {
            self.peach_special_n_to_air(gobj, p);
        }
    }

    /// `ftPe_SpecialAirN_Coll` (Kirby's `ftKb_PeSpecialAirLw_Coll`): onto the ground on
    /// landing.
    #[doc(alias = "ftPe_SpecialAirN_Coll")]
    #[doc(alias = "ftKb_PeSpecialAirLw_Coll")]
    pub fn peach_special_air_n_coll(&mut self, gobj: GObjId, p: &Toad) {
        if self.air_coll_land(gobj) {
            self.peach_special_n_to_ground(gobj, p);
        }
    }

    /// The move's hooks again after a change between the ground and the air (`setupColl`),
    /// with the bubble if it was open.
    fn peach_special_n_hooks(&mut self, gobj: GObjId, p: &Toad) {
        if self.game.fighter(gobj).u.gobj_at(p.u + u::TOAD).is_some() {
            self.peach_toad_damage_hooks(gobj, p);
        }
        let fp = self.game.fighter_mut(gobj);
        fp.accessory4_cb = Some(CodeRef(p.on_accessory4));
        fp.pre_hitlag_cb = Some(CodeRef(p.on_enter_hitlag));
        fp.post_hitlag_cb = Some(CodeRef(p.on_exit_hitlag));
        if fp.cmd_vars[CMD_ANIM] == 2 {
            self.peach_toad_bubble(gobj, p);
            self.game.fighter_mut(gobj).state_flags.bubble_faces = true;
        }
    }

    /// Off an edge (`doColl`, Kirby's `ftKb_SpecialNPe_8010C8D8`): the air state from the
    /// same frame, the lift spent if the script had just asked for it.
    #[doc(alias = "doColl")]
    #[doc(alias = "ftKb_SpecialNPe_8010C8D8")]
    pub fn peach_special_n_to_air(&mut self, gobj: GObjId, p: &Toad) {
        self.fox_ground_to_air(gobj, p.states + ms::SPECIAL_AIR_N, COLL_MF);
        let fp = self.game.fighter_mut(gobj);
        if fp.cmd_vars[CMD_PHYS] == 1 {
            fp.cmd_vars[CMD_PHYS] = 2;
        }
        self.peach_special_n_hooks(gobj, p);
    }

    /// Landing (`doAirColl`, Kirby's `ftKb_SpecialNPe_8010C9CC`): her air Toad's lift back,
    /// the ground state from the same frame.
    #[doc(alias = "doAirColl")]
    #[doc(alias = "ftKb_SpecialNPe_8010C9CC")]
    pub fn peach_special_n_to_ground(&mut self, gobj: GObjId, p: &Toad) {
        self.game
            .fighter_mut(gobj)
            .set_u_i32(p.u + u::SPECIAL_AIR_N_USED, 0);
        self.fox_air_to_ground(gobj, p.states + ms::SPECIAL_N, COLL_MF);
        self.peach_special_n_hooks(gobj, p);
    }

    /// The counter (`doHitAnim`): the spore when the script says (`cmd_vars[3]`), then `done`
    /// when the animation ends.
    fn peach_special_n_counter(&mut self, gobj: GObjId, p: &Toad, air: bool) {
        let fp = self.game.fighter_mut(gobj);
        if fp.cmd_vars[CMD_HIT_ANIM] != 0 {
            fp.cmd_vars[CMD_HIT_ANIM] = 0;
            fp.accessory4_cb = Some(CodeRef(p.on_hit_accessory4));
        }
        if !self.anim_frames_remaining(gobj) {
            if air {
                self.enter_fall(gobj);
            } else {
                self.enter_wait(gobj);
            }
        }
    }

    /// `ftPe_SpecialNHit_Anim` (Kirby's `ftKb_PeSpecialLwHit_Anim`).
    #[doc(alias = "ftPe_SpecialNHit_Anim")]
    #[doc(alias = "ftKb_PeSpecialLwHit_Anim")]
    pub fn peach_special_n_hit_anim(&mut self, gobj: GObjId, p: &Toad) {
        self.peach_special_n_counter(gobj, p, false);
    }

    /// `ftPe_SpecialAirNHit_Anim` (Kirby's `ftKb_PeSpecialAirLwHit_Anim`).
    #[doc(alias = "ftPe_SpecialAirNHit_Anim")]
    #[doc(alias = "ftKb_PeSpecialAirLwHit_Anim")]
    pub fn peach_special_air_n_hit_anim(&mut self, gobj: GObjId, p: &Toad) {
        self.peach_special_n_counter(gobj, p, true);
    }

    /// `ftPe_SpecialNHit_Phys` (Kirby's `ftKb_PeSpecialLwHit_Phys`): friction.
    #[doc(alias = "ftPe_SpecialNHit_Phys")]
    #[doc(alias = "ftKb_PeSpecialLwHit_Phys")]
    pub fn peach_special_n_hit_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// `ftPe_SpecialAirNHit_Phys` (Kirby's `ftKb_PeSpecialAirLwHit_Phys`): the move's
    /// gravity and air friction.
    #[doc(alias = "ftPe_SpecialAirNHit_Phys")]
    #[doc(alias = "ftKb_PeSpecialAirLwHit_Phys")]
    pub fn peach_special_air_n_hit_phys(&mut self, gobj: GObjId, p: &Toad) {
        let (g, t) = (
            reg(self.peach_n_attr_f32(gobj, p, attr::N_XA0)),
            reg(self.peach_n_attr_f32(gobj, p, attr::N_XA4)),
        );
        let friction = reg(self.peach_n_attr_f32(gobj, p, attr::N_X98));
        let fp = self.game.fighter_mut(gobj);
        Self::fall(fp, g, t);
        Self::self_accel_deaccel(fp, friction);
    }

    /// `ftPe_SpecialNHit_Coll` (Kirby's `ftKb_PeSpecialLwHit_Coll`): into the air off an
    /// edge.
    #[doc(alias = "ftPe_SpecialNHit_Coll")]
    #[doc(alias = "ftKb_PeSpecialLwHit_Coll")]
    pub fn peach_special_n_hit_coll(&mut self, gobj: GObjId, p: &Toad) {
        if !self.ground_coll_slide(gobj) {
            self.peach_special_n_hit_to_air(gobj, p);
        }
    }

    /// `ftPe_SpecialAirNHit_Coll` (Kirby's `ftKb_PeSpecialAirLwHit_Coll`): onto the ground
    /// on landing.
    #[doc(alias = "ftPe_SpecialAirNHit_Coll")]
    #[doc(alias = "ftKb_PeSpecialAirLwHit_Coll")]
    pub fn peach_special_air_n_hit_coll(&mut self, gobj: GObjId, p: &Toad) {
        if self.air_coll_land(gobj) {
            self.peach_special_n_hit_to_ground(gobj, p);
        }
    }

    /// Off an edge in the counter (`doHitColl`, Kirby's `ftKb_SpecialNGw_8010CC6C`).
    #[doc(alias = "doHitColl")]
    #[doc(alias = "ftKb_SpecialNGw_8010CC6C")]
    pub fn peach_special_n_hit_to_air(&mut self, gobj: GObjId, p: &Toad) {
        self.fox_ground_to_air(gobj, p.states + ms::SPECIAL_AIR_N_HIT, COLL_MF);
        self.peach_special_n_hit_hooks(gobj, p);
    }

    /// Landing in the counter (`doAirHitColl`, Kirby's `ftKb_SpecialNGw_8010CCD4`): her air
    /// Toad's lift back.
    #[doc(alias = "doAirHitColl")]
    #[doc(alias = "ftKb_SpecialNGw_8010CCD4")]
    pub fn peach_special_n_hit_to_ground(&mut self, gobj: GObjId, p: &Toad) {
        self.game
            .fighter_mut(gobj)
            .set_u_i32(p.u + u::SPECIAL_AIR_N_USED, 0);
        self.fox_air_to_ground(gobj, p.states + ms::SPECIAL_N_HIT, COLL_MF);
        self.peach_special_n_hit_hooks(gobj, p);
    }

    /// The counter's hooks (`setupHitColl`, Kirby's `ftKb_SpecialNGw_8010CD44`): for her hit
    /// while Toad is out, and her hitlag.
    #[doc(alias = "setupHitColl")]
    #[doc(alias = "ftKb_SpecialNGw_8010CD44")]
    pub fn peach_special_n_hit_hooks(&mut self, gobj: GObjId, p: &Toad) {
        if self.game.fighter(gobj).u.gobj_at(p.u + u::TOAD).is_some() {
            self.peach_toad_damage_hooks(gobj, p);
        }
        let fp = self.game.fighter_mut(gobj);
        fp.pre_hitlag_cb = Some(CodeRef(p.on_enter_hitlag));
        fp.post_hitlag_cb = Some(CodeRef(p.on_exit_hitlag));
    }

    /// An attack met Toad's bubble (`onUnkHit`, Kirby's `fn_8010CD88`): the counter from
    /// frame 9, facing where the hit came from, and Toad's own reaction.
    #[doc(alias = "onUnkHit")]
    #[doc(alias = "fn_8010CD88")]
    pub fn peach_toad_hit(&mut self, gobj: GObjId, p: &Toad) {
        let fp = self.game.fighter_mut(gobj);
        let facing = fp::fctiwz(reg(fp.specialn_facing_dir));
        fp.set_mv_i32(mv::N_FACING_DIR, facing);
        let msid = p.states
            + if fp.ground_or_air == GA_GROUND {
                ms::SPECIAL_N_HIT
            } else {
                ms::SPECIAL_AIR_N_HIT
            };
        self.change_motion_state(gobj, msid, 0, 9.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        if let Some(toad) = self.game.fighter(gobj).u.gobj_at(p.u + u::TOAD) {
            peachtoad::hit(self, toad);
        }
        self.peach_special_n_hit_hooks(gobj, p);
    }
}
