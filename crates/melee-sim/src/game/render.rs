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
                                display::rendering(&mut self.game.objs, Some(c));
                                code::run_render(self, c, cb, pass);
                                self.game.world.rendering = saved;
                                display::rendering(&mut self.game.objs, saved);
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
        self.render_camera_links(gobj, 7);
    }

    /// A camera GObj's drawing: its camera current, the passes `mask` names over its links,
    /// and its end, as `HSD_GObj_803910D8` does for all three passes.
    fn render_camera_links(&mut self, gobj: GObjId, mask: u32) {
        let c = match self.game.world.gobj(gobj).obj {
            Some(Obj::CObj(c)) => Some(c),
            None => None,
            other => panic!("a camera callback on {other:?}"),
        };
        if display::camera_begin(&mut self.game.objs, self.data, c) {
            self.render_links(gobj, mask);
            display::camera_end(&mut self.game.objs);
        }
    }

    /// Sets the game camera's links and runs the passes `mask` names over them.
    fn camera_pass(&mut self, gobj: GObjId, prios: u64, mask: u32) {
        self.game.world.gobj_mut(gobj).gxlink_prios = prios;
        self.render_links(gobj, mask);
    }

    /// The game camera's render callback, `fn_800301D0`: refraction's projection, the camera
    /// brought up to date (`Camera_8002A4AC`), the fighters' shadows, then through the camera
    /// the render links layer by layer: the background and stage in layer 2, then layers 1
    /// and 0 (`render_gxlink_pass`), the screen copied for refraction, the fighters' passes
    /// (`Camera_800311EC`, `Camera_80031328`), fog off, the foreground, and layer 3. Each
    /// link set depends on which of the camera's layers are hidden (`x398`). The debug
    /// drawings (collisions, zones, special points) aren't modeled.
    #[doc(alias = "fn_800301D0")]
    pub fn render_game_camera(&mut self, gobj: GObjId) {
        use super::camera;
        let Some(Obj::CObj(c)) = self.game.world.gobj(gobj).obj else {
            panic!("the game camera GObj holds no CObj");
        };
        crate::lb::refract::set_camera(&self.game.objs, &mut self.game.refract, c);
        camera::update_cobj(self.game, gobj);
        let shadows_debug = self.game.camera.cam.x399_b7;
        self.shadow_render(shadows_debug);
        if !display::camera_begin(&mut self.game.objs, self.data, Some(c)) {
            return;
        }
        self.game.objs.display.draws.push(display::Draw::Erase);
        self.stage_fog_update(c);
        crate::hsd::lobj::delete_current_all(&mut self.game.objs, None);
        let cam = &self.game.camera.cam;
        let bit = |hidden: bool, v: u64| if hidden { 0 } else { v };
        let prio1 = bit(cam.x398_b4, 1);
        let prio8 = bit(cam.x398_b2, 8);
        let prio80 = bit(cam.x398_b3, 0x80);
        let b0 = bit(cam.x398_b0, 0x20);
        let b1 = bit(cam.x398_b1, 0x40);
        let b4 = bit(cam.x398_b4, 0x10);
        camera::set_layer(&mut self.game.camera.cam, 2);
        self.camera_pass(gobj, prio1 | prio8, 7);
        camera::set_layer(&mut self.game.camera.cam, 1);
        self.camera_pass(gobj, prio8, 7);
        camera::set_layer(&mut self.game.camera.cam, 0);
        self.camera_pass(gobj, prio8, 3);
        crate::lb::refract::copy_screen(&mut self.game.objs, &self.game.refract);
        // `Camera_800311EC`.
        camera::set_pass(&mut self.game.camera.cam, 1);
        self.camera_pass(gobj, prio1 | b0 | b1, 3);
        camera::set_pass(&mut self.game.camera.cam, 0);
        self.camera_pass(gobj, b1 | b4 | b0, 3);
        camera::set_pass(&mut self.game.camera.cam, 0);
        self.camera_pass(gobj, prio1 | prio8, 4);
        // `Camera_80031328`.
        camera::set_pass(&mut self.game.camera.cam, 0);
        self.camera_pass(gobj, prio1 | b4 | b0 | b1, 4);
        display::fog_set(&mut self.game.objs, None);
        self.camera_pass(gobj, prio80, 7);
        camera::set_layer(&mut self.game.camera.cam, 3);
        self.camera_pass(gobj, prio8, 7);
        let cam = &self.game.camera.cam;
        if camera::stage_visible(cam)
            && (camera::draw_collisions(cam) || cam.x399_b4 || camera::draw_special_points(cam))
        {
            crate::game::code::unported_call("mpLib_8005A2DC");
        }
        display::camera_end(&mut self.game.objs);
    }

    /// The stage's fog fitted to the camera, `Ground_801C4FAC`, for stages that set guide
    /// points (`stage_info.unk8C.b3`, [`set_guide_points`](super::gr::ground::set_guide_points)):
    /// a near and a far point, the side pair the camera faces (`x148`/`x154` looking left,
    /// `x160`/`x16C` otherwise), blended by heading with the front pair (`x130`/`x13C`) when it
    /// looks into the stage; the fog starts at the near point's distance from the eye (at
    /// least 5) and ends at the far one's (at least 10, and past the start).
    #[doc(alias = "Ground_801C4FAC")]
    pub fn stage_fog_update(&mut self, c: crate::hsd::cobj::CObjId) {
        use crate::fpu::{mem, reg};
        use crate::hsd::cobj;
        use gekko_fp as fp;
        let i = &self.game.stage_info;
        if !i.unk8C.b3 {
            return;
        }
        let eye = cobj::eye_vector(&mut self.game.objs, self.data, Some(c))
            .unwrap_or(cobj::NO_EYE_VECTOR);
        let i = &self.game.stage_info;
        let (mut front_near, mut front_far) = (i.x130, i.x13C);
        let (mut near, mut far) = if reg(eye.x) < 0.0 {
            (i.x148, i.x154)
        } else {
            (i.x160, i.x16C)
        };
        let (near, far) = if reg(eye.z) < 0.0 {
            let len = crate::math::sqrt_reg(fp::fmadds(
                reg(eye.x),
                reg(eye.x),
                fp::fmuls(reg(eye.z), reg(eye.z)),
            ));
            let inv = fp::fdivs(1.0, len);
            let abs = |v: f64| if v < 0.0 { fp::fneg(v) } else { v };
            let xw = fp::fmuls(inv, abs(reg(eye.x)));
            let zw = fp::fmuls(inv, abs(reg(eye.z)));
            let scale = |v: &mut crate::vec::Vec3, w: f64| {
                v.x = mem(fp::fmuls(reg(v.x), w));
                v.y = mem(fp::fmuls(reg(v.y), w));
                v.z = mem(fp::fmuls(reg(v.z), w));
            };
            scale(&mut near, xw);
            scale(&mut far, xw);
            scale(&mut front_near, zw);
            scale(&mut front_far, zw);
            (front_near.sdk_add(near), front_far.sdk_add(far))
        } else {
            (near, far)
        };
        let at = cobj::eye_position(&mut self.game.objs, self.data, c)
            .expect("HSD_CObjGetEyePosition: a CObj without an eye WObj");
        let Some(fog_gobj) = self.game.stage_info.x12C else {
            return;
        };
        let Some(Obj::Fog(fog)) = self.game.world.gobjs[fog_gobj.0].obj else {
            return;
        };
        let sq = |v: f64| fp::fmuls(v, v);
        let (dx, dy, dz) = (
            fp::fsubs(reg(near.x), reg(at.x)),
            fp::fsubs(reg(near.y), reg(at.y)),
            fp::fsubs(reg(near.z), reg(at.z)),
        );
        let start = crate::math::sqrt_reg(fp::fadds(sq(dz), fp::fadds(sq(dx), sq(dy))));
        let (dx, dy, dz) = (
            fp::fsubs(reg(far.x), reg(at.x)),
            fp::fsubs(reg(far.y), reg(at.y)),
            fp::fsubs(reg(far.z), reg(at.z)),
        );
        let mut end = crate::math::sqrt_reg(fp::fadds(sq(dz), fp::fadds(sq(dx), sq(dy))));
        let mut begin = start;
        if end < 10.0 {
            end = 10.0;
        }
        if start < 5.0 {
            begin = 5.0;
        }
        if begin > end {
            end = fp::fadds(1.0, begin);
        }
        let f = &mut self.game.objs.fogs[fog];
        f.start = mem(begin);
        f.end = mem(end);
    }

    /// Does what a HUD render callback decided.
    fn render_hud(&mut self, d: Option<super::hud::Draw>) {
        match d {
            Some(super::hud::Draw::JObjs(g, pass)) => self.render_jobj(g, pass),
            Some(super::hud::Draw::Camera(g, mask)) => self.render_camera_links(g, mask),
            None => {}
        }
    }

    /// The debug text's drawing GObj, `DevText_DrawAll`: its camera made current, fog off,
    /// and the screens' text and rectangles handed on as draw commands.
    pub fn render_devtext(&mut self, pass: i32) {
        use super::hud::devtext::Draw as Text;
        let mut drawn = std::mem::take(&mut self.game.objs.display.devtext);
        drawn.clear();
        self.game.devtext_draw_all_into(self.data, pass, &mut drawn);
        for d in drawn.drain(..) {
            match d {
                Text::Camera(c) => {
                    display::camera_begin(&mut self.game.objs, self.data, Some(c));
                }
                Text::NoFog => display::fog_set(&mut self.game.objs, None),
                other => self
                    .game
                    .objs
                    .display
                    .draws
                    .push(display::Draw::Text(Box::new(other))),
            }
        }
        self.game.objs.display.devtext = drawn;
    }

    /// A text GObj's render callback, `HSD_SisLib_803A84BC`: in the third pass, a shown text
    /// with a string takes the current camera's viewing matrix, runs its own render
    /// callback, then is laid out and typed ([`crate::hsd::sis::SisLib::layout_text`]).
    pub fn render_text(&mut self, gobj: GObjId, pass: i32) {
        if pass != 2 {
            return;
        }
        let Some(super::state::UserData::Text(t)) = self.game.world.gobj(gobj).user_data else {
            panic!("a text render callback on a GObj without its text");
        };
        let tx = self.game.sis.text(t);
        if tx.hidden != 0 || tx.sis_buffer.is_none() {
            return;
        }
        let callback = tx.render_callback;
        let c = self
            .game
            .objs
            .cobjs
            .current
            .expect("HSD_SisLib_803A84BC: no current camera");
        crate::hsd::cobj::viewing_mtx_copy(&mut self.game.objs, self.data, c);
        if let Some(cb) = callback {
            code::unported_call(&format!("the text render callback {cb:#010x}"));
        }
        let d = self.game.sis.layout_text(self.data, t);
        self.game
            .objs
            .display
            .draws
            .push(display::Draw::SisText(Box::new(d)));
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
    /// draws, the app-SRTs of the particles drawn brought up to date.
    pub fn render_particles(&mut self, gobj: GObjId, pass: i32) {
        let links = super::ef::eflib::render(self.game, gobj, pass);
        if let 1 | 2 = pass {
            crate::hsd::psdisp::setup_app_srts(&mut self.game.objs, self.data, &links, pass as u32);
        }
        for &(link, first) in links.iter() {
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
        crate::lb::cardgame::INDICATOR_RENDER => r.card_indicator_render(gobj),
        crate::lb::mthp::RENDER => r.render_movie(gobj),
        crate::hsd::sobj::RENDER => r.render_sobjs(gobj),
        crate::hsd::sobj::CAMERA_RENDER => r.render_sprite_camera(gobj),
        super::gm::title::CAMERA_RENDER => r.title_camera_render(gobj),
        super::gm::title::ERASE_RENDER => r.title_erase_render(gobj),
        super::mn::main::FOG_RENDER => r.menu_fog_render(gobj),
        super::mn::charsel::FOG_RENDER => r.charsel_fog_render(gobj),
        super::mn::stagesel::FOG_RENDER => r.sss_fog_render(gobj),
        super::mn::main::ERASE_RENDER => r.menu_erase_render(gobj),
        super::mn::gallery::CAMERA_RENDER => r.gallery_camera_render(gobj),
        LOBJ_CALLBACK | 0x801C_4640 | 0x8009_F54C => r.render_lights(gobj),
        FOG_CALLBACK => r.render_fog(gobj),
        0x801C_1E2C => r.render_stage_fog(gobj),
        super::ef::eflib::RENDER => r.render_particles(gobj, pass),
        0x8003_01D0 => r.render_game_camera(gobj),
        0x801C_5DB0 => r.render_stage_object(gobj, pass),
        super::gr::display::STAGE_CAMERA_RENDER => r.render_stage_camera(gobj),
        c if super::gr::stadium::run_render(r, gobj, c, pass) => {}
        c if super::gr::izumi::run_render(r, gobj, c, pass) => {}
        c if super::gr::fourside::run_render(r, gobj, c, pass) => {}
        c if super::gr::pura::run_render(r, gobj, c, pass) => {}
        c if super::gr::rcruise::run_render(r, gobj, c, pass) => {}
        super::gr::corneria::WINDOW_RENDER => super::gr::corneria::window_render(r, gobj, pass),
        c if super::gr::castle::run_render(r, gobj, c, pass) => {}
        c if super::gr::bigblue::run_render(r, gobj, c, pass) => {}
        0x8008_0E18 => r.render_fighter(gobj, pass),
        0x8008_0C28 => r.draw_magnified(gobj, pass),
        0x8026_EECC => r.render_item(gobj, pass),
        super::it::link::RENDER => r.render_link(gobj, pass),
        super::it::kinds::foxillusion::RENDER => r.render_fox_illusion(gobj, pass),
        super::it::kinds::linkboomerang::RENDER => r.render_link_boomerang(gobj, pass),
        super::it::kinds::linkarrow::RENDER => r.render_link_arrow(gobj, pass),
        super::it::kinds::sword::BAT_RENDER => r.render_bat(gobj, pass),
        crate::lb::bgflash::RENDER => r.render_bg_flash(),
        crate::hsd::sis::TEXT_RENDER => r.render_text(gobj, pass),
        super::hud::magnify::CAMERA_RENDER => r.render_magnifier(gobj),
        super::hud::magnify::BUBBLE_RENDER => r.render_bubble(gobj, pass),
        // The HUD's.
        0x802F_36B8 => {
            let d = r.game.hud_camera_render(gobj);
            r.render_hud(d);
        }
        0x802F_CAC4 => {
            let d = r.nametag_camera_render(gobj);
            r.render_hud(d);
        }
        0x802F_CAA4 => {
            let d = r.game.nametag_render(gobj, pass);
            r.render_hud(d);
        }
        0x802F_770C => {
            let d = r.game.ko_mark_render(gobj, pass);
            r.render_hud(d);
        }
        0x802F_D6CC => {
            let d = r.game.hazard_render(gobj, pass);
            r.render_hud(d);
        }
        0x802F_DA4C => {
            let d = r.game.arrow_render(gobj, pass);
            r.render_hud(d);
        }
        0x802F_5DE0 => {
            let d = r.game.damage_digits_render(gobj, pass);
            r.render_hud(d);
        }
        0x802F_5E50 => {
            let d = r.game.damage_mark_render(gobj, pass);
            r.render_hud(d);
        }
        0x802F_94E0 => {
            let d = r.game.stock_icons_render(gobj, pass);
            r.render_hud(d);
        }
        0x802F_9548 => {
            let d = r.game.target_icons_render(gobj, pass);
            r.render_hud(d);
        }
        super::hud::devtext::DRAW_ALL => r.render_devtext(pass),
        _ => return false,
    }
    true
}
