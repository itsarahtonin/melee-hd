//! The power-ups a fighter takes: the items it takes on touch (`ftColl_80077C60`'s Starman and
//! mushrooms), the Starman's invincibility (`ftColl_8007B7FC`) and its timer.

use super::super::it::{self, kinds};
use super::super::state::Running;
use super::FighterId;
use super::attack::victim;
use crate::hsd::gobj::GObjId;
use crate::lb::hitcapsule;

/// The Starman's color animation.
pub const STAR_COLANIM: i32 = 107;

impl Running<'_> {
    /// Fighter `me` takes item `g`'s box `j` on touch, `ftColl_80077C60`'s first branch: the
    /// box remembers it, and unless the fighter's out of stamina, a Starman turns it
    /// invincible and a mushroom queues a growth or a shrink (each turning the box off and
    /// telling the item it hit), and the item counts for its player.
    pub(super) fn take_touch_item(&mut self, g: GObjId, j: usize, me: FighterId) {
        let h = &it::item(self.game, g).x5D4_hitboxes[j].hit;
        let mode = if h.x41_b5 { 5 } else { 0 };
        self.item_add_victim(g, j, mode, victim(me), false);
        if self.game.fighters[me.0].stamina_dead {
            return;
        }
        match it::item(self.game, g).kind {
            kinds::STAR => {
                let ip = it::item_mut(self.game, g);
                ip.x5D4_hitboxes[j].hit.state = hitcapsule::DISABLED;
                ip.xC34_damageDealt = 1;
                let frames = it::lib2725::special_word(self.game, self.data, g);
                self.start_star(me, frames);
                self.fighter_play_sfx(me, 0xF9, 0x7F, 0x40);
                self.start_rumble(me, 0x11, 0);
            }
            kinds::KINOKO => {
                let ip = it::item_mut(self.game, g);
                ip.x5D4_hitboxes[j].hit.state = hitcapsule::DISABLED;
                self.game.fighters[me.0].x200C += 1;
                it::item_mut(self.game, g).xC34_damageDealt = 1;
            }
            kinds::DKINOKO => {
                let ip = it::item_mut(self.game, g);
                ip.x5D4_hitboxes[j].hit.state = hitcapsule::DISABLED;
                self.game.fighters[me.0].x2010 += 1;
                it::item_mut(self.game, g).xC34_damageDealt = 1;
            }
            _ => {}
        }
        let fp = &self.game.fighters[me.0];
        let (slot, sub) = (i32::from(fp.player_idx), i32::from(fp.is_sub_fighter));
        self.item_pickup_stat(slot, sub, g);
    }

    /// The Starman's invincibility for `frames` frames, `ftColl_8007B7FC`: its color animation,
    /// and the crowd's cheer once it lasts past the music's cue (`it_8026B588`).
    #[doc(alias = "ftColl_8007B7FC")]
    pub fn start_star(&mut self, f: FighterId, frames: i32) {
        let fp = &mut self.game.fighters[f.0];
        fp.x221D_b6 = true;
        fp.x2004 = frames;
        let gobj = fp.gobj.expect("a fighter without its GObj");
        self.start_color_anim(gobj, STAR_COLANIM, 0);
        if frames > self.star_cue() {
            self.star_cheer(f);
        }
    }

    /// The crowd's first loop wanted once more for the fighter, `ft_80088080`.
    #[doc(alias = "ft_80088080")]
    pub fn star_cheer(&mut self, f: FighterId) {
        self.game.fighters[f.0].x2164 += 1;
        self.lb_audio_crowd_a();
    }

    /// When the Starman's cheer ends, frames before its end, `it_8026B588` (`ItemCommonData`
    /// x0DC).
    #[doc(alias = "it_8026B588")]
    pub fn star_cue(&self) -> i32 {
        it::item::common_data(self.game).i32(self.data, 0xDC)
    }
}

/// `JOBJ_LIGHTING`, `JOBJ_TEXGEN`, `JOBJ_SPECULAR` and `JOBJ_UNK_B18` to `B20`.
mod jflag {
    pub const LIGHTING: u32 = 1 << 7;
    pub const TEXGEN: u32 = 1 << 8;
    pub const SPECULAR: u32 = 1 << 16;
    pub const B18: u32 = 1 << 18;
    pub const B19: u32 = 1 << 19;
    pub const B20: u32 = 1 << 20;
}

/// `RENDER_XLU` and `RENDER_NO_ZUPDATE`.
const RENDER_XLU: u32 = 1 << 30;
const RENDER_NO_ZUPDATE: u32 = 1 << 29;

/// `PlCo.dat`'s Bunny Hood and metal multipliers (`ftLoadCommonData` x34, x38).
pub(super) const BUNNY_PARAMS: u32 = 0x34;
pub(super) const METAL_PARAMS: u32 = 0x38;

impl Running<'_> {
    /// Every animated part's joints given the shiny look (lit, texture-generated, specular,
    /// `JOBJ_UNK_B18`; not B19 or B20) and their display objects drawn opaque, the shared body
    /// of `ft_800C80A4` and the metal's `enableMetal`.
    fn shine_parts(&mut self, f: FighterId) {
        use crate::hsd::{dobj, jobj};
        let n = self.parts_table(self.game.fighters[f.0].kind).len() as usize;
        for i in 0..n {
            let bone = self.bone(f, i as i32);
            if !super::anim::bone_flag(&bone, 1) {
                continue;
            }
            let j = bone.joint;
            let mut d = jobj::get_dobj(&self.game.objs.jobjs, j);
            jobj::set_flags(
                &mut self.game.objs.jobjs,
                j,
                jflag::LIGHTING | jflag::TEXGEN | jflag::SPECULAR | jflag::B18,
            );
            jobj::clear_flags(&mut self.game.objs.jobjs, j, jflag::B19 | jflag::B20);
            while let Some(o) = d {
                dobj::modify_flags(&mut self.game.objs.dobjs, Some(o), 2, 0xE);
                d = self.game.objs.dobjs[o].next;
            }
        }
    }

    /// The parts' shiny look, `ft_800C80A4`, as the Cloaking Device's refraction draws them.
    #[doc(alias = "ft_800C80A4")]
    pub fn shine_parts_for_cloak(&mut self, f: FighterId) {
        self.shine_parts(f);
    }

    /// The parts' own look back, `ft_800C8170`: each animated part's joint flags from its bone
    /// flags, and its display objects drawn by their material's blending.
    #[doc(alias = "ft_800C8170")]
    pub fn restore_parts_look(&mut self, f: FighterId) {
        use crate::hsd::{dobj, jobj};
        let n = self.parts_table(self.game.fighters[f.0].kind).len() as usize;
        for i in 0..n {
            let bone = self.bone(f, i as i32);
            if !super::anim::bone_flag(&bone, 1) {
                continue;
            }
            let j = bone.joint;
            let mut d = jobj::get_dobj(&self.game.objs.jobjs, j);
            let b2 = bone.x8[1];
            let mut flags = 0;
            if super::anim::bone_flag(&bone, 7) {
                flags |= jflag::LIGHTING;
            }
            if b2 & 0x80 != 0 {
                flags |= jflag::TEXGEN;
            }
            if b2 & 0x40 != 0 {
                flags |= jflag::SPECULAR;
            }
            if b2 & 0x20 != 0 {
                flags |= jflag::B18;
            }
            if b2 & 0x10 != 0 {
                flags |= jflag::B20;
            }
            if b2 & 0x08 != 0 {
                flags |= jflag::B19;
            }
            jobj::clear_flags(&mut self.game.objs.jobjs, j, 0x50180);
            jobj::set_flags(&mut self.game.objs.jobjs, j, flags);
            while let Some(o) = d {
                let m = self.game.objs.dobjs[o]
                    .mobj
                    .expect("a fighter's display object without a material");
                let mode =
                    match self.game.objs.mobjs[m].rendermode & (RENDER_XLU | RENDER_NO_ZUPDATE) {
                        0 => 2,
                        RENDER_XLU => 8,
                        x if x == RENDER_XLU | RENDER_NO_ZUPDATE => 4,
                        x => panic!("mobj has unexpected blending flags (0x{x:x})."),
                    };
                dobj::modify_flags(&mut self.game.objs.dobjs, Some(o), mode, 0xE);
                d = self.game.objs.dobjs[o].next;
            }
        }
    }

    /// Turns the fighter metal for `timer` frames and `health` damage, `ftCo_800C8348`: the
    /// shiny look and `x2225_b6` (`ftCommon_80080460`) unless it already was.
    #[doc(alias = "ftCo_800C8348")]
    pub fn set_metal(&mut self, gobj: GObjId, timer: i32, health: i32) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        fp.metal_timer = timer;
        fp.metal_health = health;
        if fp.is_metal {
            return;
        }
        fp.is_metal = true;
        self.shine_parts_metal(f);
    }

    /// The metal look, `enableMetal` (`ftmetal.c`'s inline): the parts' shiny look and
    /// `x2225_b6` (`ftCommon_80080460`).
    pub(super) fn shine_parts_metal(&mut self, f: FighterId) {
        self.shine_parts(f);
        self.metal_flag_on(f);
    }

    /// `x2225_b6` set, `ftCommon_80080460`.
    #[doc(alias = "ftCommon_80080460")]
    pub fn metal_flag_on(&mut self, f: FighterId) {
        self.game.fighters[f.0].x2225_b6 = true;
    }

    /// `x2225_b6` back to the player's rules' `x2225_b5`, `ftCommon_80080474`.
    #[doc(alias = "ftCommon_80080474")]
    pub fn metal_flag_off(&mut self, f: FighterId) {
        let fp = &mut self.game.fighters[f.0];
        fp.x2225_b6 = fp.x2225_b5;
    }

    /// The Metal Box taken, `ftLib_ApplyMetalBox`: metal for the box's frames and health
    /// (`it_8026B560`, `it_8026B574`), the attributes and the ledge snap recomputed, the
    /// rumble, a small quake and the sound.
    #[doc(alias = "ftLib_ApplyMetalBox")]
    pub fn apply_metal_box(&mut self, gobj: GObjId, box_gobj: GObjId) {
        use gekko_fp as fp;
        assert!(
            it::lib26b1::kind(self.game, box_gobj) == kinds::METALB,
            "itGetKind(item_gobj) == It_Kind_MetalB"
        );
        let health = fp::fctiwz(crate::fpu::reg(it::lib26b1::special_x4(
            self.game, self.data, box_gobj,
        )));
        let timer = fp::fctiwz(crate::fpu::reg(it::lib26b1::special_x0(
            self.game, self.data, box_gobj,
        )));
        self.set_metal(gobj, timer, health);
        self.calc_attributes(gobj);
        let f = self.game.fighter_id(gobj);
        let scale = self.game.fighters[f.0].x34_scale.y;
        self.scale_ledge_snap(gobj, scale);
        self.start_rumble(f, 0xC, 0);
        let pos = self.game.fighters[f.0].cur_pos;
        super::super::camera::request_quake(self, QUAKE_SMALL, Some(pos));
        self.fighter_play_sfx(f, 0x121, 0x7F, 0x40);
    }

    /// The metal's end, `ftCo_800C8540`: unless the player is always metal, the attributes, the
    /// ledge snap and the parts' look back, and `x2225_b6` as the player's rules have it
    /// (`ftCommon_80080474`).
    #[doc(alias = "ftCo_800C8540")]
    pub fn end_metal(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        fp.metal_timer = 0;
        fp.metal_health = 0;
        if fp.is_always_metal {
            return;
        }
        fp.is_metal = false;
        self.calc_attributes(gobj);
        let scale = self.game.fighters[f.0].x34_scale.y;
        self.scale_ledge_snap(gobj, scale);
        self.restore_parts_look(f);
        self.metal_flag_off(f);
    }
}

/// `QuakeKind_Small`.
const QUAKE_SMALL: i32 = 2;

/// The Cloaking Device's color animations: its start, and its flashing before the end.
const CLOAK_COLANIM: i32 = 0x7C;
const CLOAK_ON_COLANIM: i32 = 0x80;

impl Running<'_> {
    /// The Cloaking Device's state cleared, `ftCo_800C88A0`.
    #[doc(alias = "ftCo_800C88A0")]
    pub fn clear_cloak(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.x2226_b4 = false;
        fp.x2226_b5 = false;
        fp.smash_attrs.x2134_vibrateFrame = 0;
        fp.x2030 = 0;
        fp.x2226_b7 = false;
    }

    /// The cloak's vibration back to its first frame, `fn_800C8B1C`.
    #[doc(alias = "fn_800C8B1C")]
    pub fn reset_cloak_vibration(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).smash_attrs.x2134_vibrateFrame = 0;
    }

    /// Drawn through refraction, `ftCo_800C88D4`'s inline: the vibration reset, the parts'
    /// shiny look, one more refraction user, and `x221F_b2`.
    pub fn start_cloak_look(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        fp.x2226_b5 = true;
        fp.x2226_b7 = false;
        self.reset_cloak_vibration(gobj);
        self.shine_parts_for_cloak(f);
        self.game.refract_use();
        self.game.fighters[f.0].x221F_b2 = true;
    }

    /// Cloaked for `frames` frames, `ftCo_800C88D4`: unless it already is and isn't flashing
    /// toward the end, the cloak's look, and its color animation when `flash` (else its
    /// overlay reset).
    #[doc(alias = "ftCo_800C88D4")]
    pub fn start_cloak(&mut self, gobj: GObjId, frames: i32, flash: bool) {
        let fp = self.game.fighter_mut(gobj);
        fp.x2030 = frames;
        if fp.x2226_b4 && !fp.x2226_b7 {
            return;
        }
        fp.x2226_b4 = true;
        self.start_cloak_look(gobj);
        if flash {
            self.start_color_anim_unless_held(gobj, CLOAK_COLANIM);
        } else {
            self.reset_cloak_overlay(gobj);
        }
    }

    /// The cloak's overlay reset, `ft_800C0098`: cleared, then its color animation while
    /// cloaked.
    #[doc(alias = "ft_800C0098")]
    pub fn reset_cloak_overlay(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        crate::lb::colanim::clear(&mut fp.x508);
        if fp.x2226_b4 {
            self.start_color_anim(gobj, CLOAK_ON_COLANIM, 0);
        }
    }

    /// The cloak's end, `ftCo_800C8A64`: unless the player's rules keep it, its state and
    /// overlay cleared, the parts' look back, one refraction user fewer.
    #[doc(alias = "ftCo_800C8A64")]
    pub fn end_cloak(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        fp.x2030 = 0;
        if fp.x2226_b3 {
            return;
        }
        self.clear_cloak(gobj);
        self.reset_cloak_overlay(gobj);
        self.restore_parts_look(f);
        self.game.refract_unuse();
        self.game.fighters[f.0].x221F_b2 = false;
    }
}

impl Running<'_> {
    /// The character's model for something worn on the head (`_func_8007F948_inline`,
    /// `ftData_UnkMotionStates1`): Pikachu's ears hidden (`ftPk_Init_UnkMotionStates1`),
    /// unless it already wears both a Bunny Hood and a flower.
    pub(super) fn worn_item_on(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        if (fp.x197C.is_none() || fp.x1980.is_none()) && fp.kind == super::pikachu::KIND_PIKACHU {
            let f = self.game.fighter_id(gobj);
            self.game.set_part_vis(gobj, 1, -1);
            self.set_model_variant(f, 1, 0);
        }
    }

    /// The character's model back as something worn comes off, `ftCommon_8007F8E8`
    /// (`ftData_UnkMotionStates2`): Pikachu's ears shown (`ftPk_Init_UnkMotionStates2`),
    /// unless it wears both a Bunny Hood and a flower.
    #[doc(alias = "ftCommon_8007F8E8")]
    pub fn worn_item_off(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        if (fp.x197C.is_none() || fp.x1980.is_none()) && fp.kind == super::pikachu::KIND_PIKACHU {
            let f = self.game.fighter_id(gobj);
            self.game.set_part_vis(gobj, 1, 0);
            self.set_model_variant(f, 1, 0);
        }
    }

    /// Wears the Bunny Hood `hood` for `frames` frames, `ftCommon_8007F948`.
    #[doc(alias = "ftCommon_8007F948")]
    pub fn wear_hood(&mut self, gobj: GObjId, hood: GObjId, frames: i32) {
        let fp = self.game.fighter_mut(gobj);
        fp.x197C = Some(hood);
        fp.x2014 = frames;
        self.worn_item_on(gobj);
    }

    /// The Bunny Hood taken off, `ftCommon_8007F9B4`: the attributes and the ledge snap back.
    #[doc(alias = "ftCommon_8007F9B4")]
    pub fn take_off_hood(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).x197C = None;
        self.calc_attributes(gobj);
        let scale = self.game.fighter(gobj).x34_scale.y;
        self.scale_ledge_snap(gobj, scale);
    }

    /// The hood fitted to the fighter's head, `ftCommon_8007FA00`: its parts moved by the
    /// fighter's attributes (`x120`, `x114`) and its ears scaled by its height times `x12C`.
    #[doc(alias = "ftCommon_8007FA00")]
    pub fn fit_hood(&mut self, gobj: GObjId) {
        use gekko_fp as fp;
        let f = self.game.fighter(gobj);
        let hood = f.x197C.expect("no Bunny Hood worn");
        let (a, b) = (f.co_attrs.x120, f.co_attrs.x114);
        let scale = crate::fpu::mem(fp::fmuls(
            crate::fpu::reg(f.x34_scale.y),
            crate::fpu::reg(f.co_attrs.x12C),
        ));
        kinds::rabbitc::shift_parts(self, hood, a, b);
        kinds::rabbitc::scale_ears(self, hood, scale);
    }

    /// Puts the Bunny Hood `hood` on, `ftCommon_8007FA58`: one already worn only takes the
    /// new one's frames (the new one gone); else it goes from the hand to the head, quiet,
    /// for its frames (`it_8026B54C`), the attributes and the ledge snap recomputed, then
    /// fitted; the sound and the rumble either way.
    #[doc(alias = "ftCommon_8007FA58")]
    pub fn put_on_hood(&mut self, gobj: GObjId, hood: GObjId) {
        use gekko_fp as fp;
        let f = self.game.fighter_id(gobj);
        let frames = fp::fctiwz(crate::fpu::reg(it::lib26b1::special_x0_hood(
            self.game, self.data, hood,
        )));
        if self.game.fighters[f.0].x197C.is_some() {
            self.game.fighters[f.0].x2014 = frames;
            self.fighter_play_sfx(f, 0x117, 0x7F, 0x40);
            self.start_rumble(f, 0x10, 0);
            it::item::destroy(self, hood);
            return;
        }
        it::lib26b1::silence(self.game, hood);
        it::release::drop(self, hood, crate::vec::Vec3::new(0.0, 0.0, 0.0), 0.0);
        let head = i32::from(
            self.data
                .u8(self.ft_data_x8(&self.game.fighters[f.0]).at(0x12)),
        );
        it::item::pick_up(self, hood, gobj, head);
        self.wear_hood(gobj, hood, frames);
        self.calc_attributes(gobj);
        let scale = self.game.fighters[f.0].x34_scale.y;
        self.scale_ledge_snap(gobj, scale);
        self.fighter_play_sfx(f, 0x117, 0x7F, 0x40);
        self.start_rumble(f, 0x10, 0);
        it::lib26b1::unsilence(self.game, hood);
        self.fit_hood(gobj);
    }
}

impl Running<'_> {
    /// The attributes at the fighter's height scale (a mushroom's): `ftCo_800CF6E8` on each
    /// attribute with its weight in `PlCo.dat`'s scale parameters (`Fighter_804D6524`), then
    /// the item pickup offsets and `x2C4` times the scale (`ftCo_800D105C`'s first branch).
    #[doc(alias = "ftCo_800CF6E8")]
    pub(super) fn scale_attributes(&mut self, f: FighterId) {
        use crate::fpu::{mem, reg};
        use gekko_fp as fp;
        let t = super::common_ptr(self.game, self.data, super::SCALE_PARAMS)
            .expect("no fighter scale parameters");
        let data = self.data;
        let k = |o: u32| reg(data.f32(t.at(o)));
        let fpr = &mut self.game.fighters[f.0];
        let scale = reg(fpr.x34_scale.y);
        let a = &mut fpr.co_attrs;
        if scale != 1.0 {
            let y = |v: &mut f32, o: u32| *v = mem(it::owner::y_scaled(reg(*v), scale, k(o)));
            y(&mut a.slow_walk_max, 0x10);
            y(&mut a.mid_walk_point, 0x14);
            y(&mut a.fast_walk_min, 0x18);
            y(&mut a.dash_max_velocity, 0x1C);
            y(&mut a.run_animation_scaling, 0x20);
            y(&mut a.jump_startup_time, 0x24);
            y(&mut a.jump_v_initial_velocity, 0x28);
            y(&mut a.hop_v_initial_velocity, 0x2C);
            y(&mut a.gravity, 0x30);
            y(&mut a.terminal_velocity, 0x34);
            y(&mut a.air_drift_stick_mul, 0x38);
            y(&mut a.aerial_drift_base, 0x3C);
            y(&mut a.air_drift_max, 0x40);
            y(&mut a.fast_fall_velocity, 0x44);
            y(&mut a.weight, 0x48);
            y(&mut a.shield_break_initial_velocity, 0x4C);
            y(&mut a.ledge_jump_horizontal_velocity, 0x50);
            y(&mut a.ledge_jump_vertical_velocity, 0x54);
            y(&mut a.item_throw_velocity_multiplier, 0x58);
            y(&mut a.heavy_throw_velocity_multiplier, 0x5C);
            y(&mut a.xBC.size, 0x60);
            y(&mut a.xDC, 0x64);
            y(&mut a.kirby_b_star_damage, 0x68);
            y(&mut a.normal_landing_lag, 0x6C);
            y(&mut a.landingairn_lag, 0x70);
            y(&mut a.landingairf_lag, 0x74);
            y(&mut a.landingairb_lag, 0x78);
            y(&mut a.landingairhi_lag, 0x7C);
            y(&mut a.landingairlw_lag, 0x80);
            y(&mut a.name_tag_height, 0x84);
            y(&mut a.screw_attack_launch_velocity, 0x88);
            y(&mut a.damageicejump_vel_y, 0x8C);
            y(&mut a.damageicejump_vel_x_mult, 0x90);
            y(&mut a.x170.x, 0x94);
            y(&mut a.x170.y, 0x94);
            y(&mut a.x170.z, 0x94);
            y(&mut a.x17C, 0x98);
        }
        let s = scale;
        let mul = |v: &mut f32| *v = mem(fp::fmuls(reg(*v), s));
        let p = &mut fpr.x294_itPickup;
        for q in [
            &mut p.gr_light_offset,
            &mut p.gr_heavy_offset,
            &mut p.air_light_offset,
        ] {
            mul(&mut q.x);
            mul(&mut q.y);
            mul(&mut q.z);
            mul(&mut q.w);
        }
        mul(&mut fpr.x2C4.x);
        mul(&mut fpr.x2C4.y);
    }
}
