//! The select screen's setup after its models (`mnCharSel_802640A0`'s rest) and the doors'
//! display (`mnCharSel_8025DB34` and what it calls).

use super::*;
use crate::hsd::sis::SisPtr;

/// `mnCharSel_804DC580`: the name lists' yellow.
const YELLOW: [u8; 3] = [255, 255, 0];
/// "・・・・": a name tag's text with no name.
const DOTS4: &[u8] = b"\x81\x45\x81\x45\x81\x45\x81\x45";
const DOTS11: &[u8] =
    b"\x81\x45\x81\x45\x81\x45\x81\x45\x81\x45\x81\x45\x81\x45\x81\x45\x81\x45\x81\x45\x81\x45";
/// "ＮＡＭＥ ＥＮＴＲＹ".
const NAME_ENTRY: &[u8] =
    b"\x82\x6d\x82\x60\x82\x6c\x82\x64\x20\x82\x64\x82\x6d\x82\x73\x82\x71\x82\x78";

/// Whether a match type's screen shows and clears the KO stars: VS mode's and the Special
/// Melee modes' but Camera Mode's and Stamina Mode's.
pub(super) fn counts_kos(match_type: u8) -> bool {
    match_type == VS_MELEE || (3..REG_CLASSIC).contains(&match_type)
}

impl Running<'_> {
    /// Each door's name tag (`CSSTagData`, a block of the heap, on a GObj of its own with
    /// `fn_802633B0`): its text under the door's name box, its list (hidden) of the names
    /// with "NAME ENTRY" first, the list's arrows, and the player's name tag when they have
    /// one; a name made in the name entry the screen came back from goes to the player who
    /// asked for it.
    pub(super) fn charsel_tags(&mut self, num_players: usize, ctx: i32) {
        for i in 0..num_players {
            let g = &mut *self.game;
            let gobj = g.world.create(4, 5, 0x80);
            let addr = crate::hsd::heap::mem_alloc(0x1C);
            g.world.init_user_data(gobj, 4, UserData::CssTag(i as u8));
            g.world.setup_proc(gobj, CodeRef(TAG_PROC), 4);
            let text = self.create_text_buffer(0, ctx);
            let s = &mut self.game.sis;
            let t = s.text_mut(text);
            t.x4c = 1;
            t.default_fitting = 1;
            t.default_alignment = 1;
            t.font_size = [0.058, 0.055];
            assert!(
                self.game.charsel.doors_used != 1,
                "the 1P modes' name tag (milestone 6)"
            );
            let j = self.menu_node(TAGS[i].name_joint);
            self.css_req(j, JOBJ_MASK, 2.0);
            self.css_anim_all(j);
            self.css_stop(j, JOBJ_MASK);
            let p = self.css_world(j);
            let t = self.game.sis.text_mut(text);
            t.pos = Vec3::new(
                mem(fp::fadds(reg(0.5), reg(p.x))),
                mem(fp::fsubs(reg(-0.4), reg(p.y))),
                p.z,
            );
            t.box_size = [160.0, 32.0];
            self.game.sis.add_string(text, 80.0, 0.0, Some(DOTS4));
            let j = self.menu_node(TAGS[i].list_joint);
            if i == 3 {
                self.css_translate_x(j, 16.7);
            }
            let p = self.css_world(j);
            let list = self.create_text_buffer(0, ctx);
            let s = &mut self.game.sis;
            let t = s.text_mut(list);
            t.default_fitting = 1;
            t.box_size = [154.0, 256.0];
            t.pos = Vec3::new(
                mem(fp::fsubs(reg(p.x), reg(0.6))),
                mem(fp::fsubs(fp::fsubs(reg(0.8), reg(p.y)), 1.0)),
                p.z,
            );
            t.font_size = [0.065, 0.065];
            t.x4e = 1;
            t.hidden = 1;
            s.add_string(list, 0.0, 0.0, Some(DOTS11));
            s.set_color(self.data, list, 0, YELLOW);
            let s = &mut self.game.sis;
            s.add_string(list, 0.0, 0.0, Some(NAME_ENTRY));
            s.set_color(self.data, list, 1, YELLOW);
            for _ in 0..9 {
                self.game.sis.add_string(list, 10.0, 0.0, Some(DOTS4));
            }
            let mut found = 0u8;
            while found < NO_TAG {
                if self.game.name_text(found).is_none() {
                    break;
                }
                found += 1;
            }
            let next_tag = found.wrapping_add(1);
            self.game.charsel.tags[i] = Some(TagData {
                addr,
                text,
                name_list: list,
                scroll: 0.0,
                scroll_amt: 0.0,
                scroll_force: 0.0,
                timer: 0,
                next_tag,
                port: i as u8,
                state: 0,
                use_tag: 0,
            });
            let j = self.menu_node(TAGS[i].arrow_up);
            jobj::set_flags(&mut self.game.objs.jobjs, j, HIDDEN);
            let j = self.menu_node(TAGS[i].arrow_down);
            self.set_hidden(j, next_tag <= 7);
            let player = self.door_player(i as u8);
            let c = &self.game.charsel;
            if player as i32 == i32::from(c.new_tag_port) && c.next_tag < next_tag {
                if counts_kos(self.css().match_type) {
                    self.game.vs_melee.ko_counts[player] = 0;
                }
                let tag = self.game.charsel.next_tag.wrapping_sub(1);
                self.css_start().players[player].nametag = tag;
                self.game.charsel.next_tag = next_tag;
            }
            let tag = self.css().vs.start.players[player].nametag;
            if tag != NO_TAG {
                self.game.charsel.tags[i].as_mut().expect("a tag").use_tag = 1;
                let name = self.game.name_text(tag);
                self.game
                    .sis
                    .replace_string(self.data, text, 0, name.as_deref());
                self.game.sis.text_mut(text).default_kerning = 0;
            }
        }
    }

    /// The rules' line under the doors (font 0's string 0x4A, `mnCharSel_8025BD30`) and the
    /// KO stars, in the VS modes' screens.
    pub(super) fn charsel_rules_line(&mut self, ctx: i32) {
        self.game.charsel.cpu_scroll = 0;
        let text = self.create_text(0, ctx, Vec3::new(-12.0, -23.3, 0.0), 450.0, 32.0);
        let t = self.game.sis.text_mut(text);
        t.default_alignment = 1;
        t.default_kerning = 1;
        t.default_fitting = 1;
        t.font_size = [0.07, 0.07];
        self.game.sis.reset_text(self.data, text, 0x4A);
        self.charsel_rules_string();
        let n = usize::from(self.game.charsel.doors_used);
        if counts_kos(self.css().match_type) {
            for i in 0..n {
                let j = self.menu_node(KO_STARS[i].0);
                let p = self.css_world(j);
                let y = mem(fp::fsubs(fp::fneg(reg(p.y)), reg(0.9)));
                let t = self.create_text(0, ctx, Vec3::new(p.x, y, p.z), 32.0, 32.0);
                self.game.charsel.ko_texts[i] = Some(t);
                self.game.sis.text_mut(t).font_size = [0.07, 0.07];
                self.charsel_ko_stars(i, 0);
            }
        } else {
            for i in 0..n {
                let j = self.menu_node(KO_STARS[i].0);
                jobj::set_flags_all(&mut self.game.objs.jobjs, j, HIDDEN);
            }
        }
    }

    /// "Ready to fight" (`css_models->press_start`), on render link 4 (`fn_80262F44`), held.
    pub(super) fn charsel_press_start(&mut self) {
        let (_, j) = self.charsel_model(PRESS_START, 4, PRESS_START_PROC, 3);
        jobj::req_anim_all(&mut self.game.objs, Some(j), 0.0);
        self.css_stop(Some(j), ALL_MASK);
    }

    /// The doors from the players: each door's kind and costume its player's, its timers and
    /// sliders reset, its icon as the coin's, and its sliders where the handicap and the
    /// CPU level put them.
    pub(super) fn charsel_doors_from_players(&mut self) {
        self.game.charsel.ready = 0;
        for i in 0..usize::from(self.game.charsel.doors_used) {
            let p = &self.css().vs.start.players[i];
            let (kind, color) = (p.slot_type, p.color);
            let d = &mut self.game.charsel.doors[i];
            d.p_kind = kind;
            d.costume = color;
            d.p_kind_prev = PKIND_NONE;
            d.slider_timer = 0;
            d.door_timer = 0;
            d.hold_handicap_slider = 0;
            d.hold_cpu_slider = 0;
            d.sel_icon_prev = d.sel_icon;
            let j = if self.game.game_data.x1850.handicap != 0 {
                let j = self.menu_node(DOORS[i].cpu_slider);
                let h = self.handicap_value(i);
                let x = mem(fp::fmuls(reg(1.25), f64::from((h - 1) as f32)));
                self.css_translate_x(j, x);
                self.menu_node(DOORS[i].cpu_slider2)
            } else {
                self.menu_node(DOORS[i].cpu_slider)
            };
            let p = &mut self.css_start().players[i];
            if p.cpu_level == 0 {
                p.cpu_level = 1;
            }
            let level = i32::from(p.cpu_level);
            let x = mem(fp::fmuls(reg(1.25), f64::from((level - 1) as f32)));
            self.css_translate_x(j, x);
        }
    }

    /// The match type's texture at the top left, free for all or teams, and each door's
    /// display, `mnCharSel_8025EE8C`.
    #[doc(alias = "mnCharSel_8025EE8C")]
    pub(super) fn charsel_mode_texture(&mut self, match_type: u8) {
        let info = MODE_INFO[usize::from(match_type)];
        let frame = if self.css().vs.start.rules.is_teams != 0 {
            info.1
        } else {
            info.0
        };
        let menu = self.game.charsel.menu;
        self.css_animate(menu, 0x24, TOBJ_MASK, f32::from(frame));
        for i in 0..self.game.charsel.doors_used {
            self.charsel_door(i);
        }
    }

    /// A door's costume and emblem textures on `frame`, shown or hidden,
    /// `mnCharSel_8025D5AC`.
    #[doc(alias = "mnCharSel_8025D5AC")]
    pub(super) fn charsel_door_icon(&mut self, door: usize, frame: i32, hidden: bool) {
        assert!(
            self.game.charsel.doors_used != 1,
            "the 1P modes' door icons (milestone 6)"
        );
        let menu = self.game.charsel.menu;
        let j = self.css_animate(menu, DOORS[door].costume, TOBJ_MASK, frame as f32);
        self.set_hidden(j, hidden);
        let j = self.css_animate(menu, DOORS[door].emblem, TOBJ_MASK, frame as f32);
        self.set_hidden(j, hidden);
    }

    /// A door's display brought up to date, `mnCharSel_8025DB34`: its icon hidden, the
    /// character's name in its name box, the first free costume for a new character, the
    /// door opening or closing with its kind, the sliders' name for a new kind, the player
    /// indicator, the team or port colors, the name box shown, and the costume's picture.
    #[doc(alias = "mnCharSel_8025DB34")]
    pub(super) fn charsel_door(&mut self, door: u8) {
        let d = usize::from(door);
        let sel_icon = self.game.charsel.doors[d].sel_icon;
        let mut hud = i32::from(ICONS[usize::from(sel_icon)].hud);
        self.charsel_door_icon(d, 0, true);
        let tag = self.game.charsel.tags[d].expect("a door's tag");
        if tag.use_tag == 0 && sel_icon < NO_ICON {
            self.game.sis.text_mut(tag.text).default_kerning = 1;
            let name = self.icon_name(sel_icon);
            self.game
                .sis
                .replace_string(self.data, tag.text, 0, Some(name));
        }
        let c = &mut self.game.charsel;
        let new_icon = c.doors[d].sel_icon;
        if new_icon != c.doors[d].sel_icon_prev {
            c.doors[d].sel_icon_prev = new_icon;
            self.first_free_costume(d);
            let p = self.game.charsel.pending;
            if p != 3 && p != 4 {
                self.lb_audio_call(2);
            }
        }
        assert!(
            self.game.charsel.doors_used != 1,
            "the 1P modes' door (milestone 6)"
        );
        let menu = self.game.charsel.menu;
        let j = self.menu_node(DOORS[d].door);
        let door_state = self.game.charsel.doors[d];
        let pending = self.game.charsel.pending;
        if door_state.p_kind_prev == PKIND_NONE {
            self.css_req(j, ALL_MASK, 0.0);
            self.css_anim_all(j);
            if door_state.p_kind == PKIND_NONE {
                self.css_stop(j, ALL_MASK);
            } else {
                self.charsel_ko_stars(d, 2);
                self.pick_unique_costume(d);
                self.game.charsel.doors[d].door_timer = 0x1E;
                if pending != 3 && pending != 4 {
                    self.sfx_play(0xB9, 0x7F, 0x40);
                }
            }
        } else {
            self.css_req(j, ALL_MASK, 40.0);
            self.css_anim_all(j);
            if door_state.p_kind == PKIND_NONE {
                self.charsel_ko_stars(d, 0);
                self.game.charsel.doors[d].door_timer = 0x1E;
                if pending != 3 && pending != 4 {
                    self.sfx_play(0xBA, 0x7F, 0x40);
                }
            } else {
                self.css_stop(j, ALL_MASK);
            }
        }
        let ds = self.game.charsel.doors[d];
        if ds.p_kind != ds.p_kind_prev {
            let sp90 = self.menu_node(DOORS[d].slider_name);
            let handicap = self.game.game_data.x1850.handicap;
            let level = f32::from(self.css().vs.start.players[d].cpu_level);
            match ds.p_kind {
                PKIND_HUMAN => {
                    if handicap != 0 {
                        let f = if handicap == 2 { 0.0 } else { 60.0 };
                        self.css_req(sp90, ALL_MASK, f);
                        self.css_animate(menu, DOORS[d].cpu_slider, TOBJ_MASK, level);
                    }
                }
                PKIND_CPU | PKIND_NONE => {
                    if handicap != 0 {
                        let f = match (ds.p_kind, handicap == 2) {
                            (PKIND_CPU, true) => 20.0,
                            (PKIND_CPU, false) => 80.0,
                            (_, true) => 30.0,
                            (_, false) => 90.0,
                        };
                        self.css_req(sp90, ALL_MASK, f);
                        let h = self.handicap_value(d);
                        let j = self.css_animate(menu, DOORS[d].cpu_slider, TOBJ_MASK, h as f32);
                        let h = self.handicap_value(d);
                        let x = mem(fp::fmuls(reg(1.25), f64::from((h - 1) as f32)));
                        self.css_translate_x(j, x);
                        self.css_animate(menu, DOORS[d].cpu_slider2, TOBJ_MASK, level);
                    } else {
                        let f = if ds.p_kind == PKIND_CPU { 40.0 } else { 50.0 };
                        self.css_req(sp90, ALL_MASK, f);
                        self.css_animate(menu, DOORS[d].cpu_slider, TOBJ_MASK, level);
                    }
                }
                _ => {}
            }
            self.game.charsel.doors[d].slider_timer = 0xA;
        }
        let c = &mut self.game.charsel;
        c.doors[d].p_kind_prev = c.doors[d].p_kind;
        let ds = c.doors[d];
        self.css_animate(
            menu,
            DOORS[d].indicator,
            ALL_MASK,
            f32::from(KIND_FRAME[usize::from(ds.p_kind)]),
        );
        let j = self.menu_node(DOORS[d].team);
        if self.css().vs.start.rules.is_teams == 0 {
            jobj::set_flags(&mut self.game.objs.jobjs, j, HIDDEN);
            let idx = d + if ds.p_kind != 0 { 4 } else { 0 };
            let f = f32::from(PORT_FRAME[idx]);
            self.css_animate(menu, DOORS[d].bg, TOBJ_MASK, f);
            self.css_animate(menu, DOORS[d].emblem, MOBJ_MASK, f);
        } else {
            jobj::clear_flags(&mut self.game.objs.jobjs, j, HIDDEN);
            let mut team = usize::from(ds.team);
            self.css_animate(menu, DOORS[d].team, TOBJ_MASK, f32::from(TEAM_FRAME[team]));
            if ds.p_kind != 0 {
                team += 4;
            }
            let f = f32::from(TEAM_FRAME[team]);
            self.css_animate(menu, DOORS[d].bg, TOBJ_MASK, f);
            self.css_animate(menu, DOORS[d].emblem, MOBJ_MASK, f);
            self.css_start().players[d].team = ds.team;
        }
        let tag = self.game.charsel.tags[d].expect("a door's tag");
        let hide = ds.p_kind == PKIND_NONE || (tag.use_tag == 0 && ds.sel_icon >= NO_ICON);
        self.game.sis.text_mut(tag.text).hidden = u8::from(hide);
        let ds = self.game.charsel.doors[d];
        if ds.sel_icon < NO_ICON {
            let color =
                if self.game.charsel.doors_used == 1 || self.css().vs.start.rules.is_teams == 0 {
                    ds.costume
                } else {
                    let ckind = ICONS[usize::from(ds.sel_icon)].ckind;
                    super::super::super::gm::vs_melee::team_costume(ckind, ds.team)
                };
            let player = self.door_player(door);
            self.css_start().players[player].color = color;
            hud += i32::from(color) * 0x1E;
            self.charsel_door_icon(d, hud, false);
        }
    }

    /// An icon's character's name: Mr. Game & Watch's as the English screen spells it,
    /// else the game's (`gm_80160980`).
    pub(super) fn icon_name(&self, sel_icon: u8) -> &'static [u8] {
        if self.game.game_data.saved_language() == super::super::super::gm::main_lib::LANG_US
            && sel_icon == 0x16
        {
            return GNW_NAME;
        }
        self.game
            .character_name(ICONS[usize::from(sel_icon)].ckind)
            .expect("an icon's character's name")
    }

    /// A door's KO stars, `mnCharSel_8025D1C4`: in the VS modes, the KOs its port scored this
    /// session, hidden for mode 0 or none; up to five as stars, more as a count beside one
    /// star, dim in mode 1 (its list open).
    #[doc(alias = "mnCharSel_8025D1C4")]
    pub(super) fn charsel_ko_stars(&mut self, port: usize, mode: i32) {
        if !counts_kos(self.css().match_type) {
            return;
        }
        let count = i32::from(self.game.vs_melee.ko_counts[port]);
        let mut star_x = reg(KO_STARS[port].1);
        let text = self.game.charsel.ko_texts[port].expect("a KO stars' text");
        let mut j = self.menu_node(KO_STARS[port].0);
        if mode == 0 || count == 0 {
            self.game.sis.text_mut(text).hidden = 1;
            jobj::set_flags_all(&mut self.game.objs.jobjs, j, HIDDEN);
            return;
        }
        jobj::clear_flags_all(&mut self.game.objs.jobjs, j, HIDDEN);
        let child = |r: &Self, j: Option<JObjId>| j.and_then(|j| r.game.objs.jobjs[j].child);
        let next = |r: &Self, j: Option<JObjId>| j.and_then(|j| r.game.objs.jobjs[j].next);
        if count < 6 {
            self.game.sis.text_mut(text).hidden = 1;
            let x = fp::fsubs(
                fp::fmadds(reg(0.9), fp::frsp(f64::from(count)), star_x),
                reg(4.5),
            );
            self.css_translate_x(j, mem(x));
            j = child(self, j);
            for (k, n) in [5, 4, 3, 2].into_iter().enumerate() {
                if k > 0 {
                    j = next(self, j);
                }
                if count < n {
                    jobj::set_flags(&mut self.game.objs.jobjs, j, HIDDEN);
                }
            }
        } else {
            self.game.sis.text_mut(text).hidden = 0;
            if count >= 1000 {
                star_x = fp::fsubs(star_x, 3.0);
            } else if count >= 100 {
                star_x = fp::fsubs(star_x, 1.5);
            }
            self.css_translate_x(j, mem(star_x));
            let t = self.game.sis.text_mut(text);
            t.pos.x = mem(fp::fadds(reg(0.6), star_x));
            t.text_color[3] = if mode == 1 { 0x50 } else { 0xA0 };
            let buf = SisPtr::Static(0x803F_0F15 + 0x20 * port as u32);
            let src = self
                .game
                .sis
                .table_entry(self.data, 0, 0x56)
                .expect("font 0's string 0x56");
            let end = self.game.sis.copy_string(self.data, buf, src);
            self.charsel_digits(end, count as u32);
            self.game.sis.reset_text(self.data, text, 0x56);
            self.game.sis.text_mut(text).sis_buffer = Some(buf);
            j = child(self, j);
            jobj::set_flags(&mut self.game.objs.jobjs, j, HIDDEN);
            j = next(self, j);
            jobj::set_flags(&mut self.game.objs.jobjs, j, HIDDEN);
            j = next(self, next(self, j));
            jobj::set_flags(&mut self.game.objs.jobjs, j, HIDDEN);
            j = next(self, j);
            jobj::set_flags(&mut self.game.objs.jobjs, j, HIDDEN);
        }
    }

    /// A number's glyphs (font 0's digits, its string 0x52) written at `dst`, up to 9999,
    /// with a terminator, `mnCharSel_8025BC20`; where the terminator is.
    #[doc(alias = "mnCharSel_8025BC20")]
    pub(super) fn charsel_digits(&mut self, dst: SisPtr, value: u32) -> SisPtr {
        let digits = self
            .game
            .sis
            .table_entry(self.data, 0, 0x52)
            .expect("font 0's digits");
        let step = |p: SisPtr, n: u32| match p {
            SisPtr::Arena(a) => SisPtr::Arena(a + n),
            SisPtr::Data(d) => SisPtr::Data(d.at(n)),
            SisPtr::Static(a) => SisPtr::Static(a + n),
        };
        let mut value = value.min(9999);
        let mut dst = dst;
        let put = |r: &mut Self, dst: &mut SisPtr, digit: u32| {
            let hi = r.game.sis.sis_byte(r.data, step(digits, 2 * digit));
            let lo = r.game.sis.sis_byte(r.data, step(digits, 2 * digit + 1));
            r.game.sis.set_sis_byte(*dst, hi);
            r.game.sis.set_sis_byte(step(*dst, 1), lo);
            *dst = step(*dst, 2);
        };
        let mut zeroes = 0;
        if value >= 1000 {
            put(self, &mut dst, value / 1000);
            value %= 1000;
            zeroes = 1;
        }
        if value >= 100 || zeroes != 0 {
            put(self, &mut dst, value / 100);
            value %= 100;
            zeroes += 1;
        }
        if value >= 10 || zeroes != 0 {
            put(self, &mut dst, value / 10);
            value %= 10;
        }
        put(self, &mut dst, value);
        self.game.sis.set_sis_byte(dst, 0);
        dst
    }

    /// The rules' line (font 0's string 0x4A rewritten in place), `mnCharSel_8025BD30`:
    /// Camera Mode's and Stamina Mode's own, else the time limit or stocks and the mode.
    #[doc(alias = "mnCharSel_8025BD30")]
    pub(super) fn charsel_rules_string(&mut self) {
        let dst = self
            .game
            .sis
            .table_entry(self.data, 0, 0x4A)
            .expect("font 0's string 0x4A");
        let s = |r: &mut Self, src: i32| r.game.sis.copy_table_string(r.data, 0, 0x4A, src);
        let a = |r: &mut Self, src: i32| r.game.sis.append_table_string(r.data, 0, 0x4A, src);
        match self.css().match_type {
            VS_CAMERA => return s(self, 0x4F),
            VS_STAMINA => return s(self, 0x50),
            _ => {}
        }
        assert!(
            self.game.game_data.saved_language() != super::super::super::gm::main_lib::LANG_JP,
            "the Japanese screen's rules line isn't ported"
        );
        let r = self.game.game_data.x1850.clone();
        match r.mode {
            1 => {
                self.charsel_digits(dst, u32::from(r.stock_count));
                a(self, if r.stock_count < 2 { 0x55 } else { 0x54 });
                a(self, 0x4C);
            }
            m @ (0 | 2 | 3) => {
                if r.time_limit != 0 {
                    self.charsel_digits(dst, u32::from(r.time_limit));
                    a(self, 0x53);
                } else {
                    s(self, 0x51);
                }
                a(
                    self,
                    match m {
                        0 => 0x4B,
                        2 => 0x4D,
                        _ => 0x4E,
                    },
                );
            }
            _ => {}
        }
    }
}
