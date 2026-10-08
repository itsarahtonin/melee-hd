//! The off-screen magnifier's drawing (`melee/if/ifmagnify.c`): each frame its camera
//! renders every fighter off the screen into that player's bubble texture, close up, and
//! each bubble turns toward its fighter at the screen's edge.
//!
//! What it changes is state: which players are off screen (`is_offscreen`, which fighters
//! read) and the edge, the camera's aim, the bubbles' joints, the arrows the HUD shows, and
//! what drawing the fighters changes. The textures' pixels and the erase color the camera
//! blends from the stage's colors are the presentation's.

use gekko_fp as fp;

use super::super::state::{Obj, Running, UserData};
use crate::fpu::{mem, reg};
use crate::hsd::display::{self, Draw};
use crate::hsd::gobj::GObjId;
use crate::hsd::{cobj, jobj};
use crate::math;
use crate::vec::Vec3;

/// `ifMagnify_802FBBDC`, the magnifier camera's render callback.
pub const CAMERA_RENDER: u32 = 0x802F_BBDC;
/// `ifMagnify_802FB8C0`, a player's bubble's render callback.
pub const BUBBLE_RENDER: u32 = 0x802F_B8C0;
/// `ifMagnify_802FC3BC`, the bubbles' user data's release, which does nothing.
pub const BUBBLE_FREE: u32 = 0x802F_C3BC;

/// How far the bubbles sit from the screen's center, horizontally and vertically.
const EDGE_X: f32 = 252.700_01;
const EDGE_Y: f32 = 162.7;
/// `EDGE_Y / EDGE_X`: the corners' slope.
const CORNER: f32 = 0.643_846_4;

impl Running<'_> {
    /// Whether the magnifier shows anything, `ifMagnify_IsHUDVisible`: the match shows its
    /// HUD, nothing hides it, and no boss intro plays.
    fn magnifier_visible(&self) -> bool {
        self.game.vs.state.hud_enabled != 0
            && !self.game.is_hud_hidden()
            && !super::super::camera::is_boss_intro(&self.game.camera.cam)
    }

    /// The magnifier camera's render callback, `ifMagnify_802FBBDC`: no player starts off
    /// screen; then, with the HUD shown, the camera draws its links, and for each fighter
    /// off screen that it may magnify, aims at the fighter's camera box scaled to its
    /// size, clears, draws the fighter's three passes and copies the render into the
    /// player's bubble, marking the player off screen.
    #[doc(alias = "ifMagnify_802FBBDC")]
    pub fn render_magnifier(&mut self, gobj: GObjId) {
        for p in &mut self.game.magnify.player {
            p.state.is_offscreen = false;
        }
        if !self.magnifier_visible() {
            return;
        }
        let Some(Obj::CObj(c)) = self.game.world.gobj(gobj).obj else {
            panic!("the magnifier's GObj without its camera");
        };
        let [top, bottom, left, right] =
            cobj::get_ortho(&self.game.objs.cobjs, Some(c)).expect("an orthographic camera");
        if display::camera_begin(&mut self.game.objs, self.data, Some(c)) {
            self.render_links(gobj, 7);
            display::camera_end(&mut self.game.objs);
        }
        for i in 0..6 {
            let Some(fg) = self.game.player_entity(i as i32) else {
                continue;
            };
            if self.game.magnify.player[i].state.ignore_offscreen
                || !self.game.fighter(fg).x221F_b0
                || !self.can_be_magnified(fg)
            {
                continue;
            }
            let b = self.game.fighter(fg).x890_cameraBox.expect("a camera box");
            let b = &self.game.camera.boxes[b];
            let (ext, bone) = (b.target_ext.v.z, b.bone_pos);
            let scale = fp::fdivs(reg(ext), 8.0);
            let s = |v: f32| mem(fp::fmuls(reg(v), scale));
            let cs = &mut self.game.objs.cobjs;
            cobj::set_ortho(cs, Some(c), s(top), s(bottom), s(left), s(right));
            cobj::set_interest(&mut self.game.objs, c, bone);
            let eye = Vec3::new(bone.x, bone.y, 300.0);
            cobj::set_eye_position(&mut self.game.objs, c, eye);
            if !display::camera_begin(&mut self.game.objs, self.data, Some(c)) {
                continue;
            }
            self.game.objs.display.draws.push(Draw::Erase);
            self.game.world.rendering = Some(fg);
            for pass in 0..3 {
                self.draw_magnified(fg, pass);
            }
            self.game.world.rendering = None;
            self.game
                .objs
                .display
                .draws
                .push(Draw::MagnifierCopy(i as u8));
            display::camera_end(&mut self.game.objs);
            self.game.magnify.player[i].state.is_offscreen = true;
        }
        cobj::set_ortho(&mut self.game.objs.cobjs, Some(c), top, bottom, left, right);
    }

    /// Whether the magnifier may show the fighter, `ftLib_CanBeMagnified`.
    #[doc(alias = "ftLib_CanBeMagnified")]
    fn can_be_magnified(&self, g: GObjId) -> bool {
        let fp = self.game.fighter(g);
        !(fp.invisible
            || fp.x221E_b2
            || self.game.player_more_flags_bit4(i32::from(fp.player_idx))
            || fp.is_sandbag
            || fp.x2229_b3
            || fp.x2220_b7)
    }

    /// A player's bubble's render callback, `ifMagnify_802FB8C0`: in the first pass, for a
    /// fighter off screen with the HUD shown, the bubble turns toward the fighter's screen
    /// position, moves to the screen's edge it crosses, and draws; past a side, the
    /// player's arrow shows in their color, else it hides.
    #[doc(alias = "ifMagnify_802FB8C0")]
    pub fn render_bubble(&mut self, gobj: GObjId, pass: i32) {
        if pass != 0 {
            return;
        }
        let Some(UserData::MagnifyPlayer(slot)) = self.game.world.gobj(gobj).user_data else {
            panic!("a bubble without its player");
        };
        let i = usize::from(slot);
        let mut colored = false;
        let fg = self.game.player_entity(i32::from(slot));
        if self.magnifier_visible()
            && self.game.magnify.player[i].state.is_offscreen
            && let Some(fg) = fg
        {
            let fp = self.game.fighter(fg);
            // `ftLib_GetScreenPos` leaves the position unwritten for these.
            assert!(
                !fp.x2229_b3 && !fp.x2220_b7,
                "ifMagnify_802FB8C0: a screen position ftLib_GetScreenPos doesn't write"
            );
            let sp = &fp.x2188;
            let dx = mem(fp::fsubs(fp::frsp(f64::from(sp.x)), 320.0));
            let dy = mem(fp::fneg(fp::fsubs(fp::frsp(f64::from(sp.y)), 240.0)));
            let p = &self.game.magnify.player[i];
            let (arrow, root) = (p.jobj.expect("the bubble's arrow joint"), p.gobj);
            jobj::set_rotation_z(&mut self.game.objs.jobjs, arrow, math::atan2(dy, dx));
            let (ex, ey) = self.bubble_edge(i, dx, dy);
            let t = Vec3::new(
                mem(fp::fmuls(reg(0.091_25), reg(ex))),
                mem(fp::fmuls(reg(0.1), reg(ey))),
                0.0,
            );
            let root = root.and_then(|g| match self.game.world.gobj(g).obj {
                Some(Obj::JObj(j)) => Some(j),
                _ => None,
            });
            jobj::set_translate(&mut self.game.objs.jobjs, root.expect("the bubble"), t);
            self.render_jobj(gobj, pass);
            let edge = self.game.magnify.player[i].state.edge;
            if edge == 4 || edge == 2 {
                let g = &*self.game;
                let color = super::super::gm::vs::hud_color(super::super::gm::vs::player_color(
                    slot,
                    g.player_team(i32::from(slot)) as u8,
                    u8::from(g.vs_is_teams()),
                    g.player_pkind(i32::from(slot)) as u8,
                ));
                let kind = if edge == 2 { 1 } else { 2 };
                self.game.show_arrow(slot, kind, color);
                colored = true;
            }
        }
        if !colored {
            self.game.hide_arrow(slot);
        }
    }

    /// Where the bubble sits for a fighter in direction (`x`, `y`) from the screen's
    /// center, and which edge that is (1 top, 2 left, 3 bottom, 4 right), `ifMagnify_802FB73C`.
    #[doc(alias = "ifMagnify_802FB73C")]
    fn bubble_edge(&mut self, slot: usize, x: f32, y: f32) -> (f32, f32) {
        let sign = |pos: bool, v: f32| if pos { v } else { -v };
        let clamp = |v: f64, lim: f32| {
            if v < reg(-lim) {
                -lim
            } else if v > reg(lim) {
                lim
            } else {
                mem(v)
            }
        };
        let (ox, oy) = if x == 0.0 {
            (0.0, sign(y > 0.0, EDGE_Y))
        } else {
            let ratio = fp::fdivs(reg(y), reg(x));
            if ratio > reg(CORNER) || ratio < reg(-CORNER) {
                let oy = sign(y > 0.0, EDGE_Y);
                let v = fp::fdivs(fp::fmuls(reg(oy), reg(x)), reg(y));
                (clamp(v, EDGE_X), oy)
            } else {
                let ox = sign(x > 0.0, EDGE_X);
                let v = fp::fdivs(fp::fmuls(reg(ox), reg(y)), reg(x));
                (ox, clamp(v, EDGE_Y))
            }
        };
        let edge = if ox == -EDGE_X {
            2
        } else if ox == EDGE_X {
            4
        } else if oy == EDGE_Y {
            1
        } else {
            3
        };
        self.game.magnify.player[slot].state.edge = edge;
        (ox, oy)
    }
}
