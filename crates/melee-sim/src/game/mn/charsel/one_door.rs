//! The single-player modes' select screen (one door, `mnCharSel_804D6CF5 == 1`): the records
//! panel and its texts, the CPU level slider, the stock count, and the records a hovered
//! character shows (`mnCharSel_8025C020`).

use super::*;
use crate::hsd::sis::{FmtArg, printf};

/// `mnCharSel_804DC584` to `804DC594`: the CPU levels' colors, very easy to very hard.
const LEVEL_COLORS: [[u8; 3]; 5] = [
    [20, 80, 160],
    [60, 140, 80],
    [160, 160, 0],
    [180, 80, 0],
    [220, 0, 0],
];
/// The CPU levels' names: "ＶＥＲＹ ＥＡＳＹ", "ＥＡＳＹ", "ＮＯＲＭＡＬ", "ＨＡＲＤ",
/// "ＶＥＲＹ ＨＡＲＤ".
const LEVEL_NAMES: [&[u8]; 5] = [
    b"\x82\x75\x82\x64\x82\x71\x82\x78\x20\x82\x64\x82\x60\x82\x72\x82\x78",
    b"\x82\x64\x82\x60\x82\x72\x82\x78",
    b"\x82\x6d\x82\x6e\x82\x71\x82\x6c\x82\x60\x82\x6b",
    b"\x82\x67\x82\x60\x82\x71\x82\x63",
    b"\x82\x75\x82\x64\x82\x71\x82\x78\x20\x82\x67\x82\x60\x82\x71\x82\x63",
];
/// `data2.xf0`: the stock icons' joints.
const STOCK_JOINTS: [u8; 5] = [0x35, 0x39, 0x36, 0x38, 0x37];
/// `mnCharSel_803F0EBC.name_list_joint`, `.tag_box_joint`.
pub(super) const NAME_LIST_JOINT: u8 = 0x4D;
pub(super) const TAG_BOX_JOINT: u8 = 0x4E;
/// "Ｆｔ．": the Home-Run Contest's feet.
const FEET: &[u8] = b"\x82\x65\x82\x94\x81\x44";
/// "ｍ".
const METERS: &[u8] = b"\x82\x8d";

/// `CSSMatchType`s past the regular matches' with their own records.
const STADIUM_TARGET: u8 = 0xF;
const STADIUM_HOMERUN: u8 = 0x10;
const REG_ADVENTURE: u8 = 0xC;
const REG_ALLSTAR: u8 = 0xD;
const EVENT_MATCH: u8 = 0xE;

impl Running<'_> {
    /// A name tag's list's joint: the one-door screen's one (`name_list_joint`), else the
    /// port's.
    pub(super) fn tag_list_node(&self, port: usize) -> Option<JObjId> {
        if self.game.charsel.doors_used == 1 {
            self.menu_node(NAME_LIST_JOINT)
        } else {
            self.menu_node(TAGS[port].list_joint)
        }
    }

    /// A name tag's box's joint: the one-door screen's one (`tag_box_joint`), else the port's.
    pub(super) fn tag_box_node(&self, port: usize) -> Option<JObjId> {
        if self.game.charsel.doors_used == 1 {
            self.menu_node(TAG_BOX_JOINT)
        } else {
            self.menu_node(TAGS[port].name_joint)
        }
    }

    /// A records text at `x` beside a joint's place (`lb_8000B1CC`), font sizes `w` and `h`,
    /// right-aligned (`default_alignment` 2) or not, with an empty string.
    fn charsel_record_text(
        &mut self,
        ctx: i32,
        p: Vec3,
        x: f32,
        y: Option<f32>,
        size: [f32; 2],
        right: bool,
    ) -> TextId {
        let text = self.create_text_buffer(0, ctx);
        let py = match y {
            Some(y) => mem(fp::fsubs(reg(y), reg(p.y))),
            None => mem(fp::fneg(reg(p.y))),
        };
        let t = self.game.sis.text_mut(text);
        t.pos = Vec3::new(mem(fp::fadds(reg(x), reg(p.x))), py, p.z);
        // The original sets the sizes before the alignment for a time and after it for a
        // distance; both are plain stores.
        t.font_size = size;
        if right {
            t.default_alignment = 2;
        }
        self.game.sis.add_string(text, 0.0, 0.0, None);
        text
    }

    /// The one-door screen's panel (`mnCharSel_802640A0`'s one-door switch): for Classic,
    /// Adventure and All-Star the CPU level's names (scrolling under the arrows) and the
    /// stock count (All-Star's hidden), the records' texts and their marks; for the Stadium
    /// modes and Training their panel's texts; and the VS screen's KO stars and team button
    /// hidden for the modes with no level.
    pub(super) fn charsel_one_door_panel(&mut self, ctx: i32) {
        let match_type = self.css().match_type;
        let menu = self.game.charsel.menu;
        match match_type {
            REG_CLASSIC | REG_ADVENTURE | REG_ALLSTAR => {
                let port0 = self.game.charsel.port0 as usize;
                let level = self.css().vs.start.players[port0].cpu_level;
                let c = &mut self.game.charsel;
                c.cpu_level_shown = level;
                c.cpu_level = level;
                c.cpu_slider_x = 124.0;
                let text = self.create_text_buffer(0, ctx);
                self.game.charsel.cpu_text = Some(text);
                self.game.charsel.cpu_scroll = 1;
                let t = self.game.sis.text_mut(text);
                t.pos = Vec3::new(-0.5, 9.0, 0.0);
                t.box_size = [248.0, 32.0];
                t.font_size = [0.08, 0.09];
                t.x4e = 1;
                t.default_kerning = 1;
                t.default_alignment = 1;
                t.x4c = 1;
                for (i, name) in LEVEL_NAMES.iter().enumerate() {
                    self.game.sis.add_string(text, 0.0, 0.0, Some(name));
                    self.game
                        .sis
                        .set_color(self.data, text, i as i32, LEVEL_COLORS[i]);
                }
                let entity = self.game.sis.text(text).entity.expect("a text's GObj");
                let menu_gobj = self.game.charsel.menu_gobj.expect("the menu's GObj");
                self.game.world.gx_relink_before(entity, menu_gobj);
                if match_type == REG_ALLSTAR {
                    for joint in [0x34, 0x3D, 0x45, 0x46] {
                        let j = self.menu_node(joint);
                        jobj::set_flags_all(&mut self.game.objs.jobjs, j, HIDDEN);
                    }
                } else {
                    let stocks = self.css().vs.start.players[port0].stocks as u8;
                    self.game.charsel.stocks = stocks;
                    self.charsel_stock_icons();
                    self.css_start().players[port0].stocks = stocks as i8;
                }
                let p = self.css_world(self.menu_node(0x40));
                let t = self.charsel_record_text(ctx, p, 13.5, None, [0.07, 0.055], true);
                self.game.charsel.texts[0] = Some(t);
                let j = self.menu_node(0x42);
                jobj::set_flags(&mut self.game.objs.jobjs, j, HIDDEN);
                let p = self.css_world(self.menu_node(0x41));
                let t = self.charsel_record_text(ctx, p, 13.5, None, [0.07, 0.055], true);
                self.game.charsel.texts[2] = Some(t);
                let j = self.css_animate(menu, 0x43, TOBJ_MASK, 0.0);
                jobj::set_flags(&mut self.game.objs.jobjs, j, HIDDEN);
                return;
            }
            STADIUM_TARGET | 0x11 | 0x12 => {
                // Target Test's times, and the 10-Man and 100-Man Melee's (`STADIUM_MULTIMAN_*`).
                let opts = self.game.charsel.options;
                let p = self.css_world(self.css_node(opts, 4));
                let t = self.charsel_record_text(ctx, p, 10.9, None, [0.07, 0.055], true);
                self.game.charsel.texts[0] = Some(t);
                let t = self.charsel_record_text(ctx, p, 11.6, Some(0.3), [0.06, 0.045], false);
                self.game.charsel.texts[1] = Some(t);
                let p = self.css_world(self.css_node(opts, 5));
                let t = self.charsel_record_text(ctx, p, 10.9, None, [0.07, 0.055], true);
                self.game.charsel.texts[2] = Some(t);
                let t = self.charsel_record_text(ctx, p, 11.6, Some(0.3), [0.06, 0.045], false);
                self.game.charsel.texts[3] = Some(t);
                let j = self.css_node(opts, 3);
                jobj::set_flags_all(&mut self.game.objs.jobjs, j, HIDDEN);
            }
            STADIUM_HOMERUN => {
                let opts = self.game.charsel.options;
                let p = self.css_world(self.css_node(opts, 4));
                let t = self.charsel_record_text(ctx, p, 11.0, None, [0.07, 0.055], true);
                self.game.charsel.texts[0] = Some(t);
                let units = self.create_text_buffer(0, ctx);
                let tt = self.game.sis.text_mut(units);
                tt.pos = Vec3::new(mem(fp::fadds(12.0, reg(p.x))), mem(fp::fneg(reg(p.y))), p.z);
                tt.font_size = [0.07, 0.055];
                tt.default_kerning = 1;
                let unit = if self.game.game_data.saved_language()
                    == super::super::super::gm::main_lib::LANG_JP
                {
                    METERS
                } else {
                    FEET
                };
                self.game.sis.add_string(units, 0.0, 0.0, Some(unit));
                self.game.sis.add_string(units, 0.0, 35.0, Some(unit));
                let p = self.css_world(self.css_node(opts, 5));
                let t = self.charsel_record_text(ctx, p, 11.0, None, [0.07, 0.055], true);
                self.game.charsel.texts[2] = Some(t);
            }
            TRAINING_MODE => {
                let door = self.game.charsel.door_model;
                let p = self.css_world(self.css_node(door, 9));
                let t = self.charsel_record_text(ctx, p, 10.0, None, [0.13, 0.055], true);
                self.game.charsel.texts[0] = Some(t);
            }
            0x13..=0x16 => {
                // 3-Minute, 15-Minute, Endless and Cruel Melee's KOs (`STADIUM_3_MIN_MELEE` to
                // `STADIUM_CRUEL_MELEE`).
                let opts = self.game.charsel.options;
                let p = self.css_world(self.css_node(opts, 4));
                let t = self.charsel_record_text(ctx, p, 16.0, None, [0.07, 0.055], true);
                self.game.charsel.texts[0] = Some(t);
                let p = self.css_world(self.css_node(opts, 5));
                let t = self.charsel_record_text(ctx, p, 16.0, None, [0.07, 0.055], true);
                self.game.charsel.texts[2] = Some(t);
            }
            EVENT_MATCH => {}
            other => crate::game::code::unported_call(&format!(
                "the one-door screen's panel for match type {other:#x}"
            )),
        }
        for joint in [0x2E, 0x34, 0x3A, 0x44] {
            let j = self.menu_node(joint);
            jobj::set_flags_all(&mut self.game.objs.jobjs, j, HIDDEN);
        }
    }

    /// The stock count's icons (the inline `updateStockIcons`): the first moved to 7.5, the
    /// rest shown up to the count.
    pub(super) fn charsel_stock_icons(&mut self) {
        let j = self.menu_node(STOCK_JOINTS[0]);
        self.css_translate_x(j, 7.5);
        for i in 1..5 {
            let j = self.menu_node(STOCK_JOINTS[i]);
            self.set_hidden(j, self.game.charsel.stocks <= i as u8);
        }
    }

    /// A records text's string, formatted as the original's `HSD_SisLib_803A70A0` call.
    fn charsel_record(&mut self, text: Option<TextId>, fmt: &[u8], args: &[FmtArg]) {
        let mut buf = [0u8; 0x100];
        let n = printf(fmt, args, &mut buf);
        let t = text.expect("a records text");
        self.game
            .sis
            .replace_string(self.data, t, 0, Some(&buf[..n]));
    }

    fn charsel_record_clear(&mut self, text: Option<TextId>) {
        let t = text.expect("a records text");
        self.game.sis.replace_string(self.data, t, 0, None);
    }

    /// A time record's texts (the inline `drawTimeText`): minutes and seconds, or hours too
    /// under a hundred, else 99:59:59; the hundredths beside.
    fn charsel_time_record(&mut self, hi: usize, lo: usize, t: [u8; 4]) {
        let (x, y) = (self.game.charsel.texts[hi], self.game.charsel.texts[lo]);
        let [h, m, s, cs] = t.map(i32::from);
        if h == 0 {
            self.charsel_record(x, b"%02d:%02d", &[FmtArg::Int(m), FmtArg::Int(s)]);
            self.charsel_record(y, b"%02d", &[FmtArg::Int(cs)]);
        } else if h < 100 {
            self.charsel_record(
                x,
                b"%d:%02d:%02d",
                &[FmtArg::Int(h), FmtArg::Int(m), FmtArg::Int(s)],
            );
            self.charsel_record(y, b"%02d", &[FmtArg::Int(cs)]);
        } else {
            self.charsel_record(x, b"99:59:59", &[]);
            self.charsel_record(y, b"99", &[]);
        }
    }

    /// A count of KOs (the inline `show_kos`): `"%d KOs"` in the save's language.
    fn charsel_kos(&mut self, text: Option<TextId>, n: i32) {
        let fmt: &[u8] =
            if self.game.game_data.saved_language() == super::super::super::gm::main_lib::LANG_JP {
                b"%d \x90\x6c\x94\xb2\x82\xab"
            } else {
                b"%d \x82\x6a\x82\x6e\x82\x93"
            };
        self.charsel_record(text, fmt, &[FmtArg::Int(n)]);
    }

    /// A rank mark of the menu's (the inline `inline3`): joint `joint`'s material animation
    /// held on `rank + 1`.
    fn charsel_rank_mark(&mut self, joint: u8, rank: u8) -> Option<JObjId> {
        let menu = self.game.charsel.menu;
        let frame = mem(fp::fadds(reg(f32::from(rank)), 1.0));
        self.css_animate(menu, joint, MOBJ_MASK, frame)
    }

    /// The records the one-door screen shows for door 0's character (`mnCharSel_8025C020`),
    /// cleared with `hidden` (the door empty): the mode's high score and its clear mark, the
    /// total of every character's and the mark of the worst level cleared with all; Target
    /// Test's targets broken or best time and the total time; the Home-Run Contest's best
    /// and total distances; Training's best combo.
    #[doc(alias = "mnCharSel_8025C020")]
    pub(super) fn charsel_records(&mut self, hidden: bool) {
        let icon = usize::from(self.game.charsel.doors[0].sel_icon);
        let hud = ICONS[icon].hud;
        let texts = self.game.charsel.texts;
        if hidden {
            if texts[0].is_some() {
                self.charsel_record_clear(texts[0]);
            }
            if texts[1].is_some() {
                self.charsel_record_clear(texts[1]);
            }
        }
        let match_type = self.css().match_type;
        let g = &*self.game;
        match match_type {
            REG_CLASSIC | REG_ADVENTURE | REG_ALLSTAR => {
                let mode = (match_type - REG_CLASSIC) as usize;
                let rec = g.record_1p(hud);
                let rank = [rec.x80, rec.x81, rec.x82][mode];
                let cleared = [rec.b4, rec.b5, rec.b6][mode];
                let score = g.high_score_1p(mode, hud);
                let total = g.high_score_1p_total(mode);
                let all = g.cleared_1p_all(mode);
                let worst = g.cleared_1p_worst(mode);
                let j = self.charsel_rank_mark(0x42, rank);
                if hidden {
                    jobj::set_flags(&mut self.game.objs.jobjs, j, HIDDEN);
                } else {
                    self.charsel_record(texts[0], b"%08d", &[FmtArg::Int(score)]);
                    if cleared {
                        jobj::clear_flags(&mut self.game.objs.jobjs, j, HIDDEN);
                    }
                }
                self.charsel_record(texts[2], b"%09d", &[FmtArg::Int(total)]);
                if all {
                    let j = self.charsel_rank_mark(0x43, worst);
                    jobj::clear_flags(&mut self.game.objs.jobjs, j, HIDDEN);
                }
            }
            STADIUM_TARGET => {
                if !hidden {
                    let targets = g.target_record(hud);
                    if targets != -1 {
                        let fmt: &[u8] = if g.game_data.saved_language()
                            == super::super::super::gm::main_lib::LANG_JP
                        {
                            b"%d \x8c\xc2"
                        } else {
                            b"%d \x81\x40"
                        };
                        self.charsel_record(texts[0], fmt, &[FmtArg::Int(targets)]);
                    } else {
                        let t = self.game.target_time(hud);
                        self.charsel_time_record(0, 1, t);
                    }
                }
                let j = self.css_node(self.game.charsel.options, 3);
                if let Some(t) = self.game.target_time_total() {
                    self.charsel_time_record(2, 3, t);
                    jobj::clear_flags_all(&mut self.game.objs.jobjs, j, HIDDEN);
                } else {
                    self.charsel_record_clear(texts[2]);
                    self.charsel_record_clear(texts[3]);
                    jobj::set_flags_all(&mut self.game.objs.jobjs, j, HIDDEN);
                }
            }
            STADIUM_HOMERUN => {
                let jp = g.game_data.saved_language() == super::super::super::gm::main_lib::LANG_JP;
                // Centimetres to metres or feet, cut to one decimal (`toMeters`, `toFeet`).
                let unit = if jp { 100.0 } else { 30.4788 };
                let tenths = |cm: f64| {
                    let v = fp::fmuls(10.0, fp::fdivs(cm, reg(unit)));
                    fp::fdivs(fp::frsp(f64::from(fp::fctiwz(v))), 10.0)
                };
                if !hidden {
                    let d = tenths(fp::frsp(f64::from(g.homerun_record(hud))));
                    self.charsel_record_f1(texts[0], d);
                }
                let total = tenths(fp::frsp(f64::from(self.game.homerun_record_total())));
                self.charsel_record_f1(texts[2], total);
            }
            TRAINING_MODE => {
                if !hidden {
                    let combo = i32::from(g.training_record(hud));
                    self.charsel_record(texts[0], b"%d", &[FmtArg::Int(combo)]);
                }
            }
            0x11 | 0x12 => {
                // 10-Man's and 100-Man's: the character's KOs, or its best time once won; the
                // total time once every character has won.
                let hundred = match_type == 0x12;
                if !hidden {
                    if !g.multi_man_won(hundred, hud) {
                        let kos = g.multi_man_best(hundred, hud);
                        self.charsel_kos(texts[0], kos);
                    } else {
                        let t = self.game.multi_man_time(hundred, hud);
                        self.charsel_time_record(0, 1, t);
                    }
                }
                let j = self.css_node(self.game.charsel.options, 3);
                if let Some(t) = self.game.multi_man_time_total(hundred) {
                    self.charsel_time_record(2, 3, t);
                    jobj::clear_flags_all(&mut self.game.objs.jobjs, j, HIDDEN);
                } else {
                    self.charsel_record_clear(texts[2]);
                    self.charsel_record_clear(texts[3]);
                    jobj::set_flags_all(&mut self.game.objs.jobjs, j, HIDDEN);
                }
            }
            0x13..=0x16 => {
                // The timed, Endless and Cruel Melee's best KOs and their total.
                let mode = match_type - 0x13;
                if !hidden {
                    let kos = g.multi_man_kos(mode, hud);
                    self.charsel_kos(texts[0], kos);
                }
                let total = self.game.multi_man_kos_total(mode);
                self.charsel_kos(texts[2], total);
            }
            EVENT_MATCH => {}
            other => crate::game::code::unported_call(&format!(
                "mnCharSel_8025C020's records for match type {other:#x}"
            )),
        }
    }

    /// A distance printed with `"%.1f"` (MSL's `vsnprintf`): the float's value rounded to
    /// one decimal.
    fn charsel_record_f1(&mut self, text: Option<TextId>, v: f64) {
        let s = format!("{v:.1}");
        let t = text.expect("a records text");
        self.game
            .sis
            .replace_string(self.data, t, 0, Some(s.as_bytes()));
    }

    /// The CPU level's names scrolling to the level picked (`fn_8025F0E0`'s one-door part):
    /// 31 a frame toward `124 - 248 * level`, each name 248 apart.
    pub(super) fn charsel_cpu_slider(&mut self) {
        let c = &mut self.game.charsel;
        if c.cpu_scroll == 0 {
            return;
        }
        let target = |level: u8| fp::fnmsubs(248.0, fp::frsp(f64::from(level)), 124.0);
        if c.cpu_level != c.cpu_level_shown {
            if c.cpu_level < c.cpu_level_shown {
                c.cpu_slider_x = mem(fp::fadds(reg(c.cpu_slider_x), 31.0));
                if reg(c.cpu_slider_x) >= target(c.cpu_level) {
                    c.cpu_level_shown = c.cpu_level;
                }
            } else {
                c.cpu_slider_x = mem(fp::fsubs(reg(c.cpu_slider_x), 31.0));
                if reg(c.cpu_slider_x) <= target(c.cpu_level) {
                    c.cpu_level_shown = c.cpu_level;
                }
            }
        }
        if c.cpu_level == c.cpu_level_shown {
            c.cpu_scroll = 0;
            c.cpu_slider_x = mem(target(c.cpu_level));
        }
        let text = c.cpu_text.expect("the CPU level's names");
        let x = c.cpu_slider_x;
        for j in 0..5 {
            let px = fp::fmadds(248.0, fp::frsp(f64::from(j)), reg(x));
            self.game.sis.set_position(self.data, text, j, px, 0.0);
        }
    }
}
