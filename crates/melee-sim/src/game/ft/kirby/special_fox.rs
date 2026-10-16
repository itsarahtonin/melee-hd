//! Fox's and Falco's copies (`ftkirbyspecialfox.c`): the blaster as Kirby draws and fires it,
//! kept in `u` (`xB0`). Fox's own (`ft::fox::special_n`) with his states (Fox's or Falco's set
//! by the copy), his attributes for each, and the copy's own blaster kinds; the blaster item's
//! queries of its owner for him are here too.

use gekko_fp as fp;

use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::{foxblaster, foxlaser};
use super::super::super::state::Running;
use super::super::co::ids::ms as co_ms;
use super::super::motion::flags::{KEEP_GFX, SKIP_ATTACK_COUNT, SKIP_MODEL};
use super::{attr, ms, u};
use crate::fpu::reg;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// The copies' function addresses.
pub(super) mod addr {
    pub const ON_CHANGE_ACTION: u32 = 0x800F_DCE0;
    pub const FORGET_BLASTER: u32 = 0x800F_DEB4;
    pub const REMOVE_BLASTER: u32 = 0x800F_DEE0;
    pub const FIRE: u32 = 0x800F_DF30;
    pub const CREATE_BLASTER_SHOT: u32 = 0x800F_E0E0;
    pub const ENTER: u32 = 0x800F_E100;
    pub const AIR_ENTER: u32 = 0x800F_E240;
    pub const N_START_ANIM: u32 = 0x800F_E360;
    pub const N_LOOP_ANIM: u32 = 0x800F_E450;
    pub const N_END_ANIM: u32 = 0x800F_E5C8;
    pub const AIR_N_START_ANIM: u32 = 0x800F_E688;
    pub const AIR_N_LOOP_ANIM: u32 = 0x800F_E778;
    pub const AIR_N_END_ANIM: u32 = 0x800F_E928;
    pub const N_START_IASA: u32 = 0x800F_EA50;
    pub const N_LOOP_IASA: u32 = 0x800F_EA78;
    pub const N_END_IASA: u32 = 0x800F_EAA0;
    pub const AIR_N_START_IASA: u32 = 0x800F_EAA4;
    pub const AIR_N_LOOP_IASA: u32 = 0x800F_EACC;
    pub const AIR_N_END_IASA: u32 = 0x800F_EAF4;
    pub const N_START_PHYS: u32 = 0x800F_EAF8;
    pub const N_LOOP_PHYS: u32 = 0x800F_EB18;
    pub const N_END_PHYS: u32 = 0x800F_EB38;
    pub const AIR_N_START_PHYS: u32 = 0x800F_EB58;
    pub const AIR_N_LOOP_PHYS: u32 = 0x800F_EB78;
    pub const AIR_N_END_PHYS: u32 = 0x800F_EB98;
    pub const N_START_COLL: u32 = 0x800F_EBB8;
    pub const N_LOOP_COLL: u32 = 0x800F_EBD8;
    pub const N_END_COLL: u32 = 0x800F_EBF8;
    pub const AIR_N_START_COLL: u32 = 0x800F_EC18;
    pub const AIR_N_LOOP_COLL: u32 = 0x800F_EC38;
    pub const AIR_N_END_COLL: u32 = 0x800F_EC58;
}

/// `u.kb.xB0`: the blaster he holds.
pub(crate) const BLASTER: usize = 0xB0;
/// `mv.fx.SpecialN.isBlasterLoop`: B pressed again for another shot.
const MV_LOOP: usize = 0x0;
/// `FtPart_R3rdNa`: the joint the blaster is held at.
const PART_R3RD_NA: i32 = 44;
/// The copied kinds.
const KIND_FOX: i32 = 1;
const KIND_FALCO: i32 = 22;
/// `HSD_PAD_B`.
const PAD_B: u32 = 0x200;
/// The shot's sound, facing right and left (`ftKb_Init_804D3DB8`, `ftKb_Init_804D3DC0`).
const FOX_SFX: [i32; 2] = [0x1AE17, 0x1AE1A];
const FALCO_SFX: [i32; 2] = [0x18703, 0x18706];

/// The copy's six states, as their first.
fn first_state(kind: i32) -> i32 {
    if kind == KIND_FALCO {
        ms::FC_SPECIAL_N_START
    } else {
        ms::FX_SPECIAL_N_START
    }
}

impl Running<'_> {
    /// A point along his hand's joint (`FtPart_R3rdNa`) in the world: `(0, 1.45, z)`, or the
    /// origin without him.
    fn kb_fx_hand(&mut self, gobj: Option<GObjId>, z: f32) -> Vec3 {
        let Some(gobj) = gobj else {
            return Vec3::new(0.0, 0.0, 0.0);
        };
        let j = self.bone_joint(gobj, PART_R3RD_NA);
        let offset = Vec3::new(0.0, 1.45, z);
        crate::lb::joint::world_position(&mut self.game.objs, self.data, j, Some(offset))
    }

    /// Where his shots leave the blaster, `ftKb_SpecialNFx_800FDC00`.
    #[doc(alias = "ftKb_SpecialNFx_800FDC00")]
    pub fn kb_fx_muzzle(&mut self, gobj: Option<GObjId>) -> Vec3 {
        self.kb_fx_hand(gobj, 5.016)
    }

    /// Where the blaster's grip is, `ftKb_SpecialNFx_800FDC70`.
    #[doc(alias = "ftKb_SpecialNFx_800FDC70")]
    pub fn kb_fx_grip(&mut self, gobj: Option<GObjId>) -> Vec3 {
        self.kb_fx_hand(gobj, 0.016)
    }

    /// Leaving the loop, `ftKb_SpecialNFx_OnChangeAction` (`x21EC`).
    #[doc(alias = "ftKb_SpecialNFx_OnChangeAction")]
    pub fn kb_fx_on_change_action(&mut self, gobj: GObjId) {
        self.renew_attack_instance(gobj);
        self.renew_attack_record(gobj);
    }

    /// Whether he holds no blaster, `ftKb_SpecialNFx_800FDD14`.
    #[doc(alias = "ftKb_SpecialNFx_800FDD14")]
    pub fn kb_fx_no_blaster(&self, gobj: Option<GObjId>) -> bool {
        let Some(gobj) = gobj else { return true };
        self.game.fighter(gobj).u.gobj_at(BLASTER).is_none()
    }

    /// The blaster's state for his action, `ftKb_SpecialNFx_800FDD4C`: the copy's six states
    /// in order, the back, up and down throws as 6 to 8, else 9.
    #[doc(alias = "ftKb_SpecialNFx_800FDD4C")]
    pub fn kb_fx_blaster_action(&self, gobj: Option<GObjId>) -> i32 {
        let Some(gobj) = gobj else { return 9 };
        let fp = self.game.fighter(gobj);
        let kind = fp.u_i32(u::HAT_KIND);
        if kind != KIND_FOX && kind != KIND_FALCO {
            return 9;
        }
        let m = fp.motion_id;
        let first = first_state(kind);
        if (first..first + 6).contains(&m) {
            m - first
        } else if (co_ms::THROW_B..=co_ms::THROW_LW).contains(&m) {
            m - 0xD6
        } else {
            9
        }
    }

    /// Whether his action no longer needs the blaster, `ftKb_SpecialNFx_800FDDF4`: in one of
    /// the blaster's actions, when its flags say so (`x2071_b6`).
    #[doc(alias = "ftKb_SpecialNFx_800FDDF4")]
    pub fn kb_fx_blaster_not_needed(&self, gobj: Option<GObjId>) -> bool {
        let Some(gobj) = gobj else { return true };
        let fp = self.game.fighter(gobj);
        let kind = fp.u_i32(u::HAT_KIND);
        if kind != KIND_FOX && kind != KIND_FALCO {
            return true;
        }
        let m = fp.motion_id;
        let first = first_state(kind);
        if (first..first + 6).contains(&m) || (co_ms::THROW_B..=co_ms::THROW_LW).contains(&m) {
            fp.x2070[1] & 0x02 != 0
        } else {
            true
        }
    }

    /// He forgets the blaster, `ftKb_SpecialNFx_800FDEB4`.
    #[doc(alias = "ftKb_SpecialNFx_800FDEB4")]
    pub fn kb_fx_forget_blaster(&mut self, gobj: Option<GObjId>) {
        let Some(gobj) = gobj else { return };
        let fp = self.game.fighter_mut(gobj);
        if fp.u.gobj_at(BLASTER).is_some() {
            fp.u.set_gobj_at(BLASTER, None);
        }
    }

    /// The blaster taken away, `ftKb_SpecialNFx_800FDEE0` (`it_802AEAB4`).
    #[doc(alias = "ftKb_SpecialNFx_800FDEE0")]
    pub fn kb_fx_remove_blaster(&mut self, gobj: GObjId) {
        if let Some(b) = self.game.fighter(gobj).u.gobj_at(BLASTER) {
            foxblaster::remove(self, Some(b));
            self.game.fighter_mut(gobj).u.set_gobj_at(BLASTER, None);
        }
    }

    /// A shot fired when the script asks (`cmd_vars[2]`), `ftKb_SpecialNFx_800FDF30` (and its
    /// accessory, `ftKb_SpecialNFx_CreateBlasterShot`): from the muzzle, at the copy's angle
    /// (mirrored facing left), speed and shot kind, with its sound.
    #[doc(alias = "ftKb_SpecialNFx_800FDF30")]
    #[doc(alias = "ftKb_SpecialNFx_CreateBlasterShot")]
    pub fn kb_fx_fire(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[2] == 0 {
            return;
        }
        self.game.fighters[f.0].cmd_vars[2] = 0;
        let mut pos = self.kb_fx_muzzle(Some(gobj));
        pos.z = 0.0;
        let (angle, speed, shot, sfx) = match self.game.fighters[f.0].u_i32(u::HAT_KIND) {
            KIND_FOX => (
                attr::FX_ANGLE,
                attr::FX_SPEED,
                attr::FX_SHOT_ITKIND,
                FOX_SFX,
            ),
            KIND_FALCO => (
                attr::FC_ANGLE,
                attr::FC_SPEED,
                attr::FC_SHOT_ITKIND,
                FALCO_SFX,
            ),
            _ => return,
        };
        let a = reg(self.kb_attr_f32(f, angle));
        let a = if self.game.fighters[f.0].facing_dir == 1.0 {
            a
        } else {
            fp::fsub(std::f64::consts::PI, a)
        };
        let speed = self.kb_attr_f32(f, speed);
        let kind = self.kb_attr_i32(f, shot);
        foxlaser::spawn_0(self, crate::fpu::mem(fp::frsp(a)), speed, gobj, pos, kind);
        let b = self.game.fighters[f.0]
            .u
            .gobj_at(BLASTER)
            .expect("a shot without a blaster");
        foxblaster::shot(self.game, b);
        let left = usize::from(self.game.fighters[f.0].facing_dir == -1.0);
        self.fighter_play_sfx(f, sfx[left], 0x7F, 0x40);
    }

    /// The blaster drawn, his way, at his position on his hand's joint: the copy's kind of
    /// blaster; the original asserts it's made.
    fn kb_fx_spawn_blaster(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let gun = match self.game.fighters[f.0].u_i32(u::HAT_KIND) {
            KIND_FOX => self.kb_attr_i32(f, attr::FX_GUN_ITKIND),
            KIND_FALCO => self.kb_attr_i32(f, attr::FC_GUN_ITKIND),
            k => panic!("the blaster of copy kind {k}"),
        };
        let fp = &self.game.fighters[f.0];
        let (facing, pos) = (fp.facing_dir, fp.cur_pos);
        let b = foxblaster::spawn(self, facing, Some(gobj), pos, PART_R3RD_NA, gun);
        self.game.fighters[f.0].u.set_gobj_at(BLASTER, b);
        if b.is_none() {
            panic!("the original's assertion failed: ftToSpecialNFox::Caution!!!");
        }
        self.kb_set_cleanup_cbs(gobj);
    }

    /// The copy's neutral special on the ground, `ftKb_SpecialNFx_800FE100`.
    #[doc(alias = "ftKb_SpecialNFx_800FE100")]
    pub fn kb_fx_enter(&mut self, gobj: GObjId) {
        self.land(gobj);
        let first = first_state(self.game.fighter(gobj).u_i32(u::HAT_KIND));
        self.change_motion_state(gobj, first, 0, 0.0, 1.0, 0.0, None);
        self.game.fighter_mut(gobj).cmd_vars = [0; 4];
        self.anim_update(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.gr_vel = 0.0;
        fp.self_vel.z = 0.0;
        fp.self_vel.y = 0.0;
        fp.self_vel.x = 0.0;
        fp.set_mv_i32(MV_LOOP, 0);
        self.kb_fx_spawn_blaster(gobj);
    }

    /// In the air, `ftKb_SpecialNFx_800FE240`.
    #[doc(alias = "ftKb_SpecialNFx_800FE240")]
    pub fn kb_fx_air_enter(&mut self, gobj: GObjId) {
        let first = first_state(self.game.fighter(gobj).u_i32(u::HAT_KIND));
        self.change_motion_state(gobj, first + 3, 0, 0.0, 1.0, 0.0, None);
        self.game.fighter_mut(gobj).cmd_vars = [0; 4];
        self.anim_update(gobj);
        self.game.fighter_mut(gobj).set_mv_i32(MV_LOOP, 0);
        self.kb_fx_spawn_blaster(gobj);
    }

    /// The blaster shown, and its pulse started when the script asks (`cmd_vars[3]` 1).
    fn kb_fx_update_blaster(&mut self, gobj: GObjId) {
        let b = self.game.fighter(gobj).u.gobj_at(BLASTER);
        foxblaster::set_visible(self, b.expect("a null blaster"), 1);
        let b = self.game.fighter(gobj).u.gobj_at(BLASTER);
        if self.game.fighter(gobj).cmd_vars[3] == 1
            && let Some(b) = b
        {
            self.game.fighter_mut(gobj).cmd_vars[3] = 0;
            foxblaster::start_pulse(self, b);
        }
    }

    /// The draw (`ftKb_FxSpecialNStart_Anim`, `ftKb_FxSpecialAirNStart_Anim`): into the loop
    /// (the state 1 after `start`) when it ends.
    fn kb_fx_start_anim(&mut self, gobj: GObjId, start: i32) {
        self.kb_fx_update_blaster(gobj);
        if !self.anim_frames_remaining(gobj) {
            self.change_motion_state(gobj, start + 1, SKIP_MODEL | KEEP_GFX, 0.0, 1.0, 0.0, None);
            self.kb_set_cleanup_cbs(gobj);
            self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::CREATE_BLASTER_SHOT));
            let b = self.game.fighter(gobj).u.gobj_at(BLASTER);
            foxblaster::set_visible(self, b.expect("a null blaster"), 1);
        }
    }

    /// `ftKb_FxSpecialNStart_Anim`.
    #[doc(alias = "ftKb_FxSpecialNStart_Anim")]
    pub fn kb_fx_n_start_anim(&mut self, gobj: GObjId) {
        let first = first_state(self.game.fighter(gobj).u_i32(u::HAT_KIND));
        self.kb_fx_start_anim(gobj, first);
    }

    /// `ftKb_FxSpecialAirNStart_Anim`.
    #[doc(alias = "ftKb_FxSpecialAirNStart_Anim")]
    pub fn kb_fx_air_n_start_anim(&mut self, gobj: GObjId) {
        let first = first_state(self.game.fighter(gobj).u_i32(u::HAT_KIND));
        self.kb_fx_start_anim(gobj, first + 3);
    }

    /// The loop (`ftKb_FxSpecialNLoop_Anim`, `ftKb_FxSpecialAirNLoop_Anim`, `start` their draw's
    /// state): again if B was pressed, else putting the blaster away; then the shot.
    fn kb_fx_loop_anim(&mut self, gobj: GObjId, start: i32, air: bool) {
        self.kb_fx_update_blaster(gobj);
        if !self.anim_frames_remaining(gobj) {
            if self.game.fighter(gobj).mv_i32(MV_LOOP) == 1 {
                self.game.fighter_mut(gobj).x21EC = Some(CodeRef(addr::ON_CHANGE_ACTION));
                self.change_motion_state(
                    gobj,
                    start + 1,
                    SKIP_ATTACK_COUNT | SKIP_MODEL | KEEP_GFX,
                    0.0,
                    1.0,
                    0.0,
                    None,
                );
                if air {
                    self.kb_set_cleanup_cbs(gobj);
                }
                let fp = self.game.fighter_mut(gobj);
                fp.accessory4_cb = Some(CodeRef(addr::CREATE_BLASTER_SHOT));
                fp.set_mv_i32(MV_LOOP, 0);
            } else {
                self.change_motion_state(
                    gobj,
                    start + 2,
                    SKIP_MODEL | KEEP_GFX,
                    0.0,
                    1.0,
                    0.0,
                    None,
                );
                if air {
                    self.kb_set_cleanup_cbs(gobj);
                }
                self.game.fighter_mut(gobj).cmd_vars[1] = 1;
            }
            let b = self.game.fighter(gobj).u.gobj_at(BLASTER);
            foxblaster::set_visible(self, b.expect("a null blaster"), 1);
            self.kb_set_cleanup_cbs(gobj);
        }
        self.kb_fx_fire(gobj);
    }

    /// `ftKb_FxSpecialNLoop_Anim`.
    #[doc(alias = "ftKb_FxSpecialNLoop_Anim")]
    pub fn kb_fx_n_loop_anim(&mut self, gobj: GObjId) {
        let first = first_state(self.game.fighter(gobj).u_i32(u::HAT_KIND));
        self.kb_fx_loop_anim(gobj, first, false);
    }

    /// `ftKb_FxSpecialAirNLoop_Anim`.
    #[doc(alias = "ftKb_FxSpecialAirNLoop_Anim")]
    pub fn kb_fx_air_n_loop_anim(&mut self, gobj: GObjId) {
        let first = first_state(self.game.fighter(gobj).u_i32(u::HAT_KIND));
        self.kb_fx_loop_anim(gobj, first + 3, true);
    }

    /// The put-away's start of each frame: the blaster's visibility from the script, the
    /// hand letting go when it's put away, its pulse running down; whether frames remain.
    fn kb_fx_end_update(&mut self, gobj: GObjId) -> bool {
        let vis = self.game.fighter(gobj).cmd_vars[1];
        let b = self.game.fighter(gobj).u.gobj_at(BLASTER);
        foxblaster::set_visible(self, b.expect("a null blaster"), vis as i32);
        if self.game.fighter(gobj).cmd_vars[1] == 2 {
            self.retake_held_item(gobj, false);
        }
        let b = self.game.fighter(gobj).u.gobj_at(BLASTER);
        if self.game.fighter(gobj).cmd_vars[3] == 2
            && let Some(b) = b
        {
            self.game.fighter_mut(gobj).cmd_vars[3] = 0;
            foxblaster::end_pulse(self.game, b);
        }
        self.anim_frames_remaining(gobj)
    }

    /// `ftKb_FxSpecialNEnd_Anim`: waiting at the end, the blaster forgotten.
    #[doc(alias = "ftKb_FxSpecialNEnd_Anim")]
    pub fn kb_fx_n_end_anim(&mut self, gobj: GObjId) {
        if !self.kb_fx_end_update(gobj) {
            self.kb_fx_forget_blaster(Some(gobj));
            self.enter_wait(gobj);
        }
    }

    /// `ftKb_FxSpecialAirNEnd_Anim`: falling at the end, helpless with the copy's landing lag.
    #[doc(alias = "ftKb_FxSpecialAirNEnd_Anim")]
    pub fn kb_fx_air_n_end_anim(&mut self, gobj: GObjId) {
        if self.kb_fx_end_update(gobj) {
            return;
        }
        self.kb_fx_forget_blaster(Some(gobj));
        let f = self.game.fighter_id(gobj);
        let lag = match self.game.fighters[f.0].u_i32(u::HAT_KIND) {
            KIND_FOX => self.kb_attr_f32(f, attr::FX_FREEFALL),
            KIND_FALCO => self.kb_attr_f32(f, attr::FC_FREEFALL),
            k => panic!("the landing lag of copy kind {k}"),
        };
        if lag == 0.0 {
            self.enter_fall(gobj);
            return;
        }
        self.enter_fall_special(gobj, 1, 0, 1, 1.0, lag, 0.0, true);
    }

    /// B pressed again in the window loops the shot (`ftFox_SpecialN_CheckLoopInput`).
    #[doc(alias = "ftKb_FxSpecialNStart_IASA")]
    #[doc(alias = "ftKb_FxSpecialNLoop_IASA")]
    #[doc(alias = "ftKb_FxSpecialAirNStart_IASA")]
    #[doc(alias = "ftKb_FxSpecialAirNLoop_IASA")]
    pub fn kb_fx_loop_input(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.cmd_vars[0] != 0 && fp.input.pressed_buttons & PAD_B != 0 {
            fp.set_mv_i32(MV_LOOP, 1);
        }
    }

    /// The ends' empty interrupts.
    #[doc(alias = "ftKb_FxSpecialNEnd_IASA")]
    #[doc(alias = "ftKb_FxSpecialAirNEnd_IASA")]
    pub fn kb_fx_no_iasa(&mut self, _gobj: GObjId) {}

    /// The grounded states' friction, `ft_80084F3C`.
    #[doc(alias = "ftKb_FxSpecialNStart_Phys")]
    #[doc(alias = "ftKb_FxSpecialNLoop_Phys")]
    #[doc(alias = "ftKb_FxSpecialNEnd_Phys")]
    pub fn kb_fx_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// The aerial states' physics, `ft_80084DB0`.
    #[doc(alias = "ftKb_FxSpecialAirNStart_Phys")]
    #[doc(alias = "ftKb_FxSpecialAirNLoop_Phys")]
    #[doc(alias = "ftKb_FxSpecialAirNEnd_Phys")]
    pub fn kb_fx_air_phys(&mut self, gobj: GObjId) {
        self.air_physics(gobj);
    }

    /// The grounded states' collision, `ft_80083F88`.
    #[doc(alias = "ftKb_FxSpecialNStart_Coll")]
    #[doc(alias = "ftKb_FxSpecialNLoop_Coll")]
    #[doc(alias = "ftKb_FxSpecialNEnd_Coll")]
    pub fn kb_fx_coll(&mut self, gobj: GObjId) {
        self.ground_coll_fall_off(gobj);
    }

    /// The aerial states' collision, `ftCo_AirCatchHit_Coll`.
    #[doc(alias = "ftKb_FxSpecialAirNStart_Coll")]
    #[doc(alias = "ftKb_FxSpecialAirNLoop_Coll")]
    #[doc(alias = "ftKb_FxSpecialAirNEnd_Coll")]
    pub fn kb_fx_air_coll(&mut self, gobj: GObjId) {
        self.co_air_catch_hit_coll(gobj);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::ON_CHANGE_ACTION => g.kb_fx_on_change_action(gobj),
        addr::FORGET_BLASTER => g.kb_fx_forget_blaster(Some(gobj)),
        addr::REMOVE_BLASTER => g.kb_fx_remove_blaster(gobj),
        addr::FIRE | addr::CREATE_BLASTER_SHOT => g.kb_fx_fire(gobj),
        addr::ENTER => g.kb_fx_enter(gobj),
        addr::AIR_ENTER => g.kb_fx_air_enter(gobj),
        addr::N_START_ANIM => g.kb_fx_n_start_anim(gobj),
        addr::N_LOOP_ANIM => g.kb_fx_n_loop_anim(gobj),
        addr::N_END_ANIM => g.kb_fx_n_end_anim(gobj),
        addr::AIR_N_START_ANIM => g.kb_fx_air_n_start_anim(gobj),
        addr::AIR_N_LOOP_ANIM => g.kb_fx_air_n_loop_anim(gobj),
        addr::AIR_N_END_ANIM => g.kb_fx_air_n_end_anim(gobj),
        addr::N_START_IASA | addr::N_LOOP_IASA | addr::AIR_N_START_IASA | addr::AIR_N_LOOP_IASA => {
            g.kb_fx_loop_input(gobj)
        }
        addr::N_END_IASA | addr::AIR_N_END_IASA => g.kb_fx_no_iasa(gobj),
        addr::N_START_PHYS | addr::N_LOOP_PHYS | addr::N_END_PHYS => g.kb_fx_phys(gobj),
        addr::AIR_N_START_PHYS | addr::AIR_N_LOOP_PHYS | addr::AIR_N_END_PHYS => {
            g.kb_fx_air_phys(gobj)
        }
        addr::N_START_COLL | addr::N_LOOP_COLL | addr::N_END_COLL => g.kb_fx_coll(gobj),
        addr::AIR_N_START_COLL | addr::AIR_N_LOOP_COLL | addr::AIR_N_END_COLL => {
            g.kb_fx_air_coll(gobj)
        }
        _ => return false,
    }
    true
}
