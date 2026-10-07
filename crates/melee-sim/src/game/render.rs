//! The display pass's game side: each frame, after its engine ticks, the scene loop runs the
//! render callbacks of the cameras on the max render link (`HSD_GObj_80390FC0`); a camera's
//! callback runs those of the render links its GObj names, pass by pass
//! (`HSD_GObj_80390ED0`). The callbacks reach the joint traversal and the particle drawing
//! ([`crate::hsd::display`]), which append the frame's draw commands. A callback no port covers
//! yet stops the simulation ([`code::run_render`]).

use super::code;
use super::handles::CodeRef;
use super::state::{Obj, Running};
use crate::hsd::display;
use crate::hsd::gobj::{GObjId, GX_LINKS};

/// HSD's generic render callbacks.
pub const LOBJ_CALLBACK: u32 = 0x8039_1044;
pub const JOBJ_CALLBACK: u32 = 0x8039_1070;
pub const FOG_CALLBACK: u32 = 0x8039_10B4;
pub const CAMERA_CALLBACK: u32 = 0x8039_10D8;

impl Running<'_> {
    /// Runs the render callbacks of the max render link's GObjs (the cameras) in link order,
    /// for pass 0, `HSD_GObj_80390FC0`.
    #[doc(alias = "HSD_GObj_80390FC0")]
    pub fn render_frame(&mut self) {
        let mut cur = self.game.world.gx_head[GX_LINKS];
        while let Some(g) = cur {
            if let Some(cb) = self.game.world.gobj(g).render {
                let saved = self.game.world.rendering_camera.replace(g);
                code::run_render(self, g, cb, 0);
                self.game.world.rendering_camera = saved;
            }
            cur = self.game.world.gobj(g).next_gx;
        }
    }

    /// For each pass `mask` names, runs the render callbacks of every render link the GObj's
    /// `gxlink_prios` names, `HSD_GObj_80390ED0`.
    #[doc(alias = "HSD_GObj_80390ED0")]
    pub fn render_links(&mut self, gobj: GObjId, mask: u32) {
        let mut mask = mask;
        let mut pass = 0;
        while mask != 0 {
            if mask & 1 != 0 {
                let mut prios = self.game.world.gobj(gobj).gxlink_prios;
                let mut link = 0;
                while prios != 0 {
                    if prios & 1 != 0 {
                        let mut cur = self.game.world.gx_head[link];
                        while let Some(c) = cur {
                            if let Some(cb) = self.game.world.gobj(c).render {
                                let saved = self.game.world.rendering.replace(c);
                                code::run_render(self, c, cb, pass);
                                self.game.world.rendering = saved;
                            }
                            cur = self.game.world.gobj(c).next_gx;
                        }
                    }
                    link += 1;
                    prios >>= 1;
                }
            }
            pass += 1;
            mask >>= 1;
        }
    }

    /// A joint tree GObj's render callback, `HSD_GObj_JObjCallback`: the tree for the pass's
    /// transparency.
    #[doc(alias = "HSD_GObj_JObjCallback")]
    pub fn render_jobj(&mut self, gobj: GObjId, pass: i32) {
        let j = match self.game.world.gobj(gobj).obj {
            Some(Obj::JObj(j)) => Some(j),
            None => None,
            other => panic!("HSD_GObj_JObjCallback on {other:?}"),
        };
        let trsp = display::pass_trsp(pass);
        let mut hooks = code::ParticleHooks;
        display::jobj_disp_all(&mut self.game.objs, self.data, &mut hooks, j, None, trsp, 0);
    }

    /// A camera GObj's render callback, `HSD_GObj_803910D8`: passes 0 to 2 of its links.
    #[doc(alias = "HSD_GObj_803910D8")]
    pub fn render_camera(&mut self, gobj: GObjId) {
        let c = match self.game.world.gobj(gobj).obj {
            Some(Obj::CObj(c)) => Some(c),
            None => None,
            other => panic!("HSD_GObj_803910D8 on {other:?}"),
        };
        if display::camera_begin(&mut self.game.objs, self.data, c) {
            self.render_links(gobj, 7);
            display::camera_end(&mut self.game.objs);
        }
    }

    /// A light GObj's render callback (`HSD_GObj_LObjCallback`; the stage's
    /// `Ground_801C4640` and the fighters' `ftCo_8009F54C` are the same).
    #[doc(alias = "Ground_801C4640")]
    #[doc(alias = "ftCo_8009F54C")]
    pub fn render_lights(&mut self, gobj: GObjId) {
        let l = match self.game.world.gobj(gobj).obj {
            Some(Obj::LObj(l)) => Some(l),
            None => None,
            other => panic!("a light callback on {other:?}"),
        };
        display::lobj_callback(&mut self.game.objs, self.data, l);
    }

    /// A fog GObj's render callback, `HSD_GObj_FogCallback`.
    #[doc(alias = "HSD_GObj_FogCallback")]
    pub fn render_fog(&mut self, gobj: GObjId) {
        let f = match self.game.world.gobj(gobj).obj {
            Some(Obj::Fog(f)) => Some(f),
            None => None,
            other => panic!("HSD_GObj_FogCallback on {other:?}"),
        };
        display::fog_set(&mut self.game.objs, f);
    }

    /// The stage's fog GObj's render callback, `Ground_801C1E2C`: its fog, unless the camera
    /// draws collisions or the stage keeps its fog off (`Ground_801C1E00`).
    #[doc(alias = "Ground_801C1E2C")]
    pub fn render_stage_fog(&mut self, gobj: GObjId) {
        if super::camera::draw_collisions(&self.game.camera.cam) || !self.game.stage_info.unk8C.b2 {
            return;
        }
        if let Some(Obj::Fog(f)) = self.game.world.gobj(gobj).obj {
            display::fog_set(&mut self.game.objs, Some(f));
        }
    }

    /// The particle GObjs' render callback, `efLib_render_callback`: the links it sorts and
    /// draws. Drawing a particle that has an app-SRT brings the app-SRT's matrices up to date
    /// once a display frame and freezes a one-shot one (`psDispSubAppSRT`), which no port
    /// covers yet.
    pub fn render_particles(&mut self, gobj: GObjId, pass: i32) {
        let links = super::ef::eflib::render(self.game, gobj, pass);
        let ps = &self.game.objs.particles;
        for &(_, first) in &links {
            if std::iter::successors(first, |&p| ps[p].next).any(|p| ps[p].appsrt.is_some()) {
                panic!("psDispSubAppSRT isn't ported");
            }
        }
        for (link, first) in links {
            self.game
                .objs
                .display
                .draws
                .push(display::Draw::Particles { link, first });
        }
    }
}

/// Whether `cb` is one of the render callbacks this module covers, which it runs.
pub fn run(r: &mut Running, gobj: GObjId, cb: CodeRef, pass: i32) -> bool {
    match cb.0 {
        JOBJ_CALLBACK => r.render_jobj(gobj, pass),
        CAMERA_CALLBACK => r.render_camera(gobj),
        LOBJ_CALLBACK | 0x801C_4640 | 0x8009_F54C => r.render_lights(gobj),
        FOG_CALLBACK => r.render_fog(gobj),
        0x801C_1E2C => r.render_stage_fog(gobj),
        super::ef::eflib::RENDER => r.render_particles(gobj, pass),
        _ => return false,
    }
    true
}
