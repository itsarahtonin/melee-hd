//! Camera helpers from `melee/lb/lb_00B0.c`.

use melee_data::archive::{Archives, DataRef};

use crate::fpu::reg;
use crate::hsd::Objects;
use crate::hsd::cobj::{self, CObjId, StaticDesc};

/// Where a camera descriptor is: in loaded data or in the executable's.
#[derive(Clone, Copy, Debug)]
pub enum CObjDesc<'a> {
    Data(DataRef),
    Static(&'a StaticDesc),
}

/// A CObj from a descriptor, its 1.18 aspect widened to 1.2173 for a perspective camera and
/// its scissor kept on the 640x480 screen, `lb_80013B14`.
#[doc(alias = "lb_80013B14")]
pub fn load_cobj(objs: &mut Objects, data: &Archives, desc: CObjDesc) -> CObjId {
    let c = match desc {
        CObjDesc::Data(d) => cobj::load_desc(objs, data, Some(d)).expect("a camera"),
        CObjDesc::Static(d) => cobj::load_static_desc(objs, data, d),
    };
    let cobjs = &mut objs.cobjs;
    if cobj::projection_type(cobjs, Some(c)) == cobj::PERSPECTIVE
        && reg(cobj::aspect(cobjs, Some(c))) == reg(1.18)
    {
        cobj::set_aspect(cobjs, Some(c), 1.217_333_3);
    }
    let mut s = cobj::scissor(cobjs, Some(c)).expect("a camera");
    if s[1] > 0x280 {
        s[1] = 0x280;
    }
    if s[3] > 0x1E0 {
        s[3] = 0x1E0;
    }
    cobj::set_scissor(cobjs, Some(c), s);
    c
}
