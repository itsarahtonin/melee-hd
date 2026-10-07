//! Wind sources, from `melee/lb/lb_00F9.c`: up to eight pushes (a box's, or a blast's out from
//! a point) that sway jointed parts while they fade, kept in a pool (`lb_804D63A8`) with a free
//! list (`lb_804D63AC`) and a list of the ones blowing (`lb_804D63B0`).

use gekko_fp as fp;

use crate::fpu::{mem, reg};
use crate::vec::Vec3;

/// A wind source (`struct lb_80011A50_t`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Wind {
    /// 1 for a box's push, 2 for a blast's.
    pub kind: u8,
    /// Which source a new one may replace when none is free: the highest.
    pub priority: u8,
    pub x2: i8,
    pub x3: i8,
    /// The push (a box's) or the blast's center.
    pub pos: Vec3,
    /// A box's left, top, right and bottom.
    pub x10: f32,
    pub x14: f32,
    pub x18: f32,
    pub x1c: f32,
    /// The strength, which fades by `decay` a frame.
    pub scale: f32,
    pub decay: f32,
    /// The frames left (none left when it reaches 0 counting down; negative never ends).
    pub frames: i32,
    /// The angle a frame its strength waves with.
    pub angle_step: f32,
    /// The frames it has blown.
    pub age: i32,
}

/// The pool of wind sources and its two lists, by index.
#[derive(Clone, Debug, PartialEq)]
pub struct Winds {
    pub entries: [Wind; 8],
    pub next: [Option<u8>; 8],
    pub free: Option<u8>,
    pub active: Option<u8>,
}

impl Default for Winds {
    /// The pool as `lb_8000FA94` leaves it: every source free, in order.
    fn default() -> Self {
        Winds {
            entries: [Wind::default(); 8],
            next: std::array::from_fn(|i| (i < 7).then_some(i as u8 + 1)),
            free: Some(0),
            active: None,
        }
    }
}

/// Adds a wind source like `src`, `lb_800100B0`: a free one, else (if `src` ranks no higher)
/// the blowing one with the highest priority, overwritten. None when there's no room.
#[doc(alias = "lb_800100B0")]
pub fn add(w: &mut Winds, src: &Wind) -> Option<u8> {
    let i = if let Some(i) = w.free {
        w.free = w.next[i as usize];
        w.next[i as usize] = w.active;
        w.active = Some(i);
        i
    } else {
        let mut ret = w.active?;
        let mut max = i32::from(w.entries[ret as usize].priority);
        let mut cur = Some(ret);
        while let Some(c) = cur {
            let p = i32::from(w.entries[c as usize].priority);
            if max < p {
                max = p;
                ret = c;
            }
            cur = w.next[c as usize];
        }
        if w.entries[ret as usize].priority < src.priority {
            return None;
        }
        ret
    };
    let e = &mut w.entries[i as usize];
    let (x2, x3) = (e.x2, e.x3);
    *e = Wind {
        x2,
        x3,
        age: 0,
        ..*src
    };
    Some(i)
}

/// A blast's wind from `pos`, `lb_800119DC`: priority 100, everywhere.
#[doc(alias = "lb_800119DC")]
pub fn blast(w: &mut Winds, pos: Vec3, frames: i32, scale: f32, decay: f32, angle_step: f32) {
    let src = Wind {
        kind: 2,
        priority: 0x64,
        pos,
        scale,
        decay,
        frames,
        angle_step,
        x10: -10000.0,
        x14: 10000.0,
        x18: 10000.0,
        x1c: -10000.0,
        ..Wind::default()
    };
    add(w, &src);
}

/// Ages the wind sources, `lb_800115F4`: each fades and counts down, the spent ones go back to
/// the free list. Returns the new wind level from `level`: 1 when boxes' pushes start to add up
/// to over 0.1, 2 while they do, -1 when they stop, else 0.
#[doc(alias = "lb_800115F4")]
pub fn tick(w: &mut Winds, level: i32) -> i32 {
    let mut total = 0.0;
    let mut cur = w.active;
    while let Some(c) = cur {
        let e = &mut w.entries[c as usize];
        if e.kind == 1 {
            total = fp::fadds(total, reg(e.scale));
        }
        e.scale = mem(fp::fsubs(reg(e.scale), reg(e.decay)));
        if reg(e.scale) < 0.0 {
            e.scale = 0.0;
        }
        if e.frames > 0 {
            e.frames -= 1;
        }
        e.age = e.age.wrapping_add(1);
        let next = w.next[c as usize];
        if e.frames == 0 {
            if w.active == Some(c) {
                w.active = next;
            } else {
                let mut head = w.active.expect("a wind source outside the list");
                while w.next[head as usize] != Some(c) {
                    head = w.next[head as usize].expect("a wind source outside the list");
                }
                w.next[head as usize] = next;
            }
            w.next[c as usize] = w.free;
            w.free = Some(c);
        }
        cur = next;
    }
    if total > 0.1 {
        return if level > 0 { 2 } else { 1 };
    }
    if level > 0 { -1 } else { 0 }
}
