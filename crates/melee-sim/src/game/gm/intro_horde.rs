//! Classic's horde stage's intro (`gm_1832.c`'s `fn_80185A0C` and its callbacks, the intro's
//! kind 4): the opponent's costumes, up to three, posed asleep at the screen's middle and drawn
//! one at a time by a camera of the map's scene into a color and a depth image each
//! (`fn_801852FC`, the frame's 0x17C by 0x190 pixels from x 0x82); once copied, ten sprites of
//! those images, alternating through them, scattered over a grid at random and shown one every
//! eight ticks (`fn_801857C4`, `fn_801859C8`), over a depth quad that keeps them behind the
//! screen's left (`fn_8018564C`).

use gekko_fp as fp;

use super::super::handles::CodeRef;
use super::super::state::{OBJ_CAMERA, Obj, Running};
use super::endings::toy_fall::{FMT_COLOR, FMT_DEPTH, Frame};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::sobj::{self, OBJ_SOBJ};
use crate::vec::Vec3;

/// The callbacks: the sprites' camera's render (`fn_8018564C`), the copies' camera's render
/// (`fn_801852FC`), the sprites' process (`fn_801859C8`) and the splash's (`fn_801857C4`).
pub const SPRITE_CAMERA_RENDER: u32 = 0x8018_564C;
pub const COPY_CAMERA_RENDER: u32 = 0x8018_52FC;
pub const SPRITES_PROC: u32 = 0x8018_59C8;
pub const SPLASH_PROC: u32 = 0x8018_57C4;

/// `lbl_804735E8`'s images: 0x17C by 0x190, copied from x 0x82.
pub const COPY_W: u16 = 0x17C;
pub const COPY_H: u16 = 0x190;
pub const COPY_X: u16 = 0x82;
/// `&lbl_804735E8.x40[0]`, the first image's descriptor (the depth ones follow at 0x48).
pub const IMAGES_AT: u32 = 0x8047_35E8;

/// `gm_1832_CharScale`, `gm_1832_CharOffset` and `gm_1832_SamusOffset` (which is the Ice
/// Climbers' Nana's, `CKind_PopoNana`).
const SCALE: [f32; 28] = [
    0.6, 0.35, 0.6, 0.5, 0.6, 0.35, 0.6, 0.6, 0.7, 0.6, 0.5, 0.6, 0.6, 0.6, 0.5, 0.5, 0.6, 0.5,
    0.6, 0.6, 0.5, 0.6, 0.6, 0.6, 0.6, 0.5, 0.5, 0.5,
];
const OFFSET: [[f32; 2]; 28] = [
    [0.0, -6.0],
    [0.0, -3.5],
    [-1.0, -3.5],
    [0.0, -3.5],
    [0.0, -1.0],
    [0.0, -3.0],
    [0.0, -5.0],
    [0.0, -3.5],
    [0.0, -3.5],
    [0.0, -3.5],
    [0.0, -3.0],
    [-0.5, -3.5],
    [-2.0, -6.0],
    [0.0, -2.5],
    [0.0, -2.5],
    [-1.0, -1.0],
    [0.0, -5.0],
    [0.0, -3.0],
    [-1.0, -6.0],
    [-1.0, -2.5],
    [-1.0, -3.5],
    [0.0, -3.5],
    [0.0, -4.0],
    [-1.0, -3.5],
    [0.0, -1.5],
    [0.0, -4.5],
    [0.0, -3.5],
    [0.0, -3.5],
];
const NANA_OFFSET: Vec3 = Vec3 {
    x: 3.0,
    y: 4.0,
    z: -5.0,
};
const CKIND_POPONANA: u8 = 0xE;

/// `lbl_804735E8` and `lbl_804D65F0`: the splash's state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct IntroHorde {
    /// `x40[0..3]` the color images, `x88[0..3]` the depth ones (`lb_800121FC`).
    pub images: [Frame; 6],
    /// `xD0`: each sprite's image, back and forth through the costumes.
    pub order: [u8; 10],
    /// `xDC`: the sprites' GObj.
    pub sprites: Option<GObjId>,
    /// `xE0`: the costumes shown (at most 3); `xE1`: the copies made.
    pub count: u8,
    pub copied: u8,
    /// `lbl_804D65F0`: the copies' camera.
    pub copy_camera: Option<GObjId>,
}

/// `fn_801855BC`: a square root by four Newton steps from `frsqrte`, 0 at 0; a negative or
/// NaN's is MSL's NaN constant (`MSL_TrigF_80400770`), which its one caller's 10 never asks.
#[doc(alias = "fn_801855BC")]
pub fn sqrt(x: f64) -> f64 {
    if x > 0.0 {
        let mut g = fp::frsqrte(x);
        for _ in 0..4 {
            g = fp::fmul(fp::fmul(0.5, g), fp::fsub(3.0, fp::fmul(fp::fmul(g, g), x)));
        }
        fp::fmul(x, g)
    } else if x == 0.0 {
        0.0
    } else {
        f64::from(f32::NAN)
    }
}

impl Running<'_> {
    fn horde(&mut self) -> &mut IntroHorde {
        &mut self.game.intros.horde
    }

    /// An image of the scene's heap (`lb_800121FC` with no preload entry): only the GPU's
    /// copies write it, so the block keeps what it held until the first.
    fn horde_image(&mut self, n: usize, format: u32) {
        let size = crate::hsd::heap::tex_buffer_size(u32::from(COPY_W), u32::from(COPY_H), format)
            .next_multiple_of(32);
        let addr = crate::hsd::heap::mem_alloc_unclaimed(size);
        self.horde().images[n] = Frame {
            addr,
            size,
            width: COPY_W,
            height: COPY_H,
            format,
        };
    }

    /// The splash's setup, `fn_80185A0C`: the sprites' camera over the screen at render
    /// priority 11 drawing with the depth quad first, the sprites' GObj on link 0x11 with
    /// their process (marked as run this pass), the costumes shown, the images, the sprites'
    /// order, the splash's process, the copies' camera (the map scene's camera, render
    /// priority 0 over links 0, 5 and 6), and the opponents (`fn_801851C0`).
    #[doc(alias = "fn_80185A0C")]
    pub(super) fn intro_easy_horde(&mut self) -> i32 {
        let g = self.game.world.create(0x13, 0x14, 0);
        // `HSD_SObjLib_803A55DC`, then its link taken back for this one at the same
        // priority: the same place on the list.
        self.sprite_camera_with(g, 0x280, 0x1E0, 0xB, false, CodeRef(SPRITE_CAMERA_RENDER));
        self.game.world.gobj_mut(g).gxlink_prios = 0x20000;
        let w = &mut self.game.world;
        let g2 = w.create(0xE, 0xF, 0);
        w.set_obj(g2, OBJ_SOBJ, Obj::SObj(None));
        w.setup_gx_link(g2, CodeRef(sobj::RENDER), 0x11, 0);
        let p = w.setup_proc(g2, CodeRef(SPRITES_PROC), 0);
        w.proc_mut(p).flags_3 = w.phase;
        let kind = self.game.intros.easy.data.enemy_kinds[0];
        let count = crate::game::playback::costume_count(kind).min(3);
        let h = self.horde();
        h.count = count;
        h.sprites = Some(g2);
        h.copied = 0;
        for i in 0..usize::from(count) {
            self.horde_image(i, FMT_COLOR);
            self.horde_image(3 + i, FMT_DEPTH);
        }
        let h = self.horde();
        let n = i32::from(count);
        for k in 0..10i32 {
            h.order[k as usize] = if (k / n) % 2 != 0 {
                ((n - 1) - (k % n)) as u8
            } else {
                (k % n) as u8
            };
        }
        let w = &mut self.game.world;
        let pg = w.create(0x13, 1, 0);
        w.setup_proc(pg, CodeRef(SPLASH_PROC), 0);
        let data = self.data;
        let map = self.game.intros.easy.map_scene.expect("the map's scene");
        let cams = data.ptr(map.at(4)).expect("the map's cameras");
        let g3 = self.game.world.create(0x13, 0x14, 0);
        let c = crate::hsd::cobj::load_desc(&mut self.game.objs, data, data.ptr(cams))
            .expect("a camera");
        let w = &mut self.game.world;
        w.set_obj(g3, OBJ_CAMERA, Obj::CObj(c));
        w.setup_gx_link_max(g3, CodeRef(COPY_CAMERA_RENDER), 0, false);
        w.gobj_mut(g3).gxlink_prios = 0x61;
        self.horde().copy_camera = Some(g3);
        self.intro_easy_horde_fighters()
    }

    /// The opponent's costumes from slot 1, `fn_801851C0`: each the opponent's character in
    /// the next costume, a demo player facing 0 at its character's offset (the Ice Climbers'
    /// Nana further out), at its character's scale, metal as the opponent, asleep (pose 6,
    /// `Player_80037054`); the last slot.
    #[doc(alias = "fn_801851C0")]
    #[doc(alias = "Player_80037054")]
    #[doc(alias = "ftData_800858E4")]
    fn intro_easy_horde_fighters(&mut self) -> i32 {
        let d = self.game.intros.easy.data;
        let kind = d.enemy_kinds[0];
        let mut slot = 0;
        for i in 0..i32::from(self.horde().count) {
            slot = i + 1;
            let g = &mut *self.game;
            g.set_player_ckind(slot, i32::from(kind));
            g.set_player_port(slot, 0);
            g.set_player_pkind(slot, super::super::pl::player::PKIND_DEMO);
            g.set_player_facing(slot, 0.0);
            g.set_player_costume(slot, i);
            let [x, y] = OFFSET[usize::from(kind)];
            let mut pos = Vec3::new(x, y, 0.0);
            g.set_player_pos(slot, pos);
            if kind == CKIND_POPONANA {
                pos.x = mem(fp::fadds(reg(pos.x), reg(NANA_OFFSET.x)));
                pos.y = mem(fp::fadds(reg(pos.y), reg(NANA_OFFSET.y)));
                pos.z = mem(fp::fadds(reg(pos.z), reg(NANA_OFFSET.z)));
                g.set_player_entity_pos(slot, 1, pos);
            }
            g.set_player_model_scale(slot, SCALE[usize::from(kind)]);
            g.set_player_is_metal(slot, d.enemy_metal[0]);
            // `Player_80037054` is `Player_80036F34` with the demo's `xC.b0` set, which picks
            // `ftData_800858E4` for the costume, the same load as `ftData_80085820`.
            self.player_create_demo(slot, 6);
        }
        slot
    }

    /// The copies' camera's render callback, `fn_801852FC`: through the camera, for each
    /// costume, only its fighter shown (`Player_800366DC`), the fighters' passes, and the
    /// frame copied into its color image and its depth image (cleared after); then the copies
    /// marked made, drawn or not.
    #[doc(alias = "fn_801852FC")]
    pub fn intro_horde_copy_render(&mut self, gobj: GObjId) {
        let Some(Obj::CObj(c)) = self.game.world.gobj(gobj).obj else {
            panic!("the splash's camera holds no CObj");
        };
        if crate::hsd::display::camera_begin(&mut self.game.objs, self.data, Some(c)) {
            let n = i32::from(self.horde().count);
            for i in 0..n {
                for j in 0..=n {
                    self.player_800366dc(j, i32::from(j != 0 && i + 1 == j));
                }
                self.camera_fighters(gobj, 1);
                self.horde_copy(i as u8, false);
                self.horde_copy(3 + i as u8, true);
            }
            crate::hsd::display::camera_end(&mut self.game.objs);
        }
        self.horde().copied = 1;
    }

    /// `HSD_ImageDescCopyFromEFB(image n, 0x82, 0, clear, …)`: a draw, and the GPU's pixels
    /// in the image's block, which the simulation never reads.
    fn horde_copy(&mut self, n: u8, clear: bool) {
        self.game
            .objs
            .display
            .draws
            .push(crate::hsd::display::Draw::IntroCopy {
                image: n,
                x: COPY_X,
                y: 0,
                clear,
            });
        let f = self.horde().images[usize::from(n)];
        if f.addr != 0 {
            crate::hsd::heap::with(|h| {
                h.image.hold(
                    f.addr,
                    f.addr + f.size,
                    crate::hsd::image::Src::RenderTarget,
                )
            });
        }
    }

    /// The map's camera's render callback on a horde stage, `fn_8018569C`: through the
    /// camera, only the player's fighter shown, the fighters' passes, then links 0 to 2.
    #[doc(alias = "fn_8018569C")]
    pub fn intro_horde_camera_render(&mut self, gobj: GObjId) {
        let Some(Obj::CObj(c)) = self.game.world.gobj(gobj).obj else {
            panic!("the intro's camera holds no CObj");
        };
        if crate::hsd::display::camera_begin(&mut self.game.objs, self.data, Some(c)) {
            let n = i32::from(self.horde().count);
            for i in 0..=n {
                self.player_800366dc(i, i32::from(i == 0));
            }
            self.camera_fighters(gobj, 1);
            self.game.world.gobj_mut(gobj).gxlink_prios = 0x1001;
            self.render_links(gobj, 7);
            crate::hsd::display::camera_end(&mut self.game.objs);
        }
    }

    /// The sprites' camera's render callback, `fn_8018564C`: a quad from x 200 to 640 over
    /// the screen that writes only depth (`fn_80185408`), then the sprites
    /// (`HSD_SObjLib_803A54EC`).
    #[doc(alias = "fn_8018564C")]
    #[doc(alias = "fn_80185408")]
    pub fn intro_horde_sprite_render(&mut self, gobj: GObjId) {
        self.game
            .objs
            .display
            .draws
            .push(crate::hsd::display::Draw::DepthQuad {
                top: 0.0,
                bottom: 480.0,
                left: 200.0,
                right: 640.0,
            });
        // `HSD_SObjLib_803A54EC(gobj, 7)`.
        let Some(Obj::CObj(c)) = self.game.world.gobj(gobj).obj else {
            panic!("the splash's sprites' camera holds no CObj");
        };
        if crate::hsd::display::camera_begin(&mut self.game.objs, self.data, Some(c)) {
            self.game
                .objs
                .display
                .draws
                .push(crate::hsd::display::Draw::SpriteSetup);
            self.render_links(gobj, 7);
            crate::hsd::display::camera_end(&mut self.game.objs);
        }
    }

    /// The splash's process, `fn_801857C4`: once the copies are made, the copies' camera
    /// freed; ten sprites of the images in their order, each over its depth image, at a
    /// random place in its cell of a grid three wide (`fn_801855BC` of 10), hidden and not
    /// writing depth, shown one every eight ticks from the next; then the process's GObj
    /// freed.
    #[doc(alias = "fn_801857C4")]
    pub fn intro_horde_splash(&mut self, gobj: GObjId) {
        if self.horde().copied == 0 {
            return;
        }
        let cam = self.horde().copy_camera;
        if let Some(c) = cam {
            crate::hsd::gobj::free(self, c);
        }
        let mut delay = 1u32;
        for i in 0..10u8 {
            let h = *self.horde();
            let k = usize::from(h.order[usize::from(i)]);
            let (img, depth) = (h.images[k], h.images[3 + k]);
            let g = h.sprites.expect("the splash's sprites' GObj");
            let head = match self.game.world.gobj(g).obj {
                Some(Obj::SObj(head)) => head,
                ref other => panic!("the splash's sprites' GObj holds {other:?}"),
            };
            let copy = |n: usize, f: Frame| sobj::Image::IntroCopy {
                image: n as u8,
                addr: f.addr,
                width: f.width,
                height: f.height,
                format: f.format,
            };
            let (s, head) = sobj::create(
                &mut self.game.objs.sobjs,
                self.data,
                g,
                head,
                copy(k, img),
                (0, 0),
                0x80,
                None,
            );
            self.set_sobj_head(g, head);
            let o = &mut self.game.objs.sobjs[s.0];
            o.flags |= 4;
            o.image2_copy = copy(3 + k, depth);
            let cols = fp::fctiwz(sqrt(10.0)) as u8;
            let row = i / cols;
            let rx = self.game.objs.rng.draw_f32();
            let cells = fp::frsp(f64::from((cols + 9) / cols));
            let w = fp::fdivs(290.0, cells);
            let x = fp::fadds(280.0, fp::fmuls(w, fp::frsp(f64::from(i % cols))));
            let x = fp::fmadds(40.0, reg(rx), x);
            let ry = self.game.objs.rng.draw_f32();
            let h = fp::fdivs(160.0, fp::frsp(f64::from(cols)));
            let y = fp::fmadds(h, fp::frsp(f64::from(row)), -60.0);
            let y = fp::fmadds(40.0, reg(ry), y);
            let o = &mut self.game.objs.sobjs[s.0];
            o.x = mem(x);
            o.y = mem(y);
            o.x48 = delay;
            delay += 8;
            o.flags |= 9;
        }
        crate::hsd::gobj::free(self, gobj);
    }

    /// The sprites' process, `fn_801859C8`: each sprite's wait counted down, shown at 0.
    #[doc(alias = "fn_801859C8")]
    pub fn intro_horde_sprites(&mut self, gobj: GObjId) {
        let mut cur = match self.game.world.gobj(gobj).obj {
            Some(Obj::SObj(head)) => head,
            ref other => panic!("the splash's sprites' GObj holds {other:?}"),
        };
        while let Some(s) = cur {
            let o = &mut self.game.objs.sobjs[s.0];
            if o.x48 != 0 {
                o.x48 -= 1;
                if o.x48 == 0 {
                    o.flags &= !1;
                }
            }
            cur = o.next;
        }
    }
}
