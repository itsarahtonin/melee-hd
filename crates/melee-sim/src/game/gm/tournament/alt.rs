//! Winner-out and loser-out's match-up screen (`gmtou_2.c`, scene `GS_TOU_ALT`): the next
//! match's players each with their character model, icon, name tag, ready marker and
//! handicap; each picks a character (left and right, L and R together random), a color (X and
//! Y), and readies with A or Start (B backs out of ready); once all are ready the match
//! starts; B held a second and a half on any controller goes back to the setup. The other
//! entrants wait in a row across the top, sliding into their new places after a match
//! (`gm_8019E634` reorders them from the match's result).

use gekko_fp as fp;

use super::STATE_SETUP;
use super::bracket::player_x;
use super::lib::{ckind_of, f, icon_frame, icon_of, scene_model};
use crate::fpu::{mem, reg};
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::JObjId;

/// The screen's processes.
pub const PROC_BG: u32 = 0x8019_BF18;
pub const PROC_PANEL: u32 = 0x8019_BF8C;
pub const PROC_PLAYER: u32 = 0x8019_C048;
pub const PROC_ICON: u32 = 0x8019_C3EC;
pub const PROC_BASE: u32 = 0x8019_C570;
pub const PROC_TITLE: u32 = 0x8019_C6AC;
pub const PROC_TAG: u32 = 0x8019_C744;
pub const PROC_READY: u32 = 0x8019_CA38;
pub const PROC_BAR: u32 = 0x8019_CBFC;
pub const PROC_UNPLUGGED: u32 = 0x8019_CC74;
pub const PROC_ROW: u32 = 0x8019_CDBC;
pub const PROC_COUNTS: u32 = 0x8019_CFA4;
pub const PROC_HANDICAP: u32 = 0x8019_D074;

/// `lbl_803B7D18`: the players' models' frames by state (start, end, loop).
const PLAYER_ANIMS: [(u8, u8, u8); 6] = [
    (0, 30, 0),
    (50, 59, 0),
    (60, 70, 1),
    (90, 100, 0),
    (110, 130, 0),
    (150, 160, 0),
];

const PAD_A: u32 = 0x100;
const PAD_B: u32 = 0x200;
const PAD_X: u32 = 0x400;
const PAD_Y: u32 = 0x800;
const PAD_START: u32 = 0x1000;
const PAD_R: u32 = 0x20;
const PAD_L: u32 = 0x40;
const PAD_LEFT: u32 = 0x0004_0001;
const PAD_RIGHT: u32 = 0x0008_0002;
const GM_MENU: u8 = 1;

impl Running<'_> {
    fn alt(&mut self) -> &mut super::Alt {
        &mut self.game.tou.alt
    }

    fn alt_m1(&self, i: u32) -> melee_data::archive::DataRef {
        scene_model(self, self.game.tou.alt.scene1, i)
    }
    fn alt_m4(&self, i: u32) -> melee_data::archive::DataRef {
        scene_model(self, self.game.tou.alt.scene4, i)
    }

    /// `fn_8019BA08`: a player's icons' model showing their character's (none for a random
    /// one).
    #[doc(alias = "fn_8019BA08")]
    fn alt_icon_model(&mut self, p: usize, j: JObjId) {
        let m = self.game.tou.tm.x4b8[p];
        let first = self.tou_child(Some(j));
        if m.x2 != 0 {
            let mut c = first;
            self.tou_set_tz(c, 10000.0);
            for _ in 1..=12 {
                c = self.tou_next(c);
                self.tou_set_tz(c, 10000.0);
            }
            return;
        }
        self.tou_icon_model(first, super::bracket::ICON_MODELS[usize::from(m.x1)]);
    }

    /// `fn_8019BF18`: the background's loop.
    #[doc(alias = "fn_8019BF18")]
    fn alt_bg_proc(&mut self, gobj: GObjId) {
        let j = self.tou_jobj(gobj);
        if self.alt().x4 > 800 {
            self.alt().x4 = 0;
        }
        let c = self.alt().x4;
        self.tou_frame(Some(j), mem(f(i32::from(c))));
        self.alt().x4 = c.wrapping_add(1);
    }

    /// `fn_8019BF8C`: the panel's loops.
    #[doc(alias = "fn_8019BF8C")]
    fn alt_panel_proc(&mut self, gobj: GObjId) {
        let cur = self.game.tou.tm.cur_option;
        let j = self.tou_jobj(gobj);
        if cur <= 0x1A || cur >= 0x1F {
            if self.alt().xe > 0x77 {
                self.alt().xe = 0x5A;
            }
        } else if cur <= 0x1E && self.alt().xe > 0x9F {
            self.alt().xe = 0x82;
        }
        let c = self.alt().xe;
        self.tou_frame(Some(j), mem(f(i32::from(c))));
        self.alt().xe = c.wrapping_add(1);
    }

    /// `fn_8019C048`: a player's character model, as the bracket's (`fn_80196FFC`) but for
    /// unplugged controllers whatever the slot.
    #[doc(alias = "fn_8019C048")]
    fn alt_player_proc(&mut self, gobj: GObjId) {
        let p = self.tou_index(gobj) as usize;
        let j = self.tou_jobj(gobj);
        let t = self.game.tou.tm.x524(2).expect("the players' names");
        if self.tou_pad_err(p) != 0 {
            self.tou_hide(Some(j));
            self.game.sis.set_scale(self.data, t, p as i32, 0.0, 0.0);
            return;
        }
        self.tou_show(Some(j));
        let s = if self.alt().x1d[p].state == 4 {
            0.0
        } else {
            1.0
        };
        self.game.sis.set_scale(self.data, t, p as i32, s, s);
        let x = player_x(self.game.tou.tm.x30, p as i32);
        self.tou_set_tx(Some(j), mem(x));
        self.game.sis.text_mut(t).hidden = 0;
        let mut a = self.alt().x1d[p];
        let (start, end, lp) = PLAYER_ANIMS[usize::from(a.state)];
        a.start = start;
        a.end = end;
        a.loop_ = lp;
        if a.cur < a.start {
            a.cur = a.start;
        }
        if a.cur < a.end {
            a.cur += 1;
        } else {
            a.done = 1;
            a.cur = if a.loop_ != 0 { a.start } else { a.end };
        }
        if a.state == 0 && self.alt().x38[p] == 6 && a.done != 0 && self.game.tou.tm.x4b8[p].x0 != 0
        {
            a.state = 1;
            a.done = 0;
        }
        if a.state == 1 && a.done != 0 {
            a.state = 2;
            a.done = 0;
        }
        self.alt().x1d[p] = a;
        self.tou_frame(Some(j), mem(f(i32::from(a.cur))));
    }

    /// `fn_8019C3EC`: a player's character icon.
    #[doc(alias = "fn_8019C3EC")]
    fn alt_icon_proc(&mut self, gobj: GObjId) {
        let p = self.tou_index(gobj) as usize;
        let j = self.tou_jobj(gobj);
        if self.tou_pad_err(p) != 0 {
            self.tou_hide(Some(j));
            return;
        }
        self.tou_show(Some(j));
        let x = player_x(self.game.tou.tm.x30, p as i32);
        self.tou_move(j, fp::fadds(4.5, x), 3.0, 666.0);
        if self.alt().x1d[p].state == 4 {
            self.tou_hide(Some(j));
        }
        self.alt_icon_model(p, j);
    }

    /// `fn_8019C570`: a player's base with their character's icon and color.
    #[doc(alias = "fn_8019C570")]
    fn alt_base_proc(&mut self, gobj: GObjId) {
        let p = self.tou_index(gobj) as usize;
        let j = self.tou_jobj(gobj);
        if self.tou_pad_err(p) != 0 {
            self.tou_hide(Some(j));
            return;
        }
        self.tou_show(Some(j));
        if self.alt().x1d[p].state == 4 {
            self.tou_hide(Some(j));
        }
        let m = self.game.tou.tm.x4b8[p];
        let c = self.tou_child(Some(j));
        let c = self.tou_next(c);
        if m.x2 != 0 {
            self.tou_frame(c, 201.0);
            return;
        }
        self.tou_frame(c, icon_frame(i32::from(m.x1), i32::from(m.x3)));
    }

    /// `fn_8019C6AC`: the title coming in (winner-out's or loser-out's).
    #[doc(alias = "fn_8019C6AC")]
    fn alt_title_proc(&mut self, gobj: GObjId) {
        let j = self.tou_jobj(gobj);
        if self.alt().xf > 60 {
            self.alt().xf = 60;
        }
        let c = i32::from(self.alt().xf);
        let mt = self.game.tou.tm.match_type();
        self.tou_frame(Some(j), mem(f(c + mt * 100 + 100)));
        self.alt().xf = self.alt().xf.wrapping_add(1);
    }

    /// `fn_8019C744`: a player's name tag, bouncing once out.
    #[doc(alias = "fn_8019C744")]
    fn alt_tag_proc(&mut self, gobj: GObjId) {
        let p = self.tou_index(gobj) as usize;
        let j = self.tou_jobj(gobj);
        let x = player_x(self.game.tou.tm.x30, p as i32);
        self.tou_move(j, fp::fadds(reg(0.3), x), reg(10.1), 666.0);
        if self.alt().x1d[p].state == 4 {
            if self.alt().x10[p] < 0x28 {
                self.alt().x10[p] += 1;
            }
            let y = self.game.tou.alt_bounce[usize::from(self.alt().x10[p])];
            self.tou_set_ty(Some(j), y);
        } else {
            self.alt().x10[p] = 0;
        }
        if self.tou_pad_err(p) != 0 && self.game.tou.tm.x4b8[p].x0 != 1 {
            self.tou_show(Some(j));
            let x = player_x(self.game.tou.tm.x30, p as i32);
            self.tou_move(j, fp::fadds(reg(0.3), x), reg(10.1), 666.0);
        }
    }

    /// `fn_8019CA38`: a player's "ready" marker, looping until they're ready.
    #[doc(alias = "fn_8019CA38")]
    fn alt_ready_proc(&mut self, gobj: GObjId) {
        let p = self.tou_index(gobj) as usize;
        let j = self.tou_jobj(gobj);
        if self.tou_pad_err(p) != 0 {
            self.tou_hide(Some(j));
            return;
        }
        self.tou_show(Some(j));
        let x = player_x(self.game.tou.tm.x30, p as i32);
        self.tou_move(j, x, 666.0, 666.0);
        if self.alt().x1d[p].state == 2 {
            self.tou_hide(Some(j));
        }
        if self.alt().x6[p] < 600 {
            self.alt().x6[p] += 1;
        } else {
            self.alt().x6[p] = 0;
        }
        let c = self.alt().x6[p];
        self.tou_frame(Some(j), mem(f(i32::from(c))));
    }

    /// `fn_8019CBFC`: the players' bar coming in.
    #[doc(alias = "fn_8019CBFC")]
    fn alt_bar_proc(&mut self, gobj: GObjId) {
        let j = self.tou_jobj(gobj);
        let c = i32::from(self.alt().x1c);
        self.tou_frame(Some(j), mem(f(c + 0x14)));
        if self.alt().x1c < 0xA {
            self.alt().x1c += 1;
        }
    }

    /// `fn_8019CC74`: "no controller" over an unplugged player.
    #[doc(alias = "fn_8019CC74")]
    fn alt_unplugged_proc(&mut self, gobj: GObjId) {
        let p = self.tou_index(gobj) as usize;
        let j = self.tou_jobj(gobj);
        if self.tou_pad_err(p) == 0 {
            self.tou_hide(Some(j));
            return;
        }
        self.tou_show(Some(j));
        let x = player_x(self.game.tou.tm.x30, p as i32);
        self.tou_move(j, x, 666.0, reg(0.01));
    }

    /// `fn_8019CDBC`: a waiting entrant's icon sliding to its seat across the top (in front
    /// when moving right, behind when moving left), with its name, and its seat kept once
    /// there.
    #[doc(alias = "fn_8019CDBC")]
    fn alt_row_proc(&mut self, gobj: GObjId) {
        let j = self.tou_jobj(gobj);
        let idx = self.tou_index(gobj) as usize;
        let mut x = super::super::super::mn::common::tx(self, Some(j));
        let seat = self.game.tou.tm.x37[idx].xe;
        let target = mem(fp::fmsubs(reg(5.999_997), f(i32::from(seat)), 21.5));
        super::lib::step_towards(&mut x, target, 0.4);
        let t = self.game.tou.tm.x534(idx).expect("an entrant's name");
        let (y, ty, z) = match super::lib::step_towards(&mut x, target, 0.2) {
            1 => (19.5, -197.0, -6.0),
            -1 => (15.0, -152.0, 6.0),
            _ => {
                let e = &mut self.game.tou.tm.x37[idx];
                e.xf = e.xe;
                (17.0, -172.0, 0.01)
            }
        };
        self.tou_move(j, reg(x), y, reg(z));
        let tx = fp::fmuls(10.0, reg(x));
        self.game.sis.set_position(self.data, t, 0, tx, ty);
        self.game.sis.text_mut(t).pos.z = z;
        let e = self.game.tou.tm.x37[idx];
        if e.x5 != 0 {
            self.tou_frame(Some(j), 201.0);
            return;
        }
        self.tou_frame(Some(j), icon_frame(i32::from(e.x3), i32::from(e.x7)));
    }

    /// `fn_8019CFA4`: the entrants' and the players' counts.
    #[doc(alias = "fn_8019CFA4")]
    fn alt_counts_proc(&mut self, gobj: GObjId) {
        let j = self.tou_jobj(gobj);
        let c = self.tou_child(Some(j));
        let c = self.tou_next(c);
        let c = self.tou_next(c);
        let (x2e, x30) = (self.game.tou.tm.x2e, self.game.tou.tm.x30);
        self.tou_frame(c, mem(f(i32::from(x2e))));
        let c = self.tou_next(c);
        self.tou_frame(c, mem(f(i32::from(x30))));
    }

    /// `fn_8019D074`: a player's handicap.
    #[doc(alias = "fn_8019D074")]
    fn alt_handicap_proc(&mut self, gobj: GObjId) {
        let p = self.tou_index(gobj) as usize;
        let j = self.tou_jobj(gobj);
        if self.tou_pad_err(p) != 0 {
            self.tou_hide(Some(j));
            return;
        }
        self.tou_show(Some(j));
        let x = player_x(self.game.tou.tm.x30, p as i32);
        self.tou_move(j, x, 666.0, 666.0);
    }

    /// `fn_8019D1BC`: the screen's models: for each player their character model, name,
    /// icon, base, name tag, ready marker, "no controller" and handicap; the counts, the
    /// title, the name tags' bounce read from its model, the bar, and each entrant's icon
    /// and name across the top.
    #[doc(alias = "fn_8019D1BC")]
    fn alt_models(&mut self) {
        let n = usize::from(self.game.tou.tm.x30);
        for i in 0..n {
            let ii = i as i32;
            let m = self.alt_m4(12);
            let g = self.tou_model(false, m, 0, 0x1A, 2, true, Some(PROC_PLAYER), mem(f(ii)));
            let j = self.tou_jobj(g);
            self.tou_set_ty(Some(j), -2.5);
            self.tou_set_index(g, ii);
            let mut c = self.tou_child(Some(j));
            for _ in 0..3 {
                let mo = self.tou_mobj(c);
                crate::hsd::mobj::remove_anim_by_flags(&mut self.game.objs, mo, 4);
                c = self.tou_next(c);
            }
            let x30 = self.game.tou.tm.x30;
            let pos = match x30 {
                4 => fp::fadds(73.0, fp::fmadds(240.0, f(ii), 152.0)),
                3 => fp::fadds(73.0, fp::fmadds(240.0, f(ii), 270.0)),
                _ => fp::fadds(73.0, fp::fmadds(480.0, f(ii), 270.0)),
            };
            let pm = self.game.tou.tm.x4b8[i];
            let x = fp::fmsubs(reg(0.99), fp::fsubs(pos, 320.0), 261.0);
            self.tou_add_name(i32::from(pm.x6), i32::from(pm.x0), 5, x, -11.0, 5);
            let t = self.game.tou.tm.x524(2).expect("the players' names");
            self.game.sis.text_mut(t).hidden = 1;
            let posf = mem(pos);
            let m = self.alt_m4(10);
            let g = self.tou_model(false, m, 0, 0x1A, 2, true, Some(PROC_ICON), posf);
            let j = self.tou_jobj(g);
            self.tou_set_ty(Some(j), -2.5);
            self.tou_set_index(g, ii);
            self.alt_icon_model(i, j);
            let m = self.alt_m4(4);
            let g = self.tou_model(false, m, 0, 0x1A, 2, true, Some(PROC_BASE), posf);
            let j = self.tou_jobj(g);
            self.tou_set_index(g, ii);
            let px = player_x(x30, ii);
            self.tou_move(j, fp::fsubs(px, reg(2.799_999)), reg(4.3), reg(0.001));
            let c = self.tou_child(Some(j));
            let c = self.tou_next(c);
            if pm.x2 != 0 {
                self.tou_frame(c, 201.0);
            } else {
                self.tou_frame(c, icon_frame(i32::from(pm.x1), i32::from(pm.x3)));
            }
            let m = self.alt_m4(9);
            let g = self.tou_model(false, m, 0, 0x1A, 2, true, Some(PROC_TAG), posf);
            let j = self.tou_jobj(g);
            self.tou_set_ty(Some(j), -2.5);
            self.tou_set_index(g, ii);
            let anim = if pm.x0 == 0 { mem(f(ii)) } else { 4.0 };
            self.tou_frame(Some(j), anim);
            let m = self.alt_m4(8);
            let g = self.tou_model(false, m, 0, 0x1A, 2, true, Some(PROC_READY), 0.0);
            let j = self.tou_jobj(g);
            self.tou_set_ty(Some(j), -2.5);
            self.tou_set_index(g, ii);
            let m = self.alt_m4(14);
            let g = self.tou_model(false, m, 0, 0x1A, 2, true, Some(PROC_UNPLUGGED), anim);
            let j = self.tou_jobj(g);
            self.tou_set_ty(Some(j), -2.5);
            self.tou_set_index(g, ii);
            if self.game.tou.tm.x31 != 0 {
                let m = self.alt_m4(11);
                let g = self.tou_model(false, m, 0, 0x1A, 2, true, Some(PROC_HANDICAP), anim);
                let j = self.tou_jobj(g);
                self.tou_set_ty(Some(j), -2.0);
                self.tou_set_index(g, ii);
                self.tou_frame(Some(j), mem(f(i32::from(pm.x5))));
            }
        }
        let m = self.alt_m4(1);
        self.tou_model(false, m, 0, 0x1A, 2, true, Some(PROC_COUNTS), 0.0);
        let m = self.alt_m4(5);
        self.tou_model(false, m, 0, 0x1A, 2, true, Some(PROC_TITLE), 0.0);
        let m = self.alt_m4(0);
        let g = self.tou_model(false, m, 0, 0x1A, 2, true, None, 0.0);
        let mut j16 = Some(self.tou_jobj(g));
        j16 = self.tou_child(j16);
        for _ in 0..5 {
            j16 = self.tou_next(j16);
        }
        for k in 0..=0x28 {
            self.tou_frame(j16, mem(f(k + 10)));
            let y = super::super::super::mn::common::ty(self, j16);
            self.game.tou.alt_bounce[k as usize] = y;
        }
        let m = self.alt_m4(6);
        self.tou_model(false, m, 0, 0x1A, 2, true, Some(PROC_BAR), 0.0);
        let canvas = self.game.tou.canvas;
        for i in 0..usize::from(self.game.tou.tm.x2e) {
            let ii = i as i32;
            let m = self.alt_m4(3);
            let g = self.tou_model(false, m, 0, 0x1A, 2, true, Some(PROC_ROW), 0.0);
            let j = self.tou_jobj(g);
            let e = self.game.tou.tm.x37[i];
            let x = fp::fmsubs(reg(5.999_997), f(i32::from(e.xf)), 21.5);
            self.tou_move(j, x, 17.0, reg(0.01));
            if e.x5 != 0 {
                self.tou_frame(Some(j), 201.0);
            } else {
                self.tou_frame(Some(j), icon_frame(i32::from(e.x3), i32::from(e.x7)));
            }
            self.tou_set_index(g, ii);
            let name = self.tou_name(i32::from(e.x9));
            let t = self.create_text_buffer(0, canvas);
            let tx = self.game.sis.text_mut(t);
            tx.default_kerning = 1;
            tx.x4c = 1;
            tx.font_size = [0.1, 0.1];
            tx.default_alignment = 1;
            self.game.tou.tm.x518[7 + i] = Some(t);
            let x = fp::fmuls(10.0, fp::fmsubs(reg(5.999_997), f(ii), 21.5));
            self.game.sis.add_string(t, x, -172.0, Some(&name));
            self.game
                .sis
                .set_scale(self.data, t, 0, reg(0.35), reg(0.6));
        }
    }

    /// `fn_8019DD60`: the screen: the "match-up" sound unless just set up, the cameras, the
    /// texts' canvas, the lights, the background, the panel, the players' names' text, the
    /// models, the fog, and each player's model back to coming in.
    #[doc(alias = "fn_8019DD60")]
    fn alt_make(&mut self) {
        if self.game.tou.tm.x2c == 0 {
            let id = if self.game.tou.tm.match_type() == 1 {
                0x7545
            } else {
                0x7546
            };
            self.sfx_play(id, 0x7F, 0x40);
        }
        self.game.tou.tm.x2c = 0;
        let scene = self.game.tou.alt.scene1;
        let cam = self.tou_camera(scene);
        self.tou_camera2(scene);
        self.tou_canvas(Some(cam));
        self.tou_lights(scene);
        let m = self.alt_m1(5);
        self.tou_model(false, m, 0, 0x1A, 2, true, Some(PROC_BG), 0.0);
        let m = self.alt_m1(4);
        self.tou_model(false, m, 0, 0x1A, 2, true, Some(PROC_PANEL), 0.0);
        let canvas = self.game.tou.canvas;
        let t = self.create_text_buffer(0, canvas);
        let x = self.game.sis.text_mut(t);
        x.default_kerning = 1;
        x.font_size = [0.054945, 0.08];
        x.default_alignment = 1;
        self.game.tou.tm.x518[5] = Some(t);
        self.alt_models();
        self.tou_fog(scene);
        let n = usize::from(self.game.tou.tm.x30);
        let a = self.alt();
        a.x0 = 0;
        a.x4 = 0;
        a.xe = 0x50;
        a.xf = 0;
        for i in 0..n {
            a.x18[i] = 0;
            a.x6[i] = 0;
            a.x10[i] = 0;
            a.x1d[i] = super::PlayerAnim {
                start: 0,
                cur: 0,
                end: 0x1E,
                state: 0,
                done: 0,
                loop_: 0,
            };
            a.x38[i] = 6;
        }
    }

    /// The entrant in player `p`'s seat (`get_match_player_index`: the first whose
    /// match-up seat is `p`, else the first).
    fn alt_entrant_at(&self, p: i32) -> usize {
        let tm = &self.game.tou.tm;
        (0..usize::from(tm.x2e))
            .find(|&i| i32::from(tm.x37[i].xe) == p)
            .unwrap_or(0)
    }

    /// `gm_Scene_TouAlt_OnFrame`: L, R and Start back to the main menu; once every player is
    /// ready (their seats kept) the match; else each plugged-in player's B held a second
    /// and a half back to the setup, L and R a random character, left and right the
    /// character (only unlocked ones), A or Start ready (a rumble), B unready, X and Y the
    /// color; then the cache's players and stage.
    #[doc(alias = "gm_Scene_TouAlt_OnFrame")]
    pub fn tou_alt_on_frame(&mut self) {
        let _ = self.tou_repeated(4);
        let _ = self.tou_triggered(4);
        if self.game.buttons_triggered(4) & crate::game::mn::main::PAD_LR_START != 0 {
            self.lb_audio_call(0);
            self.game.game_data.x1850.force_main_menu = 1;
            self.game.end_scene();
            self.change_game_mode_after_scene(GM_MENU);
            return;
        }
        let n = usize::from(self.game.tou.tm.x30);
        let mut confirmed = 0;
        for i in 0..n {
            let a = self.alt().x1d[i];
            if a.state == 2 && a.cur >= 0x3C && self.tou_pad_err(i) == 0 {
                confirmed += 1;
            }
        }
        if confirmed == n {
            self.alt().x0 += 1;
            if self.alt().x0 >= 0x1E {
                for e in &mut self.game.tou.tm.x37[..usize::from(self.game.tou.tm.x2e)] {
                    e.xf = e.xe;
                }
                self.tou_go_to_match();
            }
            return;
        }
        self.alt().x0 = 0;
        for i in 0..n {
            if self.tou_pad_err(i) != 0 {
                continue;
            }
            let ii = i as i32;
            let buttons = self.tou_triggered(ii);
            let pressed = self.tou_repeated(ii);
            if self.tou_held(ii) & PAD_B != 0 {
                self.alt().x18[i] = self.alt().x18[i].wrapping_add(1);
                if self.alt().x18[i] > 0x5A {
                    self.lb_audio_call(1);
                    self.set_next_game_mode_state_id(STATE_SETUP);
                    self.game.end_scene();
                    return;
                }
            } else {
                self.alt().x18[i] = 0;
            }
            let status = self.alt().x1d[i].state;
            if status != 2 && status != 1 {
                let held = self.tou_held(ii);
                if held & PAD_L != 0 && held & PAD_R != 0 {
                    let j = self.alt_entrant_at(ii);
                    self.game.tou.tm.x37[j].x5 = 1;
                    self.game.tou.tm.x4b8[i].x2 = 1;
                }
                let step: Option<i32> = if pressed & PAD_LEFT != 0 {
                    Some(-1)
                } else if pressed & PAD_RIGHT != 0 {
                    Some(1)
                } else {
                    None
                };
                if let Some(d) = step {
                    self.lb_audio_call(2);
                    let j = self.alt_entrant_at(ii);
                    self.game.tou.tm.x37[j].x5 = 0;
                    self.game.tou.tm.x4b8[i].x2 = 0;
                    let mut chr = self.game.tou.tm.x4b8[i].x1;
                    loop {
                        if d < 0 {
                            if chr != 0 {
                                chr -= 1;
                            } else {
                                chr = icon_of(0x19) as u8;
                            }
                        } else if i32::from(chr) < icon_of(0x19) {
                            chr += 1;
                        } else {
                            chr = icon_of(0) as u8;
                        }
                        let k = ckind_of(i32::from(chr)) as u8;
                        if self.game.gm.unlocks.ckind_unlocked(k) {
                            break;
                        }
                    }
                    self.game.tou.tm.x4b8[i].x1 = chr;
                    let j = self.alt_entrant_at(ii);
                    self.game.tou.tm.x37[j].x3 = chr;
                    let j = self.alt_entrant_at(ii);
                    self.game.tou.tm.x4b8[i].x3 = 0;
                    self.game.tou.tm.x37[j].x7 = 0;
                }
            }
            if buttons & (PAD_A | PAD_START) != 0 {
                if self.alt().x1d[i].state != 2 {
                    self.lb_audio_call(1);
                    self.alt().x1d[i].state = 1;
                    let tag = self.game.tou.tm.x4b8[i].x6;
                    self.tou_rumble(i, tag.min(0x78) as u8, 0xB, 0x14);
                }
            } else if buttons & PAD_B != 0 {
                if self.alt().x1d[i].state == 2 {
                    self.lb_audio_call(0);
                    self.alt().x1d[i].state = 3;
                }
            } else {
                let s = self.alt().x1d[i].state;
                if s != 2 && s != 1 {
                    if buttons & PAD_X != 0 {
                        let m = self.game.tou.tm.x4b8[i];
                        let k = ckind_of(i32::from(m.x1)) as u8;
                        let count = super::super::super::playback::costume_count(k);
                        if i32::from(m.x3) < i32::from(count) - 1 {
                            self.game.tou.tm.x4b8[i].x3 += 1;
                        }
                        let j = self.alt_entrant_at(ii);
                        self.game.tou.tm.x37[j].x7 = self.game.tou.tm.x4b8[i].x3;
                    } else if buttons & PAD_Y != 0 {
                        let c = self.game.tou.tm.x4b8[i].x3;
                        if c != 0 {
                            self.game.tou.tm.x4b8[i].x3 = c - 1;
                        }
                        let j = self.alt_entrant_at(ii);
                        self.game.tou.tm.x37[j].x7 = self.game.tou.tm.x4b8[i].x3;
                    }
                }
            }
        }
        let stage = if self.tou_stage_random() {
            self.game.tou.stage
        } else {
            self.game.tou.tm.x28 as i32
        };
        let players: Vec<(u8, i32, u8)> = (0..4)
            .map(|i| {
                let m = self.game.tou.tm.x4b8[i];
                (m.x0, ckind_of(i32::from(m.x1)), m.x3)
            })
            .collect();
        self.tou_preload_match(stage, &players, true);
    }

    /// `gm_8019E634`, the match-up after a match: the players ranked by the match's result
    /// (`fn_80166CBC`), their handicaps adjusted with the rule on (`fn_80169000`), the
    /// winners (winner-out) or losers (loser-out) stay and the others go to the back of the
    /// line, the next players taken from the front, and their sound banks.
    #[doc(alias = "gm_8019E634")]
    pub fn tou_next_match_up(&mut self) {
        super::super::super::code::unported_call("gm_8019E634 (the match-up after a match)")
    }

    /// `gm_Scene_TouAlt_OnEnter`: the menus' sound banks, the archives (`GmTou1p`,
    /// `GmTou4p`) and the strings, the screen, the menus' song; the match's characters and
    /// stage (a random one drawn) preloaded with their sound banks, and the song again.
    #[doc(alias = "gm_Scene_TouAlt_OnEnter")]
    #[doc(alias = "gm_8019ECAC_OnEnter_inline")]
    pub fn tou_alt_on_enter(&mut self) {
        let a = &mut self.game.audio;
        a.lb.unwant_banks(0x12);
        a.lb.want_banks(2, 8);
        a.lb_start_bank_loads(self.data);
        self.game.audio.lb_wait_for_banks(self.data);
        let (l, s) = self.tou_load_scene("GmTou1p");
        self.alt().tou1 = l;
        self.alt().scene1 = Some(s);
        let (l, s) = self.tou_load_scene("GmTou4p");
        self.alt().tou4 = l;
        self.alt().scene4 = Some(s);
        let us = self.game.gm.unlocks.is_us();
        self.load_font_named(0, super::lib::strings_file(us), "SIS_TournamentData");
        self.alt_make();
        self.game.audio.lb_wait_for_banks(self.data);
        let bgm = i32::from(self.game.game_data.x1850.bgm);
        self.lb_play_music(bgm);
        let mut chars = [0i32; 4];
        let mut colors = [0u8; 4];
        for i in 0..4 {
            let m = self.game.tou.tm.x4b8[i];
            if m.x0 != 3 {
                chars[i] = ckind_of(i32::from(m.x1));
                colors[i] = m.x3;
            }
        }
        self.tou_pick_stage();
        let stage = if self.tou_stage_random() {
            self.game.tou.stage
        } else {
            self.game.tou.tm.x28 as i32
        };
        let _ = self.tou_stage_chosen();
        self.setup_vs_preload_cache();
        let mut mask = 0u64;
        for (m, &c) in self.game.tou.tm.x4b8.iter().zip(&chars) {
            if m.x0 != 3 {
                mask |= crate::lb::audio::character_banks(c);
            }
        }
        mask |= crate::lb::audio::stage_banks(stage);
        let a = &mut self.game.audio;
        a.lb.unwant_banks(0x1C);
        a.lb.want_banks(0xC, mask);
        a.lb_start_bank_loads(self.data);
        self.lb_play_music(bgm);
        let _ = colors;
    }

    /// `gm_Scene_TouAlt_OnExit`: the archives let go.
    #[doc(alias = "gm_Scene_TouAlt_OnExit")]
    pub fn tou_alt_on_exit(&mut self) {
        let a = self.game.tou.alt.clone();
        self.tou_free_archive(a.tou1);
        self.tou_free_archive(a.tou4);
    }

    /// The match-up screen's processes.
    pub(crate) fn tou_alt_run(&mut self, gobj: GObjId, addr: u32) -> bool {
        match addr {
            PROC_BG => self.alt_bg_proc(gobj),
            PROC_PANEL => self.alt_panel_proc(gobj),
            PROC_PLAYER => self.alt_player_proc(gobj),
            PROC_ICON => self.alt_icon_proc(gobj),
            PROC_BASE => self.alt_base_proc(gobj),
            PROC_TITLE => self.alt_title_proc(gobj),
            PROC_TAG => self.alt_tag_proc(gobj),
            PROC_READY => self.alt_ready_proc(gobj),
            PROC_BAR => self.alt_bar_proc(gobj),
            PROC_UNPLUGGED => self.alt_unplugged_proc(gobj),
            PROC_ROW => self.alt_row_proc(gobj),
            PROC_COUNTS => self.alt_counts_proc(gobj),
            PROC_HANDICAP => self.alt_handicap_proc(gobj),
            _ => return false,
        }
        true
    }
}
