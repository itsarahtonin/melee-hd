//! The gallery's list of trophies (`tylist.c`): a column of rows (the name, its count, the
//! series mark, a new trophy's mark), sorted three ways, the stick or the D-pad moving the
//! cursor, X and Y paging a list of more than ten; A or Start shows the trophy in the gallery,
//! B ends the scene.

use gekko_fp as fp;

use super::super::handles::CodeRef;
use super::super::render::{JOBJ_CALLBACK, LOBJ_CALLBACK};
use super::super::state::{OBJ_CAMERA, OBJ_JOBJ, OBJ_LIGHT, Obj, Running};
use crate::fpu::{mem, reg};
use crate::hsd::cobj;
use crate::hsd::gobj::{self, GObjId};
use crate::hsd::jobj::{self, JObjId};
use crate::hsd::sis::{SisPtr, TextId};
use crate::vec::Vec3;

/// The list's processes and render.
pub const PROC_INTRO: u32 = 0x8031_438C;
pub const PROC_INPUT: u32 = 0x8031_3BD8;
pub const RENDER_TEXTS: u32 = 0x8031_4504;

/// The list's rows at most, and `entryCount`'s rows past the trophies' with ten or fewer.
const ROWS: usize = 12;

/// `TyListArg` (0x34 bytes), a row: its neighbours, its base, new-trophy mark and series mark,
/// its three texts, its slot among the rows shown, its trophy's place in the order, its
/// number, and its height and target height.
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Row {
    pub prev: u8,
    pub next: u8,
    pub jobjs: [Option<JObjId>; 3],
    pub texts: [Option<TextId>; 3],
    pub slot: i8,
    pub idx: i16,
    pub number: i32,
    pub target: f32,
    pub y: f32,
}

/// `TyListState` (`_tyList_804A2AC0`, 0x2AC bytes).
#[derive(Clone, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct ListState {
    pub rows: [Row; ROWS],
    /// `x270`, `x274`, `x278`: the first row, the last, the cursor's.
    pub first: u8,
    pub last: u8,
    pub cursor_row: u8,
    /// `x27C`: the list's model's GObj; `x288` the cursor; `x28C` the list's end; `x290` the
    /// count's text.
    pub gobj: Option<GObjId>,
    pub cursor: Option<JObjId>,
    pub end: Option<JObjId>,
    pub count_text: Option<TextId>,
    /// `x298`: the cursor's place in the order.
    pub selected: i16,
    pub count: i8,
    /// `x29B`: the sort; `x29C` its direction.
    pub sort: i8,
    pub sort_dir: i8,
    /// `x29D` to `x2A1`: a page's rows, its rows left, its scroll's frames left, frames before
    /// the next repeat, and which way it scrolls.
    pub x29d: i8,
    pub x29e: i8,
    pub x29f: i8,
    pub x2a0: i8,
    pub x2a1: i8,
    /// `x2A4`: a scroll's step; `x2A8` a row's height.
    pub step: f32,
    pub height: f32,
}

/// `TyListGobjEntry` (`_tyList_804A2D6C`, 0x18 bytes): the list's two cameras, the cursor's
/// slot and the one before, the stick's hold state, and the intro's frames.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct ListInput {
    pub camera: Option<GObjId>,
    pub texts_camera: Option<GObjId>,
    pub slot: i8,
    pub slot_before: i8,
    pub held: i8,
    pub x10: i8,
    pub x11: i8,
    pub x12: i8,
    pub x13: i8,
    pub x14: i8,
    pub intro: i8,
}

/// What `_tyList_80313508` makes: a model under its parent's root, or one on a GObj of its own.
enum ListModel {
    Joint(JObjId),
    GObj(GObjId),
}

impl ListModel {
    fn left(self) -> Option<JObjId> {
        match self {
            ListModel::Joint(j) => Some(j),
            ListModel::GObj(_) => None,
        }
    }

    fn right(self) -> Option<GObjId> {
        match self {
            ListModel::GObj(g) => Some(g),
            ListModel::Joint(_) => None,
        }
    }
}

/// `tylist.c`'s statics.
#[derive(Clone, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct List {
    pub state: ListState,
    pub input: ListInput,
    /// `_tyList_804A2D84`: the scene's lights' GObj (and four unused).
    pub lights: Option<GObjId>,
    /// `_tyList_804D6EE8`, `_tyList_804D6EEC`: the rows' and the count's canvases.
    pub canvas: i32,
    pub count_canvas: i32,
}

impl Running<'_> {
    fn ls(&mut self) -> &mut ListState {
        &mut self.game.toys.list.state
    }

    fn li(&mut self) -> &mut ListInput {
        &mut self.game.toys.list.input
    }

    /// `_tyList_80312834(buf, n)`: `n`'s digits in font 0's digit glyphs (its table's string
    /// 0x13A), into string `buf` of the font's table: two or three digits with a zero kept
    /// between, the terminator after.
    #[doc(alias = "_tyList_80312834")]
    fn toy_list_digits(&mut self, buf: u32, n: u32) {
        let table = self.game.sis.font_tables[0].expect("font 0's strings");
        let digits = self
            .data
            .ptr(table.at(4 * 0x13A))
            .expect("the digits' string");
        let mut at = self
            .data
            .ptr(table.at(4 * buf))
            .expect("a row's count string");
        let glyph = |r: &Self, d: u32| {
            let g = digits.at(2 * d);
            (
                r.game.sis.sis_byte(r.data, SisPtr::Data(g)),
                r.game.sis.sis_byte(r.data, SisPtr::Data(g.at(1))),
            )
        };
        let mut put = |r: &mut Self, b: u8| {
            r.game.sis.set_sis_byte(SisPtr::Data(at), b);
            at = at.at(1);
        };
        let original = n;
        let mut n = n;
        if n >= 100 {
            let (hi, lo) = glyph(self, n / 100);
            put(self, hi);
            put(self, lo);
            n %= 100;
        }
        if n >= 10 {
            let (hi, lo) = glyph(self, n / 10);
            put(self, hi);
            put(self, lo);
            n %= 10;
        } else if original >= 100 {
            let (hi, lo) = glyph(self, 0);
            put(self, hi);
            put(self, lo);
        }
        let (hi, lo) = glyph(self, n);
        put(self, hi);
        put(self, lo);
        put(self, 0);
    }

    /// `_tyList_80312904(row, highlight)`: a row's texts placed by its base and its height,
    /// black on the highlighted slot and white otherwise (99 leaves the colors), its trophy's
    /// name, its count's glyphs; with ten trophies or fewer, the rows past the list aren't.
    #[doc(alias = "_tyList_80312904")]
    fn toy_list_row(&mut self, r: u8, highlight: i8) {
        let row = self.game.toys.list.state.rows[usize::from(r)];
        if row.idx == -1 {
            return;
        }
        let count = i32::from(self.game.toys.list.state.count);
        let show = if self.toy_total() < 0xB {
            !(row.slot == -1 || i32::from(row.slot) == count - 2)
        } else {
            true
        };
        if !show {
            return;
        }
        let base = row.jobjs[0].expect("a row's base");
        let root = self.toy_list_root();
        let j = &self.game.objs.jobjs;
        let x = mem(fp::fsubs(reg(j[base].translate.x), reg(6.5)));
        let y = mem(fp::fsubs(
            fp::fsubs(-reg(row.y), reg(j[root].translate.y)),
            reg(0.41),
        ));
        let z = j[root].translate.z;
        let [t0, t1, t2] = row.texts.map(|t| t.expect("a row's text"));
        if highlight != 0x63 {
            let c = if row.slot == highlight {
                [0, 0, 0, 0xFF]
            } else {
                [0xFF, 0xFF, 0xFF, 0xFF]
            };
            for t in [t0, t1, t2] {
                self.game.sis.text_mut(t).text_color = c;
            }
        }
        let tx = self.game.sis.text_mut(t0);
        tx.pos = Vec3::new(x, y, z);
        tx.font_size = [0.028, 0.029];
        tx.default_kerning = 1;
        let name = self.toy_string(i32::from(self.toy_list_trophy(row.idx)), 2, 0x128);
        self.game.sis.reset_text(self.data, t0, name);
        let tx = self.game.sis.text_mut(t1);
        tx.pos = Vec3::new(mem(fp::fadds(reg(14.7), reg(x))), y, z);
        tx.font_size = [0.028, 0.029];
        self.game.sis.reset_text(self.data, t1, 0x13B);
        let trophy = self.toy_list_trophy(row.idx);
        let n = (self.toy_flags()[trophy as usize] & 0xFF) as u32;
        self.toy_list_digits(0x12E + row.number as u32, n);
        let tx = self.game.sis.text_mut(t2);
        tx.default_alignment = 2;
        tx.default_kerning = 1;
        tx.pos = Vec3::new(mem(fp::fadds(reg(10.5), reg(x))), y, z);
        tx.font_size = [0.038, 0.029];
        self.game.sis.reset_text(self.data, t2, row.number + 0x12E);
    }

    /// The trophy at `idx` in the gallery's order (`Toy_80308354`'s check that the trophy
    /// has an entry).
    fn toy_list_trophy(&self, idx: i16) -> i16 {
        self.game.toys.gallery.order[idx as usize]
    }

    /// The list's model's root joint.
    fn toy_list_root(&self) -> JObjId {
        let g = self.game.toys.list.state.gobj.expect("the list's model");
        match self.game.world.gobj(g).obj {
            Some(Obj::JObj(j)) => j,
            _ => panic!("the list's GObj holds no model"),
        }
    }

    /// The panel's sort buttons (its parts 2 to 4): the sort's pressed, the others not, their
    /// texture animations stopped.
    fn toy_list_sort_buttons(&mut self, sort: i8) {
        for i in 0..3 {
            let j = self.game.toys.gallery.panel_parts[2 + i];
            let o = &mut self.game.objs;
            jobj::req_anim(o, j, if i as i8 == sort { 1.0 } else { 0.0 });
            let a = self.toy_tobj_aobj(j);
            crate::hsd::aobj::set_rate(&mut self.game.objs.aobjs, a, 0.0);
            jobj::anim(
                &mut self.game.objs,
                self.data,
                &mut super::super::code::ParticleHooks,
                j,
            );
        }
    }

    /// A joint's first display object's material's first texture's animation
    /// (`jobj->u.dobj->mobj->tobj->aobj`).
    pub(super) fn toy_tobj_aobj(&self, j: Option<JObjId>) -> Option<crate::hsd::aobj::AObjId> {
        let o = &self.game.objs;
        let jobj::Contents::DObjs(d) = o.jobjs[j?].u else {
            return None;
        };
        let m = o.dobjs[d?].mobj?;
        let t = o.mobjs[m].tobj?;
        o.tobjs[t].aobj
    }

    /// `_tyList_80312BAC(state, highlight)`: the gallery's order sorted again by the list's
    /// sort, the cursor's trophy found in it, the rows filled from the trophy before the
    /// first slot, the list's end after the last trophy, and the sort's buttons.
    #[doc(alias = "_tyList_80312BAC")]
    fn toy_list_fill(&mut self, highlight: i8) {
        let s = &self.game.toys.list.state;
        let old = self.game.toys.gallery.order[s.selected as usize];
        let (sort, dir) = (s.sort, s.sort_dir);
        self.toy_order(sort as usize, dir != 0);
        let sel = self.toy_order_index(old);
        self.ls().selected = sel;
        let total = self.toy_total();
        let mut idx = i32::from(sel);
        let slot = i32::from(
            self.game.toys.list.state.rows[usize::from(self.game.toys.list.state.cursor_row)].slot,
        );
        for _ in 0..slot + 1 {
            idx -= 1;
            if idx < 0 {
                idx = total - 1;
            }
        }
        let first = self.game.toys.list.state.first;
        if let Some(e) = self.game.toys.list.state.end {
            let y = self.game.toys.list.state.rows[usize::from(first)].y;
            jobj::set_srt(&mut self.game.objs.jobjs, e, |o| o.translate.y = y);
        }
        let mut r = first;
        let count = i32::from(self.game.toys.list.state.count);
        for _ in 0..count {
            self.ls().rows[usize::from(r)].idx = idx as i16;
            self.toy_list_row(r, highlight);
            self.toy_list_marks(r);
            let row = self.game.toys.list.state.rows[usize::from(r)];
            if row.slot >= 0
                && i32::from(row.slot) < count - 2
                && i32::from(row.idx) == self.toy_total() - 1
                && let Some(e) = self.game.toys.list.state.end
            {
                jobj::set_srt(&mut self.game.objs.jobjs, e, |o| o.translate.y = row.y);
            }
            r = row.next;
            idx += 1;
            if idx >= self.toy_total() {
                idx = 0;
            }
        }
        let sort = self.game.toys.list.state.sort;
        self.toy_list_sort_buttons(sort);
    }

    /// `Toy_803062BC(trophy)`: a trophy's place in the gallery's order (the count past the
    /// end if it isn't there).
    #[doc(alias = "Toy_803062BC")]
    fn toy_order_index(&self, trophy: i16) -> i16 {
        let o = &self.game.toys.gallery.order;
        (0..super::TROPHY_COUNT)
            .find(|&i| o[i] == trophy)
            .unwrap_or(super::TROPHY_COUNT) as i16
    }

    /// `_tyList_80312E88(row, delta)`: a row moved by `delta` toward its target (999 to it),
    /// its base and mark with it (and the list's end with the last trophy's), its texts and
    /// marks placed again.
    #[doc(alias = "_tyList_80312E88")]
    fn toy_list_move_row(&mut self, r: u8, delta: f32) {
        let s = self.ls();
        let row = &mut s.rows[usize::from(r)];
        row.y = mem(fp::fadds(reg(row.y), reg(delta)));
        if reg(delta) == reg(999.0) {
            row.y = row.target;
        } else if reg(delta) < 0.0 {
            if reg(row.y) < reg(row.target) {
                row.y = row.target;
            }
        } else if reg(row.y) > reg(row.target) {
            row.y = row.target;
        }
        let row = *row;
        for i in 0..2 {
            let Some(j) = row.jobjs[i] else { continue };
            jobj::set_srt(&mut self.game.objs.jobjs, j, |o| o.translate.y = row.y);
            if i32::from(row.idx) == self.toy_total() - 1
                && let Some(e) = self.game.toys.list.state.end
            {
                jobj::set_srt(&mut self.game.objs.jobjs, e, |o| o.translate.y = row.y);
            }
        }
        self.toy_list_row(r, 0x63);
        self.toy_list_marks(r);
    }

    /// `_tyList_8031305C(input, state, sound)`: a scroll's frame: each row (but the one
    /// leaving) moved a step; at its last, every row put at its target and shifted a slot,
    /// the row coming in given its trophy, the cursor's row with it, a page's count down,
    /// and at its end the cursor shown again (and a repeat's wait if X or Y is held). The
    /// frames left.
    #[doc(alias = "_tyList_8031305C")]
    fn toy_list_scroll(&mut self, sound: bool) -> i8 {
        if self.game.toys.list.state.x29f <= 0 {
            return self.game.toys.list.state.x29f;
        }
        let s = &self.game.toys.list.state;
        let mut delta = s.step;
        if s.x2a1 == 0 {
            delta = mem(fp::fmuls(reg(delta), -reg(1.0)));
        }
        let count = i32::from(s.count);
        for i in 0..count as usize {
            let s = &self.game.toys.list.state;
            let row = s.rows[i];
            if (s.x2a1 != 0 || row.slot != -1) && (s.x2a1 != 1 || count - 1 != i32::from(row.slot))
            {
                self.toy_list_move_row(i as u8, delta);
            }
        }
        self.ls().x29f -= 1;
        if self.game.toys.list.state.x29f == 0 {
            let mut r = 0u8;
            for _ in 0..count {
                self.toy_list_move_row(r, 999.0);
                let s = self.ls();
                let row = &mut s.rows[usize::from(r)];
                if s.x2a1 == 0 {
                    row.slot -= 1;
                    if row.slot < -1 {
                        row.slot = (count - 2) as i8;
                    }
                } else {
                    row.slot += 1;
                    if i32::from(row.slot) >= count - 1 {
                        row.slot = -1;
                    }
                }
                r = row.next;
            }
            let total = self.toy_total();
            let slot = self.game.toys.list.input.slot;
            if self.game.toys.list.state.x2a1 == 0 {
                let s = self.ls();
                let last_idx = s.rows[usize::from(s.last)].idx;
                let f = usize::from(s.first);
                s.rows[f].idx = last_idx + 1;
                if i32::from(s.rows[f].idx) >= total {
                    s.rows[f].idx = 0;
                }
                let first = self.game.toys.list.state.first;
                self.toy_list_row(first, slot);
                let s = self.ls();
                s.cursor_row = s.rows[usize::from(s.cursor_row)].next;
                s.selected = s.rows[usize::from(s.cursor_row)].idx;
                s.first = s.rows[usize::from(s.first)].next;
                s.last = s.rows[usize::from(s.last)].next;
            } else {
                let s = self.ls();
                let first_idx = s.rows[usize::from(s.first)].idx;
                let l = usize::from(s.last);
                s.rows[l].idx = first_idx - 1;
                if s.rows[l].idx < 0 {
                    s.rows[l].idx = (total - 1) as i16;
                }
                // `entry`, past the loop: the row after the last one shifted, the first.
                self.toy_list_row(r, slot);
                let s = self.ls();
                s.cursor_row = s.rows[usize::from(s.cursor_row)].prev;
                s.selected = s.rows[usize::from(s.cursor_row)].idx;
                s.first = s.rows[usize::from(s.first)].prev;
                s.last = s.rows[usize::from(s.last)].prev;
            }
            if self.game.toys.list.state.x29e > 0 {
                self.ls().x29e -= 1;
            }
            if self.game.toys.list.state.x29e == 0 {
                let c = self.game.toys.list.state.cursor;
                jobj::clear_flags_all(&mut self.game.objs.jobjs, c, jobj::HIDDEN);
                let s = &self.game.toys.list.state;
                let (row, slot) = (s.cursor_row, s.rows[usize::from(s.cursor_row)].slot);
                self.toy_list_row(row, slot);
                let held = self.game.pads.copy[0].button & 0xC00 != 0;
                self.ls().x2a0 = if held { 5 } else { 0 };
            }
            if sound {
                self.lb_audio_call(2);
            }
        }
        self.game.toys.list.state.x29f
    }

    /// `_tyList_80313358(state, page, frames, way)`: a scroll of `frames` frames (a page of
    /// `page` rows, the way it goes, unless `page` is -1): each row's target the height of
    /// the row it moves to, its texts placed again.
    #[doc(alias = "_tyList_80313358")]
    fn toy_list_start_scroll(&mut self, page: i8, frames: i8, way: i8) {
        let s = self.ls();
        if page != -1 {
            s.x29e = page;
            s.x2a1 = way;
        }
        s.x29f = frames;
        s.step = mem(fp::fdivs(reg(s.height), fp::frsp(f64::from(frames))));
        let count = s.count;
        for i in 0..count as usize {
            let s = self.ls();
            let to = if s.x2a1 == 0 {
                s.rows[i].prev
            } else {
                s.rows[i].next
            };
            s.rows[i].target = s.rows[usize::from(to)].y;
            self.toy_list_row(i as u8, count + 1);
        }
    }

    /// `_tyList_80313464(row)`: a row's series mark, and its new-trophy mark made again
    /// (`ToyFigureListMarkN_Top`) if its trophy is new.
    #[doc(alias = "_tyList_80313464")]
    fn toy_list_marks(&mut self, r: u8) {
        let row = self.game.toys.list.state.rows[usize::from(r)];
        let trophy = self.toy_list_trophy(row.idx);
        self.toy_mark(row.jobjs[2], i32::from(trophy));
        if let Some(m) = row.jobjs[1] {
            jobj::unref(&mut self.game.objs, Some(m));
            self.ls().rows[usize::from(r)].jobjs[1] = None;
        }
        if self.toy_flags()[trophy as usize] & 0x8000 != 0 {
            let g = self.game.toys.list.state.gobj;
            let m = self.toy_list_model(g, "ToyFigureListMarkN_Top_joint", [-8.4, row.y, 0.0]);
            self.ls().rows[usize::from(r)].jobjs[1] = m.left();
        }
    }

    /// `_tyList_80313508(parent, symbol, x, y, z)`: a model of `TyMnView`'s at (x, y, z)
    /// (unless all three are zero): the last of `parent`'s root's children, or without a
    /// parent, on a GObj of its own (class 6, link 7, drawn on link 0x39).
    #[doc(alias = "_tyList_80313508")]
    fn toy_list_model(&mut self, parent: Option<GObjId>, symbol: &str, at: [f32; 3]) -> ListModel {
        let a = self.game.toys.gallery.view_archive.expect("TyMnView").id;
        let joint = self
            .data
            .get(a)
            .public(symbol)
            .map(|offset| melee_data::archive::DataRef { archive: a, offset });
        let Some(joint) = joint else {
            panic!("*** Can't Get Public Address!(tyList Model)");
        };
        let data = self.data;
        let j = jobj::load_joint(&mut self.game.objs, data, Some(joint)).expect("a list model");
        if reg(at[0]) != 0.0 || reg(at[1]) != 0.0 || reg(at[2]) != 0.0 {
            let jobjs = &mut self.game.objs.jobjs;
            jobj::set_srt(jobjs, j, |o| o.translate.x = at[0]);
            jobj::set_srt(jobjs, j, |o| o.translate.y = at[1]);
            jobj::set_srt(jobjs, j, |o| o.translate.z = at[2]);
        }
        match parent {
            Some(p) => {
                let root = match self.game.world.gobj(p).obj {
                    Some(Obj::JObj(r)) => r,
                    _ => panic!("the list's parent holds no model"),
                };
                let jobjs = &mut self.game.objs.jobjs;
                jobj::add_child(jobjs, Some(root), Some(j));
                let mut c = jobjs[root].child.expect("a child");
                while let Some(n) = jobjs[c].next {
                    c = n;
                }
                ListModel::Joint(c)
            }
            None => {
                let g = self.game.world.create(6, 7, 0);
                let w = &mut self.game.world;
                w.set_obj(g, OBJ_JOBJ, Obj::JObj(j));
                w.setup_gx_link(g, CodeRef(JOBJ_CALLBACK), 0x39, 0);
                ListModel::GObj(g)
            }
        }
    }

    /// `_tyList_80313774`: the list from the table's selection, sort and cursor's slot: its
    /// rows (ten trophies or fewer: one each and two past them; else twelve) linked in a
    /// ring from the trophy before the gallery's, the list's model (`ToyFigurePanel_listpos01`)
    /// with each row's base and texts a row's height apart, its end and cursor, the cursor's
    /// slot scrolled to, and the rows filled.
    #[doc(alias = "_tyList_80313774")]
    fn toy_list_build(&mut self) {
        let t = &self.game.toys.table;
        let (sel, sort, dir, slot) = (t.selected, t.sort, t.sort_dir, t.x3 as i8);
        let s = self.ls();
        s.selected = sel;
        s.sort = sort;
        s.sort_dir = dir;
        let li = self.li();
        li.slot = slot;
        li.slot_before = slot;
        let total = self.toy_total();
        let count = if total <= 0xA { total + 2 } else { 0xC };
        self.ls().count = count as i8;
        let g = &self.game.toys.gallery;
        let prev = g.entries[usize::from(g.entries[usize::from(g.selected_entry)].prev)].trophy;
        let mut idx = i32::from(self.toy_order_index(prev));
        let n = count as u8;
        let s = self.ls();
        for i in 0..n {
            let r = &mut s.rows[usize::from(i)];
            r.prev = if i == 0 { n - 1 } else { i - 1 };
            r.next = if i == n - 1 { 0 } else { i + 1 };
        }
        s.cursor_row = 1;
        s.first = 0;
        s.last = n - 1;
        let root = self
            .toy_list_model(None, "ToyFigurePanel_listpos01_joint", [0.0, 0.0, 0.0])
            .right()
            .expect("the list's GObj");
        self.ls().gobj = Some(root);
        let rj = self.toy_list_root();
        let ty = self.game.objs.jobjs[rj].translate.y;
        let step = mem(fp::fsubs(reg(5.11), reg(ty)));
        let mut pos = mem(-reg(step));
        self.ls().height = step;
        let mut r = 0u8;
        let view = self.game.toys.gallery.view_archive.expect("TyMnView").id;
        let canvas = self.game.toys.list.canvas;
        for i in 0..count {
            let base = self
                .toy_list_model(Some(root), "ToyFigureListBase_Top_joint", [0.0, pos, 0.0])
                .left();
            let child = base.and_then(|b| self.game.objs.jobjs[b].child);
            self.toy_add_anims(
                base,
                [None, Some("ToyFigureListBase_Top_matanim_joint"), None],
                view,
                0,
            );
            let z = Vec3::new(0.0, 0.0, 17.2);
            let t0 = self.create_text(0, canvas, z, 640.0, 64.0);
            let t1 = self.create_text(0, canvas, z, 64.0, 64.0);
            let t2 = self.create_text(0, canvas, z, 192.0, 64.0);
            let s = self.ls();
            let row = &mut s.rows[usize::from(r)];
            row.number = i;
            row.jobjs[0] = base;
            row.jobjs[2] = child;
            row.texts = [Some(t0), Some(t1), Some(t2)];
            row.y = pos;
            row.slot = (i - 1) as i8;
            row.idx = idx as i16;
            idx += 1;
            if idx >= self.toy_total() {
                idx = 0;
            }
            pos = mem(fp::fadds(reg(pos), reg(step)));
            r = self.game.toys.list.state.rows[usize::from(r)].next;
        }
        let s = self.ls();
        s.selected = s.rows[usize::from(s.cursor_row)].idx;
        let y0 = s.rows[0].y;
        let end = self
            .toy_list_model(Some(root), "ToyFigureListBaseend_Top_joint", [0.0, y0, 0.0])
            .left();
        self.ls().end = end;
        let slot = self.game.toys.list.input.slot;
        let y = self.game.toys.list.state.rows[(slot + 1) as usize].y;
        let cursor = self
            .toy_list_model(Some(root), "ToyFigureListCursor_Top_joint", [0.0, y, 0.0])
            .left();
        self.ls().cursor = cursor;
        for _ in 0..slot {
            self.toy_list_start_scroll(0, 1, 1);
            self.toy_list_scroll(false);
        }
        let s = &self.game.toys.list.state;
        if let Some(i) = (0..usize::from(s.count as u8)).find(|&i| s.rows[i].slot == slot) {
            let s = self.ls();
            s.selected = s.rows[i].idx;
            s.cursor_row = i as u8;
        }
        self.toy_list_fill(slot);
    }

    /// The cursor's row chosen by its slot (`tyList_80313BD8_inline`): its trophy selected,
    /// the cursor moved to it with a sound, every row's texts placed again.
    fn toy_list_follow(&mut self) {
        let slot = self.game.toys.list.input.slot;
        for i in 0..self.game.toys.list.state.count as usize {
            let row = self.game.toys.list.state.rows[i];
            if row.slot == slot {
                let s = self.ls();
                s.selected = row.idx;
                s.cursor_row = i as u8;
                self.lb_audio_call(2);
                if let Some(c) = self.game.toys.list.state.cursor {
                    jobj::set_srt(&mut self.game.objs.jobjs, c, |o| o.translate.y = row.y);
                }
            }
            self.toy_list_row(i as u8, slot);
        }
    }

    /// `_tyList_80313BD8`, the list's input.
    #[doc(alias = "_tyList_80313BD8")]
    pub fn toy_list_input(&mut self, _gobj: GObjId) {
        let total = self.toy_total();
        if total > 10 {
            if self.toy_list_scroll(true) != 0 {
                return;
            }
            if self.game.toys.list.state.x2a0 != 0 {
                self.ls().x2a0 -= 1;
                return;
            }
            let e = i32::from(self.game.toys.list.state.x29e);
            if e != 0 {
                let d = i32::from(self.game.toys.list.state.x29d);
                let half = d / 2;
                let mut r = if d % 2 != 0 {
                    if e <= half { 5 - e } else { 4 - (d - e) }
                } else if e == half {
                    4 - (d - (e + 1))
                } else if e < half {
                    4 - e
                } else {
                    4 - (d - e)
                };
                if r < 2 {
                    r = 2;
                }
                self.toy_list_start_scroll(-1, r as i8, -1);
                return;
            }
        }
        let mut x = reg(self.toy_stick_x());
        let mut y = reg(self.toy_axis(1));
        if x < -reg(0.8) {
            self.li().x12 += 1;
            x = -1.0;
            y = 0.0;
        } else if x > reg(0.8) {
            self.li().x12 += 1;
            x = 1.0;
            y = 0.0;
        } else if y < -reg(0.6) || self.toy_held() & 4 != 0 {
            self.li().x12 += 1;
            x = 0.0;
            y = 1.0;
        } else if y > reg(0.6) || self.toy_held() & 8 != 0 {
            self.li().x12 += 1;
            x = 0.0;
            y = -1.0;
        } else {
            y = 0.0;
            x = y;
            let li = self.li();
            li.held = 0;
            li.x12 = 0;
            li.x14 = 6;
            li.x13 = 0;
            li.x11 = 0;
            li.x10 = 0;
        }
        if self.game.buttons_triggered(4) & super::super::mn::main::PAD_LR_START != 0 {
            self.lb_audio_call(0);
            let p = self.game.world.current_proc().expect("the list's process");
            self.game.world.remove_proc(p);
            self.toy_gallery_leave(false);
            self.toy_list_close(false);
            self.game.game_data.x1850.force_main_menu = 1;
            self.game.toys.table.exit = 1;
            return;
        }
        if self.toy_triggered() & 0x200 != 0 {
            self.lb_audio_call(0);
            let p = self.game.world.current_proc().expect("the list's process");
            self.game.world.remove_proc(p);
            self.toy_gallery_leave(false);
            self.toy_list_close(false);
            self.game.toys.table.exit = 1;
            return;
        }
        if self.toy_total() == 0 {
            return;
        }
        if self.toy_triggered() & 0x1100 != 0 {
            self.lb_audio_call(1);
            let p = self.game.world.current_proc().expect("the list's process");
            self.game.world.remove_proc(p);
            self.toy_gallery_leave(false);
            self.toy_list_close(true);
            self.toy_gallery();
            return;
        }
        let li = self.li();
        if li.x12 > li.x14 {
            li.x12 = li.x14 + 1;
            li.x14 = 3;
            li.held = 0;
        }
        let x10 = fp::frsp(f64::from(self.game.toys.list.input.x10));
        if (x < 0.0 && x != x10) || self.toy_triggered() & 0x41 != 0 {
            self.li().x10 = fp::fctiwz(x) as i8;
            let s = self.ls();
            s.sort -= 1;
            if s.sort < 0 {
                s.sort = 2;
            }
            let slot = self.game.toys.list.input.slot;
            self.toy_list_fill(slot);
            self.lb_audio_call(1);
            return;
        }
        if (x > 0.0 && x != x10) || self.toy_triggered() & 0x22 != 0 {
            self.li().x10 = fp::fctiwz(x) as i8;
            let s = self.ls();
            s.sort += 1;
            if s.sort >= 3 {
                s.sort = 0;
            }
            let slot = self.game.toys.list.input.slot;
            self.toy_list_fill(slot);
            self.lb_audio_call(1);
            return;
        }
        if self.toy_total() < 2 {
            return;
        }
        if self.toy_total() > 10 {
            if self.toy_held() & 0x400 != 0 {
                let c = self.game.toys.list.state.cursor;
                jobj::set_flags_all(&mut self.game.objs.jobjs, c, jobj::HIDDEN);
                let s = &self.game.toys.list.state;
                let last = s.rows[usize::from(s.last)];
                let before = s.rows[usize::from(last.prev)].idx;
                if last.idx == 0 || i32::from(before) + 9 < self.toy_total() {
                    self.toy_list_start_scroll(9, 4, 0);
                } else {
                    let n = (self.toy_total() - i32::from(last.idx)) as i8;
                    self.toy_list_start_scroll(n, 4, 0);
                }
                let s = self.ls();
                s.x29d = s.x29e;
                return;
            }
            if self.toy_held() & 0x800 != 0 {
                let c = self.game.toys.list.state.cursor;
                jobj::set_flags_all(&mut self.game.objs.jobjs, c, jobj::HIDDEN);
                let s = &self.game.toys.list.state;
                let first = s.rows[usize::from(s.first)];
                let after = s.rows[usize::from(first.next)].idx;
                if i32::from(first.idx) == self.toy_total() - 1 || i32::from(after) - 9 > 0 {
                    self.toy_list_start_scroll(9, 4, 1);
                } else {
                    self.toy_list_start_scroll(after as i8, 4, 1);
                }
                let s = self.ls();
                s.x29d = s.x29e;
                return;
            }
        }
        if y == 0.0 {
            return;
        }
        if self.game.toys.list.input.held != 0 {
            return;
        }
        let count = i32::from(self.game.toys.list.state.count);
        let li = self.li();
        li.held = 1;
        li.slot = fp::fctiwz(fp::fadds(fp::frsp(f64::from(li.slot)), y)) as i8;
        if li.slot < 0 {
            li.slot = 0;
        } else if i32::from(li.slot) > count - 3 {
            li.slot = (count - 3) as i8;
        }
        if li.slot != li.slot_before {
            li.x12 = 0;
            li.x13 = 0;
            li.x11 = fp::fctiwz(y) as i8;
            self.toy_list_follow();
            let li = self.li();
            li.slot_before = li.slot;
            return;
        }
        if self.toy_total() <= 10 {
            return;
        }
        let li = self.li();
        if li.x11 == 0 {
            li.x13 = 5;
            li.x12 = li.x14 + 1;
            li.x14 = 3;
            li.held = 0;
        }
        li.x11 = fp::fctiwz(y) as i8;
        let v = li.x13;
        if v == 0 {
            li.x13 = 1;
            return;
        }
        if v < 5 {
            li.x13 = v + 1;
            return;
        }
        for i in 0..count as u8 {
            self.toy_list_row(i, count as i8);
        }
        let c = self.game.toys.list.state.cursor;
        jobj::set_flags_all(&mut self.game.objs.jobjs, c, jobj::HIDDEN);
        if y > 0.0 {
            self.toy_list_start_scroll(1, 6, 0);
        } else {
            self.toy_list_start_scroll(1, 6, 1);
        }
    }

    /// `_tyList_8031438C`, the list's intro: 26 frames of the panel's sort buttons animating
    /// in, then on the last the texts' camera drawn (on links 0x39 and 0x3C... as its links
    /// say) and the count's text; then the input's process.
    #[doc(alias = "_tyList_8031438C")]
    pub fn toy_list_intro(&mut self, gobj: GObjId) {
        let intro = self.game.toys.list.input.intro;
        if intro != 0 {
            if intro > 1 {
                let sort = self.game.toys.list.state.sort;
                for i in 0..3 {
                    let j = self.game.toys.gallery.panel_parts[2 + i];
                    jobj::req_anim(
                        &mut self.game.objs,
                        j,
                        if i as i8 == sort { 1.0 } else { 0.0 },
                    );
                    // `archive->jobjs[0]`'s texture animation, each time.
                    let a = self.toy_tobj_aobj(self.game.toys.gallery.panel_parts[2]);
                    crate::hsd::aobj::set_rate(&mut self.game.objs.aobjs, a, 0.0);
                }
                let panel = self.game.toys.gallery.panel.expect("the panel");
                let root = match self.game.world.gobj(panel).obj {
                    Some(Obj::JObj(j)) => Some(j),
                    _ => None,
                };
                jobj::anim_all(
                    &mut self.game.objs,
                    self.data,
                    &mut super::super::code::ParticleHooks,
                    root,
                );
            } else {
                if let Some(c) = self.game.toys.list.input.texts_camera {
                    self.game.world.gobj_mut(c).gxlink_prios = 0x4210_0000_0000_0000;
                }
                let cc = self.game.toys.list.count_canvas;
                let t = self.create_text_buffer(3, cc);
                let tx = self.game.sis.text_mut(t);
                tx.pos.z = 17.2;
                tx.font_size = [0.038, 0.029];
                tx.default_kerning = 1;
                tx.default_alignment = 2;
                self.ls().count_text = Some(t);
                let total = self.toy_total();
                let mut out = [0u8; 16];
                let n = crate::hsd::sis::printf(
                    b"%d",
                    &[crate::hsd::sis::FmtArg::Int(total)],
                    &mut out,
                );
                self.game.sis.add_string(t, 290.0, 320.0, Some(&out[..n]));
            }
            self.li().intro -= 1;
            return;
        }
        self.game.world.remove_all_procs(gobj);
        let cam = self.game.toys.list.input.camera.expect("the list's camera");
        self.game.world.setup_proc(cam, CodeRef(PROC_INPUT), 0);
        self.game.world.mark_ran(cam);
    }

    /// `_tyList_80314504`, the texts' camera's render: the screen erased to black, then its
    /// links.
    #[doc(alias = "_tyList_80314504")]
    pub fn toy_list_texts_render(&mut self, gobj: GObjId) {
        // `data->cobj` is the GObj's `hsd_obj` (`TyListData` over the GObj).
        let c = match self.game.world.gobj(gobj).obj {
            Some(Obj::CObj(c)) => c,
            _ => panic!("the list's texts' camera has no CObj"),
        };
        if crate::hsd::display::camera_begin(&mut self.game.objs, self.data, Some(c)) {
            let d = &mut self.game.objs.display.draws;
            d.push(crate::hsd::display::Draw::EraseColor([0, 0, 0, 0xFF]));
            d.push(crate::hsd::display::Draw::Erase);
            self.render_links(gobj, 7);
            crate::hsd::display::fog_set(&mut self.game.objs, None);
            crate::hsd::display::camera_end(&mut self.game.objs);
        }
    }

    /// `_tyList_8031457C`: the list's two cameras (`TyMnView`'s camera): the panel's
    /// (`Toy_80306954`'s render, the intro's process) and the texts' (a viewport over the
    /// panel, its scissor as tall as the trophies' rows), each with its texts' canvas.
    #[doc(alias = "_tyList_8031457C")]
    fn toy_list_cameras(&mut self) {
        let a = self.game.toys.gallery.view_archive.expect("TyMnView").id;
        let desc = self
            .data
            .get(a)
            .public("ScMenFigure_cam_int1_camera")
            .map(|offset| melee_data::archive::DataRef { archive: a, offset });
        let data = self.data;
        if let Some(desc) = desc {
            let g = self.game.world.create(1, 2, 0);
            self.li().camera = Some(g);
            let c = crate::lb::cobj::load_cobj(
                &mut self.game.objs,
                data,
                crate::lb::cobj::CObjDesc::Data(desc),
            );
            let w = &mut self.game.world;
            w.set_obj(g, OBJ_CAMERA, Obj::CObj(c));
            w.setup_gx_link_max(g, CodeRef(super::gallery::RENDER_MAIN), 0, false);
            w.gobj_mut(g).gxlink_prios = 0x9010_0000_0000_0000;
            w.setup_proc(g, CodeRef(PROC_INTRO), 0);
            w.mark_ran(g);
            self.li().intro = 0x1A;
        }
        let cam = self.game.toys.list.input.camera;
        let n = self.add_canvas(3, cam, 0xC, 0xC, 0, 0x3F, 0, 0);
        self.game.toys.list.count_canvas = n;
        if let Some(desc) = desc {
            let g = self.game.world.create(1, 2, 0);
            self.li().texts_camera = Some(g);
            let c = crate::lb::cobj::load_cobj(
                &mut self.game.objs,
                data,
                crate::lb::cobj::CObjDesc::Data(desc),
            );
            let w = &mut self.game.world;
            w.set_obj(g, OBJ_CAMERA, Obj::CObj(c));
            w.setup_gx_link_max(g, CodeRef(RENDER_TEXTS), 0, false);
            w.gobj_mut(g).gxlink_prios = 0x0210_0000_0000_0000;
            let o = &mut self.game.objs;
            cobj::set_interest(o, c, Vec3::new(1.1, -0.24, 0.0));
            cobj::set_eye_position(o, c, Vec3::new(1.1, -0.24, 40.059_63));
            let mut vp = [0x76i16, 0x230, 0x4E, 0x19C];
            cobj::set_viewport(&mut o.cobjs, Some(c), vp);
            vp[2] = 0x60;
            let total = self.toy_total();
            vp[3] = if total < 10 {
                (total * 0x1E + 0x60) as i16
            } else {
                0x18A
            };
            cobj::set_scissor(
                &mut self.game.objs.cobjs,
                Some(c),
                [vp[0] as u16, vp[1] as u16, vp[2] as u16, vp[3] as u16],
            );
            let n = self.add_canvas(0, Some(g), 0xB, 0xB, 0, 0x3E, 0, 0);
            self.game.toys.list.canvas = n;
        }
    }

    /// `tyList_803147C4`: the list: its cameras, the scene's lights (on link 0x34), the
    /// panel again, the rows if there are trophies, and the pads renewed.
    #[doc(alias = "tyList_803147C4")]
    pub fn toy_list_open(&mut self) {
        self.game.toys.list.input = ListInput::default();
        self.toy_list_cameras();
        self.game.toys.list.lights = None;
        let a = self
            .game
            .toys
            .gallery
            .view_archive
            .unwrap_or_else(|| panic!("*** BG data aren't being loaded!"));
        let lights = self
            .data
            .get(a.id)
            .public("ScMenFigure_scene_lights")
            .map(|offset| melee_data::archive::DataRef {
                archive: a.id,
                offset,
            });
        if let Some(list) = lights {
            let g = self.game.world.create(2, 3, 0);
            self.game.toys.list.lights = Some(g);
            let (l, _) = self.toy_load_lights(list, false);
            let w = &mut self.game.world;
            if let Some(l) = l {
                w.set_obj(g, OBJ_LIGHT, Obj::LObj(l));
            }
            w.setup_gx_link(g, CodeRef(LOBJ_CALLBACK), 52, 0);
        }
        self.toy_panel();
        if self.toy_total() != 0 {
            self.game.toys.list.state = ListState::default();
            self.toy_list_build();
        }
        self.pads_renew_status();
    }

    /// `_tyList_803148E4(chosen)`: the list's selection back to the table (and, unless a
    /// trophy was chosen, the gallery's order back to its first sort and the table's sort
    /// with it), the selection's new mark cleared; its GObjs freed (only when `chosen`; the
    /// scene's end frees the rest) and its texts too.
    #[doc(alias = "_tyList_803148E4")]
    fn toy_list_close(&mut self, chosen: bool) {
        if self.toy_total() != 0 {
            if !chosen {
                let sel = self.game.toys.list.state.selected;
                let t = self.game.toys.gallery.order[sel as usize];
                self.toy_order(0, false);
                self.ls().selected = self.toy_order_index(t);
            }
            let s = &self.game.toys.list.state;
            let (sel, sort, dir) = (s.selected, s.sort, s.sort_dir);
            let trophy = self.game.toys.gallery.order[sel as usize];
            let slot = self.game.toys.list.input.slot;
            let t = &mut self.game.toys.table;
            t.selected = sel;
            t.selected_trophy = trophy;
            if chosen {
                t.sort = sort;
                t.sort_dir = dir;
                t.x3 = slot as u8;
            } else {
                t.sort = 0;
                t.sort_dir = 0;
                t.x3 = 0;
            }
            let f = &mut self.toy_flags_mut()[trophy as usize];
            if *f & 0x8000 != 0 {
                *f ^= 0x8000;
            }
        }
        if let Some(g) = self.game.toys.list.state.gobj.take()
            && chosen
        {
            gobj::free(self, g);
        }
        if chosen {
            self.sis_reset();
        }
        if let Some(g) = self.game.toys.gallery.panel.take()
            && chosen
        {
            gobj::free(self, g);
        }
        if let Some(g) = self.game.toys.list.lights
            && chosen
        {
            gobj::free(self, g);
        }
        if let Some(g) = self.game.toys.list.input.camera {
            self.game.world.remove_all_procs(g);
            if chosen {
                gobj::free(self, g);
            }
        }
        if let Some(g) = self.game.toys.list.input.texts_camera
            && chosen
        {
            gobj::free(self, g);
        }
    }
}
