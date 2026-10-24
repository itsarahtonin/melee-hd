//! The main menu's lookups against ssbm-rs's ports: which of a list's selections show for the
//! save's unlocks (`mn_80229938`) and how many show before one (`mn_80229A04`), and each
//! list's light color (`mn_8022C010`, `mn_8022BFBC`).

use leaf_check::{reference, sym};
use melee_sim::game::mn::main::{LIGHT_COLORS, light_color, shown_before_with, shown_with};
use ssbm_types::fns;

/// The save data's unlock flags (`save_data.x186C`): the game data's byte 0x186C (the card
/// data starting at 0x1868), through `gmMainLib_804D3EE0`, pointed at the data if the
/// reference hasn't yet.
fn x186c(ctx: &ssbm_rt::Ctx) -> u32 {
    let ptr = sym("gmMainLib_804D3EE0");
    if ctx.read_u32(ptr) == 0 {
        ctx.write_u32(ptr, sym("gmMainLib_8045A6C0"));
    }
    ctx.read_u32(ptr) + 0x186C
}

#[test]
fn shown_selections_match() {
    let Some(ctx) = reference() else { return };
    let at = x186c(&ctx);
    for flags in [0u8, 4, 8, 0xC, 0xF3, 0xFF] {
        ctx.write_u8(at, flags);
        for kind in 0..34u8 {
            for sel in 0..12 {
                let theirs = fns::mn_80229938(&ctx, i32::from(kind), sel) != 0;
                assert_eq!(
                    shown_with(flags, kind, sel),
                    theirs,
                    "mn_80229938({kind}, {sel}) with {flags:#x}"
                );
                let theirs = fns::mn_80229A04(&ctx, i32::from(kind), sel);
                assert_eq!(
                    shown_before_with(flags, kind, sel),
                    theirs,
                    "mn_80229A04({kind}, {sel}) with {flags:#x}"
                );
            }
        }
    }
}

#[test]
fn light_colors_match() {
    let Some(ctx) = reference() else { return };
    for kind in 0..34u8 {
        let sels = if kind == 0 { 0..5u16 } else { 0..12 };
        for sel in sels {
            let group = fns::mn_8022C010(&ctx, i32::from(kind), i32::from(sel));
            assert_eq!(
                i32::from(light_color(kind, sel)),
                group,
                "mn_8022C010({kind}, {sel})"
            );
            let color = ssbm_rt::Handle::addr(fns::mn_8022BFBC(&ctx, group));
            assert_eq!(
                color,
                sym("mn_804D4B50") + 4 * group as u32,
                "mn_8022BFBC({group})"
            );
            let rgb = [0, 1, 2].map(|k| ctx.read_u8(color + k));
            assert_eq!(LIGHT_COLORS[group as usize], rgb, "color {group}");
        }
    }
}
