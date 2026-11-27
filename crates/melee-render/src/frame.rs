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

/// The note for draws whose stages read a TEV register component the original left unset
/// ([`Gx::unset`]), counted a draw each.
pub const UNSET_READ: &str = "a draw read a TEV register component HSD_TExpSetReg left unset";

/// The XFB the frame is copied to. Its address is the presentation's.
pub const XFB_ADDR: u32 = 0x00F0_0000;

/// The aspect ratio the game's 640 by 480 picture is made for: 73:60 (584 square pixels by
/// 480), the width the console's picture fills on a 4:3 screen and Slippi Dolphin's "73:60"
/// option. The game camera's perspective aspect, 1.2173333 (`lb_80013B14`), is the same
/// within 0.06%: 584.32 pixels by 480.
pub const GAME_ASPECT: f32 = 73.0 / 60.0;

/// The least a piece of a streamed frame holds ([`Frames::frame_streamed`]): enough that
/// handing it on costs little beside building it, little enough that the renderer's threads
/// start on a frame's first draws early.
pub const STREAM_PIECE: usize = 4096;

/// What takes a frame's stream in pieces as it's built ([`Frames::frame_streamed`]).
pub trait Stream {
    /// The frame starts: the address space is as the frame's draws find it (its ranges given
    /// back and taken for the frame).
    fn start(&mut self, _space: &Space) {}
    /// The stream's next bytes, whole commands, with the marks so far (every draw these bytes
    /// hold has its mark).
    fn piece(&mut self, space: &Space, bytes: &[u8], marks: &[(u32, u32)]);
}

/// What the frame's stream starts from and keeps from frame to frame: GX's state and HSD's.
pub struct Frames {
    pub gx: Gx,
    pub hsd: Hsd,
    pub space: Space,
    /// How many rewritten display lists the address space knows.
    patched: usize,
    /// Frames built so far.
    pub built: u32,
    /// Where the last frame's stream starts each of the display pass's draws it drew: its
    /// offset in the stream and its index in the pass's list, in order (a draw that sends
    /// nothing shares its offset with the next). The fast renderer's comparison harness
    /// (`melee-render-check fast`) keys each GX draw by the display draw it came from.
    pub marks: Vec<(u32, u32)>,
    /// The last piped frame's time on the game thread, from its build's start until its last
    /// piece was handed on ([`Frames::frame_piped`]; the paced bench's measurement only).
    pub handed_in: std::time::Duration,
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
            marks: Vec::new(),
            handed_in: std::time::Duration::ZERO,
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
        self.build(game, data, draws, blend, None, None)
    }

    /// The frame [`Frames::frame_blended`] builds, handed to `to` in pieces as it's built
    /// (whole commands, at least [`STREAM_PIECE`] bytes but for the last), so whatever runs the
    /// stream starts on the frame's first draws while the rest are built: the fast renderer's
    /// threads draw beside the game thread rather than after it. The pieces are the frame's
    /// stream in order, and [`Frames::marks`] counts offsets from the frame's start as before.
    pub fn frame_streamed(
        &mut self,
        game: &Game,
        data: &Archives,
        draws: &[Draw],
        blend: Option<Blend<'_>>,
        to: &mut dyn Stream,
    ) {
        let rest = self.build(game, data, draws, blend, Some(to), None);
        to.piece(&self.space, &rest, &self.marks);
    }

    /// The frame [`Frames::frame_streamed`] hands to `to`, with `to` running each piece on a
    /// thread of its own while the game thread builds the next (and its frame's start before
    /// the first), so running the stream (through melee-gx to the renderer) leaves the game
    /// thread. The address space is shared meanwhile: what the stream reads of it the build
    /// wrote before handing on the piece that reads it. No marks are handed on.
    pub fn frame_piped(
        &mut self,
        game: &Game,
        data: &Archives,
        draws: &[Draw],
        blend: Option<Blend<'_>>,
        to: &mut (dyn Stream + Send),
    ) {
        self.build(game, data, draws, blend, None, Some(to));
    }

    fn build(
        &mut self,
        game: &Game,
        data: &Archives,
        draws: &[Draw],
        blend: Option<Blend<'_>>,
        to: Option<&mut dyn Stream>,
        piped: Option<&mut (dyn Stream + Send)>,
    ) -> Vec<u8> {
        let start = std::time::Instant::now();
        let patches = &game.objs.pobjs.refract_patches;
        if self.patched != patches.len() {
            self.patched = patches.len();
            let list = game
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
            self.space.set_patches(list);
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
        let Frames {
            gx,
            hsd,
            space,
            marks,
            handed_in,
            ..
        } = self;
        let space: &Space = space;
        let Some(run) = piped else {
            return build_rest(gx, hsd, space, marks, (game, data, draws, blend), to);
        };
        // The pieces run on a thread of their own while the rest are built.
        std::thread::scope(|s| {
            let (tx, rx) = std::sync::mpsc::channel::<Piece>();
            let (spare_tx, spare) = std::sync::mpsc::channel::<Vec<u8>>();
            let runner = std::thread::Builder::new()
                .name("stream".to_owned())
                .spawn_scoped(s, move || {
                    let mut marks = Vec::new();
                    for piece in rx {
                        match piece {
                            Piece::Start => run.start(space),
                            Piece::Bytes(bytes, more) => {
                                marks.extend_from_slice(&more);
                                run.piece(space, &bytes, &marks);
                                let _ = spare_tx.send(bytes);
                            }
                        }
                    }
                })
                .expect("the stream's thread");
            let mut pipe = Pipe {
                tx,
                spare,
                marks: 0,
            };
            let last = build_rest(
                gx,
                hsd,
                space,
                marks,
                (game, data, draws, blend),
                Some(&mut pipe),
            );
            pipe.piece(space, &last, marks);
            drop(pipe);
            *handed_in = start.elapsed();
            runner.join().expect("the stream's thread");
        });
        Vec::new()
    }
}

/// What a piped frame hands the thread that runs it ([`Frames::frame_piped`]).
enum Piece {
    /// The frame's start.
    Start,
    /// A piece's bytes, and the marks added since the last.
    Bytes(Vec<u8>, Vec<(u32, u32)>),
}

/// Hands a piped frame's pieces to the thread that runs them.
struct Pipe {
    tx: std::sync::mpsc::Sender<Piece>,
    spare: std::sync::mpsc::Receiver<Vec<u8>>,
    /// The marks handed on so far.
    marks: usize,
}

impl Stream for Pipe {
    fn start(&mut self, _space: &Space) {
        self.tx
            .send(Piece::Start)
            .expect("the stream's thread stopped");
    }

    fn piece(&mut self, _space: &Space, bytes: &[u8], marks: &[(u32, u32)]) {
        let mut piece = self.spare.try_recv().unwrap_or_default();
        piece.clear();
        piece.extend_from_slice(bytes);
        let more = marks[self.marks.min(marks.len())..].to_vec();
        self.marks = marks.len();
        self.tx
            .send(Piece::Bytes(piece, more))
            .expect("the stream's thread stopped");
    }
}

/// The frame's stream after [`Frames::build`]'s set-up: its draws, handed to `to` in pieces
/// if given; returns what's left of it.
fn build_rest(
    gx: &mut Gx,
    hsd: &mut Hsd,
    space: &Space,
    marks: &mut Vec<(u32, u32)>,
    (game, data, draws, blend): (&Game, &Archives, &[Draw], Option<Blend<'_>>),
    mut to: Option<&mut dyn Stream>,
) -> Vec<u8> {
    // The arrays HSD's caches hold from the frames before keep their addresses.
    space.begin_frame(&[
        hsd.prev_array.map(|d| d.archive),
        hsd.prev_vtxdesc.map(|d| d.archive),
    ]);
    let mut ctx = Ctx {
        gx,
        hsd,
        game,
        data,
        space,
        blend,
    };
    if let Some(to) = to.as_deref_mut() {
        to.start(space);
    }
    ctx.gx.invalidate_vtx_cache();
    ctx.gx.invalidate_tex_all();
    ctx.start_render(false);
    marks.clear();
    // Bytes handed on so far: the marks count from the frame's start.
    let mut sent = 0u32;
    let mut i = 0;
    while i < draws.len() {
        if let Some(to) = to.as_deref_mut()
            && ctx.gx.fifo.len() >= STREAM_PIECE
            && !ctx.gx.in_display_list()
        {
            to.piece(ctx.space, &ctx.gx.fifo, marks);
            sent += ctx.gx.fifo.len() as u32;
            ctx.gx.fifo.clear();
        }
        marks.push((sent + ctx.gx.fifo.len() as u32, i as u32));
        i += ctx.draw(draws, i);
    }
    // What the frame sends after its draws (the copy to the XFB) is the list's end.
    marks.push((sent + ctx.gx.fifo.len() as u32, draws.len() as u32));
    if let Some(n) = ctx.hsd.lights_after_render_mode.take() {
        ctx.load_lights(n);
    }
    ctx.copy_xfb();
    for at in gx.copied_to.drain(..) {
        space.pin_at(at);
    }
    if gx.unset_reads != 0 {
        *hsd.notes.entry(UNSET_READ.to_owned()).or_default() += gx.unset_reads;
        gx.unset_reads = 0;
    }
    gx.take_fifo()
}

impl Ctx<'_> {
    /// `HSD_LObjSetupInit`'s GX side for the lights the display pass's light set `n` names
    /// ([`Draw::Lights`]).
    pub fn load_lights(&mut self, n: u16) {
        let mut set = self.game.objs.display.light_sets[usize::from(n)];
        // Lights set up in the current camera's view take its blended view.
        if self.blend.is_some()
            && let (Some(b), Some(v)) = (self.hsd.motion.camera_b, self.hsd.camera)
            && set.view_mtx == b.view_mtx
        {
            set.view_mtx = v.view_mtx;
        }
        let objs = &self.game.objs;
        self.hsd.lights.setup_init(self.gx, objs, &set);
    }

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
        // A fighter material's lights after its render mode come before anything else but its
        // shapes, whose material setup loads them (`fighter_mobj_setup`).
        if !matches!(draws[i], Draw::Shape { .. })
            && let Some(n) = self.hsd.lights_after_render_mode.take()
        {
            self.load_lights(n);
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
                // Fountain of Dreams' reflection, which the water shows across the widened
                // view: its camera and copy widen with it (`Ctx::reflection_widening`).
                if let Some((width, k)) = self.reflection_widening()
                    && self.is_reflection_camera(*c)
                {
                    v.viewport[1] = v.viewport[0] + f32::from(width);
                    v.scissor[1] = v.scissor[0] + width;
                    widen(&mut v, k);
                }
                // Cameras that draw the whole screen (not a shadow's or a magnifier
                // bubble's, which draw into textures); the render pass can't tell, since
                // the frame stays in the shadows' offscreen pass, as the original's does.
                else if v.viewport == [0.0, 640.0, 0.0, 480.0]
                    && let Some(aspect) = self.hsd.widescreen
                    && !fills_screen(self.game, *c)
                {
                    widen(&mut v, aspect / GAME_ASPECT);
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
                self.load_lights(*n);
                1
            }
            Draw::LightsAfterRenderMode(n) => {
                self.hsd.lights_after_render_mode = Some(*n);
                1
            }
            Draw::Rendering(g) => {
                if let Some(g) = *g {
                    self.particles_rendering(g);
                }
                self.hsd.rendering = *g;
                // The GObj's own fighter: the fighters' pool keeps a scene's fighters past its
                // end (the next scene's setup makes the original's pool anew), whose GObjs the
                // next scene's take again, so a search by GObj can find an earlier scene's.
                self.hsd.rendering_fighter =
                    g.and_then(|g| match self.game.world.gobj(g).user_data {
                        Some(melee_sim::game::state::UserData::Fighter(f)) => Some(f),
                        _ => None,
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
            Draw::FighterJoint { jobj, vmtx } => {
                self.fighter_joint(*jobj, vmtx.as_ref());
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
            Draw::Toon(image) => {
                self.hsd.toon = *image;
                1
            }
            Draw::FighterFlags(flags) => {
                self.hsd.fighter_flags = Some(*flags);
                1
            }
            Draw::ItemFlags(flags) => {
                self.hsd.item_flags = Some(*flags);
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
            Draw::Axes(ends) => {
                self.axes(ends);
                1
            }
            Draw::ZModeDirect {
                compare,
                func,
                update,
            } => {
                self.gx
                    .set_z_mode(u8::from(*compare), *func, u8::from(*update));
                1
            }
            Draw::EraseColor([r, g, b, a]) => {
                self.hsd.erase_color = Some(Color::new(*r, *g, *b, *a));
                1
            }
            Draw::Erase => {
                // A camera whose erase color the display pass set (Fountain of Dreams'
                // reflection, Pokémon Stadium's text) erases in that color, whatever its
                // viewport: with the reflection size code the reflection's covers the screen.
                // The game camera erases in its background color.
                let full = self
                    .hsd
                    .camera
                    .is_some_and(|v| v.viewport == [0.0, 640.0, 0.0, 480.0]);
                let color = match self.hsd.erase_color.take() {
                    Some(c) => c,
                    None if full => self.erase_color(),
                    None => {
                        self.hsd.note_once("an erase without an erase color");
                        Color::new(0, 0, 0, 0xFF)
                    }
                };
                self.erase_screen(color, false);
                1
            }
            Draw::EraseFull => {
                match self.hsd.erase_color.take() {
                    Some(c) => self.erase_screen_alpha(c),
                    None => self.hsd.note_once("an erase without an erase color"),
                }
                1
            }
            Draw::Scissor { x, y, w, h } => {
                self.game_scissor(*x, *y, *w, *h);
                1
            }
            Draw::ResultsCopy { image, x, y, clear } => {
                self.results_copy(*image, *x, *y, *clear);
                1
            }
            Draw::SnapCopy => {
                self.snap_copy();
                1
            }
            Draw::ToyFallCopy { image, x, y, clear } => {
                self.toy_fall_copy(*image, *x, *y, *clear);
                1
            }
            Draw::IntroCopy { image, x, y, clear } => {
                self.intro_horde_copy(*image, *x, *y, *clear);
                1
            }
            Draw::DepthQuad {
                top,
                bottom,
                left,
                right,
            } => {
                self.depth_quad(*top, *bottom, *left, *right);
                1
            }
            Draw::BlurCopy => {
                self.blur_copy();
                1
            }
            Draw::Blur {
                pos,
                scale,
                alpha,
                size,
                tint,
            } => {
                self.blur(*pos, *scale, *alpha, *size, *tint);
                1
            }
            Draw::EraseWithDepth => {
                match self.hsd.erase_color.take() {
                    Some(c) => self.erase_screen(c, true),
                    None => self.hsd.note_once("an erase without an erase color"),
                }
                1
            }
            Draw::SpriteSetup => {
                self.sprite_setup();
                1
            }
            Draw::SObj(s) => {
                self.sobj(*s);
                1
            }
            Draw::MoviePlanes(m) => {
                self.movie_planes(*m);
                1
            }
            Draw::StillPlanes(s) => {
                self.still_planes(*s);
                1
            }
            Draw::SpriteEnd => {
                self.sprite_end();
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
            Draw::ShadowVolume(s) => {
                self.shadow_volume(*s);
                1
            }
            Draw::Particles { .. } => self.particles(draws, i),
            Draw::MagnifierCopy(i) => {
                self.magnifier_copy(*i);
                1
            }
            Draw::RefractCopy => {
                self.refract_copy();
                1
            }
            Draw::RefractEnd => {
                // `lbRefract_800225D4`.
                self.gx.set_tev_direct(0);
                self.gx.set_num_ind_stages(0);
                self.invalidate(crate::hsd::state::STATE_ALL);
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
            Draw::Text(d) => {
                self.devtext(d);
                1
            }
            Draw::Perf(d) => {
                self.perf(d);
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
            Draw::Debug(d) => {
                self.debug_draw(d);
                1
            }
        }
    }

    /// The erase color of the camera drawing: the game camera's background color.
    fn erase_color(&self) -> Color {
        let cam = &self.game.camera;
        if cam.hidden_layers()[5] {
            Color::new(0, 0, 0, 0xFF)
        } else {
            let [r, g, b] = cam.background_color();
            Color::new(r, g, b, 0xFF)
        }
    }

    /// `HSD_VICopyXFBAsync` for the screen pass: the EFB copied to the XFB and cleared.
    fn copy_xfb(&mut self) {
        // The current render mode's vertical filter: the deflicker filter's unless the game
        // set a mode without it (the opening movie's, `gmMainLib_8015F588(0)`).
        let deflicker = self.game.main_lib.video.is_none_or(|(_, d)| d);
        let vfilter = if deflicker { &VFILTER } else { &VFILTER_NONE };
        let gx = &mut *self.gx;
        gx.set_copy_filter(GX_FALSE, &[[6, 6]; 12], GX_TRUE, vfilter);
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

/// Widescreen: a camera drawing to the screen sees `k` (`aspect / GAME_ASPECT`) times as wide,
/// keeping its height, so the frame, shown at `aspect`, keeps the proportions it has at the
/// game's 73:60. The gameplay camera (the simulation's, which decides who is off screen) stays
/// as it is; only the view widens, and the 2D cameras' content (the HUD) keeps its shape and
/// its place in the 73:60 middle. What's made to cover the screen ([`fills_screen`]) isn't
/// widened, so it stretches across the picture.
fn widen(v: &mut melee_sim::hsd::display::CameraView, k: f32) {
    use melee_sim::hsd::cobj;
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

/// Whether camera `c` draws something made to cover the whole 640 by 480 screen, which
/// widescreen stretches to the picture's edges rather than widening: the background flash's
/// (`lbbgflash.c`), whose color and wipes are 640 by 480 quads seen by a camera framing
/// exactly that. Widened, it would see more than the quads and leave the added sides
/// uncovered. Known by the camera's owner, as the flash records its camera GObj.
fn fills_screen(game: &Game, c: melee_sim::hsd::cobj::CObjId) -> bool {
    use melee_sim::game::state::Obj;
    game.bgflash.data.camera.is_some_and(|g| {
        !game.world.gobjs.is_free(g.0) && game.world.gobj(g).obj == Some(Obj::CObj(c))
    })
}

/// The render mode's vertical filter (`GXNtsc480IntDf`, and progressive scan's with the
/// deflicker filter, `gmMainLib_803D4A80`).
const VFILTER: [u8; 7] = [8, 8, 10, 12, 10, 8, 8];
/// Without the deflicker filter (`GXNtsc480Int`, `GXNtsc480Prog`).
const VFILTER_NONE: [u8; 7] = [0, 0, 21, 22, 21, 0, 0];
