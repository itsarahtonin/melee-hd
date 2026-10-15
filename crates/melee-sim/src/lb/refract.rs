//! Refraction (`melee/lb/lbrefract.c`): the textures materials with a refraction effect sample
//! the screen through. At the match's start it makes one indirect texture per entry of
//! `LbRf.dat`'s table and a texture chain for each; each frame the game camera's render
//! callback copies the screen into the source texture while anything uses refraction.
//!
//! The simulation keeps the users' count, the table, the texture chains (TObjs, whose
//! matrices material setup makes) and the source texture's projection. The textures' pixels
//! (the generated indirect maps and the screen copy) are the presentation's: the TObjs name
//! them ([`ImageRef::Refract`], [`ImageRef::RefractFrame`]).

use melee_data::archive::{ArchiveId, Archives, DataRef};

use crate::game::state::Game;
use crate::hsd::Objects;
use crate::hsd::cobj::{self, CObjId};
use crate::hsd::display::Draw;
use crate::hsd::tobj::{self, GX_IDENTITY, ImageRef, TObjId, TObjs};
use crate::mtx::Mtx34;
use crate::vec::Vec3;

/// `lbl_804336D0` and `refract_data`.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Refract {
    /// `refractionUserCount`: how many things use refraction; the screen is copied while any
    /// do.
    pub users: i32,
    /// `refract_data`: `LbRf.dat`'s `lbRefData` (a count, then each texture's two shape
    /// parameters).
    pub data: Option<DataRef>,
    /// `tobj_list`: each texture's chain, its indirect map then the screen copy.
    pub tobjs: Vec<TObjId>,
    /// `texture_mtx`: the screen copy's projection for the camera that last drew.
    pub texture_mtx: Mtx34,
}

/// One of lbrefract.c's static `HSD_TObjDesc`s, as `TObjLoad` reads it (no rotation,
/// palette or TEV; unit scale).
struct Desc {
    id: u32,
    src: u32,
    flags: u32,
    mag_filt: u32,
    image: ImageRef,
}

/// A TObj from a static descriptor and the TObj it loads after itself, `HSD_TObjLoadDesc`.
/// The level-of-detail descriptors (`loddesc0`, `loddesc1`) are in the executable and only
/// set GX's filtering, so the TObjs hold none.
fn load(tobjs: &mut TObjs, d: &Desc, next: impl FnOnce(&mut TObjs) -> Option<TObjId>) -> TObjId {
    let t = tobj::alloc(tobjs);
    let next = next(tobjs);
    let o = &mut tobjs[t];
    o.next = next;
    o.id = d.id;
    o.src = d.src;
    o.mtxid = GX_IDENTITY;
    o.scale = Vec3::new(1.0, 1.0, 1.0);
    o.repeat_s = 1;
    o.repeat_t = 1;
    o.flags = d.flags | tobj::MTX_DIRTY;
    o.blending = 1.0;
    o.mag_filt = d.mag_filt;
    o.imagedesc = Some(d.image);
    o.tlut_no = 0xFF;
    t
}

impl Game {
    /// Sets refraction up for a scene, `lbRefract_800222A4`: no users, `LbRf.dat`'s table
    /// (`archive`), and for each of its textures a chain of `tobjdesc1` (the indirect map)
    /// and `tobjdesc0` (the screen copy).
    #[doc(alias = "lbRefract_800222A4")]
    pub fn refract_init(&mut self, data: &Archives, archive: ArchiveId) {
        let table = data
            .get(archive)
            .public("lbRefData")
            .map(|offset| DataRef { archive, offset })
            .expect("LbRf.dat's lbRefData");
        let n = data.u8(table);
        // The screen copy's image (320 by 240 in RGB565), the textures' list and their image
        // descriptors, then each entry's 32 by 32 IA8 indirect map ahead of its textures.
        crate::hsd::heap::with(|h| {
            h.mem_alloc(0x2_5800);
            h.mem_alloc(u32::from(n) * 4);
            h.mem_alloc(u32::from(n) * 0x18);
        });
        let tobjs = &mut self.objs.tobjs;
        let chains = (0..n)
            .map(|i| {
                crate::hsd::heap::mem_alloc(0x800);
                let map = Desc {
                    id: 1,
                    src: 1,
                    flags: 0x81,
                    mag_filt: 1,
                    image: ImageRef::Refract(i),
                };
                let copy = Desc {
                    id: 0,
                    src: 0,
                    flags: 0x83,
                    mag_filt: 1,
                    image: ImageRef::RefractFrame,
                };
                load(tobjs, &map, |tobjs| Some(load(tobjs, &copy, |_| None)))
            })
            .collect();
        self.refract = Refract {
            users: 0,
            data: Some(table),
            tobjs: chains,
            texture_mtx: self.refract.texture_mtx,
        };
    }

    /// One more user of refraction, `lbRefract_80022BB8`.
    #[doc(alias = "lbRefract_80022BB8")]
    pub fn refract_use(&mut self) {
        self.refract.users += 1;
    }

    /// One user fewer, `lbRefSetUnuse`; fewer than none is the game's error.
    #[doc(alias = "lbRefSetUnuse")]
    pub fn refract_unuse(&mut self) {
        self.refract.users -= 1;
        assert!(self.refract.users >= 0, "lbRefSetUnuse error!");
    }
}

/// The screen copy's projection for camera `c` while anything uses refraction,
/// `lbRefract_8002247C`.
#[doc(alias = "lbRefract_8002247C")]
pub fn set_camera(objs: &Objects, refract: &mut Refract, c: CObjId) {
    if refract.users == 0 {
        return;
    }
    let o = &objs.cobjs[c];
    let p = o.projection;
    refract.texture_mtx = match o.projection_type {
        cobj::PERSPECTIVE => Mtx34::light_perspective(p[0], p[1], 0.5, -0.5, 0.5, 0.5),
        cobj::FRUSTUM => Mtx34::light_frustum(p[0], p[1], p[2], p[3], o.near, 0.5, -0.5, 0.5, 0.5),
        _ => Mtx34::light_ortho(p[0], p[1], p[2], p[3], 0.5, -0.5, 0.5, 0.5),
    };
}

/// Copies the screen into the refraction source while anything uses refraction,
/// `lbRefract_80022560`: a draw command.
#[doc(alias = "lbRefract_80022560")]
pub fn copy_screen(objs: &mut Objects, refract: &Refract) {
    if refract.users != 0 {
        objs.display.draws.push(Draw::RefractCopy);
    }
}
