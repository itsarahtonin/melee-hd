//! The single-player modes' statics (`melee_sim::game::gm::one_player`): their selects' data,
//! the 1P runs' state and the tables their loads write, compared with memory.

use melee_sim::game::gm::one_player::{
    self as op, AllStarData, ClassicStage, MatchupRef, Unk1PData,
};
use melee_sim::game::gm::vs_melee::{CssData, SssData};
use melee_sim::game::mirror::{Cx, Mirror};
use melee_sim::game::structs::PlayerInitData;

use crate::compare::Comparer;

/// `gmClassic_80470708`, `gm_80490960`, `gm_80490AA8`, and Training's `css_data` and
/// `sss_data` (`gmtrainingmode.c`'s).
const CLASSIC_CSS: u32 = 0x8047_0708;
const TARGET_CSS: u32 = 0x8049_0960;
const TARGET_START: u32 = 0x8049_0AA8;
/// `gm_80490BE0`, Target Test's match's exit data.
const TARGET_EXIT: u32 = 0x8049_0BE0;
const TRAINING_CSS: u32 = 0x8048_E230;
const TRAINING_SSS: u32 = 0x8048_E378;
/// The 1P runs: `lbl_80472CB0` (Classic's), `lbl_80472C30` (Adventure's), `gm_80473A18`
/// (All-Star's).
const CLASSIC_RUN: u32 = 0x8047_2CB0;
const ADVENTURE_RUN: u32 = 0x8047_2C30;
const ALLSTAR_RUN: u32 = 0x8047_3A18;
const CLASSIC_INTRO: u32 = 0x8049_0880;
const CLASSIC_ORDER: u32 = 0x8049_08A0;
const ADVENTURE_X910: u32 = 0x8049_0910;
const ALLSTAR_X940: u32 = 0x8049_0940;
const ALLSTAR_OPPONENTS: u32 = 0x803D_EBE8;
/// `gmClassic_803DDEC8`, its stage list's entries 16 bytes each (`xC` the matchup), and the
/// bonus stages' matchups.
const CLASSIC_TABLES: u32 = 0x803D_DEC8;
const CLASSIC_BONUS: [u32; 3] = [0x804D_4318, 0x804D_4320, 0x804D_4328];
/// `gm_80473814.stage_id`.
const TRAINING_STAGE: u32 = 0x8047_3814 + 6;
/// Training's `lbl_80473700` and `gm_80473814`, its menu's archive and models
/// (`lbl_804D6628`, `lbl_804D662C`), and its match state's enter and exit data.
const TRAINING_STATE: u32 = 0x8047_3700;
const TRAINING_SUB: u32 = 0x8047_3814;
const TRAINING_ARCHIVE: u32 = 0x804D_6628;
const TRAINING_MODELS: u32 = 0x804D_662C;
const TRAINING_ENTER: u32 = 0x8048_E4C0;
const TRAINING_EXIT: u32 = 0x8048_E5F8;
/// `gmClassic_80472AF8`, `gmClassic_8047086C`: the 1P runs' matches' enter and exit data;
/// `lbl_804706D8`, the match's won trophies.
const MATCH_START: u32 = 0x8047_2AF8;
const MATCH_EXIT: u32 = 0x8047_086C;
const MATCH_TROPHIES: u32 = 0x8047_06D8;
/// `lbl_80472D28`, the clear screen's state (`gmregclear.c`).
const CLEAR_STATE: u32 = 0x8047_2D28;
/// The ports and entries: Target Test's, the Home-Run Contest's, Training's.
const BYTES: [(u32, &str); 6] = [
    (0x804D_68E8, "gm_804D68E8"),
    (0x804D_68E9, "gm_804D68E9"),
    (0x804D_68F8, "gm_804D68F8"),
    (0x804D_68F9, "gm_804D68F9"),
    (0x804D_68C0, "gm_804D68C0"),
    (0x804D_68C1, "gm_804D68C1"),
];

impl Comparer<'_> {
    /// A select's data (`CSSData`) at `at`: its port, match type and pending change, its KO
    /// counts' pointer (null: no single-player mode's entry sets it), and its VS data.
    pub(crate) fn one_player_css(&mut self, name: &str, css: &CssData, at: u32) {
        self.check(
            name,
            "port, match type, pending, KO counts' pointer",
            (css.unk_0x0, css.match_type, css.pending_scene_change, 0u32),
            (
                self.u16(at),
                self.u8(at + 2),
                self.u8(at + 3),
                self.u32(at + 4),
            ),
        );
        let m = self.mem_view();
        css.vs.compare(&m, at + 8, &format!("{name}.vs"), self);
    }

    fn one_player_sss(&mut self, name: &str, s: &SssData, at: u32) {
        self.check(
            name,
            "stage, x1, no LRAS, forced, start",
            (s.unk_stage, s.x1, s.no_lras, s.force_stage_id, s.start_game),
            (
                self.u8(at),
                self.u8(at + 1),
                self.u8(at + 2),
                self.u8(at + 3) as i8,
                self.u8(at + 4),
            ),
        );
        let m = self.mem_view();
        s.vs.compare(&m, at + 8, &format!("{name}.vs"), self);
    }

    /// A 1P run's state (`Unk1PData`) at `at`.
    fn one_player_run(&mut self, name: &str, r: &Unk1PData, at: u32) {
        let p = r.player;
        let ours: Vec<u8> = [
            p.ckind as u8,
            p.color,
            p.cpu_level,
            p.slot,
            p.nametag,
            p.stocks,
            p.x6,
            p.mode,
        ]
        .into_iter()
        .chain(r.x8)
        .chain(r.xc)
        .collect();
        let theirs: Vec<u8> = (0..0x14).map(|i| self.u8(at + i)).collect();
        self.check(name, "player, x8 to x13", ours, theirs);
        self.check(
            name,
            "x14, x18, x1C, x20",
            (r.x14, r.x18 as u32, r.x1c as u32, r.x20),
            (
                self.u16(at + 0x14),
                self.u32(at + 0x18),
                self.u32(at + 0x1C),
                self.u32(at + 0x20),
            ),
        );
        for (i, o) in r.opponents.iter().enumerate() {
            let a = at + 0x24 + 0xC * i as u32;
            self.check(
                name,
                &format!("opponent {i}"),
                (
                    [o.ckind as u8, o.color, o.cpu_level, o.cpu_kind],
                    o.attack_ratio.to_bits(),
                    o.defense_ratio.to_bits(),
                ),
                (
                    [0, 1, 2, 3].map(|k| self.u8(a + k)),
                    self.u32(a + 4),
                    self.u32(a + 8),
                ),
            );
        }
        let ours = r.getters.map(|g| g.map_or(0, |g| g.0));
        let theirs: [u32; 11] = core::array::from_fn(|i| self.u32(at + 0x48 + 4 * i as u32));
        self.check(name, "the getters", ours, theirs);
    }

    fn one_player_allstar(&mut self, name: &str, a: &AllStarData, at: u32, whole: bool) {
        self.one_player_run(name, &a.base, at);
        self.check(name, "x74", a.x74, self.u16(at + 0x74));
        if !whole {
            return;
        }
        let ours: Vec<u8> = a
            .x76
            .iter()
            .chain(&a.x90)
            .chain(&a.x94)
            .chain(&a.x96)
            .copied()
            .collect();
        let theirs: Vec<u8> = (0x76..0x9C).map(|i| self.u8(at + i)).collect();
        self.check(name, "x76 to x9B", ours, theirs);
        self.check(name, "x9C", a.x9c, self.u32(at + 0x9C));
    }

    /// The single-player modes' statics.
    pub(crate) fn one_player(&mut self) {
        let o = &self.game.one_player;
        self.one_player_css("gmClassic_80470708", &o.css, CLASSIC_CSS);
        self.one_player_css("gm_80490960", &o.target_css, TARGET_CSS);
        self.one_player_css("css_data (Training's)", &o.training_css, TRAINING_CSS);
        self.one_player_sss("sss_data (Training's)", &o.training_sss, TRAINING_SSS);
        let m = self.mem_view();
        o.target_start
            .compare(&m, TARGET_START, "gm_80490AA8", self);
        self.match_exit_info("gm_80490BE0", &o.target_exit, TARGET_EXIT);
        let ours = [
            o.target_slot,
            o.target_entries,
            o.homerun_slot,
            o.homerun_entries,
            o.training_slot,
            o.training_entries,
        ];
        for ((at, name), v) in BYTES.into_iter().zip(ours) {
            self.check(name, "value", v, self.u8(at));
        }
        self.one_player_allstar("lbl_80472CB0", &o.classic, CLASSIC_RUN, false);
        self.one_player_run("lbl_80472C30", &o.adventure.base, ADVENTURE_RUN);
        let a = &o.adventure;
        let ours: Vec<u8> = a
            .x74
            .iter()
            .chain(&a.x79)
            .chain(core::iter::once(&a.x7c))
            .chain(&a.x7d)
            .copied()
            .collect();
        let theirs: Vec<u8> = (0x74..0x80).map(|i| self.u8(ADVENTURE_RUN + i)).collect();
        self.check("lbl_80472C30", "x74 to x7F", ours, theirs);
        self.one_player_allstar("gm_80473A18", &o.allstar, ALLSTAR_RUN, true);
        for (name, at, ours) in [
            (
                "gmClassicIntroDataBuffer",
                CLASSIC_INTRO,
                &o.classic_intro[..],
            ),
            ("gm_804908A0", CLASSIC_ORDER, &o.classic_order[..]),
            ("gm_80490910", ADVENTURE_X910, &o.adventure_x910[..]),
            ("gm_80490940", ALLSTAR_X940, &o.allstar_x940[..]),
        ] {
            let theirs: Vec<u8> = (0..ours.len() as u32).map(|i| self.u8(at + i)).collect();
            self.check(name, "bytes", ours.to_vec(), theirs);
        }
        let ours: Vec<u8> = o.allstar_opponents.iter().flatten().copied().collect();
        let theirs: Vec<u8> = (0..100).map(|i| self.u8(ALLSTAR_OPPONENTS + i)).collect();
        self.check("gm_803DEBE8", "the opponents", ours, theirs);
        for (i, r) in o.classic_picks.iter().enumerate() {
            let ours = match *r {
                MatchupRef::None => 0,
                MatchupRef::Table(t, k) => CLASSIC_TABLES + u32::from(t) + 6 * u32::from(k),
                MatchupRef::Bonus(k) => CLASSIC_BONUS[usize::from(k) - 1],
            };
            let theirs = self.u32(CLASSIC_TABLES + 0x10 * i as u32 + 0xC);
            self.check("gmClassic_803DDEC8.x00", &format!("[{i}].xC"), ours, theirs);
        }
        let _: &[ClassicStage] = &op::CLASSIC_STAGES;
        self.check(
            "gm_80473814.stage_id",
            "value",
            self.game.modes.training_stage as u16,
            self.u16(TRAINING_STAGE),
        );
        self.training();
        let o = &self.game.one_player;
        let m = self.mem_view();
        o.match_start
            .compare(&m, MATCH_START, "gmClassic_80472AF8", self);
        self.match_exit_info("gmClassic_8047086C", &o.match_exit, MATCH_EXIT);
        let theirs: [(i16, u8, u8); 12] = core::array::from_fn(|i| {
            let a = MATCH_TROPHIES + 4 * i as u32;
            (self.u16(a) as i16, self.u8(a + 2), self.u8(a + 3))
        });
        self.check("lbl_804706D8", "won trophies", o.match_trophies, theirs);
        let theirs = self.u32(CLEAR_STATE + 0x104) as i32;
        self.check(
            "lbl_80472D28",
            "x104 (the clear's score)",
            o.clear_score,
            theirs,
        );
    }

    /// Training's statics (`melee_sim::game::gm::training`): `lbl_80473700`, the menu's part
    /// of `gm_80473814`, the menu's archive and the match state's enter and exit data.
    fn training(&mut self) {
        let t = &self.game.training;
        let a = TRAINING_STATE;
        self.check(
            "lbl_80473700",
            "count, mode",
            (t.count, t.mode),
            (self.u32(a) as i32, self.u32(a + 4) as i32),
        );
        let theirs: [i32; 27] = core::array::from_fn(|i| self.u32(a + 8 + 4 * i as u32) as i32);
        self.check("lbl_80473700", "char_data", t.char_data, theirs);
        let m = self.mem_view();
        for (i, p) in t.players.iter().enumerate() {
            p.compare(
                &m,
                a + 0x74 + PlayerInitData::SIZE * i as u32,
                &format!("lbl_80473700.players[{i}]"),
                self,
            );
        }
        let theirs: [i32; 4] = core::array::from_fn(|i| self.u32(a + 0x104 + 4 * i as u32) as i32);
        self.check("lbl_80473700", "result_cache", t.result_cache, theirs);
        let s = TRAINING_SUB;
        self.check(
            "gm_80473814",
            "cursor, open, behavior",
            (t.cursor, t.open, t.cpu_type),
            (self.u8(s), self.u8(s + 1), self.u8(s + 3)),
        );
        let theirs: [i32; 39] = core::array::from_fn(|i| self.u32(s + 0x144 + 4 * i as u32) as i32);
        self.check("gm_80473814", "anim_frames", t.anim_frames, theirs);
        let theirs: [i32; 7] = core::array::from_fn(|i| self.u32(s + 0x1E0 + 4 * i as u32) as i32);
        self.check("gm_80473814", "menu_values", t.menu_values, theirs);
        // The menu's objects and archive live with Training's scene: past it the words name
        // its freed heap in both, which nothing reads until the next match's menu sets them
        // again (`fn_8018A000`, `fn_80189B88`).
        let live = melee_sim::game::gm::machine::scene_kind(self.game.machine.scene_info)
            == Some(melee_sim::game::gm::training::GS_TRAINING);
        if live {
            self.compare_handle(
                "gm_80473814.gobj",
                "GObjId",
                t.gobj.map(|g| g.0),
                self.u32(s + 0xA4),
            );
            for (i, j) in t.jobjs.iter().enumerate() {
                let theirs = self.u32(s + 0xA8 + 4 * i as u32);
                self.compare_handle(
                    &format!("gm_80473814.jobjs[{i}]"),
                    "JObjId",
                    j.map(|j| j.0),
                    theirs,
                );
            }
            self.compare_handle(
                "gm_80473814.text",
                "TextId",
                t.text.map(|t| t.0),
                self.u32(s + 0x1FC),
            );
            let archive = self.u32(TRAINING_ARCHIVE);
            let theirs = (archive != 0)
                .then(|| self.data_ref(self.u32(archive + 0x20)).map(|d| d.archive))
                .flatten();
            self.check("lbl_804D6628", "GmTrain", t.archive, theirs);
            let models = self.data_ref(self.u32(TRAINING_MODELS));
            self.check("lbl_804D662C", "scene models", t.models, models);
        }
        let m = self.mem_view();
        t.enter
            .compare(&m, TRAINING_ENTER, "training_enter_data", self);
        let e = &self.game.training.exit;
        self.match_exit_info("training_exit_data", e, TRAINING_EXIT);
    }
}
