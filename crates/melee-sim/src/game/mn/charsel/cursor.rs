//! A port's hand on the select screen (`mnCharSel_CursorThink`): moved by its control stick,
//! hidden while its controller is out, B held to leave, and A, B, X and Y on the buttons,
//! coins, icons, name boxes and sliders. Its float steps follow the machine code (ssbm-rs
//! transliterates the function for its fused multiply-adds).

// NaN takes the original's branches, which the negated comparisons spell out.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use super::*;

/// `mnCharSel_CursorThink`.
pub const CURSOR_THINK: u32 = 0x8026_02A0;

/// The doors' toggle and team boxes' heights (`cursor.y` against the doubles the machine
/// code loads, `@2330` to `@2334`).
const TOGGLE_TOP: f64 = 0.200_000_095_367_431_46;
const TOGGLE_BOTTOM: f64 = -4.600_000_095_367_432;
const TEAM_TOP: f64 = -0.999_999_904_632_568_3;
const TEAM_BOTTOM: f64 = -5.800_000_095_367_432;

impl Running<'_> {
    fn cursor(&self, port: usize) -> Cursor {
        self.game.charsel.cursors[port].expect("a hand")
    }

    fn cursor_mut(&mut self, port: usize) -> &mut Cursor {
        self.game.charsel.cursors[port].as_mut().expect("a hand")
    }

    fn token_mut(&mut self, door: usize) -> &mut Token {
        self.game.charsel.tokens[door].as_mut().expect("a coin")
    }

    /// The stick's push past its dead zone as the hand's motion (the inline `getStickDelta`):
    /// none under a squared length of 200, else what's past it along the stick's angle.
    fn stick_delta(&self, port: usize) -> (f64, f64) {
        let p = &self.game.pads.copy[port];
        let x = f64::from(p.stick_x as f32);
        let y = f64::from(p.stick_y as f32);
        // The machine code squares y first and fuses x's.
        let mag = fp::fmadds(x, x, fp::fmuls(y, y));
        if mag < reg(200.0) {
            return (0.0, 0.0);
        }
        let adj = fp::fsubs(mag, reg(200.0));
        let angle = crate::math::atan2(mem(x), mem(y));
        let dx = fp::fmuls(adj, reg(crate::math::sin(angle)));
        let dy = fp::fmuls(adj, reg(crate::math::cos(angle)));
        (dx, dy)
    }

    /// A door's coin taken by `port`'s hand: the door without a character (its icon 0xD),
    /// the coin drawn just before the hand.
    fn take_coin(&mut self, port: usize, door: usize) {
        self.token_mut(door).held_by = port as u8 + 1;
        let c = self.cursor_mut(port);
        c.state = 1;
        c.held = door as u8;
        self.game.charsel.doors[door].sel_icon = 0xD;
        let tg = self.game.charsel.tokens[door].expect("a coin").gobj;
        let hg = self.cursor(port).gobj;
        self.game.world.gx_relink_before(tg, hg);
        self.sfx_play(0xB7, 0x7F, 0x40);
    }

    /// `mnCharSel_CursorThink`, a hand's process.
    #[doc(alias = "mnCharSel_CursorThink")]
    pub fn charsel_cursor_think(&mut self, gobj: GObjId) {
        let UserData::CssCursor(me) = self
            .game
            .world
            .gobj(gobj)
            .user_data
            .expect("a hand's user data")
        else {
            panic!("a hand without its user data")
        };
        let me = usize::from(me);
        let j = self.css_gobj_jobj(gobj);
        let n_doors = self.css_door_count();
        if self.game.charsel.pending == 0 && !self.cursor_input(me, j, n_doors) {
            return;
        }
        self.cursor_display(me, j);
    }

    /// The hand's frame while nothing is pending; false when it's hidden for its controller
    /// being out, which ends the process.
    fn cursor_input(&mut self, me: usize, j: Option<JObjId>, n_doors: usize) -> bool {
        self.cursor_mut(me).hover = 0;
        assert!(
            self.game.charsel.doors_used != 1,
            "the 1P modes' hand (milestone 6)"
        );
        let port = usize::from(self.cursor(me).port);
        let trigger = self.game.pads.copy[port].trigger;
        let buttons = self.game.pads.copy[port].button;
        let (dx, dy) = self.stick_delta(port);
        if self.game.pads.copy[port].err != 0 {
            self.cursor_unplugged(me, port);
            let c = self.cursor_mut(me);
            c.state = 3;
            jobj::set_flags_all(&mut self.game.objs.jobjs, j, HIDDEN);
            return false;
        }
        if self.cursor(me).state == 3 {
            self.cursor_mut(me).state = 2;
            jobj::clear_flags_all(&mut self.game.objs.jobjs, j, HIDDEN);
        }
        let bit = PORT_BIT[port];
        if buttons & PAD_B != 0 {
            if self.game.charsel.b_released & bit != 0 {
                let c = self.cursor_mut(me);
                c.b_timer = c.b_timer.wrapping_add(1);
                if c.b_timer > 0x1E {
                    self.game.charsel.pending = 2;
                }
            } else {
                self.cursor_mut(me).b_timer = 0;
            }
        } else {
            self.game.charsel.b_released |= bit;
            self.cursor_mut(me).b_timer = 0;
        }
        let c = self.cursor_mut(me);
        c.x = mem(fp::fmadds(reg(0.0002), dx, reg(c.x)));
        c.y = mem(fp::fmadds(reg(0.0002), dy, reg(c.y)));
        let port = usize::from(c.port);
        self.game.charsel.tags[port]
            .as_mut()
            .expect("a tag")
            .scroll_amt = 0.0;
        let tag = self.game.charsel.tags[port].expect("a tag");
        if tag.state != 0 {
            let lj = self.menu_node(TAGS[port].list_joint);
            let p = self.css_world(lj);
            let c = self.cursor_mut(me);
            let top = fp::fadds(reg(0.8), reg(p.y));
            if top < reg(c.y) {
                let amt = mem(fp::fsubs(reg(c.y), top));
                c.y = mem(top);
                self.game.charsel.tags[port]
                    .as_mut()
                    .expect("a tag")
                    .scroll_amt = amt;
            }
            let c = self.cursor_mut(me);
            let bottom = fp::fadds(reg(-14.8), reg(p.y));
            if bottom > reg(c.y) {
                let amt = mem(fp::fsubs(reg(c.y), bottom));
                c.y = mem(bottom);
                self.game.charsel.tags[port]
                    .as_mut()
                    .expect("a tag")
                    .scroll_amt = amt;
            }
            let c = self.cursor_mut(me);
            let right = fp::fadds(reg(4.3), reg(p.x));
            if right < reg(c.x) {
                c.x = mem(right);
            }
            let left = fp::fadds(reg(-5.3), reg(p.x));
            if left > reg(c.x) {
                c.x = mem(left);
            }
        } else {
            let c = self.cursor_mut(me);
            if reg(25.0) < reg(c.y) {
                c.y = 25.0;
            }
            if reg(-22.0) > reg(c.y) {
                c.y = -22.0;
            }
            if reg(26.0) < reg(c.x) {
                c.x = 26.0;
            }
            if reg(-35.0) > reg(c.x) {
                c.x = -35.0;
            }
        }
        let port = usize::from(self.cursor(me).port);
        if self.game.charsel.tags[port].expect("a tag").state != 0 {
            return true;
        }
        if self.cursor(me).state == 1 {
            self.cursor_holding(me, trigger);
            return true;
        }
        let c = self.cursor(me);
        if reg(c.x) > reg(17.3) && reg(c.y) > reg(22.0) {
            self.cursor_mut(me).hover = 1;
            if trigger & PAD_A != 0 {
                self.game.charsel.pending = 2;
                return true;
            }
        }
        let match_type = self.css().match_type;
        if match_type != VS_STAMINA {
            let c = self.cursor(me);
            if reg(c.x) > reg(-17.0) && reg(c.x) < reg(15.0) && reg(c.y) > reg(22.0) {
                self.cursor_mut(me).hover = 1;
                if trigger & PAD_A != 0 {
                    self.game.charsel.pending = 3;
                    for k in 0..usize::from(self.game.charsel.doors_used) {
                        if self.cursor(k).state == 1 && self.charsel_coin_back(k as u8) == 0 {
                            self.charsel_door(k as u8);
                            self.cursor_mut(k).state = 2;
                        }
                    }
                    return false;
                }
            }
        }
        let c = self.cursor(me);
        if reg(c.x) < reg(-25.5) && reg(c.y) > reg(22.0) && match_type < REG_CLASSIC {
            self.cursor_mut(me).hover = 1;
            if trigger & PAD_A != 0 {
                self.lb_audio_call(2);
                let r = &mut self.css_start().rules;
                r.is_teams = (r.is_teams + 1) & 1;
                if self.css().vs.start.rules.is_teams == 0 {
                    for k in 0..usize::from(self.game.charsel.doors_used) {
                        if self.costume_taken(k) {
                            self.game.charsel.doors[k].costume = 0;
                            while self.costume_taken(k) {
                                let c = &mut self.game.charsel.doors[k].costume;
                                *c = c.wrapping_add(1);
                            }
                        }
                    }
                }
                self.charsel_mode_texture(match_type);
                return true;
            }
        }
        // `try_costume_change`.
        let p = usize::from(self.cursor(me).port);
        if self.game.charsel.tokens[p].expect("a coin").held_by == 0
            && self.css().vs.start.rules.is_teams == 0
            && self.game.charsel.doors[p].p_kind != PKIND_NONE
        {
            self.charsel_costume_change(p, trigger);
        }
        self.cursor_clicks(me, trigger, n_doors);
        true
    }

    /// A hand whose controller is out goes back below its door, letting go of what it holds
    /// (a coin back on its character, or a random one), its human door turned CPU, and its
    /// name list closing (`mnCharSel_CursorThink`'s `err` branch).
    fn cursor_unplugged(&mut self, me: usize, port: usize) {
        let c = self.cursor(me);
        if c.state == 3 {
            return;
        }
        let c = self.cursor_mut(me);
        c.x = mem(fp::fmsubs(15.0, f64::from(f32::from(c.port)), 31.0));
        c.y = -21.5;
        c.b_timer = 0;
        let c = self.cursor(me);
        if c.state == 1 {
            let door = c.held;
            if door <= 3 {
                self.cursor_mut(me).state = 2;
                if self.charsel_coin_back(door) != 0 {
                    self.charsel_random_pick(door, true);
                }
            } else if door <= 7 {
                self.game.charsel.doors[usize::from(door - 4)].hold_cpu_slider = 0;
            }
        } else {
            let d = self.game.charsel.doors[port];
            if d.sel_icon >= NO_ICON
                && d.p_kind != PKIND_NONE
                && self.charsel_coin_back(port as u8) != 0
            {
                self.charsel_random_pick(port as u8, true);
            }
        }
        if self.game.charsel.doors[port].p_kind == PKIND_HUMAN {
            self.game.charsel.doors[port].p_kind = PKIND_CPU;
            let p = &mut self.css_start().players[port];
            p.slot_type = PKIND_CPU;
            p.nametag = NO_TAG;
            self.game.charsel.tags[port]
                .as_mut()
                .expect("a tag")
                .use_tag = 0;
        }
        self.charsel_door(port as u8);
        let t = self.game.charsel.tags[port].as_mut().expect("a tag");
        if t.state != 0 {
            t.state = 4;
        }
        self.game.charsel.doors[port].hold_handicap_slider = 0;
    }

    /// The hand holding something (`x5` 1): a coin dropped below the icons goes back (its
    /// own door's empties), over an icon it shows that character and A places it, on a
    /// random spot A picks at random once every icon is out; a slider follows the hand.
    fn cursor_holding(&mut self, me: usize, trigger: u32) {
        let c = self.cursor(me);
        let door = c.held;
        let port = c.port;
        match door {
            0..=3 => {
                let d = usize::from(door);
                if reg(c.y) < reg(0.2) {
                    if port != door || self.game.charsel.doors[d].p_kind == PKIND_NONE {
                        self.charsel_coin_back(door);
                        self.sfx_play(0xB8, 0x7F, 0x40);
                    } else {
                        self.token_mut(d).held_by = 0;
                        self.game.charsel.doors[d].sel_icon = NO_ICON;
                        let player = self.door_player(door);
                        self.css_start().players[player].ckind = CHKIND_NONE;
                    }
                    self.charsel_door(door);
                    self.cursor_mut(me).state = 2;
                    return;
                }
                if trigger & PAD_A != 0 {
                    let m = self.game.charsel.tokens[d].expect("a coin");
                    let (x, y) = (reg(m.target_x), reg(m.target_y));
                    if y < reg(6.0)
                        && y > reg(-1.0)
                        && ((x > reg(-30.0) && x < reg(-24.4)) || (x > reg(24.4) && x < reg(30.2)))
                    {
                        let all = (0..25).all(|i| self.game.charsel.icons[i].state >= 2);
                        if all {
                            self.charsel_random_pick(door, false);
                            loop {
                                let sel = usize::from(self.game.charsel.doors[d].sel_icon);
                                let n = crate::game::playback::costume_count(ICONS[sel].ckind);
                                let c = self.game.objs.rng.below(i32::from(n)) as u8;
                                self.game.charsel.doors[d].costume = c;
                                if !self.costume_taken(d) {
                                    break;
                                }
                            }
                            self.charsel_door(door);
                            self.cursor_mut(me).state = 2;
                            self.sfx_play(0xB8, 0x7F, 0x40);
                            return;
                        }
                    }
                }
                let m = self.game.charsel.tokens[d].expect("a coin");
                let (x, y) = (reg(m.target_x), reg(m.target_y));
                let over = (0..25).find(|&i| {
                    let ic = self.game.charsel.icons[i];
                    x > reg(ICONS[i].bound_l)
                        && x < reg(ICONS[i].bound_r)
                        && y < reg(ic.bound_u)
                        && y > reg(ic.bound_d)
                        && ic.state >= 1
                });
                if let Some(i) = over {
                    self.game.charsel.doors[d].sel_icon = i as u8;
                    self.charsel_door(door);
                    if trigger & PAD_A != 0 {
                        let player = self.door_player(door);
                        let sel = usize::from(self.game.charsel.doors[d].sel_icon);
                        self.css_start().players[player].ckind = ICONS[sel].ckind as i8;
                        let joint = ICONS[i].joint_vs;
                        let n = self.menu_node(joint);
                        self.css_req(n, TOBJ_MASK, 10.0);
                        self.game.charsel.icons[i].anim_timer = 0xC;
                        self.token_mut(d).held_by = 0;
                        let tg = self.game.charsel.tokens[d].expect("a coin").gobj;
                        let last = usize::from(self.game.charsel.doors_used) - 1;
                        let hg = self.cursor(last).gobj;
                        self.game.world.gx_relink_before(tg, hg);
                        self.game.charsel.doors[d].selected_since_load = 1;
                        self.cursor_mut(me).state = 2;
                        let sel = usize::from(self.game.charsel.doors[d].sel_icon);
                        self.sfx_play_track(ICONS[sel].sfx, 0x7F, 0x40, sel as i32 + 0x8A);
                        self.preload_character_sfx(u32::from(ICONS[sel].ckind));
                        self.sfx_play(0xB8, 0x7F, 0x40);
                        return;
                    }
                } else {
                    self.charsel_door_icon(d, 0, true);
                    let tag = self.game.charsel.tags[d].expect("a tag");
                    if tag.use_tag == 0 {
                        self.game.sis.text_mut(tag.text).hidden = 1;
                    }
                    self.game.charsel.doors[d].sel_icon_prev = NO_ICON;
                    if trigger & PAD_A != 0 {
                        self.lb_audio_call(3);
                        return;
                    }
                }
                // `cancel_or_costume`.
                if trigger & PAD_B != 0 {
                    if self.charsel_coin_back(door) == 0 {
                        self.charsel_door(door);
                        self.cursor_mut(me).state = 2;
                    }
                } else {
                    self.charsel_costume_change(d, trigger);
                }
            }
            4..=7 => self.cursor_slider(me, usize::from(door - 4), trigger, false),
            8..=11 => self.cursor_slider(me, usize::from(door - 8), trigger, true),
            _ => {}
        }
    }

    /// A slider the hand holds (the inline `updateGrabbedSlider`): its knob under the hand
    /// along its 10 units, the level (or handicap) from where it is, and A lets go.
    fn cursor_slider(&mut self, me: usize, door: usize, trigger: u32, handicap: bool) {
        let joint = if handicap || self.game.game_data.x1850.handicap == 0 {
            DOORS[door].cpu_slider
        } else {
            DOORS[door].cpu_slider2
        };
        let s = self.menu_node(joint);
        let pos = self.css_world(s);
        let tx = self.game.objs.jobjs[s.expect("a slider")].translate.x;
        let base_x = fp::fsubs(reg(tx), reg(pos.x));
        let c = self.cursor(me);
        let mut x = mem(fp::fadds(fp::fsubs(reg(c.x), reg(-2.9)), base_x));
        if reg(x) < 0.0 {
            x = 0.0;
        }
        if reg(x) > reg(10.0) {
            x = 10.0;
        }
        let v = fp::fctiwz(fp::fmadds(reg(0.8), reg(x), reg(0.5))) + 1;
        let frame = if handicap {
            let p = &mut self.css_start().players[door];
            p.handicap = v as i8;
            f32::from(p.handicap as u8)
        } else {
            let p = &mut self.css_start().players[door];
            p.cpu_level = v as u8;
            f32::from(p.cpu_level)
        };
        self.css_req(s, TOBJ_MASK, frame);
        self.css_anim_all(s);
        self.css_stop(s, TOBJ_MASK);
        self.css_translate_x(s, x);
        let c = self.cursor_mut(me);
        c.x = mem(fp::fadds(reg(-2.9), fp::fsubs(reg(x), base_x)));
        c.y = mem(fp::fadds(reg(1.7), reg(pos.y)));
        if trigger & PAD_A != 0 {
            c.state = 2;
            if handicap {
                self.game.charsel.doors[door].hold_handicap_slider = 0;
            } else {
                self.game.charsel.doors[door].hold_cpu_slider = 0;
            }
            self.sfx_play(0xB7, 0x7F, 0x40);
        }
    }

    /// A on a door's buttons (`door_clicks`): its HMN/CPU/N/A toggle, its team, its sliders;
    /// the hand's own name box; the nearest coin a hand may take; B takes the hand's own
    /// coin; and a hand over the icons opens its closed door and takes its coin.
    fn cursor_clicks(&mut self, me: usize, trigger: u32, n_doors: usize) {
        let a = trigger & PAD_A != 0;
        let match_type = self.css().match_type;
        if a {
            for door in 0..usize::from(self.game.charsel.doors_used) {
                if door == 3 && match_type == VS_CAMERA {
                    continue;
                }
                let d = self.game.charsel.doors[door];
                if (d.hold_cpu_slider | d.hold_handicap_slider) == 0
                    && self.game.charsel.tokens[door].expect("a coin").held_by == 0
                    && self.cursor(door).state != 1
                {
                    if self.game.charsel.tags[door].expect("a tag").state != 0 {
                        continue;
                    }
                    let c = self.cursor(me);
                    let (cx, cy) = (reg(c.x), reg(c.y));
                    if cx > reg(DOORS[door].toggle_l)
                        && cx < reg(DOORS[door].toggle_r)
                        && cy < TOGGLE_TOP
                        && cy > TOGGLE_BOTTOM
                    {
                        self.cursor_mut(me).y = -2.2;
                        let dd = &mut self.game.charsel.doors[door];
                        dd.p_kind = dd.p_kind.wrapping_add(1);
                        match dd.p_kind {
                            2 => dd.p_kind = 3,
                            4 => {
                                dd.p_kind = if self.game.pads.copy[door].err != 0 {
                                    PKIND_CPU
                                } else {
                                    PKIND_HUMAN
                                };
                            }
                            _ => {}
                        }
                        let kind = self.game.charsel.doors[door].p_kind;
                        self.css_start().players[door].slot_type = kind;
                        if kind == PKIND_CPU {
                            self.css_start().players[door].nametag = NO_TAG;
                            self.game.charsel.tags[door]
                                .as_mut()
                                .expect("a tag")
                                .use_tag = 0;
                            if self.game.charsel.doors[door].selected_since_load == 0
                                && usize::from(self.cursor(me).port) != door
                            {
                                self.charsel_random_pick(door as u8, true);
                            }
                        }
                        self.charsel_door(door as u8);
                        self.lb_audio_call(2);
                        break;
                    }
                }
                let d = self.game.charsel.doors[door];
                if self.css().vs.start.rules.is_teams == 1 && d.p_kind != PKIND_NONE {
                    let c = self.cursor(me);
                    if reg(c.x) > reg(DOORS[door].team_l) && reg(c.x) < reg(DOORS[door].team_r) {
                        let cy = reg(c.y);
                        if cy < TEAM_TOP && cy > TEAM_BOTTOM {
                            self.cursor_mut(me).y = -3.4;
                            let dd = &mut self.game.charsel.doors[door];
                            dd.team = (dd.team + 1) % 3;
                            let team = dd.team;
                            self.css_start().players[door].team = team;
                            self.charsel_door(door as u8);
                            self.lb_audio_call(2);
                        }
                    }
                }
                let d = self.game.charsel.doors[door];
                if d.hold_cpu_slider == 0 && d.p_kind == PKIND_CPU {
                    let joint = if self.game.game_data.x1850.handicap != 0 {
                        DOORS[door].cpu_slider2
                    } else {
                        DOORS[door].cpu_slider
                    };
                    if self.cursor_grab(me, joint, door as u8 + 4) {
                        self.game.charsel.doors[door].hold_cpu_slider = 1;
                        self.sfx_play(0xB7, 0x7F, 0x40);
                    }
                }
                let d = self.game.charsel.doors[door];
                if d.hold_handicap_slider == 0 && self.game.game_data.x1850.handicap == 2 {
                    let pk = d.p_kind;
                    if pk != PKIND_NONE
                        && (pk == PKIND_CPU || door == usize::from(self.cursor(me).port))
                        && self.cursor_grab(me, DOORS[door].cpu_slider, door as u8 + 8)
                    {
                        self.game.charsel.doors[door].hold_handicap_slider = 1;
                        self.sfx_play(0xB7, 0x7F, 0x40);
                    }
                }
            }
            let port = usize::from(self.cursor(me).port);
            if (port != 3 || match_type != VS_CAMERA)
                && self.game.charsel.doors[port].p_kind == PKIND_HUMAN
            {
                let nj = self.menu_node(TAGS[port].name_joint);
                let p = self.css_world(nj);
                let c = self.cursor(me);
                let (cx, cy) = (reg(c.x), reg(c.y));
                if cx > fp::fsubs(reg(p.x), reg(4.7))
                    && cx < fp::fadds(reg(5.2), reg(p.x))
                    && cy < fp::fadds(reg(2.0), reg(p.y))
                    && cy > fp::fsubs(reg(p.y), reg(1.0))
                {
                    let lj = self.menu_node(TAGS[port].list_joint);
                    let p = self.css_world(lj);
                    let c = self.cursor_mut(me);
                    c.x = p.x;
                    c.y = mem(fp::fsubs(reg(p.y), reg(5.0)));
                    self.game.charsel.tags[port].as_mut().expect("a tag").state = 1;
                }
            }
            let mut closest = -1i32;
            let mut closest_dist = reg(9.0);
            for ci in 0..n_doors {
                let c = self.cursor(me);
                let cy = reg(c.y);
                if !(cy < reg(0.2)) && !(cy > reg(22.0)) {
                    let d = self.game.charsel.doors[ci];
                    if d.p_kind != PKIND_NONE
                        && d.sel_icon < NO_ICON
                        && (d.p_kind != PKIND_HUMAN || usize::from(c.port) == ci)
                    {
                        let m = self.game.charsel.tokens[ci].expect("a coin");
                        if m.held_by == 0 {
                            let ddx = fp::fadds(reg(3.8), fp::fsubs(reg(c.x), reg(m.target_x)));
                            let ddy = fp::fadds(reg(-2.6), fp::fsubs(cy, reg(m.target_y)));
                            let dist = fp::fmadds(ddx, ddx, fp::fmuls(ddy, ddy));
                            if dist < closest_dist {
                                closest_dist = dist;
                                closest = ci as i32;
                            }
                        }
                    }
                }
            }
            if closest >= 0 {
                let port = usize::from(self.cursor(me).port);
                let ci = closest as usize;
                self.take_coin(port, ci);
                let m = self.game.charsel.tokens[ci].expect("a coin");
                let c = self.cursor_mut(me);
                c.x = mem(fp::fsubs(reg(m.target_x), reg(2.7)));
                c.y = mem(fp::fsubs(reg(m.target_y), reg(-2.0)));
            }
        } else if trigger & PAD_B != 0 {
            let port = usize::from(self.cursor(me).port);
            let d = self.game.charsel.doors[port];
            if d.p_kind != PKIND_NONE && d.sel_icon < NO_ICON {
                let cy = reg(self.cursor(me).y);
                if cy > reg(0.2)
                    && cy < reg(22.0)
                    && self.game.charsel.tokens[port].expect("a coin").held_by == 0
                {
                    self.take_coin(port, port);
                }
            }
        }
        let c = self.cursor(me);
        let cy = reg(c.y);
        if cy > reg(0.2) && cy < reg(22.0) {
            let port = usize::from(c.port);
            if port != 3 || match_type != VS_CAMERA {
                if self.game.charsel.doors[port].p_kind == PKIND_NONE {
                    self.game.charsel.doors[port].p_kind = PKIND_HUMAN;
                    self.css_start().players[port].slot_type = PKIND_HUMAN;
                    self.charsel_door(port as u8);
                    self.lb_audio_call(2);
                }
                if self.game.charsel.doors[port].sel_icon >= NO_ICON {
                    self.take_coin(port, port);
                    let c = self.cursor(me);
                    let x = mem(fp::fadds(reg(2.7), reg(c.x)));
                    let y = mem(fp::fadds(reg(-2.0), reg(c.y)));
                    let t = self.token_mut(port);
                    t.x = x;
                    t.target_x = x;
                    t.y = y;
                    t.target_y = y;
                }
            }
        }
    }

    /// The hand grabs a slider's knob within its reach (a squared distance under 5 from the
    /// knob's grip), holding `held`.
    fn cursor_grab(&mut self, me: usize, joint: u8, held: u8) -> bool {
        let s = self.menu_node(joint);
        let p = self.css_world(s);
        let c = self.cursor(me);
        let gx = fp::fadds(reg(-2.9), reg(p.x));
        let gy = fp::fadds(reg(1.7), reg(p.y));
        let dx = fp::fsubs(reg(c.x), gx);
        let dy = fp::fsubs(reg(c.y), gy);
        if fp::fmadds(dx, dx, fp::fmuls(dy, dy)) < reg(5.0) {
            let c = self.cursor_mut(me);
            c.state = 1;
            c.held = held;
            c.x = mem(gx);
            c.y = mem(gy);
            return true;
        }
        false
    }

    /// The hand's display (the inline `updateCursorDisplay`): pointing or open by where it
    /// is, its color by its port (and team), flashing over a button, at its place.
    fn cursor_display(&mut self, me: usize, j: Option<JObjId>) {
        let c = self.cursor(me);
        if c.state != 1 {
            let y = reg(c.y);
            self.cursor_mut(me).state = if y < reg(0.2) || y > reg(22.0) { 0 } else { 2 };
        }
        let c = self.cursor(me);
        let s = self.css_node(j, 2);
        self.css_req(s, TOBJ_MASK, f32::from(c.state));
        self.css_anim_all(s);
        self.css_stop(s, TOBJ_MASK);
        let ch = &self.game.charsel;
        let mut color = if ch.doors_used == 1 {
            i32::from(ch.port0)
        } else if self.css().vs.start.rules.is_teams == 0 {
            i32::from(c.port)
        } else {
            i32::from(TEAM_COLOR[usize::from(ch.doors[usize::from(c.port)].team)])
        };
        let port = if ch.doors_used == 1 {
            if c.port != 0 {
                i32::from(ch.port1)
            } else {
                i32::from(ch.port0)
            }
        } else {
            i32::from(c.port)
        };
        if c.hover != 0 {
            color = (ch.frame & 3) as i32 + port * 4;
        } else {
            color += port * 4;
        }
        let k = self.css_node(j, 3);
        self.css_req(k, TOBJ_MASK, color as f32);
        self.css_anim_all(k);
        self.css_stop(k, TOBJ_MASK);
        self.css_translate_x(j, c.x);
        self.css_translate_y(j, c.y);
        self.css_translate_z(j, 0.0);
        self.css_anim_all(j);
    }
}
