//! A frame's GX command stream: the scene loop's frame (`gm_801A4B60`'s render: the caches
//! invalidated, `HSD_StartRender`, the display pass, `HSD_VICopyXFBAsync`) with each of the
//! display pass's draws turned into what the original sends GX for it.

use melee_data::archive::Archives;
use melee_sim::game::state::Game;
use melee_sim::hsd::display::Draw;

use crate::blend::Blend;
use crate::gx::enums::*;
use crate::gx::{Color, Gx};
use crate::hsd::Hsd;
use crate::hsd::draw::Ctx;
use crate::mem::Space;

/// The XFB the frame is copied to. Its address is the presentation's.
pub const XFB_ADDR: u32 = 0x00F0_0000;

/// The aspect ratio the game's 640 by 480 picture is made for: 73:60 (584 square pixels by
/// 480), the width the console's picture fills on a 4:3 screen and Slippi Dolphin's "73:60"
/// option. The game camera's perspective aspect, 1.2173333 (`lb_80013B14`), is the same
/// within 0.06%: 584.32 pixels by 480.
pub const GAME_ASPECT: f32 = 73.0 / 60.0;

/// What the frame's stream starts from and keeps from frame to frame: GX's state and HSD's.
pub struct Frames {
    pub gx: Gx,
    pub hsd: Hsd,
    pub space: Space,
    /// How many rewritten display lists the address space knows.
    patched: usize,
    /// Frames built so far.
    pub built: u32,
}

impl Default for Frames {
    fn default() -> Self {
        Self::new()
    }
}

impl Frames {
    /// GX as the game sets it up at boot: `GXInit`, then the video setup.
    pub fn new() -> Frames {
        let mut gx = Gx::new();
        gx.init();
        let mut hsd = Hsd::default();
        // `HSD_GXInit`: every light slot loaded with a black light.
        let mut light = crate::gx::LightObj::default();
        gx.init_light_pos(&mut light, 1.0, 0.0, 0.0);
        gx.init_light_dir(&mut light, 1.0, 0.0, 0.0);
        gx.init_light_attn(&mut light, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0);
        gx.init_light_color(&mut light, Color::default());
        for i in 0..8 {
            gx.load_light_obj_imm(&light, 1 << i);
        }
        crate::hsd::state::States {
            state: &mut hsd.state,
            tev: &mut hsd.tev,
        }
        .invalidate(crate::hsd::state::STATE_ALL);
        // The game's `main`: flush primitives of eight vertices after XF changes.
        gx.set_misc(GX_MT_XF_FLUSH, 8);
        Frames {
            gx,
            hsd,
            space: Space::default(),
            patched: usize::MAX,
            built: 0,
        }
    }

    /// The stream for one frame's draws, the copy to the XFB at its end.
    pub fn frame(&mut self, game: &Game, data: &Archives, draws: &[Draw]) -> Vec<u8> {
        self.frame_blended(game, data, draws, None)
    }

    /// The stream for a frame between two ticks: `draws` as `game` was left by the latest
    /// tick's display pass (B), its transforms blended toward B from those of the tick before,
    /// `blend.from` (A), by `blend.t` ([`crate::blend`]). Without a blend, B as it is: the
    /// frame [`Frames::frame`] draws.
    pub fn frame_blended(
        &mut self,
        game: &Game,
        data: &Archives,
        draws: &[Draw],
        blend: Option<Blend<'_>>,
    ) -> Vec<u8> {
        let patches = &game.objs.pobjs.refract_patches;
        if self.patched != patches.len() {
            self.patched = patches.len();
            self.space.patches = game
                .objs
                .pobjs
                .pool
                .iter()
                .filter_map(|(_, p)| {
                    let list = p.display?;
                    let &(stride, pos, tex) = patches.get(&list)?;
                    Some(crate::mem::Patch {
                        list,
                        len: u32::from(p.n_display) << 5,
                        stride,
                        pos,
                        tex,
                    })
                })
                .collect();
            self.space.patches.sort_by_key(|p| (p.list, p.len));
            self.space.patches.dedup();
        }
        self.built += 1;
        self.hsd.debug_dl = std::env::var("MELEE_RENDER_DL")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            == Some(self.built - 1);
        let blend = blend.filter(|b| b.from.is_filled());
        if blend.is_some() {
            self.hsd.motion.start();
        }
        let mut ctx = Ctx {
            gx: &mut self.gx,
            hsd: &mut self.hsd,
            game,
            data,
            space: &self.space,
            blend,
        };
        ctx.gx.invalidate_vtx_cache();
        ctx.gx.invalidate_tex_all();
        ctx.start_render(false);
        let mut i = 0;
        while i < draws.len() {
            i += ctx.draw(draws, i);
        }
        ctx.copy_xfb();
        self.gx.take_fifo()
    }
}

impl Ctx<'_> {
    /// `HSD_StartRender`: the pixel format and field mode for the pass.
    pub fn start_render(&mut self, offscreen: bool) {
        self.hsd.offscreen = offscreen;
        self.gx.set_pixel_fmt(GX_PF_RGB8_Z24, GX_ZC_LINEAR);
        self.gx.set_field_mode(GX_FALSE, GX_FALSE);
    }

    /// Draws `draws[i]` (with the shapes after it of the same joint and pass); returns how
    /// many it drew.
    fn draw(&mut self, draws: &[Draw], i: usize) -> usize {
        if self.hsd.debug_dl {
            eprintln!("CMD {i} {:?}", draws[i]);
        }
        match &draws[i] {
            Draw::Camera(c, view) => {
                let mut v = self.game.objs.display.cameras[usize::from(*view)];
                if let Some(b) = self.blend {
                    let m = &mut self.hsd.motion;
                    let n = m.next_camera(*c);
                    m.camera_b = Some(v);
                    m.camera_blends = crate::blend::camera_blends(self.game, *c, &v);
                    if m.camera_blends
                        && let Some(x) = b.camera(*c, n, &v)
                    {
                        v = x;
                    }
                }
                // Cameras that draw the whole screen (not a shadow's or a magnifier
                // bubble's, which draw into textures); the render pass can't tell, since
                // the frame stays in the shadows' offscreen pass, as the original's does.
                if v.viewport == [0.0, 640.0, 0.0, 480.0]
                    && let Some(aspect) = self.hsd.widescreen
                {
                    widen(&mut v, aspect);
                }
                self.camera_begin(&v);
                if self.hsd.offscreen
                    && let Some((s, _)) = self
                        .game
                        .objs
                        .shadows
                        .iter()
                        .find(|(_, s)| s.camera == Some(*c))
                {
                    self.shadow_start(melee_sim::hsd::shadow::ShadowId(s));
                }
                1
            }
            Draw::EndCamera => 1,
            Draw::Lights(n) => {
                let mut set = self.game.objs.display.light_sets[usize::from(*n)];
                // Lights set up in the current camera's view take its blended view.
                if self.blend.is_some()
                    && let (Some(b), Some(v)) = (self.hsd.motion.camera_b, self.hsd.camera)
                    && set.view_mtx == b.view_mtx
                {
                    set.view_mtx = v.view_mtx;
                }
                let objs = &self.game.objs;
                self.hsd.lights.setup_init(self.gx, objs, &set);
                1
            }
            Draw::Rendering(g) => {
                // A refracted fighter's callback ends each tree it draws by putting the TEV
                // back to direct and the caches out of date (`ftCo_800C8AF0`,
                // `lbRefract_800225D4`).
                if let Some(f) = self.hsd.rendering_fighter
                    && self.hsd.rendering != *g
                    && self.game.fighters[f.0].x2226_b5
                {
                    self.gx.set_tev_direct(0);
                    self.gx.set_num_ind_stages(0);
                    self.invalidate(crate::hsd::state::STATE_ALL);
                }
                if let Some(g) = *g {
                    self.particles_rendering(g);
                }
                self.hsd.rendering = *g;
                self.hsd.rendering_fighter = g.and_then(|g| {
                    let game = self.game;
                    game.fighters
                        .iter()
                        .find(|(_, f)| f.gobj == Some(g))
                        .map(|(i, _)| melee_sim::game::handles::FighterId(i))
                });
                1
            }
            Draw::Shadows { start, len } => {
                let start = usize::from(*start);
                let named = &self.game.objs.display.shadow_tobjs[start..start + usize::from(*len)];
                self.hsd.shadows.clear();
                self.hsd.shadows.extend_from_slice(named);
                1
            }
            Draw::AppSrt { srt, view, scale } => {
                let (mut view, mut scale) = (*view, *scale);
                if let Some(b) = self.blend {
                    let n = self.hsd.motion.next_app_srt(*srt);
                    if self.hsd.motion.camera_blends {
                        let idnum = self.game.objs.app_srts[*srt].idnum;
                        (view, scale) = b.app_srt(*srt, idnum, n, &view, scale);
                    }
                }
                self.hsd.app_srts.insert(*srt, (view, scale));
                1
            }
            Draw::SpecularInit { jobj, vmtx } => {
                self.specular_init(*jobj, vmtx.as_ref());
                1
            }
            Draw::Invalidate(mask) => {
                self.invalidate(*mask);
                1
            }
            Draw::Hilight(v) => {
                self.hsd.hilight = *v;
                1
            }
            Draw::Fog(f) => {
                self.fog_set(*f);
                1
            }
            Draw::PointSize(size, tex_offsets) => {
                self.hsd
                    .state
                    .set_point_size(self.gx, *size, u32::from(*tex_offsets));
                1
            }
            Draw::Shape {
                jobj,
                trsp,
                rendermode,
                vmtx,
                ..
            } => {
                let n = draws[i..]
                    .iter()
                    .position(|d| {
                        !matches!(d, Draw::Shape { jobj: j, trsp: t, rendermode: r, vmtx: v, .. }
                            if j == jobj && t == trsp && r == rendermode && v == vmtx)
                    })
                    .unwrap_or(draws.len() - i);
                self.joint_shapes(&draws[i..i + n]);
                n
            }
            Draw::EraseColor([r, g, b, a]) => {
                self.hsd.erase_color = Some(Color::new(*r, *g, *b, *a));
                1
            }
            Draw::Erase => {
                // The game camera erases color in its background color; a camera whose erase
                // color the display pass set (Fountain of Dreams' reflection, Pokémon
                // Stadium's text) color in that color.
                let full = self
                    .hsd
                    .camera
                    .is_some_and(|v| v.viewport == [0.0, 640.0, 0.0, 480.0]);
                let color = match self.hsd.erase_color.take() {
                    _ if full => self.erase_color(),
                    Some(c) => c,
                    None => {
                        self.hsd.note_once("an erase without an erase color");
                        Color::new(0, 0, 0, 0xFF)
                    }
                };
                self.erase_screen(color, false);
                1
            }
            Draw::MagnifierErase(slot) => {
                match self.magnifier_erase_color(*slot) {
                    Some(c) => self.erase_screen(c, true),
                    None => self.hsd.note_once("a magnifier bubble without a fighter"),
                }
                1
            }
            Draw::Offscreen(b) => {
                self.start_render(*b);
                1
            }
            Draw::ShadowCopy(s) => {
                self.shadow_copy(*s);
                1
            }
            Draw::Particles { .. } => self.particles(draws, i),
            Draw::MagnifierCopy(i) => {
                self.magnifier_copy(*i);
                1
            }
            Draw::ReflectionCopy => {
                self.reflection_copy();
                1
            }
            Draw::ScreenCopy {
                screen,
                x,
                y,
                clear,
            } => {
                self.screen_copy(*screen, *x, *y, *clear);
                1
            }
            Draw::SisText(d) => {
                self.sis_text(d);
                1
            }
            Draw::Quad(_) => {
                let quads: Vec<_> = draws[i..]
                    .iter()
                    .map_while(|d| match d {
                        Draw::Quad(q) => Some(*q),
                        _ => None,
                    })
                    .collect();
                self.quads(&quads);
                quads.len()
            }
            Draw::Afterimage(f) => {
                self.afterimage(*f);
                1
            }
            other => {
                let name = format!("{other:?}");
                let name = name.split(['(', ' ', '{']).next().unwrap_or("?").to_owned();
                self.hsd.note(format!("draw {name}"));
                1
            }
        }
    }

    /// The erase color of the camera drawing: the game camera's background color.
    fn erase_color(&self) -> Color {
        let cam = &self.game.camera.cam;
        if cam.x398_b5 {
            Color::new(0, 0, 0, 0xFF)
        } else {
            let c = &cam.background_color;
            Color::new(c.r, c.g, c.b, 0xFF)
        }
    }

    /// `HSD_VICopyXFBAsync` for the screen pass: the EFB copied to the XFB and cleared.
    fn copy_xfb(&mut self) {
        let gx = &mut *self.gx;
        gx.set_copy_filter(GX_FALSE, &[[6, 6]; 12], GX_TRUE, &VFILTER);
        gx.set_disp_copy_gamma(GX_GM_1_0);
        self.hsd.state.set_color_update(gx, true);
        self.hsd.state.set_alpha_update(gx, true);
        self.hsd.state.set_z_mode(gx, true, GX_LEQUAL, true);
        gx.set_copy_clear(Color::new(0, 0, 0, 0), 0x00FF_FFFF);
        gx.set_copy_clamp(GX_CLAMP_TOP | GX_CLAMP_BOTTOM);
        gx.set_disp_copy_src(
            0,
            0,
            crate::hsd::camera::FB_WIDTH,
            crate::hsd::camera::EFB_HEIGHT,
        );
        let lines = gx.set_disp_copy_y_scale(
            f32::from(crate::hsd::camera::XFB_HEIGHT) / f32::from(crate::hsd::camera::EFB_HEIGHT),
        );
        gx.set_disp_copy_dst(crate::hsd::camera::FB_WIDTH, lines as u16);
        gx.copy_disp(XFB_ADDR, GX_TRUE);
        gx.pix_mode_sync();
        gx.set_draw_done();
    }
}

/// Widescreen: a camera drawing to the screen sees `aspect / GAME_ASPECT` times as wide,
/// keeping its height, so the frame, shown at `aspect`, keeps the proportions it has at the
/// game's 73:60. The gameplay camera (the simulation's, which decides who is off screen) stays
/// as it is; only the view widens, and the 2D cameras' content (the HUD) keeps its shape and
/// its place in the 73:60 middle.
fn widen(v: &mut melee_sim::hsd::display::CameraView, aspect: f32) {
    use melee_sim::hsd::cobj;
    let k = aspect / GAME_ASPECT;
    match v.projection_type {
        cobj::PERSPECTIVE => v.projection[1] *= k,
        cobj::FRUSTUM | cobj::ORTHO => {
            let [_, _, l, r] = v.projection;
            let (c, h) = ((l + r) / 2.0, (r - l) / 2.0 * k);
            v.projection[2] = c - h;
            v.projection[3] = c + h;
        }
        _ => {}
    }
}

/// The render mode's vertical filter (`GXNtsc480IntDf`).
const VFILTER: [u8; 7] = [8, 8, 10, 12, 10, 8, 8];
