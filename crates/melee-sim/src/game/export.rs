//! The game's objects laid out as the original keeps them, for the heap's image
//! ([`crate::hsd::image`]) as a scene's heap is made anew: the scheduler's GObjs and processes,
//! and what [`crate::hsd::export`] lays out of HSD's objects. A field melee-sim doesn't know
//! (a user data block of a kind it doesn't place, the remove function) stays unknown.

use melee_data::archive::Archives;

use super::handles::{FighterId, ItemId};
use super::mirror::{LayCx, LayOut, Mirror};
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
/// made and still has, and the free lists' links over them.
pub fn scene_end(game: &Game, data: &Archives, img: &mut Image) {
    live_objects(&game.objs, data, img);
    crate::hsd::export::id_table(&game.objs, img);
    for (slot, _) in game.world.gobjs.slots().iter().enumerate() {
        gobj(game, img, GObjId(slot as u32));
    }
    for (slot, _) in game.world.procs.slots().iter().enumerate() {
        proc(game, data, img, ProcId(slot as u32));
    }
    bg_flash(game, img);
}

/// The game's objects the scene's heap holds as it ends, laid out before the heap's claims
/// go (`Heap::end_scene_image`), so a layout over what a block held reads under its owner's
/// claim: the stage's collision joints (when this scene loaded them, `geometry_live`), the
/// scene's fighters with their parts tables and DObj lists, its items, its stage parts
/// (`Ground`), its deferred spawns and its animation buffers.
pub fn scene_game_objects(
    game: &Game,
    data: &Archives,
    img: &mut Image,
    geometry_live: bool,
    dynamics_live: bool,
) {
    if geometry_live {
        coll_joints(game, data, img);
        coll_arrays(game, data, img);
        zako_slots(game, data, img);
    }
    if dynamics_live {
        dynamics_links(game, data, img);
    }
    // The scene's own objects (an earlier scene's slots name blocks of its freed heap).
    let fighters = std::any::type_name::<super::structs::Fighter>();
    for (slot, _) in game.fighters.iter() {
        if img.in_scene(fighters, slot) {
            fighter(game, data, img, FighterId(slot));
            fighter_parts(game, data, img, FighterId(slot));
        }
    }
    let items = std::any::type_name::<super::structs::Item>();
    for (slot, _) in game.items.iter() {
        if img.in_scene(items, slot) {
            item(game, data, img, ItemId(slot));
        }
    }
    let kind = std::any::type_name::<super::structs::Ground>();
    for (slot, _) in game.grounds.iter() {
        if img.in_scene(kind, slot) {
            ground(game, data, img, super::handles::GroundId(slot));
        }
    }
    let kind = std::any::type_name::<super::ef::Queued>();
    for (slot, _) in game.ef.queued.iter() {
        if img.in_scene(kind, slot) {
            queued(game, img, super::ef::QueuedId(slot));
        }
    }
    let bufs = std::any::type_name::<super::ft::anim_buf::AnimBuf>();
    for (slot, _) in game.fighter_anim_bufs.iter() {
        if img.in_scene(bufs, slot) {
            anim_buf(game, img, slot);
        }
    }
}

/// The stage's vertices and lines (`groundCollVtx`, 0x18 each, and `groundCollLine`, 8 each),
/// as their blocks go back with the scene's heap: each vertex's data position, place and
/// place before; each line in the data's groups its data's line and flags; the rest of each
/// block (past the stage's, and a line no group holds) what it held.
fn coll_arrays(game: &Game, data: &Archives, img: &mut Image) {
    let Some(g) = &game.geometry else { return };
    if g.vertices_addr != 0 {
        let at = g.vertices_addr;
        let mut l = Lay::leftover(img, data, at, 2048 * 0x18);
        for (i, v) in g.vertices.iter().enumerate().take(2048) {
            let a = at + 0x18 * i as u32;
            for (k, f) in [v.base.x, v.base.y, v.pos.x, v.pos.y, v.prev.x, v.prev.y]
                .into_iter()
                .enumerate()
            {
                l.put_u32(a + 4 * k as u32, f.to_bits());
            }
        }
        l.put(img);
    }
    if g.lines_addr != 0 {
        let at = g.lines_addr;
        let lines = match game.stage_info.coll_data {
            None => None,
            Some(c) => data.ptr(c.at(8)).and_then(|d| img.data_addr(d)),
        };
        let mut l = Lay::leftover(img, data, at, 1536 * 8);
        for r in &g.ranges {
            for id in r.start as i32..r.start as i32 + r.count as i32 {
                let Some(line) = g.lines.get(id as usize) else {
                    continue;
                };
                let a = at + 8 * id as u32;
                // A stage without collision data's line is the executable's (`mpLib_803BF760`).
                l.put_word(a, lines.map(|d| d + 0x10 * id as u32));
                l.put_u32(a + 4, line.flags);
            }
        }
        l.put(img);
    }
}

/// The enemy spawner's slots (`grZakoGenerator_Data`, 81 of 12 bytes), as their block goes
/// back with the scene's heap: each slot's point, timer and spawn; its trophy word where the
/// stage wrote it, else what the block held.
fn zako_slots(game: &Game, data: &Archives, img: &mut Image) {
    let z = &game.gr.zako;
    let (Some(slots), at) = (&z.slots, z.addr) else {
        return;
    };
    if at == 0 {
        return;
    }
    let mut l = Lay::leftover(img, data, at, 0x3CC);
    let cx = Addrs { game, img };
    for (i, s) in slots.iter().enumerate().take(81) {
        let a = at + 0xC * i as u32;
        l.put_u16(a, s.point as u16);
        l.put_u16(a + 2, s.timer as u16);
        l.put_word(a + 4, cx.handle("GObjId", s.item.map(|g| g.0)));
        if let Some(x8) = s.x8 {
            l.put_u32(a + 8, x8 as u32);
        }
    }
    l.put(img);
}

/// The dynamics' links (`lb_8000FCDC`'s block, 0x140 links of 0x98), as their block goes
/// back with its scene's heap: each link as melee-sim holds it, its unwritten fields (the last
/// link's dominant axis, `unk_54`) what the block held.
fn dynamics_links(game: &Game, data: &Archives, img: &mut Image) {
    let block = game.ft.dynamics_block;
    let len = crate::lb::dynamics::LINKS * 0x98;
    let mut l = Lay::leftover(img, data, block, len);
    let cx = Addrs { game, img };
    for (k, link) in game.fighter_dynamics.iter() {
        link.lay(&mut l, block + 0x98 * k, &cx);
    }
    l.put(img);
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

/// `Ground` (0x204), as its block goes back to the heap, over what the block held where the
/// stage's code never writes (the padding, a typed member's bytes past its fields): a stage
/// part's union is laid out where melee-sim keeps it as bytes or lays its member out
/// ([`ground_vars`]); another typed member's bytes are its type's, unknown.
pub fn ground(game: &Game, data: &Archives, img: &mut Image, g: super::handles::GroundId) {
    let at = game.grounds.addr(g.0);
    if at == 0 {
        return;
    }
    let o = &game.grounds[g.0];
    let ga = |x: Option<GObjId>| x.map_or(0, |x| game.world.gobjs.addr(x.0));
    let code = |c: Option<super::handles::CodeRef>| c.map_or(0, |c| c.0);
    let mut l = Lay::leftover(img, data, at, 0x204);
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
    match (&o.u, o.u.type_name()) {
        (super::gr::GroundVars::Bytes(b), _) => l.bytes(0xC4, b),
        (v, Some(ty)) => {
            let cx = Addrs { game, img };
            if !ground_vars(&mut l, at + 0xC4, v, &cx) {
                // A typed member isn't laid out yet: its bytes are its type's.
                l.forget_as(at + 0xC4, super::gr::vars::SIZE as u32, ty);
            }
        }
        _ => {}
    }
    l.put(img);
}

/// The typed members of a stage part's union [`ground_vars`] lays out.
pub const LAID_GROUND_VARS: &[&str] = &[
    "ZebesCreatureVars",
    "ZebesBackVars",
    "ZebesBubbleVars",
    "ZebesIslandVars",
    "ZebesStageVars",
    "ZebesLavaVars",
    "CorneriaCityVars",
    "CorneriaCity8Vars",
    "IzumiVars",
    "Izumi2Vars",
    "Izumi3Vars",
];

/// The types of what the heap's image doesn't lay out, each with a note: an object of one
/// leaves unknown bytes in its block as it goes, and a new object made over those stops a
/// session where the original's bytes differ (`Comparer::unlaid_view`, "unknown leftover
/// bytes of a freed TYPE under NEW: its layout isn't ported"). `melee-oracle list-unlaid`
/// prints them; laying them out is a campaign of its own (docs/engine.md, a session's heap
/// leftovers).
pub fn unlaid_types() -> Vec<(String, &'static str)> {
    use crate::hsd::heap::{Class, Kind};
    let mut v = Vec::new();
    for k in Kind::ALL {
        let why = match k {
            Kind::GObj
            | Kind::GObjProc
            | Kind::AObj
            | Kind::FObj
            | Kind::RObj
            | Kind::IdEntry
            | Kind::Fighter
            | Kind::FighterParts
            | Kind::FighterDObjList
            | Kind::FighterX59C
            | Kind::Item
            | Kind::EffectQueued
            | Kind::BgFlash
            | Kind::Particle
            | Kind::Generator
            | Kind::AppSrt
            | Kind::Shadow => continue,
            Kind::SList => "a list node: only the lights' lists are laid out",
            Kind::DList
            | Kind::TevReg
            | Kind::Chan
            | Kind::Render
            | Kind::ZList
            | Kind::DebugMenu => "a pool melee-sim's model allocates nothing from",
            Kind::Rvalue
            | Kind::FighterAttrs
            | Kind::FighterX2040
            | Kind::Player
            | Kind::ItemDynamicBones
            | Kind::ItemLink
            | Kind::Effect
            | Kind::SObj
            | Kind::AudioUserData
            | Kind::Vec
            | Kind::Mtx => "an object pool's objects (`HSD_ObjAlloc`)",
        };
        v.push((k.name().to_owned(), why));
    }
    for c in Class::ALL {
        let why = match c {
            Class::JObj
            | Class::DObj
            | Class::MObj
            | Class::TObj
            | Class::PObj
            | Class::CObj
            | Class::WObj
            | Class::FtJObj
            | Class::FtIntpJObj
            | Class::FtPObj
            | Class::FtMObj
            | Class::GrMObj
            | Class::ItMObj
            | Class::RfDObj
            | Class::RfPObj => continue,
            Class::LObj | Class::Fog => "an HSD class's objects (`hsdNew`)",
        };
        v.push((c.name().to_owned(), why));
    }
    for t in super::gr::GroundVars::TYPED {
        if !LAID_GROUND_VARS.contains(t) {
            v.push((
                (*t).to_owned(),
                "a typed member of a stage part's union (`Ground::u`)",
            ));
        }
    }
    for t in super::it::vars::UNLAID {
        v.push((
            (*t).to_owned(),
            "a hand-written member of an item's variables",
        ));
    }
    for (t, why) in [
        (
            crate::hsd::image::HEAP_BLOCK,
            "an `HSD_MemAlloc` block whose caller names no type",
        ),
        (
            crate::hsd::image::PIECE,
            "a memory piece no class or pool names",
        ),
        (
            "the THP decoder's work area",
            "the movie player's decode buffer",
        ),
        (
            "a GObj's user data remover",
            "the word after a GObj's user data",
        ),
        ("HSD_Archive", "an archive's header block"),
        ("HSD_MemoryEntry", "the memory pieces' size classes"),
        (
            "a disc read's bytes past a file",
            "the bytes a read brings past a file's end",
        ),
    ] {
        v.push((t.to_owned(), why));
    }
    v
}

/// A typed member of a stage part's union at `a`, its fields alone (the bytes past them are
/// what the block held, which the stage's code never writes): whether melee-sim lays the
/// member out (Brinstar's and Corneria's city, so far; the others are unknown,
/// `GroundVars::TYPED`).
fn ground_vars(l: &mut Lay, a: u32, v: &super::gr::GroundVars, cx: &Addrs) -> bool {
    use super::gr::GroundVars as V;
    let gobj = |l: &mut Lay, at: u32, g: Option<GObjId>| {
        l.put_word(at, cx.handle("GObjId", g.map(|g| g.0)));
    };
    let jobj = |l: &mut Lay, at: u32, j: Option<crate::hsd::jobj::JObjId>| {
        l.put_word(at, cx.handle("JObjId", j.map(|j| j.0)));
    };
    // A spawn configuration's address, which melee-sim keeps only once found (0 before).
    let addr = |l: &mut Lay, at: u32, w: u32| {
        if w != 0 {
            l.put_u32(at, w);
        } else {
            l.forget(at, 4);
        }
    };
    let acid = |l: &mut Lay, at: u32, s: &super::gr::vars::AcidState| {
        l.put_u8(at, s.x00);
        l.put_u8(at + 1, s.x01);
        l.put_u16(at + 2, s.x02 as u16);
        for (k, f) in [s.x04, s.x08, s.x0C, s.x10].into_iter().enumerate() {
            l.put_u32(at + 4 + 4 * k as u32, f.to_bits());
        }
        jobj(l, at + 0x14, s.x14);
        jobj(l, at + 0x18, s.x18);
        gobj(l, at + 0x1C, s.x1C);
        l.put_u16(at + 0x20, s.x20 as u16);
    };
    match v {
        V::ZebesCreature(v) => {
            l.put_u8(a, v.x0);
            jobj(l, a + 4, v.stored_jobj);
            l.put_u16(a + 8, v.x8 as u16);
            l.put_u16(a + 0xA, v.xA as u16);
        }
        V::ZebesBack(v) => l.put_u16(a, v.xC4 as u16),
        V::ZebesBubble(v) => {
            gobj(l, a, v.xC4);
            l.put_u32(a + 4, v.xC8 as u32);
        }
        V::ZebesIsland(v) => {
            acid(l, a, &v.acid);
            l.put_u32(a + 0x24, v.xE8 as u32);
            addr(l, a + 0x28, v.xEC);
        }
        V::ZebesStage(v) => {
            l.put_u16(a, v.xC4 as u16);
            l.put_u16(a + 2, v.xC6 as u16);
            acid(l, a + 4, &v.acid);
            l.put_u32(a + 0x28, v.xEC);
            gobj(l, a + 0x2C, v.xF0);
            l.put_u16(a + 0x30, v.xF4 as u16);
            l.put_u16(a + 0x32, v.xF6 as u16);
            l.put_u16(a + 0x34, v.xF8 as u16);
            l.put_u16(a + 0x36, v.xFA as u16);
            addr(l, a + 0x38, v.xFC);
            gobj(l, a + 0x3C, v.x100);
        }
        V::ZebesLava(v) => {
            l.put_u16(a, v.xC4 as u16);
            l.put_u16(a + 2, v.xC6 as u16);
            l.put_u32(a + 4, v.xC8);
            for (k, f) in [v.xCC, v.xD0, v.xD4, v.xD8].into_iter().enumerate() {
                l.put_u32(a + 8 + 4 * k as u32, f.to_bits());
            }
            l.put_word(a + 0x18, cx.handle("LObjId", v.xDC.map(|o| o.0)));
        }
        V::CorneriaCity(v) => {
            l.put_u8(a, v.xC4);
            l.put_u8(a + 1, v.xC5);
            l.put_bits(a + 2, 0, 1, u32::from(v.still));
        }
        V::Izumi(v) => {
            l.put_word(a, cx.handle("TObjId", v.xC4.map(|o| o.0)));
            gobj(l, a + 4, v.xC8);
            gobj(l, a + 8, v.xCC);
            jobj(l, a + 0xC, v.xD0);
            jobj(l, a + 0x10, v.xD4);
        }
        V::Izumi2(v) => {
            jobj(l, a, v.xC4);
            jobj(l, a + 4, v.xC8);
        }
        V::Izumi3(v) => {
            for (k, h) in [v.xC4, v.xC6, v.xC8, v.xCA].into_iter().enumerate() {
                l.put_u16(a + 2 * k as u32, h as u16);
            }
            jobj(l, a + 8, v.xCC);
            for (k, f) in [v.xD0, v.xD4, v.xD8, v.xDC].into_iter().enumerate() {
                l.put_u32(a + 0xC + 4 * k as u32, f.to_bits());
            }
        }
        V::CorneriaCity8(v) => {
            l.put_u8(a, v.xC4);
            l.put_u8(a + 1, v.xC5);
            l.put_u8(a + 2, v.xC6);
            for (k, &w) in v.xC8.iter().enumerate() {
                l.put_u32(a + 4 + 4 * k as u32, w);
            }
            l.put_bits(a + 0x1C, 0, 1, u32::from(v.still));
        }
        _ => return false,
    }
    true
}

/// The addresses the game's handles name now, for a generated layout ([`LayOut`]): a pool's
/// slot's address where the allocator model gives it, a leftover handle's word, and a place in
/// loaded data where the image knows the file's; none (an unknown word) otherwise.
struct Addrs<'a> {
    game: &'a Game,
    img: &'a Image,
}

impl LayCx for Addrs<'_> {
    fn handle(&self, kind: &'static str, h: Option<u32>) -> Option<u32> {
        let Some(h) = h else { return Some(0) };
        if h >= crate::hsd::pool::UNKNOWN {
            return self.img.unknown_word(h);
        }
        let g = self.game;
        let a = match kind {
            "GObjId" => g.world.gobjs.addr(h),
            "JObjId" => g.objs.jobjs.pool.addr(h),
            "DObjId" => g.objs.dobjs.pool.addr(h),
            "MObjId" => g.objs.mobjs.pool.addr(h),
            "TObjId" => g.objs.tobjs.pool.addr(h),
            "LObjId" => g.objs.lobjs.pool.addr(h),
            "FighterId" => g.fighters.addr(h),
            "ItemId" => g.items.addr(h),
            "ItemLinkId" => g.it.links.addr(h),
            "BoneTableId" => g.it.bone_tables.addr(h),
            "PartsId" => g.fighter_parts.addr(h),
            "AttrsId" => g.fighter_attrs.addr(h),
            "AnimBufId" => g.fighter_anim_bufs.addr(h),
            "DObjListId" => g.fighter_dobj_lists.addr(h),
            "DynamicsId" => match g.ft.dynamics_block {
                0 => g.fighter_dynamics.addr(h),
                b => b + 0x98 * h,
            },
            "EffectId" => g.ef.effects.addr(h),
            "QueuedId" => g.ef.queued.addr(h),
            // The items' tables in the executable: the handle is the address.
            "GroundId" => g.grounds.addr(h),
            "CameraBoxId" if g.camera.boxes.addr != 0 => g.camera.boxes.addr + 0x6C * h,
            "ItemLogicRef" | "ItemStatesRef" => h,
            _ => 0,
        };
        (a != 0).then_some(a)
    }

    /// A null place is unknown: the mirror imports a pointer into data no archive holds (the
    /// executable's tables) as none too, so its word can't be told from null.
    fn data(&self, d: Option<melee_data::archive::DataRef>) -> Option<u32> {
        d.and_then(|d| self.img.lay_addr(d))
    }
}

/// `Fighter` (0x23EC), as its block goes back with its scene's heap: the fields the comparer
/// checks, its dynamic bone sets up to their count, over what the block held where creation
/// leaves it (a field the original wrote but melee-sim doesn't know stays unknown).
pub fn fighter(game: &Game, data: &Archives, img: &mut Image, f: FighterId) {
    let at = game.fighters.addr(f.0);
    if at == 0 {
        return;
    }
    let o = &game.fighters[f.0];
    let mut l = Lay::leftover(img, data, at, super::structs::Fighter::SIZE);
    let cx = Addrs { game, img };
    o.lay(&mut l, at, &cx);
    let n = o.dynamics_num.max(0) as usize;
    o.dynamic_bone_sets.lay_counted(&mut l, at + 0x2F0, &cx, n);
    // The action's figatree (`x590`) is in the main animation buffer, the second one
    // (`x598`) in the main or the second: their places are the buffers', whichever other
    // copies of their archives there are (two fighters' buffers can hold one).
    let in_buf = |b: Option<super::handles::AnimBufId>, t: Option<melee_data::archive::DataRef>| {
        let (b, t) = (b?, t?);
        let base = game.fighter_anim_bufs.addr(b.0);
        (base != 0).then(|| base + super::ft::anim_buf::ARCHIVE_HEADER + t.offset)
    };
    if o.x590.is_some() {
        l.put_word(at + 0x590, in_buf(o.x59C, *o.x590));
    }
    if o.x598.addr.is_some() {
        l.put_word(at + 0x598, o.x598.addr);
    } else if o.x598.tree.is_some() {
        let b = if o.x598.in_main { o.x59C } else { o.x5A0 };
        l.put_word(at + 0x598, in_buf(b, o.x598.tree));
    }
    l.put(img);
}

/// `Item` (0xFCC), as its block goes back (`HSD_ObjFree`, or with its scene's heap): the
/// fields the comparer checks, its dynamic bones and hurtboxes up to their counts, over what
/// the block held where the item never writes (a stale variable, the variables past its
/// member, the unused words; a field the original wrote but melee-sim doesn't know stays
/// unknown).
pub fn item(game: &Game, data: &Archives, img: &mut Image, i: ItemId) {
    let at = game.items.addr(i.0);
    if at == 0 {
        return;
    }
    let o = &game.items[i.0];
    let mut l = Lay::leftover(img, data, at, super::structs::Item::SIZE);
    let cx = Addrs { game, img };
    o.lay(&mut l, at, &cx);
    let n = o.x374_dynamicBonesNum.max(0) as usize;
    o.xD4_dynamicBones.lay_counted(&mut l, at + 0xD4, &cx, n);
    let n = usize::from(o.xAC8_hurtboxNum);
    o.xACC_itemHurtbox.lay_counted(&mut l, at + 0xACC, &cx, n);
    l.put(img);
}

/// A deferred spawn (`EF_QueuedEffect`, 0x24), as its block goes back (`HSD_ObjFree`, or with
/// its scene's heap): its link, kind, id, joint and the words its kind takes; the kind byte's
/// padding and the words it doesn't take keep what the block held (`efAsync_Spawn` writes only
/// those).
pub fn queued(game: &Game, img: &mut Image, q: super::ef::QueuedId) {
    use super::ef::SpawnKind::*;
    let at = game.ef.queued.addr(q.0);
    if at == 0 {
        return;
    }
    let o = &game.ef.queued[q.0];
    let words: &[f32] = match super::ef::SpawnKind::from_u8(o.kind) {
        Some(PosOffset | AttachOffset | CameraShake) => &[o.params.x, o.params.y, o.params.z],
        Some(AttachParam | PosParam) => &[o.params.x],
        Some(PosOffsetParam) => &[o.params.x, o.params.y, o.params.z, o.extra1],
        Some(PosOffsetParam2) => &[o.params.x, o.params.y, o.params.z, o.extra1, o.extra2],
        _ => &[],
    };
    let mut l = Lay::new(at, 0x24);
    {
        let cx = Addrs { game, img };
        l.put_word(at, cx.handle("QueuedId", o.next.map(|n| n.0)));
        l.put_word(at + 0xC, cx.handle("JObjId", o.jobj.map(|j| j.0)));
    }
    l.u8(4, o.kind);
    l.u32(8, o.gfx_id as u32);
    l.f32s(0x10, words);
    img.unclaim(at + 5, at + 8);
    img.unclaim(at + 0x10 + 4 * words.len() as u32, at + 0x24);
    l.put(img);
}

/// A fighter's animation buffer (`fighter_x59C_alloc_data`, 0x8000): the archives its loads
/// copied there, each relocated to the buffer, the newest over the older ones' longer runs
/// (`ft::anim_buf`); what the buffer held before them stays as the image had it.
pub fn anim_buf(game: &Game, img: &mut Image, slot: u32) {
    let at = game.fighter_anim_bufs.addr(slot);
    if at == 0 {
        return;
    }
    for l in game.fighter_anim_bufs[slot].layers().iter().rev() {
        img.hold(
            at,
            at + l.length(),
            crate::hsd::image::Src::File {
                archive: l.archive,
                offset: 0,
                base: at + super::ft::anim_buf::ARCHIVE_HEADER,
            },
        );
    }
}

/// A fighter's parts table (`fighter_parts_alloc_data`, `FighterBone`s) and DObj list
/// (`fighter_dobj_list_alloc_data`, the DObjs `ftParts_SetupParts` listed), over what their
/// blocks held where the fighter never writes (the table's entries past its bones, the list's
/// past its count).
pub fn fighter_parts(game: &Game, data: &Archives, img: &mut Image, f: FighterId) {
    let fp = &game.fighters[f.0];
    let cx = Addrs { game, img };
    let mut lays = Vec::new();
    if let Some(p) = fp.parts {
        let at = game.fighter_parts.addr(p.0);
        if at != 0 {
            let bones = &game.fighter_parts[p.0];
            // The whole block: past the bones it holds what it held.
            let len = crate::hsd::heap::Kind::FighterParts
                .init()
                .0
                .max(0x10 * bones.len() as u32);
            let mut l = Lay::leftover(img, data, at, len);
            for (i, bone) in bones.iter().enumerate() {
                bone.lay(&mut l, at + 0x10 * i as u32, &cx);
            }
            lays.push(l);
        }
    }
    if let Some(d) = fp.dobj_list.data {
        let at = game.fighter_dobj_lists.addr(d.0);
        if at != 0 {
            let list = &game.fighter_dobj_lists[d.0];
            let n = (fp.dobj_list.count as usize).min(list.len());
            let len = crate::hsd::heap::Kind::FighterDObjList.init().0;
            let mut l = Lay::leftover(img, data, at, len);
            for (i, o) in list.iter().take(n).enumerate() {
                l.put_word(at + 4 * i as u32, cx.handle("DObjId", o.map(|o| o.0)));
            }
            lays.push(l);
        }
    }
    for l in lays {
        l.put(img);
    }
}

/// `mpLib_803BF760`'s joints (`MapJoint`s), where a stage without collision data points.
const DEFAULT_MAP_JOINTS: u32 = 0x803B_F738;

/// The stage's collision joints (`CollJoint`, 0x34 each, `groundCollJoint`), as the scene's
/// heap goes: the list's links, each joint's descriptor in the data, flags, stamp, dirty bit,
/// bounds, model joint and callbacks; the rest of the dirty bit's byte and the byte after it
/// keep what the block held.
pub fn coll_joints(game: &Game, data: &Archives, img: &mut Image) {
    let Some(g) = &game.geometry else { return };
    let at = g.joints_addr;
    if at == 0 || g.joints.is_empty() {
        return;
    }
    let descs = match game.stage_info.coll_data {
        None => Some(DEFAULT_MAP_JOINTS),
        Some(c) => data.ptr(c.at(0x24)).and_then(|d| img.data_addr(d)),
    };
    let len = 0x34 * g.joints.len() as u32;
    let mut l = Lay::leftover(img, data, at, len);
    let cx = Addrs { game, img };
    let user = |u: Option<super::gr::CollUser>| match u {
        None => Some(0),
        Some(super::gr::CollUser::Ground(gr)) => cx.handle("GroundId", Some(gr.0)),
        Some(super::gr::CollUser::GObj(o)) => cx.handle("GObjId", Some(o.0)),
    };
    for (j, o) in g.joints.iter().enumerate() {
        let a = at + 0x34 * j as u32;
        l.put_u32(a, o.next.map_or(0, |n| at + 0x34 * u32::from(n)));
        l.put_word(a + 4, descs.map(|d| d + 0x28 * j as u32));
        l.put_u32(a + 8, o.flags);
        l.put_u16(a + 0xC, o.stamp as u16);
        l.put_bits(a + 0xE, 0, 1, u32::from(o.dirty));
        for (i, v) in [
            o.bounding_min.x,
            o.bounding_min.y,
            o.bounding_max.x,
            o.bounding_max.y,
        ]
        .iter()
        .enumerate()
        {
            l.put_u32(a + 0x10 + 4 * i as u32, v.to_bits());
        }
        l.put_word(a + 0x20, cx.handle("JObjId", o.jobj.map(|j| j.0)));
        l.put_u32(a + 0x24, o.ledge_callback.map_or(0, |c| c.0));
        l.put_word(a + 0x28, user(o.ledge_user_data));
        l.put_u32(a + 0x2C, o.callback.map_or(0, |c| c.0));
        l.put_word(a + 0x30, user(o.user_data));
    }
    l.put(img);
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_laid_ground_vars_are_typed_members() {
        for t in super::LAID_GROUND_VARS {
            assert!(super::super::gr::GroundVars::TYPED.contains(t), "{t}");
        }
        let list = super::unlaid_types();
        assert!(list.iter().any(|(t, _)| t == "MapVars"));
        assert!(
            !list
                .iter()
                .any(|(t, _)| t == "ZebesStageVars" || t == "Item")
        );
    }
}
