//! The fighters' shadows (`melee/lb/lbshadow.c`): each frame, before the game camera draws,
//! every fighter's shadow camera is aimed down the stage's shadow light at the fighter's
//! camera box, fitted around it, and renders the fighter's joints into the shadow's texture.
//! What that changes is state: the shadows' cameras and their textures' projections, which
//! joints each shadow casts from, whether it's on, and the joints the rendering brings up
//! to date.

use gekko_fp as fp;

use crate::fpu::{mem, reg};
use crate::game::state::Running;
use crate::hsd::display::Draw;
use crate::hsd::gobj::GObjId;
use crate::hsd::lobj::{self, LObjId};
use crate::hsd::shadow::{self, ViewingRect};
use crate::vec::Vec3;

/// `HSD_GOBJ_PLINK_FIGHTER`.
const P_LINK_FIGHTER: u8 = 8;
/// The render link of the fighters' light GObj (`ftCo_8009F54C`'s).
const FIGHTER_LIGHT_LINK: usize = 4;
/// A light the shadows take as theirs (`HSD_LObjGetFlags` bit 0x400).
const SHADOW_LIGHT: u16 = 0x400;

impl Running<'_> {
    /// The first fighter GObj; walks over the fighters read each one's `next` after it (the
    /// rendering changes no links).
    fn first_fighter_gobj(&self) -> Option<GObjId> {
        self.game.world.first_in(P_LINK_FIGHTER)
    }

    /// The light the shadows come from: among the fighters' light GObjs' lights, the first
    /// marked as the shadows', else the last that isn't ambient.
    fn shadow_light(&self) -> LObjId {
        let w = &self.game.world;
        let ls = &self.game.objs.lobjs;
        let mut fallback = None;
        let mut found = None;
        let mut g = w.gx_head[FIGHTER_LIGHT_LINK];
        'gobjs: while let Some(lg) = g {
            let mut l = match w.gobj(lg).obj {
                Some(crate::game::state::Obj::LObj(l)) => Some(l),
                _ => None,
            };
            while let Some(x) = l {
                if ls[x].flags & 3 != 0 {
                    fallback = Some(x);
                }
                if ls[x].flags & SHADOW_LIGHT != 0 {
                    found = Some(x);
                    break 'gobjs;
                }
                l = ls[x].next;
            }
            g = w.gobj(lg).next_gx;
        }
        found.or(fallback).expect("lobj")
    }

    /// Renders the fighters' shadows, `lbShadow_8000F38C`; `draw_volume` also draws each
    /// shadow camera's volume into its texture (the camera's debug flag).
    #[doc(alias = "lbShadow_8000F38C")]
    pub fn shadow_render(&mut self, draw_volume: bool) {
        let mut next = self.first_fighter_gobj();
        while let Some(g) = next {
            next = self.game.world.gobj(g).next;
            self.game.fighter_mut(g).x20A4.x0_b2 = false;
        }
        let l = self.shadow_light();
        let mut light_pos = lobj::position(&mut self.game.objs, self.data, Some(l))
            .expect("coudn t get light position ...");
        let mut light_dir = Vec3::ZERO;
        if let Some(i) = lobj::interest(&mut self.game.objs, self.data, Some(l)) {
            light_pos = light_pos - i;
        }
        let v = light_pos - light_dir;
        let sq = |a: f32| fp::fmuls(reg(a), reg(a));
        let dist = mem(fp::fadds(sq(v.z), fp::fadds(sq(v.x), sq(v.y))));
        let dist = crate::math::sqrt(dist);
        let no_light = reg(dist) < reg(0.001);
        let mut up;
        if no_light {
            light_pos = Vec3::new(0.0, 0.0, 1.0);
            light_dir = Vec3::ZERO;
            up = Vec3::new(0.0, 1.0, 0.0);
        } else if fp::fadds(sq(v.x), sq(v.z)) > reg(0.000_001_000_000_1) {
            up = Vec3::new(0.0, 1.0, 0.0);
            let mut n = light_dir - light_pos;
            if reg(n.normalize()) < reg(100.0) {
                let f = |n: f32, d: f32| mem(fp::fnmsubs(reg(100.0), reg(n), reg(d)));
                light_pos = Vec3::new(
                    f(n.x, light_dir.x),
                    f(n.y, light_dir.y),
                    f(n.z, light_dir.z),
                );
            }
            let right = up.cross_normalized(n);
            up = n.cross_normalized(right);
        } else {
            up = Vec3::new(0.0, 0.0, -1.0);
        }
        self.show_main_models();
        let mut next = self.first_fighter_gobj();
        while let Some(g) = next {
            next = self.game.world.gobj(g).next;
            let f = self.game.fighter_id(g);
            self.game.fighters[f.0].x20A4.x0_b0 = no_light;
            if let Some(s) = self.game.fighters[f.0].x20A4.shadow {
                shadow::delete_object(&mut self.game.objs, Some(s), None);
                let fp = &self.game.fighters[f.0];
                let mut added = false;
                if fp.x21FC_flag[0] & 1 != 0 {
                    let (shown, accessory) = (
                        !fp.invisible && !fp.x221E_b5 && fp.x5AC.xC[1].is_some(),
                        fp.x20A0_accessory,
                    );
                    if shown {
                        let root = match self.game.world.gobj(g).obj {
                            Some(crate::game::state::Obj::JObj(j)) => Some(j),
                            _ => None,
                        };
                        shadow::add_object(&mut self.game.objs, Some(s), root);
                        added = true;
                    }
                    if accessory.is_some() {
                        shadow::add_object(&mut self.game.objs, Some(s), accessory);
                        added = true;
                    }
                }
                self.game.fighters[f.0].x20A4.x0_b3 = !added;
            }
            let fp = &self.game.fighters[f.0];
            let ls = &fp.x20A4;
            let any = ls.x0_b0 || ls.x0_b1 || ls.x0_b2 || ls.x0_b3 || ls.x0_b4 || ls.x0_b5;
            let casts = !fp.x221E_b1 && !fp.x2226_b4 && !fp.is_sub_fighter;
            let s = ls.shadow.expect("a fighter without a shadow");
            shadow::set_active(&mut self.game.objs, s, !any && casts);
            let fp = &self.game.fighters[f.0];
            let (Some(cm), false) = (fp.x890_cameraBox, fp.x20A4.x0_b3) else {
                continue;
            };
            let intensity = self.game.gr.param.as_ref().map_or(128, |p| i32::from(p.x4)) as u8;
            self.game.objs.shadows[s.0].intensity = intensity;
            let b = &self.game.camera.boxes[cm];
            let (bone, ext) = (b.bone_pos, b.target_ext.v.z);
            let eye = bone.sdk_add(light_pos);
            let interest = bone.sdk_add(light_dir);
            let c = self.game.objs.shadows[s.0].camera.expect("a shadow camera");
            crate::hsd::cobj::set_eye_position(&mut self.game.objs, c, eye);
            crate::hsd::cobj::set_interest(&mut self.game.objs, c, interest);
            crate::hsd::cobj::set_up_vector(&mut self.game.objs, self.data, Some(c), up);
            let mut rect = ViewingRect::new(eye, interest, up, false);
            let mut fitted = false;
            for _ in 0..0x14 {
                let top = mem(fp::fmuls(reg(1.2), reg(ext)));
                let bot = mem(fp::fmuls(reg(1.2), fp::fneg(reg(ext))));
                rect.add_rect(bone, top, bot, bot, top);
                if rect.check() {
                    fitted = true;
                    break;
                }
            }
            if fitted {
                let r = [rect.top, rect.bottom, rect.left, rect.right];
                shadow::set_viewing_rect(&mut self.game.objs, self.data, s, r);
            } else {
                crate::hsd::cobj::set_ortho(
                    &mut self.game.objs.cobjs,
                    Some(c),
                    128.0,
                    -128.0,
                    -128.0,
                    128.0,
                );
            }
            crate::hsd::display::set_offscreen(&mut self.game.objs, true);
            self.game.world.rendering = Some(g);
            let mut hooks = crate::game::code::ParticleHooks;
            shadow::start_render(&mut self.game.objs, self.data, &mut hooks, s);
            if draw_volume {
                self.game.objs.display.draws.push(Draw::ShadowVolume(s));
            }
            shadow::end_render(&mut self.game.objs, s);
        }
        self.show_low_models();
    }
}
