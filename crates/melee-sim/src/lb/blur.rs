//! A copy of the frame drawn back as a sprite, blurred and tinted (`lbspdisplay.c`'s
//! `lb_800138EC`, `CameraBlurData`): a GObj with an orthographic camera over the screen whose
//! render callback (`fn_80013614`) runs its owner's callback, then draws the image: plain, or
//! (mode 1) as `lb_80012994` does, 21 copies offset by up to twice the blur's size over
//! 64 pixels, each at its own alpha, tinted toward a sepia by the tint factor. Only the
//! single-player clear screen makes one ([`crate::game::gm::clear`]), over its copy of the
//! frozen match.

use super::super::game::handles::CodeRef;
use super::super::game::state::{OBJ_CAMERA, Obj, Running, UserData};
use crate::hsd::cobj;
use crate::hsd::display::{self, Draw};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// `fn_80013614`, the sprite's render callback.
pub const RENDER: u32 = 0x8001_3614;
/// `fn_800138AC`, its user data's release (`HSD_Free`).
pub const REMOVE: u32 = 0x8001_38AC;
/// `&lbl_80472D28.x30`: the clear screen's copy, the image the blur draws.
pub const CLEAR_IMAGE: u32 = 0x8047_2D58;

/// `CameraBlurData` (0x24 bytes of `HSD_MemAlloc`).
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Blur {
    /// Where the original's heap put it.
    pub addr: u32,
    /// `x0` to `xC`: where the image goes and its scale.
    pub pos: [f32; 2],
    pub scale: [f32; 2],
    /// `x10`: the first copy's alpha; `x11`: the blur's size; `x12`: 1 blurred.
    pub base_alpha: u8,
    pub blur_size: u8,
    pub mode: u8,
    /// `x18`: the owner's callback before each draw.
    pub callback: Option<CodeRef>,
    /// `x1C`: the image (its address; only the clear screen's).
    pub image: u32,
    /// `x20`: the tint, unknown until the owner's callback first writes it.
    pub tint: Option<f32>,
}

impl Running<'_> {
    /// The sprite of `image` at render priority `prio` with alpha `alpha` at `pos` scaled
    /// by `scale`, `lb_800138EC`: a GObj (class 14, link 15) with an orthographic camera onto
    /// the 640 by 480 screen, and its data, drawn plain until a mode is set.
    #[doc(alias = "lb_800138EC")]
    pub fn blur_create(&mut self, prio: u8, alpha: u8, pos: [f32; 2], scale: [f32; 2]) -> GObjId {
        let data = self.data;
        let gobj = self.game.world.create(14, 15, 0);
        let o = &mut self.game.objs;
        let c = cobj::alloc(o);
        cobj::set_projection_type(&mut o.cobjs, Some(c), 3);
        cobj::set_viewport(&mut o.cobjs, Some(c), [0, 640, 0, 480]);
        cobj::set_scissor(&mut o.cobjs, Some(c), [0, 640, 0, 480]);
        cobj::set_eye_position(o, c, Vec3::new(0.0, 0.0, 1.0));
        cobj::set_interest(o, c, Vec3::new(0.0, 0.0, 0.0));
        cobj::set_roll(o, data, Some(c), 0.0);
        cobj::set_near(&mut o.cobjs, Some(c), 0.0);
        cobj::set_far(&mut o.cobjs, Some(c), 2.0);
        cobj::set_ortho(&mut o.cobjs, Some(c), 0.0, -480.0, 0.0, 640.0);
        self.game.world.set_obj(gobj, OBJ_CAMERA, Obj::CObj(c));
        let addr = crate::hsd::heap::mem_alloc(0x24);
        self.game.blur = Some(Blur {
            addr,
            pos,
            scale,
            base_alpha: alpha,
            blur_size: 0,
            mode: 0,
            callback: None,
            image: CLEAR_IMAGE,
            tint: None,
        });
        self.game.world.init_user_data(gobj, 0, UserData::Blur);
        self.game
            .world
            .setup_gx_link_max(gobj, CodeRef(RENDER), prio, false);
        gobj
    }

    /// The sprite's render callback, `fn_80013614`: the owner's callback, then the sprite
    /// through its camera.
    #[doc(alias = "fn_80013614")]
    #[doc(alias = "lb_800138D8")]
    #[doc(alias = "lb_800138CC")]
    pub fn blur_render(&mut self, gobj: GObjId) {
        let b = self.game.blur.expect("the blur's data");
        if let Some(cb) = b.callback {
            match cb.0 {
                crate::game::gm::clear::BLUR_CALLBACK => self.clear_blur_callback(gobj),
                other => {
                    crate::game::code::unported_call(&format!("the blur's callback {other:#x}"))
                }
            }
        }
        let b = self.game.blur.expect("the blur's data");
        let Some(Obj::CObj(c)) = self.game.world.gobj(gobj).obj else {
            panic!("the blur holds no camera");
        };
        display::camera_begin(&mut self.game.objs, self.data, Some(c));
        self.game.objs.display.draws.push(Draw::Blur {
            pos: b.pos,
            scale: b.scale,
            alpha: b.base_alpha,
            size: (b.mode == 1).then_some(b.blur_size),
            tint: b.tint.unwrap_or(0.0),
        });
    }

    /// Frees the sprite's data (`fn_800138AC`, its GObj's user data release).
    pub fn blur_free(&mut self) {
        if let Some(b) = self.game.blur {
            let game = &*self.game;
            crate::hsd::export::with_image(|img| {
                crate::game::export::menu_block(game, img, &UserData::Blur)
            });
            self.game.blur = None;
            crate::hsd::heap::mem_free(b.addr);
        }
    }
}
