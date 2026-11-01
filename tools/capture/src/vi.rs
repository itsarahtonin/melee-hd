//! The cutscenes' statics (`melee/vi`), compared with memory: each cutscene's enter data,
//! congratulations' values the debug cutscene mode leaves, and each scene's scene, archives,
//! erase color and the objects it keeps.

use melee_sim::game::mirror::Cx;
use melee_sim::game::vi::Loaded;

use crate::compare::Comparer;
use crate::import::sym;

impl Comparer<'_> {
    /// An archive a cutscene keeps: its `HSD_Archive` block's address.
    fn vi_archive(&mut self, name: &str, a: Loaded) {
        let theirs = self.u32(sym(name));
        self.check(name, "archive", a.block, theirs);
    }

    /// An erase color.
    fn vi_color(&mut self, name: &str, ours: [u8; 4]) {
        let at = sym(name);
        let theirs = [0, 1, 2, 3].map(|k| self.u8(at + k));
        self.check(name, "erase color", ours, theirs);
    }

    /// The cutscenes' statics.
    pub(crate) fn cutscenes(&mut self) {
        let v = self.game.vi;
        for (name, d) in [
            ("un_804D6F3C", v.desc_0102),
            ("un_804D6F60", v.desc_0401),
            ("un_804D6F84", v.desc_0501),
            ("un_804D6FA8", v.desc_0502),
            ("un_804D6FD8", v.desc_1101),
            ("un_804D7004", v.desc_1201v1),
            ("un_804D7038", v.desc_1201v2),
        ] {
            let at = sym(name);
            let theirs: [u8; 8] = std::array::from_fn(|k| self.u8(at + k as u32));
            self.check(name, "enter data", d, theirs);
        }
        let at = sym("gm_8049C178");
        let theirs = [self.u8(at), self.u8(at + 1)];
        self.check(
            "gm_8049C178",
            "congratulations' character and value",
            v.gover,
            theirs,
        );
        // `vi0102.c`.
        self.compare_data("un_804D6F30", v.v0102.scene, self.u32(sym("un_804D6F30")));
        self.vi_archive("un_804D6F38", v.v0102.archive);
        self.vi_color("erase_colors_vi0102", v.v0102.erase);
        // `vi0401.c`.
        let a = v.v0401;
        for (name, d) in [
            ("un_804D6F48", a.scene),
            ("un_804D6F4C", a.info),
            ("un_804D6F50", a.damage),
        ] {
            self.compare_data(name, d, self.u32(sym(name)));
        }
        self.vi_archive("un_804D6F54", a.waits);
        let theirs = self.u32(sym("un_804D6F58"));
        self.compare_handle("un_804D6F58", "JObjId", a.spot.map(|j| j.0), theirs);
        self.vi_color("erase_colors_vi0401", a.erase);
        // `vi0402.c`.
        self.compare_data("un_804D6F68", v.v0402.scene, self.u32(sym("un_804D6F68")));
        // `vi0501.c`.
        let a = v.v0501;
        self.compare_data("un_804D6F70", a.scene, self.u32(sym("un_804D6F70")));
        self.vi_archive("un_804D6F74", a.archive);
        self.vi_archive("un_804D6F78", a.waits);
        self.vi_color("erase_colors_vi0501", a.erase);
        self.check(
            "un_804D6F80",
            "the stage's old scale",
            a.scale.to_bits(),
            self.u32(sym("un_804D6F80")),
        );
        let at = sym("un_804A2E98");
        for (k, g) in a.kirbys.iter().enumerate() {
            let theirs = self.u32(at + 4 * k as u32);
            self.compare_handle(
                &format!("un_804A2E98[{k}]"),
                "GObjId",
                g.map(|g| g.0),
                theirs,
            );
        }
        // `vi0502.c`.
        let a = v.v0502;
        self.compare_data("un_804D6F90", a.scene, self.u32(sym("un_804D6F90")));
        self.vi_archive("un_804D6F94", a.waits);
        self.vi_archive("un_804D6F98", a.irals);
        self.vi_archive("un_804D6F9C", a.archive);
        self.vi_color("erase_colors_vi0502", a.erase);
        let theirs = self.u32(sym("kirby_gobj"));
        self.compare_handle("kirby_gobj", "GObjId", a.kirby.map(|g| g.0), theirs);
        // `vi0601.c`.
        self.compare_data("un_804D6FB0", v.v0601.scene, self.u32(sym("un_804D6FB0")));
        // Its alpha is a stack byte `Camera_GetBackgroundColor` never writes, which nothing
        // reads: the erase writes color and depth only (`HSD_EraseRect`'s alpha update off,
        // a logic-op copy, alpha compare always), so it never reaches the screen (docs/
        // engine.md, uninitialized values).
        let at = sym("erase_colors_vi0601");
        let theirs = [0, 1, 2].map(|k| self.u8(at + k));
        let e = v.v0601.erase;
        self.check(
            "erase_colors_vi0601",
            "erase color",
            [e[0], e[1], e[2]],
            theirs,
        );
    }
}
