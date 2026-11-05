//! The single-player intros' statics (`melee_sim::game::gm::intro`), compared with memory:
//! Classic's intro's state (`lbl_804735A8`), its enter data and what it shows
//! (`lbl_8047368C`), its archives, scenes, table and announcer's call.

use melee_sim::game::mirror::Cx;

use crate::compare::Comparer;
use crate::import::sym;

impl Comparer<'_> {
    /// The intros' statics.
    pub(crate) fn intros(&mut self) {
        let e = self.game.intros.easy;
        let at = sym("lbl_804735A8");
        self.check("lbl_804735A8", "x0", e.done, self.u32(at) as i32);
        for (k, j) in e.joints.iter().enumerate() {
            let theirs = self.u32(at + 4 + 4 * k as u32);
            self.compare_handle(
                &format!("lbl_804735A8.x4[{k}]"),
                "JObjId",
                j.map(|j| j.0),
                theirs,
            );
        }
        for (k, t) in e.texts.iter().enumerate() {
            let theirs = self.u32(at + 0x1C + 4 * k as u32);
            self.compare_handle(
                &format!("lbl_804735A8.x4[{}]", k + 6),
                "TextId",
                t.map(|t| t.0),
                theirs,
            );
        }
        self.check(
            "lbl_804735A8",
            "x38 to x3E",
            [e.tick, e.map_frame, e.map_frame2, e.x3e],
            [0x38, 0x3A, 0x3C, 0x3E].map(|o| self.u16(at + o)),
        );
        let at = sym("lbl_8047368C");
        let theirs: Vec<u8> = (0..0x24).map(|i| self.u8(at + i)).collect();
        self.check("lbl_8047368C", "bytes", e.data.bytes().to_vec(), theirs);
        for (name, a) in [("lbl_804D65F4", e.archive), ("lbl_804D65F8", e.map_archive)] {
            let theirs = self.u32(sym(name));
            self.check(name, "archive", a.block, theirs);
        }
        for (name, d) in [
            ("lbl_804D65FC", e.scene),
            ("lbl_804D6600", e.map_scene),
            ("lbl_804D6604", e.table),
        ] {
            self.compare_data(name, d, self.u32(sym(name)));
        }
        let theirs = self.u32(sym("lbl_804D6608")) as i32;
        self.check("lbl_804D6608", "the call's sound", e.call, theirs);
    }
}
