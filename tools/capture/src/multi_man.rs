//! Multi-Man Melee's statics (`melee_sim::game::gm::multi_man`, `gmmultiman.c`'s and
//! `gm_181A.c`'s), compared with memory.

use melee_sim::game::mirror::{Cx, Mirror};
use melee_sim::game::structs::PlayerInitData;

use crate::compare::Comparer;

/// `gm_804950F0`, `gm_80495238`, `gm_80495370`: the select's data, the match's start and its
/// exit data; `gm_804D68F0` and `gm_804D68F1`.
const CSS: u32 = 0x8049_50F0;
const START: u32 = 0x8049_5238;
const EXIT: u32 = 0x8049_5370;
const SLOT: u32 = 0x804D_68F0;
const ENTRIES: u32 = 0x804D_68F1;
/// `lbl_80472ED8` (`x54` the table, `x6A4` the six tables' addresses), `lbl_80473594`,
/// `lbl_803D8D08`, `lbl_804D65E0`.
const HORDES: u32 = 0x8047_2ED8;
const RECORD: u32 = 0x8047_3594;
const BLOCKS: u32 = 0x803D_8D08;
const RESULT: u32 = 0x804D_65E0;

impl Comparer<'_> {
    /// Multi-Man Melee's statics.
    pub(crate) fn multi_man(&mut self) {
        let mm = &self.game.multi_man;
        self.one_player_css("gm_804950F0", &mm.css, CSS);
        let m = self.mem_view();
        mm.start.compare(&m, START, "gm_80495238", self);
        self.match_exit_info("gm_80495370", &mm.exit, EXIT);
        self.check(
            "gm_804D68F0",
            "port, count",
            (mm.slot, mm.entries),
            (self.u8(SLOT), self.u8(ENTRIES) as i8),
        );
        let a = HORDES;
        self.check(
            "lbl_80472ED8",
            "x0, x4, x8",
            (mm.x0, mm.x4, mm.x8),
            (
                self.u32(a) as i32,
                self.u32(a + 4) as i32,
                self.u32(a + 8) as i32,
            ),
        );
        let m = self.mem_view();
        mm.xC.compare(&m, a + 0xC, "lbl_80472ED8.xC", self);
        // `x30` to `x53`: nothing writes them.
        let theirs: Vec<u8> = (0x30..0x54).map(|i| self.u8(a + i)).collect();
        self.check("lbl_80472ED8", "x30 to x53", vec![0u8; 0x24], theirs);
        for (i, e) in mm.x54.iter().enumerate() {
            let at = a + 0x54 + 0x10 * i as u32;
            let theirs = (
                self.u32(at) as i32,
                [
                    self.u8(at + 4),
                    self.u8(at + 5),
                    self.u8(at + 6),
                    self.u8(at + 7),
                ],
                self.u32(at + 8),
                self.u32(at + 0xC),
            );
            let ours = (
                e.x0,
                [e.x4, e.x5, e.x6, e.x7],
                e.x8.to_bits(),
                e.xC.to_bits(),
            );
            self.check("lbl_80472ED8.x54", &format!("[{i}]"), ours, theirs);
        }
        // The tables live in `GmKumite.dat`, which the match's scene loads: past it the words
        // name its freed heap in both, and only the next match's start sets them again.
        let live = melee_sim::game::gm::machine::scene_kind(self.game.machine.scene_info)
            == Some(melee_sim::game::gm::machine::GS_VS)
            && melee_sim::game::gm::multi_man::is_multi_man(self.game.game_mode);
        if live && mm.tables[0].is_some() {
            for (i, t) in mm.tables.into_iter().enumerate() {
                let theirs = self.u32(a + 0x6A4 + 4 * i as u32);
                self.compare_data(&format!("lbl_80472ED8.x6A4[{i}]"), t, theirs);
            }
        }
        let r = mm.record;
        let theirs = (
            self.u8(RECORD),
            self.u16(RECORD + 2),
            self.u32(RECORD + 4) as i32,
            self.u32(RECORD + 8) as i32,
            self.u32(RECORD + 0xC) as i32,
            self.u8(RECORD + 0x10) as i8,
            self.u8(RECORD + 0x11),
        );
        self.check(
            "lbl_80473594",
            "x0, x2, x4, x8, xC, x10, x11",
            (r.x0, r.x2, r.x4, r.x8, r.xC, r.x10, r.x11),
            theirs,
        );
        for (b, blk) in mm.blocks.iter().enumerate() {
            let at = BLOCKS + 0xC0 * b as u32;
            let theirs: [u32; 27] = core::array::from_fn(|i| self.u32(at + 4 * i as u32));
            self.check("lbl_803D8D08", &format!("[{b}].scores"), blk.scores, theirs);
            let theirs: [u8; 28] = core::array::from_fn(|i| self.u8(at + 0x6C + i as u32));
            self.check("lbl_803D8D08", &format!("[{b}].icons"), blk.icons, theirs);
            let theirs: [u16; 28] = core::array::from_fn(|i| self.u16(at + 0x88 + 2 * i as u32));
            self.check("lbl_803D8D08", &format!("[{b}].times"), blk.times, theirs);
        }
        let x = mm.result;
        self.check(
            "lbl_804D65E0",
            "x0, x2, x4",
            (x.x0, x.x2, x.x4),
            (
                self.u8(RESULT),
                self.u16(RESULT + 2),
                self.u32(RESULT + 4) as i32,
            ),
        );
        let _ = PlayerInitData::SIZE;
    }
}
