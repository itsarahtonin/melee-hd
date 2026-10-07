//! PowerPC integer semantics C leaves undefined and MWCC's code defines.

/// `slw`: shifts of 32 or more give 0.
pub fn slw(a: u32, n: u32) -> u32 {
    let n = n & 63;
    if n >= 32 { 0 } else { a << n }
}

/// `slw` on a signed value.
pub fn slw_i(a: i32, n: u32) -> i32 {
    slw(a as u32, n) as i32
}

/// `divw`. Where the quotient overflows (a divisor of 0, or `i32::MIN` by -1) the result is
/// all ones for a negative dividend and 0 otherwise, as ssbm-rs's processor gives it.
pub fn divw(a: i32, b: i32) -> i32 {
    if b == 0 || (a == i32::MIN && b == -1) {
        if a < 0 { -1 } else { 0 }
    } else {
        a / b
    }
}
