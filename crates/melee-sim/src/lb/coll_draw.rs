//! The collision bubbles' and ranges' debug drawings (`lbcollision.c`'s `lbColl_80009F54` to
//! `lbColl_8000AB2C`, `lbgx.c`, `lb_0146.c`): which bubbles draw in which pass, and the state
//! drawing them changes (a hurt capsule's or a hit's place, brought up to date once). Where
//! the original issues GX commands, the pass appends a [`DebugDraw`] for the presentation.
//!
//! Every bubble's colour is translucent (alpha 0x80), so every bubble draws in the
//! translucent pass (2) only.

use melee_data::archive::Archives;

use crate::game::structs::{HitCapsule, HitResult, HurtCapsule};
use crate::hsd::Objects;
use crate::hsd::display::Draw;
use crate::hsd::jobj::{self, JObjId};
use crate::mtx::Mtx34;
use crate::vec::Vec3;

/// The translucent pass, the one a colour with alpha below 0xFF draws in.
pub const PASS_XLU: i32 = 2;

/// A hit's colour by its element (`lbColl_804D36A0`, `804D36A4` for a grab, `804D36DC` for
/// an inert hit) and the hits' ambient colour (`lbColl_804D36A8`).
const HIT: [u8; 4] = [0xFF, 0x00, 0x00, 0x80];
const HIT_CATCH: [u8; 4] = [0xFF, 0x00, 0xFF, 0x80];
const HIT_INERT: [u8; 4] = [0xFF, 0xFF, 0xFF, 0x80];
const HIT_AMB: [u8; 4] = [0x80, 0x00, 0x00, 0x80];
/// A hurt capsule's colour and ambient colour by its state (`lbColl_803B9928`: vulnerable,
/// invincible, intangible).
const HURT: [([u8; 4], [u8; 4]); 3] = [
    ([0xFF, 0xFF, 0x00, 0x80], [0x80, 0x80, 0x00, 0x80]),
    ([0x00, 0xFF, 0x00, 0x80], [0x00, 0x80, 0x00, 0x80]),
    ([0x00, 0x00, 0xFF, 0x80], [0x00, 0x00, 0x80, 0x80]),
];
/// The shield's (`lbColl_804D36C4`), the reflector's (`804D36CC`) and the absorber's
/// (`804D36D4`) colours with their ambient colours.
pub const SHIELD: ([u8; 4], [u8; 4]) = ([0x00, 0xFF, 0xFF, 0x80], [0x00, 0x80, 0x80, 0x80]);
pub const REFLECT: ([u8; 4], [u8; 4]) = ([0x00, 0xFF, 0x80, 0x80], [0x00, 0x80, 0x40, 0x80]);
pub const ABSORB: ([u8; 4], [u8; 4]) = ([0x00, 0x80, 0xFF, 0x80], [0x00, 0x40, 0x80, 0x80]);
/// A thrown fighter's hit (`lbColl_804D36E8`, `804D36EC`).
const THROWN: ([u8; 4], [u8; 4]) = ([0xFF, 0xFF, 0xFF, 0x80], [0x80, 0x80, 0x80, 0x80]);
/// The fighter's `x1614` capsules (`lbColl_804D36F0`, `804D36F4`).
const X1614: ([u8; 4], [u8; 4]) = ([0xFF, 0xFF, 0x00, 0x80], [0x80, 0x80, 0x00, 0x80]);

/// `HitElement_Catch` and `HitElement_Inert`.
const ELEMENT_CATCH: u32 = 8;
const ELEMENT_INERT: u32 = 11;
/// `HurtCapsule_Intangible`.
const HURT_INTANGIBLE: i32 = 2;

/// A debug drawing the presentation makes in GX directly, HAL's state caches invalidated
/// around it.
#[derive(melee_derive::CloneFrom, Debug, PartialEq, melee_derive::StateEq)]
pub enum DebugDraw {
    /// A capsule from `a` to `b` in the world, `size` its radius (`lbColl_80008FC8`).
    Capsule {
        a: Vec3,
        b: Vec3,
        color: [u8; 4],
        amb: [u8; 4],
        size: f32,
    },
    /// A capsule from `a` to `b` in the world through a joint's matrix `mtx`
    /// (`lbColl_DrawHitResult`: the points taken into the matrix's space, then
    /// `lbColl_800096B4`).
    InSpace {
        mtx: Mtx34,
        a: Vec3,
        b: Vec3,
        color: [u8; 4],
        amb: [u8; 4],
        size: f32,
    },
    /// A pickup range's box around `pos` (`lbGx_8001E2F8`): `range` its offset (x, along
    /// `facing`), its half width and half height.
    Range {
        range: crate::quat::Quat,
        pos: Vec3,
        color: [u8; 4],
        facing: f32,
    },
    /// A fighter's damage line (`lb_80014770`, its two points).
    Damage([Vec3; 2]),
    /// The end of an item's debug drawings: HAL's state caches out of date, the TEV reset and
    /// the vertex description cleared (`it_8026EECC`'s tail).
    ItemEnd,
}

/// Appends a debug drawing.
pub fn push(objs: &mut Objects, d: DebugDraw) {
    objs.display.draws.push(Draw::Debug(Box::new(d)));
}

/// The pass a colour draws in: the opaque pass (0) for an opaque colour, else the
/// translucent (2).
fn pass_of(color: [u8; 4]) -> i32 {
    if color[3] == 0xFF { 0 } else { PASS_XLU }
}

/// `HSD_JObjGetMtxPtr`: the joint's matrix brought up to date.
fn joint_mtx(objs: &mut Objects, data: &Archives, j: Option<JObjId>) -> Mtx34 {
    let j = j.expect("a bubble's joint");
    jobj::setup_matrix(objs, data, Some(j));
    objs.jobjs[j].mtx
}

/// A hit's bubble, `lbColl_80009F54`: an enabled hit the original doesn't hide (`x43_b2`),
/// coloured by its element, its size scaled by `scale_y` unless it keeps its own (`x43_b1`).
#[doc(alias = "lbColl_80009F54")]
pub fn hit(objs: &mut Objects, hit: &HitCapsule, pass: i32, scale_y: f32) -> bool {
    if hit.state == 0 || hit.x43_b2 {
        return false;
    }
    let color = match hit.element {
        ELEMENT_CATCH => HIT_CATCH,
        ELEMENT_INERT => HIT_INERT,
        _ => HIT,
    };
    hit_capsule(objs, hit, pass, scale_y, (color, HIT_AMB))
}

/// A thrown fighter's hit's bubble, `lbColl_8000A044`.
// Ports `lbColl_8000A044`; aliased once a check reaches it (the fighters' displays past their bubbles
// are behind `b5`'s, which isn't ported).
pub fn thrown_hit(objs: &mut Objects, hit: &HitCapsule, pass: i32, scale_y: f32) -> bool {
    if hit.state == 0 || hit.x43_b2 {
        return false;
    }
    hit_capsule(objs, hit, pass, scale_y, THROWN)
}

fn hit_capsule(
    objs: &mut Objects,
    hit: &HitCapsule,
    pass: i32,
    scale_y: f32,
    (color, amb): ([u8; 4], [u8; 4]),
) -> bool {
    if pass_of(color) != pass {
        return false;
    }
    let size = if hit.x43_b1 {
        hit.scale
    } else {
        crate::fpu::mem(gekko_fp::fmuls(
            crate::fpu::reg(hit.scale),
            crate::fpu::reg(scale_y),
        ))
    };
    push(
        objs,
        DebugDraw::Capsule {
            a: hit.x58,
            b: hit.x4C,
            color,
            amb,
            size,
        },
    );
    true
}

/// One of the fighter's `x1614` capsules, `lbColl_8000A1A8`: from `x14` to `x8`, `x0` times
/// `scale_y` wide.
// Ports `lbColl_8000A1A8`; aliased once a check reaches it (the fighters' displays past their bubbles
// are behind `b5`'s, which isn't ported).
pub fn x1614(
    objs: &mut Objects,
    c: &crate::game::structs::Fighter_x1614_t,
    pass: i32,
    scale_y: f32,
) -> bool {
    let (color, amb) = X1614;
    if pass_of(color) != pass {
        return false;
    }
    let size = crate::fpu::mem(gekko_fp::fmuls(
        crate::fpu::reg(c.x0),
        crate::fpu::reg(scale_y),
    ));
    push(
        objs,
        DebugDraw::Capsule {
            a: c.x14,
            b: c.x8,
            color,
            amb,
            size,
        },
    );
    true
}

/// A hurt capsule's bubble, coloured by its state: `lbColl_8000A244` (`state` none), or
/// `lbColl_8000A584` with the state the fighter's invincibility gives it (an intangible
/// capsule stays intangible). The capsule's ends are brought up to date once (with the
/// depth `z` for a fighter drawn with a depth scale, `mtx`), then drawn in the bone's
/// space.
#[doc(alias = "lbColl_8000A244")]
// Ports `lbColl_8000A584` too; aliased once a session draws an invincible fighter's bubbles.
pub fn hurt(
    objs: &mut Objects,
    data: &Archives,
    hurt: &mut HurtCapsule,
    state: Option<u32>,
    pass: i32,
    mtx: Option<&Mtx34>,
    z: f32,
) -> bool {
    let i = match state {
        None => hurt.state as usize,
        Some(_) if hurt.state == HURT_INTANGIBLE => 2,
        Some(s) => s as usize,
    };
    let (color, amb) = HURT[i];
    if pass_of(color) != pass {
        return false;
    }
    if !hurt.skip_update_pos {
        hurt.a_pos = crate::lb::joint::world_position(objs, data, hurt.bone, Some(hurt.a_offset));
        hurt.b_pos = crate::lb::joint::world_position(objs, data, hurt.bone, Some(hurt.b_offset));
        if mtx.is_some() {
            hurt.b_pos.z = z;
            hurt.a_pos.z = z;
        }
        hurt.skip_update_pos = true;
    }
    let bone = joint_mtx(objs, data, hurt.bone);
    let space = mtx.map_or(bone, |m| m.concat(&bone));
    push(
        objs,
        DebugDraw::InSpace {
            mtx: space,
            a: hurt.a_pos,
            b: hurt.b_pos,
            color,
            amb,
            size: hurt.scale,
        },
    );
    true
}

/// A shield's, reflector's or absorber's bubble (`lbColl_DrawHit` for `lbColl_8000A78C`,
/// `8000A95C` and `8000AB2C`): its place brought up to date once, then drawn in its bone's
/// space.
#[doc(alias = "lbColl_8000A78C")]
// Ports `lbColl_8000A95C` and `lbColl_8000AB2C` too; aliased once a session draws a
// reflector's or an absorber's bubble.
pub fn hit_result(
    objs: &mut Objects,
    data: &Archives,
    hit: &mut HitResult,
    (color, amb): ([u8; 4], [u8; 4]),
    pass: i32,
    mtx: Option<&Mtx34>,
    z: f32,
) -> bool {
    if pass_of(color) != pass {
        return false;
    }
    if !hit.skip_update_pos {
        hit.pos = crate::lb::joint::world_position(objs, data, hit.bone, Some(hit.offset));
        if mtx.is_some() {
            hit.pos.z = z;
        }
        hit.skip_update_pos = true;
    }
    let bone = joint_mtx(objs, data, hit.bone);
    let space = mtx.map_or(bone, |m| m.concat(&bone));
    push(
        objs,
        DebugDraw::InSpace {
            mtx: space,
            a: hit.pos,
            b: hit.pos,
            color,
            amb,
            size: hit.size,
        },
    );
    true
}

/// A pickup range, `lbGx_8001E2F8`: drawn in the translucent pass.
// The pass test of `lbGx_8001E2F8` (the presentation's).
pub fn range(
    objs: &mut Objects,
    range: crate::quat::Quat,
    pos: Vec3,
    color: [u8; 4],
    pass: i32,
    facing: f32,
) -> bool {
    if pass != PASS_XLU {
        return false;
    }
    push(
        objs,
        DebugDraw::Range {
            range,
            pos,
            color,
            facing,
        },
    );
    true
}

/// A fighter's damage line, `lb_80014770`: drawn in the translucent pass.
// The pass test of `lb_80014770` (the presentation's).
pub fn damage(objs: &mut Objects, pts: [Vec3; 2], pass: i32) -> bool {
    if pass != PASS_XLU {
        return false;
    }
    push(objs, DebugDraw::Damage(pts));
    true
}
