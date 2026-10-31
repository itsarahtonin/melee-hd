//! The trophy gallery's two debug views (`toy.c`), reached only from the debug ROM's level
//! with a button held at the gallery's entry. Z: the parameters' view (`_Toy_8030B530`), where
//! the buttons held move, turn and scale the trophy and its stands, each change written into
//! `TyDatai`'s model table and shown in a debug text of the trophy's six parameters. L: the
//! fog view (`_Toy_8030E110`), where the frustum camera alone draws the trophy without its
//! stands, its light and a white fog that Y turns on and off.

use gekko_fp as fp;

use super::super::state::{Obj, Running};
use super::view::{dead_zone, int_f};
use crate::fpu::{mem, reg};
use crate::hsd::cobj::{self, CObjId};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::msl::Arg;
use crate::vec::Vec3;

/// `_Toy_8030B530`: the Z view's camera process.
pub const PROC_Z_VIEW: u32 = 0x8030_B530;
/// `_Toy_8030E110`: the L view's camera process.
pub const PROC_L_VIEW: u32 = 0x8030_E110;

const PAD_DLEFT: u32 = 0x1;
const PAD_DRIGHT: u32 = 0x2;
const PAD_DDOWN: u32 = 0x4;
const PAD_DUP: u32 = 0x8;
const PAD_Z: u32 = 0x10;
const PAD_R: u32 = 0x20;
const PAD_L: u32 = 0x40;
const PAD_A: u32 = 0x100;
const PAD_B: u32 = 0x200;
const PAD_X: u32 = 0x400;
const PAD_Y: u32 = 0x800;
const PAD_START: u32 = 0x1000;

/// The Z view's text's six lines (`"X   %3.2f\n..."`).
const PARAMS_FORMAT: &[u8] = b"X   %3.2f\nY   %3.2f\nZ   %3.2f\nMS  %3.2f\nSS  %3.2f\nMD  %3.2f";

/// `|v| > 0.8` on the float's bits with the sign cleared, as the code tests it.
fn past(v: f32) -> bool {
    reg(v.abs()) > reg(0.8)
}

impl Running<'_> {
    /// The Z view's text: erased, its cursor home, then the six parameters.
    pub(super) fn toy_z_print(&mut self, v: [f32; 6]) {
        let Some(t) = self.game.toys.gallery.debug_text else {
            return;
        };
        let s = crate::msl::vsprintf(PARAMS_FORMAT, &v.map(|x| Arg::Float(f64::from(x))));
        let text = &mut self.game.hud.devtext.pool[usize::from(t)];
        text.erase();
        text.set_cursor_xy(0, 0);
        text.print(&s);
    }

    /// `_Toy_80311788`: the Z view's text (id 1, at 540, 130, 10 by 7 cells), shown without
    /// a cursor or wrapping, blue, its cells 12 by 18, with the parameters of the trophy the
    /// gallery's zeroed blocks select (their order's first, trophy 0).
    #[doc(alias = "_Toy_80311788")]
    pub(super) fn toy_z_text(&mut self) {
        let t = self.game.hud.devtext.create(1, 0x21C, 0x82, 0xA, 7);
        self.game.toys.gallery.debug_text = t;
        let Some(t) = t else {
            // `OSReport("*** Couldn't Open Debug Window!")`.
            return;
        };
        self.game.hud.devtext.show(t);
        let text = &mut self.game.hud.devtext.pool[usize::from(t)];
        text.hide_cursor();
        text.no_wrap();
        text.set_bg_color([0x40, 0x80, 0xD0, 0x60]);
        text.set_scale(12.0, 18.0);
        let g = &self.game.toys.gallery;
        let trophy = i32::from(g.order[g.selected as usize]);
        let v = [0, 1, 2, 3, 4, 5].map(|f| self.toy_param(trophy, f));
        self.toy_z_print(v);
    }

    /// `_Toy_803062EC(trophy, field, v)`: one of a trophy's six parameters (its model's place,
    /// scales and turn) set to `v` in `TyDatai`'s English model table (`tyInitModelTbl`; the
    /// list's end if the trophy isn't in it), and the Z view's text of that entry.
    #[doc(alias = "_Toy_803062EC")]
    fn toy_param_edit(&mut self, trophy: i32, field: u32, v: f32) {
        let mut at = self.datai_sym("tyInitModelTbl");
        loop {
            let id = self.data.u32(at) as i32;
            if id == -1 || id == trophy {
                break;
            }
            at = at.at(0x24);
        }
        if field <= 5 {
            self.datai_write(at.at(8 + 4 * field), v.to_bits());
        }
        if self.game.toys.gallery.debug_text.is_some() {
            let v = [0, 1, 2, 3, 4, 5].map(|f| f32::from_bits(self.datai_u32(at.at(8 + 4 * f))));
            self.toy_z_print(v);
        }
    }

    /// The shown trophy's root, first stand and model (`anim->gobj`'s joint, its child and
    /// that one's next).
    fn toy_shown_joints(&self) -> (JObjId, JObjId) {
        let g = self.game.toys.anim.gobj.expect("the shown trophy");
        let Some(Obj::JObj(root)) = self.game.world.gobj(g).obj else {
            panic!("the shown trophy has no joint");
        };
        let stand = self.game.objs.jobjs[root]
            .child
            .expect("the trophy's stand");
        let model = self.game.objs.jobjs[stand]
            .next
            .expect("the trophy's model");
        (stand, model)
    }

    fn toy_cobj_in(&self, gobj: GObjId) -> CObjId {
        match self.game.world.gobj(gobj).obj {
            Some(Obj::CObj(c)) => c,
            _ => panic!("the gallery's view has no camera"),
        }
    }

    /// The view's yaw and pitch turned by `x` and `y` (three degrees a frame at full tilt),
    /// the pitch held to 89 degrees each way, the yaw kept within a turn, and the lights
    /// turned with them.
    fn toy_debug_turn(&mut self, x: f32, y: f32) {
        let g = &mut self.game.toys.gallery;
        g.yaw = mem(fp::fnmsubs(reg(3.0), reg(x), reg(g.yaw)));
        g.pitch = mem(fp::fnmsubs(reg(3.0), reg(y), reg(g.pitch)));
        if reg(g.pitch) < reg(-89.0) {
            g.pitch = -89.0;
        }
        if reg(g.pitch) > reg(89.0) {
            g.pitch = 89.0;
        }
        if reg(g.yaw) < reg(-360.0) {
            g.yaw = mem(fp::fadds(reg(g.yaw), reg(360.0)));
        }
        if reg(g.yaw) > reg(360.0) {
            g.yaw = mem(fp::fsubs(reg(g.yaw), reg(360.0)));
        }
        g.light_yaw = g.yaw;
        g.light_pitch = g.pitch;
    }

    /// The next light, round after the sixth, and its back.
    fn toy_next_light(&mut self) {
        let g = &mut self.game.toys.gallery;
        g.idle = 0;
        g.light += 1;
        if g.light == 6 {
            g.light = 0;
        }
        let l = g.light;
        self.toy_set_light(l);
        self.toy_back(l);
    }

    /// The view's distance changed by `k` of itself, held to 5 to 250.
    fn toy_debug_zoom(&mut self, k: f32) {
        let g = &mut self.game.toys.gallery;
        g.dist = mem(fp::fnmsubs(
            reg(g.dist),
            fp::fmuls(reg(0.025), reg(k)),
            reg(g.dist),
        ));
        if reg(g.dist) < reg(5.0) {
            g.dist = 5.0;
        }
        if reg(g.dist) > reg(250.0) {
            g.dist = 250.0;
        }
    }

    /// `_Toy_8030B530`, the Z view (the frustum camera's process), while a trophy shows: Z
    /// opens the list; X closes in or comes back as the view's A and B do; X held turns the
    /// model about y (the D-pad a hundredth of a degree) and the stick's y zooms; A held moves
    /// it a hundredth along the stick's or the D-pad's direction (x and z from below 25
    /// degrees under, else x and y); B held scales it (up and down) or its stands (right and
    /// left); else the stick turns the view. Each change is written into `TyDatai`. Then
    /// Start the next light, L and R the trophy before or after, Start again the view back;
    /// port 2's D-pad moves the frustum a thousandth (its Start only reports); else the C
    /// stick pans.
    #[doc(alias = "_Toy_8030B530")]
    pub fn toy_z_view_proc(&mut self, gobj: GObjId) {
        let c = self.toy_cobj_in(gobj);
        if self.game.toys.anim.gobj.is_none() {
            return;
        }
        let (stand, model) = self.toy_shown_joints();
        let sx = self.toy_stick_x();
        let sy = self.toy_axis(1);
        let (mut adj_x, mut adj_y) = (dead_zone(sx), dead_zone(sy));
        let cx = self.toy_axis(2);
        let cy = self.toy_axis(3);
        let (adj_sx, adj_sy) = (dead_zone(cx), dead_zone(cy));
        let g = &mut self.game.toys.gallery;
        if g.cooldown != 0 {
            g.cooldown -= 1;
            return;
        }
        if self.toy_triggered() & PAD_Z != 0 {
            self.lb_audio_call(0);
            if self.game.toys.gallery.state == 0 {
                self.toy_gallery_leave(true);
                self.game.world.mark_ran(gobj);
                self.toy_list_open();
                return;
            }
        }
        if matches!(self.game.toys.gallery.state, 1 | 3) {
            self.toy_transition(c);
            return;
        }
        if self.toy_triggered() & PAD_X != 0 {
            self.toy_keep_frustum(c);
            // `OSReport("*** Pad = %d\n", Toy_80305B88())`.
            let _ = self.toy_triggered();
            self.game.toys.gallery.x5c = 0;
            match self.game.toys.gallery.state {
                0 => {
                    self.toy_close_links(true);
                    let g = &mut self.game.toys.gallery;
                    g.state = 1;
                    g.x5c = 0;
                    return;
                }
                2 => {
                    self.game.toys.gallery.x5c = 0;
                    self.toy_view_reset(false);
                    self.toy_close_links(false);
                    let g = &mut self.game.toys.gallery;
                    g.x5c = 0;
                    g.state = 3;
                    self.lb_audio_call(0);
                    return;
                }
                _ => {}
            }
        }
        let trophy = i32::from(self.game.toys.anim.trophy);
        let held = self.toy_held();
        if held & PAD_X != 0 {
            if self.toy_held() & PAD_DRIGHT != 0 {
                adj_x = 0.01;
                self.game.toys.gallery.cooldown = 8;
            }
            if self.toy_held() & PAD_DLEFT != 0 {
                adj_x = -0.01;
                self.game.toys.gallery.cooldown = 8;
            }
            let jobjs = &mut self.game.objs.jobjs;
            jobj::set_srt(jobjs, model, |o| {
                o.rotate.y = mem(fp::fmadds(reg(0.017_453_292), reg(adj_x), reg(o.rotate.y)));
            });
            let deg = mem(fp::fmuls(reg(57.295_78), reg(jobjs[model].rotate.y)));
            self.toy_param_edit(trophy, 5, deg);
            if past(adj_y) {
                self.toy_debug_zoom(adj_y);
            }
        } else if self.toy_held() & PAD_A != 0 {
            for (pad, x, y) in [
                (PAD_DRIGHT, Some(1.0), None),
                (PAD_DLEFT, Some(-1.0), None),
                (PAD_DUP, None, Some(1.0)),
                (PAD_DDOWN, None, Some(-1.0)),
            ] {
                if self.toy_held() & pad != 0 {
                    if let Some(x) = x {
                        adj_x = x;
                    }
                    if let Some(y) = y {
                        adj_y = y;
                    }
                    self.game.toys.gallery.cooldown = 8;
                }
            }
            let angle = mem(crate::game::ft::co::guard::stick_angle(
                reg(adj_y),
                reg(adj_x),
            ));
            let below = reg(self.game.toys.gallery.pitch) < reg(-25.0);
            let jobjs = &mut self.game.objs.jobjs;
            if past(adj_x) {
                let d = mem(fp::fmuls(reg(0.01), reg(crate::math::cos(angle))));
                jobj::set_srt(jobjs, model, |o| {
                    o.translate.x = mem(fp::fadds(reg(o.translate.x), reg(d)));
                });
                let v = jobjs[model].translate.x;
                self.toy_param_edit(trophy, 0, v);
            }
            if past(adj_y) {
                let jobjs = &mut self.game.objs.jobjs;
                let s = reg(crate::math::sin(angle));
                if below {
                    let d = mem(fp::fmuls(reg(0.01), fp::fneg(s)));
                    jobj::set_srt(jobjs, model, |o| {
                        o.translate.z = mem(fp::fadds(reg(o.translate.z), reg(d)));
                    });
                    let v = jobjs[model].translate.z;
                    self.toy_param_edit(trophy, 2, v);
                } else {
                    let d = mem(fp::fmuls(reg(0.01), s));
                    jobj::set_srt(jobjs, model, |o| {
                        o.translate.y = mem(fp::fadds(reg(o.translate.y), reg(d)));
                    });
                    let v = jobjs[model].translate.y;
                    self.toy_param_edit(trophy, 1, v);
                }
            }
        } else if self.toy_held() & PAD_B != 0 {
            let up = reg(adj_y) > reg(0.8) || self.toy_held() & PAD_DUP != 0;
            let down = !up && (reg(adj_y) < reg(-0.8) || self.toy_held() & PAD_DDOWN != 0);
            let right =
                !up && !down && (reg(adj_x) > reg(0.8) || self.toy_held() & PAD_DRIGHT != 0);
            let left = !up
                && !down
                && !right
                && (reg(adj_x) < reg(-0.8) || self.toy_held() & PAD_DLEFT != 0);
            if up || down {
                let k = if up { 0.01 } else { -0.01 };
                let add = |v: f32| mem(fp::fadds(reg(v), reg(k)));
                let jobjs = &mut self.game.objs.jobjs;
                jobj::set_srt(jobjs, model, |o| o.scale.x = add(o.scale.x));
                jobj::set_srt(jobjs, model, |o| o.scale.y = add(o.scale.y));
                jobj::set_srt(jobjs, model, |o| o.scale.z = add(o.scale.z));
                let v = jobjs[model].scale.x;
                self.toy_param_edit(trophy, 3, v);
                let pad = if up { PAD_DUP } else { PAD_DDOWN };
                if self.toy_held() & pad != 0 {
                    self.game.toys.gallery.cooldown = 8;
                }
            } else if right || left {
                let k = if right { 0.01 } else { -0.01 };
                let add = |v: f32| mem(fp::fadds(reg(v), reg(k)));
                let jobjs = &mut self.game.objs.jobjs;
                jobj::set_srt(jobjs, stand, |o| o.scale.x = add(o.scale.x));
                jobj::set_srt(jobjs, stand, |o| o.scale.z = add(o.scale.z));
                let second = jobjs[model].next.expect("the trophy's second stand");
                jobj::set_srt(jobjs, second, |o| o.scale.x = add(o.scale.x));
                jobj::set_srt(jobjs, second, |o| o.scale.z = add(o.scale.z));
                let v = jobjs[second].scale.x;
                self.toy_param_edit(trophy, 4, v);
                let pad = if right { PAD_DRIGHT } else { PAD_DLEFT };
                if self.toy_held() & pad != 0 {
                    self.game.toys.gallery.cooldown = 8;
                }
            }
        } else {
            // In the view, the D-pad alone switches on `_Toy_sbss_804D6E60`, whose every case
            // only asks the saved language.
            self.toy_debug_turn(adj_x, adj_y);
        }
        let moved_x = mem(fp::fmuls(reg(0.3), reg(adj_sx)));
        let moved_y = mem(fp::fmuls(reg(0.3), reg(adj_sy)));
        if self.toy_triggered() & PAD_START != 0 {
            self.toy_next_light();
        }
        self.toy_stand_by_view(c);
        if self.toy_triggered() & (PAD_L | PAD_R) != 0 {
            let back = self.toy_triggered() & 0x441 != 0;
            self.toy_ring_step(back);
            self.toy_view_reset(false);
            self.toy_view_place(0.0, 0.0);
            self.game.toys.gallery.idle = 0x95E;
            let e = self.game.toys.gallery.selected_entry;
            self.toy_show_entry(e);
            let t = self.game.toys.gallery.entries[usize::from(e)].trophy;
            self.toy_panel_texts(i32::from(t));
            let mark = self.game.toys.gallery.panel_parts[8];
            self.toy_mark(mark, i32::from(t));
        }
        if self.toy_triggered() & PAD_START != 0 {
            self.toy_view_reset(false);
        }
        // Port 2's buttons tune the frustum (its Start reports it).
        let p2 = self.game.pads.copy[1];
        if p2.trigger & PAD_START != 0 {
            return;
        }
        let cobjs = &mut self.game.objs.cobjs;
        let (top, bottom) = (cobj::top(cobjs, Some(c)), cobj::bottom(cobjs, Some(c)));
        let step = |v: f32, up: bool| {
            if up {
                mem(fp::fadds(reg(v), reg(0.001)))
            } else {
                mem(fp::fsubs(reg(v), reg(0.001)))
            }
        };
        if p2.button & (PAD_DUP | PAD_DDOWN) != 0 {
            let up = p2.button & PAD_DUP != 0;
            cobj::set_top(cobjs, Some(c), step(top, up));
            cobj::set_bottom(cobjs, Some(c), step(bottom, up));
            return;
        }
        if p2.button & (PAD_DLEFT | PAD_DRIGHT) != 0 {
            let up = p2.button & PAD_DLEFT == 0;
            let (right, left) = (cobj::right(cobjs, Some(c)), cobj::left(cobjs, Some(c)));
            cobj::set_right(cobjs, Some(c), step(right, up));
            cobj::set_left(cobjs, Some(c), step(left, up));
            return;
        }
        self.toy_view_place(moved_x, moved_y);
    }

    /// `_Toy_8030E110`, the L view (the frustum camera's process): as the view, B opens the
    /// list and A, X or Y close in, B or two minutes idle come back, A with the stick pans and
    /// the stick zooms in the close-up; in every state Y turns the fog on or off, the C stick
    /// turns the view, L and R (unless the close-up moved) show the trophy before or after,
    /// Start the next light, Z puts the view and the light back; both stands stay hidden.
    #[doc(alias = "_Toy_8030E110")]
    pub fn toy_l_view_proc(&mut self, gobj: GObjId) {
        let c = self.toy_cobj_in(gobj);
        let (mut moved_x, mut moved_y) = (false, false);
        let sx = self.toy_stick_x();
        let sy = self.toy_axis(1);
        {
            let g = &mut self.game.toys.gallery;
            g.pan = [0.0, 0.0];
            g.stick = [dead_zone(sx), dead_zone(sy)];
        }
        let cx = self.toy_axis(2);
        let cy = self.toy_axis(3);
        self.game.toys.gallery.cstick = [dead_zone(cx), dead_zone(cy)];
        let g = &mut self.game.toys.gallery;
        if g.cooldown != 0 {
            g.cooldown -= 1;
            self.toy_view_place(0.0, 0.0);
            return;
        }
        match self.game.toys.gallery.state {
            0 => {
                if self.toy_triggered() & PAD_B != 0 {
                    self.lb_audio_call(0);
                    self.toy_gallery_leave(true);
                    self.game.world.mark_ran(gobj);
                    self.toy_list_open();
                    return;
                }
                if self.toy_total() == 0 {
                    return;
                }
                if self.toy_triggered() & (PAD_A | PAD_X | PAD_Y) != 0 {
                    self.lb_audio_call(1);
                    self.toy_keep_frustum(c);
                    let g = &mut self.game.toys.gallery;
                    g.x5c = 0;
                    g.state = 1;
                    self.toy_view_place(0.0, 0.0);
                    return;
                }
            }
            1 | 3 => self.toy_transition(c),
            2 => {
                let t = self.toy_triggered();
                if t & PAD_B != 0 || int_f(self.game.toys.gallery.x5c) > reg(7200.0) {
                    self.lb_audio_call(0);
                    self.toy_keep_frustum(c);
                    self.toy_info_flags(9);
                    self.game.toys.gallery.x5c = 0;
                    let data = self.data;
                    let o = &mut self.game.objs;
                    let _ = cobj::eye_position(o, data, c);
                    let eye = Vec3::new(0.0, 8.0, 0.0);
                    let i = cobj::interest(o, data, c).expect("the camera's interest");
                    let d = |a: f32, b: f32| mem(fp::fdivs(fp::fsubs(reg(a), reg(b)), reg(10.0)));
                    self.game.toys.step = Vec3::new(d(eye.x, i.x), d(eye.y, i.y), d(eye.z, i.z));
                    let g = &mut self.game.toys.gallery;
                    g.dist_step = mem(fp::fdivs(fp::fsubs(reg(g.dist), reg(38.0)), reg(10.0)));
                    g.pitch_step = mem(fp::fdivs(reg(g.pitch), reg(10.0)));
                    g.state = 3;
                }
                let t = self.toy_triggered();
                let g = &self.game.toys.gallery;
                let sum = fp::fadds(
                    reg(g.cstick[1]),
                    fp::fadds(
                        reg(g.cstick[0]),
                        fp::fadds(reg(g.stick[0]), reg(g.stick[1])),
                    ),
                );
                if fp::fadds(sum, fp::frsp(f64::from(t))) == 0.0 {
                    self.game.toys.gallery.x5c += 1;
                } else {
                    self.game.toys.gallery.x5c = 0;
                }
                let g = &self.game.toys.gallery;
                let scale = fp::fdivs(reg(g.dist), reg(38.0));
                let pan = |v: f32| mem(fp::fmuls(fp::fmuls(reg(0.3), reg(v)), scale));
                let s = g.stick;
                if self.toy_held() & PAD_A != 0 && reg(s[0]) != 0.0 && reg(s[0]) < 0.0 {
                    self.game.toys.gallery.pan[0] = pan(s[0]);
                }
                if self.toy_held() & PAD_A != 0 && reg(s[0]) != 0.0 && reg(s[0]) > 0.0 {
                    self.game.toys.gallery.pan[0] = pan(s[0]);
                }
                if self.toy_held() & PAD_A != 0 && reg(s[1]) != 0.0 && reg(s[1]) > 0.0 {
                    self.game.toys.gallery.pan[1] = pan(s[1]);
                }
                if self.toy_held() & PAD_A != 0 && reg(s[1]) != 0.0 && reg(s[1]) < 0.0 {
                    self.game.toys.gallery.pan[1] = pan(s[1]);
                }
                let p = self.game.toys.gallery.pan;
                let hide = if reg(p[0]) != 0.0 || reg(p[1]) != 0.0 || self.toy_held() & PAD_A != 0 {
                    0
                } else {
                    1
                };
                self.toy_axes_hidden(hide);
                if reg(p[0]) != 0.0 || reg(p[1]) != 0.0 {
                    moved_x = true;
                }
                if !moved_x {
                    let y = self.game.toys.gallery.stick[1];
                    if reg(y) != 0.0 {
                        let g = &mut self.game.toys.gallery;
                        g.dist = mem(fp::fnmsubs(
                            reg(g.dist),
                            fp::fmuls(reg(0.025), reg(y)),
                            reg(g.dist),
                        ));
                        moved_y = true;
                    }
                    let g = &mut self.game.toys.gallery;
                    if reg(g.dist) < reg(5.0) {
                        g.dist = 5.0;
                    }
                    if reg(g.dist) > reg(250.0) {
                        g.dist = 250.0;
                    }
                }
            }
            _ => {}
        }
        if self.toy_triggered() & PAD_Y != 0 {
            self.game.toys.gallery.fog_on ^= 1;
        }
        let [cx, cy] = self.game.toys.gallery.cstick;
        self.toy_debug_turn(cx, cy);
        if fp::fadds(reg(cx), reg(cy)) != 0.0 {
            self.game.toys.gallery.spin = 0.0;
        }
        if !moved_x && !moved_y && self.toy_triggered() & (PAD_L | PAD_R) != 0 {
            if self.toy_total() == 1 {
                self.game.toys.gallery.idle = 0x961;
                return;
            }
            let x = self.game.toys.gallery.stick[0];
            if reg(x) < 0.0 || self.toy_triggered() & 0x441 != 0 {
                self.toy_ring_step(true);
            } else if reg(x) > 0.0 || self.toy_triggered() & 0x822 != 0 {
                self.toy_ring_step(false);
            }
            self.toy_view_reset(false);
            self.game.toys.gallery.idle = 0x95E;
            let e = self.game.toys.gallery.selected_entry;
            self.toy_show_entry(e);
            self.game.toys.gallery.cooldown = 0x14;
        }
        if self.toy_triggered() & PAD_START != 0 {
            self.lb_audio_call(2);
            self.toy_next_light();
        }
        if self.toy_triggered() & PAD_Z != 0 {
            self.toy_view_reset(false);
            self.game.toys.gallery.light = 0;
            self.toy_set_light(0);
            self.toy_back(0);
            let a = &mut self.game.toys.anim;
            a.x11 = 0;
            a.x10 = 0;
            a.x0e = 1;
        }
        let a = self.game.toys.anim;
        let jobjs = &mut self.game.objs.jobjs;
        jobj::set_flags_all(jobjs, a.jobj[0], jobj::HIDDEN);
        jobj::set_flags_all(jobjs, a.jobj[1], jobj::HIDDEN);
        let p = self.game.toys.gallery.pan;
        self.toy_view_place(p[0], p[1]);
        let g = &mut self.game.toys.gallery;
        g.stick_prev = g.stick;
        g.cstick_prev = g.cstick;
    }
}
