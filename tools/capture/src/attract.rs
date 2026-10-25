//! The attract loop's statics (`gmopeningmode.c`'s `vs_enter_data`, `gm_181A.c`'s camera
//! step, the movies' sprites' GObjs), compared with memory.

use melee_sim::game::mirror::{Cx, Mem, Mirror};

use crate::MEM1;
use crate::compare::Comparer;
use crate::import::sym;

/// `gmopeningmode.c`'s `vs_enter_data` (a `StartMeleeData`).
const VS_ENTER_DATA: u32 = 0x8049_C188;
/// `gmopeningmode.c`'s `vs_exit_data` (a `MatchExitInfo`).
const VS_EXIT_DATA: u32 = 0x8049_C2C0;

impl Comparer<'_> {
    /// The attract match's start data and camera step, and the movies' GObjs.
    pub(crate) fn attract(&mut self) {
        let game = self.game;
        let mem = Mem {
            bytes: self.mem,
            base: MEM1,
        };
        game.attract
            .start
            .compare(&mem, VS_ENTER_DATA, "vs_enter_data", self);
        self.match_exit_info("vs_exit_data", &game.attract.exit, VS_EXIT_DATA);
        self.check(
            "lbl_804D65E8",
            "the attract camera's step",
            game.attract.camera_step,
            self.u8(sym("lbl_804D65E8")),
        );
        let m = game.movies;
        for (name, g) in [("gm_804D6850", m.howto), ("gm_804D6858", m.omake)] {
            let theirs = self.u32(sym(name));
            self.compare_handle(name, "GObjId", g.map(|g| g.0), theirs);
        }
    }
}
