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
        // The magnifier's player entries (`ifMagnify_804A1DE0.player`, 0x10 each from
        // `0x804A1DF4`), in the executable's data.
        UserData::MagnifyPlayer(slot) => Some(0x804A_1DF4 + 0x10 * u32::from(slot)),
        UserData::Sound(s) => known(game.sound_objs.addr(s.0)),
        UserData::Effect(e) => known(game.ef.effects.addr(e.0)),
        UserData::Text(t) => {
            (game.sis.arena_block != 0).then(|| game.sis.arena_block.wrapping_add(t.0))
        }
        // A canvas's own camera names itself.
        UserData::SisCamera(g) => known(game.world.gobjs.addr(g.0)),
        // Tournament mode's, set without a kind: a number, or a bracket box's address.
        UserData::TouIndex(i) => Some(i as u32),
        UserData::TouBox(i) => Some(super::gm::tournament::BRACKET_ADDR + 0xDC * u32::from(i)),
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
        | UserData::DataDel(a)
        | UserData::Info(a)
        | UserData::Count(a)
        | UserData::Diagram(a)
        | UserData::DiagramPopup(a)
        | UserData::Diagram2(a)
        | UserData::Diagram3(a)
        | UserData::SoundTest(a) => known(a),
        // `&ifStock_804A157C[player]` (0x54 bytes each), a static.
        UserData::StockIcons(p) => Some(0x804A_157C + 0x54 * u32::from(p)),
        // `&un_804DDBA8[slot]`, a byte each (`un_802FE260`).
        UserData::ArrowSlot(slot) => Some(0x804D_DBA8 + u32::from(slot)),
        // `NameTag_Create`'s heap byte.
        UserData::NameTagSlot(slot) => known(game.hud.nametags.user_data_addrs[usize::from(slot)]),
        UserData::Raw(p) => Some(p.0),
        UserData::Toy(a) => known(a),
        // The select screen's blocks, while their GObjs have them.
        UserData::CssCursor(i) => game.charsel.cursors[usize::from(i)]
            .as_ref()
            .and_then(|c| known(c.addr)),
        UserData::CssToken(i) => game.charsel.tokens[usize::from(i)]
            .as_ref()
            .and_then(|c| known(c.addr)),
        UserData::CssTag(i) => game.charsel.tags[usize::from(i)]
            .as_ref()
            .and_then(|c| known(c.addr)),
        UserData::StadiumScreen(which) => game.gr.stadium.screens[usize::from(which)]
            .as_ref()
            .and_then(|s| known(s.addr)),
        UserData::IzumiReflection => game
            .gr
            .izumi
            .reflection
            .as_ref()
            .and_then(|r| known(r.addr)),
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

/// Which of the scene's blocks that outlive their owners' state are this scene's, so
/// [`scene_game_objects`] lays them out: the stage's collision joints and arrays, the
/// dynamics' links and the enemy spawner's slots (a past stage's stay in the globals).
pub struct Live {
    pub geometry: bool,
    pub dynamics: bool,
    pub zako: bool,
}

/// The game's objects the scene's heap holds as it ends, laid out before the heap's claims
/// go (`Heap::end_scene_image`), so a layout over what a block held reads under its owner's
/// claim: the stage's collision joints (when this scene loaded them, [`Live`]), the
/// scene's fighters with their parts tables and DObj lists, its items, its stage parts
/// (`Ground`), its deferred spawns and its animation buffers.
pub fn scene_game_objects(game: &Game, data: &Archives, img: &mut Image, live: Live) {
    if live.geometry {
        coll_joints(game, data, img);
        coll_arrays(game, data, img);
    }
    if live.zako {
        zako_slots(game, data, img);
    }
    if live.dynamics {
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
    let kind = std::any::type_name::<super::structs::ItemLink>();
    for (slot, _) in game.it.links.iter() {
        if img.in_scene(kind, slot) {
            item_link(game, data, img, super::handles::ItemLinkId(slot));
        }
    }
    let kind = std::any::type_name::<super::it::BoneTable>();
    for (slot, _) in game.it.bone_tables.iter() {
        if img.in_scene(kind, slot) {
            bone_table(game, img, super::handles::BoneTableId(slot));
        }
    }
    let kind = std::any::type_name::<crate::lb::sound_obj::SoundObj>();
    for (slot, _) in game.sound_objs.iter() {
        if img.in_scene(kind, slot) {
            sound_obj(game, Some(data), img, super::handles::SoundObjId(slot));
        }
    }
    let kind = std::any::type_name::<crate::hsd::sobj::SObj>();
    for (slot, _) in game.objs.sobjs.iter() {
        if img.in_scene(kind, slot) {
            sobj(game, data, img, crate::hsd::sobj::SObjId(slot), false);
        }
    }
    let kind = std::any::type_name::<super::ef::Effect>();
    for (slot, _) in game.ef.effects.iter() {
        if img.in_scene(kind, slot) {
            effect(game, img, super::ef::EffectId(slot));
        }
    }
    let kind = std::any::type_name::<super::ef::Queued>();
    for (slot, _) in game.ef.queued.iter() {
        if img.in_scene(kind, slot) {
            queued(game, img, super::ef::QueuedId(slot));
        }
    }
    // The DObj lists from `fighter_x2040_alloc_data`: every live list in the scene but the
    // fighters' own (`fighter_dobj_list_alloc_data`, laid out with their parts).
    let lists = std::any::type_name::<Vec<Option<crate::hsd::dobj::DObjId>>>();
    for (slot, _) in game.fighter_dobj_lists.iter() {
        let own = game
            .fighters
            .iter()
            .any(|(_, f)| f.dobj_list.data.is_some_and(|d| d.0 == slot));
        if !own && img.in_scene(lists, slot) {
            x2040_list(game, data, img, super::handles::DObjListId(slot));
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
    bg_flash_fields(img, &mut l, u);
    l.keep_render_target(&game.bgflash.render_leftovers, img.render_fill);
    l.put(img);
}

/// The background flash's user data's fields laid out into `l` (its block at `l`'s start):
/// its alpha and color overlay.
pub fn bg_flash_fields(img: &Image, l: &mut Lay, u: &super::structs::BgFlashUserData) {
    l.u8(0, u.x0);
    color_overlay(img, l, 4, &u.x4);
}

/// `Ground` (0x204), as its block goes back to the heap, over what the block held where the
/// stage's code never writes (the padding, a typed member's bytes past its fields): a stage
/// part's union as melee-sim keeps it as bytes or lays its member out (`GroundVars::lay`).
pub fn ground(game: &Game, data: &Archives, img: &mut Image, g: super::handles::GroundId) {
    let at = game.grounds.addr(g.0);
    if at == 0 {
        return;
    }
    let mut l = Lay::leftover(img, data, at, 0x204);
    lay_ground(game, img, &mut l, g);
    // The member's tables in blocks of their own.
    let blocks = {
        let cx = Addrs { game, img };
        game.grounds[g.0].u.lay_blocks(&cx)
    };
    for b in blocks {
        b.put(img);
    }
    if let Some(runs) = game.gr.render_leftovers.get(&g.0) {
        l.keep_render_target(runs, img.render_fill);
    }
    l.put(img);
}

/// [`ground`]'s fields, laid into `l` (a block at the stage part's address): what the comparer
/// takes ours to be where the block held the GPU's pixels (`melee_capture`'s ground view).
pub fn lay_ground(game: &Game, img: &Image, l: &mut Lay, g: super::handles::GroundId) {
    let at = game.grounds.addr(g.0);
    let o = &game.grounds[g.0];
    let ga = |x: Option<GObjId>| x.map_or(0, |x| game.world.gobjs.addr(x.0));
    let code = |c: Option<super::handles::CodeRef>| c.map_or(0, |c| c.0);
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
    color_overlay(img, l, 0x40, &o.color_overlay);
    l.f32(0xC0, o.xC0);
    match &o.u {
        super::gr::GroundVars::Bytes(b) => l.bytes(0xC4, b),
        v => {
            let cx = Addrs { game, img };
            if !v.lay(l, at + 0xC4, &cx)
                && let Some(ty) = v.type_name()
            {
                // A typed member melee-sim doesn't lay out: its bytes are its type's.
                l.forget_as(at + 0xC4, super::gr::vars::SIZE as u32, ty);
            }
        }
    }
}

/// The types of what the heap's image doesn't lay out, each with a note: an object of one
/// leaves unknown bytes in its block as it goes, and a new object made over those stops a
/// session where the original's bytes differ (`Comparer::unlaid_view`, "unknown leftover
/// bytes of a freed TYPE under NEW: its layout isn't ported"). `melee-oracle list-unlaid`
/// prints them; laying them out is a campaign of its own (docs/engine.md, a session's heap
/// leftovers).
pub fn unlaid_types() -> Vec<(String, &'static str)> {
    use crate::hsd::heap::Kind;
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
            | Kind::FighterAttrs
            | Kind::FighterParts
            | Kind::FighterDObjList
            | Kind::FighterX2040
            | Kind::FighterX59C
            | Kind::Item
            | Kind::Effect
            | Kind::EffectQueued
            | Kind::BgFlash
            | Kind::Particle
            | Kind::Generator
            | Kind::AppSrt
            | Kind::Shadow
            | Kind::Vec
            | Kind::Mtx
            | Kind::ItemDynamicBones
            | Kind::ItemLink
            | Kind::SList
            | Kind::Rvalue
            | Kind::AudioUserData => continue,
            Kind::Player => "a pool the game allocates nothing from (`Player_AllocData`)",
            Kind::DList
            | Kind::TevReg
            | Kind::Chan
            | Kind::Render
            | Kind::ZList
            | Kind::DebugMenu => "a pool melee-sim's model allocates nothing from",
            Kind::SObj => continue,
        };
        v.push((k.name().to_owned(), why));
    }
    // Every HSD class's objects are laid out (`hsd::export`).
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
        (
            "a disc read's bytes past a file",
            "the bytes a read brings past a file's end",
        ),
    ] {
        v.push((t.to_owned(), why));
    }
    v
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
            "GeneratorId" => g.objs.generators.pool.addr(h),
            // A text is its offset in the text arena's block.
            "TextId" if g.sis.arena_block != 0 => g.sis.arena_block.wrapping_add(h),
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
    // The common attributes are a struct copy of the file's (`fp->co_attrs =
    // *fp->ft_data->x0`), whose padding after their last byte comes along.
    if let Some(src) = o.ft_data.and_then(|d| data.ptr(d)) {
        let pad: Vec<u8> = (0x181..0x184).map(|k| data.u8(src.at(k))).collect();
        l.put_bytes(at + 0x110 + 0x181, &pad);
    }
    // A hit's struct copy of its attacker's hit data (`fn_80037F00`'s `fp->dmg.x18d8 = *ev`,
    // the attacker's `x2074`) brings the attacker's bits past the named ones and its padding,
    // which melee-sim doesn't keep: unknown.
    l.forget(at + 0x18E9, 3);
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
    if let Some(runs) = game.ft.render_leftovers.get(&f.0) {
        l.keep_render_target(runs, img.render_fill);
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
    // The state's animation description (`xD0`) is written only by a state with an
    // animation (`Item_80268E5C`, always a place in the item's file): an item whose states
    // have none keeps the word its block held, which a place in no file reads as none.
    let state_desc = l.kept(at + 0xD0, 4);
    o.lay(&mut l, at, &cx);
    if o.xD0_itemStateDesc.is_none() {
        l.restore(state_desc);
    }
    let n = o.x374_dynamicBonesNum.max(0) as usize;
    o.xD4_dynamicBones.lay_counted(&mut l, at + 0xD4, &cx, n);
    let n = usize::from(o.xAC8_hurtboxNum);
    o.xACC_itemHurtbox.lay_counted(&mut l, at + 0xACC, &cx, n);
    if let Some(runs) = game.it.render_leftovers.get(&i.0) {
        l.keep_render_target(runs, img.render_fill);
    }
    l.put(img);
}

/// `ItemLink` (0x1D8), as its block goes back (`HSD_ObjFree`, or with its scene's heap): the
/// fields the comparer checks, over what the block held where the link never writes.
pub fn item_link(game: &Game, data: &Archives, img: &mut Image, k: super::handles::ItemLinkId) {
    let at = game.it.links.addr(k.0);
    if at == 0 {
        return;
    }
    let mut l = Lay::leftover(img, data, at, super::structs::ItemLink::SIZE);
    let cx = Addrs { game, img };
    game.it.links[k.0].lay(&mut l, at, &cx);
    l.put(img);
}

/// An item's table of its model's joints (`DynamicBoneTable`, 0x190: 100 `HSD_JObj*`), as its
/// block goes back: the joints `Item_802682F0`'s walk wrote, the rest what the block held.
pub fn bone_table(game: &Game, img: &mut Image, t: super::handles::BoneTableId) {
    let at = game.it.bone_tables.addr(t.0);
    if at == 0 {
        return;
    }
    let o = &game.it.bone_tables[t.0];
    let mut l = Lay::new(at, 0x190);
    {
        let cx = Addrs { game, img };
        for (i, j) in o.bones.iter().enumerate().take(usize::from(o.len)) {
            l.put_word(at + 4 * i as u32, cx.handle("JObjId", j.map(|j| j.0)));
        }
    }
    // The entries past the joints keep what the block held: an image layout over its claim
    // would hide them, so only the written ones go in.
    img.unclaim(at + 4 * u32::from(o.len), at + 0x190);
    l.put(img);
}

/// A sound object's user data (`lbAudioAx_UserData`, 0x48), as its block goes back
/// (`HSD_ObjFree`, whose link then takes its first word, or with its scene's heap): every word
/// `fn_80025FAC` writes; its first word, which nothing writes, what the block held (with `data`,
/// from the image).
pub fn sound_obj(
    game: &Game,
    data: Option<&Archives>,
    img: &mut Image,
    s: super::handles::SoundObjId,
) {
    let at = game.sound_objs.addr(s.0);
    if at == 0 {
        return;
    }
    let o = &game.sound_objs[s.0];
    let mut l = match data {
        Some(d) => Lay::leftover(img, d, at, 0x48),
        None => Lay::new(at, 0x48),
    };
    {
        let cx = Addrs { game, img };
        l.put_word(at + 4, cx.handle("GObjId", o.gobj.map(|g| g.0)));
        l.put_word(at + 8, cx.handle("GObjId", o.owner.map(|g| g.0)));
    }
    l.u32(0xC, o.behavior as u32);
    l.u32(0x10, o.update.map_or(0, |c| c.0));
    for (i, w) in [
        o.id,
        o.start_vol,
        o.end_vol,
        o.vol,
        o.pan_left,
        o.pan_right,
        o.pan,
        o.voice_id,
        o.current_frame,
        o.end_frame,
    ]
    .into_iter()
    .enumerate()
    {
        l.u32(0x14 + 4 * i as u32, w as u32);
    }
    l.f32(0x3C, o.direction);
    l.u32(0x40, o.track as u32);
    l.u32(0x44, u32::from(o.x44));
    l.put(img);
}

/// A sprite (`HSD_SObj`, 0x9C), as its block goes back (`HSD_ObjFree`, unlinked first:
/// `freed`) or with its scene's heap: its fields, its texture object as `GXInitTexObj` (or
/// `GXInitTexObjCI`) made it and its first draw's `GXLoadTexObj` left it, its palette's object
/// (`GXInitTlutObj`'s bits; those the stack held, bits 21 to 23 of `loadTlut0`, and its padding,
/// keep what the block held) and its second image's texture object. A part the sprite doesn't
/// make (no palette, no second image) and the priority's padding keep what the block held; an
/// image's address the image doesn't place is unknown, and so is a palette `GXLoadTlut` loaded
/// (its region's offset).
pub fn sobj(
    game: &Game,
    data: &Archives,
    img: &mut Image,
    s: crate::hsd::sobj::SObjId,
    freed: bool,
) {
    use crate::hsd::sobj::{GxTexObj, Image as SImage, gx_init_tex_obj};
    let at = game.objs.sobjs.addr(s.0);
    if at == 0 {
        return;
    }
    let o = &game.objs.sobjs[s.0];
    let mut l = Lay::new(at, 0x9C);
    let cx = Addrs { game, img };
    let sa = |x: Option<crate::hsd::sobj::SObjId>| x.map_or(0, |x| game.objs.sobjs.addr(x.0));
    l.u32(0, o.x0);
    if freed {
        l.u32(4, 0);
        l.u32(8, 0);
    } else {
        l.u32(4, sa(o.next));
        l.u32(8, sa(o.prev));
    }
    l.put_word(at + 0xC, cx.handle("GObjId", o.gobj.map(|g| g.0)));
    l.f32s(
        0x10,
        &[
            o.x, o.y, o.rotation, o.scale_x, o.scale_y, o.s0, o.t0, o.s1, o.t1,
        ],
    );
    l.u16(0x34, o.width);
    l.u16(0x36, o.height);
    l.bytes(0x38, &o.color0);
    l.bytes(0x3C, &o.color1);
    l.u32(0x40, o.flags);
    l.u8(0x44, o.priority);
    l.u32(0x48, o.x48);
    l.u32(0x4C, o.callback.map_or(0, |c| c.0));
    // A texture object at `off`, unknown where its image's address is.
    let tex = |l: &mut Lay, off: u32, t: GxTexObj, image: Option<u32>| {
        l.u32(off, t.mode0);
        l.u32(off + 4, t.mode1);
        l.u32(off + 8, t.image0);
        match image {
            Some(_) => l.u32(off + 0xC, t.image3),
            None => l.forget(at + off + 0xC, 4),
        }
        l.u32(off + 0x10, t.user_data);
        l.u32(off + 0x14, t.fmt);
        l.u32(off + 0x18, t.tlut_name);
        l.u16(off + 0x1C, t.load_cnt);
        l.u8(off + 0x1E, t.load_fmt);
        l.u8(off + 0x1F, t.flags);
    };
    let tx = o.tex;
    let (image_ptr, tlut) = match o.image {
        SImage::Data(d) => (cx.data(data.ptr(d)), tx.tlut),
        SImage::Movie { .. } => (Some(0), None),
        SImage::None => (None, None),
    };
    let mut t = gx_init_tex_obj(
        image_ptr.unwrap_or(0),
        tx.width,
        tx.height,
        tx.format,
        tx.wrap_s,
        tx.wrap_t,
        tx.mipmap,
    );
    if let Some(tl) = tlut {
        // `GXInitTexObjCI`, and the palette's object.
        t.flags &= !2;
        t.tlut_name = data.u32(tl.at(8));
        let lut = cx.data(data.ptr(tl));
        if let Some(lut) = lut {
            l.put_bits(at + 0x74, 11, 21, (lut & 0x3FFF_FFFF) >> 5);
        }
        l.u8(0x74, 0x64);
        l.u16(0x78, data.u16(tl.at(0xC)));
        let ci = matches!(tx.format, 8 | 9);
        if o.loaded && ci {
            // `GXLoadTlut` sets the region's place in the low bits.
            l.forget(at + 0x70, 4);
        } else {
            l.u32(0x70, (data.u32(tl.at(4)) & 3) << 10);
        }
    }
    if o.loaded {
        t = t.loaded(0);
    }
    tex(&mut l, 0x50, t, image_ptr);
    if let Some(d) = o.image2 {
        let image2 = cx.data(data.ptr(d));
        let mut t2 = gx_init_tex_obj(
            image2.unwrap_or(0),
            data.u16(d.at(4)),
            data.u16(d.at(6)),
            data.u32(d.at(8)),
            tx.wrap_s,
            tx.wrap_t,
            data.u32(d.at(0xC)) as u8,
        );
        if o.loaded && o.flags & 0x14 == 4 {
            t2 = t2.loaded(1);
        }
        tex(&mut l, 0x7C, t2, image2);
    }
    l.put(img);
}

/// `EF_Effect` (0x2C), as its block goes back (`HSD_ObjFree`, or with its scene's heap): its
/// links, GObjs, joint, update and lifetime and flags; `user_data`, `params`, `x2A` and `x2B`
/// as the effect holds them where it wrote them or knows the block's ([`super::ef::EffectLeft`]),
/// else unknown.
pub fn effect(game: &Game, img: &mut Image, e: super::ef::EffectId) {
    use super::ef::Held;
    let at = game.ef.effects.addr(e.0);
    if at == 0 {
        return;
    }
    let o = &game.ef.effects[e.0];
    let mut l = Lay::new(at, 0x2C);
    {
        let cx = Addrs { game, img };
        l.put_word(at, cx.handle("EffectId", o.next.map(|n| n.0)));
        l.put_word(at + 4, cx.handle("GObjId", o.gobj.map(|g| g.0)));
        l.put_word(at + 8, cx.handle("GObjId", o.parent.map(|g| g.0)));
        l.put_word(at + 0xC, cx.handle("JObjId", o.attach_jobj.map(|j| j.0)));
        let ud = match o.left.user_data {
            Held::Written => cx.handle("FighterId", o.user_data.map(|f| f.0)),
            Held::Word(w) => Some(w),
            Held::Unknown => None,
        };
        l.put_word(at + 0x14, ud);
    }
    l.u32(0x10, o.update.map_or(0, |u| u.addr()));
    let p = [o.params.x, o.params.y, o.params.z];
    for (i, f) in p.iter().enumerate() {
        if o.param_known(i) {
            l.f32(0x18 + 4 * i as u32, *f);
        }
    }
    l.u16(0x24, o.lifetime);
    l.bytes(
        0x26,
        &[
            o.expire_flags as u8,
            o.scale_flags,
            o.state_flags,
            o.is_async,
        ],
    );
    for (i, b) in o.left.x2a.iter().enumerate() {
        if let Some(b) = *b {
            l.u8(0x2A + i as u32, b);
        }
    }
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
    // The attributes' block (`fighter_dat_attrs_alloc_data`, 0x424): the character's
    // attributes copied in (`PUSH_ATTRS`), past them what the block held.
    if let Some(a) = fp.dat_attrs_backup {
        let at = game.fighter_attrs.addr(a.0);
        if at != 0 {
            let len = crate::hsd::heap::Kind::FighterAttrs.init().0;
            let mut l = Lay::leftover(img, data, at, len);
            if fp.dat_attrs == Some(a) {
                let b = &game.fighter_attrs[a.0];
                l.bytes(0, &b[..b.len().min(len as usize)]);
            }
            lays.push(l);
        }
    }
    for l in lays {
        l.put(img);
    }
}

/// A DObj list from `fighter_x2040_alloc_data` (0x80 bytes, 32 `HSD_DObj*`): a fighter's
/// second model's (`ftCo_800C884C`, `x203C`) or a Kirby or Jigglypuff hat's, as its block goes
/// back (`HSD_ObjFree`, or with its scene's heap). Its owner writes only the entries it lists
/// (`ft_800C85B8`, the hats' `ftParts_80075650`), each a DObj, never null, so an entry melee-sim
/// holds as none is one the owner never wrote: what the block held.
pub fn x2040_list(game: &Game, data: &Archives, img: &mut Image, list: super::handles::DObjListId) {
    let at = game.fighter_dobj_lists.addr(list.0);
    if at == 0 {
        return;
    }
    let mut l = Lay::leftover(img, data, at, crate::hsd::heap::Kind::FighterX2040.init().0);
    let cx = Addrs { game, img };
    for (i, o) in game.fighter_dobj_lists[list.0]
        .iter()
        .enumerate()
        .take(0x20)
    {
        if let Some(o) = o {
            l.put_word(at + 4 * i as u32, cx.handle("DObjId", Some(o.0)));
        }
    }
    l.put(img);
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
    fn the_laid_types_are_off_the_list() {
        let list = super::unlaid_types();
        for t in [
            "MapVars",
            "ZebesStageVars",
            "Item",
            "Effect",
            "LObj",
            "HSD_Archive",
        ] {
            assert!(!list.iter().any(|(n, _)| n == t), "{t}");
        }
        assert!(list.iter().any(|(t, _)| t == "Player"));
    }
}
