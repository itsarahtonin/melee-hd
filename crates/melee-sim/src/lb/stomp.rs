//! Whether a fighter's feet come down on an item it can step on (`lb_0146.c`).

use gekko_fp as fp;

use crate::fpu::{mem, reg};
use crate::game::structs::{lb_80014638_arg0_t, lb_80014638_arg1_t};

/// Whether feet that went from `feet.x0` to `feet.xC` (each the left and right edges in x and
/// y, the height in z) came down onto the item's span (`top`: its left and right edges in x
/// and y, its top in z), widened by `top.unk_x` and raised by `top.unk_y`, `lb_80014638`.
#[doc(alias = "lb_80014638")]
pub fn stepped_on(feet: &lb_80014638_arg0_t, top: &lb_80014638_arg1_t) -> bool {
    let (mut from, to, span) = (feet.x0, feet.xC, top.x8);
    from.x = mem(fp::fadds(reg(from.x), reg(top.unk_x)));
    from.y = mem(fp::fadds(reg(from.y), reg(top.unk_x)));
    from.z = mem(fp::fadds(reg(from.z), reg(top.unk_y)));
    if from.z < to.z {
        return false;
    }
    if from.z < span.z || to.z > span.z {
        return false;
    }
    let dz = fp::fsubs(reg(to.z), reg(from.z));
    let adz = if dz < 0.0 { fp::fneg(dz) } else { dz };
    let t = if adz < f64::from(0.01_f32) {
        1.0
    } else {
        fp::fdivs(fp::fsubs(reg(span.z), reg(from.z)), dz)
    };
    if fp::fmadds(t, fp::fsubs(reg(to.x), reg(from.x)), reg(from.x)) > reg(span.y) {
        return false;
    }
    if fp::fmadds(t, fp::fsubs(reg(to.y), reg(from.y)), reg(from.y)) < reg(span.x) {
        return false;
    }
    true
}
