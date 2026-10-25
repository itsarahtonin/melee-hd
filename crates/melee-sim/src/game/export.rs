//! The game's objects laid out as the original keeps them, for the heap's image
//! ([`crate::hsd::image`]) as a scene's heap is made anew: the scheduler's GObjs and processes,
//! and what [`crate::hsd::export`] lays out of HSD's objects. A field melee-sim doesn't know
//! (a user data block of a kind it doesn't place, the remove function) stays unknown.

use melee_data::archive::Archives;

use super::state::{Game, Obj, UserData};
use crate::hsd::export::{Lay, live_objects};
use crate::hsd::gobj::{GObjId, ProcId};
use crate::hsd::image::Image;

/// Where a GObj's user data lives, where melee-sim knows it.
fn user_data_addr(game: &Game, ud: &UserData) -> Option<u32> {
    let known = |a: u32| (a != 0).then_some(a);
    match *ud {
        UserData::Fighter(f) => known(game.fighters.addr(f.0)),
        UserData::Item(i) => known(game.items.addr(i.0)),
        UserData::ItemLink(l) => known(game.it.links.addr(l.0)),
        UserData::Ground(g) => known(game.grounds.addr(g.0)),
        UserData::BgFlash => known(game.bgflash.user_addr),
        UserData::Sound(s) => known(game.sound_objs.addr(s.0)),
        UserData::Effect(e) => known(game.ef.effects.addr(e.0)),
        UserData::Text(t) => {
            (game.sis.arena_block != 0).then(|| game.sis.arena_block.wrapping_add(t.0))
        }
        UserData::MainMenuList(a)
        | UserData::MainMenuPanel(a)
        | UserData::GalleryList(a)
        | UserData::GalleryCursor(a)
        | UserData::RulesList(a)
        | UserData::RulePlusList(a)
        | UserData::ItemSwitch(a)
        | UserData::StageSwitch(a)
        | UserData::NameList(a)
        | UserData::NameNew(a)
        | UserData::GlyphVariants(a)
        | UserData::SssName(a)
        | UserData::SssPreview(a)
        | UserData::SssHover(a)
        | UserData::OptionMenu(a)
        | UserData::Vibration(a)
        | UserData::DataDel(a) => known(a),
        _ => None,
    }
}

/// `HSD_GObj` (0x38).
fn gobj(game: &Game, img: &mut Image, g: GObjId) {
    let w = &game.world;
    let at = w.gobjs.addr(g.0);
    if at == 0 {
        return;
    }
    let o = &w.gobjs[g.0];
    let ga = |x: Option<GObjId>| x.map_or(0, |x| w.gobjs.addr(x.0));
    let mut l = Lay::new(at, 0x38);
    l.u16(0, o.classifier);
    l.bytes(
        2,
        &[
            o.p_link,
            o.gx_link,
            o.p_priority,
            o.render_priority,
            o.obj_kind,
            o.user_data_kind,
        ],
    );
    l.u32(8, ga(o.next));
    l.u32(0xC, ga(o.prev));
    l.u32(0x10, ga(o.next_gx));
    l.u32(0x14, ga(o.prev_gx));
    l.u32(0x18, o.proc.map_or(0, |p| w.procs.addr(p.0)));
    l.u32(0x1C, o.render.map_or(0, |c| c.0));
    l.u32(0x20, (o.gxlink_prios >> 32) as u32);
    l.u32(0x24, o.gxlink_prios as u32);
    let objs = &game.objs;
    let obj = match o.obj {
        None => Some(0),
        Some(Obj::JObj(j)) => Some(objs.jobjs.pool.addr(j.0)),
        Some(Obj::CObj(c)) => Some(objs.cobjs.pool.addr(c.0)),
        Some(Obj::LObj(x)) => Some(objs.lobjs.pool.addr(x.0)),
        Some(Obj::Fog(f)) => Some(objs.fogs.fogs.addr(f.0)),
        Some(Obj::SObj(s)) => Some(s.map_or(0, |s| objs.sobjs.addr(s.0))),
    };
    l.word(0x28, obj.filter(|&a| a != 0 || o.obj.is_none()));
    let ud = match &o.user_data {
        None => Some(0),
        Some(u) => user_data_addr(game, u),
    };
    l.word(0x2C, ud);
    l.put(img);
}

/// `HSD_GObjProc` (0x18).
fn proc(game: &Game, data: &Archives, img: &mut Image, p: ProcId) {
    let w = &game.world;
    let at = w.procs.addr(p.0);
    if at == 0 {
        return;
    }
    let o = &w.procs[p.0];
    let pa = |x: Option<ProcId>| x.map_or(0, |x| w.procs.addr(x.0));
    let mut l = Lay::new(at, 0x18);
    l.u32(0, pa(o.child));
    l.u32(4, pa(o.next));
    l.u32(8, pa(o.prev));
    l.u8(0xC, o.s_link);
    // The flags' byte's low four bits are what the block held (`flags_1` to `flags_3` are
    // its top bits' bitfields).
    if let Some(old) = img.byte_at(data, at + 0xD) {
        let top = u8::from(o.flags_1) << 7 | u8::from(o.flags_2) << 6 | (o.flags_3 & 3) << 4;
        l.u8(0xD, top | old & 0xF);
    }
    l.u32(0x10, w.gobjs.addr(o.gobj.0));
    l.u32(0x14, o.on_invoke.0);
    l.put(img);
}

/// What the scene's heap holds as it's made anew, as far as melee-sim lays it out: the
/// scheduler's GObjs and processes (the scene's own, live or freed), HSD's objects the scene
/// made and still has, its stage parts (`Ground`), and the free lists' links over them.
pub fn scene_end(game: &Game, data: &Archives, img: &mut Image) {
    live_objects(&game.objs, img);
    crate::hsd::export::id_table(&game.objs, img);
    for (slot, _) in game.world.gobjs.slots().iter().enumerate() {
        gobj(game, img, GObjId(slot as u32));
    }
    for (slot, _) in game.world.procs.slots().iter().enumerate() {
        proc(game, data, img, ProcId(slot as u32));
    }
    bg_flash(game, img);
    let kind = std::any::type_name::<super::structs::Ground>();
    for (slot, _) in game.grounds.iter() {
        if img.in_scene(kind, slot) {
            ground(game, img, super::handles::GroundId(slot));
        }
    }
}

/// A color overlay (`ColorStruct`, 0x80) at `off` of `l`: its script's place where the image
/// knows the file it's in.
fn color_overlay(img: &Image, l: &mut Lay, off: u32, c: &super::structs::ColorOverlay) {
    use crate::lb::command::Slot;
    let s = &c.script;
    l.u32(off, s.timer);
    l.u32(off + 4, s.frame_count);
    l.word(off + 8, crate::hsd::export::data_addr(img, s.cursor));
    l.u32(off + 0xC, s.loop_count);
    for (i, r) in s.ret.iter().enumerate() {
        let w = match *r {
            Slot::Data(d) => img.data_addr(d),
            Slot::Word(w) => Some(w),
        };
        l.word(off + 0x10 + 4 * i as u32, w);
    }
    l.u32(off + 0x24, c.x24 as u32);
    l.bytes(off + 0x28, &c.x28_colanim);
    let rgba = |g: &super::structs::_GXColor| [g.r, g.g, g.b, g.a];
    l.bytes(off + 0x2C, &rgba(&c.x2C_hex));
    l.f32s(
        off + 0x30,
        &[
            c.x30_color_red,
            c.x34_color_green,
            c.x38_color_blue,
            c.x3C_color_alpha,
            c.x40_colorblend_red,
            c.x44_colorblend_green,
            c.x48_colorblend_blue,
            c.x4C_colorblend_alpha,
        ],
    );
    l.bytes(off + 0x50, &rgba(&c.x50_light_color));
    l.f32s(
        off + 0x54,
        &[
            c.x54_light_red,
            c.x58_light_green,
            c.x5C_light_blue,
            c.x60_light_alpha,
            c.x64_lightblend_red,
            c.x68_lightblend_green,
            c.x6C_lightblend_blue,
            c.x70_lightblend_alpha,
            c.x74_light_rot_x,
            c.x78_light_rot_yz,
        ],
    );
    let bits = [
        c.x7C_color_enable,
        c.x7C_flag2,
        c.x7C_light_enable,
        c.x7C_flag4,
        c.x7C_flag5,
        c.x7C_flag6,
        c.x7C_flag7,
        c.x7C_flag8,
    ];
    l.u8(off + 0x7C, msb_bits(&bits));
}

/// Bitfield flags as the original packs them, the first in the top bit.
fn msb_bits(bits: &[bool; 8]) -> u8 {
    bits.iter().fold(0, |b, &f| b << 1 | u8::from(f))
}

/// The background flash's user data (`BgFlashUserData`, 0x84), while it has one.
pub fn bg_flash(game: &Game, img: &mut Image) {
    let (Some(u), at) = (&game.bgflash.user, game.bgflash.user_addr) else {
        return;
    };
    if at == 0 {
        return;
    }
    let mut l = Lay::new(at, 0x84);
    l.u8(0, u.x0);
    color_overlay(img, &mut l, 4, &u.x4);
    l.put(img);
}

/// `Ground` (0x204), as its block goes back to the heap: a stage part's union is laid out
/// where melee-sim keeps it as bytes (the typed ones stay unknown).
pub fn ground(game: &Game, img: &mut Image, g: super::handles::GroundId) {
    let at = game.grounds.addr(g.0);
    if at == 0 {
        return;
    }
    let o = &game.grounds[g.0];
    let ga = |x: Option<GObjId>| x.map_or(0, |x| game.world.gobjs.addr(x.0));
    let code = |c: Option<super::handles::CodeRef>| c.map_or(0, |c| c.0);
    let mut l = Lay::new(at, 0x204);
    l.u32(0, o.x0 as u32);
    l.u32(4, ga(o.gobj));
    l.u32(8, code(o.x8_callback));
    l.u32(0xC, code(o.xC_callback));
    let f = &o.x10_flags;
    l.u8(
        0x10,
        msb_bits(&[f.b0, f.b1, f.b2, f.b3, f.b4, f.b5, f.b6, f.b7]),
    );
    let f = &o.x11_flags;
    l.u8(
        0x11,
        (f.b012 & 7) << 5 | msb_bits(&[false, false, false, f.b3, f.b4, f.b5, f.b6, f.b7]),
    );
    l.u32(0x14, o.map_id as u32);
    l.u32(0x18, ga(o.x18));
    l.u32(0x1C, code(o.x1C_callback));
    for (i, &v) in o.x20.iter().enumerate() {
        l.u32(0x20 + 4 * i as u32, v as u32);
    }
    color_overlay(img, &mut l, 0x40, &o.color_overlay);
    l.f32(0xC0, o.xC0);
    if let super::gr::GroundVars::Bytes(b) = &o.u {
        l.bytes(0xC4, b);
    }
    l.put(img);
}
