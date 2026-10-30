//! The trophy gallery's debug code (`toy.c`), reached only from the debug ROM's level: the
//! unlock editor that R held at the gallery's entry opens (`_Toy_803114E8`), a debug text of
//! the nine classes' counts that, once confirmed, takes every trophy and gives each class as
//! many as its count says before the gallery opens.

use super::super::handles::CodeRef;
use super::super::state::Running;
use super::TROPHY_COUNT;
use crate::fpu::reg;
use crate::hsd::gobj::{self, GObjId};
use crate::msl::Arg;

/// `_Toy_80310B48`: the unlock editor's process.
pub const PROC_EDITOR: u32 = 0x8031_0B48;

/// `_Toy_803B8910`: each class's letter in the editor (`Rea_%c`).
const LETTERS: [(i32, u8); 9] = [
    (7, b'A'),
    (6, b'B'),
    (5, b'C'),
    (4, b'D'),
    (3, b'E'),
    (2, b'F'),
    (0, b'I'),
    (1, b'X'),
    (8, b'S'),
];

/// `ToyParamEditor` (`_Toy_sbss_804D6E5C`, a 0x18-byte heap block): its GObj, the class
/// under the cursor, the frames before the stick repeats, and each class's count.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Editor {
    pub block: u32,
    pub gobj: Option<GObjId>,
    pub slot: u8,
    pub delay: u8,
    pub values: [i16; 9],
}

impl Running<'_> {
    fn editor(&mut self) -> &mut Editor {
        self.game
            .toys
            .gallery
            .editor
            .as_mut()
            .expect("the unlock editor")
    }

    fn editor_text(&mut self) -> &mut super::super::hud::devtext::DevText {
        let t = self
            .game
            .toys
            .gallery
            .editor_text
            .expect("the editor's text");
        &mut self.game.hud.devtext.pool[usize::from(t)]
    }

    /// `_Toy_803114E8`: the unlock editor: its block, its text (id 1, at 40, 40, 14 by 9
    /// cells, blue, light grey with the cursor's line orange) with each class's count at 0,
    /// and its GObj with the editor's process.
    #[doc(alias = "_Toy_803114E8")]
    pub(super) fn toy_editor(&mut self) {
        let block = crate::hsd::heap::mem_alloc(0x18);
        let t = self.game.hud.devtext.create(1, 0x28, 0x28, 0xE, 9);
        self.game.toys.gallery.editor_text = t;
        self.game.toys.gallery.editor = Some(Editor {
            block,
            ..Editor::default()
        });
        let Some(t) = t else {
            // `OSReport("*** Couldn't Open Debug Window2!")`; the block keeps what the heap
            // left, which nothing reads.
            return;
        };
        self.game.hud.devtext.show(t);
        let text = self.editor_text();
        text.hide_cursor();
        text.no_wrap();
        text.set_bg_color([0x40, 0x80, 0xD0, 0x60]);
        text.set_scale(12.0, 18.0);
        text.erase();
        text.set_cursor_xy(0, 0);
        text.store_color_index(0);
        text.set_text_color([0xE2, 0xE2, 0xE2, 0xFF]);
        text.store_color_index(1);
        text.set_text_color([0xFF, 0x80, 0x20, 0xFF]);
        for i in 0..9 {
            self.toy_editor_line(i, 0, i == 0);
        }
        let g = self.game.world.create(0, 0, 0);
        self.editor().gobj = Some(g);
        self.game.world.setup_proc(g, CodeRef(PROC_EDITOR), 0);
        self.game.world.mark_ran(g);
    }

    /// `_Toy_803109A0(class, count, cursor)`: a class's line, `Rea_` with its letter, its
    /// count and how many it has, orange under the cursor (the last without a line break).
    #[doc(alias = "_Toy_803109A0")]
    fn toy_editor_line(&mut self, class: i32, count: i32, cursor: bool) {
        let letter = LETTERS
            .iter()
            .find(|e| e.0 == class)
            .expect("a class's letter")
            .1;
        self.editor_text().store_color_index(u8::from(cursor));
        let size = crate::game::gm::trophies::class_size(class);
        let format: &[u8] = if class == 8 {
            b"Rea_%c %3d/%3d"
        } else {
            b"Rea_%c %3d/%3d\n"
        };
        let line = crate::msl::vsprintf(
            format,
            &[Arg::Int(i32::from(letter)), Arg::Int(count), Arg::Int(size)],
        );
        // `DevText_Printf(text, buf)`: the line is the format, with nothing to convert.
        self.editor_text().printf(&line, &[]);
    }

    /// `_Toy_80310B48`, the unlock editor's process: the stick's x and y (the first port
    /// tilted past 0.1, else the fourth's) beyond 0.6 step a count or the cursor, repeating
    /// every 4 or 8 frames; B leaves the scene; A or Start take every trophy, give each class
    /// its count (in order with Start, at random with A) and open the gallery; Y or D-pad left
    /// add one, X or D-pad right take one.
    #[doc(alias = "_Toy_80310B48")]
    pub fn toy_editor_proc(&mut self, gobj: GObjId) {
        let pads = &self.game.pads.copy;
        let mut stick_x = 0.0;
        for p in pads.iter().take(4) {
            stick_x = p.nml_stick_x;
            if reg(stick_x).abs() > reg(0.1) {
                break;
            }
        }
        let mut stick_y = 0.0;
        let mut port = None;
        for (i, p) in pads.iter().take(4).enumerate() {
            stick_y = p.nml_stick_y;
            if reg(stick_y).abs() > reg(0.1) {
                port = Some(i);
                break;
            }
        }
        if let Some(i) = port {
            self.game.menu.confirm_port = i as i8;
        }
        let (sx, sy) = (reg(stick_x), reg(stick_y));
        let (dir_x, dir_y): (f64, f64) = if sx < reg(-0.6) {
            (1.0, 0.0)
        } else if sx > reg(0.6) {
            (-1.0, 0.0)
        } else if sy < reg(-0.6) {
            (0.0, 1.0)
        } else if sy > reg(0.6) {
            (0.0, -1.0)
        } else {
            self.editor().delay = 0;
            (0.0, 0.0)
        };
        let e = self.editor();
        if e.delay as i8 != 0 {
            e.delay = e.delay.wrapping_sub(1);
            return;
        }
        if self.toy_triggered() & 0x200 != 0 {
            self.lb_audio_call(0);
            gobj::free(self, gobj);
            self.editor().gobj = None;
            self.game.toys.table.exit = 1;
            return;
        }
        if self.toy_triggered() & 0x1100 != 0 {
            self.lb_audio_call(1);
            self.toy_take_all();
            for slot in 0..9 {
                let count = self.editor().values[slot as usize];
                if count != 0 {
                    self.toy_mark_class(slot, false, false);
                    let random = self.toy_triggered() & 0x1000 == 0;
                    self.toy_give_class(slot, i32::from(count), random);
                } else if slot == 2 {
                    self.toy_mark_class(slot, false, false);
                }
            }
            self.toy_mark_class(0, true, false);
            self.toy_mark_class(1, true, false);
            self.toy_mark_class(8, true, false);
            self.toy_mark_class(3, true, false);
            let text = self.editor_text();
            text.hide_background();
            text.hide_text();
            self.toy_gallery();
            gobj::free(self, gobj);
            self.editor().gobj = None;
            return;
        }
        let mut changed = false;
        if dir_x > 0.0 || self.toy_held() & 0x800 != 0 || self.toy_triggered() & 0x1 != 0 {
            self.lb_audio_call(2);
            let slot = self.editor_slot();
            let e = self.editor();
            e.values[slot] = e.values[slot].wrapping_add(1);
            let size = crate::game::gm::trophies::class_size(slot as i32);
            let e = self.editor();
            if i32::from(e.values[slot]) > size {
                e.values[slot] = size as i16;
            }
            e.delay = 4;
            changed = true;
        }
        if dir_x < 0.0 || self.toy_held() & 0x400 != 0 || self.toy_triggered() & 0x2 != 0 {
            self.lb_audio_call(2);
            let slot = self.editor_slot();
            let e = self.editor();
            e.values[slot] = e.values[slot].wrapping_sub(1);
            if e.values[slot] < 0 {
                e.values[slot] = 0;
            }
            e.delay = 4;
            changed = true;
        }
        if dir_x == 0.0 && dir_y != 0.0 {
            self.lb_audio_call(2);
            let e = self.editor();
            // `(u8)(s32)((f32)(s8) slot + dirY)`.
            let moved = gekko_fp::fctiwz(gekko_fp::fadds(
                gekko_fp::frsp(f64::from(e.slot as i8)),
                dir_y,
            ));
            e.slot = (moved as u8 as i8).clamp(0, 9) as u8;
            e.delay = 8;
            changed = true;
        }
        if changed {
            let text = self.editor_text();
            text.erase();
            text.set_cursor_xy(0, 0);
            for i in 0..9 {
                let e = *self.editor();
                self.toy_editor_line(
                    i,
                    i32::from(e.values[i as usize]),
                    i == i32::from(e.slot as i8),
                );
            }
        }
    }

    /// The class under the cursor, as an index of the counts. The cursor goes one past the
    /// last class (9), where the original's count is the halfword past the editor's block
    /// and the class's size an unset register ([Melee's bugs](../../../../docs/melee-bugs.md)):
    /// the port stops there.
    fn editor_slot(&mut self) -> usize {
        let slot = self.editor().slot as i8;
        if slot as usize >= 9 {
            crate::game::code::unported_call(
                "the trophy unlock editor's count past its last class (_Toy_80310B48)",
            );
        }
        slot as usize
    }

    /// `Toy_80311960`: every trophy taken, from the save and the screens' table alike.
    fn toy_take_all(&mut self) {
        let s = &mut self.game.game_data.thing.save_data;
        s.trophy_flags = [0; TROPHY_COUNT];
        s.trophy_category_flags = 0;
        s.trophy_count = 0;
        self.toy_reset_table();
    }

    /// `_Toy_803053C4(class, count, random)`: `count` trophies of `class` the region can win
    /// given, each from 1 to 254 of it (`Toy_SetUnlockState`) and not flagged new: with
    /// `random`, each one not held yet on a coin toss, pass after pass over the trophies
    /// until `count` are given; else the first `count` in order.
    #[doc(alias = "_Toy_803053C4")]
    fn toy_give_class(&mut self, class: i32, mut count: i32, random: bool) {
        let candidate =
            |r: &Self, t: usize| r.toy_winnable(t as i32) && r.toy_param_int(t as i32, 6) == class;
        if random {
            while count != 0 {
                // A pass that can give nothing never ends: the original hangs there.
                if !(0..TROPHY_COUNT).any(|t| candidate(self, t) && self.toy_flags()[t] & 0xFF == 0)
                {
                    panic!(
                        "the trophy unlock editor hangs: class {class} has fewer trophies to give than {count} more"
                    );
                }
                for t in 0..TROPHY_COUNT {
                    if candidate(self, t) {
                        if self.game.objs.rng.below(2) == 0 && self.toy_flags()[t] & 0xFF == 0 {
                            let add = self.game.objs.rng.below(0xFE) + 1;
                            self.add_trophy(t as i32, add);
                            count -= 1;
                            self.toy_flags_mut()[t] ^= 0x8000;
                        }
                        if count == 0 {
                            break;
                        }
                    }
                }
            }
        } else {
            for t in 0..TROPHY_COUNT {
                if candidate(self, t) {
                    let add = self.game.objs.rng.below(0xFE) + 1;
                    self.add_trophy(t as i32, add);
                    count -= 1;
                    self.toy_flags_mut()[t] ^= 0x8000;
                    if count == 0 {
                        return;
                    }
                }
            }
        }
    }

    /// `_Toy_80311F5C`'s debug part: the Z view's text and the editor's text let go, and the
    /// editor's block freed.
    pub(super) fn toy_debug_leave(&mut self) {
        use crate::game::hud::devtext::Link;
        if self.game.toys.gallery.debug_z != 0
            && let Some(t) = self.game.toys.gallery.debug_text.take()
        {
            self.game.hud.devtext.remove(Link::Held(t));
        }
        if let Some(t) = self.game.toys.gallery.editor_text.take() {
            self.game.hud.devtext.remove(Link::Held(t));
        }
        if let Some(e) = self.game.toys.gallery.editor.take() {
            crate::hsd::heap::mem_free(e.block);
        }
    }
}
