//! The Home-Run Contest's statics (`melee_sim::game::gm::home_run`, `gm_180A.c`), compared
//! with memory.

use crate::compare::Comparer;

/// `lbl_80472E48`, `lbl_80472EC8` and `lbl_804D65C8` to `lbl_804D65D8`.
const STATE: u32 = 0x8047_2E48;
const DIST: u32 = 0x8047_2EC8;
const ARCHIVE: u32 = 0x804D_65C8;
const NO_CONTEST: u32 = 0x804D_65CC;
const COUNTER: u32 = 0x804D_65D0;
const FANFARE: u32 = 0x804D_65D4;
const END_TIMER: u32 = 0x804D_65D8;

impl Comparer<'_> {
    /// The contest's statics.
    pub(crate) fn home_run(&mut self) {
        use melee_sim::game::mirror::Cx;
        let h = &self.game.home_run;
        let flags = h.b76 << 6 | h.b54 << 4 | h.b32 << 2 | h.b10;
        self.check(
            "lbl_80472E48",
            "flags, player, name tag, wait, port",
            (flags, h.ckind, h.nametag, h.wait, h.port as u8),
            (
                self.u8(STATE),
                self.u32(STATE + 4) as i32,
                self.u32(STATE + 8) as i32,
                self.u32(STATE + 0xC),
                self.u8(STATE + 0x10),
            ),
        );
        let theirs: [i32; 0x1B] =
            core::array::from_fn(|i| self.u32(STATE + 0x14 + 4 * i as u32) as i32);
        self.check("lbl_80472E48", "records", h.records, theirs);
        let theirs: [i32; 4] = core::array::from_fn(|i| self.u32(DIST + 4 * i as u32) as i32);
        self.check("lbl_80472EC8", "distances", h.dist, theirs);
        self.check(
            "lbl_804D65D4",
            "fanfare, end timer",
            (h.fanfare, h.end_timer),
            (self.u32(FANFARE) as i32, self.u32(END_TIMER) as i32),
        );
        // The archive and models stay set past the match, into its freed heap: they compare
        // while the archive is loaded.
        let a = h.archive;
        if a.is_some_and(|a| self.data.loaded.iter().any(|l| l.id == a)) {
            let ca = self.u32(ARCHIVE);
            let theirs = (ca != 0)
                .then(|| self.data.get(self.u32(ca + 0x20)).map(|d| d.archive))
                .flatten();
            self.check("lbl_804D65C8", "archive", a, theirs);
            let (n, c) = (h.no_contest, h.counter);
            self.compare_data("lbl_804D65CC", n, self.u32(NO_CONTEST));
            self.compare_data("lbl_804D65D0", c, self.u32(COUNTER));
        }
    }
}
