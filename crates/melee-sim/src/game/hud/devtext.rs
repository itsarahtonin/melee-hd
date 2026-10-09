//! The debug text screens (`melee/if/textdraw.c` and `textlib.c`): up to 32 character grids
//! from a fixed pool, drawn in id order over the scene by their own GObj and camera. Every
//! scene sets the system up (`un_802FF78C`); a retail disc never shows a screen, but the
//! setup's GObjs and camera are in the scheduler and the pool is state, so all of it is
//! simulation. Drawing a screen becomes draw commands ([`Draw`]).

use gekko_fp as fp;
use melee_data::archive::Archives;

use super::super::handles::CodeRef;
use super::super::state::{Game, OBJ_CAMERA, Obj};
use crate::fpu::reg;
use crate::hsd::cobj::{self, CObjId, StaticDesc};
use crate::hsd::gobj::GObjId;
use crate::hsd::pool::Pool;
use crate::vec::Vec3;

/// The pool's size.
pub const POOL: usize = 32;

/// `DEVTEXT_FLAG_HIDETEXT`, `_HIDEBACKGROUND`, `_NOWRAP`, `_SHOWCURSOR`.
pub const HIDE_TEXT: u8 = 0x80;
pub const HIDE_BACKGROUND: u8 = 0x40;
pub const NO_WRAP: u8 = 0x20;
pub const SHOW_CURSOR: u8 = 0x10;

/// `DevText_DrawAll`, the drawing GObj's render callback.
pub const DRAW_ALL: u32 = 0x8030_2608;
/// `HSD_GObj_803910D8`, the camera GObj's render callback.
pub const CAMERA_RENDER: u32 = 0x8039_10D8;

/// `HSD_RP_BOTTOMHALF`: the render pass the screens draw in.
pub const PASS_BOTTOM_HALF: i32 = 2;

/// `devtext_CObjDesc` (`.data` at 0x803FDC48) with its eye and interest.
pub const CAMERA_DESC: StaticDesc = StaticDesc {
    flags: 0,
    projection_type: 1,
    viewport: [0, 640, 0, 480],
    scissor: [0, 640, 0, 480],
    eye: Vec3::new(0.0, 40.241_425, 300.241),
    interest: Vec3::new(0.0, 10.0, 0.0),
    roll: 0.0,
    near: 0.1,
    far: 32768.0,
    projection: [30.0, 1.3636, 0.0, 0.0],
};

/// A grid cell (`DevTextGlyph`): a character and its colour index (the top two bits of the
/// second byte; the other six are never set).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Glyph {
    pub chr: u8,
    pub attr: u8,
}

impl Glyph {
    pub fn color(self) -> u8 {
        self.attr >> 6
    }

    fn set_color(&mut self, c: u8) {
        self.attr = self.attr & 0x3F | (c & 3) << 6;
    }
}

/// A screen (`DevText`). `prev` and `next` are pool indices, and `buf` stands for the buffer
/// its creator passes, which the screen alone writes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DevText {
    pub x: i16,
    pub y: i16,
    pub w: u8,
    pub h: u8,
    pub cursor_x: u8,
    pub cursor_y: u8,
    pub scale_x: f32,
    pub scale_y: f32,
    pub bg_color: [u8; 4],
    pub text_colors: [[u8; 4]; 4],
    pub id: i8,
    pub line_width: u8,
    pub flags: u8,
    /// The byte at 0x27: the cursor's blink counter (top six bits) and the colour index new
    /// characters take (bottom two).
    pub unk: u8,
    pub current_color: u8,
    pub buf: Vec<Glyph>,
    pub prev: Option<u8>,
    pub next: Option<u8>,
}

/// A debug menu's state (`un_80304138_objalloc_t`): the debug menus aren't ported, only
/// their pool, which every scene resets.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DebugMenu;

/// The system's globals (`textdraw.c`'s statics and `textlib_1.c`'s menu pool).
#[derive(Clone, Debug, Default)]
pub struct DevTexts {
    /// `devtext_pool`.
    pub pool: Vec<DevText>,
    /// `devtext_poolhead`: the first free screen.
    pub pool_head: Option<u8>,
    /// `devtext_drawlist`: the screens shown, by id.
    pub draw_list: Option<u8>,
    /// `devtext_setup_classifier` to `devtext_setup_render_priority`.
    pub classifier: i32,
    pub p_link: i32,
    pub priority: i32,
    pub gx_link: i32,
    pub render_priority: i32,
    /// `devtext_cobj`: the orthographic camera the first drawing makes.
    pub cobj: Option<CObjId>,
    /// `devtext_gobj`: the GObj that draws the screens.
    pub gobj: Option<GObjId>,
    /// `un_804A2688`: the debug menus' pool, once a scene has set it up.
    pub menus: Option<Pool<DebugMenu>>,
}

/// What drawing a screen asks of the presentation layer, its floats as the registers
/// pass them.
#[derive(Clone, Debug, PartialEq)]
pub enum Draw {
    /// `HSD_FogSet(NULL)`.
    NoFog,
    /// `HSD_CObjSetCurrent`: draw through this camera.
    Camera(CObjId),
    /// `hsd_80391A04`: the character cell size and line width the next glyphs use.
    TextScale { x: f64, y: f64, line_width: i32 },
    /// `DrawRectangle`.
    Rect {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        color: [u8; 4],
    },
    /// `DrawASCII`.
    Ascii {
        chr: i32,
        x: f64,
        y: f64,
        color: [u8; 4],
    },
}

impl DevTexts {
    fn text(&mut self, i: u8) -> &mut DevText {
        &mut self.pool[usize::from(i)]
    }

    /// `DevText_GetGObj`.
    #[doc(alias = "DevText_GetGObj")]
    pub fn get_gobj(&self) -> Option<GObjId> {
        self.gobj
    }

    /// Links every screen into the free list in order and empties the draw list,
    /// `DevText_InitPool`.
    #[doc(alias = "DevText_InitPool")]
    pub fn init_pool(&mut self) {
        self.pool.resize_with(POOL, DevText::default);
        self.pool[0].prev = None;
        for i in 0..POOL as u8 - 1 {
            self.text(i + 1).prev = Some(i);
            self.text(i).next = Some(i + 1);
        }
        self.text(POOL as u8 - 1).next = None;
        self.pool_head = Some(0);
        self.draw_list = None;
    }

    /// Takes the screen `*list` names out of its list and puts it back on the free list,
    /// `DevText_Remove`. `list` is the draw list or, if `in_draw_list` is false, `at`'s
    /// screen reached through a `next` link (the original takes a pointer to a link).
    #[doc(alias = "DevText_Remove")]
    pub fn remove(&mut self, link: Link) {
        let text = self.link(link).expect("a screen to remove");
        let next = self.text(text).next;
        let prev = self.text(text).prev;
        if let Some(n) = next {
            self.text(n).prev = prev;
        }
        match prev {
            Some(p) => self.text(p).next = next,
            None => self.set_link(link, next),
        }
        let head = self.pool_head;
        let t = self.text(text);
        t.next = head;
        t.prev = None;
        self.pool_head = Some(text);
    }

    fn link(&self, link: Link) -> Option<u8> {
        match link {
            Link::DrawList => self.draw_list,
            Link::PoolHead => self.pool_head,
            Link::NextOf(i) => self.pool[usize::from(i)].next,
        }
    }

    fn set_link(&mut self, link: Link, v: Option<u8>) {
        match link {
            Link::DrawList => self.draw_list = v,
            Link::PoolHead => self.pool_head = v,
            Link::NextOf(i) => self.text(i).next = v,
        }
    }

    /// Inserts `text` into a list after the screens whose ids are no greater,
    /// `DevText_AddToList`. Like the original, it leaves the following screen's `prev`, and
    /// a screen inserted first in a list that has screens keeps the old head as its `next`.
    #[doc(alias = "DevText_AddToList")]
    pub fn add_to_list(&mut self, list: Link, text: u8) {
        let Some(head) = self.link(list) else {
            self.set_link(list, Some(text));
            return;
        };
        let id = self.text(text).id;
        let (mut next, mut prev) = (Some(head), None);
        while let Some(n) = next {
            if id >= self.text(n).id {
                prev = Some(n);
                next = self.text(n).next;
            } else {
                break;
            }
        }
        self.text(text).next = next;
        match prev {
            Some(p) => {
                self.text(p).next = Some(text);
                self.text(text).prev = Some(p);
            }
            None => {
                self.text(text).prev = None;
                self.set_link(list, Some(text));
            }
        }
    }

    /// Shows a screen, `DevText_Show`.
    #[doc(alias = "DevText_Show")]
    pub fn show(&mut self, text: u8) {
        self.add_to_list(Link::DrawList, text);
    }

    /// A new screen with id `id` at (`x`, `y`), `w` by `h` cells, cleared, `DevText_Create`;
    /// none if a shown screen has the id. An empty pool asserts in the original.
    #[doc(alias = "DevText_Create")]
    pub fn create(&mut self, id: i8, x: i32, y: i32, w: i32, h: i32) -> Option<u8> {
        let mut cur = self.draw_list;
        while let Some(c) = cur {
            if self.text(c).id == id {
                return None;
            }
            cur = self.text(c).next;
        }
        let text = self
            .pool_head
            .unwrap_or_else(|| panic!("the original's assertion failed: TW : Screen alloc Fail"));
        self.pool_head = self.text(text).next;
        let t = self.text(text);
        t.x = x as i16;
        t.y = y as i16;
        t.w = w as u8;
        t.h = h as u8;
        t.cursor_x = 0;
        t.cursor_y = 0;
        t.scale_x = 10.0;
        t.scale_y = 16.0;
        t.bg_color = [0x60, 0xD0, 0xB0, 0x70];
        t.text_colors = [
            [0xFF, 0xFF, 0xFF, 0xFF],
            [0xFF, 0x80, 0x80, 0xFF],
            [0x80, 0xFF, 0x80, 0xFF],
            [0x80, 0x80, 0xFF, 0xFF],
        ];
        t.id = id;
        t.line_width = 10;
        t.flags = SHOW_CURSOR;
        t.unk = 0;
        t.current_color = 0;
        t.prev = None;
        t.next = None;
        // `memzero(buf, h * w * 2)`, with the sizes as the caller passed them.
        t.buf = vec![Glyph::default(); (h * w).max(0) as usize];
        Some(text)
    }
}

/// Where a list link the original passes by pointer lives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Link {
    DrawList,
    PoolHead,
    NextOf(u8),
}

impl DevText {
    /// Moves every line up one, clearing the last, `DevText_EraseFirstLine`.
    #[doc(alias = "DevText_EraseFirstLine")]
    pub fn erase_first_line(&mut self) {
        let w = usize::from(self.w);
        let mut start = 0;
        for _ in 0..i32::from(self.h) - 1 {
            self.buf.copy_within(start + w..start + 2 * w, start);
            start += w;
        }
        self.buf[start..start + w].fill(Glyph::default());
    }

    fn clamp(v: i32, max: i32) -> u8 {
        (if max <= v {
            max - 1
        } else if v < 0 {
            0
        } else {
            v
        }) as u8
    }

    /// `DevText_SetCursorXY`.
    #[doc(alias = "DevText_SetCursorXY")]
    pub fn set_cursor_xy(&mut self, x: i32, y: i32) {
        self.cursor_x = Self::clamp(x, i32::from(self.w));
        self.cursor_y = Self::clamp(y, i32::from(self.h));
    }

    /// `DevText_SetCursorX`.
    #[doc(alias = "DevText_SetCursorX")]
    pub fn set_cursor_x(&mut self, x: i32) {
        self.cursor_x = Self::clamp(x, i32::from(self.w));
    }

    #[doc(alias = "DevText_HideCursor")]
    pub fn hide_cursor(&mut self) {
        self.flags &= !SHOW_CURSOR;
    }

    /// Stops lines wrapping, `DevText_80302AC0`.
    #[doc(alias = "DevText_80302AC0")]
    pub fn no_wrap(&mut self) {
        self.flags |= NO_WRAP;
    }

    #[doc(alias = "DevText_ShowBackground")]
    pub fn show_background(&mut self) {
        self.flags &= !HIDE_BACKGROUND;
    }

    #[doc(alias = "DevText_HideBackground")]
    pub fn hide_background(&mut self) {
        self.flags |= HIDE_BACKGROUND;
    }

    #[doc(alias = "DevText_ShowText")]
    pub fn show_text(&mut self) {
        self.flags &= !HIDE_TEXT;
    }

    #[doc(alias = "DevText_HideText")]
    pub fn hide_text(&mut self) {
        self.flags |= HIDE_TEXT;
    }

    #[doc(alias = "DevText_SetScale")]
    pub fn set_scale(&mut self, x: f32, y: f32) {
        self.scale_x = x;
        self.scale_y = y;
    }

    /// Moves the screen, keeping it at least 8 from the top left, `DevText_SetXY`.
    #[doc(alias = "DevText_SetXY")]
    pub fn set_xy(&mut self, x: i32, y: i32) {
        self.x = x.max(8) as i16;
        self.y = y.max(8) as i16;
    }

    /// Picks the colour new characters take; returns the last, `DevText_StoreColorIndex`.
    #[doc(alias = "DevText_StoreColorIndex")]
    pub fn store_color_index(&mut self, index: u8) -> u8 {
        let old = self.current_color;
        self.current_color = index & 3;
        old
    }

    /// Sets the current colour index's colour; returns the last, `DevText_SetTextColor`.
    #[doc(alias = "DevText_SetTextColor")]
    pub fn set_text_color(&mut self, color: [u8; 4]) -> [u8; 4] {
        let i = usize::from(self.current_color);
        std::mem::replace(&mut self.text_colors[i], color)
    }

    #[doc(alias = "DevText_SetBGColor")]
    pub fn set_bg_color(&mut self, color: [u8; 4]) -> [u8; 4] {
        std::mem::replace(&mut self.bg_color, color)
    }

    /// Clears every cell, `DevText_Erase`.
    #[doc(alias = "DevText_Erase")]
    pub fn erase(&mut self) {
        let n = usize::from(self.w) * usize::from(self.h);
        self.buf[..n].fill(Glyph::default());
    }

    fn advance_line(&mut self) {
        self.cursor_x = 0;
        if i32::from(self.cursor_y) < i32::from(self.h) - 1 {
            self.cursor_y += 1;
        } else {
            self.erase_first_line();
        }
    }

    /// Writes a string at the cursor in the current colour, `DevText_Print`.
    #[doc(alias = "DevText_Print")]
    pub fn print(&mut self, s: &[u8]) {
        for &c in s.iter().take_while(|&&c| c != 0) {
            if c == b'\n' {
                self.advance_line();
                continue;
            }
            let i = usize::from(self.cursor_x) + usize::from(self.cursor_y) * usize::from(self.w);
            let color = self.current_color;
            let g = &mut self.buf[i];
            g.chr = c;
            g.set_color(color);
            if i32::from(self.cursor_x) < i32::from(self.w) - 1 {
                self.cursor_x += 1;
            } else if self.flags & NO_WRAP == 0 {
                self.advance_line();
            }
        }
    }

    /// Writes a number in decimal, `DevText_PrintInt`.
    #[doc(alias = "DevText_PrintInt")]
    pub fn print_int(&mut self, n: i32) {
        self.print(&num_to_str(n));
    }

    /// Draws the screen, `DevText_Draw`: its background twice, each glyph, and a blinking
    /// cursor, whose counter is state.
    #[doc(alias = "DevText_Draw")]
    pub fn draw(&mut self, out: &mut Vec<Draw>) {
        let s32 = |v: i32| fp::frsp(f64::from(v));
        out.push(Draw::TextScale {
            x: reg(self.scale_x),
            y: reg(self.scale_y),
            line_width: i32::from(self.line_width),
        });
        let (x, y) = (i32::from(self.x), i32::from(self.y));
        let (sx, sy) = (reg(self.scale_x), reg(self.scale_y));
        if self.flags & HIDE_BACKGROUND == 0 {
            for pad in [8, 4] {
                out.push(Draw::Rect {
                    x: (s32(x - pad)),
                    y: (s32(y - pad)),
                    w: (fp::fmadds(sx, s32(i32::from(self.w)), s32(pad))),
                    h: (fp::fmadds(sy, s32(i32::from(self.h)), s32(pad))),
                    color: self.bg_color,
                });
            }
        }
        if self.flags & HIDE_TEXT != 0 {
            return;
        }
        let mut yy = y;
        for row in 0..i32::from(self.h) {
            let mut xx = x;
            for col in 0..i32::from(self.w) {
                let g = self.buf[(col + i32::from(self.w) * row) as usize];
                if g.chr != 0 {
                    out.push(Draw::Ascii {
                        chr: i32::from(g.chr as i8),
                        x: (s32(xx)),
                        y: (s32(yy)),
                        color: self.text_colors[usize::from(g.color())],
                    });
                }
                xx = fp::fctiwz(fp::fadds(s32(xx), sx));
            }
            yy = fp::fctiwz(fp::fadds(s32(yy), sy));
        }
        if self.flags & SHOW_CURSOR != 0 {
            self.unk = self.unk.wrapping_add(1) & 0x3F;
            if self.unk > 16 {
                self.unk = 0;
            } else if 8 < self.unk {
                out.push(Draw::Rect {
                    x: (fp::fmadds(sx, s32(i32::from(self.cursor_x)), s32(x))),
                    y: (fp::fsubs(
                        fp::fmadds(sy, s32(i32::from(self.cursor_y)), s32(y)),
                        s32(4),
                    )),
                    w: sx,
                    h: sy,
                    color: [0xFF, 0xFF, 0xFF, 0xC0],
                });
            }
        }
    }
}

/// A number's decimal digits, a minus first if negative, `DevText_NumToStr`. Like the
/// original, `i32::MIN` stays negative and its digits come out as `'0' + (n % 10)` of a
/// negative remainder.
#[doc(alias = "DevText_NumToStr")]
pub fn num_to_str(n: i32) -> Vec<u8> {
    let mut out = Vec::new();
    let mut num = n;
    if num < 0 {
        out.push(b'-');
        num = num.wrapping_neg();
    }
    let start = out.len();
    loop {
        out.push((num % 10 + i32::from(b'0')) as u8);
        num /= 10;
        if num == 0 {
            break;
        }
    }
    out[start..].reverse();
    out
}

/// A C string's length, 0 for none, `DevText_StrLen`.
#[doc(alias = "DevText_StrLen")]
pub fn str_len(s: Option<&[u8]>) -> i32 {
    s.map_or(0, |s| s.iter().take_while(|&&c| c != 0).count() as i32)
}

impl Game {
    /// The screens' camera on its GObj (`classifier`, `p_link`, `priority`), rendering on
    /// `gx_link` from the last render link, `DevText_CreateCObj`.
    #[doc(alias = "DevText_CreateCObj")]
    pub fn devtext_create_cobj(
        &mut self,
        data: &Archives,
        classifier: i32,
        p_link: i32,
        priority: i32,
        gx_link: i32,
        gx_priority: u8,
    ) {
        let g = self
            .world
            .create(classifier as u16, p_link as u8, priority as u8);
        let c = cobj::load_static_desc(&mut self.objs, data, &CAMERA_DESC);
        self.world.set_obj(g, OBJ_CAMERA, Obj::CObj(c));
        self.world
            .setup_gx_link_max(g, CodeRef(CAMERA_RENDER), gx_priority, false);
        self.world.gobj_mut(g).gxlink_prios = 1u64.wrapping_shl(gx_link as u32);
    }

    /// Sets the screens up for a scene: their camera, an empty pool and the GObj that draws
    /// them, `DevText_Setup`.
    #[doc(alias = "DevText_Setup")]
    #[allow(clippy::too_many_arguments)]
    pub fn devtext_setup(
        &mut self,
        data: &Archives,
        classifier: i32,
        p_link: i32,
        priority: i32,
        gx_link: i32,
        render_priority: i32,
        camera_priority: u8,
    ) -> Option<GObjId> {
        let d = &mut self.hud.devtext;
        d.classifier = classifier;
        d.p_link = p_link;
        d.priority = priority;
        d.gx_link = gx_link;
        d.render_priority = render_priority;
        d.cobj = None;
        self.devtext_create_cobj(data, classifier, p_link, priority, gx_link, camera_priority);
        self.hud.devtext.init_pool();
        let g = self
            .world
            .create(classifier as u16, p_link as u8, priority as u8);
        self.world.setup_gx_link(
            g,
            CodeRef(DRAW_ALL),
            gx_link as u8,
            (render_priority & 0xFF) as u8,
        );
        self.hud.devtext.gobj = Some(g);
        Some(g)
    }

    /// The scene's debug text (`un_802FF78C`): the screens' setup, then a fresh debug menu
    /// pool (`un_80304138`).
    #[doc(alias = "un_802FF78C")]
    pub fn setup_scene_text(&mut self, data: &Archives) -> Option<GObjId> {
        let g = self.devtext_setup(data, 21, 24, 0, 17, 0, 11);
        if g.is_some() {
            self.init_debug_menus();
        }
        g
    }

    /// Resets the debug menus' pool, `un_80304138` (`HSD_ObjAllocInit(un_804A2688, 32, 4)`).
    #[doc(alias = "un_80304138")]
    pub fn init_debug_menus(&mut self) {
        self.hud.devtext.menus = Some(Pool::default());
    }

    /// The orthographic camera the screens draw through, made the first time,
    /// `DevText_SetupCObj`.
    #[doc(alias = "DevText_SetupCObj")]
    pub fn devtext_setup_cobj(&mut self, data: &Archives, out: &mut Vec<Draw>) {
        let c = match self.hud.devtext.cobj {
            Some(c) => c,
            None => {
                let o = &mut self.objs;
                let c = cobj::alloc(o);
                cobj::set_projection_type(&mut o.cobjs, Some(c), 3);
                cobj::set_viewport(&mut o.cobjs, Some(c), [0, 640, 0, 480]);
                cobj::set_scissor(&mut o.cobjs, Some(c), [0, 640, 0, 480]);
                cobj::set_eye_position(o, c, Vec3::new(0.0, 0.0, 1.0));
                cobj::set_interest(o, c, Vec3::new(0.0, 0.0, 0.0));
                cobj::set_roll(o, data, Some(c), 0.0);
                cobj::set_near(&mut o.cobjs, Some(c), 0.0);
                cobj::set_far(&mut o.cobjs, Some(c), 2.0);
                cobj::set_ortho(&mut o.cobjs, Some(c), -20.0, 500.0, -20.0, 660.0);
                self.hud.devtext.cobj = Some(c);
                c
            }
        };
        out.push(Draw::Camera(c));
    }

    /// The drawing GObj's render callback, `DevText_DrawAll`: in the bottom half pass, no
    /// fog, the screens' camera and each shown screen in turn.
    #[doc(alias = "DevText_DrawAll")]
    pub fn devtext_draw_all(&mut self, data: &Archives, pass: i32) -> Vec<Draw> {
        let mut out = Vec::new();
        self.devtext_draw_all_into(data, pass, &mut out);
        out
    }

    /// [`Game::devtext_draw_all`] appending to `out`.
    pub fn devtext_draw_all_into(&mut self, data: &Archives, pass: i32, out: &mut Vec<Draw>) {
        if pass as u32 != PASS_BOTTOM_HALF as u32 {
            return;
        }
        let mut text = self.hud.devtext.draw_list;
        out.push(Draw::NoFog);
        self.devtext_setup_cobj(data, out);
        while let Some(t) = text {
            let d = &mut self.hud.devtext.pool[usize::from(t)];
            d.draw(out);
            text = d.next;
        }
    }
}
