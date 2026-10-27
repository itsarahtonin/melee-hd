//! Camera Mode's statics, compared with memory: the card check's exit and texts
//! (`gm_804D68C8`, `gmCamera_VsCameraTextLayout`), the match's menu
//! (`gmCamera_VsCamUiState`), the frame's copy (`cmsnap_data`) and the snapshot being written
//! (`lbSnap_80433380`'s header and name); and Stamina Mode's (`gm_804975F8`).

use melee_sim::game::mirror::Cx;

use crate::compare::Comparer;
use crate::import::sym;

impl Comparer<'_> {
    /// Stamina Mode's statics (`gm_804975F8`: the match's end's count, each port's
    /// knockout and its slowdown's count), which its match start sets.
    pub(crate) fn stamina_mode(&mut self) {
        let st = self.game.gm.stamina;
        let a = sym("gm_804975F8");
        let elim: [bool; 4] = core::array::from_fn(|i| self.u32(a + 4 + 4 * i as u32) != 0);
        let slomo: [u16; 4] = core::array::from_fn(|i| self.u16(a + 0x14 + 2 * i as u32));
        self.check(
            "gm_804975F8",
            "done, eliminated, slow-mo",
            (st.done_counter, st.eliminated, st.slomo_counter),
            (self.u16(a), elim, slomo),
        );
    }

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
        self.snap_written();
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

    /// The snapshot `lbSnap_8001DC0C` wrote: its header and compressed image in its buffer,
    /// the card file's comment (`lbSnap_80433380.filename`), the banner drawn into
    /// `LbMcSnap`'s icon data, and the encoder's quality (`lbl_804D6398`).
    fn snap_written(&mut self) {
        let w = &self.game.snap_write;
        self.check(
            "lbl_804D6398",
            "the encoder's quality",
            w.quality,
            self.u32(sym("lbl_804D6398")) as i32,
        );
        let snap = self.game.snap;
        if let (Some(h), Some(at)) = (w.header, snap.snap) {
            self.check(
                "lbSnap_80433380.snap",
                "kind, size, quality, image's bytes, stage",
                (h.x0, h.width, h.height, h.x8, h.xc, h.stkind),
                (
                    self.u32(at) as i32,
                    self.u16(at + 4),
                    self.u16(at + 6),
                    self.u8(at + 8),
                    self.u32(at + 0xC) as i32,
                    self.u16(at + 0x10),
                ),
            );
            let items: [i32; 8] =
                core::array::from_fn(|i| self.u32(at + 0x14 + 4 * i as u32) as i32);
            self.check("lbSnap_80433380.snap", "items out", h.items, items);
            self.check(
                "lbSnap_80433380.snap",
                "fighters out",
                h.fighters,
                self.u32(at + 0x34) as i32,
            );
            let base = at + melee_sim::lb::snap_write::HEADER;
            if let Some(k) = (0..w.image.len()).find(|&k| w.image[k] != self.u8(base + k as u32)) {
                let theirs = self.u8(base + k as u32);
                self.diff(
                    "lbSnap_80433380.snap",
                    &format!("compressed image's byte {k:#x} (of {:#x})", w.image.len()),
                    w.image[k],
                    theirs,
                );
            }
        }
        let name = sym("lbSnap_80433380") + 4;
        if let Some(f) = w.filename {
            let theirs: Vec<u8> = (0..0x40).map(|i| self.u8(name + i)).collect();
            self.check(
                "lbSnap_80433380.filename",
                "comment",
                String::from_utf8_lossy(&f).into_owned(),
                String::from_utf8_lossy(&theirs).into_owned(),
            );
        }
        // The banner where the trace's icon data (`lbSnap_80433380.icon_data`) points.
        let icons = self.u32(sym("lbSnap_80433380") + 0x44);
        if let Some(b) = w
            .banner
            .as_ref()
            .filter(|_| icons != 0 && snap.icon_data.is_some())
        {
            let banner = self.u32(icons);
            if let Some(k) = (0..b.len()).find(|&k| b[k] != self.u8(banner + k as u32)) {
                let theirs = self.u8(banner + k as u32);
                self.diff(
                    "LbMcSnap MemSnapIconData",
                    &format!("banner's byte {k:#x}"),
                    b[k],
                    theirs,
                );
            }
        }
    }
}
