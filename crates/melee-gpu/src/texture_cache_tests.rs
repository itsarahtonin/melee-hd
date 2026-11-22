// SPDX-License-Identifier: GPL-3.0-or-later

//! The texture cache's eviction (`to_evict`): a texture decoded from memory stays while the
//! decoded textures fit their budget, however long nothing samples it, so one that comes back
//! isn't decoded again (docs/fast-renderer.md, F5's fourth round); past the budget the least
//! recently used go; copies from the EFB go after `COPY_KEPT_FOR` frames unused.

use crate::{COPY_KEPT_FOR, to_evict};

#[test]
fn decoded_textures_stay_within_the_budget_and_the_oldest_go_past_it() {
    // Slots 1 to 3 decoded (100 bytes each, last used at frames 5, 50 and 900), 4 a copy.
    let entries = [(1, 5, 100), (2, 50, 100), (3, 900, 100), (4, 10, 0)];
    assert_eq!(to_evict(&entries, 1000, 300), vec![4], "within the budget");
    let mut gone = to_evict(&entries, 1000, 150);
    gone.sort_unstable();
    assert_eq!(gone, vec![1, 2, 4], "past it, the least recently used");
    // A copy used recently stays.
    assert!(to_evict(&[(4, 990, 0)], 1000, 0).is_empty());
    assert_eq!(
        to_evict(&[(4, 1000 - COPY_KEPT_FOR - 1, 0)], 1000, 0),
        vec![4]
    );
    // Nothing used this frame goes, even past the budget.
    assert!(to_evict(&[(1, 1000, 500)], 1000, 0).is_empty());
}

/// On a renderer: a texture unused for far longer than a copy is kept stays.
#[test]
fn a_renderer_keeps_a_texture_unused_for_a_while() {
    let Some(mut r) = crate::testing::renderer() else {
        return;
    };
    let id = r.add_texture(4, 4, &[vec![0x80; 64]]) as usize;
    let copy = r.copy_texture(8, 8) as usize;
    r.frame += 10 * COPY_KEPT_FOR;
    r.evict();
    assert!(r.textures[id].is_some(), "the decoded texture stays");
    assert!(r.textures[copy].is_none(), "the copy's goes");
}

/// A texture's key holds the palette's format only for an image that reads a palette: the
/// format a palette texture left in the register doesn't make an RGB texture again, and does
/// make a palette texture again.
#[test]
fn the_palette_format_keys_only_a_palette_texture() {
    use melee_gx::State;
    use melee_gx::texture::{SETIMAGE0, SETIMAGE3, SETTLUT, reg};
    let Some(mut r) = crate::testing::renderer() else {
        return;
    };
    let mem = crate::mesh_tests::Flat((0..4096).map(|i| (i * 7) as u8).collect());
    let id = |r: &mut crate::Renderer, format: u32, tlut_format: u32| {
        let mut s = State::new();
        s.bp[reg(SETIMAGE0, 0)] = 7 | 7 << 10 | format << 20;
        s.bp[reg(SETIMAGE3, 0)] = 0x400 >> 5;
        s.bp[reg(SETTLUT, 0)] = tlut_format << 10;
        r.texture_resolved(&s, 0, &mem).0
    };
    // CMPR (14) and C4 (8).
    let rgb = id(&mut r, 14, 0);
    assert_eq!(
        id(&mut r, 14, 2),
        rgb,
        "an RGB texture with another palette format"
    );
    let palette = id(&mut r, 8, 0);
    assert_ne!(id(&mut r, 8, 2), palette, "a palette texture with another");
}

/// A texture prepared from its bytes before it's drawn is the one a draw of those bytes takes,
/// wherever they are; other bytes make their own.
#[test]
fn a_draw_takes_the_texture_prepared_from_its_bytes() {
    use melee_gx::State;
    use melee_gx::texture::{SETIMAGE0, SETIMAGE3, reg};
    let Some(mut r) = crate::testing::renderer() else {
        return;
    };
    let bytes: Vec<u8> = (0..4096).map(|i| (i * 13) as u8).collect();
    // An 8 by 8 RGB565 image (128 bytes) at 0x400.
    let prep = crate::TexturePrep {
        width: 8,
        height: 8,
        format: 4,
        levels: 1,
        tlut_format: 0,
        tlut: Vec::new(),
        bytes: bytes[0x400..0x480].into(),
    };
    assert_eq!(r.prepare_textures(std::slice::from_ref(&prep)), 1);
    assert_eq!(r.prepare_textures(&[prep]), 0, "made once");
    let made = r.textures.iter().flatten().count();
    let mem = crate::mesh_tests::Flat(bytes);
    let id = |r: &mut crate::Renderer, at: u32| {
        let mut s = State::new();
        s.bp[reg(SETIMAGE0, 0)] = 7 | 7 << 10 | 4 << 20;
        s.bp[reg(SETIMAGE3, 0)] = at >> 5;
        r.texture_resolved(&s, 0, &mem).0
    };
    id(&mut r, 0x400);
    assert_eq!(
        r.textures.iter().flatten().count(),
        made,
        "the prepared one"
    );
    id(&mut r, 0x480);
    assert_eq!(r.textures.iter().flatten().count(), made + 1, "other bytes");
}
