//! The blaster (`ftfoxspecialn.c`): drawing it, firing shots while B is pressed again, putting
//! it away, and the shots fired during Fox's and Falco's throws. The blaster item's own code
//! is `it::kinds::foxblaster`, which also has the queries it asks of its owner.

use gekko_fp as fp;

use super::super::super::code::Val;
use super::super::super::it::kinds::{foxblaster, foxlaser};
use super::super::super::state::Running;
use super::super::motion::flags::{KEEP_GFX, SKIP_ATTACK_COUNT, SKIP_MODEL};
use super::{BLASTER, KIND_FALCO, KIND_FOX, PAD_B, addr, attr, ext, ms, mv};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// `FtPart_RThumbNb`: the joint the blaster is held at.
const PART_R_THUMB_NB: i32 = 49;

/// The shot's sound, facing right and left (`foxSFX`, `falcoSFX`).
const FOX_SFX: [i32; 2] = [110103, 110106];
const FALCO_SFX: [i32; 2] = [100099, 100102];

impl Running<'_> {
    /// The blaster GObj Fox holds (`u.fx.x222C_blasterGObj`).
    pub fn fox_blaster(&self, gobj: GObjId) -> Option<GObjId> {
        self.game.fighter(gobj).u.gobj_at(BLASTER)
    }

    fn set_fox_blaster(&mut self, gobj: GObjId, b: Option<GObjId>) {
        self.game.fighter_mut(gobj).u.set_gobj_at(BLASTER, b);
    }

    /// `ftFox_SpecialN_GetHoldJoint`: a point `z` along the right thumb's joint, in the world.
    fn hold_joint(&mut self, gobj: GObjId, z: f32) -> Vec3 {
        let offset = Vec3::new(0.0, 1.232_500_1, z);
        let j = self.part_joint(gobj, PART_R_THUMB_NB);
        crate::lb::joint::world_position(&mut self.game.objs, self.data, j, Some(offset))
    }

    /// Where Fox's shots leave the blaster, `ftFx_SpecialN_FtGetHoldJoint`.
    #[doc(alias = "ftFx_SpecialN_FtGetHoldJoint")]
    pub fn fox_muzzle(&mut self, gobj: GObjId) -> Vec3 {
        self.hold_joint(gobj, 4.263_6)
    }

    /// Where the blaster's grip is, `ftFx_SpecialN_ItGetHoldJoint`.
    #[doc(alias = "ftFx_SpecialN_ItGetHoldJoint")]
    pub fn fox_grip(&mut self, gobj: GObjId) -> Vec3 {
        self.hold_joint(gobj, 0.013_600_001)
    }

    /// Leaving the neutral special's loop, `ftFx_SpecialN_OnChangeAction` (`x21EC`).
    #[doc(alias = "ftFx_SpecialN_OnChangeAction")]
    pub fn fox_special_n_on_change_action(&mut self, gobj: GObjId) {
        self.ext_gobj(ext::FT_800892A0, gobj);
        self.ext_gobj(ext::FT_80089824, gobj);
    }

    /// Takes the blaster away, `ftFx_SpecialN_RemoveBlaster`.
    #[doc(alias = "ftFx_SpecialN_RemoveBlaster")]
    pub fn fox_remove_blaster(&mut self, gobj: GObjId) {
        if let Some(b) = self.fox_blaster(gobj) {
            foxblaster::remove(self, Some(b));
            self.set_fox_blaster(gobj, None);
        }
        if !self.fox_has_blaster(gobj) {
            self.set_damage_cb(gobj, None);
        }
    }

    /// Fires a shot when the script asks (`cmd_vars[2]`), `ftFx_SpecialN_CreateBlasterShot`.
    #[doc(alias = "ftFx_SpecialN_CreateBlasterShot")]
    pub fn fox_create_blaster_shot(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.cmd_vars[2] != 0 {
            fp.cmd_vars[2] = 0;
            self.fox_fire_shot(gobj);
        }
    }

    /// `ftFox_SpecialN_PrepareBlasterShot` and `ftFox_SpecialN_FireBlasterShot`.
    fn fox_fire_shot(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let mut pos = self.fox_muzzle(gobj);
        pos.z = 0.0;
        let angle = reg(self.fox_attr_f32(f, attr::BLASTER_ANGLE));
        let angle = if self.game.fighters[f.0].facing_dir == 1.0 {
            angle
        } else {
            fp::fsub(std::f64::consts::PI, angle)
        };
        let speed = self.fox_attr_f32(f, attr::BLASTER_VEL);
        let kind = self.fox_attr_i32(f, attr::BLASTER_SHOT_ITKIND);
        let angle = crate::fpu::mem(fp::frsp(angle));
        foxlaser::spawn_0(self, angle, speed, gobj, pos, kind);
        let b = self.fox_blaster(gobj).expect("a shot without a blaster");
        foxblaster::shot(self.game, b);
        let left = usize::from(self.game.fighters[f.0].facing_dir == -1.0);
        match self.game.ft_kind(gobj) {
            KIND_FOX => self.fighter_play_sfx(f, FOX_SFX[left], 0x7F, 0x40),
            KIND_FALCO => self.fighter_play_sfx(f, FALCO_SFX[left], 0x7F, 0x40),
            _ => {}
        }
    }

    /// Draws the blaster, `ftFox_SpecialN_SpawnBlaster`.
    fn fox_spawn_blaster(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let part = self.bone_index(&self.game.fighters[f.0], PART_R_THUMB_NB);
        let kind = self.fox_attr_i32(f, attr::BLASTER_GUN_ITKIND);
        let fp = &self.game.fighters[f.0];
        let (facing, pos) = (fp.facing_dir, fp.cur_pos);
        let b = foxblaster::spawn(self, facing, Some(gobj), pos, part, kind);
        self.set_fox_blaster(gobj, b);
        let Some(b) = b else {
            panic!("ftToSpecialNFox::Caution!!! (the blaster wasn't made)");
        };
        let data = self.data;
        crate::game::it::lib26b1::set_scale(self.game, data, b, mem(fp::frsp(0.85)));
        self.set_damage_cb(gobj, Some(addr::REMOVE_BLASTER_CB));
    }

    /// `ftFox_SpecialN_InitializeState`: the script's variables cleared, a frame of
    /// animation.
    fn fox_special_n_init(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).cmd_vars = [0; 4];
        self.anim_update(gobj);
    }

    /// The grounded neutral special, `ftFx_SpecialN_Enter`.
    #[doc(alias = "ftFx_SpecialN_Enter")]
    pub fn fox_special_n_enter(&mut self, gobj: GObjId) {
        self.land(gobj);
        self.change_motion_state(gobj, ms::SPECIAL_N_START, 0, 0.0, 1.0, 0.0, None);
        self.fox_special_n_init(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.gr_vel = 0.0;
        fp.self_vel = Vec3::new(0.0, 0.0, 0.0);
        fp.set_mv_i32(mv::BLASTER_LOOP, 0);
        self.fox_spawn_blaster(gobj);
    }

    /// The aerial neutral special, `ftFx_SpecialAirN_Enter`.
    #[doc(alias = "ftFx_SpecialAirN_Enter")]
    pub fn fox_special_air_n_enter(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::SPECIAL_AIR_N_START, 0, 0.0, 1.0, 0.0, None);
        self.fox_special_n_init(gobj);
        self.game.fighter_mut(gobj).set_mv_i32(mv::BLASTER_LOOP, 0);
        self.fox_spawn_blaster(gobj);
    }

    /// `ftFox_SpecialN_UpdateBlaster`: the blaster shown, and its pulse started when the
    /// script asks (`cmd_vars[3]` 1).
    fn fox_update_blaster(&mut self, gobj: GObjId) {
        let b = self.fox_blaster(gobj);
        foxblaster::set_visible(self, b.expect("a null blaster"), 1);
        let b = self.fox_blaster(gobj);
        if self.game.fighter(gobj).cmd_vars[3] == 1
            && let Some(b) = b
        {
            self.game.fighter_mut(gobj).cmd_vars[3] = 0;
            foxblaster::start_pulse(self, b);
        }
    }

    /// `ftFox_SpecialN_StartAnimation`: into the loop when the draw ends.
    fn fox_special_n_start_anim(&mut self, gobj: GObjId, loop_msid: i32) {
        self.fox_update_blaster(gobj);
        if !self.anim_frames_remaining(gobj) {
            self.change_motion_state(gobj, loop_msid, SKIP_MODEL | KEEP_GFX, 0.0, 1.0, 0.0, None);
            self.set_damage_cb(gobj, Some(addr::REMOVE_BLASTER_CB));
            self.game.fighter_mut(gobj).accessory4_cb =
                Some(code_ref(addr::SPECIAL_N_CREATE_BLASTER_SHOT));
            let b = self.fox_blaster(gobj).expect("a null blaster");
            foxblaster::set_visible(self, b, 1);
        }
    }

    /// The grounded draw, `ftFx_SpecialNStart_Anim`.
    #[doc(alias = "ftFx_SpecialNStart_Anim")]
    pub fn fox_special_n_start_anim_cb(&mut self, gobj: GObjId) {
        self.fox_special_n_start_anim(gobj, ms::SPECIAL_N_LOOP);
    }

    /// The aerial draw, `ftFx_SpecialAirNStart_Anim`.
    #[doc(alias = "ftFx_SpecialAirNStart_Anim")]
    pub fn fox_special_air_n_start_anim(&mut self, gobj: GObjId) {
        self.fox_special_n_start_anim(gobj, ms::SPECIAL_AIR_N_LOOP);
    }

    /// `ftFox_SpecialN_BeginLoopTransition`: the loop again.
    fn fox_special_n_loop_again(&mut self, gobj: GObjId, loop_msid: i32) {
        self.game.fighter_mut(gobj).x21EC = Some(code_ref(addr::SPECIAL_N_ON_CHANGE_ACTION));
        self.change_motion_state(
            gobj,
            loop_msid,
            SKIP_ATTACK_COUNT | SKIP_MODEL | KEEP_GFX,
            0.0,
            1.0,
            0.0,
            None,
        );
    }

    /// `ftFox_SpecialN_FinishLoopTransition`.
    fn fox_special_n_loop_finish(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.accessory4_cb = Some(code_ref(addr::SPECIAL_N_CREATE_BLASTER_SHOT));
        fp.set_mv_i32(mv::BLASTER_LOOP, 0);
        let b = self.fox_blaster(gobj).expect("a null blaster");
        foxblaster::set_visible(self, b, 1);
    }

    /// `ftFox_SpecialN_FinishEndTransition`.
    fn fox_special_n_end_finish(&mut self, gobj: GObjId) {
        let b = self.fox_blaster(gobj);
        self.game.fighter_mut(gobj).cmd_vars[1] = 1;
        foxblaster::set_visible(self, b.expect("a null blaster"), 1);
    }

    /// The shot fired this frame, if the script asks, in the loops' animation callbacks.
    fn fox_loop_shot(&mut self, gobj: GObjId) {
        self.fox_create_blaster_shot(gobj);
    }

    /// The grounded loop, `ftFx_SpecialNLoop_Anim`: again if B was pressed, else putting
    /// the blaster away.
    #[doc(alias = "ftFx_SpecialNLoop_Anim")]
    pub fn fox_special_n_loop_anim(&mut self, gobj: GObjId) {
        self.fox_update_blaster(gobj);
        if !self.anim_frames_remaining(gobj) {
            if self.game.fighter(gobj).mv_i32(mv::BLASTER_LOOP) == 1 {
                self.fox_special_n_loop_again(gobj, ms::SPECIAL_N_LOOP);
                self.fox_special_n_loop_finish(gobj);
            } else {
                self.change_motion_state(
                    gobj,
                    ms::SPECIAL_N_END,
                    SKIP_MODEL | KEEP_GFX,
                    0.0,
                    1.0,
                    0.0,
                    None,
                );
                self.fox_special_n_end_finish(gobj);
            }
            self.set_damage_cb(gobj, Some(addr::REMOVE_BLASTER_CB));
        }
        self.fox_loop_shot(gobj);
    }

    /// The aerial loop, `ftFx_SpecialAirNLoop_Anim`.
    #[doc(alias = "ftFx_SpecialAirNLoop_Anim")]
    pub fn fox_special_air_n_loop_anim(&mut self, gobj: GObjId) {
        self.fox_update_blaster(gobj);
        if !self.anim_frames_remaining(gobj) {
            if self.game.fighter(gobj).mv_i32(mv::BLASTER_LOOP) == 1 {
                self.fox_special_n_loop_again(gobj, ms::SPECIAL_AIR_N_LOOP);
                self.set_damage_cb(gobj, Some(addr::REMOVE_BLASTER_CB));
                self.fox_special_n_loop_finish(gobj);
            } else {
                self.change_motion_state(
                    gobj,
                    ms::SPECIAL_AIR_N_END,
                    SKIP_MODEL | KEEP_GFX,
                    0.0,
                    1.0,
                    0.0,
                    None,
                );
                self.set_damage_cb(gobj, Some(addr::REMOVE_BLASTER_CB));
                self.fox_special_n_end_finish(gobj);
            }
            self.set_damage_cb(gobj, Some(addr::REMOVE_BLASTER_CB));
        }
        self.fox_loop_shot(gobj);
    }

    /// `ftFox_SpecialN_UpdateEndAnimation`: the blaster's visibility from the script, the
    /// hand letting go when it's put away, its pulse running down; whether frames remain.
    fn fox_special_n_end_update(&mut self, gobj: GObjId) -> bool {
        let vis = self.game.fighter(gobj).cmd_vars[1];
        let b = self.fox_blaster(gobj);
        foxblaster::set_visible(self, b.expect("a null blaster"), vis as i32);
        if self.game.fighter(gobj).cmd_vars[1] == 2 {
            self.ext(
                ext::FTPICKUPITEM_80094818,
                &[Val::GObj(Some(gobj)), Val::Int(0)],
            );
        }
        let b = self.fox_blaster(gobj);
        if self.game.fighter(gobj).cmd_vars[3] == 2
            && let Some(b) = b
        {
            self.game.fighter_mut(gobj).cmd_vars[3] = 0;
            foxblaster::end_pulse(self.game, b);
        }
        self.anim_frames_remaining(gobj)
    }

    /// `ftFox_SpecialN_RemoveBlasterNULL`: Fox forgets the blaster.
    fn fox_forget_blaster(&mut self, gobj: GObjId) {
        if self.fox_blaster(gobj).is_some() {
            self.set_fox_blaster(gobj, None);
        }
        self.set_damage_cb(gobj, None);
    }

    /// The grounded end, `ftFx_SpecialNEnd_Anim`.
    #[doc(alias = "ftFx_SpecialNEnd_Anim")]
    pub fn fox_special_n_end_anim(&mut self, gobj: GObjId) {
        if !self.fox_special_n_end_update(gobj) {
            self.fox_forget_blaster(gobj);
            self.enter_wait(gobj);
        }
    }

    /// The aerial end, `ftFx_SpecialAirNEnd_Anim`: falling, or falling helplessly with the
    /// blaster's landing lag.
    #[doc(alias = "ftFx_SpecialAirNEnd_Anim")]
    pub fn fox_special_air_n_end_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if !self.fox_special_n_end_update(gobj) {
            self.fox_forget_blaster(gobj);
            let lag = self.fox_attr_f32(f, attr::BLASTER_LANDING_LAG);
            if lag == 0.0 {
                self.enter_fall(gobj);
                return;
            }
            self.fall_special(gobj, 1.0, lag);
        }
    }

    /// `ftFox_SpecialN_CheckLoopInput`: B pressed again in the window loops the shot
    /// (`ftFx_SpecialNStart_IASA`, `ftFx_SpecialNLoop_IASA` and the aerial ones).
    #[doc(alias = "ftFx_SpecialNStart_IASA")]
    #[doc(alias = "ftFx_SpecialNLoop_IASA")]
    #[doc(alias = "ftFx_SpecialAirNStart_IASA")]
    #[doc(alias = "ftFx_SpecialAirNLoop_IASA")]
    pub fn fox_special_n_loop_input(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.cmd_vars[0] != 0 && fp.input.pressed_buttons & PAD_B != 0 {
            fp.set_mv_i32(mv::BLASTER_LOOP, 1);
        }
    }

    /// The blaster during Fox's and Falco's throws, `ftFx_Throw_Anim`: drawn, shown and put
    /// away by the script, firing a shot down or back when it asks.
    #[doc(alias = "ftFx_Throw_Anim")]
    pub fn fox_throw_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let kind = self.game.fighters[f.0].kind;
        if kind != KIND_FOX && kind != KIND_FALCO {
            return;
        }
        if !self.anim_frames_remaining(gobj) {
            self.set_fox_blaster(gobj, None);
            self.set_damage_cb(gobj, None);
            return;
        }
        match self.game.fighters[f.0].cmd_vars[1] {
            1 => {
                let Some(b) = self.fox_blaster(gobj) else {
                    let part = self.bone_index(&self.game.fighters[f.0], PART_R_THUMB_NB);
                    let kind = self.fox_attr_i32(f, attr::BLASTER_GUN_ITKIND);
                    let fp = &self.game.fighters[f.0];
                    let (facing, pos) = (fp.facing_dir, fp.cur_pos);
                    let b = foxblaster::spawn(self, facing, Some(gobj), pos, part, kind);
                    self.set_fox_blaster(gobj, b);
                    if let Some(b) = b {
                        let fp = &self.game.fighters[f.0];
                        let scale = fp::fmuls(
                            0.850_000_023_841_857_9,
                            fp::fmuls(reg(fp.x34_scale.y), reg(fp.co_attrs.model_scaling)),
                        );
                        let data = self.data;
                        crate::game::it::lib26b1::set_scale(self.game, data, b, mem(scale));
                        self.set_damage_cb(gobj, Some(addr::REMOVE_BLASTER_CB));
                    }
                    return;
                };
                let vis = self.game.fighters[f.0].cmd_vars[1] as i32;
                foxblaster::set_visible(self, b, vis);
                match self.game.fighters[f.0].cmd_vars[3] {
                    1 => {
                        self.game.fighters[f.0].cmd_vars[3] = 0;
                        let b = self.fox_blaster(gobj).expect("a null blaster");
                        foxblaster::start_pulse(self, b);
                    }
                    2 => {
                        self.game.fighters[f.0].cmd_vars[3] = 0;
                        let b = self.fox_blaster(gobj).expect("a null blaster");
                        foxblaster::end_pulse(self.game, b);
                    }
                    _ => {}
                }
                // `ftCheckThrowB0`.
                if !self.game.fighters[f.0].throw_flag(0) {
                    return;
                }
                self.game.fighters[f.0].set_throw_flag(0, false);
                let mut muzzle = self.fox_muzzle(gobj);
                let mut grip = self.fox_grip(gobj);
                grip.z = 0.0;
                muzzle.z = 0.0;
                let angle = crate::math::atan2(
                    crate::fpu::mem(fp::fsubs(reg(muzzle.y), reg(grip.y))),
                    crate::fpu::mem(fp::fsubs(reg(muzzle.x), reg(grip.x))),
                );
                let speed = self.fox_attr_f32(f, attr::BLASTER_VEL);
                let shot = self.fox_attr_i32(f, attr::BLASTER_SHOT_ITKIND);
                let motion = self.game.fighters[f.0].motion_id;
                if matches!(motion, ms::THROW_B | ms::THROW_HI | ms::THROW_LW) {
                    foxlaser::spawn_1(self, angle, speed, gobj, muzzle, shot);
                } else {
                    foxlaser::spawn_0(self, angle, speed, gobj, muzzle, shot);
                }
                let b = self.fox_blaster(gobj).expect("a null blaster");
                foxblaster::shot(self.game, b);
                let kind = self.game.ft_kind(gobj);
                match self.game.fighters[f.0].motion_id {
                    ms::THROW_HI | ms::THROW_LW => match kind {
                        KIND_FOX => self.fighter_play_sfx(f, 110109, 0x7F, 0x40),
                        KIND_FALCO => self.fighter_play_sfx(f, 100105, 0x7F, 0x40),
                        _ => {}
                    },
                    ms::THROW_B => {
                        let right = usize::from(self.game.fighters[f.0].facing_dir == 1.0);
                        match kind {
                            KIND_FOX => self.fighter_play_sfx(f, FOX_SFX[right], 0x7F, 0x40),
                            KIND_FALCO => self.fighter_play_sfx(f, FALCO_SFX[right], 0x7F, 0x40),
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
            2 => {
                if self.fox_blaster(gobj).is_some() {
                    self.set_fox_blaster(gobj, None);
                    self.set_damage_cb(gobj, None);
                    match self.game.ft_kind(gobj) {
                        KIND_FOX => self.fighter_play_sfx(f, 110100, 0x7F, 0x40),
                        KIND_FALCO => self.fighter_play_sfx(f, 100096, 0x7F, 0x40),
                        _ => {}
                    }
                }
            }
            0 => {
                self.set_fox_blaster(gobj, None);
                self.set_damage_cb(gobj, None);
                self.ext(
                    ext::FTPICKUPITEM_80094818,
                    &[Val::GObj(Some(gobj)), Val::Int(0)],
                );
            }
            _ => {}
        }
    }
}

fn code_ref(a: u32) -> super::super::super::handles::CodeRef {
    super::super::super::handles::CodeRef(a)
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::SPECIAL_N_ON_CHANGE_ACTION => g.fox_special_n_on_change_action(gobj),
        addr::SPECIAL_N_REMOVE_BLASTER => g.fox_remove_blaster(gobj),
        addr::SPECIAL_N_CREATE_BLASTER_SHOT => g.fox_create_blaster_shot(gobj),
        addr::SPECIAL_N_ENTER => g.fox_special_n_enter(gobj),
        addr::SPECIAL_AIR_N_ENTER => g.fox_special_air_n_enter(gobj),
        // ftFx_SpecialNStart_Anim, ftFx_SpecialNLoop_Anim, ftFx_SpecialNEnd_Anim.
        0x800E_62A4 => g.fox_special_n_start_anim_cb(gobj),
        0x800E_6368 => g.fox_special_n_loop_anim(gobj),
        0x800E_65BC => g.fox_special_n_end_anim(gobj),
        // The aerial ones.
        0x800E_667C => g.fox_special_air_n_start_anim(gobj),
        0x800E_6740 => g.fox_special_air_n_loop_anim(gobj),
        0x800E_69BC => g.fox_special_air_n_end_anim(gobj),
        // The draw's and the loop's IASA, grounded and aerial.
        0x800E_6AB4 | 0x800E_6ADC | 0x800E_6B08 | 0x800E_6B30 => g.fox_special_n_loop_input(gobj),
        // The ends' IASA.
        0x800E_6B04 | 0x800E_6B58 => g.fox_no_iasa(gobj),
        // Physics: grounded friction, aerial.
        0x800E_6B5C | 0x800E_6B7C | 0x800E_6B9C => g.fox_ground_phys(gobj),
        0x800E_6BBC | 0x800E_6BDC | 0x800E_6BFC => g.fox_air_phys(gobj),
        // Collision.
        0x800E_6C1C | 0x800E_6C3C | 0x800E_6C5C => g.fox_special_n_coll(gobj),
        0x800E_6C7C | 0x800E_6C9C | 0x800E_6CBC => g.fox_special_air_n_coll(gobj),
        addr::THROW_ANIM => g.fox_throw_anim(gobj),
        _ => return false,
    }
    true
}

impl Running<'_> {
    /// The states' empty IASA callbacks (`ftFx_SpecialNEnd_IASA` and the many like it).
    #[doc(alias = "ftFx_SpecialNEnd_IASA")]
    #[doc(alias = "ftFx_SpecialAirNEnd_IASA")]
    #[doc(alias = "ftFx_SpecialSStart_IASA")]
    #[doc(alias = "ftFx_SpecialAirSStart_IASA")]
    #[doc(alias = "ftFx_SpecialSEnd_IASA")]
    #[doc(alias = "ftFx_SpecialAirSEnd_IASA")]
    #[doc(alias = "ftFx_SpecialHiHold_IASA")]
    #[doc(alias = "ftFx_SpecialHiHoldAir_IASA")]
    #[doc(alias = "ftFx_SpecialHi_IASA")]
    #[doc(alias = "ftFx_SpecialAirHi_IASA")]
    #[doc(alias = "ftFx_SpecialHiLanding_IASA")]
    #[doc(alias = "ftFx_SpecialHiFall_IASA")]
    #[doc(alias = "ftFx_SpecialHiBound_IASA")]
    #[doc(alias = "ftFx_SpecialAirLwStart_IASA")]
    #[doc(alias = "ftFx_SpecialLwTurn_IASA")]
    #[doc(alias = "ftFx_SpecialAirLwTurn_IASA")]
    #[doc(alias = "ftFx_SpecialLwHit_IASA")]
    #[doc(alias = "ftFx_SpecialAirLwHit_IASA")]
    #[doc(alias = "ftFx_SpecialLwEnd_IASA")]
    #[doc(alias = "ftFx_SpecialAirLwEnd_IASA")]
    #[doc(alias = "ftFx_AppealS_IASA")]
    pub fn fox_no_iasa(&mut self, _gobj: GObjId) {}

    /// Grounded friction, `ft_80084F3C` (`ftFx_SpecialNStart_Phys` and the others that only
    /// call it).
    #[doc(alias = "ftFx_SpecialNStart_Phys")]
    #[doc(alias = "ftFx_SpecialNLoop_Phys")]
    #[doc(alias = "ftFx_SpecialNEnd_Phys")]
    #[doc(alias = "ftFx_SpecialHiHold_Phys")]
    #[doc(alias = "ftFx_SpecialLwStart_Phys")]
    #[doc(alias = "ftFx_SpecialLwEnd_Phys")]
    #[doc(alias = "ftFx_AppealS_Phys")]
    pub fn fox_ground_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// Aerial physics, `ft_80084DB0` (`ftFx_SpecialAirNStart_Phys` and the others that only
    /// call it).
    #[doc(alias = "ftFx_SpecialAirNStart_Phys")]
    #[doc(alias = "ftFx_SpecialAirNLoop_Phys")]
    #[doc(alias = "ftFx_SpecialAirNEnd_Phys")]
    #[doc(alias = "ftFx_SpecialHiFall_Phys")]
    pub fn fox_air_phys(&mut self, gobj: GObjId) {
        self.air_physics(gobj);
    }

    /// The grounded neutral special's collision, `ft_80083F88`.
    #[doc(alias = "ftFx_SpecialNStart_Coll")]
    #[doc(alias = "ftFx_SpecialNLoop_Coll")]
    #[doc(alias = "ftFx_SpecialNEnd_Coll")]
    pub fn fox_special_n_coll(&mut self, gobj: GObjId) {
        self.ground_coll_fall_off(gobj);
    }

    /// The aerial neutral special's collision, `ftCo_AirCatchHit_Coll`.
    #[doc(alias = "ftFx_SpecialAirNStart_Coll")]
    #[doc(alias = "ftFx_SpecialAirNLoop_Coll")]
    #[doc(alias = "ftFx_SpecialAirNEnd_Coll")]
    pub fn fox_special_air_n_coll(&mut self, gobj: GObjId) {
        self.co_air_catch_hit_coll(gobj);
    }
}

pub(super) fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    let gobj = |i: usize| args[i].gobj().expect("a null fighter");
    Some(match f {
        addr::SPECIAL_N_FT_GET_HOLD_JOINT => Val::Vec(r.fox_muzzle(gobj(0))),
        addr::SPECIAL_N_IT_GET_HOLD_JOINT => Val::Vec(r.fox_grip(gobj(0))),
        _ => return None,
    })
}
