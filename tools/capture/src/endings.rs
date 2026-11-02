//! The single-player endings' statics (milestone 6's task 5), compared with memory: the game
//! over modes' data (`gmgover.c`), the debug mode's single-player states' enter data, and
//! game over's and coming soon's screens (`gm_19EF.c`).

use melee_sim::game::mirror::Cx;

use crate::compare::Comparer;
use crate::import::sym;

impl Comparer<'_> {
    /// A run of bytes at a symbol.
    fn endings_bytes<const N: usize>(&mut self, name: &str, what: &str, ours: [u8; N]) {
        let at = sym(name);
        let theirs: [u8; N] = std::array::from_fn(|k| self.u8(at + k as u32));
        self.check(name, what, ours, theirs);
    }

    /// The endings' statics.
    pub(crate) fn endings(&mut self) {
        let e = self.game.endings;
        self.endings_bytes(
            "gm_8049C178",
            "the ending's character, costume, mode, slot, port, tag",
            e.gover.0,
        );
        self.endings_bytes(
            "gm_804D6920",
            "the game over modes' exit data",
            e.gover_exit,
        );
        let theirs = self.u16(sym("lbl_804D659A"));
        self.check(
            "lbl_804D659A",
            "the count-down tick's wait",
            e.tick_wait,
            theirs,
        );
        // The debug mode's states' enter data.
        let d = e.debug_game_over;
        let at = sym("game_over_data");
        self.check(
            "game_over_data",
            "score, x4, x8, result, character, slot, tag, cost, coins",
            (
                d.score, d.x4, d.x8, d.result, d.ckind, d.slot, d.tag, d.cost, d.coins,
            ),
            (
                self.u32(at),
                self.u32(at + 4) as i32,
                self.u32(at + 8),
                self.u32(at + 0xC) as i32,
                self.u32(at + 0x10),
                self.u8(at + 0x14),
                self.u8(at + 0x15),
                self.u16(at + 0x16),
                self.u16(at + 0x18),
            ),
        );
        self.endings_bytes(
            "intro_easy_enter_data",
            "Classic's intro's enter data",
            e.intro_easy,
        );
        let at = sym("intro_allstar_enter_data");
        let theirs = [self.u32(at), self.u32(at + 4)];
        self.check(
            "intro_allstar_enter_data",
            "All-Star's intro's enter data",
            e.intro_allstar,
            theirs,
        );
        self.game_over_statics();
    }

    /// Game over's and coming soon's statics (`gm_19EF.c`).
    fn game_over_statics(&mut self) {
        let g = self.game.endings.game_over;
        let theirs = self.u32(sym("lbl_804D6698"));
        self.check("lbl_804D6698", "archive", g.blocks.1, theirs);
        for (k, name) in [
            "lbl_804D669C",
            "lbl_804D66A0",
            "lbl_804D66A4",
            "lbl_804D66A8",
        ]
        .into_iter()
        .enumerate()
        {
            let theirs = self.u32(sym(name));
            self.compare_data(name, g.scenes[k], theirs);
        }
        let theirs = self.u32(sym("lbl_804D66AC"));
        self.compare_data("lbl_804D66AC", g.trophy, theirs);
        let names = [
            "lbl_804D66B0",
            "lbl_804D66B8",
            "lbl_804D66C0",
            "lbl_804D66C8",
            "lbl_804D66D0",
            "lbl_804D66D8",
            "lbl_804D66E0",
            "lbl_804D66E8",
        ];
        for (n, name) in g.nodes.iter().zip(names) {
            let at = sym(name);
            let theirs = self.u32(at);
            self.compare_handle(name, "JObjId", n.jobj.map(|j| j.0), theirs);
            // The trophy's and the stand's frames nothing writes.
            let theirs = self.u32(at + 4) as i32;
            self.check(name, "frame", n.frame, theirs);
        }
        let theirs = self.u32(sym("lbl_804D66F0"));
        self.check("lbl_804D66F0", "archive", g.soon_block, theirs);
        let theirs = self.u32(sym("lbl_804D66F4"));
        self.compare_handle("lbl_804D66F4", "JObjId", g.soon_jobj.map(|j| j.0), theirs);
        let theirs = self.u32(sym("lbl_804D66F8"));
        self.check("lbl_804D66F8", "frame", g.soon_frame, theirs);
        let at = sym("lbl_80479A98");
        self.check(
            "lbl_80479A98",
            "state, mode, score, half, step, yes, slot, tag, result",
            (
                g.state, g.mode, g.score, g.half, g.step, g.yes, g.slot, g.tag, g.result,
            ),
            (
                self.u8(at),
                self.u32(at + 4) as i32,
                self.u32(at + 8) as i32,
                self.u32(at + 0xC) as i32,
                self.u32(at + 0x10),
                self.u8(at + 0x14),
                self.u8(at + 0x15),
                self.u8(at + 0x16),
                self.u32(at + 0x18) as i32,
            ),
        );
        self.check(
            "lbl_80479A98",
            "cost, held, short, coins",
            (g.cost, g.held, g.short, g.coins),
            (
                self.u16(at + 0x1C),
                self.u16(at + 0x1E),
                self.u8(at + 0x20),
                self.u16(at + 0x22),
            ),
        );
        let theirs = self.u32(at + 0x24);
        self.compare_handle("lbl_80479A98.x24", "TextId", g.text.map(|t| t.0), theirs);
        let theirs = self.u32(at + 0x28);
        self.compare_handle("lbl_80479A98.x28", "GObjId", g.counter.map(|o| o.0), theirs);
        for (k, j) in g.coin_jobjs.iter().enumerate() {
            let theirs = self.u32(at + 0x2C + 4 * k as u32);
            self.compare_handle(
                &format!("lbl_80479A98.coins[{k}]"),
                "JObjId",
                j.map(|j| j.0),
                theirs,
            );
        }
        for (k, j) in g.reels.iter().enumerate() {
            let theirs = self.u32(at + 0x54 + 4 * k as u32);
            self.compare_handle(
                &format!("lbl_80479A98.reels[{k}]"),
                "JObjId",
                j.map(|j| j.0),
                theirs,
            );
        }
        self.check(
            "lbl_80479A98",
            "paying, reel frames, coins shown",
            (g.paying, g.reel_frames, g.shown),
            (
                self.u8(at + 0x60),
                [0x64, 0x68, 0x6C].map(|o| self.u32(at + o) as i32),
                self.u32(at + 0x70) as i32,
            ),
        );
    }
}
