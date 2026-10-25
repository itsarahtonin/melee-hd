//! The off-screen magnifier's drawing (`melee/if/ifmagnify.c`): each frame its camera
//! renders every fighter off the screen into that player's bubble texture, close up, and
//! each bubble turns toward its fighter at the screen's edge.
//!
//! What it changes is state: which players are off screen (`is_offscreen`, which fighters
//! read) and the edge, the camera's aim, the bubbles' joints, the arrows the HUD shows, and
//! what drawing the fighters changes. The textures' pixels and the erase color the camera
//! blends from the stage's colors are the presentation's.

use gekko_fp as fp;

use melee_data::archive::{ArchiveId, DataRef};

use super::super::handles::CodeRef;
use super::super::mirror::{Cx, Mem, Mirror};
use super::super::state::{OBJ_CAMERA, OBJ_JOBJ, Obj, Running, UserData};
use super::super::structs::{HSD_ImageDesc, ifMagnify};
use crate::fpu::{mem, reg};
use crate::hsd::display::{self, Draw};
use crate::hsd::gobj::{self, GObjId};
use crate::hsd::tobj::ImageRef;
use crate::hsd::{cobj, jobj};
use crate::lb::cobj::{CObjDesc, load_cobj};
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

/// `ifMagnify_804A1DE0`'s address, for the bubbles' image copies (`image_descs`).
const MAGNIFY: u32 = 0x804A_1DE0;
/// Where `image_descs` starts in it, and an image descriptor's size.
const IMAGE_DESCS: u32 = 0x74;
const IMAGE_DESC_SIZE: u32 = 0x18;

/// The magnifier camera's descriptor, `ifMagnify_803F97E8`: orthographic over the screen, its
/// eye 300 in front.
const CAMERA_DESC: cobj::StaticDesc = cobj::StaticDesc {
    flags: 0,
    projection_type: 3,
    viewport: [0, 640, 0, 480],
    scissor: [0, 640, 0, 480],
    eye: Vec3::new(0.0, 0.0, 300.0),
    interest: Vec3::new(0.0, 0.0, 0.0),
    roll: 0.0,
    near: 0.1,
    far: 32768.0,
    projection: [0.0, 0.0, 0.0, 0.0],
};

/// A bubble's texture's image (`ifMagnifyPlayer::idesc`): the model's own, in `IfAll.dat`,
/// for the first player, and the magnifier's copies (`image_descs`, static) for the others.
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct MagnifyImage(pub Option<ImageRef>);

impl Mirror for MagnifyImage {
    const SIZE: u32 = 4;

    fn import(m: &Mem, at: u32, cx: &mut dyn Cx) -> Self {
        MagnifyImage(match m.u32(at) {
            0 => None,
            p => Some(match cx.import_data(p) {
                Some(d) => ImageRef::Data(d),
                None => ImageRef::Static(p),
            }),
        })
    }

    fn compare(&self, m: &Mem, at: u32, path: &str, cx: &mut dyn Cx) {
        let theirs = m.u32(at);
        match self.0 {
            Some(ImageRef::Data(d)) => cx.compare_data(path, Some(d), theirs),
            Some(ImageRef::Static(a)) if a == theirs => {}
            None if theirs == 0 => {}
            ours => cx.mismatch(path, format!("{ours:?}"), format!("{theirs:#x}")),
        }
    }
}

/// An image's pixels (`HSD_ImageDesc::image_ptr`) in the magnifier's copies: a buffer from the
/// heap the bubbles render into, which only the presentation reads, so only whether there is
/// one is state (see engine.md).
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct TexBuffer(pub bool);

impl Mirror for TexBuffer {
    const SIZE: u32 = 4;

    fn import(m: &Mem, at: u32, _: &mut dyn Cx) -> Self {
        TexBuffer(m.u32(at) != 0)
    }

    fn compare(&self, m: &Mem, at: u32, path: &str, cx: &mut dyn Cx) {
        let theirs = m.u32(at) != 0;
        if self.0 != theirs {
            cx.mismatch(path, format!("{}", self.0), format!("{theirs}"));
        }
    }
}

impl Running<'_> {
    /// Sets the off-screen magnifier up, `ifMagnify_802FC870`: cleared, the stage's four
    /// colors (`ifMagnify_802FC7C0`), `IfAll.dat`'s `lupe` model, each player's bubble and
    /// the magnifier's camera.
    #[doc(alias = "ifMagnify_802FC870")]
    #[doc(alias = "ifMagnify_802FC7C0")]
    pub fn create_magnifier(&mut self, if_all: ArchiveId) {
        let keep = self.game.magnify.image_descs.clone();
        self.game.magnify = ifMagnify {
            image_descs: keep,
            ..Default::default()
        };
        // `Ground_801C0604` and the others hand back the stage parameters' colors.
        let p = self
            .game
            .gr
            .param
            .as_ref()
            .expect("stage_info.param is null");
        let colors = (p.xB8.clone(), p.xC0.clone(), p.xD0.clone(), p.xD8.clone());
        let m = &mut self.game.magnify;
        (m.x4, m.x8, m.xC, m.x10) = colors;
        let lupe = self
            .data
            .get(if_all)
            .public("lupe")
            .expect("Cannot find symbol lupe.");
        self.game.magnify.model_desc = Some(DataRef {
            archive: if_all,
            offset: lupe,
        });
        for slot in 0..6 {
            self.create_bubble(slot);
        }
        self.create_magnifier_camera();
    }

    /// A player's bubble, `ifMagnify_802FC3C0`: the `lupe` model on a GObj of its own, its
    /// texture the model's image for the first player and a copy of it on the heap for the
    /// others, its arrow and its frame in the player's color.
    #[doc(alias = "ifMagnify_802FC3C0")]
    fn create_bubble(&mut self, slot: u8) {
        let i = usize::from(slot);
        if let Some(g) = self.game.magnify.player[i].gobj {
            gobj::free(self, g);
        }
        let data = self.data;
        let g = &mut *self.game;
        let gobj = g.world.create(0xE, 15, 0);
        g.world
            .init_user_data(gobj, 0xE, UserData::MagnifyPlayer(slot));
        let desc = g.magnify.model_desc.expect("the magnifier's model");
        let model = data.ptr(desc).expect("lupe's model");
        let j = jobj::load_joint(&mut g.objs, data, data.ptr(model)).expect("lupe's joint");
        g.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(j));
        g.world.setup_gx_link(gobj, CodeRef(BUBBLE_RENDER), 0xB, 0);
        let child = super::super::gm::pause::tree_nodes(&g.objs.jobjs, Some(j), &[2])[0]
            .expect("the bubble's image joint");
        let d = g.objs.jobjs[child].dobj().expect("child->u.dobj");
        let d = g.objs.dobjs[d].next.expect("dobj->next");
        let m = g.objs.dobjs[d].mobj.expect("dobj->mobj");
        let t = g.objs.mobjs[m].tobj.expect("mobj->tobj");
        if slot == 0 {
            g.magnify.player[0].idesc = MagnifyImage(g.objs.tobjs[t].imagedesc);
        } else {
            let Some(ImageRef::Data(src)) = g.magnify.player[0].idesc.0 else {
                panic!("the first bubble's image isn't the model's");
            };
            // The copy's pixels: a buffer `HSD_MemAlloc` gives (`GXGetTexBufferSize`), which
            // the scene's heap keeps until the scene ends. Only the GPU writes it, when the
            // bubble is copied: until then it holds what the block held.
            let size = crate::hsd::heap::tex_buffer_size(
                u32::from(data.u16(src.at(4))),
                u32::from(data.u16(src.at(6))),
                data.u32(src.at(8)),
            )
            .next_multiple_of(32);
            let at = crate::hsd::heap::mem_alloc_unclaimed(size);
            g.magnify_buffers[i] = (at, size);
            g.magnify.image_descs[i - 1] = HSD_ImageDesc {
                image_ptr: TexBuffer(true),
                width: data.u16(src.at(4)),
                height: data.u16(src.at(6)),
                format: data.u32(src.at(8)) as i32,
                mipmap: data.u32(src.at(0xC)),
                minLOD: data.f32(src.at(0x10)),
                maxLOD: data.f32(src.at(0x14)),
            };
            let at = MAGNIFY + IMAGE_DESCS + IMAGE_DESC_SIZE * (u32::from(slot) - 1);
            g.magnify.player[i].idesc = MagnifyImage(Some(ImageRef::Static(at)));
            g.objs.tobjs[t].imagedesc = Some(ImageRef::Static(at));
        }
        let arrow = super::super::gm::pause::tree_nodes(&g.objs.jobjs, Some(j), &[1])[0]
            .expect("the bubble's arrow joint");
        g.magnify.player[i].jobj = Some(arrow);
        let color = super::super::gm::vs::hud_color(super::super::gm::vs::player_color(
            slot,
            g.player_team(i32::from(slot)) as u8,
            u8::from(g.vs_is_teams()),
            g.player_pkind(i32::from(slot)) as u8,
        ));
        for jj in [arrow, child] {
            let d = g.objs.jobjs[jj].dobj().expect("u.dobj");
            let m = g.objs.dobjs[d].mobj.expect("dobj->mobj");
            let mat = g.objs.mobjs[m].mat.as_mut().expect("mobj->mat");
            mat.diffuse.r = color[0];
            mat.diffuse.g = color[1];
            mat.diffuse.b = color[2];
        }
        let p = &mut g.magnify.player[i];
        p.gobj = Some(gobj);
        p.state.is_offscreen = false;
        p.state.ignore_offscreen = false;
    }

    /// The magnifier's camera, `ifMagnify_802FC618`: orthographic over the first bubble's
    /// image at a tenth of its size, its viewport and scissor the image's.
    #[doc(alias = "ifMagnify_802FC618")]
    fn create_magnifier_camera(&mut self) {
        let data = self.data;
        let g = &mut *self.game;
        let gobj = g.world.create(0xE, 15, 0);
        let c = load_cobj(&mut g.objs, data, CObjDesc::Static(&CAMERA_DESC));
        g.world.set_obj(gobj, OBJ_CAMERA, Obj::CObj(c));
        g.world
            .setup_gx_link_max(gobj, CodeRef(CAMERA_RENDER), 0, false);
        g.world.gobj_mut(gobj).gxlink_prios = 0x10;
        let Some(ImageRef::Data(src)) = g.magnify.player[0].idesc.0 else {
            panic!("the first bubble's image isn't the model's");
        };
        let (w, h) = (data.u16(src.at(4)), data.u16(src.at(6)));
        let scale = reg(0.1);
        let half_h = mem(fp::fmuls(scale, fp::frsp(f64::from(h))));
        let half_w = mem(fp::fmuls(scale, fp::frsp(f64::from(w))));
        let neg = |v: f32| mem(fp::fneg(reg(v)));
        cobj::set_ortho(
            &mut g.objs.cobjs,
            Some(c),
            half_h,
            neg(half_h),
            neg(half_w),
            half_w,
        );
        cobj::set_viewport(&mut g.objs.cobjs, Some(c), [0, w as i16, 0, h as i16]);
        cobj::set_scissor(&mut g.objs.cobjs, Some(c), [0, w, 0, h]);
    }

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
            self.game
                .objs
                .display
                .draws
                .push(Draw::MagnifierErase(i as u8));
            self.game.world.rendering = Some(fg);
            display::rendering(&mut self.game.objs, Some(fg));
            for pass in 0..3 {
                self.draw_magnified(fg, pass);
            }
            self.game.world.rendering = None;
            display::rendering(&mut self.game.objs, None);
            self.game
                .objs
                .display
                .draws
                .push(Draw::MagnifierCopy(i as u8));
            // The GPU's pixels in the bubble's buffer, which the simulation never reads.
            let (at, size) = self.game.magnify_buffers[i];
            if at != 0 {
                crate::hsd::heap::with(|h| {
                    h.image
                        .hold(at, at + size, crate::hsd::image::Src::RenderTarget)
                });
            }
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
