//! The text system's functions that make and free GObjs (`sysdolphin/baselib/sislib.c`):
//! texts with GObjs of their own, canvases with their cameras, and the fonts' archives. The
//! text state itself is [`crate::hsd::sis`].

use melee_data::archive::{ArchiveId, DataRef};

use super::super::handles::CodeRef;
use super::super::state::{OBJ_CAMERA, Obj, Running, UserData};
use crate::hsd::cobj;
use crate::hsd::gobj::{self, GObjId};
use crate::hsd::sis::{
    self, CANVAS_CAMERA, CANVAS_SIZE, Canvas, CanvasId, SisPtr, TEXT_SIZE, Text, TextId,
};
use crate::vec::Vec3;

impl Running<'_> {
    /// A new text in font `font` at the end of the list; with a canvas (the `context`-th of
    /// the font's, if not negative), a GObj of its own that draws it and frees it,
    /// `HSD_SisLib_803A5ACC`.
    #[doc(alias = "HSD_SisLib_803A5ACC")]
    pub fn create_text(
        &mut self,
        font: i32,
        context: i32,
        pos: Vec3,
        box_w: f32,
        box_h: f32,
    ) -> TextId {
        let g = &mut *self.game;
        let mut gobj = None;
        if context >= 0 {
            let mut n = context;
            let mut cur = g.sis.first_canvas;
            let canvas = loop {
                let c = cur.unwrap_or_else(|| {
                    panic!("a text on canvas {context} of font {font}, past the last, isn't ported")
                });
                let e = *g.sis.canvas(c);
                if i32::from(e.font) == font {
                    n -= 1;
                    if n < 0 {
                        break e;
                    }
                }
                cur = e.next;
            };
            let o = g
                .world
                .create(canvas.classifier, canvas.p_link, canvas.p_priority);
            g.world.setup_gx_link(
                o,
                CodeRef(sis::TEXT_RENDER),
                canvas.gx_link,
                canvas.gx_priority,
            );
            gobj = Some((o, canvas.classifier));
        }
        let mut tail = None;
        let mut cur = g.sis.first_text;
        while let Some(c) = cur {
            tail = Some(c);
            cur = g.sis.text(c).next;
        }
        let t = TextId(g.sis.alloc(TEXT_SIZE as i32));
        if g.sis.first_text.is_none() {
            g.sis.first_text = Some(t);
        }
        if let Some(l) = tail {
            g.sis.text_mut(l).next = Some(t);
        }
        if let Some((o, class)) = gobj {
            g.world.init_user_data(o, class as u8, UserData::Text(t));
        }
        g.sis.texts.insert(
            t.0,
            Text {
                pos,
                box_size: [box_w, box_h],
                x14: [0.0; 4],
                font_size: [1.0, 1.0],
                bg_color: [0, 0, 0, 0],
                text_color: [0xFF; 4],
                x34: [1.0, 1.0],
                x3c: [0.0, 0.0],
                x44: 0,
                x46: 0,
                default_fitting: 0,
                default_kerning: 0,
                default_alignment: 0,
                x4b: None,
                x4c: 0,
                hidden: 0,
                x4e: 0,
                font: font as u8,
                next: None,
                entity: gobj.map(|(o, _)| o),
                render_callback: None,
                sis_buffer: None,
                x60: None,
                alloc_data: None,
                state_stack: None,
                state_stack_used: 0,
                state_stack_capacity: 0,
                current_width: None,
                current_height: None,
                x78: None,
                x80: None,
                x88: None,
                active_color: None,
                x90: None,
                x92: None,
                x94: None,
                x98: None,
                fitting: None,
                kerning: None,
                alignment: None,
                x9f: None,
            },
        );
        t
    }

    /// Frees a text, through its GObj if it has one, `HSD_SisLib_803A5CC4`. A text not on
    /// the list does nothing.
    #[doc(alias = "HSD_SisLib_803A5CC4")]
    pub fn destroy_text(&mut self, t: TextId) {
        let mut cur = self.game.sis.first_text;
        while let Some(c) = cur {
            let next = self.game.sis.text(c).next;
            if c == t {
                self.free_text_or_gobj(c);
                return;
            }
            cur = next;
        }
    }

    fn free_text_or_gobj(&mut self, c: TextId) {
        match self.game.sis.text(c).entity {
            Some(g) => {
                gobj::free(self, g);
                if let Some(t) = self.game.sis.texts.get_mut(&c.0) {
                    t.entity = None;
                }
            }
            None => self.game.sis.free_text(c),
        }
    }

    /// Frees every text, `HSD_SisLib_803A5D30`.
    #[doc(alias = "HSD_SisLib_803A5D30")]
    pub fn destroy_all_texts(&mut self) {
        let mut cur = self.game.sis.first_text;
        while let Some(c) = cur {
            let next = self.game.sis.text(c).next;
            self.free_text_or_gobj(c);
            cur = next;
        }
    }

    /// Adds a canvas for font `font`'s texts at the end of the list; without a parent GObj,
    /// with its own orthographic camera GObj. Returns how many canvases the font had,
    /// `HSD_SisLib_803A611C`.
    #[doc(alias = "HSD_SisLib_803A611C")]
    #[allow(clippy::too_many_arguments)]
    pub fn add_canvas(
        &mut self,
        font: i32,
        parent: Option<GObjId>,
        classifier: u16,
        p_link: u8,
        p_priority: u8,
        gx_link: u8,
        gx_priority: u8,
        render_priority: u32,
    ) -> i32 {
        let g = &mut *self.game;
        let (mut tail, mut count) = (None, 0);
        let mut cur = g.sis.first_canvas;
        while let Some(c) = cur {
            tail = Some(c);
            let e = g.sis.canvas(c);
            if i32::from(e.font) == font {
                count += 1;
            }
            cur = e.next;
        }
        let c = CanvasId(g.sis.alloc(CANVAS_SIZE as i32));
        if g.sis.first_canvas.is_none() {
            g.sis.first_canvas = Some(c);
        }
        if let Some(l) = tail {
            g.sis.canvases.get_mut(&l.0).expect("a canvas").next = Some(c);
        }
        let mut camera = None;
        if parent.is_none() {
            let o = g.world.create(classifier, p_link, p_priority);
            let cam = cobj::load_static_desc(&mut g.objs, self.data, &CANVAS_CAMERA);
            cobj::set_ortho(&mut g.objs.cobjs, Some(cam), 0.0, -480.0, 0.0, 640.0);
            g.world.set_obj(o, OBJ_CAMERA, Obj::CObj(cam));
            g.world
                .setup_gx_link_max(o, CodeRef(sis::CAMERA_RENDER), render_priority as u8, false);
            g.world.gobj_mut(o).gxlink_prios = 1u64.wrapping_shl(u32::from(gx_link));
            g.world
                .init_user_data(o, classifier as u8, UserData::SisCamera(o));
            camera = Some(o);
        }
        g.sis.canvases.insert(
            c.0,
            Canvas {
                next: None,
                gobj: camera,
                classifier,
                font: font as u16,
                p_link,
                p_priority,
                gx_link,
                gx_priority,
            },
        );
        count
    }

    /// Takes font `font`'s strings from `archive`'s table `symbol`, `HSD_SisLib_803A62A0`;
    /// the archive is loaded by name (`HSD_SisLib_803A945C`) outside the simulation.
    #[doc(alias = "HSD_SisLib_803A62A0")]
    pub fn load_font(&mut self, font: i32, archive: ArchiveId, symbol: &str) {
        let s = &mut self.game.sis;
        s.font_archives[font as usize] = Some(archive);
        let at = self.data.get(archive).public(symbol).unwrap_or_else(|| {
            panic!("the original's assertion failed: Cannot find symbol {symbol}")
        });
        s.font_tables[font as usize] = Some(DataRef {
            archive,
            offset: at,
        });
    }

    /// A full-screen text with a string buffer of its own, `HSD_SisLib_803A6754`.
    #[doc(alias = "HSD_SisLib_803A6754")]
    pub fn create_text_buffer(&mut self, font: i32, context: i32) -> TextId {
        let t = self.create_text(font, context, Vec3::new(0.0, 0.0, 0.0), 640.0, 480.0);
        let s = &mut self.game.sis;
        let a = s.alloc(sis::BUFFER_SIZE as i32);
        s.text_mut(t).alloc_data = Some(a);
        s.buffers.insert(a, sis::Buffer::default());
        let buf = s.alloc(0x80);
        s.buffers.insert(
            a,
            sis::Buffer {
                end: buf,
                data: Some(buf),
                size: 0x80,
                count: 0,
            },
        );
        s.arena[buf as usize] = 0;
        s.reset_text(self.data, t, 0);
        let s = &mut self.game.sis;
        s.text_mut(t).sis_buffer = Some(SisPtr::Arena(buf));
        t
    }
}
