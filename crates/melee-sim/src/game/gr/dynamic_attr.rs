//! Dynamic attribute areas, `melee/gr/grdynamicattr.c`: up to four circles on floors that
//! change the terrain under them for a while, on an in-use list and a free list.

use super::super::state::Game;
use super::{DynamicAttr, DynamicAttrs};

/// Puts every area on the free list, `grDynamicAttr_801CA0B4`: the last area first.
#[doc(alias = "grDynamicAttr_801CA0B4")]
pub fn reset(attrs: &mut DynamicAttrs) {
    attrs.active = None;
    attrs.free = None;
    let mut prev = None;
    for i in 0..attrs.areas.len() as u8 {
        attrs.areas[i as usize].next = prev;
        prev = Some(i);
        attrs.free = prev;
    }
}

/// Takes a free area for a circle of `radius` around `pos` on `floor`, `grDynamicAttr_801CA0F8`.
#[doc(alias = "grDynamicAttr_801CA0F8")]
pub fn add(
    attrs: &mut DynamicAttrs,
    attr: i32,
    pos: crate::vec::Vec3,
    floor: i32,
    radius: f32,
    timer: i32,
) -> Option<u8> {
    assert_ne!(floor, -1, "grdynamicattr.c: floor_id!=GC_Id_None");
    let i = attrs.free?;
    let next_free = attrs.areas[i as usize].next;
    attrs.free = next_free;
    attrs.areas[i as usize] = DynamicAttr {
        next: attrs.active,
        attr,
        pos,
        floor,
        radius,
        timer,
    };
    attrs.active = Some(i);
    Some(i)
}

/// Returns an area to the free list, `grDynamicAttr_801CA1C0`. The original also frees an
/// area it doesn't find in use.
#[doc(alias = "grDynamicAttr_801CA1C0")]
pub fn remove(attrs: &mut DynamicAttrs, i: u8) {
    if attrs.active == Some(i) {
        attrs.active = attrs.areas[i as usize].next;
    } else {
        let mut cur = attrs.active;
        while let Some(c) = cur {
            if attrs.areas[c as usize].next == Some(i) {
                attrs.areas[c as usize].next = attrs.areas[i as usize].next;
                return;
            }
            cur = attrs.areas[c as usize].next;
        }
    }
    attrs.areas[i as usize].next = attrs.free;
    attrs.free = Some(i);
}

/// Counts the areas' timers down, freeing those that run out, `grDynamicAttr_801CA224`.
#[doc(alias = "grDynamicAttr_801CA224")]
pub fn update(game: &mut Game) {
    let attrs = &mut game.gr.dynamic_attrs;
    let mut cur = attrs.active;
    while let Some(c) = cur {
        let next = attrs.areas[c as usize].next;
        let a = &mut attrs.areas[c as usize];
        if a.timer > 0 {
            a.timer -= 1;
            if a.timer <= 0 {
                remove(attrs, c);
            }
        }
        cur = next;
    }
}
