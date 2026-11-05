//! All-Star's intro's statics (`melee_sim::game::gm::intro_allstar`), compared with memory: its
//! state (`lbl_804736B0`), its enter data (`lbl_804D6618`), its archive and scene
//! (`lbl_804D6610`, `lbl_804D6614`).

use melee_sim::game::mirror::Cx;

use crate::compare::Comparer;
use crate::import::sym;

impl Comparer<'_> {
    /// All-Star's intro's statics.
    pub(crate) fn intro_allstar(&mut self) {
        let s = self.game.intro_allstar;
        let at = sym("lbl_804736B0");
        self.check(
            "lbl_804736B0",
            "x0, x4",
            [s.done, s.tick],
            [self.u32(at) as i32, self.u32(at + 4) as i32],
        );
        for (k, j) in s.joints.iter().enumerate() {
            let theirs = self.u32(at + 8 + 4 * k as u32);
            self.compare_handle(
                &format!("lbl_804736B0.x{:X}", 8 + 4 * k),
                "JObjId",
                j.map(|j| j.0),
                theirs,
            );
        }
        let at = sym("lbl_804D6618");
        let theirs = [self.u32(at) as i32, self.u32(at + 4) as i32];
        self.check("lbl_804D6618", "the enter data", s.enter, theirs);
        let theirs = self.u32(sym("lbl_804D6610"));
        self.check("lbl_804D6610", "archive", s.archive.block, theirs);
        self.compare_data("lbl_804D6614", s.scene, self.u32(sym("lbl_804D6614")));
    }
}
