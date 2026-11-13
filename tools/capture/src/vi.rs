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
        self.check("erase_colors_vi0601", "erase color", v.v0601.erase, theirs);
        self.cutscenes_bowser();
    }

    /// A GObj handle a cutscene keeps.
    fn vi_gobj(&mut self, path: &str, at: u32, g: Option<melee_sim::hsd::gobj::GObjId>) {
        let theirs = self.u32(at);
        self.compare_handle(path, "GObjId", g.map(|g| g.0), theirs);
    }

    /// A JObj handle a cutscene keeps.
    fn vi_jobj(&mut self, name: &str, j: Option<melee_sim::hsd::jobj::JObjId>) {
        let theirs = self.u32(sym(name));
        self.compare_handle(name, "JObjId", j.map(|j| j.0), theirs);
    }

    /// The statics of the metal cutscene and Bowser's three (`vi1101.c` to `vi1202.c`).
    fn cutscenes_bowser(&mut self) {
        let v = self.game.vi;
        // `vi1101.c`.
        let a = v.v1101;
        self.compare_data("un_804D6FC0", a.scene, self.u32(sym("un_804D6FC0")));
        self.compare_data("un_804D6FC4", a.scene_cam2, self.u32(sym("un_804D6FC4")));
        self.vi_archive("un_804D6FC8", a.waits);
        self.vi_archive("un_804D6FCC", a.archive);
        let at = sym("un_804D6FD0");
        self.vi_gobj("un_804D6FD0.unk0", at, a.metal[0]);
        self.vi_gobj("un_804D6FD0.unk4", at + 4, a.metal[1]);
        // `vi1201v1.c`.
        let a = v.v1201v1;
        self.compare_data("un_804D6FE0", a.scene, self.u32(sym("un_804D6FE0")));
        self.compare_data("un_804D6FE4", a.stand, self.u32(sym("un_804D6FE4")));
        self.vi_archive("un_804D6FE8", a.archive);
        self.compare_data("un_804D6FEC", a.trophy, self.u32(sym("un_804D6FEC")));
        self.vi_jobj("un_804D6FF0", a.anchor);
        self.vi_color("un_804D6FF4", a.erase);
        let theirs = self.u32(sym("un_804D6FF8")) as i32;
        self.check("un_804D6FF8", "the fog's ticks", a.fog_ticks, theirs);
        let theirs = [self.u8(sym("un_804D6FFC")), self.u8(sym("un_804D6FFD"))];
        self.check("un_804D6FFC", "character", [a.ckind, a.costume], theirs);
        self.vi_gobj("un_804D7000", sym("un_804D7000"), a.fighter);
        // `vi1201v2.c`.
        let a = v.v1201v2;
        self.compare_data("un_804D7010", a.scene, self.u32(sym("un_804D7010")));
        self.compare_data("un_804D7014", a.stand, self.u32(sym("un_804D7014")));
        self.vi_archive("un_804D7018", a.waits);
        self.vi_archive("un_804D701C", a.archive);
        self.compare_data("un_804D7020", a.trophy, self.u32(sym("un_804D7020")));
        self.vi_jobj("un_804D7024", a.anchor);
        self.vi_color("un_804D7028", a.erase);
        let theirs = self.u32(sym("un_804D702C")) as i32;
        self.check("un_804D702C", "word", a.x2c, theirs);
        self.vi_gobj("un_804D7030", sym("un_804D7030"), a.trophy_gobj);
        self.vi_gobj("un_804D7034", sym("un_804D7034"), a.stand_gobj);
        // `vi1202.c`.
        let a = v.v1202;
        self.compare_data("un_804D7040", a.scene, self.u32(sym("un_804D7040")));
        self.compare_data("un_804D7044", a.stand, self.u32(sym("un_804D7044")));
        self.compare_data("un_804D7048", a.trophy, self.u32(sym("un_804D7048")));
        self.vi_jobj("un_804D704C", a.anchor);
    }
}
