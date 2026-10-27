//! Camera Mode's statics, compared with memory: the card check's exit and texts
//! (`gm_804D68C8`, `gmCamera_VsCameraTextLayout`), the match's menu
//! (`gmCamera_VsCamUiState`), the frame's copy (`cmsnap_data`) and the snapshot being written
//! (`lbSnap_80433380`'s header and name).

use melee_sim::game::mirror::Cx;

use crate::compare::Comparer;
use crate::import::sym;

impl Comparer<'_> {
    /// Camera Mode's statics.
    pub(crate) fn camera_mode(&mut self) {
        let c = self.game.camera_mode;
        let exit = sym("gm_804D68C8");
        self.check(
            "gm_804D68C8",
            "the card check's exit",
            (c.check_exit, 0),
            (self.u32(exit), self.u32(exit + 4)),
        );
        // The layout's state pointer, once the check's entry set it.
        let layout = sym("gmCamera_VsCameraTextLayout");
        let state = self.u32(layout);
        if state != 0 {
            self.check("gmCamera_VsCameraTextLayout", "state", exit, state);
        }
        for (i, t) in c.check_texts.iter().enumerate() {
            let theirs = self.u32(layout + 4 + 4 * i as u32);
            self.compare_handle(
                &format!("gmCamera_VsCameraTextLayout.text[{i}]"),
                "TextId",
                t.map(|t| t.0),
                theirs,
            );
        }
        let ui = c.ui;
        let a = sym("gmCamera_VsCamUiState");
        let p = "gmCamera_VsCamUiState";
        let archive = self.u32(a);
        let theirs = (archive != 0)
            .then(|| self.data_ref(self.u32(archive + 0x20)).map(|d| d.archive))
            .flatten();
        self.check(p, "ifvscam", ui.archive, theirs);
        self.compare_handle(
            &format!("{p}.x4"),
            "JObjId",
            ui.info.map(|j| j.0),
            self.u32(a + 4),
        );
        self.compare_handle(
            &format!("{p}.x8"),
            "JObjId",
            ui.model.map(|j| j.0),
            self.u32(a + 8),
        );
        self.check(
            p,
            "state, frames, info, choice, image, blocks",
            (
                ui.state,
                ui.frames,
                ui.show_info,
                ui.choice,
                ui.snap_image,
                ui.blocks,
            ),
            (
                self.u32(a + 0xC) as i32,
                self.u32(a + 0x10) as i32,
                self.u32(a + 0x14) as i32,
                self.u32(a + 0x18) as i32,
                self.u32(a + 0x1C),
                self.u32(a + 0x20) as i32,
            ),
        );
        for (i, s) in ui.slots.iter().enumerate() {
            let at = a + 0x24 + 0x10 * i as u32;
            self.check(
                p,
                &format!("x24[{i}]"),
                (s.result, s.free_blocks, s.free_files, s.verdict),
                (
                    self.u32(at) as i32,
                    self.u32(at + 4) as i32,
                    self.u32(at + 8) as i32,
                    self.u32(at + 0xC) as i32,
                ),
            );
        }
        self.check(p, "x44", ui.verdict, self.u32(a + 0x44) as i32);
        for (i, t) in ui.texts.iter().enumerate() {
            let theirs = self.u32(a + 0x48 + 4 * i as u32);
            self.compare_handle(&format!("{p}.x48[{i}]"), "TextId", t.map(|t| t.0), theirs);
        }
        self.check(p, "x54", ui.canvas, self.u32(a + 0x54) as i32);
        let s = sym("cmsnap_data");
        self.check(
            "cmsnap_data",
            "status, image",
            (c.snap.status, c.snap.image),
            (self.u32(s) as i32, self.u32(s + 4)),
        );
        // The image's descriptor once the match's start set it up: 640 by 480 RGB565.
        if c.snap.image != 0 {
            self.check(
                "cmsnap_data",
                "image's size and format",
                (640u16, 480u16, 4u32),
                (self.u16(s + 8), self.u16(s + 0xA), self.u32(s + 0xC)),
            );
        }
    }
}
