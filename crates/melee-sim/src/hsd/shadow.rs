//! Shadows (`sysdolphin/baselib/shadow.c`): a camera that renders the joints it casts from
//! into a texture, which materials then sample.
//!
//! The rendering is the display pass's; the simulation keeps what allocating and setting up
//! a shadow changes: its camera and texture (allocated from the CObj and TObj pools), the
//! references it takes on the joints it casts from, and its place on the materials' shadow
//! texture list.

use super::Objects;
use super::cobj::{self, CObjId};
use super::jobj::{self, JObjId};
use super::mobj;
use super::pool::Pool;
use super::tobj::{self, TObjId};

/// A shadow's slot (`HSD_Shadow`, from `shadow_alloc_data`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ShadowId(pub u32);

/// `HSD_Shadow`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shadow {
    /// The joints it casts from, most recently added first (`objects`).
    pub objects: Vec<JObjId>,
    pub camera: Option<CObjId>,
    pub texture: Option<TObjId>,
    pub scale_s: f32,
    pub scale_t: f32,
    pub trans_s: f32,
    pub trans_t: f32,
    pub active: bool,
    pub intensity: u8,
    /// Its texture's image (`texture->imagedesc`): format, size, and whether its buffer is
    /// allocated (`image_ptr`).
    pub image: ShadowImage,
}

/// A shadow texture's `HSD_ImageDesc`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ShadowImage {
    pub format: u32,
    pub width: u16,
    pub height: u16,
    pub allocated: bool,
}

/// The shadow pool.
pub type Shadows = Pool<Shadow>;

/// A shadow's texture, `makeShadowTObj`.
#[doc(alias = "makeShadowTObj")]
fn make_texture(objs: &mut Objects) -> TObjId {
    let t = tobj::alloc(&mut objs.tobjs);
    let o = &mut objs.tobjs[t];
    o.src = 0;
    o.wrap_s = 0;
    o.wrap_t = 0;
    o.flags = 0x54_0103;
    t
}

/// A new shadow with a 256 by 256 texture, `HSD_ShadowAlloc`.
#[doc(alias = "HSD_ShadowAlloc")]
pub fn alloc(objs: &mut Objects) -> ShadowId {
    let s = ShadowId(objs.shadows.alloc(Shadow::default()));
    let camera = cobj::alloc(objs);
    let texture = make_texture(objs);
    objs.shadows[s.0] = Shadow {
        objects: Vec::new(),
        camera: Some(camera),
        texture: Some(texture),
        scale_s: 0.5,
        scale_t: -0.5,
        trans_s: 0.5,
        trans_t: 0.5,
        active: false,
        intensity: 0,
        image: ShadowImage {
            format: 0,
            width: 256,
            height: 256,
            allocated: false,
        },
    };
    cobj::set_viewportfx4(&mut objs.cobjs, Some(camera), [0.0, 256.0, 0.0, 256.0]);
    cobj::set_scissor(&mut objs.cobjs, Some(camera), [0, 256, 0, 256]);
    s
}

/// Sizes the shadow's texture, allocating its image anew if its size changes,
/// `HSD_ShadowSetSize`.
#[doc(alias = "HSD_ShadowSetSize")]
pub fn set_size(objs: &mut Objects, s: ShadowId, width: u16, height: u16) {
    let sh = &mut objs.shadows[s.0];
    let img = &mut sh.image;
    if !img.allocated || img.width != width || img.height != height {
        img.allocated = true;
        img.width = width;
        img.height = height;
        let camera = sh.camera;
        let (w, h) = (f32::from(width), f32::from(height));
        cobj::set_viewportfx4(&mut objs.cobjs, camera, [0.0, w, 0.0, h]);
        cobj::set_scissor(&mut objs.cobjs, camera, [0, width, 0, height]);
    }
}

/// Casts the shadow from a joint too unless it does, taking a reference on the joint,
/// `HSD_ShadowAddObject`.
#[doc(alias = "HSD_ShadowAddObject")]
pub fn add_object(objs: &mut Objects, s: Option<ShadowId>, j: Option<JObjId>) {
    let (Some(s), Some(j)) = (s, j) else { return };
    if objs.shadows[s.0].objects.contains(&j) {
        return;
    }
    objs.shadows[s.0].objects.insert(0, j);
    jobj::ref_(&mut objs.jobjs, Some(j));
}

/// Turns the shadow on or off: its texture joins or leaves the materials' shadow textures,
/// `HSD_ShadowSetActive`.
#[doc(alias = "HSD_ShadowSetActive")]
pub fn set_active(objs: &mut Objects, s: ShadowId, active: bool) {
    if objs.shadows[s.0].active == active {
        return;
    }
    objs.shadows[s.0].active = active;
    let texture = objs.shadows[s.0]
        .texture
        .expect("a shadow without a texture");
    if active {
        if !objs.shadows[s.0].image.allocated {
            let img = objs.shadows[s.0].image;
            set_size(objs, s, img.width, img.height);
        }
        mobj::add_shadow_texture(objs, texture);
    } else {
        mobj::delete_shadow_texture(objs, Some(texture));
    }
}
