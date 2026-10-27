//! The results screen's panel (`gmresult.c`'s model and texts, `gmresultplayer.c`'s steps):
//! the panel coming in (`fn_80178BB4`: its model, each player's column, the match kind's
//! title, the KO counts and times, the places' colors, the human players' KO count), the
//! players' rows (`fn_80177748`), the wait for a button (`fn_80177920`), and each player's
//! statistics pages (`fn_80178050`, `fn_80174B4C`), which humans turn and scroll and
//! computers page through on their own.

use gekko_fp as fp;
use melee_data::archive::DataRef;

use super::super::handles::CodeRef;
use super::super::state::{OBJ_JOBJ, Obj, Running};
use super::results_scene::{
    COLUMNS, DASHES, MINUS, NO_TIME, PANEL_RENDER, PKIND_CPU, PKIND_HUMAN, PKIND_NA, PLUS,
    STATS_TEXT_RENDER, TITLE_OFFSET, canceled, column_color,
};
use crate::fpu::mem;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::hsd::sis::{FmtArg, TextId};
use crate::vec::Vec3;

/// `JOBJ_HIDDEN`, `TOBJ_MASK` and `ALL_TYPE_MASK`.
const HIDDEN: u32 = 1 << 4;
const TOBJ_MASK: u32 = jobj::type_mask::TOBJ;
const ALL_TYPE_MASK: u32 = 0xFFFF;

/// `PAD_BUTTON_A`, `B`, `START`, and the stick's right and left as triggers.
const PAD_A: u32 = 0x100;
const PAD_B: u32 = 0x200;
const PAD_START: u32 = 0x1000;
const STICK_RIGHT: u32 = 1 << 19;
const STICK_LEFT: u32 = 1 << 18;

/// A statistics page's entry (`StatsEntry`): its label's string in the panel's font (or
/// none), the getter of its value and the getter of its label (the opponent's name), by
/// address.
#[derive(Clone, Copy)]
struct StatsEntry {
    value: i16,
    check: u32,
    get: u32,
}

const fn e(value: i16, check: u32, get: u32) -> StatsEntry {
    StatsEntry { value, check, get }
}

/// `lbl_803D6488`: the KOs page.
const KOS_PAGE: [StatsEntry; 13] = [
    e(0x08, 0, 0),
    e(0x08, 0, 0),
    e(-1, 0x8017_AE70, 0x8017_BB94),
    e(-1, 0x8017_AED8, 0x8017_BC50),
    e(-1, 0x8017_AF40, 0x8017_BD0C),
    e(-1, 0x8017_AFA8, 0x8017_BDC8),
    e(0x09, 0, 0),
    e(-1, 0x8017_B07C, 0x8017_BB94),
    e(-1, 0x8017_B0E4, 0x8017_BC50),
    e(-1, 0x8017_B14C, 0x8017_BD0C),
    e(-1, 0x8017_B1B4, 0x8017_BDC8),
    e(0x0C, 0x8017_B21C, 0),
    e(0x0C, 0, 0),
];

/// `lbl_803D6558`: the statistics page.
const STATS_PAGE: [StatsEntry; 48] = [
    e(0x0A, 0, 0),
    e(0x0A, 0, 0),
    e(0x0B, 0x8017_AE0C, 0),
    e(0x0E, 0, 0),
    e(0x0F, 0x8017_B010, 0),
    e(0x0C, 0, 0),
    e(0x0D, 0x8017_B21C, 0),
    e(0x10, 0, 0),
    e(0x11, 0x8017_B280, 0),
    e(0x12, 0, 0),
    e(0x13, 0x8017_B2E4, 0),
    e(0x14, 0, 0),
    e(0x15, 0x8017_B348, 0),
    e(0x16, 0, 0),
    e(0x17, 0x8017_B3AC, 0),
    e(0x18, 0, 0),
    e(0x19, 0x8017_B410, 0),
    e(0x1A, 0, 0),
    e(0x1B, 0x8017_B4D0, 0),
    e(0x1C, 0, 0),
    e(0x1D, 0x8017_B534, 0),
    e(0x1E, 0, 0),
    e(0x1F, 0x8017_B598, 0),
    e(0x20, 0, 0),
    e(0x21, 0x8017_B5FC, 0),
    e(0x22, 0, 0),
    e(0x23, 0x8017_B660, 0),
    e(0x24, 0, 0),
    e(0x25, 0x8017_B6C4, 0),
    e(0x26, 0, 0),
    e(0x27, 0x8017_B728, 0),
    e(0x28, 0, 0),
    e(0x29, 0x8017_B78C, 0),
    e(0x2A, 0, 0),
    e(0x2B, 0x8017_B7F0, 0),
    e(0x2C, 0, 0),
    e(0x2D, 0x8017_B854, 0),
    e(0x2E, 0, 0),
    e(0x2F, 0x8017_B8B8, 0),
    e(0x30, 0, 0),
    e(0x31, 0x8017_B91C, 0),
    e(0x32, 0, 0),
    e(0x33, 0x8017_B9F4, 0),
    e(0x34, 0, 0),
    e(0x35, 0x8017_BACC, 0),
    e(0x36, 0, 0),
    e(0x37, 0x8017_BB30, 0),
    e(0x36, 0, 0),
];

/// A statistics page as `fn_801748EC` finds it (`StatsList`): the KOs and statistics pages
/// from `lbl_803D6878`, or the bonuses' page, a player's `lbl_8046E190` entry taken as a
/// list (mode 2, its count; its entries never read).
#[derive(Clone, Copy)]
struct StatsList {
    mode: u8,
    count: u8,
    entries: &'static [StatsEntry],
}

/// `lbl_803B7B18`'s English and Japanese player labels (`lbl_803D79E0`, `lbl_803D79D0`), in
/// Shift-JIS.
const PLAYER_LABELS_US: [&[u8]; 4] = [
    &[0x82, 0x6F, 0x82, 0x50, 0],
    &[0x82, 0x6F, 0x82, 0x51, 0],
    &[0x82, 0x6F, 0x82, 0x52, 0],
    &[0x82, 0x6F, 0x82, 0x53, 0],
];
const PLAYER_LABELS_JP: [&[u8]; 4] = [
    &[0x82, 0x50, 0x82, 0x6F, 0],
    &[0x82, 0x51, 0x82, 0x6F, 0],
    &[0x82, 0x52, 0x82, 0x6F, 0],
    &[0x82, 0x53, 0x82, 0x6F, 0],
];

impl Running<'_> {
    fn rp_node(&self, root: Option<JObjId>, index: i32) -> Option<JObjId> {
        super::pause::tree_nodes(&self.game.objs.jobjs, root, &[index])[0]
    }

    fn rp_hide(&mut self, j: Option<JObjId>) {
        jobj::set_flags_all(&mut self.game.objs.jobjs, j, HIDDEN);
    }

    fn rp_show(&mut self, j: Option<JObjId>) {
        jobj::clear_flags_all(&mut self.game.objs.jobjs, j, HIDDEN);
    }

    fn rp_anim_all(&mut self, j: Option<JObjId>) {
        jobj::anim_all(
            &mut self.game.objs,
            self.data,
            &mut super::super::code::ParticleHooks,
            j,
        );
    }

    /// `HSD_ForeachAnim(j, JOBJ_TYPE, ALL_TYPE_MASK, HSD_AObjSetRate, AOBJ_ARG_AF, rate)`.
    fn rp_foreach_rate(&mut self, j: Option<JObjId>, rate: f32) {
        let Some(j) = j else { return };
        jobj::foreach_anim(&mut self.game.objs, j, ALL_TYPE_MASK, &mut |objs, a| {
            crate::hsd::aobj::set_rate(&mut objs.aobjs, Some(a), rate);
        });
    }

    /// `HSD_ForeachAnim(j, JOBJ_TYPE, ALL_TYPE_MASK, HSD_AObjSetCurrentFrame, AOBJ_ARG_AF,
    /// frame)`.
    fn rp_foreach_frame(&mut self, j: Option<JObjId>, frame: f32) {
        let Some(j) = j else { return };
        jobj::foreach_anim(&mut self.game.objs, j, ALL_TYPE_MASK, &mut |objs, a| {
            crate::hsd::aobj::set_current_frame(&mut objs.aobjs, &mut objs.fobjs, Some(a), frame);
        });
    }

    /// The rate, then the frame, then `HSD_JObjAnimAll` (`gmresultplayer.c`'s `inline0`).
    fn rp_freeze_at(&mut self, j: Option<JObjId>, frame: f32) {
        self.rp_foreach_rate(j, 0.0);
        self.rp_foreach_frame(j, frame);
        self.rp_anim_all(j);
    }

    /// A joint's first display object's material's first texture.
    fn rp_tobj(&self, j: JObjId, second_dobj: bool) -> Option<crate::hsd::tobj::TObjId> {
        let o = &self.game.objs;
        let mut d = o.jobjs[j].dobj();
        if second_dobj {
            d = d.and_then(|d| o.dobjs[d].next);
        }
        let m = d.and_then(|d| o.dobjs[d].mobj);
        m.and_then(|m| o.mobjs[m].tobj)
    }

    /// The title's texture animation: `anim` requested, and the frames of its picture (the
    /// first 30, or 30 from the picture's from 180 on), its texture animations stopped
    /// (`fn_80178BB4`'s three copies, on `x30`'s second display object).
    fn rp_title_texture(&mut self, anim: i32) {
        let x30 = self.game.results.data.x30.expect("the panel's title joint");
        let data = self.data;
        let t = self.rp_tobj(x30, true);
        let a = t.and_then(|t| self.game.objs.tobjs[t].aobj);
        let objs = &mut self.game.objs;
        crate::hsd::tobj::req_anim(objs, t, mem(fp::frsp(f64::from(anim))));
        crate::hsd::tobj::anim(objs, data, t);
        if anim < 0x19 {
            crate::hsd::aobj::set_current_frame(&mut objs.aobjs, &mut objs.fobjs, a, 0.0);
            crate::hsd::aobj::set_end_frame(&mut objs.aobjs, a, 29.0);
        } else {
            let start = fp::fctiwz(fp::fmuls(
                30.0,
                fp::frsp(f64::from(anim.wrapping_sub(0xB4))),
            ));
            let s = fp::frsp(f64::from(start));
            crate::hsd::aobj::set_current_frame(&mut objs.aobjs, &mut objs.fobjs, a, mem(s));
            crate::hsd::aobj::set_end_frame(
                &mut objs.aobjs,
                a,
                mem(fp::fsubs(fp::fadds(30.0, s), 1.0)),
            );
        }
        super::super::hud::status::stop_anim_type(objs, data, x30, 1, TOBJ_MASK);
    }

    /// The panel comes in, `fn_80178BB4`: the flames' fade (`fn_80179854`), the panel's model
    /// and texts (`fn_80175DC8`, `fn_80175C5C`), the title's picture after a cancelled match
    /// or a team battle, each column set up, the human players' KOs counted up as coins,
    /// and the screen's next state.
    #[doc(alias = "fn_80178BB4")]
    pub(super) fn results_panel_in(&mut self, gobj: GObjId) {
        let me = self.results_end().clone();
        let mut ko_count = 0i32;
        self.results_flames_away();
        self.results_panel_model(gobj);
        self.results_panel_texts();
        if canceled(me.outcome) {
            self.rp_title_texture(0xB8);
        } else if me.is_teams == 1 {
            let t = i32::from(self.game.results.data.x5) + 0xB5;
            self.rp_title_texture(t);
        }
        for i in 0..6 {
            let js = self.game.results.data.players[i].jobjs;
            for k in [0, 4, 8, 9, 10, 11] {
                self.rp_hide(js[k]);
            }
            let d6 = js[6].and_then(|j| self.game.objs.jobjs[j].dobj());
            crate::lb::anim::dobj_set_rate_all(&mut self.game.objs, d6, 0.0);
            let p = &me.player_standings[i];
            if p.pkind == PKIND_HUMAN {
                ko_count = ko_count.wrapping_add(i32::from(p.xE));
            }
            if p.pkind == PKIND_NA {
                self.rp_hide(js[1]);
                jobj::remove_anim_all(&mut self.game.objs, js[1]);
                self.rp_hide(js[5]);
                self.rp_hide(js[7]);
                continue;
            }
            let (ckind, cid) = (i32::from(p.ckind), i32::from(p.ftkind));
            if !canceled(me.outcome) && me.is_teams == 0 && p.is_big_loser == 0 {
                let t = fp::fctiwz(super::vs::stock_icon_frame(ckind, cid, 0));
                self.rp_title_texture(t);
            }
            let icon = fp::fctiwz(super::vs::stock_icon_frame(ckind, cid, 0));
            self.results_icon_frame(js[5], icon);
            let rank =
                super::vs::player_color(i as u8, p.team, u8::from(me.is_teams == 1), p.pkind);
            let j2 = js[2].expect("a column's rank joint");
            let o = &mut self.game.objs;
            let m = o.jobjs[j2].dobj().and_then(|d| o.dobjs[d].mobj);
            let ra = m.and_then(|m| o.mobjs[m].aobj);
            let f = mem(fp::fadds(1.0, fp::frsp(f64::from(rank as u8))));
            crate::hsd::aobj::set_current_frame(&mut o.aobjs, &mut o.fobjs, ra, f);
            crate::hsd::aobj::set_rate(&mut o.aobjs, ra, 0.0);
            if me.match_kind != 3 {
                let costume = i32::from(p.x3_u[0] >> 2);
                let taunt = super::vs::stock_icon_frame(ckind, cid, costume);
                self.rp_foreach_rate(js[7], 0.0);
                self.rp_foreach_frame(js[7], mem(taunt));
                self.rp_anim_all(js[7]);
                let a7 = js[7].and_then(|j| self.game.objs.jobjs[j].aobj);
                let o = &mut self.game.objs;
                crate::hsd::aobj::set_rate(&mut o.aobjs, a7, 1.0);
                crate::hsd::aobj::set_current_frame(&mut o.aobjs, &mut o.fobjs, a7, 0.0);
            } else {
                self.rp_foreach_rate(js[7], 0.0);
                self.rp_foreach_frame(js[7], 0.0);
                self.rp_anim_all(js[7]);
                self.rp_hide(js[7]);
            }
            jobj::remove_anim_all(&mut self.game.objs, js[3]);
            self.rp_hide(js[3]);
        }
        if ko_count > 0 {
            self.game.start_coin_counter(0x6C, 0x78, ko_count, 5);
        }
        self.game.results.data.state = 1;
    }

    /// A column's character icon at `frame`, frozen (`fn_80174FD0`).
    #[doc(alias = "fn_80174FD0")]
    fn results_icon_frame(&mut self, j: Option<JObjId>, frame: i32) {
        let j = j.expect("a column's icon joint");
        let data = self.data;
        let t = self.rp_tobj(j, false);
        let o = &mut self.game.objs;
        let a = t.and_then(|t| o.tobjs[t].aobj);
        let f = mem(fp::frsp(f64::from(frame)));
        crate::hsd::aobj::set_current_frame(&mut o.aobjs, &mut o.fobjs, a, f);
        crate::hsd::aobj::set_rate(&mut o.aobjs, a, 0.0);
        crate::hsd::tobj::anim(o, data, t);
    }

    /// The panel's model, `fn_80175DC8`: `pnlsce`'s first model on the model GObj with its
    /// animations at their first frame, the columns' joints (`fn_80179F84`), the title's
    /// and the rows' joints, each player's fifteen joints and its statistics' spot, the
    /// match kind's title animation (`fn_801785B0`) and its text.
    #[doc(alias = "fn_80175DC8")]
    fn results_panel_model(&mut self, gobj: GObjId) {
        let data = self.data;
        let me = self.results_end().clone();
        let pnl = self.game.results.data.pnlsce.expect("the panel's scene");
        let models = data.ptr(pnl).expect("the panel's models");
        let model: DataRef = data.ptr(models).expect("the panel's model");
        let g = &mut *self.game;
        let root = jobj::load_joint(&mut g.objs, data, data.ptr(model)).expect("the panel's joint");
        g.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(root));
        g.world.setup_gx_link(gobj, CodeRef(PANEL_RENDER), 11, 0);
        crate::lb::anim::add_model_anim(&mut g.objs, data, Some(root), 0, model);
        jobj::req_anim_all(&mut g.objs, Some(root), 0.0);
        self.rp_anim_all(Some(root));
        let r = Some(root);
        let columns = self.rp_node(r, 0x41);
        self.results_columns(columns);
        let x24 = self.rp_node(r, 0x68);
        self.game.results.data.x24 = x24;
        let x28 = self.rp_node(r, 0x69);
        self.game.results.data.x28 = x28;
        for i in 0..6 {
            let n = self.rp_node(r, i as i32 + 0x62);
            self.game.results.data.x34[i] = n;
        }
        let x30 = self.rp_node(r, 0xA);
        self.game.results.data.x30 = x30;
        for j in 0..4 {
            self.game.results.data.players[j].done = false;
            for (k, col) in COLUMNS.iter().enumerate() {
                let n = self.rp_node(r, col[j]);
                self.game.results.data.players[j].jobjs[k] = n;
            }
            let j12 = self.game.results.data.players[j].jobjs[12];
            let p = crate::lb::joint::world_position(&mut self.game.objs, data, j12, None);
            self.game.results.data.players[j].stats_position = p;
        }
        self.results_title_anim(gobj);
        let id = match me.match_kind {
            0 => 2,
            1 => 3,
            2 => 4,
            3 => 5,
            _ => return,
        };
        if let Some(t) = self.game.results.data.x2c {
            self.destroy_text(t);
        }
        let x28 = self.game.results.data.x28;
        let p =
            crate::lb::joint::world_position(&mut self.game.objs, data, x28, Some(TITLE_OFFSET));
        let t = self.create_text(0, 0, Vec3::new(p.x, -p.y, p.z), 12.0, 300.0);
        self.game.sis.reset_text(data, t, id);
    }

    /// The match kind's title, `fn_801785B0`: the other kinds' joints' animations removed and
    /// the kind's own shown at the frame of the Sudden Death penalty's size, its texture
    /// animations stopped (by the match kind: stock 0x15, time 0x11, coins 0xC, bonus 0x10).
    #[doc(alias = "fn_801785B0")]
    fn results_title_anim(&mut self, gobj: GObjId) {
        let me = self.results_end().clone();
        let root = match self.game.world.gobj(gobj).obj {
            Some(Obj::JObj(j)) => Some(j),
            other => panic!("the results' model holds {other:?}"),
        };
        let (removed, shown): ([i32; 3], i32) = match me.match_kind {
            2 => ([0x15, 0x11, 0x10], 0xC),
            1 => ([0x15, 0xC, 0x10], 0x11),
            3 => ([0x11, 0xC, 0x15], 0x10),
            _ => ([0x11, 0xC, 0x10], 0x15),
        };
        for n in removed {
            let c = self.rp_node(root, n);
            jobj::remove_anim_all(&mut self.game.objs, c);
        }
        let node = self.rp_node(root, shown);
        let raw = i32::from(me.sd_penalty);
        let fv = mem(fp::frsp(f64::from(raw.abs())));
        self.rp_foreach_rate(node, 0.0);
        self.rp_foreach_frame(node, fv);
        self.rp_anim_all(node);
        self.rp_foreach_frame(node, 0.0);
        self.rp_foreach_rate(node, 1.0);
        let n = node.expect("the match kind's title joint");
        super::super::hud::status::stop_anim_type(&mut self.game.objs, self.data, n, 1, TOBJ_MASK);
    }

    /// The panel's texts, `fn_80175C5C`: the rows' spots and the panel's top (`xC`, the
    /// first row below it), two text buffers made and left, then each player's time (or
    /// score, coins or bonus) and KO counts.
    #[doc(alias = "fn_80175C5C")]
    fn results_panel_texts(&mut self) {
        let data = self.data;
        let x24 = self.game.results.data.x24;
        let position = crate::lb::joint::world_position(&mut self.game.objs, data, x24, None);
        for i in 0..6 {
            let j = self.game.results.data.x34[i];
            let p = crate::lb::joint::world_position(&mut self.game.objs, data, j, None);
            self.game.results.data.x4c[i] = p;
        }
        let d = &mut self.game.results.data;
        d.xc = mem(fp::fsubs(f64::from(d.x4c[0].y), f64::from(position.y)));
        self.create_text_buffer(0, 0);
        self.create_text_buffer(0, 0);
        for i in 0..4 {
            self.results_column_time(i, position);
            self.results_column_kos(i);
        }
    }

    /// Prints a panel text's line (`HSD_SisLib_803A6B98` with the panel's arguments) at
    /// (`x`, `y`), scaled and colored.
    #[allow(clippy::too_many_arguments)]
    fn rp_line(
        &mut self,
        t: TextId,
        y: f32,
        fmt: &[u8],
        args: &[FmtArg],
        sx: f32,
        sy: f32,
        color: [u8; 4],
    ) {
        let mut s = [0u8; 0x200];
        let len = crate::hsd::sis::printf(fmt, args, &mut s);
        let id = self
            .game
            .sis
            .add_string(t, 0.0, f64::from(y), Some(&s[..=len]));
        let data = self.data;
        self.game
            .sis
            .set_scale(data, t, id, f64::from(sx), f64::from(sy));
        self.game
            .sis
            .set_color(data, t, id, [color[0], color[1], color[2]]);
    }

    /// A column's KO counts, `fn_80175240` (not in a bonus match): its KOs, falls and
    /// self-destructs (at most 999) three rows apart, in the column's color; dashes for an
    /// empty column.
    #[doc(alias = "fn_80175240")]
    fn results_column_kos(&mut self, slot: usize) {
        let me = self.results_end().clone();
        if me.match_kind == 3 {
            return;
        }
        let d = &self.game.results.data;
        let first = fp::fmuls(
            1.14,
            fp::fsubs(f64::from(d.x4c[4].y), f64::from(d.x4c[0].y)),
        );
        let second = fp::fmuls(
            1.12,
            fp::fsubs(f64::from(d.x4c[5].y), f64::from(d.x4c[4].y)),
        );
        let color = column_color(&me, slot);
        let t = self.create_text_buffer(0, 0);
        self.game.results.data.players[slot].ko_count = Some(t);
        let at = self.game.results.data.x4c[slot];
        let tx = self.game.sis.text_mut(t);
        tx.pos = Vec3::new(at.x, mem(fp::fsubs(-f64::from(at.y), 30.0)), at.z);
        tx.default_alignment = 1;
        tx.default_kerning = 1;
        let rows = [0.0, mem(-first), mem(fp::fsubs(-first, second))];
        let (sx, sy) = (0.11_f32, 0.08_f32);
        if me.player_standings[slot].pkind != PKIND_NA {
            let values = [
                super::results_stats::stat_value(&me, self.saved_us(), 0x8017_AE0C, slot),
                super::results_stats::stat_value(&me, self.saved_us(), 0x8017_B010, slot),
                super::results_stats::stat_value(&me, self.saved_us(), 0x8017_B21C, slot),
            ];
            for (y, v) in rows.into_iter().zip(values) {
                self.rp_line(t, y, b"%d\0", &[FmtArg::Int(v.min(999))], sx, sy, color);
            }
        } else {
            let grey = [0xA0, 0xA0, 0xA0, color[3]];
            for y in rows {
                self.rp_line(t, y, b"%s\0", &[FmtArg::Str(MINUS)], sx, sy, grey);
            }
        }
    }

    /// A column's time, score, coins or bonus, `fn_80175A94`: its text at the column's spot
    /// (or past its statistics' spot, in a bonus match) at the panel's top.
    #[doc(alias = "fn_80175A94")]
    fn results_column_time(&mut self, slot: usize, position: Vec3) {
        let me = self.results_end().clone();
        let data = self.data;
        let x = if me.match_kind == 3 {
            let j12 = self.game.results.data.players[slot].jobjs[12];
            let p = crate::lb::joint::world_position(&mut self.game.objs, data, j12, None);
            mem(fp::fadds(5.5, f64::from(p.x)))
        } else {
            self.game.results.data.x4c[slot].x
        };
        let t = self.create_text_buffer(0, 0);
        self.game.results.data.players[slot].ko_time = Some(t);
        let tx = self.game.sis.text_mut(t);
        tx.pos = Vec3::new(x, -position.y, position.z);
        tx.default_alignment = 1;
        tx.default_kerning = 1;
        match me.match_kind {
            2 => {
                let mut c = column_color(&me, slot);
                let line = if me.player_standings[slot].pkind != PKIND_NA {
                    let coins = super::results_stats::stat_value(&me, false, 0x8017_ADA8, slot);
                    let v = super::results_stats::clamp5(coins);
                    (b"%d\0".as_slice(), FmtArg::Int(v))
                } else {
                    c = [0xA0, 0xA0, 0xA0, c[3]];
                    (b"%s\0".as_slice(), FmtArg::Str(MINUS))
                };
                self.rp_line(t, -30.0, line.0, &[line.1], 0.09, 0.08, c);
            }
            1 => self.results_column_ko_time(slot),
            3 => self.results_column_bonus(slot),
            _ => self.results_column_score(slot),
        }
    }

    /// A signed number with its sign in Shift-JIS (`fn_8017556C`'s and `fn_801756E0`'s).
    fn rp_signed(&mut self, t: TextId, v: i32, sx: f32, color: [u8; 4]) {
        if v < 0 {
            let v = v.wrapping_neg();
            self.rp_line(
                t,
                -30.0,
                b"%s%d\0",
                &[FmtArg::Str(MINUS), FmtArg::Int(v)],
                sx,
                0.08,
                color,
            );
        } else if v > 0 {
            self.rp_line(
                t,
                -30.0,
                b"%s%d\0",
                &[FmtArg::Str(PLUS), FmtArg::Int(v)],
                sx,
                0.08,
                color,
            );
        } else {
            self.rp_line(t, -30.0, b"%d\0", &[FmtArg::Int(v)], sx, 0.08, color);
        }
    }

    /// A stock match's score, `fn_8017556C`.
    #[doc(alias = "fn_8017556C")]
    fn results_column_score(&mut self, slot: usize) {
        let me = self.results_end().clone();
        let c = column_color(&me, slot);
        let t = self.game.results.data.players[slot]
            .ko_time
            .expect("a column's time text");
        if me.player_standings[slot].pkind != PKIND_NA {
            let v = super::results_stats::clamp4(me.player_standings[slot].score);
            self.rp_signed(t, v, 0.11, c);
        } else {
            let grey = [0xA0, 0xA0, 0xA0, c[3]];
            self.rp_line(t, -30.0, b"%s\0", &[FmtArg::Str(MINUS)], 0.11, 0.08, grey);
        }
    }

    /// A bonus match's score, `fn_801756E0` (dashes after a cancelled match).
    #[doc(alias = "fn_801756E0")]
    fn results_column_bonus(&mut self, slot: usize) {
        let me = self.results_end().clone();
        let c = column_color(&me, slot);
        let t = self.game.results.data.players[slot]
            .ko_time
            .expect("a column's time text");
        if me.player_standings[slot].pkind != PKIND_NA && !canceled(me.outcome) {
            let v = super::results_stats::clamp7(me.player_standings[slot].score);
            self.rp_signed(t, v, 0.08, c);
        } else {
            let grey = [0xA0, 0xA0, 0xA0, c[3]];
            self.rp_line(t, -30.0, b"%s\0", &[FmtArg::Str(MINUS)], 0.08, 0.08, grey);
        }
    }

    /// A time match's time of the column's last KO, `fn_80175880`: dashes for the winner,
    /// a winning team's players still in or out at the end, and after a cancelled match a
    /// player still in or tied with the winner; else its minutes and seconds.
    #[doc(alias = "fn_80175880")]
    fn results_column_ko_time(&mut self, slot: usize) {
        let me = self.results_end().clone();
        let mut c = column_color(&me, slot);
        let t = self.game.results.data.players[slot]
            .ko_time
            .expect("a column's time text");
        let x6 = usize::from(self.game.results.data.x6);
        let p = &me.player_standings[slot];
        let w = &me.player_standings[x6];
        if p.pkind != PKIND_NA {
            let dashes = x6 == slot
                || (me.is_teams == 1
                    && me.team_standings[usize::from(p.team)].is_big_loser == 0
                    && (p.stocks > 0 || (p.x28 == me.frame_count && w.stocks == 0)))
                || (canceled(me.outcome) && (p.stocks > 0 || p.score == w.score));
            if dashes {
                self.rp_line(t, -30.0, NO_TIME, &[], 0.11, 0.08, c);
            } else {
                let seconds = p.x28 / 60;
                let shown = seconds.min(5999);
                let args = [
                    FmtArg::Int((shown / 60) as i32),
                    FmtArg::Int((shown % 60) as i32),
                ];
                self.rp_line(t, -30.0, b"%d\x81\x46%02d\0", &args, 0.11, 0.08, c);
            }
        } else {
            c = [0xA0, 0xA0, 0xA0, c[3]];
            self.rp_line(t, -30.0, b"%s\0", &[FmtArg::Str(MINUS)], 0.11, 0.08, c);
        }
    }

    /// The players' rows shown, `fn_80177748`: each placed player's character icon and its
    /// place (the fifth picture for a winner or the winning team's best, the fourth after a
    /// cancelled match).
    #[doc(alias = "fn_80177748")]
    pub(super) fn results_rows(&mut self) {
        let me = self.results_end().clone();
        // `var_r24` carries over between players where the match is neither a free for all
        // nor a team battle, which no match is: it holds what the caller left in r24 before.
        let mut place: Option<i32> = None;
        for i in 0..4 {
            let p = &me.player_standings[i];
            if p.pkind == PKIND_NA {
                continue;
            }
            let js = self.game.results.data.players[i].jobjs;
            self.rp_show(js[0]);
            let f = super::vs::stock_icon_frame(i32::from(p.ckind), 0, 0);
            self.rp_freeze_at(js[0], mem(f));
            self.rp_show(js[4]);
            if canceled(me.outcome) {
                place = Some(4);
            } else if me.is_teams == 0 {
                place = Some(if p.is_small_loser == 0 {
                    5
                } else {
                    i32::from(p.is_big_loser)
                });
            } else if me.is_teams == 1 {
                place = Some(if usize::from(self.game.results.data.x6) == i {
                    5
                } else {
                    i32::from(me.team_standings[usize::from(p.team)].is_big_loser)
                });
            }
            let v = place.unwrap_or_else(|| {
                panic!(
                    "fn_80177748 reads r24 its caller left: is_teams {}",
                    me.is_teams
                )
            });
            self.rp_freeze_at(js[4], mem(fp::frsp(f64::from(v))));
        }
    }

    /// Waiting for a button, `fn_80177920`: any button of a human player's controller (or of
    /// any controller, with no human playing) starts the statistics, the panel's animation
    /// running on and the rows shown.
    #[doc(alias = "fn_80177920")]
    pub(super) fn results_wait(&mut self, gobj: GObjId) {
        let me = self.results_end().clone();
        let pads = &self.game.pads;
        let mut humans = 0;
        for i in 0..4 {
            if me.player_standings[i].pkind == PKIND_HUMAN && pads.master[i].err == 0 {
                humans += 1;
                if pads.copy[i].trigger != 0 {
                    self.game.results.data.state = 3;
                    break;
                }
            }
        }
        if humans == 0 {
            for i in 0..4 {
                if pads.master[i].err == 0 && pads.copy[i].trigger != 0 {
                    self.game.results.data.state = 3;
                    break;
                }
            }
        }
        if self.game.results.data.state == 3 {
            let j = match self.game.world.gobj(gobj).obj {
                Some(Obj::JObj(j)) => Some(j),
                other => panic!("the results' model holds {other:?}"),
            };
            crate::lb::anim::set_framerate(&mut self.game.objs, j, 1.0);
            self.results_rows();
        }
    }

    /// The list a page shows, `fn_801748EC` (a page other than the bonuses' one of
    /// `lbl_803D6878`'s).
    #[doc(alias = "fn_801748EC")]
    fn stats_list(&self, page: u8, slot: usize) -> StatsList {
        match page {
            0 => StatsList {
                mode: 0,
                count: 0x0D,
                entries: &KOS_PAGE,
            },
            1 => StatsList {
                mode: 1,
                count: 0x30,
                entries: &STATS_PAGE,
            },
            2 => {
                let s = self.game.results.stats_info[slot];
                StatsList {
                    mode: s.x0,
                    count: s.x1,
                    entries: &[],
                }
            }
            p => panic!("fn_801748EC: no statistics page {p}"),
        }
    }

    /// A statistics getter's value (`StatsEntry::check`).
    fn stats_check(&self, getter: u32, slot: usize) -> i32 {
        let me = self.results_end();
        match getter {
            0x8017_BACC => super::results_stats::clamp_to(me.player_standings[slot].x9C, 999_999),
            0x8017_BB30 => super::results_stats::clamp_to(me.player_standings[slot].xA0, 999_999),
            g => super::results_stats::stat_value(me, self.saved_us(), g, slot),
        }
    }

    /// The opponent's name a KO's row shows (`fn_8017BB94` to `fn_8017BDC8`): its name tag's
    /// or its port's label, none where the row's count is none.
    #[doc(alias = "fn_8017BB94")]
    #[doc(alias = "fn_8017BC50")]
    #[doc(alias = "fn_8017BD0C")]
    #[doc(alias = "fn_8017BDC8")]
    fn stats_label(&self, getter: u32, slot: usize) -> Option<Vec<u8>> {
        let target = match getter {
            0x8017_BB94 => 0,
            0x8017_BC50 => 1,
            0x8017_BD0C => 2,
            0x8017_BDC8 => 3,
            g => panic!("no results label getter {g:#010x}"),
        };
        let me = self.results_end();
        let v = if slot == target || me.player_standings[target].pkind == PKIND_NA {
            -1
        } else {
            super::results_stats::clamp_to(
                i32::from(me.player_standings[slot].kills[target]),
                999_999,
            )
        };
        if v == -1 {
            return None;
        }
        let tag = me.player_standings[target].x4;
        if tag == 0x78 {
            let l = if self.saved_us() {
                PLAYER_LABELS_US[target]
            } else {
                PLAYER_LABELS_JP[target]
            };
            Some(l.to_vec())
        } else {
            Some(self.results_name_tag(tag))
        }
    }

    /// A name tag's name as `"%s"` reads it from the tag: its eight bytes and the ones after
    /// them up to a zero (`GetPersistentNameData(tag)->namedata`).
    fn results_name_tag(&self, tag: u8) -> Vec<u8> {
        use super::main_lib::NAMETAG_BANK_SIZE;
        let s = usize::from(tag);
        let n = &self.game.game_data.thing.nametag_banks[s / NAMETAG_BANK_SIZE].inner
            [s % NAMETAG_BANK_SIZE];
        let mut b: Vec<u8> = n.namedata.iter().map(|&c| c as u8).collect();
        b.extend([n.x1A0 as u8, n.rumble_enabled, n.x1A2 as u8, n.padding_x1A2]);
        let end = b.iter().position(|&c| c == 0).unwrap_or_else(|| {
            super::super::code::unported_call("a name tag's name running past its block")
        });
        b.truncate(end + 1);
        b
    }

    /// Whether an entry shows, `fn_801743C4`: one with a label string, or one whose value
    /// isn't none and whose label isn't none.
    #[doc(alias = "fn_801743C4")]
    fn stats_entry_shows(&self, slot: usize, e: &StatsEntry) -> bool {
        if e.value >= 0 {
            return true;
        }
        if e.check != 0 && self.stats_check(e.check, slot) == -1 {
            return false;
        }
        if e.get != 0 {
            if self.stats_label(e.get, slot).is_none() {
                return false;
            }
        } else {
            return false;
        }
        true
    }

    /// The entries a page shows, `fn_80174A60` (the bonuses' page its count).
    #[doc(alias = "fn_80174A60")]
    fn stats_shown(&self, list: &StatsList, slot: usize) -> i32 {
        if list.mode != 2 {
            (0..usize::from(list.count))
                .filter(|&i| self.stats_entry_shows(slot, &list.entries[i]))
                .count() as i32
        } else {
            i32::from(list.count)
        }
    }

    /// A player's statistics' texts freed, `fn_80174920`.
    #[doc(alias = "fn_80174920")]
    fn stats_texts_free(&mut self, slot: usize) {
        for i in 0..10 {
            for k in 0..3 {
                if let Some(t) = self.game.results.data.players[slot].stats_text[k][i] {
                    self.destroy_text(t);
                    self.game.results.data.players[slot].stats_text[k][i] = None;
                }
            }
        }
    }

    /// A player's statistics page drawn anew, `fn_80174B4C`: its old texts freed, the page's
    /// arrows by the page and the scroll, then up to ten rows from the scroll's: each a
    /// label string, a label and a value (right aligned), clipped to the panel's rows
    /// (`fn_801749B8`).
    #[doc(alias = "fn_80174B4C")]
    fn stats_page(&mut self, slot: usize) {
        self.stats_texts_free(slot);
        let page = self.game.results.data.players[slot].page;
        let list = self.stats_list(page, slot);
        let num_pages = self.game.results.data.num_pages;
        let pd = self.game.results.data.players[slot];
        let mut pos = pd.stats_position;
        let offset = f64::from(pd.scroll_offset);
        let start = fp::fctiwz(fp::fmuls(10.0, offset)) / 10;
        let y = fp::fmadds(
            1.75,
            fp::fsubs(offset, fp::frsp(f64::from(start.wrapping_sub(1)))),
            f64::from(pos.y),
        );
        pos.y = mem(y);
        let visible = self.stats_shown(&list, 0);
        let p = &mut self.game.results.data.players[slot];
        p.x0_3 = page != 0;
        p.x0_4 = i32::from(page) < i32::from(num_pages) - 1;
        p.x0_2 = f64::from(p.scroll_offset) < fp::frsp(f64::from(visible.wrapping_sub(10)));
        p.x0_1 = p.scroll_offset > 0.0;
        let data = self.data;
        let mut count = 0;
        let mut idx = start;
        while count < 10 {
            if i32::from(list.count) <= idx + 1 {
                break;
            }
            if list.mode != 2 && !self.stats_entry_shows(slot, &list.entries[idx as usize]) {
                idx += 1;
                continue;
            }
            let t0 = self.create_text(0, 0, Vec3::new(pos.x, -pos.y, pos.z), 11.0, 10.0);
            let tx = self.game.sis.text_mut(t0);
            tx.default_fitting = 1;
            tx.x34 = [0.05, 0.054_687_5];
            tx.render_callback = Some(STATS_TEXT_RENDER);
            let t1 = self.create_text_buffer(0, 0);
            let tx = self.game.sis.text_mut(t1);
            tx.pos = Vec3::new(pos.x, -pos.y, pos.z);
            tx.render_callback = Some(STATS_TEXT_RENDER);
            let t2 = self.create_text_buffer(0, 0);
            let tx = self.game.sis.text_mut(t2);
            tx.pos = Vec3::new(mem(fp::fadds(11.0, f64::from(pos.x))), -pos.y, pos.z);
            tx.render_callback = Some(STATS_TEXT_RENDER);
            tx.default_alignment = 2;
            let st = &mut self.game.results.data.players[slot].stats_text;
            st[0][count] = Some(t0);
            st[1][count] = Some(t1);
            st[2][count] = Some(t2);
            pos.y = mem(fp::fsubs(f64::from(pos.y), 1.75));
            self.stats_row(slot, [t0, t1, t2], &list, idx);
            let _ = data;
            idx += 1;
            count += 1;
        }
    }

    /// An entry's texts, `fn_80174468`: its label string, or a bonus's name (every other
    /// row of the bonuses' page); its value (dashes for none, a bonus's points signed); its
    /// label (the opponent's name).
    #[doc(alias = "fn_80174468")]
    fn stats_row(&mut self, slot: usize, t: [TextId; 3], list: &StatsList, idx: i32) {
        let data = self.data;
        let mut label_id = -1;
        let mut value_id = -1;
        let entry = (list.mode != 2).then(|| list.entries[idx as usize]);
        let me_x44c = |r: &Self| r.results_end().x44C[slot].clone();
        let nth = |values: &[u8], n: i32| -> usize {
            let mut n = n;
            let mut i = 0;
            while i < 256 {
                if values[i] != 0 {
                    n -= 1;
                    if n == 0 {
                        break;
                    }
                }
                i += 1;
            }
            i
        };
        if let Some(e) = entry {
            if e.value >= 0 {
                self.game.sis.reset_text(data, t[0], i32::from(e.value));
            }
        } else if idx & 1 == 1 && idx < i32::from(list.count) {
            let x = me_x44c(self);
            let i = nth(&x.x0, idx / 2 + 1);
            let kind = super::decisions::decision_kind(i as i32);
            let us = super::super::gm::language_setting_is_us();
            let name = (super::decisions::decision_text(kind, us) - 2) as u16;
            self.game.sis.reset_text(data, t[0], i32::from(name) + 60);
        }
        let (zero, neg30) = (0.0, -30.0);
        let add = |r: &mut Self, t: TextId, fmt: &[u8], args: &[FmtArg]| -> i32 {
            let mut s = [0u8; 0x200];
            let len = crate::hsd::sis::printf(fmt, args, &mut s);
            r.game.sis.add_string(t, zero, neg30, Some(&s[..=len]))
        };
        match entry {
            None => {
                if idx & 1 == 0 {
                    let pair = idx / 2 - 1;
                    if pair >= 0 {
                        let x = me_x44c(self);
                        let i = nth(&x.x0, pair + 1);
                        let v = x.x104[i];
                        value_id = if v < 0 {
                            add(
                                self,
                                t[2],
                                b"%s%d\0",
                                &[FmtArg::Str(MINUS), FmtArg::Int(v.wrapping_neg())],
                            )
                        } else {
                            add(self, t[2], b"%d\0", &[FmtArg::Int(v)])
                        };
                    } else {
                        value_id = add(self, t[2], b"%s\0", &[FmtArg::Str(MINUS)]);
                    }
                }
            }
            Some(e) if e.check != 0 => {
                let v = self.stats_check(e.check, slot);
                value_id = if v < 0 {
                    add(self, t[2], b"%s\0", &[FmtArg::Str(DASHES)])
                } else {
                    add(self, t[2], b"%d\0", &[FmtArg::Int(v)])
                };
                if e.get != 0 {
                    let l = self.stats_label(e.get, slot);
                    label_id = self.stats_label_line(t[1], l);
                }
            }
            Some(e) => {
                if e.get != 0 {
                    let l = self.stats_label(e.get, slot);
                    label_id = self.stats_label_line(t[1], l);
                }
            }
        }
        if label_id >= 0 {
            self.game.sis.set_scale(
                data,
                t[1],
                label_id,
                f64::from(0.05_f32),
                f64::from(0.054_687_5_f32),
            );
        }
        if value_id >= 0 {
            self.game.sis.set_scale(
                data,
                t[2],
                value_id,
                f64::from(0.059_f32),
                f64::from(0.054_687_5_f32),
            );
        }
    }

    /// A label's line (`"%s"` of the getter's string; a null string prints nothing).
    fn stats_label_line(&mut self, t: TextId, label: Option<Vec<u8>>) -> i32 {
        let Some(l) = label else {
            super::super::code::unported_call("a results label printed from a null string");
        };
        let mut s = [0u8; 0x200];
        let len = crate::hsd::sis::printf(b"%s\0", &[FmtArg::Str(&l)], &mut s);
        self.game.sis.add_string(t, 0.0, -30.0, Some(&s[..=len]))
    }

    /// A human's page turned (B or the stick left back, A or the stick right on) or scrolled
    /// (the stick down or up past 0.3), `fn_80177B7C`; whether anything changed.
    #[doc(alias = "fn_80177B7C")]
    fn stats_input(&mut self, slot: usize) -> bool {
        let s = self.game.pads.copy[slot];
        let mut stick_y = f64::from(s.nml_stick_y);
        let abs = f32::from_bits(s.nml_stick_y.to_bits() & !0x8000_0000);
        if f64::from(abs) < f64::from(0.3_f32) {
            stick_y = 0.0;
        }
        let trigger = s.trigger;
        let num_pages = self.game.results.data.num_pages;
        if trigger & (PAD_B | STICK_LEFT) != 0 {
            let p = &mut self.game.results.data.players[slot];
            if p.page != 0 {
                p.page -= 1;
                p.scroll_offset = 0.0;
                self.lb_audio_call(2);
                return true;
            }
            false
        } else if trigger & (PAD_A | STICK_RIGHT) != 0 {
            let p = &mut self.game.results.data.players[slot];
            if i32::from(p.page) < i32::from(num_pages) - 1 {
                p.page += 1;
                p.scroll_offset = 0.0;
                self.lb_audio_call(2);
                return true;
            }
            false
        } else if stick_y < 0.0 {
            let amount = fp::fmuls(-stick_y, f64::from(0.2_f32));
            let page = self.game.results.data.players[slot].page;
            let list = self.stats_list(page, slot);
            let max = self.stats_shown(&list, slot).wrapping_sub(10);
            let maxf = fp::frsp(f64::from(max));
            let p = &mut self.game.results.data.players[slot];
            if f64::from(p.scroll_offset) < maxf {
                p.scroll_offset = mem(fp::fadds(f64::from(p.scroll_offset), amount));
                if f64::from(p.scroll_offset) > maxf {
                    p.scroll_offset = mem(maxf);
                }
                return true;
            }
            false
        } else if stick_y > 0.0 {
            let amount = fp::fmuls(stick_y, f64::from(0.2_f32));
            let p = &mut self.game.results.data.players[slot];
            if p.scroll_offset > 0.0 {
                p.scroll_offset = mem(fp::fsubs(f64::from(p.scroll_offset), amount));
                if p.scroll_offset < 0.0 {
                    p.scroll_offset = 0.0;
                }
                return true;
            }
            false
        } else {
            false
        }
    }

    /// A computer's pages, `fn_80177DD0`: a connected controller turns them as a human's;
    /// otherwise each scrolls on by 0.05 a frame, the KOs page for 181 frames and the
    /// statistics page for 1001 before the next; whether anything changed.
    #[doc(alias = "fn_80177DD0")]
    fn stats_cpu(&mut self, slot: usize) -> bool {
        if self.game.pads.copy[slot].err == 0 {
            return self.stats_input(slot);
        }
        let page = self.game.results.data.players[slot].page;
        let list = self.stats_list(page, slot);
        let max = self.stats_shown(&list, slot).wrapping_sub(10).max(0);
        let maxf = fp::frsp(f64::from(max));
        let num_pages = self.game.results.data.num_pages;
        let p = &mut self.game.results.data.players[slot];
        let scroll = |p: &mut super::results_scene::ResultsPlayerData| -> bool {
            p.scroll_offset = mem(fp::fadds(f64::from(p.scroll_offset), f64::from(0.05_f32)));
            if f64::from(p.scroll_offset) > maxf {
                p.scroll_offset = mem(maxf);
                false
            } else {
                true
            }
        };
        match p.page {
            0 => {
                let old = p.x2;
                p.x2 = p.x2.wrapping_add(1);
                if i32::from(old) > 180 {
                    p.x2 = 0;
                    p.scroll_offset = 0.0;
                    p.page = 1;
                    true
                } else {
                    scroll(p)
                }
            }
            1 => {
                let old = p.x2;
                p.x2 = p.x2.wrapping_add(1);
                if i32::from(old) > 1000 {
                    if i32::from(p.page) < i32::from(num_pages) - 1 {
                        p.x2 = 0;
                        p.scroll_offset = 0.0;
                        p.page = 2;
                        true
                    } else {
                        p.x2 = p.x2.wrapping_sub(1);
                        false
                    }
                } else {
                    scroll(p)
                }
            }
            2 if i32::from(p.x2) < 1000 => {
                let r = scroll(p);
                p.x2 = p.x2.wrapping_add(1);
                r
            }
            _ => false,
        }
    }

    /// The KO counts gone and the times moved up to the panel's top, `fn_80175D34`.
    #[doc(alias = "fn_80175D34")]
    fn results_counts_away(&mut self) {
        let data = self.data;
        let x24 = self.game.results.data.x24;
        let pos = crate::lb::joint::world_position(&mut self.game.objs, data, x24, None);
        for i in 0..4 {
            if let Some(t) = self.game.results.data.players[i].ko_count.take() {
                self.destroy_text(t);
            }
            if let Some(t) = self.game.results.data.players[i].ko_time {
                self.game.sis.text_mut(t).pos.y = -pos.y;
            }
        }
    }

    /// The statistics' frame, `fn_80178050`: at the panel's 50th frame its animation stops
    /// and the pages come up (humans' and computers'; an empty column is done), with the
    /// screen's end 20 frames after it's done, or 10 with a human; then each human turns and
    /// scrolls its pages and is done at Start (Start again takes it back), computers page
    /// on their own (with no human, Start on any controller ends the screen), the arrows
    /// follow, and the screen ends once every player is done.
    #[doc(alias = "fn_80178050")]
    pub(super) fn results_stats_frame(&mut self, gobj: GObjId) {
        let me = self.results_end().clone();
        let j = match self.game.world.gobj(gobj).obj {
            Some(Obj::JObj(j)) => Some(j),
            other => panic!("the results' model holds {other:?}"),
        };
        let mut end = false;
        let frame = crate::lb::anim::current_frame(&self.game.objs, j);
        if frame >= 50.0 && self.game.results.data.phase == 0 {
            crate::lb::anim::set_framerate(&mut self.game.objs, j, 0.0);
            self.game.results.data.phase = 1;
            let no_human = !(0..4).any(|i| me.player_standings[i].pkind == PKIND_HUMAN);
            self.game.results.no_human = i32::from(no_human);
            self.game.results.data.x3 = if no_human { 0x14 } else { 0x0A };
        }
        self.results_counts_away();
        let phase = self.game.results.data.phase;
        if phase == 0 {
            return;
        }
        if phase == 1 {
            self.game.results.data.phase = 2;
            for k in 0..4 {
                match me.player_standings[k].pkind {
                    PKIND_NA => self.game.results.data.players[k].done = true,
                    PKIND_HUMAN => self.stats_page(k),
                    PKIND_CPU => {
                        self.stats_page(k);
                        self.game.results.data.players[k].done = true;
                    }
                    _ => {}
                }
            }
        }
        let no_human = self.game.results.no_human != 0;
        for k in 0..4 {
            let kind = me.player_standings[k].pkind;
            let s = self.game.pads.copy[k];
            if kind == PKIND_HUMAN {
                if !self.game.results.data.players[k].done {
                    if self.stats_input(k) {
                        self.stats_page(k);
                    }
                    let s = self.game.pads.copy[k];
                    if s.err != 0 || s.trigger & PAD_START != 0 {
                        self.game.results.data.players[k].done = true;
                        self.stats_texts_free(k);
                        self.results_column_turn(k, 1.0);
                        let js = self.game.results.data.players[k].jobjs;
                        self.rp_hide(js[0]);
                        self.rp_hide(js[4]);
                        let p = &mut self.game.results.data.players[k];
                        p.x0_1 = false;
                        p.x0_2 = false;
                        p.x0_3 = false;
                        p.x0_4 = false;
                        self.lb_audio_call(1);
                    }
                } else if s.trigger & PAD_START != 0 {
                    self.stats_texts_free(k);
                    self.game.results.data.players[k].done = false;
                    self.stats_page(k);
                    self.results_column_turn(k, 0.0);
                    let js = self.game.results.data.players[k].jobjs;
                    self.rp_show(js[0]);
                    self.rp_show(js[4]);
                    self.lb_audio_call(0);
                }
            } else if kind == PKIND_CPU {
                if self.stats_cpu(k) {
                    self.stats_page(k);
                }
                if no_human {
                    self.game.results.data.players[k].done = false;
                    let s = self.game.pads.copy[k];
                    if s.err == 0 && s.trigger & PAD_START != 0 {
                        self.lb_audio_call(1);
                        end = true;
                    }
                }
            } else if kind == PKIND_NA && no_human && s.err == 0 && s.trigger & PAD_START != 0 {
                self.lb_audio_call(1);
                end = true;
            }
        }
        for k in 0..4 {
            if me.player_standings[k].pkind == PKIND_NA {
                continue;
            }
            let p = self.game.results.data.players[k];
            for (shown, j) in [
                (p.x0_1, p.jobjs[8]),
                (p.x0_2, p.jobjs[9]),
                (p.x0_3, p.jobjs[10]),
                (p.x0_4, p.jobjs[11]),
            ] {
                if shown {
                    self.rp_show(j);
                } else {
                    self.rp_hide(j);
                }
            }
        }
        if self.game.results.data.players[..4].iter().all(|p| p.done) {
            end = true;
        }
        if end {
            self.game.results.data.state = 4;
        }
    }

    /// A column's card turned (done at `rate` 1, back at 0): its display objects' and its
    /// frame's animations from frame 50 (`fn_80178050`'s two copies).
    fn results_column_turn(&mut self, k: usize, rate: f32) {
        let js = self.game.results.data.players[k].jobjs;
        let d6 = js[6].and_then(|j| self.game.objs.jobjs[j].dobj());
        let o = &mut self.game.objs;
        crate::lb::anim::dobj_set_rate_all(o, d6, rate);
        crate::lb::anim::dobj_req_anim_all(o, d6, 50.0);
        let a1 = js[1].and_then(|j| o.jobjs[j].aobj);
        crate::hsd::aobj::set_rate(&mut o.aobjs, a1, rate);
        crate::hsd::aobj::req_anim(&mut o.aobjs, &mut o.fobjs, a1, 50.0);
        self.rp_anim_all(js[6]);
        self.rp_anim_all(js[1]);
    }

    /// The statistics' texts' render callback, `fn_801749B8`: the scissor from the first
    /// player's statistics' spot down to its last row's joint, on screen.
    #[doc(alias = "fn_801749B8")]
    pub fn results_stats_text_render(&mut self) {
        let Some(c) = self.game.results.data.cobj else {
            return;
        };
        let data = self.data;
        let at = self.game.results.data.players[0].stats_position;
        let a = crate::lb::screen::world_to_screen(&mut self.game.objs, data, c, at, false)
            .expect("the panel's camera's projection");
        let j13 = self.game.results.data.players[0].jobjs[13];
        let p = crate::lb::joint::world_position(&mut self.game.objs, data, j13, None);
        let b = crate::lb::screen::world_to_screen(&mut self.game.objs, data, c, p, false)
            .expect("the panel's camera's projection");
        let top = (fp::fctiwz(f64::from(a.y)) as u16) & 0xFFFC;
        let bottom = (fp::fctiwz(f64::from(b.y)) as u16) & 0xFFDC;
        self.game
            .objs
            .display
            .draws
            .push(crate::hsd::display::Draw::Scissor {
                x: 0,
                y: u32::from(top),
                w: 0x27F,
                h: u32::from(bottom).wrapping_sub(u32::from(top)),
            });
    }
}
