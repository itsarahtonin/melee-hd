//! Pokémon Stadium's state outside its objects' variables: the screens' user data, which the
//! original allocates with `HSD_MemAlloc` and frees with `HSD_Free`, its parameters' pointer
//! and the transformation files loaded.

use melee_sim::game::gr::stadium::{SCREEN_FEED, SCREEN_TEXT, SCREEN_ZOOM, Screen};
use melee_sim::game::mirror::{Cx, Mem, Mirror};
use melee_sim::game::state::UserData;
use melee_sim::game::structs::HSD_ImageDesc;
use melee_sim::hsd::sis::TextId;

use crate::MEM1;
use crate::compare::Comparer;
use crate::import::{Importer, sym};

/// `HSD_Free`, the screens' release function.
pub const HSD_FREE: u32 = 0x8037_F1B0;

/// `yakumono_param` in grpstadium.c.
pub const YAKUMONO: u32 = 0x804D_6978;

/// The screen a GObj's render callback makes it.
pub fn screen_of_render(render: u32) -> Option<u8> {
    match render {
        0x801D_2ED0 => Some(SCREEN_TEXT),
        0x801D_2FD0 => Some(SCREEN_FEED),
        0x801D_3084 => Some(SCREEN_ZOOM),
        _ => None,
    }
}

fn w(mem: &[u8], a: u32) -> u32 {
    let i = (a - MEM1) as usize;
    u32::from_be_bytes(mem[i..i + 4].try_into().unwrap())
}

fn h(mem: &[u8], a: u32) -> u16 {
    let i = (a - MEM1) as usize;
    u16::from_be_bytes(mem[i..i + 2].try_into().unwrap())
}

/// The screen whose user data (its image descriptor first) is at `p`, if any: the GObjs on
/// the process links whose render callback is a screen's.
pub fn screen_at(mem: &[u8], p: u32) -> Option<u8> {
    let heads = w(mem, sym("HSD_GObjPLinkHead"));
    for link in 0..melee_sim::hsd::gobj::P_LINKS as u32 {
        let mut g = w(mem, heads + 4 * link);
        while g != 0 {
            if let Some(s) = screen_of_render(w(mem, g + 0x1C))
                && w(mem, g + 0x2C) == p
            {
                return Some(s);
            }
            g = w(mem, g + 8);
        }
    }
    None
}

/// Imports a screen's user data at `ud` into the stage's state; its index.
pub(crate) fn import_screen(imp: &mut Importer, which: u8, ud: u32) -> UserData {
    let mem = Mem {
        bytes: &imp.cap.call.mem1,
        base: MEM1,
    };
    let desc = HSD_ImageDesc::import(&mem, ud, imp);
    let m: &[u8] = &imp.cap.call.mem1;
    let screen = if which == SCREEN_TEXT {
        let base = w(m, sym("HSD_SisLib_804D796C"));
        let text = |a: u32| (a != 0).then(|| TextId(a - base));
        Screen {
            desc,
            win_static: text(w(m, ud + 0x18)),
            win_dynamic: text(w(m, ud + 0x1C)),
            ..Default::default()
        }
    } else if which == SCREEN_ZOOM {
        Screen {
            desc,
            flag: h(m, ud + 0x18) & 0x8000 != 0,
            copy_x: h(m, ud + 0x1A),
            copy_y: h(m, ud + 0x1C),
            ..Default::default()
        }
    } else {
        // The feed's block (0x1C bytes) ends at `x1A`, which nothing writes.
        Screen {
            desc,
            flag: h(m, ud + 0x18) & 0x8000 != 0,
            ..Default::default()
        }
    };
    imp.game.gr.stadium.screens[which as usize] = Some(Screen { addr: ud, ..screen });
    UserData::StadiumScreen(which)
}

/// Compares a screen's user data with memory (the GObj at `t`).
pub(crate) fn compare_screen(cmp: &mut Comparer, path: &str, which: u8, t: u32) {
    let mem: &[u8] = cmp.mem;
    let ud = w(mem, t + 0x2C);
    let release = w(mem, t + 0x30);
    let render = w(mem, t + 0x1C);
    cmp.check(
        path,
        "screen and release",
        (Some(which), HSD_FREE),
        (screen_of_render(render), release),
    );
    let Some(s) = cmp.game.gr.stadium.screens[which as usize].as_ref() else {
        cmp.diff(path, "screen user data", "none", format!("{ud:#x}"));
        return;
    };
    let m = Mem {
        bytes: cmp.mem,
        base: MEM1,
    };
    s.desc
        .compare(&m, ud, &format!("{path} screen {which} desc"), cmp);
    if which == SCREEN_TEXT {
        let base = w(mem, sym("HSD_SisLib_804D796C"));
        let text = |a: u32| (a != 0).then(|| a - base);
        cmp.check(
            path,
            "screen texts",
            (s.win_static.map(|t| t.0), s.win_dynamic.map(|t| t.0)),
            (text(w(mem, ud + 0x18)), text(w(mem, ud + 0x1C))),
        );
    } else if which == SCREEN_ZOOM {
        cmp.check(
            path,
            "screen flag and copy origin",
            (s.flag, s.copy_x, s.copy_y),
            (
                h(mem, ud + 0x18) & 0x8000 != 0,
                h(mem, ud + 0x1A),
                h(mem, ud + 0x1C),
            ),
        );
    } else {
        // The flag is the halfword's top bit; the rest and `x1A` are never written.
        cmp.check(path, "screen flag", s.flag, h(mem, ud + 0x18) & 0x8000 != 0);
    }
}

/// The stage's statics: its parameters' pointer and the transformation files memory holds.
pub(crate) fn import_statics(imp: &mut Importer) {
    let yaku = imp.cap.u32(YAKUMONO);
    imp.game.gr.stadium.yakumono = imp.data_or_unhandled(yaku);
    for (i, name) in melee_sim::game::gr::stadium::FILES.iter().enumerate() {
        imp.game.gr.stadium.files[i] = imp.data.archive_named(name);
    }
    // The transformation buffer's address, as map 2 holds it.
    let info = sym("stage_info");
    let map2 = imp.cap.u32(info + 0x180 + 8);
    imp.game.gr.stadium.buffer_address =
        if imp.cap.u32(info + 0x88) as i32 == melee_sim::game::gr::GR_KIND_PSTADIUM && map2 != 0 {
            imp.cap.u32(imp.cap.u32(map2 + 0x2C) + 0xC4 + 8)
        } else {
            melee_sim::game::ft::data::MAIN_MEMORY
        };
}

/// Compares the stage's statics.
pub(crate) fn compare_statics(cmp: &mut Comparer) {
    let yaku = cmp.u32(YAKUMONO);
    cmp.compare_data(
        "grpstadium yakumono_param",
        cmp.game.gr.stadium.yakumono,
        yaku,
    );
}
