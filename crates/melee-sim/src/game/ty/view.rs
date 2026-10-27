//! The gallery's view (`_Toy_80309404`, the frustum camera's process): the sticks and
//! buttons that turn, zoom, pan and change the trophy shown, and the close-up's transitions
//! (`_Toy_80308F04`).

use gekko_fp as fp;

use super::super::state::Running;
use super::gallery::{CLOSE_FRUSTUM, VIEW_FRUSTUM};
use crate::fpu::{mem, reg};
use crate::hsd::cobj;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

/// `HSD_PAD_*` as the copy statuses hold them.
const PAD_DLEFT: u32 = 0x1;
const PAD_DRIGHT: u32 = 0x2;
const PAD_DDOWN: u32 = 0x4;
const PAD_DUP: u32 = 0x8;
const PAD_Z: u32 = 0x10;
const PAD_A: u32 = 0x100;
const PAD_B: u32 = 0x200;
const PAD_X: u32 = 0x400;
const PAD_Y: u32 = 0x800;
const PAD_START: u32 = 0x1000;

/// `(f32) n` of an `s32`, as the code converts it (exact for the counts it converts).
fn int_f(n: i32) -> f64 {
    fp::frsp(f64::from(n))
}

/// A stick's value past its dead zone of 0.2, rescaled to 0 to 1:
/// `-((0.2f * sign) - v) / 0.8f`, or 0 inside it.
pub(super) fn dead_zone(v: f32) -> f32 {
    let r = reg(v);
    if r > reg(-0.2) && r < reg(0.2) {
        return 0.0;
    }
    let sign = if r > 0.0 { 1 } else { -1 };
    mem(fp::fdivs(fp::fnmsubs(reg(0.2), int_f(sign), r), reg(0.8)))
}

impl Running<'_> {
    /// `gm_801677E8`: the port that last gave the screens input.
    fn toy_port(&mut self, port: usize) {
        self.game.menu.confirm_port = port as i8;
    }

    /// `Toy_80305B88`: the first port's triggered buttons that has any (its port noted),
    /// else the fourth's (none).
    #[doc(alias = "Toy_80305B88")]
    pub fn toy_triggered(&mut self) -> u32 {
        let mut b = 0;
        for i in 0..4 {
            b = self.game.pads.copy[i].trigger;
            if b != 0 {
                self.toy_port(i);
                break;
            }
        }
        b
    }

    /// `Toy_80305C44`: the same for the buttons held.
    #[doc(alias = "Toy_80305C44")]
    pub fn toy_held(&mut self) -> u32 {
        let mut b = 0;
        for i in 0..4 {
            b = self.game.pads.copy[i].button;
            if b != 0 {
                self.toy_port(i);
                break;
            }
        }
        b
    }

    /// `Toy_80305D00`: the first port's control stick's x tilted past 0.1, else the fourth's
    /// (no port noted).
    #[doc(alias = "Toy_80305D00")]
    pub fn toy_stick_x(&self) -> f32 {
        let mut v = 0.0;
        for i in 0..4 {
            v = self.game.pads.copy[i].nml_stick_x;
            if reg(v).abs() > reg(0.1) {
                break;
            }
        }
        v
    }

    /// `Toy_80305DB0`, `Toy_80305EB4`, `Toy_80305FB8`: the first port's control stick's y,
    /// C-stick's x or y tilted past 0.1 (its port noted), else the fourth's.
    #[doc(alias = "Toy_80305DB0")]
    #[doc(alias = "Toy_80305EB4")]
    #[doc(alias = "Toy_80305FB8")]
    pub fn toy_axis(&mut self, axis: u8) -> f32 {
        let mut v = 0.0;
        for i in 0..4 {
            let p = &self.game.pads.copy[i];
            v = match axis {
                1 => p.nml_stick_y,
                2 => p.nml_sub_stick_x,
                _ => p.nml_sub_stick_y,
            };
            if reg(v).abs() > reg(0.1) {
                self.toy_port(i);
                break;
            }
        }
        v
    }

    /// `_Toy_80309338(a, NULL)`: a point's distance from the origin (MSL's inline `sqrtf`).
    #[doc(alias = "_Toy_80309338")]
    fn toy_distance(p: Vec3) -> f32 {
        let (dy, dx, dz) = (
            fp::fsubs(reg(p.y), 0.0),
            fp::fsubs(reg(p.x), 0.0),
            fp::fsubs(reg(p.z), 0.0),
        );
        let s = fp::fmadds(dz, dz, fp::fmadds(dx, dx, fp::fmuls(dy, dy)));
        // `fcmpo` with zero: NaN and zero come back as they are.
        if s.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
            return mem(s);
        }
        crate::math::sqrt(mem(s))
    }

    /// `_Toy_803102C4`: whether the close-up's axes hide.
    #[doc(alias = "_Toy_803102C4")]
    fn toy_axes_hidden(&mut self, hidden: i8) {
        self.game.toys.gallery.axes_hidden = hidden;
    }

    /// `_Toy_80308DC8(cobj)`: the stand's look by the view's pitch (the first from above or
    /// close up, else the second), and below ten degrees which of the two stands shows (the
    /// second when the eye is below the floor or higher than ten degrees).
    #[doc(alias = "_Toy_80308DC8")]
    fn toy_stand_by_view(&mut self, c: cobj::CObjId) {
        let data = self.data;
        let o = &mut self.game.objs;
        let _ = cobj::interest(o, data, c);
        let eye = cobj::eye_position(o, data, c).expect("the camera's eye");
        let pitch = reg(self.game.toys.gallery.pitch);
        if pitch >= reg(25.0) || (pitch >= reg(-3.0) && reg(Self::toy_distance(eye)) <= reg(15.0)) {
            self.toy_stand_look(1, true);
            return;
        }
        self.toy_stand_look(2, true);
        let a = self.game.toys.anim;
        let (j0, j1) = (a.jobj[0], a.jobj[1]);
        let jobjs = &mut self.game.objs.jobjs;
        let second = reg(self.game.toys.gallery.pitch) >= reg(10.0) || reg(eye.y) < 0.0;
        if second {
            self.game.toys.anim.x0e = 1;
            jobj::clear_flags_all(jobjs, j1, jobj::HIDDEN);
            jobj::set_flags_all(jobjs, j0, jobj::HIDDEN);
        } else {
            self.game.toys.anim.x0e = 0;
            jobj::clear_flags_all(jobjs, j0, jobj::HIDDEN);
            jobj::set_flags_all(jobjs, j1, jobj::HIDDEN);
        }
    }

    /// The info board's sprites hidden (9) or shown (8).
    fn toy_info_flags(&mut self, flags: u32) {
        let info = self.game.toys.gallery.info.expect("the info board");
        let mut s = match self.game.world.gobj(info).obj {
            Some(super::super::state::Obj::SObj(s)) => s,
            _ => None,
        };
        while let Some(o) = s {
            self.game.objs.sobjs[o.0].flags = flags;
            s = self.game.objs.sobjs[o.0].next;
        }
    }

    /// The frustum kept for a transition (`HSD_CObjGetTop` to `GetLeft` into
    /// `_Toy_sbss_804D6E80` to `E8C`).
    fn toy_keep_frustum(&mut self, c: cobj::CObjId) {
        let cobjs = &self.game.objs.cobjs;
        let f = [
            cobj::top(cobjs, Some(c)),
            cobj::bottom(cobjs, Some(c)),
            cobj::right(cobjs, Some(c)),
            cobj::left(cobjs, Some(c)),
        ];
        self.game.toys.gallery.frustum = f;
    }

    /// The three cameras' links (`gxlink_prios`) for the close-up or the view.
    fn toy_close_links(&mut self, close: bool) {
        let g = &self.game.toys.gallery;
        let (main, text, light) = (
            g.cams[0].expect("the main camera"),
            g.cams[1].expect("the texts' camera"),
            g.cams[3].expect("the light's camera"),
        );
        let w = &mut self.game.world;
        if close {
            w.gobj_mut(main).gxlink_prios = 0x0048_0000_0000_0000;
            w.gobj_mut(text).gxlink_prios = 0;
            w.gobj_mut(light).gxlink_prios = 0;
        } else {
            w.gobj_mut(main).gxlink_prios = 0x5048_0000_0000_0000;
            w.gobj_mut(text).gxlink_prios = 0x8000_0000_0000_0000;
            w.gobj_mut(light).gxlink_prios = 0x4000_0000_0000_0000;
        }
    }

    /// `_Toy_80308F04(cobj)`: the close-up's transitions, ten frames each: the frustum
    /// narrowing (state 1) to the close-up's, then the close-up (state 2, the info board as
    /// the stand's base says, the pads renewed); or the view coming back (state 3): the
    /// distance, pitch and interest stepping back, the stand's look, the frustum widening,
    /// then the view (state 0) put back.
    #[doc(alias = "_Toy_80308F04")]
    fn toy_transition(&mut self, c: cobj::CObjId) {
        let cobjs = &self.game.objs.cobjs;
        let (top, bottom, right, left) = (
            cobj::top(cobjs, Some(c)),
            cobj::bottom(cobjs, Some(c)),
            cobj::right(cobjs, Some(c)),
            cobj::left(cobjs, Some(c)),
        );
        let g = &self.game.toys.gallery;
        let ten = int_f(g.x5c) < reg(10.0);
        let [st, sb, sr, sl] = super::gallery::FRUSTUM_STEP;
        if g.state as u8 == 1 {
            if ten {
                let set =
                    |r: &mut Self, i: usize, step: f32, cur: f32, lim: Option<f32>, which: u8| {
                        let v = mem(fp::fsubs(reg(r.game.toys.gallery.frustum[i]), reg(step)));
                        r.game.toys.gallery.frustum[i] = v;
                        if reg(cur) != reg(v) {
                            let v = match lim {
                                Some(l) if reg(v) < reg(l) => l,
                                _ => v,
                            };
                            set_side(&mut r.game.objs.cobjs, c, which, v);
                        }
                    };
                set(self, 0, st, top, Some(CLOSE_FRUSTUM[0]), 0);
                set(self, 1, sb, bottom, Some(CLOSE_FRUSTUM[1]), 1);
                set(self, 2, sr, right, Some(CLOSE_FRUSTUM[3]), 2);
                set(self, 3, sl, left, None, 3);
                self.game.toys.gallery.x5c += 1;
            } else {
                let cobjs = &mut self.game.objs.cobjs;
                set_side(cobjs, c, 0, CLOSE_FRUSTUM[0]);
                set_side(cobjs, c, 1, CLOSE_FRUSTUM[1]);
                set_side(cobjs, c, 2, CLOSE_FRUSTUM[3]);
                set_side(cobjs, c, 3, CLOSE_FRUSTUM[2]);
                let flags = if self.game.toys.gallery.base_shown != 0 {
                    9
                } else {
                    8
                };
                self.toy_info_flags(flags);
                let g = &mut self.game.toys.gallery;
                g.state = 2;
                g.x5c = 0;
                g.cooldown = 4;
                self.pads_renew_status();
            }
            return;
        }
        if ten {
            let g = &mut self.game.toys.gallery;
            g.dist = mem(fp::fsubs(reg(g.dist), reg(g.dist_step)));
            g.pitch = mem(fp::fsubs(reg(g.pitch), reg(g.pitch_step)));
            let step = self.game.toys.step;
            let data = self.data;
            let o = &mut self.game.objs;
            let i = cobj::interest(o, data, c).expect("the camera's interest");
            let add = |a: f32, b: f32| mem(fp::fadds(reg(a), reg(b)));
            cobj::set_interest(
                o,
                c,
                Vec3::new(add(i.x, step.x), add(i.y, step.y), add(i.z, step.z)),
            );
            self.toy_stand_by_view(c);
            let set = |r: &mut Self, i: usize, step: f32, cur: f32, lim: Option<f32>, which: u8| {
                let v = mem(fp::fadds(reg(r.game.toys.gallery.frustum[i]), reg(step)));
                r.game.toys.gallery.frustum[i] = v;
                if reg(cur) != reg(v) {
                    let v = match lim {
                        Some(l) if reg(v) > reg(l) => l,
                        _ => v,
                    };
                    set_side(&mut r.game.objs.cobjs, c, which, v);
                }
            };
            set(self, 0, st, top, Some(VIEW_FRUSTUM[0]), 0);
            set(self, 1, sb, bottom, Some(VIEW_FRUSTUM[1]), 1);
            set(self, 2, sr, right, Some(VIEW_FRUSTUM[3]), 2);
            set(self, 3, sl, left, None, 3);
            self.game.toys.gallery.x5c += 1;
        } else {
            self.game.toys.gallery.state = 0;
            self.toy_view_reset(true);
            let cobjs = &mut self.game.objs.cobjs;
            set_side(cobjs, c, 0, VIEW_FRUSTUM[0]);
            set_side(cobjs, c, 1, VIEW_FRUSTUM[1]);
            set_side(cobjs, c, 2, VIEW_FRUSTUM[3]);
            set_side(cobjs, c, 3, VIEW_FRUSTUM[2]);
            self.toy_axes_hidden(1);
            let g = &mut self.game.toys.gallery;
            g.x5c = 0;
            g.idle = 0x95E;
            g.cooldown = 4;
            self.pads_renew_status();
        }
    }

    /// `_Toy_80309404`, the gallery's view (the frustum camera's process).
    #[doc(alias = "_Toy_80309404")]
    pub fn toy_view_proc(&mut self, gobj: GObjId) {
        let c = match self.game.world.gobj(gobj).obj {
            Some(super::super::state::Obj::CObj(c)) => c,
            _ => panic!("the gallery's view has no camera"),
        };
        let data = self.data;
        let eye = cobj::eye_position(&mut self.game.objs, data, c).expect("the camera's eye");
        let dist = Self::toy_distance(eye);
        let (mut zoomed, mut moved, mut rotate) = (false, false, false);
        let sx = self.toy_stick_x();
        let sy = self.toy_axis(1);
        {
            let g = &mut self.game.toys.gallery;
            g.stick = [sx, sy];
            g.pan = [0.0, 0.0];
            g.stick = [dead_zone(g.stick[0]), dead_zone(g.stick[1])];
        }
        let cx = self.toy_axis(2);
        let cy = self.toy_axis(3);
        {
            let g = &mut self.game.toys.gallery;
            g.cstick = [dead_zone(cx), dead_zone(cy)];
        }
        let cd = self.game.toys.gallery.cooldown;
        if cd != 0 {
            self.game.toys.gallery.cooldown = cd - 1;
            self.toy_view_place(0.0, 0.0);
            return;
        }
        if self.game.buttons_triggered(4) & super::super::mn::main::PAD_LR_START != 0 {
            self.lb_audio_call(0);
            let p = self.game.world.current_proc().expect("the view's process");
            self.game.world.remove_proc(p);
            self.toy_gallery_leave(true);
            self.game.world.mark_ran(gobj);
            self.game.game_data.x1850.force_main_menu = 1;
            self.game.toys.table.exit = 1;
            return;
        }
        let deg_inc = |v: f32| v;
        let _ = deg_inc;
        match self.game.toys.gallery.state {
            0 => {
                let t = self.toy_triggered();
                if t & PAD_B != 0 {
                    self.lb_audio_call(0);
                    self.toy_gallery_leave(true);
                    self.game.world.mark_ran(gobj);
                    self.toy_list_open();
                    return;
                }
                if self.toy_total() == 0 {
                    return;
                }
                let g = &self.game.toys.gallery;
                let csum = fp::fadds(reg(g.cstick[0]), reg(g.cstick[1]));
                let reset = if csum != 0.0 {
                    true
                } else {
                    self.toy_triggered() & 0xF != 0
                };
                if reset {
                    self.game.toys.gallery.idle = 0;
                } else {
                    let g = &mut self.game.toys.gallery;
                    if int_f(g.idle) > reg(2400.0) {
                        g.spin = mem(fp::fadds(reg(g.spin), reg(0.01)));
                        if reg(g.spin) > reg(1.0) {
                            g.spin = 1.0;
                        }
                        g.yaw = mem(fp::fadds(reg(g.yaw), reg(g.spin)));
                        if reg(g.yaw) < reg(-360.0) {
                            g.yaw = mem(fp::fadds(reg(g.yaw), reg(360.0)));
                        }
                        if reg(g.yaw) > reg(360.0) {
                            g.yaw = mem(fp::fsubs(reg(g.yaw), reg(360.0)));
                        }
                        g.light_yaw = g.yaw;
                    }
                    g.idle += 1;
                }
                if self.toy_total() == 1 {
                    let g = &mut self.game.toys.gallery;
                    if fp::fadds(reg(g.stick[0]), reg(g.stick[1])) != 0.0 {
                        g.stick = [0.0, 0.0];
                    }
                }
                let t = self.toy_triggered();
                if t & (PAD_A | PAD_X | PAD_Y) != 0 {
                    self.lb_audio_call(1);
                    self.toy_keep_frustum(c);
                    self.game.toys.gallery.x5c = 0;
                    self.toy_close_links(true);
                    self.game.toys.gallery.state = 1;
                    self.toy_view_place(0.0, 0.0);
                    return;
                }
                rotate = true;
            }
            1 | 3 => {
                self.toy_transition(c);
            }
            2 => {
                if self.toy_triggered() & PAD_A != 0 {
                    let g = &mut self.game.toys.gallery;
                    g.base_shown ^= 1;
                    let flags = if g.base_shown != 0 { 9 } else { 8 };
                    self.toy_info_flags(flags);
                }
                let t = self.toy_triggered();
                if t & PAD_B != 0 || int_f(self.game.toys.gallery.x5c) > reg(7200.0) {
                    self.lb_audio_call(0);
                    self.toy_keep_frustum(c);
                    self.toy_close_links(false);
                    self.toy_info_flags(9);
                    self.game.toys.gallery.x5c = 0;
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
                let tf = fp::frsp(f64::from(t));
                if fp::fadds(sum, tf) == 0.0 {
                    self.game.toys.gallery.x5c += 1;
                } else {
                    self.game.toys.gallery.x5c = 0;
                }
                let scale = fp::fdivs(reg(dist), reg(38.0));
                let pan = |k: f32| mem(fp::fmuls(reg(k), scale));
                if self.toy_held() & PAD_DLEFT != 0 {
                    self.game.toys.gallery.pan[0] = pan(-0.3);
                }
                if self.toy_held() & PAD_DRIGHT != 0 {
                    self.game.toys.gallery.pan[0] = pan(0.3);
                }
                if self.toy_held() & PAD_DUP != 0 {
                    self.game.toys.gallery.pan[1] = pan(0.3);
                }
                if self.toy_held() & PAD_DDOWN != 0 {
                    self.game.toys.gallery.pan[1] = pan(-0.3);
                }
                let stick_pan = |v: f32| mem(fp::fmuls(fp::fmuls(reg(0.3), reg(v)), scale));
                let s = self.game.toys.gallery.stick;
                if self.toy_held() & PAD_A != 0 && reg(s[0]) != 0.0 && reg(s[0]) < 0.0 {
                    self.game.toys.gallery.pan[0] = stick_pan(s[0]);
                }
                if self.toy_held() & PAD_A != 0 && reg(s[0]) != 0.0 && reg(s[0]) > 0.0 {
                    self.game.toys.gallery.pan[0] = stick_pan(s[0]);
                }
                if self.toy_held() & PAD_A != 0 && reg(s[1]) != 0.0 && reg(s[1]) > 0.0 {
                    self.game.toys.gallery.pan[1] = stick_pan(s[1]);
                }
                if self.toy_held() & PAD_A != 0 && reg(s[1]) != 0.0 && reg(s[1]) < 0.0 {
                    self.game.toys.gallery.pan[1] = stick_pan(s[1]);
                }
                let p = self.game.toys.gallery.pan;
                let hide = if reg(p[0]) != 0.0 || reg(p[1]) != 0.0 || self.toy_held() & PAD_A != 0 {
                    0
                } else {
                    1
                };
                self.toy_axes_hidden(hide);
                if reg(p[0]) != 0.0 || reg(p[1]) != 0.0 {
                    moved = true;
                }
                if !moved {
                    let g = &self.game.toys.gallery;
                    let z = reg(g.dist);
                    let new = if self.toy_held() & PAD_Y != 0 {
                        zoomed = true;
                        Some(fp::fnmsubs(reg(0.025), reg(dist), z))
                    } else if self.toy_held() & PAD_X != 0 {
                        zoomed = true;
                        Some(fp::fnmsubs(reg(-0.025), reg(dist), z))
                    } else {
                        let y = reg(self.game.toys.gallery.stick[1]);
                        if y != 0.0 {
                            zoomed = true;
                            Some(fp::fnmsubs(reg(dist), fp::fmuls(reg(0.025), y), z))
                        } else {
                            None
                        }
                    };
                    let g = &mut self.game.toys.gallery;
                    if let Some(n) = new {
                        g.dist = mem(n);
                    }
                    if reg(g.dist) < reg(5.0) {
                        g.dist = 5.0;
                    }
                    if reg(g.dist) > reg(250.0) {
                        g.dist = 250.0;
                    }
                }
                rotate = true;
            }
            _ => {}
        }
        if rotate {
            let g = &mut self.game.toys.gallery;
            g.yaw = mem(fp::fnmsubs(reg(3.0), reg(g.cstick[0]), reg(g.yaw)));
            g.pitch = mem(fp::fnmsubs(reg(3.0), reg(g.cstick[1]), reg(g.pitch)));
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
            if fp::fadds(reg(g.cstick[0]), reg(g.cstick[1])) != 0.0 {
                g.spin = 0.0;
            }
            if !moved && !zoomed && self.toy_total() > 1 {
                self.toy_cycle();
            }
            if self.toy_triggered() & PAD_START != 0 {
                self.lb_audio_call(2);
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
            if self.toy_triggered() & PAD_Z != 0 {
                self.toy_view_reset(false);
                self.game.toys.gallery.light = 0;
                self.toy_set_light(0);
                self.toy_back(0);
                let a = &mut self.game.toys.anim;
                a.x11 = 2;
                a.x10 = 2;
                a.x0f = 0;
                a.x0e = 0;
                self.toy_stand_look(2, false);
            }
            self.toy_stand_by_view(c);
        }
        let p = self.game.toys.gallery.pan;
        self.toy_view_place(p[0], p[1]);
        let g = &mut self.game.toys.gallery;
        g.stick_prev = g.stick;
        g.cstick_prev = g.cstick;
    }

    /// The view's change of trophy: with the stick past 0.9 or L or R, the one before (the
    /// stick to the left, L, X or the D-pad's left) or after (the stick to the right, R, Y or
    /// the D-pad's right), the ring of loaded models turned with it beyond three trophies;
    /// the view put back, the trophy shown, its new mark cleared, its texts and mark, and
    /// twenty frames before the next input.
    fn toy_cycle(&mut self) {
        let sx = self.game.toys.gallery.stick[0];
        if reg(sx).abs().partial_cmp(&reg(0.9)) != Some(std::cmp::Ordering::Greater)
            && self.toy_triggered() & 0x60 == 0
        {
            return;
        }
        let total = self.toy_total();
        if reg(sx) < 0.0 || self.toy_triggered() & 0x441 != 0 {
            self.lb_audio_call(2);
            let g = &mut self.game.toys.gallery;
            g.selected -= 1;
            if g.selected < 0 {
                g.selected = (total - 1) as i16;
            }
            if total > 3 {
                let g = &self.game.toys.gallery;
                let sel = i32::from(g.selected);
                let t = if sel - 1 < 0 {
                    g.order[(total + sel - 1) as usize]
                } else {
                    g.order[(sel - 1) as usize]
                };
                let e = g.entries[usize::from(g.first)].prev;
                self.toy_entry_set(e, t);
                self.toy_entry_load(e);
                let last = self.game.toys.gallery.last;
                if let Some(a) = self.game.toys.gallery.entries[usize::from(last)]
                    .archive
                    .take()
                {
                    self.toy_free(a);
                }
                let g = &mut self.game.toys.gallery;
                g.selected_entry = g.entries[usize::from(g.selected_entry)].prev;
                g.first = g.entries[usize::from(g.first)].prev;
                g.last = g.entries[usize::from(g.last)].prev;
            } else {
                let g = &mut self.game.toys.gallery;
                g.selected_entry = g.entries[usize::from(g.selected_entry)].prev;
            }
        } else if reg(sx) > 0.0 || self.toy_triggered() & 0x822 != 0 {
            self.lb_audio_call(2);
            let g = &mut self.game.toys.gallery;
            g.selected += 1;
            if i32::from(g.selected) >= total {
                g.selected = 0;
            }
            if total > 3 {
                let g = &self.game.toys.gallery;
                let sel = i32::from(g.selected);
                let t = if sel + 1 >= total {
                    g.order[(sel - total + 1) as usize]
                } else {
                    g.order[(sel + 1) as usize]
                };
                let e = g.entries[usize::from(g.last)].next;
                self.toy_entry_set(e, t);
                self.toy_entry_load(e);
                let first = self.game.toys.gallery.first;
                if let Some(a) = self.game.toys.gallery.entries[usize::from(first)]
                    .archive
                    .take()
                {
                    self.toy_free(a);
                }
                let g = &mut self.game.toys.gallery;
                g.selected_entry = g.entries[usize::from(g.selected_entry)].next;
                g.first = g.entries[usize::from(g.first)].next;
                g.last = g.entries[usize::from(g.last)].next;
            } else {
                let g = &mut self.game.toys.gallery;
                g.selected_entry = g.entries[usize::from(g.selected_entry)].next;
            }
        } else {
            return;
        }
        self.toy_view_reset(false);
        self.toy_view_place(0.0, 0.0);
        self.game.toys.gallery.idle = 0x95E;
        let e = self.game.toys.gallery.selected_entry;
        self.toy_show_entry(e);
        self.toy_clear_new();
        let t = self.game.toys.gallery.entries[usize::from(e)].trophy;
        self.toy_panel_texts(i32::from(t));
        let mark = self.game.toys.gallery.panel_parts[8];
        self.toy_mark(mark, i32::from(t));
        self.game.toys.gallery.cooldown = 0x14;
    }
}

/// `HSD_CObjSetTop`, `SetBottom`, `SetRight`, `SetLeft` by side (0 to 3).
fn set_side(cobjs: &mut cobj::CObjs, c: cobj::CObjId, which: u8, v: f32) {
    match which {
        0 => cobj::set_top(cobjs, Some(c), v),
        1 => cobj::set_bottom(cobjs, Some(c), v),
        2 => cobj::set_right(cobjs, Some(c), v),
        _ => cobj::set_left(cobjs, Some(c), v),
    }
}
