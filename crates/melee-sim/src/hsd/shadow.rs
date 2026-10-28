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
use crate::hsd::heap::Kind;
use crate::hsd::pool::kind as pool_kind;
use crate::mtx::Mtx34;
use crate::vec::Vec3;

/// A shadow's slot (`HSD_Shadow`, from `shadow_alloc_data`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, melee_derive::StateEq)]
pub struct ShadowId(pub u32);

/// `HSD_Shadow`.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
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
    /// Where the original's allocators put the list nodes holding `objects` (in their order),
    /// the texture's image descriptor and its buffer (`docs/engine.md`).
    pub object_nodes: Vec<u32>,
    pub imagedesc_addr: u32,
    pub image_addr: u32,
}

/// A shadow texture's `HSD_ImageDesc`.
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct ShadowImage {
    pub format: u32,
    pub width: u16,
    pub height: u16,
    pub allocated: bool,
}

/// The shadow pool.
pub type Shadows = Pool<Shadow, { pool_kind(Kind::Shadow) }>;

/// A shadow's texture (the inline `makeShadowTObj`).
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
    // `HSD_ImageDescAlloc`.
    let imagedesc_addr = super::heap::piece_alloc(0x18);
    objs.tobjs[texture].imagedesc = Some(tobj::ImageRef::Shadow(s));
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
        object_nodes: Vec::new(),
        imagedesc_addr,
        image_addr: 0,
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
        if img.allocated {
            super::heap::mem_free(sh.image_addr);
        }
        let size = super::heap::tex_buffer_size(u32::from(width), u32::from(height), 0);
        // Only the GPU writes it, at each copy: until then it holds what the block held.
        sh.image_addr = super::heap::mem_alloc_unclaimed(size);
        let img = &mut sh.image;
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
    // `HSD_SListAllocAndPrepend`.
    let node = super::heap::obj_alloc(Kind::SList);
    objs.shadows[s.0].object_nodes.insert(0, node);
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

/// Stops casting the shadow from a joint, or from every joint, dropping the references,
/// `HSD_ShadowDeleteObject`.
#[doc(alias = "HSD_ShadowDeleteObject")]
pub fn delete_object(objs: &mut Objects, s: Option<ShadowId>, j: Option<JObjId>) {
    let Some(s) = s else { return };
    match j {
        Some(j) => {
            if let Some(i) = objs.shadows[s.0].objects.iter().position(|&o| o == j) {
                super::export::with_image(|img| super::export::shadow_node(objs, img, s, i));
                jobj::unref(objs, Some(j));
                objs.shadows[s.0].objects.remove(i);
                let node = objs.shadows[s.0].object_nodes.remove(i);
                super::heap::obj_free(Kind::SList, node);
            }
        }
        None => {
            while let Some(&o) = objs.shadows[s.0].objects.first() {
                super::export::with_image(|img| super::export::shadow_node(objs, img, s, 0));
                jobj::unref(objs, Some(o));
                objs.shadows[s.0].objects.remove(0);
                let node = objs.shadows[s.0].object_nodes.remove(0);
                super::heap::obj_free(Kind::SList, node);
            }
        }
    }
}

/// Renders the joints the shadow casts from through its camera, `HSD_ShadowStartRender`:
/// nothing without joints; else the camera current, each joint's tree in the shadow render
/// mode for the opaque and edge passes, and the camera's end. The texture's clearing and
/// colors are GX's.
#[doc(alias = "HSD_ShadowStartRender")]
pub fn start_render(
    objs: &mut Objects,
    data: &melee_data::archive::Archives,
    hooks: &mut dyn super::particle::Hooks,
    s: ShadowId,
) {
    if objs.shadows[s.0].objects.is_empty() {
        return;
    }
    let camera = objs.shadows[s.0].camera;
    super::display::camera_begin(objs, data, camera);
    let mut i = 0;
    while let Some(&j) = objs.shadows[s.0].objects.get(i) {
        super::display::jobj_disp_all(
            objs,
            data,
            hooks,
            Some(j),
            None,
            super::display::TRSP_OPA | super::display::TRSP_TEXEDGE,
            super::display::RENDER_SHADOW,
        );
        i += 1;
    }
    super::display::camera_end(objs);
}

/// Copies the render into the shadow's texture, allocating its image first if it has none,
/// and makes the texture's projection from the camera, `HSD_ShadowEndRender` (with the
/// static `makeMatrix`).
#[doc(alias = "HSD_ShadowEndRender")]
pub fn end_render(objs: &mut Objects, s: ShadowId) {
    let img = objs.shadows[s.0].image;
    if !img.allocated {
        set_size(objs, s, img.width, img.height);
    }
    objs.display.draws.push(super::display::Draw::ShadowCopy(s));
    // The GPU's pixels in the texture's buffer, which the simulation never reads: for the heap's
    // image, a render target (docs/engine.md, "Uninitialized values").
    let sh = &objs.shadows[s.0];
    if sh.image_addr != 0 {
        let size =
            super::heap::tex_buffer_size(u32::from(sh.image.width), u32::from(sh.image.height), 0);
        let at = sh.image_addr;
        super::heap::with(|h| h.image.hold(at, at + size, super::image::Src::RenderTarget));
    }
    let sh = &objs.shadows[s.0];
    let c = sh.camera.expect("a shadow without a camera");
    let o = &objs.cobjs[c];
    let p = o.projection;
    let (ss, st, ts, tt) = (sh.scale_s, sh.scale_t, sh.trans_s, sh.trans_t);
    let prj = match o.projection_type {
        cobj::PERSPECTIVE => Mtx34::light_perspective(p[0], p[1], ss, st, ts, tt),
        cobj::FRUSTUM => Mtx34::light_frustum(p[0], p[1], p[2], p[3], o.near, ss, st, ts, tt),
        cobj::ORTHO => Mtx34::light_ortho(p[0], p[1], p[2], p[3], ss, st, ts, tt),
        _ => panic!("a shadow camera without a projection"),
    };
    let m = prj.concat(&o.view_mtx);
    let t = sh.texture.expect("a shadow without a texture");
    objs.tobjs[t].mtx = m;
}

/// Fits the shadow's camera to a rectangle, `HSD_ShadowSetViewingRect`: an orthographic
/// camera takes it as its planes, a perspective one the aspect and field of view it spans
/// at the eye's distance, a frustum one its planes scaled to the near plane.
#[doc(alias = "HSD_ShadowSetViewingRect")]
pub fn set_viewing_rect(
    objs: &mut Objects,
    data: &melee_data::archive::Archives,
    s: ShadowId,
    rect: [f32; 4],
) {
    use crate::fpu::{mem, reg};
    use gekko_fp as fp;
    let [top, bottom, left, right] = rect;
    let c = objs.shadows[s.0].camera;
    let distance = cobj::eye_distance(objs, data, c);
    assert!(reg(distance) > 0.0, "distance > 0.0F");
    match objs.cobjs[c.expect("a shadow without a camera")].projection_type {
        cobj::PERSPECTIVE => {
            let abs = |v: f32| f32::from_bits(v.to_bits() & 0x7FFF_FFFF);
            let width = if reg(abs(top)) > reg(abs(bottom)) {
                abs(top)
            } else {
                abs(bottom)
            };
            let height = if reg(abs(left)) > reg(abs(right)) {
                abs(left)
            } else {
                abs(right)
            };
            cobj::set_aspect(&mut objs.cobjs, c, mem(fp::fdivs(reg(height), reg(width))));
            cobj::set_fov(&mut objs.cobjs, c, crate::math::atan2(height, distance));
        }
        cobj::ORTHO => cobj::set_ortho(&mut objs.cobjs, c, top, bottom, left, right),
        cobj::FRUSTUM => {
            let scale = fp::fdivs(reg(cobj::near(&objs.cobjs, c)), reg(distance));
            assert!(scale > 0.0, "scale > 0.0F");
            let f = |v: f32| mem(fp::fmuls(scale, reg(v)));
            cobj::set_frustum(&mut objs.cobjs, c, f(top), f(bottom), f(left), f(right));
        }
        _ => panic!("a shadow camera without a projection"),
    }
}

/// `HSD_ViewingRect`: a rectangle grown around points as an eye at `origin` sees them.
#[derive(Clone, Copy, Debug, melee_derive::StateEq)]
pub struct ViewingRect {
    pub origin: Vec3,
    pub eye_v: Vec3,
    pub eye_vn: Vec3,
    pub up_v: Vec3,
    pub right_v: Vec3,
    pub distance: f32,
    pub top: f32,
    pub bottom: f32,
    pub left: f32,
    pub right: f32,
    pub perspective: bool,
}

impl ViewingRect {
    /// An empty rectangle for an eye at `position` looking at `interest`,
    /// `HSD_ViewingRectInit`.
    #[doc(alias = "HSD_ViewingRectInit")]
    pub fn new(position: Vec3, interest: Vec3, up: Vec3, perspective: bool) -> ViewingRect {
        let eye_v = interest.sdk_sub(position);
        let eye_vn = eye_v.sdk_normalize();
        let v = up.sdk_normalize();
        let right_v = eye_vn.sdk_cross(v);
        let up_v = right_v.sdk_cross(eye_vn);
        ViewingRect {
            origin: position,
            eye_v,
            eye_vn,
            up_v,
            right_v,
            distance: eye_v.sdk_length(),
            top: -f32::MAX,
            right: -f32::MAX,
            bottom: f32::MAX,
            left: f32::MAX,
            perspective,
        }
    }

    /// Whether the rectangle has area, `HSD_ViewingRectCheck`.
    #[doc(alias = "HSD_ViewingRectCheck")]
    pub fn check(&self) -> bool {
        use crate::fpu::reg;
        reg(self.top) > reg(self.bottom) && reg(self.right) > reg(self.left)
    }

    /// Grows the rectangle to take in a rectangle around `position`, `HSD_ViewingRectAddRect`.
    #[doc(alias = "HSD_ViewingRectAddRect")]
    pub fn add_rect(&mut self, position: Vec3, t0: f32, b0: f32, l0: f32, r0: f32) {
        use crate::fpu::{mem, reg};
        use gekko_fp as fp;
        let o2p = position.sdk_sub(self.origin);
        let dot = o2p.sdk_dot(self.eye_vn);
        let (x, y, top, bottom, left, right);
        if self.perspective {
            if reg(dot) <= 0.0 {
                return;
            }
            let scale = mem(fp::fdivs(reg(self.distance), reg(dot)));
            let e2p = o2p.sdk_scale(scale).sdk_sub(self.eye_v);
            x = self.right_v.sdk_dot(e2p);
            y = self.up_v.sdk_dot(e2p);
            let s = |v: f32| mem(fp::fmuls(reg(v), reg(scale)));
            (top, bottom, left, right) = (s(t0), s(b0), s(l0), s(r0));
        } else {
            let e2p = o2p.sdk_sub(self.eye_vn.sdk_scale(dot));
            x = self.right_v.sdk_dot(e2p);
            y = self.up_v.sdk_dot(e2p);
            (top, bottom, left, right) = (t0, b0, l0, r0);
        }
        let add = |a: f32, b: f32| mem(fp::fadds(reg(a), reg(b)));
        if reg(add(x, right)) > reg(self.right) {
            self.right = add(x, right);
        }
        if reg(add(x, left)) < reg(self.left) {
            self.left = add(x, left);
        }
        if reg(add(y, top)) > reg(self.top) {
            self.top = add(y, top);
        }
        if reg(add(y, bottom)) < reg(self.bottom) {
            self.bottom = add(y, bottom);
        }
    }
}
