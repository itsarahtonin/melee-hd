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
