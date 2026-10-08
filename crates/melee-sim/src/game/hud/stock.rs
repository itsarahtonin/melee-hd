//! The stock icons (`melee/if/ifstock.c`): beside each player's damage display, a row of
//! character icons for the stocks left (a counter past five), one icon in a timed match, or
//! a coin counter, with the losing icon's pop and a stolen stock's flight between two rows;
//! and the multi-man and single-player tallies, which a VS match only empties.

use gekko_fp as fp;
use melee_data::archive::DataRef;

use super::super::code;
use super::super::gm::vs::stock_icon_frame;
use super::super::handles::CodeRef;
use super::super::state::{Game, OBJ_JOBJ, Obj, Running, UserData};
use super::super::structs::IfStockUserData;
use super::Draw;
use super::HIDDEN;
use crate::fpu::{mem, reg};
use crate::hsd::gobj::{self, GObjId};
use crate::hsd::jobj::{self, JObjId};
use crate::hsd::{Objects, aobj, tobj};
use crate::vec::Vec3;

/// The icons' process and render callback.
pub const PROC_ICONS: u32 = 0x802F_9410;
pub const RENDER_ICONS: u32 = 0x802F_94E0;
/// The tallies' processes (single-player modes').
pub const PROC_TALLY: u32 = 0x802F_A6C4;
pub const PROC_TALLY_2: u32 = 0x802F_AC34;

/// The icon joint's first material's first texture.
fn icon_tobj(objs: &Objects, j: JObjId) -> Option<crate::hsd::tobj::TObjId> {
    let d = objs.jobjs[j].dobj().expect("jobj->u.dobj");
    let m = objs.dobjs[d].mobj.expect("dobj->mobj");
    objs.mobjs[m].tobj
}

/// A stock icon's texture on `frame`, held there.
fn set_icon_frame(objs: &mut Objects, j: JObjId, frame: f32) {
    let t = icon_tobj(objs, j);
    tobj::req_anim_all(objs, t, frame);
    let a = t.and_then(|t| objs.tobjs[t].aobj);
    aobj::set_rate(&mut objs.aobjs, a, 0.0);
}

impl Game {
    /// A player's stock icon frame: their character and costume, `gm_80168BF8`.
    #[doc(alias = "gm_80168BF8")]
    pub fn player_stock_icon(&self, slot: i32) -> f64 {
        stock_icon_frame(
            self.player_ckind(slot),
            self.player_fighter_kind(slot),
            self.player_costume(slot) as i32,
        )
    }

    /// The icons' render callback, `fn_802F94E0`: drawn unless the player's display hides
    /// its digits.
    #[doc(alias = "fn_802F94E0")]
    pub fn stock_icons_render(&self, gobj: GObjId, pass: i32) -> Option<Draw> {
        let Some(UserData::StockIcons(p)) = self.world.gobj(gobj).user_data else {
            panic!("stock icons without their player");
        };
        let p = self.hud.stock_users[usize::from(p)].player;
        let hidden = self.hud.status.players[usize::from(p)]
            .flags
            .hide_all_digits;
        (!hidden).then_some(Draw::JObjs(gobj, pass))
    }

    /// Colours a player's icons, `ifStock_802FB4EC`.
    #[doc(alias = "ifStock_802FB4EC")]
    pub fn color_stock_icons(&mut self, player: i32, c: [u8; 3]) {
        for i in 0..7 {
            let j = self.hud.stock.player[player as usize].x4[i + 1].expect("an icon");
            let d = self.objs.jobjs[j].dobj().expect("jobj->u.dobj");
            let m = self.objs.dobjs[d].mobj.expect("dobj->mobj");
            let mat = self.objs.mobjs[m].mat.as_mut().expect("mobj->mat");
            mat.diffuse.r = c[0];
            mat.diffuse.g = c[1];
            mat.diffuse.b = c[2];
        }
    }

    /// The icons' user data (`ifStock_804A157C[player]`).
    fn stock_user(&self, gobj: GObjId) -> usize {
        match self.world.gobj(gobj).user_data {
            Some(UserData::StockIcons(p)) => usize::from(p),
            _ => panic!("stock icons without their player"),
        }
    }
}

impl Running<'_> {
    /// `efSync_Spawn(kind, gobj, pos)`: effects are another port's.
    fn spawn_stock_effect(&mut self, kind: i32, gobj: GObjId, pos: Vec3) {
        crate::game::ef::spawn(self, kind, Some(gobj), &[crate::game::ef::Arg::Vec(pos)]);
    }

    /// A stock flies from `donor`'s row to `taker`'s (a team match's shared stock); 0, or
    /// 1 if the donor has none, 2 if both of its flights are taken, `ifStock_802F7EFC`.
    #[doc(alias = "ifStock_802F7EFC")]
    pub fn steal_stock(&mut self, taker: i32, donor: i32) -> i32 {
        let data = self.data;
        let g = &mut *self.game;
        if g.player_stocks(donor) == 0 {
            return 1;
        }
        let (t, d) = (taker as usize, donor as usize);
        let mut stocks = g.player_stocks(donor);
        if stocks < 0 {
            stocks = 1;
        }
        g.hud.stock.player[d].stocks = stocks;
        let u = &mut g.hud.stock_users[d];
        let slot = if u.anim[5] == 0 {
            5
        } else if u.anim[6] == 0 {
            6
        } else {
            return 2;
        };
        u.anim[slot] = 1;
        g.hud.stock_users[t].flag = 0;
        let u = &mut g.hud.stock_users[d];
        u.x3[slot - 5] = taker as u8;
        if stocks <= 5 && stocks > 0 {
            u.anim[(stocks - 1) as usize] = 10;
        }
        let j = if stocks <= 5 && stocks > 0 { stocks } else { 1 };
        let row = g.hud.stock.player[d].x4;
        let pos = g.objs.jobjs[row[0].expect("the icon row")].translate;
        let icon = row[j as usize];
        jobj::req_anim_all(&mut g.objs, icon, 0.0);
        jobj::anim_all(
            &mut g.objs,
            data,
            &mut crate::game::code::ParticleHooks,
            icon,
        );
        let s = g.objs.jobjs[icon.expect("an icon")].translate;
        let start = Vec3::new(
            mem(fp::fadds(reg(s.x), reg(pos.x))),
            mem(fp::fadds(reg(s.y), reg(pos.y))),
            mem(fp::fadds(reg(s.z), reg(pos.z))),
        );
        let ts = g.player_stocks(taker);
        let k = if ts < 5 && ts > 0 { ts } else { 0 };
        let trow = g.hud.stock.player[t].x4;
        let pos = g.objs.jobjs[trow[0].expect("the icon row")].translate;
        let icon = trow[k as usize];
        jobj::req_anim_all(&mut g.objs, icon, 0.0);
        jobj::anim_all(
            &mut g.objs,
            data,
            &mut crate::game::code::ParticleHooks,
            icon,
        );
        let e = g.objs.jobjs[trow[1].expect("an icon")].translate;
        let end = Vec3::new(
            mem(fp::fadds(
                reg(e.x),
                fp::fmadds(2.400_000_095_367_431_6, fp::frsp(f64::from(k)), reg(pos.x)),
            )),
            start.y,
            mem(fp::fadds(reg(e.z), reg(pos.z))),
        );
        let mid = Vec3::new(
            mem(fp::fmuls(0.5, fp::fadds(reg(end.x), reg(start.x)))),
            mem(fp::fadds(10.0, reg(start.y))),
            mem(fp::fmuls(0.5, fp::fadds(reg(end.z), reg(start.z)))),
        );
        let f = &mut g.hud.stock_users[d].steal[slot - 5];
        f.start = start;
        f.end = end;
        f.mid = mid;
        0
    }

    /// The icons for a stock match, `ifStock_802F8298`: up to five icons, a counter past
    /// five, a lost stock's icon popping (with its effect), and the flights of stolen stocks.
    #[doc(alias = "ifStock_802F8298")]
    pub fn stock_icons_proc(&mut self, gobj: GObjId) {
        let data = self.data;
        let Some(Obj::JObj(root)) = self.game.world.gobj(gobj).obj else {
            panic!("stock icons without their model");
        };
        let ui = self.game.stock_user(gobj);
        let p = usize::from(self.game.hud.stock_users[ui].player);
        let mut stocks = self.game.player_stocks(p as i32);
        if stocks > 99 {
            stocks = 99;
        }
        if stocks < 0 {
            stocks = 1;
        }
        self.game.hud.stock.player[p].stocks = stocks;
        let frame = mem(self.game.player_stock_icon(p as i32));
        let row = self.game.hud.stock.player[p].clone();
        if stocks <= 5 {
            jobj::set_flags_all(&mut self.game.objs.jobjs, row.x3C, HIDDEN);
            for i in 0..5 {
                let icon = row.x4[i + 1].expect("an icon");
                if (i as i32) < stocks {
                    jobj::clear_flags_all(&mut self.game.objs.jobjs, Some(icon), HIDDEN);
                    let u = &mut self.game.hud.stock_users[p];
                    u.anim[i] = if u.flag != 0 { 0 } else { 10 };
                } else {
                    let u = &mut self.game.hud.stock_users[p];
                    if u.flag == 0 {
                        u.anim[i] = 10;
                    }
                    if u.anim[i] == 0 {
                        let objs = &self.game.objs;
                        let a = objs.jobjs[root].translate;
                        let b = objs.jobjs[icon].translate;
                        let at = Vec3::new(
                            mem(fp::fadds(reg(b.x), reg(a.x))),
                            mem(fp::fadds(reg(b.y), reg(a.y))),
                            mem(fp::fadds(reg(b.z), reg(a.z))),
                        );
                        self.spawn_stock_effect(0x474, gobj, at);
                    }
                    let u = &mut self.game.hud.stock_users[p];
                    if u.anim[i] < 10 {
                        u.anim[i] += 1;
                    }
                }
                let a = self.game.hud.stock_users[p].anim[i];
                let objs = &mut self.game.objs;
                jobj::req_anim_all(objs, Some(icon), mem(fp::frsp(f64::from(a))));
                set_icon_frame(objs, icon, frame);
            }
        } else {
            let g = &mut *self.game;
            let objs = &mut g.objs;
            jobj::clear_flags_all(&mut objs.jobjs, row.x3C, HIDDEN);
            if stocks >= 10 {
                jobj::req_anim_all(objs, row.x44, mem(fp::frsp(f64::from(stocks / 10))));
                jobj::req_anim_all(objs, row.x40, mem(fp::frsp(f64::from(stocks % 10))));
            } else {
                jobj::req_anim_all(objs, row.x44, mem(fp::frsp(f64::from(stocks))));
                jobj::set_flags_all(&mut objs.jobjs, row.x40, HIDDEN);
            }
            for i in 0..5 {
                let icon = row.x4[i + 1].expect("an icon");
                let a = if i == 0 { 0 } else { 10 };
                g.hud.stock_users[p].anim[i] = a;
                jobj::req_anim_all(&mut g.objs, Some(icon), mem(fp::frsp(f64::from(a))));
                set_icon_frame(&mut g.objs, icon, frame);
            }
        }
        for i in 5..=6 {
            let icon = row.x4[i + 1].expect("a flying icon");
            let a = self.game.hud.stock_users[p].anim[i];
            if a == 0 {
                jobj::set_flags_all(&mut self.game.objs.jobjs, Some(icon), HIDDEN);
            } else {
                jobj::clear_flags_all(&mut self.game.objs.jobjs, Some(icon), HIDDEN);
                if a <= 10 {
                    let f = self.game.hud.stock_users[p].steal[i - 5].clone();
                    let t = fp::fmuls(0.100_000_001_490_116_12, fp::frsp(f64::from(a)));
                    let mut c = Vec3::quadratic([f.start, f.mid, f.end], mem(t));
                    let r0 = self.game.objs.jobjs[row.x4[0].expect("the icon row")].translate;
                    c.x = mem(fp::fsubs(reg(c.x), reg(r0.x)));
                    c.y = mem(fp::fsubs(reg(c.y), reg(r0.y)));
                    c.z = mem(fp::fsubs(reg(c.z), reg(r0.z)));
                    jobj::set_translate(&mut self.game.objs.jobjs, icon, c);
                    if a == 1 {
                        c.x = f.start.x;
                        c.y = f.start.y;
                        self.spawn_stock_effect(0x475, gobj, c);
                    } else if a == 10 {
                        c.x = f.end.x;
                        c.y = f.end.y;
                        self.spawn_stock_effect(0x476, gobj, c);
                    }
                    self.game.hud.stock_users[p].anim[i] += 1;
                } else {
                    let u = &mut self.game.hud.stock_users[p];
                    u.anim[i] = 0;
                    let other = usize::from(u.x3[i - 5]);
                    self.game.hud.stock_users[other].flag = 1;
                }
            }
            set_icon_frame(&mut self.game.objs, icon, frame);
        }
        jobj::anim_all(
            &mut self.game.objs,
            data,
            &mut crate::game::code::ParticleHooks,
            Some(root),
        );
    }

    /// The coin counter, `ifStock_802F89F8`: up to five digits of the player's coins.
    #[doc(alias = "ifStock_802F89F8")]
    pub fn stock_coins_proc(&mut self, gobj: GObjId) {
        let data = self.data;
        let Some(Obj::JObj(root)) = self.game.world.gobj(gobj).obj else {
            panic!("stock icons without their model");
        };
        let ui = self.game.stock_user(gobj);
        let p = usize::from(self.game.hud.stock_users[ui].player);
        let g = &mut *self.game;
        let first = g.hud.stock.player[p].x4[1].expect("an icon");
        let coins = g.player_coins(p as i32);
        g.hud.stock.player[p].coins = coins;
        let coins = if coins as u32 > 99_999 { 99_999 } else { coins };
        let mut count = 0;
        let mut c = coins;
        for i in 0..32 {
            if c == 0 {
                count = i;
                break;
            }
            c /= 10;
        }
        let count = count.min(5);
        jobj::req_anim_all(&mut g.objs, Some(first), 0.0);
        let frame = mem(g.player_stock_icon(p as i32));
        set_icon_frame(&mut g.objs, first, frame);
        let row = g.hud.stock.player[p].clone();
        let at = |k: usize| -> Option<JObjId> {
            if k < 8 {
                row.x4[k]
            } else {
                [
                    row.x24, row.x28, row.x2C, row.x30, row.x34, row.x38, row.x3C, row.x40, row.x44,
                ][k - 8]
            }
        };
        for i in 0..5 {
            let j = at(13 - i);
            if (i as i32) < count {
                jobj::clear_flags_all(&mut g.objs.jobjs, j, HIDDEN);
                let pos = count - i as i32;
                let digit = if pos - 1 == 0 {
                    coins % 10
                } else {
                    let mut div = 1;
                    for _ in 0..pos - 1 {
                        div *= 10;
                    }
                    coins / div % 10
                };
                jobj::req_anim_all(&mut g.objs, j, mem(fp::frsp(f64::from(digit))));
            } else {
                jobj::set_flags_all(&mut g.objs.jobjs, j, HIDDEN);
                if count == 0 && i == 0 {
                    jobj::req_anim_all(&mut g.objs, row.x38, 0.0);
                    jobj::clear_flags_all(&mut g.objs.jobjs, row.x38, HIDDEN);
                }
            }
        }
        jobj::anim_all(
            &mut g.objs,
            data,
            &mut crate::game::code::ParticleHooks,
            Some(root),
        );
    }

    /// The icons' process by the row's mode (stocks, one icon, coins), `fn_802F9410`.
    #[doc(alias = "fn_802F9410")]
    pub fn stock_proc(&mut self, gobj: GObjId) {
        let ui = self.game.stock_user(gobj);
        match self.game.hud.stock_users[ui].mode {
            0 => self.stock_icons_proc(gobj),
            1 => {
                let data = self.data;
                let Some(Obj::JObj(root)) = self.game.world.gobj(gobj).obj else {
                    panic!("stock icons without their model");
                };
                let p = usize::from(self.game.hud.stock_users[ui].player);
                let g = &mut *self.game;
                let first = g.hud.stock.player[p].x4[1].expect("an icon");
                jobj::req_anim_all(&mut g.objs, Some(first), 0.0);
                let frame = mem(g.player_stock_icon(p as i32));
                set_icon_frame(&mut g.objs, first, frame);
                jobj::anim_all(
                    &mut g.objs,
                    data,
                    &mut crate::game::code::ParticleHooks,
                    Some(root),
                );
            }
            2 => self.stock_coins_proc(gobj),
            _ => {}
        }
    }

    /// A player's row of icons in mode `mode` (0 stocks, 1 one icon, 2 coins), replacing
    /// any, darkened for a metal or invisible player, `ifStock_802F98E8`.
    #[doc(alias = "ifStock_802F98E8")]
    pub fn create_stock_icons(&mut self, player: u8, mode: u8) {
        let data = self.data;
        let Some(models) = self.game.hud.stock.x0 else {
            return;
        };
        let p = usize::from(player);
        let u = &mut self.game.hud.stock_users[p];
        u.player = player;
        u.mode = mode;
        u.flag = 1;
        if let Some(old) = self.game.hud.stock.player[p].x0 {
            gobj::free(self, old);
        }
        let g = &mut *self.game;
        let o = g.world.create(14, 15, 0);
        g.world.gobj_mut(o).user_data = Some(UserData::StockIcons(player));
        let model = data.ptr(models).expect("the icons' model");
        let root = jobj::load_joint(&mut g.objs, data, data.ptr(model)).expect("a model");
        g.world.set_obj(o, OBJ_JOBJ, Obj::JObj(root));
        g.world.setup_gx_link(o, CodeRef(RENDER_ICONS), 11, 0);
        super::super::gm::vs::add_model_anim(&mut g.objs, data, Some(root), model, 0);
        jobj::req_anim_all(&mut g.objs, Some(root), 0.0);
        g.world.setup_proc(o, CodeRef(PROC_ICONS), 17);
        let pos = g.player_hud_position(i32::from(player));
        jobj::set_translate(&mut g.objs.jobjs, root, pos);
        let indices: Vec<i32> = (0..17).collect();
        let found = super::super::gm::pause::tree_nodes(&g.objs.jobjs, Some(root), &indices);
        let r = &mut g.hud.stock.player[p];
        for (k, j) in found.into_iter().enumerate() {
            match k {
                0..=7 => r.x4[k] = j,
                8 => r.x24 = j,
                9 => r.x28 = j,
                10 => r.x2C = j,
                11 => r.x30 = j,
                12 => r.x34 = j,
                13 => r.x38 = j,
                14 => r.x3C = j,
                15 => r.x40 = j,
                _ => r.x44 = j,
            }
        }
        r.x0 = Some(o);
        let coins = g.player_coins(i32::from(player));
        g.hud.stock.player[p].coins = coins;
        let mut stocks = g.player_stocks(i32::from(player));
        if stocks < 0 {
            stocks = 1;
        }
        g.hud.stock.player[p].stocks = stocks;
        let frame = mem(g.player_stock_icon(i32::from(player)));
        let row = g.hud.stock.player[p].clone();
        let hide =
            |g: &mut Game, j: Option<JObjId>| jobj::set_flags_all(&mut g.objs.jobjs, j, HIDDEN);
        match mode {
            0 => {
                hide(g, row.x24);
                if stocks <= 5 {
                    hide(g, row.x3C);
                    for i in 0..7 {
                        let icon = row.x4[i + 1];
                        let a = if (i as i32) < stocks || i >= 5 {
                            if i >= 5 {
                                hide(g, icon);
                            }
                            0
                        } else {
                            hide(g, icon);
                            10
                        };
                        g.hud.stock_users[p].anim[i] = a;
                        let icon = icon.expect("an icon");
                        jobj::req_anim_all(&mut g.objs, Some(icon), mem(fp::frsp(f64::from(a))));
                        set_icon_frame(&mut g.objs, icon, frame);
                    }
                }
            }
            1 | 2 => {
                if mode == 1 {
                    hide(g, row.x24);
                    hide(g, row.x3C);
                } else {
                    for j in [row.x28, row.x2C, row.x30, row.x34, row.x3C] {
                        hide(g, j);
                    }
                }
                for i in 0..7 {
                    let icon = row.x4[i + 1];
                    let a = if i == 0 { 0 } else { 10 };
                    g.hud.stock_users[p].anim[i] = a;
                    if i < 5 {
                        let ic = icon.expect("an icon");
                        jobj::req_anim_all(&mut g.objs, Some(ic), mem(fp::frsp(f64::from(a))));
                        set_icon_frame(&mut g.objs, ic, frame);
                        if i != 0 {
                            hide(g, icon);
                        }
                    } else {
                        hide(g, icon);
                    }
                }
            }
            _ => {}
        }
        jobj::anim_all(
            &mut g.objs,
            data,
            &mut crate::game::code::ParticleHooks,
            Some(root),
        );
        let flags = g.vs.state.fighters[p].x4[0];
        if flags & 0x40 != 0 {
            g.color_stock_icons(i32::from(player), [0x08, 0x08, 0x08]);
        } else if flags & 0x80 != 0 {
            g.color_stock_icons(i32::from(player), [0x3C, 0x3C, 0x46]);
        }
        self.stock_proc(o);
    }

    /// The single-player tally's process, `fn_802FA6C4`: outside those modes it only frees
    /// its icons once.
    #[doc(alias = "fn_802FA6C4")]
    pub fn stock_tally_proc(&mut self, _gobj: GObjId) {
        let g = &mut *self.game;
        if g.vs_is_singleplayer() && g.gm.spawn_flags & 0x40 != 0 {
            code::unported_call("fn_802FA6C4 in a single-player mode");
        }
        if g.hud.stock_1774.x0 == 1 {
            for j in (0..130).rev() {
                if let Some(o) = self.game.hud.stock_1774.x10C[1 + j].take() {
                    gobj::free(self, o);
                }
            }
        }
        self.game.hud.stock_1774.x0 = 0;
    }

    /// The challenger record's tally's process, `fn_802FAC34`: without a record it only
    /// frees its icons once.
    #[doc(alias = "fn_802FAC34")]
    pub fn stock_tally_proc_2(&mut self, _gobj: GObjId) {
        if self.game.vs_has_x58() {
            code::unported_call("fn_802FAC34 with a challenger's record");
        }
        if self.game.hud.stock_1acc.x0 == 1 {
            for i in (0..130).rev() {
                if let Some(o) = self.game.hud.stock_1acc.x10C[i].take() {
                    gobj::free(self, o);
                }
            }
        }
        self.game.hud.stock_1acc.x0 = 0;
    }

    /// The icons' models from `IfAll.dat`'s `Stc_scemdls`, the tallies' processes, and the
    /// single-player icons none of a VS match needs, `ifStock_802FAEC4`.
    #[doc(alias = "ifStock_802FAEC4")]
    pub fn load_stock_icons(&mut self) {
        let data = self.data;
        let a = self.game.hud.archive.expect("IfAll.dat");
        let g = &mut *self.game;
        g.hud.stock = Default::default();
        g.hud.stock_1acc = Default::default();
        g.hud.stock_1a8c = [None; 16];
        g.hud.stock_1774 = Default::default();
        let at = data
            .get(a)
            .public("Stc_scemdls")
            .expect("IfAll.dat's Stc_scemdls");
        let models = DataRef {
            archive: a,
            offset: at,
        };
        g.hud.stock.x0 = Some(models);
        g.hud.stock.x4 = data.ptr(models.at(4));
        g.hud.stock_1acc.x108 = None;
        g.hud.stock_1acc.x0 = 0;
        let o = g.world.create(14, 15, 0);
        g.world.setup_proc(o, CodeRef(PROC_TALLY_2), 17);
        g.hud.stock_1acc.x108 = Some(o);
        if g.gm.spawn_flags & 0x10 == 0 {
            g.hud.stock_1774.x10C[0] = None;
            g.hud.stock_1774.x0 = 0;
            let o = g.world.create(14, 15, 0);
            g.world.setup_proc(o, CodeRef(PROC_TALLY), 17);
            g.hud.stock_1774.x10C[0] = Some(o);
        } else if g.gm.spawn_flags & 0x40 != 0 {
            code::unported_call("ifStock_802FA5BC");
        }
        if g.vs_x9_is_1() {
            code::unported_call("ifStock_802FA118");
        }
        if (super::super::gm::GM_10MAN_VS..=super::super::gm::GM_CRUEL_VS).contains(&g.game_mode) {
            code::unported_call("the multi-man counter of ifStock_802FAEC4");
        }
    }

    /// Frees the single-player tally's icons, `ifStock_802FB390`.
    #[doc(alias = "ifStock_802FB390")]
    pub fn free_stock_tally(&mut self) {
        if let Some(o) = self.game.hud.stock_1774.x10C[0].take() {
            gobj::free(self, o);
        }
        for i in (0..130).rev() {
            if let Some(o) = self.game.hud.stock_1774.x10C[i + 1].take() {
                gobj::free(self, o);
            }
        }
    }

    /// `ifStock_802FB41C`.
    #[doc(alias = "ifStock_802FB41C")]
    pub fn free_stock_bonus_icons(&mut self) {
        for i in 0..16 {
            if let Some(o) = self.game.hud.stock_1a8c[i].take() {
                gobj::free(self, o);
            }
        }
    }

    /// `ifStock_802FB484`.
    #[doc(alias = "ifStock_802FB484")]
    pub fn free_stock_tally_2(&mut self) {
        for i in 0..16 {
            if let Some(o) = self.game.hud.stock_1acc.x10C[i].take() {
                gobj::free(self, o);
            }
        }
    }

    /// Frees a player's icons, `ifStock_802FB650`.
    #[doc(alias = "ifStock_802FB650")]
    pub fn free_stock_icons(&mut self, player: i32) {
        if let Some(o) = self.game.hud.stock.player[player as usize].x0 {
            gobj::free(self, o);
        }
        self.game.hud.stock.player[player as usize] = Default::default();
    }

    /// Clears the second flag of a player's icons' processes, `ifStock_802FB6AC`.
    #[doc(alias = "ifStock_802FB6AC")]
    pub fn stock_icons_flags(&mut self, player: i32) {
        if let Some(o) = self.game.hud.stock.player[player as usize].x0 {
            self.game.world.clear_flags_2(o);
        }
    }
}

/// The type of the icons' user data, for checks.
pub type StockUser = IfStockUserData;
