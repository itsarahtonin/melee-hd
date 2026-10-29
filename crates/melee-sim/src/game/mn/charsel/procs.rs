//! The select screen's processes but the hands' (`fn_8025F0E0` to `fn_802633B0`), the coins'
//! picks and returns, and the scene's frame and exit.

use super::setup::counts_kos;
use super::*;

/// `mnCharSel_804DC560`, `804DC564`: a name list's white and the gray of names taken.
const WHITE: [u8; 3] = [255, 255, 255];
const GRAY: [u8; 3] = [100, 100, 100];
/// "ＮＡＭＥ　ＣＡＮＣＥＬ".
const NAME_CANCEL: &[u8] =
    b"\x82\x6d\x82\x60\x82\x6c\x82\x64\x81\x40\x82\x62\x82\x60\x82\x6d\x82\x62\x82\x64\x82\x6b";

impl Running<'_> {
    /// Runs one of the screen's processes.
    pub fn charsel_run(&mut self, gobj: GObjId, f: u32) -> bool {
        match f {
            CURSOR_THINK => self.charsel_cursor_think(gobj),
            MENU_PROC => self.charsel_menu_proc(gobj),
            DEBUG_CAMERA_PROC => self.charsel_debug_camera_proc(gobj),
            ANIM_PROC => self.charsel_anim_proc(gobj),
            TOKEN_PROC => self.charsel_token_proc(gobj),
            PRESS_START_PROC => self.charsel_press_start_proc(gobj),
            BG_PROC => self.charsel_bg_proc(gobj),
            TAG_PROC => self.charsel_tag_proc(gobj),
            _ => return false,
        }
        true
    }

    pub(super) fn css_gobj_jobj(&self, gobj: GObjId) -> Option<JObjId> {
        match self.game.world.gobj(gobj).obj {
            Some(Obj::JObj(j)) => Some(j),
            _ => None,
        }
    }

    /// `fn_8025F0E0`, the menu's process: its blinking joint restarted but every tenth frame,
    /// the icons' picked flash ended, each door's open or close and its sliders' name settled
    /// as their timers end, the costumes' pictures dimmed while a coin is held, and its
    /// animations.
    #[doc(alias = "fn_8025F0E0")]
    pub fn charsel_menu_proc(&mut self, gobj: GObjId) {
        let j = self.css_gobj_jobj(gobj);
        if !self.game.charsel.frame.is_multiple_of(10) {
            let n = self.css_node(j, 0x27);
            self.css_req(n, ALL_MASK, 0.0);
        }
        let one = self.game.charsel.doors_used == 1;
        for i in 0..25 {
            let t = self.game.charsel.icons[i].anim_timer;
            if t != 0 {
                let t = t - 1;
                self.game.charsel.icons[i].anim_timer = t;
                if t == 0 {
                    let joint = if one {
                        ICONS[i].joint_1p
                    } else {
                        ICONS[i].joint_vs
                    };
                    let n = self.css_node(j, joint);
                    self.css_req(n, TOBJ_MASK, 0.0);
                    self.css_anim_all(n);
                    self.css_stop(n, TOBJ_MASK);
                }
            }
        }
        if self.game.charsel.doors_used == 4 {
            for i in 0..4 {
                let t = self.game.charsel.doors[i].door_timer;
                if t != 0 {
                    let t = t - 1;
                    self.game.charsel.doors[i].door_timer = t;
                    if t == 0 {
                        let n = self.css_node(j, DOORS[i].door);
                        self.css_stop(n, ALL_MASK);
                    }
                }
                let t = self.game.charsel.doors[i].slider_timer;
                if t != 0 {
                    let t = t - 1;
                    self.game.charsel.doors[i].slider_timer = t;
                    if t == 0 {
                        self.charsel_sliders_settle(i);
                    }
                }
            }
        } else {
            super::super::super::code::unported_call(
                "fn_8025F0E0's CPU level slider (the 1P modes', milestone 6)",
            );
        }
        if one {
            super::super::super::code::unported_call(
                "fn_8025F0E0's one-door costume (the 1P modes', milestone 6)",
            );
        }
        for i in 0..usize::from(self.game.charsel.doors_used) {
            let n = self.menu_node(DOORS[i].costume);
            let held = self.game.charsel.tokens[i].expect("a coin").held_by != 0;
            let m = crate::hsd::jobj::get_dobj(&self.game.objs.jobjs, n)
                .and_then(|d| self.game.objs.dobjs[d].mobj);
            if let Some(m) = m {
                crate::hsd::mobj::set_alpha(
                    &mut self.game.objs.mobjs,
                    m,
                    if held { 0.5 } else { 1.0 },
                );
            }
        }
        self.css_anim_all(j);
    }

    /// A door's sliders' name and positions as its kind and the handicap rule leave them
    /// (the slider timer's end in `fn_8025F0E0`).
    fn charsel_sliders_settle(&mut self, i: usize) {
        let menu = self.game.charsel.menu;
        let handicap = self.game.game_data.x1850.handicap;
        let kind = self.game.charsel.doors[i].p_kind;
        let level = f32::from(self.css().vs.start.players[i].cpu_level);
        if handicap != 0 {
            let f = match (kind == PKIND_CPU, handicap == 1) {
                (true, true) => 0x5A,
                (true, false) => 0x1E,
                (false, true) => 0x50,
                (false, false) => 0x14,
            };
            self.css_animate(menu, DOORS[i].slider_name, ALL_MASK, f as f32);
            let h = self.handicap_value(i);
            self.css_animate(menu, DOORS[i].cpu_slider, TOBJ_MASK, h as f32);
            self.css_animate(menu, DOORS[i].cpu_slider2, TOBJ_MASK, level);
        } else {
            let f = if kind == PKIND_CPU { 50.0 } else { 40.0 };
            self.css_animate(menu, DOORS[i].slider_name, ALL_MASK, f);
            self.css_animate(menu, DOORS[i].cpu_slider, TOBJ_MASK, level);
        }
    }

    /// `fn_8025FAC0`, Camera Mode's camera model: its child's animations restarted every
    /// 40 frames.
    // Ports `fn_8025FAC0`; aliased once a check reaches it.
    pub fn charsel_debug_camera_proc(&mut self, gobj: GObjId) {
        let j = self.css_gobj_jobj(gobj);
        if self.game.charsel.frame.is_multiple_of(40) {
            let child = j.and_then(|j| self.game.objs.jobjs[j].child);
            jobj::req_anim_all(&mut self.game.objs, child, 0.0);
        }
        self.css_anim_all(j);
    }

    /// `fn_8025FB2C`: a model's animations.
    // Ports `fn_8025FB2C`; aliased once a check reaches it.
    pub fn charsel_anim_proc(&mut self, gobj: GObjId) {
        let j = self.css_gobj_jobj(gobj);
        self.css_anim_all(j);
    }

    /// `fn_80263354`, the background: its animations, restarted every 200 frames.
    #[doc(alias = "fn_80263354")]
    pub fn charsel_bg_proc(&mut self, gobj: GObjId) {
        let j = self.css_gobj_jobj(gobj);
        if self.game.charsel.frame.is_multiple_of(200) {
            jobj::req_anim_all(&mut self.game.objs, j, 0.0);
        }
        self.css_anim_all(j);
    }

    /// A door's coin dropped on a random unlocked icon, `mnCharSel_8025FB50`: the character
    /// picked, its first free costume, the coin on the icon (at once with `snap`), drawn
    /// before the last hand, the icon's flash and the character's call.
    #[doc(alias = "mnCharSel_8025FB50")]
    pub(super) fn charsel_random_pick(&mut self, door: u8, snap: bool) {
        let icon = loop {
            let i = self.game.objs.rng.below(0x19) as usize;
            if self.game.charsel.icons[i].state != 0 {
                break i;
            }
        };
        let player = self.door_player(door);
        self.css_start().players[player].ckind = ICONS[icon].ckind as i8;
        let d = usize::from(door);
        self.game.charsel.doors[d].sel_icon = icon as u8;
        if self.game.charsel.doors[d].sel_icon != self.game.charsel.doors[d].sel_icon_prev {
            self.first_free_costume(d);
        }
        self.charsel_coin_on_icon(d, icon, snap);
        self.charsel_icon_flash(icon);
        let sel = usize::from(self.game.charsel.doors[d].sel_icon);
        self.charsel_pick_call(sel);
    }

    /// A coin set on an icon (placed, `x5` 0) and drawn before the last hand.
    fn charsel_coin_on_icon(&mut self, d: usize, icon: usize, snap: bool) {
        let bu = self.game.charsel.icons[icon].bound_u;
        let t = self.game.charsel.tokens[d].as_mut().expect("a coin");
        t.held_by = 0;
        t.target_x = mem(fp::fadds(reg(3.4), reg(ICONS[icon].bound_l)));
        t.target_y = mem(fp::fadds(reg(-3.0), reg(bu)));
        if snap {
            t.x = t.target_x;
            t.y = t.target_y;
        }
        let g = t.gobj;
        let last = usize::from(self.game.charsel.doors_used) - 1;
        let hand = self.game.charsel.cursors[last].expect("a hand").gobj;
        self.game.world.gx_relink_before(g, hand);
    }

    /// An icon's picked flash: its textures from frame 10, for 12 frames.
    fn charsel_icon_flash(&mut self, icon: usize) {
        let joint = if self.game.charsel.doors_used == 1 {
            ICONS[icon].joint_1p
        } else {
            ICONS[icon].joint_vs
        };
        let n = self.menu_node(joint);
        self.css_req(n, TOBJ_MASK, 10.0);
        self.game.charsel.icons[icon].anim_timer = 0xC;
    }

    /// The announcer calls the character picked (`lbAudioAx_80023870` on its icon's call) and
    /// its fanfare's group is kept (`gm_80168C5C`).
    fn charsel_pick_call(&mut self, sel: usize) {
        self.sfx_play_track(ICONS[sel].sfx, 0x7F, 0x40, sel as i32 + 0x8A);
        self.preload_character_sfx(u32::from(ICONS[sel].ckind));
    }

    /// A held coin put back on its player's character's icon, `mnCharSel_8025FDEC`: 0 if the
    /// player has a character, else 1 and nothing done.
    #[doc(alias = "mnCharSel_8025FDEC")]
    pub(super) fn charsel_coin_back(&mut self, door: u8) -> i32 {
        let player = self.door_player(door);
        let ckind = self.css().vs.start.players[player].ckind;
        if ckind >= CKIND_PLAYABLE {
            return 1;
        }
        let d = usize::from(door);
        let sel = usize::from(self.game.charsel.doors[d].sel_icon);
        if i32::from(ckind) != i32::from(ICONS[sel].ckind) {
            self.game.charsel.doors[d].costume = 0;
        }
        let icon = (0..25)
            .find(|&i| i32::from(ckind) == i32::from(ICONS[i].ckind))
            .unwrap_or(25);
        let bu = self.game.charsel.icons.get(icon).map_or(0.0, |i| i.bound_u);
        let t = self.game.charsel.tokens[d].as_mut().expect("a coin");
        t.target_x = mem(fp::fadds(reg(3.4), reg(ICONS[icon].bound_l)));
        t.target_y = mem(fp::fadds(reg(-3.0), reg(bu)));
        let c = &mut self.game.charsel;
        c.doors[d].sel_icon_prev = icon as u8;
        c.doors[d].sel_icon = icon as u8;
        let g = c.tokens[d].expect("a coin").gobj;
        let last = usize::from(c.doors_used) - 1;
        let hand = c.cursors[last].expect("a hand").gobj;
        self.game.world.gx_relink_before(g, hand);
        self.game.charsel.tokens[d]
            .as_mut()
            .expect("a coin")
            .held_by = 0;
        self.charsel_icon_flash(icon);
        self.pick_unique_costume(d);
        let p = self.game.charsel.pending;
        if p != 3 && p != 4 {
            let sel = usize::from(self.game.charsel.doors[d].sel_icon);
            self.charsel_pick_call(sel);
        }
        0
    }

    /// X and Y step a door's costume forward and back past the costumes other doors have,
    /// `mnCharSel_CostumeChange`.
    #[doc(alias = "mnCharSel_CostumeChange")]
    pub(super) fn charsel_costume_change(&mut self, door: usize, input: u32) {
        let d = self.game.charsel.doors[door];
        if d.sel_icon >= NO_ICON || d.sel_icon_prev >= NO_ICON {
            return;
        }
        let prev = d.costume;
        let n = crate::game::playback::costume_count(ICONS[usize::from(d.sel_icon)].ckind);
        if input & PAD_X != 0 {
            loop {
                let c = &mut self.game.charsel.doors[door].costume;
                *c = ((i32::from(*c) + 1) % i32::from(n)) as u8;
                if !self.costume_taken(door) {
                    break;
                }
            }
        } else if input & PAD_Y != 0 {
            loop {
                let c = &mut self.game.charsel.doors[door].costume;
                if *c != 0 {
                    *c -= 1;
                } else {
                    *c = n.wrapping_sub(1);
                }
                if !self.costume_taken(door) {
                    break;
                }
            }
        }
        if prev != self.game.charsel.doors[door].costume {
            self.charsel_door(door as u8);
            self.lb_audio_call(2);
        }
    }

    /// `fn_80262648`, a door's coin: hidden for a closed door or no character; its color by
    /// its kind, team or port (redone every 40 frames); placed, pushed apart from the other
    /// placed coins (with the random seed's jitter) and kept on its icon, else following the
    /// hand that holds it; and moving toward where it should be.
    #[doc(alias = "fn_80262648")]
    pub fn charsel_token_proc(&mut self, gobj: GObjId) {
        let UserData::CssToken(me) = self
            .game
            .world
            .gobj(gobj)
            .user_data
            .expect("a coin's user data")
        else {
            panic!("a coin without its user data")
        };
        let me = usize::from(me);
        let j = self.css_gobj_jobj(gobj);
        let tok = self.game.charsel.tokens[me].expect("a coin");
        let prev_color = tok.color;
        let n_doors = self.css_door_count();
        let door = usize::from(tok.door);
        let ds = self.game.charsel.doors[door];
        if ds.p_kind == PKIND_NONE || ds.sel_icon >= NO_ICON {
            jobj::set_flags_all(&mut self.game.objs.jobjs, j, HIDDEN);
            return;
        }
        let color = if self.game.charsel.doors_used == 1 {
            if door != 0 {
                8
            } else {
                self.game.charsel.port0 as u8
            }
        } else if self.css().vs.start.rules.is_teams == 0 {
            if ds.p_kind == PKIND_CPU {
                8
            } else {
                door as u8
            }
        } else if ds.p_kind == PKIND_CPU {
            TEAM_COLOR[usize::from(ds.team)] + 4
        } else {
            TEAM_COLOR[usize::from(ds.team)]
        };
        let t = self.game.charsel.tokens[me].as_mut().expect("a coin");
        t.color = color;
        jobj::clear_flags_all(&mut self.game.objs.jobjs, j, HIDDEN);
        t.timer = t.timer.wrapping_add(1);
        if prev_color != color || t.timer > 0x27 {
            let frame = if color < 4 {
                if self.game.charsel.doors_used == 1 {
                    f64::from(i32::from(self.game.charsel.port0).wrapping_mul(4) as f32)
                } else {
                    f64::from((i32::from(tok.door) * 4) as f32)
                }
            } else {
                16.0
            };
            let n = self.css_node(j, 4);
            self.css_req(n, TOBJ_MASK, mem(frame));
            self.css_anim_all(n);
            self.css_stop(n, TOBJ_MASK);
            let n = self.css_node(j, 3);
            self.css_req(n, MOBJ_MASK, f32::from(u16::from(color) * 0x28));
            self.game.charsel.tokens[me].as_mut().expect("a coin").timer = 0;
        }
        let held = self.game.charsel.tokens[me].expect("a coin").held_by;
        if held == 0 {
            for _ in 0..0x14 {
                for k in 0..n_doors {
                    let other = self.game.charsel.tokens[k].expect("a coin");
                    let od = self.game.charsel.doors[k];
                    if k == door
                        || other.held_by != 0
                        || od.p_kind == PKIND_NONE
                        || od.sel_icon >= NO_ICON
                    {
                        continue;
                    }
                    let t = self.game.charsel.tokens[me].expect("a coin");
                    let dx = loop {
                        let r = reg(self.game.objs.rng.draw_f32());
                        let dx = fp::fmadds(
                            reg(0.1),
                            fp::fsubs(r, 0.5),
                            fp::fsubs(reg(t.target_x), reg(other.target_x)),
                        );
                        if dx != 0.0 {
                            break dx;
                        }
                    };
                    let dy = loop {
                        let r = reg(self.game.objs.rng.draw_f32());
                        let dy = fp::fmadds(
                            reg(0.1),
                            fp::fsubs(r, 0.5),
                            fp::fsubs(reg(t.target_y), reg(other.target_y)),
                        );
                        if dy != 0.0 {
                            break dy;
                        }
                    };
                    if fp::fmadds(dx, dx, fp::fmuls(dy, dy)) < 8.0 {
                        let angle = coin_angle(dx, dy);
                        let t = self.game.charsel.tokens[me].as_mut().expect("a coin");
                        t.target_x = mem(fp::fnmsubs(
                            reg(0.01),
                            reg(crate::math::sin(angle)),
                            reg(t.target_x),
                        ));
                        t.target_y = mem(fp::fnmsubs(
                            reg(0.01),
                            reg(crate::math::cos(angle)),
                            reg(t.target_y),
                        ));
                    }
                }
                let sel = usize::from(self.game.charsel.doors[door].sel_icon);
                let ic = ICONS[sel];
                let mi = self.game.charsel.icons.get(sel).copied();
                let (bu, bd) = mi.map_or((0.0, 0.0), |i| (i.bound_u, i.bound_d));
                let t = self.game.charsel.tokens[me].as_mut().expect("a coin");
                if reg(t.target_x) < fp::fadds(reg(1.5), reg(ic.bound_l)) {
                    t.target_x = mem(fp::fadds(reg(t.target_x), reg(0.02)));
                    if reg(t.target_x) < reg(ic.bound_l) {
                        t.target_x = ic.bound_l;
                    }
                }
                let x = reg(t.target_x);
                if x > fp::fsubs(reg(ic.bound_r), reg(1.5)) {
                    t.target_x = mem(fp::fsubs(x, reg(0.02)));
                    if reg(t.target_x) > reg(ic.bound_r) {
                        t.target_x = ic.bound_r;
                    }
                }
                let y = reg(t.target_y);
                if y > fp::fsubs(reg(bu), reg(1.5)) {
                    t.target_y = mem(fp::fsubs(y, reg(0.02)));
                    if reg(t.target_y) > reg(bu) {
                        t.target_y = bu;
                    }
                }
                if reg(t.target_y) < fp::fadds(reg(1.5), reg(bd)) {
                    t.target_y = mem(fp::fadds(reg(t.target_y), reg(0.02)));
                    if reg(t.target_y) < reg(bd) {
                        t.target_y = bd;
                    }
                }
            }
        } else {
            let h = self.game.charsel.cursors[usize::from(held) - 1].expect("a hand");
            let t = self.game.charsel.tokens[me].as_mut().expect("a coin");
            t.target_x = mem(fp::fadds(reg(2.7), reg(h.x)));
            t.target_y = mem(fp::fadds(reg(-2.0), reg(h.y)));
        }
        let t = self.game.charsel.tokens[me].as_mut().expect("a coin");
        let tx = reg(t.target_x);
        let dx = fp::fsubs(reg(t.x), tx);
        let dy = fp::fsubs(reg(t.y), reg(t.target_y));
        if fp::fmadds(dx, dx, fp::fmuls(dy, dy)) < 4.0 {
            t.x = t.target_x;
            t.y = t.target_y;
        } else {
            let angle = coin_angle(dx, dy);
            t.x = mem(fp::fmadds(3.0, reg(crate::math::sin(angle)), reg(t.x)));
            t.y = mem(fp::fmadds(3.0, reg(crate::math::cos(angle)), reg(t.y)));
        }
        let (x, y) = (t.x, t.y);
        self.css_translate_x(j, x);
        self.css_translate_y(j, y);
        self.css_translate_z(j, 1.0);
        self.css_anim_all(j);
    }

    /// `fn_80262F44`, "Ready to fight": shown once two or more doors are in with a character
    /// each (in teams, on two teams), no hand holding a coin and no name list open; Start
    /// then (after the entry's wait) asks for the stage select, rumbling the humans'
    /// controllers, or plays the error sound if it isn't shown.
    #[doc(alias = "fn_80262F44")]
    pub fn charsel_press_start_proc(&mut self, gobj: GObjId) {
        let j = self.css_gobj_jobj(gobj);
        if self.css().match_type == VS_CAMERA {
            let child = self
                .game
                .charsel
                .debug_camera
                .and_then(|d| self.game.objs.jobjs[d].child);
            if self.game.pads.copy[3].err != 0 {
                jobj::clear_flags_all(&mut self.game.objs.jobjs, child, HIDDEN);
                self.charsel_ready_hide(j);
                return self.charsel_start_press();
            }
            jobj::set_flags_all(&mut self.game.objs.jobjs, child, HIDDEN);
        }
        if self.game.charsel.doors_used == 1 {
            super::super::super::code::unported_call(
                "fn_80262F44's one-door screen (the 1P modes', milestone 6)",
            );
        }
        let n = usize::from(self.game.charsel.doors_used);
        let d = self.game.charsel.doors;
        let mut valid = 0;
        let mut ok = true;
        for door in d.iter().take(n) {
            if door.p_kind != PKIND_NONE {
                if door.sel_icon >= NO_ICON {
                    ok = false;
                    break;
                }
                valid += 1;
            }
        }
        if ok && valid >= 2 {
            if self.css().vs.start.rules.is_teams == 1 {
                let two_teams = (0..n - 1).any(|i| {
                    d[i].p_kind != PKIND_NONE
                        && (i..n).any(|k| d[k].p_kind != PKIND_NONE && d[i].team != d[k].team)
                });
                ok = two_teams;
            }
            if ok {
                for i in 0..n {
                    let held = self.game.charsel.cursors[i].expect("a hand").state == 1;
                    if held || self.game.charsel.tags[i].expect("a tag").state != 0 {
                        ok = false;
                        break;
                    }
                }
            }
        } else {
            ok = false;
        }
        if ok {
            let r = self.game.charsel.ready;
            if r == 0 {
                self.css_req(j, ALL_MASK, 0.0);
            } else if r > 100 {
                self.css_req(j, ALL_MASK, 10.0);
                self.game.charsel.ready = 10;
            }
            let c = &mut self.game.charsel;
            c.ready = c.ready.wrapping_add(1);
            jobj::clear_flags_all(&mut self.game.objs.jobjs, j, HIDDEN);
            self.css_anim_all(j);
        } else {
            self.charsel_ready_hide(j);
        }
        self.charsel_start_press();
    }

    fn charsel_ready_hide(&mut self, j: Option<JObjId>) {
        let c = &mut self.game.charsel;
        c.ready = 0;
        c.start_wait = 10;
        jobj::set_flags_all(&mut self.game.objs.jobjs, j, HIDDEN);
    }

    /// Start pressed on any connected hand's controller, once the wait is over.
    fn charsel_start_press(&mut self) {
        let mut trigger = 0u32;
        for i in 0..usize::from(self.game.charsel.doors_used) {
            if self.game.charsel.cursors[i].expect("a hand").state != 3 {
                trigger |= self.game.pads.copy[i].trigger;
            }
        }
        if self.game.charsel.start_wait != 0 || trigger & PAD_START == 0 {
            return;
        }
        if self.game.charsel.ready != 0 {
            let c = &mut self.game.charsel;
            c.pending = 1;
            c.start_wait = 0xFF;
            for i in 0..usize::from(c.doors_used) {
                let tag = self.css().vs.start.players[i].nametag;
                if self.game.charsel.doors[i].p_kind == PKIND_HUMAN
                    && self.rumble_enabled_for(i, tag)
                {
                    self.rumble_start(i as u8, 0, 0xB, 0x1E);
                }
            }
        } else {
            self.lb_audio_call(3);
        }
    }

    /// `fn_802633B0`, a name tag: opening its list (1, 2), the list scrolled with the hand
    /// and a name picked or "NAME ENTRY" (3), closing it (4, 5).
    #[doc(alias = "fn_802633B0")]
    pub fn charsel_tag_proc(&mut self, gobj: GObjId) {
        let UserData::CssTag(me) = self
            .game
            .world
            .gobj(gobj)
            .user_data
            .expect("a tag's user data")
        else {
            panic!("a tag without its user data")
        };
        let me = usize::from(me);
        let tag = self.game.charsel.tags[me].expect("a tag");
        assert!(
            self.game.charsel.doors_used != 1,
            "the 1P modes' name tag (milestone 6)"
        );
        let port = usize::from(tag.port);
        let num_entries = i32::from(tag.next_tag) - if tag.next_tag > NO_TAG { 8 } else { 7 };
        let list = self.menu_node(TAGS[port].list);
        let trigger = self.game.pads.copy[port].trigger;
        match tag.state {
            1 => {
                self.charsel_ko_stars(port, 1);
                let sel = self.game.charsel.doors[port].sel_icon;
                let name = if sel < NO_ICON {
                    self.icon_name(sel)
                } else {
                    NAME_CANCEL
                };
                self.game
                    .sis
                    .replace_string(self.data, tag.name_list, 0, Some(name));
                self.game.sis.set_color(self.data, tag.text, 0, GRAY);
                self.css_req(list, ALL_MASK, 0.0);
                let t = self.game.charsel.tags[me].as_mut().expect("a tag");
                t.state = 2;
                t.timer = 0;
                self.lb_audio_call(2);
            }
            2 => {
                let t = self.game.charsel.tags[me].as_mut().expect("a tag");
                t.timer += 1;
                if t.timer as u32 > 0xA {
                    t.state = 3;
                    let nl = t.name_list;
                    self.css_stop(list, ALL_MASK);
                    self.game.sis.text_mut(nl).hidden = 0;
                }
            }
            3 => self.charsel_tag_list(me, num_entries, trigger),
            4 => {
                self.css_req(list, ALL_MASK, 20.0);
                self.game.sis.text_mut(tag.name_list).hidden = 1;
                if self.game.game_data.x1850.handicap != 0 && self.game.charsel.doors_used == 4 {
                    let h = self.handicap_value(port);
                    let menu = self.game.charsel.menu;
                    let j = self.css_animate(menu, DOORS[port].cpu_slider, TOBJ_MASK, h as f32);
                    let h = self.handicap_value(port);
                    let x = mem(fp::fmuls(reg(1.25), f64::from((h - 1) as f32)));
                    self.css_translate_x(j, x);
                }
                let t = self.game.charsel.tags[me].as_mut().expect("a tag");
                t.state = 5;
                t.timer = 0;
            }
            5 => {
                let t = self.game.charsel.tags[me].as_mut().expect("a tag");
                t.timer += 1;
                if t.timer as u32 > 0xA {
                    self.charsel_ko_stars(port, 2);
                    self.game.sis.set_color(self.data, tag.text, 0, WHITE);
                    self.game.charsel.tags[me].as_mut().expect("a tag").state = 0;
                }
            }
            _ => {}
        }
    }

    /// An open name list (`fn_802633B0`'s state 3): scrolled by the hand pushing past its
    /// ends, a page at a time, its arrows, its rows' names (gray where another port has
    /// one), and A on a row: the character's name back, a new name, or a name tag.
    fn charsel_tag_list(&mut self, me: usize, num_entries: i32, trigger: u32) {
        let tag = self.game.charsel.tags[me].expect("a tag");
        let port = usize::from(tag.port);
        let player = port;
        let vel = reg(tag.scroll_amt);
        if vel != 0.0 {
            let t = self.game.charsel.tags[me].as_mut().expect("a tag");
            t.scroll_force = mem(fp::fmadds(10.0, vel, if vel < 0.0 { -1.0 } else { 1.0 }));
        }
        let t = self.game.charsel.tags[me].as_mut().expect("a tag");
        if reg(t.scroll_force) != 0.0 {
            let pos = reg(t.scroll);
            let new_pos = fp::fadds(pos, reg(t.scroll_force));
            let page = fp::fctiwz(fp::fdivs(pos, 32.0));
            t.scroll = mem(new_pos);
            let new_page = fp::fctiwz(fp::fdivs(reg(t.scroll), 32.0));
            if reg(t.scroll) > 0.0 {
                t.scroll = 0.0;
            }
            let bottom = fp::fmuls(fp::fneg(32.0), fp::frsp(f64::from(num_entries)));
            if t.next_tag > 7 {
                if reg(t.scroll) < bottom {
                    t.scroll = mem(bottom);
                }
            } else {
                t.scroll = 0.0;
            }
            if reg(t.scroll_amt) == 0.0 {
                if page < new_page {
                    t.scroll = mem(fp::fmuls(32.0, fp::frsp(f64::from(page))));
                    t.scroll_force = 0.0;
                } else if page > new_page {
                    t.scroll = mem(fp::fmuls(32.0, fp::frsp(f64::from(new_page))));
                    t.scroll_force = 0.0;
                }
            }
        }
        let tag = self.game.charsel.tags[me].expect("a tag");
        let up = self.menu_node(TAGS[port].arrow_up);
        self.set_hidden(up, reg(tag.scroll) == 0.0);
        let down = self.menu_node(TAGS[port].arrow_down);
        if tag.next_tag > 7 {
            let bottom = fp::fmuls(fp::fneg(32.0), fp::frsp(f64::from(num_entries)));
            self.set_hidden(down, reg(tag.scroll) == bottom);
        } else {
            jobj::set_flags(&mut self.game.objs.jobjs, down, HIDDEN);
        }
        let s = reg(tag.scroll);
        self.game
            .sis
            .set_position(self.data, tag.name_list, 0, 0.0, s);
        let y = fp::fmadds(32.0, fp::frsp(f64::from(tag.next_tag)), s);
        self.game
            .sis
            .set_position(self.data, tag.name_list, 1, 0.0, y);
        let scroll_i = fp::fctiwz(s);
        let page_off = scroll_i / 32 + 1;
        for j in 0..9 {
            let row = j - page_off;
            if row < 0 {
                self.game
                    .sis
                    .replace_string(self.data, tag.name_list, j + 2, None);
            } else {
                let name = self.game.name_text(row as u8);
                self.game
                    .sis
                    .replace_string(self.data, tag.name_list, j + 2, name.as_deref());
                self.game
                    .sis
                    .set_color(self.data, tag.name_list, j + 2, WHITE);
                for p2 in 0..usize::from(self.game.charsel.doors_used) {
                    if p2 != port && i32::from(self.css().vs.start.players[p2].nametag) == row {
                        self.game
                            .sis
                            .set_color(self.data, tag.name_list, j + 2, GRAY);
                        break;
                    }
                }
            }
            let y = fp::fmadds(
                32.0,
                fp::frsp(f64::from(j + 1)),
                fp::frsp(f64::from(scroll_i % 32 - 0x20)),
            );
            self.game
                .sis
                .set_position(self.data, tag.name_list, j + 2, 10.0, y);
        }
        if trigger & PAD_A != 0 {
            let origin = self.menu_node(TAGS[port].list_joint);
            let o = self.css_world(origin);
            let hand_y = self.game.charsel.cursors[port].expect("a hand").y;
            let cursor_row = fp::fmsubs(
                0.5,
                fp::fadds(reg(0.8), fp::fsubs(reg(o.y), reg(hand_y))),
                fp::fdivs(reg(tag.scroll), 32.0),
            );
            let row = fp::fctiwz(cursor_row);
            if row == 0 {
                let sel = self.game.charsel.doors[port].sel_icon;
                let name = self.icon_name_or_none(sel);
                self.game.sis.replace_string(self.data, tag.text, 0, name);
                let t = self.game.sis.text_mut(tag.text);
                t.default_kerning = 1;
                t.hidden = u8::from(sel >= NO_ICON);
                if self.css().vs.start.players[player].nametag != NO_TAG
                    && counts_kos(self.css().match_type)
                {
                    self.css_ko_counts()[port] = 0;
                }
                self.css_start().players[player].nametag = NO_TAG;
                let t = self.game.charsel.tags[me].as_mut().expect("a tag");
                t.use_tag = 0;
                t.state = 4;
                self.lb_audio_call(2);
            } else if i32::from(tag.next_tag) == row {
                if !self.game.name_list_full() {
                    let c = &mut self.game.charsel;
                    c.new_tag_port = tag.port as i8;
                    c.pending = 4;
                    for k in 0..usize::from(c.doors_used) {
                        if self.game.charsel.cursors[k].expect("a hand").state == 1
                            && self.charsel_coin_back(k as u8) == 0
                        {
                            self.charsel_door(k as u8);
                            self.game.charsel.cursors[k].as_mut().expect("a hand").state = 2;
                        }
                    }
                    return;
                }
            } else if i32::from(tag.next_tag) > row {
                let taken = (0..usize::from(self.game.charsel.doors_used)).any(|p| {
                    p != player && i32::from(self.css().vs.start.players[p].nametag) == row - 1
                });
                if !taken {
                    let name = self.game.name_text((row - 1) as u8);
                    self.game
                        .sis
                        .replace_string(self.data, tag.text, 0, name.as_deref());
                    let t = self.game.sis.text_mut(tag.text);
                    t.default_kerning = 0;
                    t.hidden = 0;
                    if i32::from(self.css().vs.start.players[player].nametag) != row - 1
                        && counts_kos(self.css().match_type)
                    {
                        self.css_ko_counts()[port] = 0;
                    }
                    self.css_start().players[player].nametag = (row - 1) as u8;
                    let t = self.game.charsel.tags[me].as_mut().expect("a tag");
                    t.use_tag = 1;
                    t.state = 4;
                    self.lb_audio_call(2);
                }
            }
        }
        if trigger & PAD_B != 0 {
            self.game.charsel.tags[me].as_mut().expect("a tag").state = 4;
            self.lb_audio_call(0);
        }
    }

    /// An icon's name for a name box, none past the icons (the original hands
    /// `gm_80160980` `icons[0x19].char_kind`, 0: Captain Falcon's name, which a hidden text
    /// then holds).
    fn icon_name_or_none(&self, sel: u8) -> Option<&'static [u8]> {
        if sel < NO_ICON {
            Some(self.icon_name(sel))
        } else {
            self.game.character_name(ICONS[usize::from(sel)].ckind)
        }
    }

    /// `mnCharSel_Scene_OnFrame`: the frame counted; while nothing is pending (or Start was
    /// pressed), the preload cache's game cache set to the characters the doors have, and
    /// refreshed; L, R and Start back to the main menu; the Start wait; then the pending
    /// change: on to the stage select (unless a coin is up), back to the menu, the rules,
    /// or the name entry.
    #[doc(alias = "mnCharSel_Scene_OnFrame")]
    pub fn charsel_on_frame(&mut self) {
        let c = &mut self.game.charsel;
        c.frame = c.frame.wrapping_add(1);
        if c.pending <= 1 {
            assert!(c.doors_used != 1, "the 1P modes' game cache (milestone 6)");
            let n = if self.css().match_type == VS_CAMERA {
                3
            } else {
                4
            };
            for i in 0..n {
                let p = &self.css().vs.start.players[i];
                let (slot, ckind, color) = (p.slot_type, p.ckind, p.color);
                let up = self.game.charsel.tokens[i].expect("a coin").held_by != 0;
                let e = &mut self.dvd_mut().cache.scene.game_cache.entries[i];
                if (slot == PKIND_HUMAN || slot == PKIND_CPU) && !up {
                    e.char_id = i32::from(ckind);
                    e.color = color;
                } else {
                    e.char_id = i32::from(CHKIND_NONE);
                }
            }
            self.preload_refresh();
        }
        if self.game.buttons_triggered(4) & super::super::main::PAD_LR_START != 0 {
            self.lb_audio_call(0);
            self.rumble_stop_all();
            self.free_gobj_links(1, 8);
            self.sis_reset();
            self.game.game_data.x1850.force_main_menu = 1;
            self.change_game_mode_after_scene(super::super::super::gm::machine::GM_MENU);
            self.game.end_scene();
            return;
        }
        let c = &mut self.game.charsel;
        if c.start_wait != 0 {
            c.start_wait -= 1;
        }
        match c.pending {
            1 => {
                for i in 0..4 {
                    let up = self.game.charsel.tokens[0].expect("a coin").held_by != 0;
                    if self.game.charsel.doors[i].p_kind != PKIND_NONE && up {
                        self.game.charsel.pending = 0;
                    }
                }
                if self.game.charsel.pending != 0 {
                    self.game.end_scene();
                    if self.game.charsel.doors_used == 4 {
                        self.sfx_play_channel(0x147, 6);
                    }
                    self.lb_audio_call(1);
                }
            }
            2 => {
                self.game.end_scene();
                self.lb_audio_call(0);
            }
            // The rules over the screen (`mn_80231804` with `MnExtAll`, the automatic handicap
            // allowed), the screen's GObjs and texts let go first; the screen waits (5).
            3 => {
                self.free_gobj_links(1, 8);
                self.sis_reset();
                let ext = self.game.charsel.archives[1].expect("MnExtAll").0;
                self.rules_from_screen(ext, 1);
                self.game.charsel.pending = 5;
                self.lb_audio_call(1);
            }
            4 => super::super::super::code::unported_call(
                "mnNameNew_EnterFromMnCharSel (the name entry, task 8)",
            ),
            _ => {}
        }
    }

    /// `mnCharSel_Scene_OnExit`: the texts freed, the archives let go, the pending change in
    /// the `CSSData`; on to the stage select, the menus' sound banks swapped for the
    /// characters' (bank 4).
    #[doc(alias = "mnCharSel_Scene_OnExit")]
    pub fn charsel_on_exit(&mut self) {
        self.sis_close_all();
        for a in &mut self.game.charsel.archives.clone() {
            if let Some((_, file, archive)) = a.take() {
                self.free_archive_blocks(file, archive);
            }
        }
        self.game.charsel.archives = [None; 2];
        let pending = self.game.charsel.pending;
        self.css_mut().pending_scene_change = pending;
        if pending != 0 {
            return;
        }
        assert!(
            self.game.charsel.doors_used != 1,
            "the 1P modes' banks (milestone 6)"
        );
        let n = if self.css().match_type == VS_CAMERA {
            3
        } else {
            4
        };
        let mut mask = 0u64;
        for i in 0..n {
            let p = &self.css().vs.start.players[i];
            let (slot, ckind) = (p.slot_type, p.ckind);
            let up = self.charsel_exit_coin_up(i);
            if (slot == PKIND_HUMAN || slot == PKIND_CPU) && !up {
                mask |= crate::lb::audio::character_banks(i32::from(ckind));
            }
        }
        self.game.audio.lb.unwant_banks(0x14);
        self.game.audio.lb.want_banks(4, mask);
        self.game.audio.lb_start_bank_loads(self.data);
    }
}

impl Running<'_> {
    /// Whether door `i`'s coin is held, as the scene's exit reads it (`mnCharSel_804A0BD0[i]
    /// ->x5`): after L, R and Start freed the coins (`mn_8022F138` over the process links),
    /// the table still points at their blocks, and the exit reads the byte the freed block
    /// kept (docs/melee-bugs.md). Nothing allocates between the free and the exit, in the
    /// same tick; a block the heap has handed out again stops.
    fn charsel_exit_coin_up(&mut self, i: usize) -> bool {
        if let Some(t) = self.game.charsel.tokens[i] {
            return t.held_by != 0;
        }
        let (addr, held_by) = self.game.charsel.left_tokens[i].expect("a coin");
        let cell = addr - crate::hsd::heap::HEADER;
        let reused = crate::hsd::heap::with(|h| {
            h.os.allocated
                .iter()
                .any(|&(c, size)| c <= cell && cell < c + size)
        })
        .unwrap_or(false);
        if reused {
            crate::game::code::unported_call(
                "the character select's exit reading a freed coin's block the heap gave out again",
            );
        }
        held_by != 0
    }
}

/// The angle a coin moves along to close `(dx, dy)`: a quarter turn when `dy` is about 0,
/// else from `atanf` (the token process's two copies).
fn coin_angle(dx: f64, dy: f64) -> f32 {
    if dy < fp::frsp(0.01) && dy > fp::fneg(fp::frsp(0.01)) {
        mem(fp::frsp(if dx < 0.0 {
            core::f64::consts::FRAC_PI_2
        } else {
            -core::f64::consts::FRAC_PI_2
        }))
    } else {
        let a = reg(crate::math::atan(mem(fp::fdivs(dx, dy))));
        mem(fp::frsp(fp::fadd(
            if dy < 0.0 { 0.0 } else { core::f64::consts::PI },
            a,
        )))
    }
}
