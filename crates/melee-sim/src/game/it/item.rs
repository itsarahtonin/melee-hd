//! The item core, `melee/it/item.c`: making items and their GObjs, the per-frame processes,
//! state changes, and destroying items.

use gekko_fp as fp;
use melee_data::archive::{Archives, DataRef};

use super::super::code;
use super::super::handles::{BoneTableId, CodeRef, ItemId, ItemStatesRef};
use super::super::state::{Game, Obj, Running, UserData};
use super::super::structs::{Item, SpawnItem};
use super::data::{Article, Attr, Common, attr, common};
use super::kinds::*;
use super::{
    BoneTable, CLASS_ITEM, P_LINK_ITEM, USER_DATA_ITEM, coll, flag, is_item, item_mut, item_of,
    jobj_of, lib26b1, lib2725, set_flag, tables,
};
use crate::fpu::{mem, reg};
use crate::game::camera;
use crate::game::ft::dynamics::BoneSlot;
use crate::hsd::gobj::{self, GObjId};
use crate::hsd::jobj::{self, JObjId};
use crate::lb::dynamics::Collider;
use crate::vec::Vec3;

/// The item processes, by priority (`Item_8026862C`).
pub const PROC_HITLAG: u32 = 0x8026_93E4;
pub const PROC_ANIM: u32 = 0x8026_9528;
pub const PROC_PHYS: u32 = 0x8026_97D4;
pub const PROC_COLL: u32 = 0x8026_9978;
pub const PROC_ACCESSORY: u32 = 0x8026_9A9C;
pub const PROC_JUMPED_ON: u32 = 0x8026_9B60;
pub const PROC_GRAB: u32 = 0x8026_9BE4;
pub const PROC_HIT: u32 = 0x8026_9C5C;
pub const PROC_REACT: u32 = 0x8026_A294;
pub const PROC_DYNAMICS: u32 = 0x8026_A788;

/// `Item_OnUserDataRemove`, the items' user data release function.
const ON_USER_DATA_REMOVE: u32 = 0x8026_AE10;

/// Runs an item process, if `addr` is one.
pub fn run_proc(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        PROC_HITLAG => proc_hitlag(r, gobj),
        PROC_ANIM => proc_anim(r, gobj),
        PROC_PHYS => proc_phys(r, gobj),
        PROC_COLL => proc_coll(r, gobj),
        PROC_ACCESSORY => proc_accessory(r, gobj),
        PROC_JUMPED_ON => proc_jumped_on(r, gobj),
        PROC_GRAB => proc_grab(r, gobj),
        PROC_HIT => proc_hit(r, gobj),
        PROC_REACT => proc_react(r, gobj),
        PROC_DYNAMICS => proc_dynamics(r, gobj),
        super::spawn::PROC_SPAWN => super::spawn::proc_spawn(r, gobj),
        _ => return false,
    }
    true
}

/// Frees an item's user data, `Item_OnUserDataRemove`: its joint table, then the item. A
/// freed item's first word links it to the next free item (`HSD_ObjFree`), which it keeps
/// when reused.
pub fn free_user_data(game: &mut Game, item: ItemId) {
    if let Some(t) = game.items[item.0].xBBC_dynamicBoneTable {
        let g = &*game;
        crate::hsd::export::with_image(|img| crate::game::export::bone_table(g, img, t));
        game.it.bone_tables.free(t.0);
    }
    game.items[item.0].x0 = game.items.free_head().map(ItemId);
    game.items.free(item.0);
}

/// Calls an item callback of the event kind (`HSD_GObjEvent`).
pub fn call(r: &mut Running, gobj: GObjId, f: CodeRef) {
    code::run_proc(r, gobj, f);
}

/// Calls an item callback that answers whether to destroy the item (`HSD_GObjPredicate`).
pub fn call_pred(r: &mut Running, gobj: GObjId, f: CodeRef) -> bool {
    code::run_pred(r, gobj, f)
}

/// Whether a GObj is a fighter, `ftLib_IsFighter` (its classifier is `HSD_GOBJ_CLASS_FIGHTER`).
pub fn is_fighter(game: &Game, gobj: Option<GObjId>) -> bool {
    gobj.is_some_and(|g| game.world.gobjs[g.0].classifier == 4)
}

/// Sets a joint's translation (the inline `HSD_JObjSetTranslate`).
pub fn set_translate(game: &mut Game, j: JObjId, v: Vec3) {
    jobj::set_srt(&mut game.objs.jobjs, j, |o| o.translate = v);
}

/// Sets a joint's scale to the item's in all three axes (`HSD_JObjSetScaleItem`, and
/// `Item_8026849C`).
#[doc(alias = "Item_8026849C")]
pub fn apply_scale(game: &mut Game, gobj: GObjId) {
    let s = super::item(game, gobj).scl;
    let j = jobj_of(game, gobj).expect("an item without a model");
    jobj::set_srt(&mut game.objs.jobjs, j, |o| o.scale = Vec3::new(s, s, s));
}

/// Turns the model to face the item's way, if the item turns (`HSD_JObjSetFacingDirItem`):
/// a quarter turn about y per unit of facing, in double precision.
pub fn apply_facing(game: &mut Game, gobj: GObjId, j: Option<JObjId>) {
    let ip = super::item(game, gobj);
    if ip.xDC8_word.flags.x19 {
        let y = fp::frsp(fp::fmul(
            fp::fdiv(std::f64::consts::PI, 2.0),
            reg(ip.facing_dir),
        ));
        lib2725::set_rotation_y(game, j.expect("an item without a model"), mem(y));
    }
}

/// The hold kind `Item_802674AC` gives an item kind.
#[doc(alias = "Item_802674AC")]
pub fn hold_kind_of(kind: i32) -> i32 {
    if kind == FOODS {
        2
    } else if kind == PKIND_RANDOM {
        3
    } else if kind < L_GUN_RAY {
        0
    } else if kind < KURIBOH {
        1
    } else if kind < OCTAROCK_STONE {
        6
    } else if kind < MARIO_FIRE {
        7
    } else if kind < UNK4 {
        8
    } else if kind == UNK4 {
        12
    } else if kind < PKIND_RANDOM {
        11
    } else if kind < PKIND_TERMINATE {
        9
    } else if kind < OLD_KURI {
        10
    } else if kind < ARWING_LASER {
        4
    } else {
        5
    }
}

/// Adds `delta` to the count a hold kind's items keep (`Item_804A0C64`), as `Item_802675A8`
/// and `Item_802676F4` pick it.
pub fn bump_count(n: &mut super::super::structs::HSD_ObjAllocUnk, hold_kind: i32, delta: i32) {
    let add = |c: &mut i32| *c = c.wrapping_add(delta);
    match hold_kind {
        0 => add(&mut n.x0),
        1 => add(&mut n.x8),
        2 => add(&mut n.x10),
        3 => add(&mut n.x58),
        4 => add(&mut n.x3C),
        5 => add(&mut n.x40),
        6 => add(&mut n.x2C),
        7 => add(&mut n.x34),
        8 => n.x18 = n.x18.wrapping_add(delta as u32),
        9 => add(&mut n.x1C),
        10 => add(&mut n.x24),
        11 => add(&mut n.x48),
        12 => add(&mut n.x50),
        _ => {}
    }
}

/// Counts an item out, `Item_802675A8`.
#[doc(alias = "Item_802675A8")]
pub fn uncount(game: &mut Game, gobj: GObjId) {
    let (hold_kind, x18) = {
        let ip = super::item(game, gobj);
        (ip.hold_kind, ip.x18)
    };
    bump_count(&mut game.it.counts, hold_kind, -1);
    if super::super::gm::is_training(game) && x18 == 1 {
        game.it.counts.x60 = game.it.counts.x60.wrapping_sub(1);
    }
}

/// Counts a new item in, `Item_802676F4` (hold kind 9 isn't counted here).
#[doc(alias = "Item_802676F4")]
pub fn count(game: &mut Game, gobj: GObjId) {
    let (hold_kind, kind, x18) = {
        let ip = super::item(game, gobj);
        (ip.hold_kind, ip.kind, ip.x18)
    };
    if hold_kind != 9 {
        bump_count(&mut game.it.counts, hold_kind, 1);
    }
    match hold_kind {
        0 if kind == M_BALL => game.it.counts.x1C = game.it.counts.x1C.wrapping_add(1),
        2 => super::lib2725::count_food(game),
        _ => {}
    }
    if super::super::gm::is_training(game) && x18 == 1 {
        game.it.counts.x60 = game.it.counts.x60.wrapping_add(1);
    }
}

/// Whether a hold kind's items are at their limit, `Item_8026784C`. For Poké Balls (9) the
/// count drops by one when over.
#[doc(alias = "Item_8026784C")]
pub fn at_limit(game: &mut Game, hold_kind: i32) -> bool {
    let n = &mut game.it.counts;
    match hold_kind {
        0 => n.x0 >= n.x4,
        1 => n.x8 >= n.xC,
        2 => n.x10 >= n.x14,
        3 => n.x58 >= n.x5C,
        5 => n.x40 >= n.x44,
        7 => n.x34 >= n.x38,
        9 => {
            if n.x1C > n.x20 {
                n.x1C = n.x1C.wrapping_sub(1);
                true
            } else {
                false
            }
        }
        10 => n.x24 >= n.x28,
        11 => n.x48 >= n.x4C,
        12 => n.x50 >= n.x54,
        _ => false,
    }
}

/// Registers a character item's article as its character loads, `it_8026B3F8`.
#[doc(alias = "it_8026B3F8")]
pub fn set_char_article(game: &mut Game, article: Option<DataRef>, kind: i32) {
    game.it.char_articles[(kind - KURIBOH) as usize] = article;
}

/// An item kind's article (`it_804D6D24` and friends, by `Item_80267978`'s split).
pub fn article_of(game: &Game, data: &Archives, kind: i32) -> Option<DataRef> {
    let g = &game.it;
    if kind < KURIBOH {
        data.ptr(g.common_articles?.at(4 * kind as u32))
    } else if kind < PKIND_START {
        g.char_articles[(kind - KURIBOH) as usize]
    } else if kind < OLD_KURI {
        data.ptr(g.pokemon_articles?.at(4 * (kind - PKIND_START) as u32))
    } else {
        let a = game.gr.stage_articles[(kind - OLD_KURI) as usize];
        assert!(
            a.is_some(),
            "not found zako model data! check ground dat file!"
        );
        a
    }
}

/// Points an item at its kind's article and tables, `Item_80267978`.
#[doc(alias = "Item_80267978")]
pub fn set_kind_data(game: &mut Game, data: &Archives, gobj: GObjId) {
    let kind = super::item(game, gobj).kind;
    let article = article_of(game, data, kind);
    let logic = tables::logic_ref(kind);
    let ip = item_mut(game, gobj);
    ip.xC4_article_data = article;
    ip.xB8_itemLogicTable = Some(logic);
    ip.xBC_itemStateContainer = Some(ItemStatesRef(tables::logic(logic).states));
}

/// The item's `ItemAttr`.
pub fn attr_of(ip: &Item) -> Attr {
    // Only a stage's hurtbox item has attributes outside the archives (`it_803F8C08`).
    assert!(
        ip.xCC_item_attr.is_some() || ip.kind == super::kinds::PKIND_RANDOM,
        "an item without attributes"
    );
    Attr(ip.xCC_item_attr)
}

/// The item's article.
pub fn article(ip: &Item) -> Article {
    Article(ip.xC4_article_data.expect("an item without an article"))
}

/// `ItCo.dat`'s common item data.
pub fn common_data(game: &Game) -> Common {
    Common(game.it.common.expect("ItCo.dat isn't loaded"))
}

/// Sets a new item's variables, `Item_80267AA8`.
#[doc(alias = "Item_80267AA8")]
pub fn init_vars(r: &mut Running, gobj: GObjId, spawn: &SpawnItem) {
    let data = r.data;
    let yaku_attr = r.game.it.yaku_attr;
    let made = r.game.it.made;
    r.game.it.made = made.wrapping_add(1);
    {
        let ip = item_mut(r.game, gobj);
        ip.kind = spawn.kind;
        ip.hold_kind = spawn.hold_kind;
        ip.x18 = spawn.x10;
        ip.x1C = made;
        ip.entity = Some(gobj);
    }
    set_kind_data(r.game, data, gobj);
    {
        let ip = item_mut(r.game, gobj);
        ip.msid = -1;
        let a = article(ip);
        ip.xC8_joint = a.model(data).joint(data);
        // The stage items' spawn (`it_802E6AEC`) points the kind's article at the
        // executable's attributes, which no archive holds.
        ip.xCC_item_attr = if ip.kind == super::kinds::PKIND_RANDOM && yaku_attr {
            None
        } else {
            Some(a.attr(data))
        };
        ip.owner = None;
        ip.xDC8_word.flags.x0 = false;
        ip.ecb_lock = -1;
        ip.xDC4 = 0;
        ip.xDC8_word.flags.xF = 0;
        ip.xCC4 = 0;
        set_flag(&mut ip.xDCF_flag, 6, false);
        set_flag(&mut ip.xDCF_flag, 7, false);
        ip.xCB0_source_ply = 6;
        ip.xCB4 = -1;
        ip.xC38 = -1;
        ip.xCE0 = Vec3::new(0.0, 0.0, 0.0);
        ip.xCD4 = Vec3::new(0.0, 0.0, 0.0);
        set_flag(&mut ip.xDD0_flag, 1, false);
        ip.xDC8_word.flags.x14 = false;
        ip.xDC8_word.flags.xE = false;
        ip.xCEC_fighterGObj = None;
        ip.xCF0_itemGObj = None;
        ip.xCF4_fighterGObjUnk = None;
        ip.toucher = None;
        ip.xC64_reflectGObj = None;
        ip.xCFC = None;
        ip.atk_victim = None;
        ip.grab_victim = None;
        ip.xCCC_incDamageDirection = 0.0;
        ip.xCB8_outDamageDirection = 0.0;
        ip.xC68 = 0.0;
        ip.xCD0 = 0.0;
    }
    let lifetime = common_data(r.game).u32(data, common::LIFETIME);
    lib2725::set_lifetime(r.game, data, gobj, mem(fp::frsp(f64::from(lifetime))));
    let ip = item_mut(r.game, gobj);
    let a = attr_of(ip);
    set_flag(&mut ip.xDD0_flag, 3, false);
    ip.spin_spd = a.f32(data, attr::SPIN_SPEED);
    ip.xDC8_word.flags.x19 = a.x1_3(data);
    ip.xDC8_word.flags.x17 = a.x1_1(data);
    ip.xBCC_unk = a.vec2(data, attr::X30);
    ip.xBD4_grabRange = a.vec2(data, attr::GRAB_RANGE);
    ip.xDC8_word.flags.x1A = a.x1_4(data);
    ip.xBEC = a.ecb(data, attr::ECB);
    ip.xBDC = a.ecb(data, attr::ECB);
    ip.xDC8_word.flags.x1C = true;
    ip.xDC8_word.flags.x1D = true;
    ip.xDC8_word.flags.x1E = true;
    ip.xC1C = a.ecb(data, attr::X40);
    ip.xC0C = ip.xC1C.clone();
    ip.xBFC = ip.xC0C.clone();
    ip.xDC8_word.flags.xC = a.x1_5(data);
    ip.xDC8_word.flags.xD = true;
    ip.xDCD_flag.x0.b01 = a.cam_kind(data);
    set_flag(&mut ip.xDCE_flag, 3, false);
    ip.xD54_throwNum = 0;
    ip.xD50_landNum = 0;
    ip.xD58 = 0;
    ip.xD5C = 2;
    ip.xDCC_flag.b3 = true;
    ip.xDCC_flag.b4567 = 15;
    ip.xAC4_ignoreItemID = super::IgnoreId(Some(spawn.x40 as u32));
    ip.destroy_type = 4;
    set_flag(&mut ip.xDCE_flag, 5, false);
    set_flag(&mut ip.xDCE_flag, 4, false);
    ip.xC54 = 0.0;
    // The original clears xC58.x three times and never y or z.
    ip.xC58.x = 0.0;
    let f = &mut ip.xDC8_word.flags;
    f.x9 = false;
    f.x3 = false;
    f.x4 = false;
    f.x5 = false;
    f.x6 = false;
    f.x7 = false;
    f.x8 = false;
    set_flag(&mut ip.xDD0_flag, 5, false);
    ip.xD09 = 0;
    set_flag(&mut ip.xDD0_flag, 4, false);
    ip.xCA8 = 0;
    ip.xCBC_hitlagFrames = 0.0;
    ip.xCC0 = 0.0;
    ip.xDC8_word.flags.xA = false;
    ip.xDC8_word.flags.xB = false;
    ip.xDCC_flag.b1 = false;
    ip.xC70 = 1.0;
    ip.xC6C = 1.0;
    ip.xDCC_flag.b2 = false;
    set_flag(&mut ip.xDD0_flag, 0, false);
    set_flag(&mut ip.xDCE_flag, 6, false);
    set_flag(&mut ip.xDCF_flag, 0, false);
    set_flag(&mut ip.xDCF_flag, 1, false);
    set_flag(&mut ip.xDCF_flag, 2, false);
    set_flag(&mut ip.xDD1_flag, 0, false);
    set_flag(&mut ip.xDCE_flag, 7, true);
    ip.scl = a.f32(data, attr::SCALE);
    ip.x3C = 0.0;
    ip.xDCD_flag.x0.b3 = false;
    ip.xDCD_flag.x0.b4 = false;
    set_flag(&mut ip.xDD0_flag, 7, false);
    ip.xBC0 = Default::default();
    set_flag(&mut ip.xDD1_flag, 1, false);
    ip.xDCD_flag.x0.b2 = false;
    ip.xD6C = NO_SFX;
    ip.sfx_unk2 = NO_SFX;
    ip.sfx_unk1 = NO_SFX;
    ip.xD70 = a.i32(data, attr::X6C);
    ip.xD74 = a.i32(data, attr::X70);
    ip.xD78 = a.i32(data, attr::X74);
    ip.destroy_sfx = a.i32(data, attr::DESTROY_SFX);
    ip.xD80 = a.i32(data, attr::X7C);
    ip.xD84 = a.i32(data, attr::X80);
    super::hitbox::clear_hit_ids(r.game, gobj);
    super::hitbox::clear_hit_ids2(r.game, gobj);
    lib2725::stop_holding_still(r.game, gobj);
    let parent = spawn.x0_parent_gobj;
    let parent_is_fighter = is_fighter(r.game, parent);
    let ip = item_mut(r.game, gobj);
    ip.xDC8_word.flags.x1 = !parent_is_fighter;
    ip.x5C8 = 0;
    ip.xBC4 = Default::default();
    ip.x5C9 = 255;
    super::anim::clear_overlay(ip);
    set_flag(&mut ip.xDCF_flag, 3, false);
    set_flag(&mut ip.xDCF_flag, 4, false);
    set_flag(&mut ip.xDCF_flag, 5, false);
    ip.xDAA = [1];
    if r.game.it.db_menu.ShowItemPickupRange {
        set_flag(&mut item_mut(r.game, gobj).xDAA, 4, true);
    }
    let team = match parent {
        None => u8::MAX,
        Some(p) if parent_is_fighter => {
            let f = super::owner::fighter_of(r.game, p);
            r.game.fighters[f.0].team
        }
        Some(p) if is_item(r.game, Some(p)) => super::item(r.game, p).x20_team_id,
        Some(_) => u8::MAX,
    };
    item_mut(r.game, gobj).x20_team_id = team;
}

/// Gives an item its model, `Item_802680CC`: its kind's joint tree, or a bare joint.
#[doc(alias = "Item_802680CC")]
pub fn make_model(r: &mut Running, gobj: GObjId) {
    let joint = super::item(r.game, gobj).xC8_joint;
    let j = match joint {
        Some(_) => jobj::load_joint(&mut r.game.objs, r.data, joint).expect("a loaded joint"),
        None => {
            let j = jobj::alloc(&mut r.game.objs);
            r.game.objs.jobjs[j].mtx = crate::mtx::Mtx34::IDENTITY;
            r.game.objs.jobjs[j].scl = None;
            j
        }
    };
    let kind = jobj_kind(r.game);
    r.game.world.set_obj(gobj, kind, Obj::JObj(j));
}

/// `HSD_GObj_JObjKind`.
pub fn jobj_kind(_game: &Game) -> u8 {
    super::super::state::OBJ_JOBJ
}

/// The joint after `j` in depth-first order, or none at the end of the tree.
pub fn next_in_tree(game: &Game, j: JObjId) -> Option<JObjId> {
    let jobjs = &game.objs.jobjs;
    if let Some(c) = jobjs[j].child {
        return Some(c);
    }
    if let Some(n) = jobjs[j].next {
        return Some(n);
    }
    let mut cur = j;
    loop {
        let p = jobjs[cur].parent?;
        if let Some(n) = jobjs[p].next {
            return Some(n);
        }
        cur = p;
    }
}

/// Gives the model's materials the items' class (`it_mobj`), `Item_8026814C`.
#[doc(alias = "Item_8026814C")]
pub fn set_material_class(game: &mut Game, gobj: GObjId) {
    let mut cur = jobj_of(game, gobj);
    while let Some(j) = cur {
        let mut d = jobj::get_dobj(&game.objs.jobjs, Some(j));
        while let Some(dobj) = d {
            if let Some(m) = game.objs.dobjs[dobj].mobj {
                crate::hsd::mobj::change_class(
                    &mut game.objs.mobjs,
                    m,
                    crate::hsd::mobj::MObjClass::Item,
                );
            }
            d = game.objs.dobjs[dobj].next;
        }
        cur = next_in_tree(game, j);
    }
}

/// Lists the model's joints in depth-first order for an item whose model has bones,
/// `Item_802682F0`.
#[doc(alias = "Item_802682F0")]
pub fn make_bone_table(r: &mut Running, gobj: GObjId) {
    let data = r.data;
    let count = article(super::item(r.game, gobj))
        .model(data)
        .bone_count(data);
    if count == 0 {
        item_mut(r.game, gobj).xBBC_dynamicBoneTable = None;
        return;
    }
    let mut table = BoneTable::default();
    let mut cur = jobj_of(r.game, gobj);
    let mut i = 0;
    while let Some(j) = cur {
        table.bones[i] = Some(j);
        i += 1;
        cur = next_in_tree(r.game, j);
    }
    table.len = i as u8;
    let id = BoneTableId(r.game.it.bone_tables.alloc(table));
    item_mut(r.game, gobj).xBBC_dynamicBoneTable = Some(id);
}

/// Sets up the item's dynamic bones from its article, `Item_80268560`.
#[doc(alias = "Item_80268560")]
pub fn init_dynamics(r: &mut Running, gobj: GObjId) {
    let data = r.data;
    let ip = item_mut(r.game, gobj);
    match article(ip).dynamics(data) {
        None => {
            ip.x374_dynamicBonesNum = 0;
            ip.xB68 = 0;
        }
        Some(d) => {
            let n = data.u32(d) as i32;
            ip.x374_dynamicBonesNum = n;
            let descs = data.ptr(d.at(4));
            for i in 0..n.max(0) as usize {
                // `BoneDynamicsDesc`: the bone, then the chain's description.
                let entry = descs
                    .expect("an item's dynamics without bones")
                    .at(0x18 * i as u32);
                let bone = data.u32(entry) as usize;
                let count = data.u32(entry.at(8));
                let table = super::item(r.game, gobj)
                    .xBBC_dynamicBoneTable
                    .expect("an item with dynamic bones has a bone table");
                let j = r.game.it.bone_tables[table.0].bones[bone];
                let mut desc = super::item(r.game, gobj).xD4_dynamicBones[i]
                    .dyn_desc
                    .clone();
                r.game.load_dynamics_chain(data, j, &mut desc, count);
                let b = &mut item_mut(r.game, gobj).xD4_dynamicBones[i];
                b.skeleton = j;
                b.flags = 0;
                r.game.set_dynamics_params(data, entry.at(4), &mut desc);
                item_mut(r.game, gobj).xD4_dynamicBones[i].dyn_desc = desc;
            }
        }
    }
}

/// Makes an item, `Item_8026862C`: its GObj, user data, model, processes and initial state,
/// unless its hold kind is at its limit.
#[doc(alias = "Item_8026862C")]
pub fn create(r: &mut Running, spawn: &SpawnItem) -> Option<GObjId> {
    if at_limit(r.game, spawn.hold_kind) {
        return None;
    }
    let gobj = r.game.world.create(CLASS_ITEM, P_LINK_ITEM, 0);
    r.game
        .world
        .setup_gx_link(gobj, tables::render_func(spawn.kind), 6, 0);
    let id = ItemId(r.game.items.alloc_stale(Item::default()));
    // A block new from the heap holds what the scenes before left there (docs/engine.md, a
    // session's heap leftovers).
    let addr = r.game.items.addr(id.0);
    if let Some(left) =
        crate::game::leftovers::import(r.data, addr, crate::game::leftovers::pool_grew())
    {
        r.game.items[id.0] = left;
        // Where the block held the GPU's pixels, which the comparer leaves out while they
        // hold the image's fill (docs/engine.md, "Uninitialized values").
        let rt = crate::game::leftovers::render_target_runs(
            addr,
            <crate::game::structs::Item as crate::game::mirror::Mirror>::SIZE,
        );
        if rt.is_empty() {
            r.game.it.render_leftovers.remove(&id.0);
        } else {
            r.game.it.render_leftovers.insert(id.0, rt);
        }
    } else {
        r.game.it.render_leftovers.remove(&id.0);
    }
    // The variables the block's last item left are stale until the new kind writes them.
    r.game.items[id.0].xDD4_itemVar.mark_stale();
    // So are its hitboxes' joints, until the script makes each box (`HitJObj`).
    for h in &mut r.game.items[id.0].x5D4_hitboxes {
        h.hit.jobj.mark_unknown();
    }
    r.game
        .world
        .init_user_data(gobj, USER_DATA_ITEM, UserData::Item(id));
    let _ = ON_USER_DATA_REMOVE;
    init_vars(r, gobj, spawn);
    make_model(r, gobj);
    make_bone_table(r, gobj);
    set_material_class(r.game, gobj);
    apply_scale(r.game, gobj);
    coll::init_hurtboxes(r, gobj);
    init_dynamics(r, gobj);
    let w = &mut r.game.world;
    for (f, prio) in [
        (PROC_HITLAG, 0),
        (PROC_ANIM, 1),
        (PROC_PHYS, 4),
        (PROC_COLL, 5),
        (PROC_ACCESSORY, 9),
        (PROC_JUMPED_ON, 11),
        (PROC_GRAB, 12),
        (PROC_HIT, 13),
        (PROC_REACT, 14),
        (PROC_DYNAMICS, 16),
    ] {
        w.setup_proc(gobj, CodeRef(f), prio);
    }
    init_position(r, gobj, spawn);
    on_spawned(r, gobj);
    // `foobar`: common items and enemies blink as they appear.
    match super::item(r.game, gobj).hold_kind {
        0 | 6 => {
            let data = r.data;
            let t = common_data(r.game).i32(data, common::X2C);
            let ip = item_mut(r.game, gobj);
            if ip.xD0C == 2 {
                set_flag(&mut ip.xDD0_flag, 7, true);
            }
            ip.xD40 = mem(fp::frsp(f64::from(t)));
            set_flag(&mut ip.xDD0_flag, 6, true);
            super::hitbox::start_blink(r.game, gobj);
            super::anim::start_colanim(r.game, data, gobj, 3, 0);
        }
        _ => {
            let ip = item_mut(r.game, gobj);
            ip.xD40 = 0.0;
            set_flag(&mut ip.xDD0_flag, 6, false);
        }
    }
    count(r.game, gobj);
    // `foobar2`: a character's item keeps the character's data loaded.
    let (kind, owner) = {
        let ip = super::item(r.game, gobj);
        (ip.kind, ip.owner)
    };
    if (MARIO_FIRE..UNK4).contains(&kind) && is_fighter(r.game, owner) {
        let f = super::owner::fighter_of(r.game, owner.unwrap());
        let fkind = r.game.fighters[f.0].kind;
        let ip = item_mut(r.game, gobj);
        ip.xDC8_word.flags.xE = true;
        ip.ecb_lock = fkind;
        super::owner::inc_fighter_refs(r.game, fkind);
    }
    // `foobar3`: the camera follows some items.
    let cam = super::item(r.game, gobj).xDCD_flag.x0.b01;
    if cam != 0 {
        let b = camera::alloc_box(&mut r.game.camera.boxes, if cam == 1 { 0 } else { 2 });
        item_mut(r.game, gobj).x520_cameraBox = Some(b);
        {
            let c = common_data(r.game);
            let d = r.data;
            let cb = &mut r.game.camera.boxes.boxes[b.0 as usize];
            cb.target_ext.h.x = c.f32(d, common::X14C);
            cb.target_ext.h.y = c.f32(d, common::X150);
            cb.target_ext.v.x = c.f32(d, common::X154);
            cb.target_ext.v.y = c.f32(d, common::X158);
        }
    }
    Some(gobj)
}

/// Places a new item and sets its motion and collision, `Item_80267130`.
#[doc(alias = "Item_80267130")]
pub fn init_position(r: &mut Running, gobj: GObjId, spawn: &SpawnItem) {
    let model = jobj_of(r.game, gobj);
    {
        let ip = item_mut(r.game, gobj);
        ip.pos = spawn.prev_pos;
        ip.facing_dir = spawn.facing_dir;
        ip.init_facing_dir = spawn.facing_dir;
        ip.owner = spawn.x0_parent_gobj;
        let dir = if reg(ip.facing_dir) == fp::fneg(1.0) {
            -1
        } else {
            1
        };
        ip.x378_itemColl.set_facing_dir(dir);
    }
    apply_facing(r.game, gobj, model);
    lib2725::set_velocity(r.game, gobj, spawn.vel);
    {
        let ip = item_mut(r.game, gobj);
        ip.x5CC_currentAnimFrame = 0.0;
        ip.x5D0_animFrameSpeed = 1.0;
    }
    lib2725::set_damage_taken(r.game, gobj, spawn.x3C_damage);
    super::hitbox::clear_dealt(r.game, gobj);
    {
        let ip = item_mut(r.game, gobj);
        ip.xC64_reflectGObj = None;
        ip.xDCC_flag.b0 = false;
        ip.xC90_absorbGObj = None;
        ip.xD0C = 0;
    }
    lib26b1::enable_pickup(r.game, gobj);
    item_mut(r.game, gobj).xDC8_word.flags.x13 = false;
    if spawn.x48_ground_or_air == 1 {
        super::maplib::enter_air(item_mut(r.game, gobj));
    } else {
        super::maplib::enter_ground(item_mut(r.game, gobj));
    }
    super::maplib::init_coll(r, gobj, spawn);
    lib2725::init_spin(r.game, gobj);
    let pos = super::item(r.game, gobj).pos;
    set_translate(r.game, model.expect("an item without a model"), pos);
    let x6c = common_data(r.game).f32(r.data, common::X6C);
    lib2725::clamp_fall(r.game, gobj, x6c);
    lib2725::clear_hitboxes(r.game, r.data, gobj);
    coll::set_hurtbox_state(r.game, gobj, coll::HURT_ENABLED);
    coll::clear_x1b(r.game, gobj);
    {
        let ip = item_mut(r.game, gobj);
        ip.on_accessory = None;
        ip.touched = None;
        ip.entered_hitlag = None;
        ip.exited_hitlag = None;
        ip.jumped_on = None;
    }
    let owner = super::item(r.game, gobj).owner;
    debug_owner_bubbles(r.game, gobj, owner);
    super::lib279c::track_spawn(r, gobj, spawn);
    super::anim::clear_overlay(item_mut(r.game, gobj));
    hold_kind_scale(r, gobj);
}

/// The CPU level of the single-player mode being played, `gm_8017E068`: -1 outside Classic,
/// Adventure and All-Star.
#[doc(alias = "gm_8017E068")]
pub fn one_player_cpu_level(r: &Running) -> i32 {
    // `fn_8017DF28`: those modes' records.
    if (3..=5).contains(&r.game.gr.game_mode) {
        code::unported_call("fn_8017DF28 (a single-player mode's record)");
    }
    -1
}

/// Enemies and stage items take the common data's damage scale for the single-player mode's
/// CPU level (`ItUnkHoldKind`).
fn hold_kind_scale(r: &mut Running, gobj: GObjId) {
    if matches!(super::item(r.game, gobj).hold_kind, 4..=7) {
        let level = one_player_cpu_level(r);
        if level >= 0 {
            let c = common_data(r.game).f32(r.data, common::X80 + 4 * level as u32);
            item_mut(r.game, gobj).xC3C = c;
        }
    }
}

/// Copies the debug menu's collision bubble setting into a new item's debug draw flags
/// (`db_80225DD8` for an owner, else `db_80225D64`): a fighter owner's own setting
/// (`fn_8022697C`) for its items.
#[doc(alias = "db_80225DD8")]
#[doc(alias = "db_80225D64")]
pub fn debug_owner_bubbles(game: &mut Game, gobj: GObjId, owner: Option<GObjId>) {
    let bubbles = game.it.db_bubbles as u8;
    if owner.is_none() || !is_fighter(game, owner) {
        item_mut(game, gobj).xDAA[0] |= bubbles;
        return;
    }
    let f = super::owner::fighter_of(game, owner.unwrap());
    let x = game.fighters[f.0].x21FC_flag[0];
    let ip = item_mut(game, gobj);
    ip.xDAA[0] &= 0xFC;
    if x != 0 {
        ip.xDAA[0] |= x & 3;
    }
}

/// Calls the kind's `spawned` callback, `Item_8026A810`.
#[doc(alias = "Item_8026A810")]
pub fn on_spawned(r: &mut Running, gobj: GObjId) {
    if let Some(cb) = logic(r.game, gobj).spawned {
        call(r, gobj, cb);
    }
}

/// The item's logic table.
pub fn logic(game: &Game, gobj: GObjId) -> &'static tables::ItemLogic {
    tables::logic(
        super::item(game, gobj)
            .xB8_itemLogicTable
            .expect("an item without a logic table"),
    )
}

/// Makes an item in the air, `Item_80268B18`.
#[doc(alias = "Item_80268B18")]
pub fn spawn_air(r: &mut Running, spawn: &mut SpawnItem) -> Option<GObjId> {
    spawn.x48_ground_or_air = 1;
    spawn.x10 = 0;
    spawn.hold_kind = hold_kind_of(spawn.kind);
    create(r, spawn)
}

/// Makes an item on the ground, `Item_80268B5C`.
#[doc(alias = "Item_80268B5C")]
pub fn spawn_ground(r: &mut Running, spawn: &mut SpawnItem) -> Option<GObjId> {
    spawn.x48_ground_or_air = 0;
    spawn.x10 = 0;
    spawn.hold_kind = hold_kind_of(spawn.kind);
    create(r, spawn)
}

/// A drop by kind's spawn of `kind` at `pos`: no parent, facing left, still, with a first
/// collision (the inline `Item_InitSpawnPosition` and `Item_InitSpawnCommonFields` of the
/// items' spawns by position), its depth put on the plane when `on_plane`.
pub fn drop_spawn(kind: i32, pos: Vec3, on_plane: bool) -> SpawnItem {
    let mut spawn = SpawnItem {
        kind,
        ..Default::default()
    };
    spawn.prev_pos = pos;
    if on_plane {
        spawn.prev_pos.z = 0.0;
    }
    spawn.pos = spawn.prev_pos;
    spawn.facing_dir = -1.0;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = None;
    spawn.x4_parent_gobj2 = None;
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    spawn
}

/// Makes an item on the ground with `x18` set, `Item_80268B9C`.
#[doc(alias = "Item_80268B9C")]
pub fn spawn_ground_marked(r: &mut Running, spawn: &mut SpawnItem) {
    spawn.x48_ground_or_air = 0;
    spawn.x10 = 1;
    spawn.hold_kind = hold_kind_of(spawn.kind);
    create(r, spawn);
}

/// Adds a state's animations to the model, skipping the joints dynamic bones move,
/// `Item_80268BE0`.
#[doc(alias = "Item_80268BE0")]
pub fn add_anims(
    r: &mut Running,
    j: Option<JObjId>,
    aj: Option<DataRef>,
    mj: Option<DataRef>,
    sj: Option<DataRef>,
    item: ItemId,
) {
    let Some(j) = j else { return };
    let data = r.data;
    // An `HSD_AnimJoint`'s child and next are its first two words, as are a material and a
    // shape animation joint's.
    let child = |d: Option<DataRef>| d.and_then(|d| data.ptr(d));
    let next = |d: Option<DataRef>| d.and_then(|d| data.ptr(d.at(4)));
    if aj.is_some() {
        let ip = &r.game.items[item.0];
        let n = ip.x374_dynamicBonesNum;
        for i in 0..n.max(0) as usize {
            if ip.xD4_dynamicBones[i].skeleton == Some(j) {
                if next(aj).is_some() {
                    let nj = r.game.objs.jobjs[j].next;
                    add_anims(r, nj, next(aj), next(mj), next(sj), item);
                }
                return;
            }
        }
    }
    jobj::add_anim(&mut r.game.objs, data, Some(j), aj, mj, sj);
    if let Some(c) = r.game.objs.jobjs[j].child {
        add_anims(r, Some(c), child(aj), child(mj), child(sj), item);
    }
    if let Some(n) = r.game.objs.jobjs[j].next {
        add_anims(r, Some(n), next(aj), next(mj), next(sj), item);
    }
}

/// Gives the model a state's animations from the start, `Item_80268D34`.
#[doc(alias = "Item_80268D34")]
pub fn set_state_anims(r: &mut Running, gobj: GObjId, desc: DataRef) {
    set_state_anims_at(r, gobj, desc, 0.0);
}

/// [`set_state_anims`] with the animations requested from `frame`.
pub fn set_state_anims_at(r: &mut Running, gobj: GObjId, desc: DataRef, frame: f32) {
    let data = r.data;
    let j = jobj_of(r.game, gobj);
    let i = item_of(r.game, gobj);
    jobj::remove_anim_all(&mut r.game.objs, j);
    let joint = r.game.items[i.0].xC8_joint;
    if let Some(joint) = joint {
        let first = j.and_then(|j| r.game.objs.jobjs[j].child);
        crate::game::gr::anime::reset_tree_srt(
            &mut r.game.objs,
            data,
            first,
            data.ptr(joint.at(8)),
        );
    }
    add_anims(
        r,
        j,
        data.ptr(desc),
        data.ptr(desc.at(4)),
        data.ptr(desc.at(8)),
        i,
    );
    let speed = r.game.items[i.0].x5D0_animFrameSpeed;
    crate::lb::anim::set_framerate(&mut r.game.objs, j, speed);
    jobj::req_anim_all(&mut r.game.objs, j, frame);
}

/// Restarts the model's animation at `frame`, `Item_80268DD4`.
#[doc(alias = "Item_80268DD4")]
pub fn seek_anim(r: &mut Running, gobj: GObjId, frame: f32) {
    let j = jobj_of(r.game, gobj);
    let speed = super::item(r.game, gobj).x5D0_animFrameSpeed;
    crate::lb::anim::set_framerate(&mut r.game.objs, j, speed);
    jobj::req_anim_all(&mut r.game.objs, j, frame);
    jobj::anim_all(
        &mut r.game.objs,
        r.data,
        &mut crate::game::code::ParticleHooks,
        j,
    );
    let f = crate::lb::anim::current_frame(&r.game.objs, j);
    item_mut(r.game, gobj).x5CC_currentAnimFrame = f;
}

/// Starts a state's script (found by [`super::anim::state_script`]), `Item_80268E40`.
#[doc(alias = "Item_80268E40")]
pub fn start_script(ip: &mut Item, script: Option<DataRef>) {
    ip.x524_cmd.script.cursor = script;
    ip.x524_cmd.script.loop_count = 0;
    ip.x524_cmd.script.set_timer_f32(0.0);
}

/// `Item_StateChangeFlags`.
pub mod change {
    pub const ANIM_UPDATE: u32 = 1 << 1;
    pub const DROP_UPDATE: u32 = 1 << 2;
    pub const MODEL_UPDATE: u32 = 1 << 3;
    pub const HIT_PRESERVE: u32 = 1 << 4;
    pub const SFX_PRESERVE: u32 = 1 << 5;
    pub const COLANIM_PRESERVE: u32 = 1 << 6;
    pub const UNK_UPDATE: u32 = 1 << 7;
    pub const CMD_UPDATE: u32 = 1 << 8;
}

/// Changes the item's state, `Item_80268E5C`: its animation and script, the state's
/// callbacks, and what `flags` keep or reset.
#[doc(alias = "Item_80268E5C")]
pub fn change_state(r: &mut Running, gobj: GObjId, msid: i32, flags: u32) {
    use change::*;
    let data = r.data;
    let model = jobj_of(r.game, gobj);
    let pos = {
        let ip = item_mut(r.game, gobj);
        ip.msid = msid;
        ip.xDC8_word.flags.x14 = false;
        ip.pos
    };
    set_translate(r.game, model.expect("an item without a model"), pos);
    lib26b1::flush_effects(r, gobj);
    if super::item(r.game, gobj).xDC8_word.flags.x2 {
        lib26b1::hold_flags_off(r.game, gobj);
        lib2725::stop_holding_still(r.game, gobj);
    }
    if flags & MODEL_UPDATE != 0 {
        lib2725::stop_spin(r.game, data, gobj);
    }
    if flags & COLANIM_PRESERVE == 0 {
        let colanims = r.game.it.colanims.expect("no item color animations");
        let i = i32::from_be_bytes(super::item(r.game, gobj).x548_colorOverlay.x28_colanim);
        if data.u8(colanims.at(8 * i as u32 + 5)) != 0 {
            super::anim::clear_overlay(item_mut(r.game, gobj));
        }
    }
    if flags & SFX_PRESERVE == 0 {
        stop_sfx2(r, gobj);
    }
    apply_facing(r.game, gobj, model);
    {
        let ip = item_mut(r.game, gobj);
        ip.xC40 = if flags & DROP_UPDATE != 0 {
            ip.xC44
        } else {
            1.0
        };
    }
    if flags & HIT_PRESERVE == 0 && super::item(r.game, gobj).xDC8_word.flags.x16 {
        lib2725::clear_hitboxes(r.game, data, gobj);
    }
    let states = super::item(r.game, gobj)
        .xBC_itemStateContainer
        .expect("an item without states");
    let state = tables::states(states)[msid as usize];
    {
        let ip = item_mut(r.game, gobj);
        ip.xCC8_knockback = 0.0;
        ip.anim_id = state.anim_id;
        ip.x5CC_currentAnimFrame = 0.0;
    }
    if state.anim_id != -1 {
        let ip = item_mut(r.game, gobj);
        let descs = article(ip).states(data).expect("an article without states");
        let desc = descs.at(0x10 * state.anim_id as u32);
        ip.xD0_itemStateDesc = Some(desc);
        let kind = super::item(r.game, gobj).kind;
        let script = super::anim::state_script(r.game, data, kind, state.anim_id, desc);
        if flags & ANIM_UPDATE != 0 {
            set_state_anims(r, gobj, desc);
            if flags & UNK_UPDATE != 0 {
                let ip = item_mut(r.game, gobj);
                let a = attr_of(ip);
                ip.scl = a.f32(data, attr::SCALE);
                apply_scale(r.game, gobj);
                let ip = item_mut(r.game, gobj);
                let a = attr_of(ip);
                ip.xBCC_unk = a.vec2(data, attr::X30);
                ip.xBD4_grabRange = a.vec2(data, attr::GRAB_RANGE);
                ip.xBEC = a.ecb(data, attr::ECB);
                ip.xBDC = a.ecb(data, attr::ECB);
            } else {
                apply_scale(r.game, gobj);
            }
            start_script(item_mut(r.game, gobj), script);
        } else if flags & CMD_UPDATE != 0 {
            start_script(item_mut(r.game, gobj), script);
        }
        jobj::anim_all(
            &mut r.game.objs,
            data,
            &mut crate::game::code::ParticleHooks,
            model,
        );
        super::anim::run_overlay(r, gobj);
        super::anim::run_script(r, gobj);
    } else {
        jobj::remove_anim_all(&mut r.game.objs, model);
        item_mut(r.game, gobj).x524_cmd.script.cursor = None;
    }
    let ip = item_mut(r.game, gobj);
    ip.animated = state.animated;
    ip.physics_updated = state.physics_updated;
    ip.collided = state.collided;
    ip.on_accessory = None;
    ip.touched = None;
    ip.entered_hitlag = None;
    ip.exited_hitlag = None;
    ip.jumped_on = None;
    set_flag(&mut ip.xDD0_flag, 5, false);
    ip.xD09 = 0;
    coll::refresh_hurtboxes(r.game, gobj);
}

/// The item's hitlag process (priority 0), `Item_802693E4`: ends a hitlag a hit or a clank
/// asked to end, and counts the hitlag down.
#[doc(alias = "Item_802693E4")]
pub fn proc_hitlag(r: &mut Running, gobj: GObjId) {
    let i = item_of(r.game, gobj);
    if r.game.items[i.0].xDC8_word.flags.x5 {
        end_hit_hitlag(r, gobj);
        r.game.items[i.0].xDC8_word.flags.x5 = false;
    }
    if r.game.items[i.0].xDC8_word.flags.x6 {
        end_clank_hitlag(r, gobj);
        r.game.items[i.0].xDC8_word.flags.x6 = false;
    }
    let ip = &mut r.game.items[i.0];
    if reg(ip.xCBC_hitlagFrames) > 0.0 {
        ip.xCBC_hitlagFrames = mem(fp::fsubs(reg(ip.xCBC_hitlagFrames), 1.0));
        if reg(ip.xCBC_hitlagFrames) <= 0.0 {
            ip.xCBC_hitlagFrames = 0.0;
            ip.xDC8_word.flags.xB = false;
            ip.xDC8_word.flags.xA = false;
            let f = &ip.xDC8_word.flags;
            if !f.x8 && !f.x7 {
                exit_hitlag(r, gobj);
            }
        }
    }
}

/// Animates the model and runs its script, `Item_802694CC`.
#[doc(alias = "Item_802694CC")]
pub fn animate(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj);
    jobj::anim_all(
        &mut r.game.objs,
        r.data,
        &mut crate::game::code::ParticleHooks,
        j,
    );
    let f = crate::lb::anim::current_frame(&r.game.objs, j);
    item_mut(r.game, gobj).x5CC_currentAnimFrame = f;
    super::anim::run_script(r, gobj);
}

/// The item's animation process (priority 1), `Item_80269528`: animates it and calls its
/// state's `animated` callback, counts its blink and its lifetime down, and keeps it clear of
/// fighters and other items.
#[doc(alias = "Item_80269528")]
pub fn proc_anim(r: &mut Running, gobj: GObjId) {
    if !super::item(r.game, gobj).xDC8_word.flags.x9 {
        animate(r, gobj);
        if let Some(cb) = super::item(r.game, gobj).animated
            && call_pred(r, gobj, cb)
        {
            item_mut(r.game, gobj).destroy_type = 0;
            destroy(r, gobj);
            return;
        }
    }
    if flag(super::item(r.game, gobj).xDD0_flag, 6) {
        let ip = item_mut(r.game, gobj);
        ip.xD40 = mem(fp::fsubs(reg(ip.xD40), 1.0));
        if reg(ip.xD40) <= 0.0 {
            set_flag(&mut ip.xDD0_flag, 6, false);
            ip.xD40 = 0.0;
            super::anim::clear_overlay(ip);
            if !flag(ip.xDD0_flag, 7) {
                super::hitbox::end_blink(r.game, gobj);
            }
        }
    }
    let ip = super::item(r.game, gobj);
    if ip.xDC8_word.flags.x15 && ip.kind < L_GUN_RAY && !flag(ip.xDD0_flag, 3) {
        let x34 = common_data(r.game).u32(r.data, common::X34);
        let ip = item_mut(r.game, gobj);
        ip.xD44_lifeTimer = mem(fp::fsubs(reg(ip.xD44_lifeTimer), 1.0));
        if reg(ip.xD44_lifeTimer) <= 0.0 {
            ip.destroy_type = 0;
            destroy(r, gobj);
            return;
        }
        if reg(ip.xD44_lifeTimer) <= fp::frsp(f64::from(x34)) {
            lib2725::flicker(r.game, r.data, gobj);
        }
    }
    coll::keep_clear(r.game, r.data, gobj);
    coll::set_x1b(r.game, gobj);
    super::anim::run_overlay(r, gobj);
}

/// Destroys the item if it passed a blast zone it's checked against, `Item_802696CC`.
#[doc(alias = "Item_802696CC")]
pub fn check_blast_zones(r: &mut Running, gobj: GObjId) -> bool {
    use crate::game::gr::stage;
    let ip = super::item(r.game, gobj);
    let zones = ip.xDCC_flag.b4567;
    let (x, y) = (reg(ip.pos.x), reg(ip.pos.y));
    let out = (zones & 8 != 0 && x > reg(stage::blast_right(r.game)))
        || (zones & 4 != 0 && x < reg(stage::blast_left(r.game)))
        || (zones & 1 != 0 && y > 10000.0)
        || (zones & 2 != 0 && y < reg(stage::blast_bottom(r.game)));
    if out {
        item_mut(r.game, gobj).destroy_type = 3;
        destroy(r, gobj);
    }
    out
}

/// Adds two vectors as `PSVECAdd` does (paired-single adds of x, y and z).
pub fn vec_add(a: Vec3, b: Vec3) -> Vec3 {
    let add = |a: f32, b: f32| mem(fp::fadds(reg(a), reg(b)));
    Vec3::new(add(a.x, b.x), add(a.y, b.y), add(a.z, b.z))
}

/// The item's physics process (priority 4), `Item_802697D4`: its state's `physics_updated`
/// callback, then its velocity, the floor's motion and the blast zones.
#[doc(alias = "Item_802697D4")]
pub fn proc_phys(r: &mut Running, gobj: GObjId) {
    let x9 = super::item(r.game, gobj).xDC8_word.flags.x9;
    if !x9 && let Some(cb) = super::item(r.game, gobj).physics_updated {
        call(r, gobj, cb);
    }
    let ip = super::item(r.game, gobj);
    if !ip.xDC8_word.flags.x13 {
        if !ip.xDC8_word.flags.x9 {
            let mut v = vec_add(ip.x40_vel, ip.x70_nudge);
            if ip.ground_or_air == 0 && !ip.xDCD_flag.x0.b3 {
                super::maplib::slide(r.game, r.data, gobj, &mut v);
            }
            let ip = item_mut(r.game, gobj);
            ip.pos = vec_add(ip.pos, v);
        }
        let ip = super::item(r.game, gobj);
        if ip.xDCC_flag.b3 && ip.xDCC_flag.b4567 & 0xF != 0 && check_blast_zones(r, gobj) {
            return;
        }
        let ip = super::item(r.game, gobj);
        if reg(ip.xD3C_spinSpeed) != 0.0 && ip.ground_or_air == 1 {
            lib2725::spin(r.game, r.data, gobj);
        }
    }
    let ip = super::item(r.game, gobj);
    if flag(ip.xDD1_flag, 1) && ip.ground_or_air == 1 {
        let mut v = super::item(r.game, gobj).x58_vec_unk;
        crate::game::gr::grlib::item_wind(r.game, &mut v);
        item_mut(r.game, gobj).x58_vec_unk = v;
    }
    let ip = item_mut(r.game, gobj);
    ip.pos = vec_add(ip.pos, ip.x58_vec_unk);
    lib2725::clear_x58(r.game, gobj);
    let ip = super::item(r.game, gobj);
    if ip.ground_or_air == 0 {
        let line = if flag(ip.xDCE_flag, 3) && super::ground::on_line(r.game, gobj) {
            ip.xC30
        } else {
            ip.x378_itemColl.floor.index
        };
        let pos = ip.pos;
        if let Some(speed) = super::maplib::floor_speed(r.game, line, pos) {
            item_mut(r.game, gobj).x64_vec_unk2 = speed;
        }
    }
    let ip = item_mut(r.game, gobj);
    ip.pos = vec_add(ip.pos, ip.x64_vec_unk2);
    lib2725::clear_x64(r.game, gobj);
    coll::refresh_hurtboxes(r.game, gobj);
}

/// The item's collision process (priority 5), `Item_80269978`: its state's `collided`
/// callback, then its model follows it and its hit and hurt boxes follow the model.
#[doc(alias = "Item_80269978")]
pub fn proc_coll(r: &mut Running, gobj: GObjId) {
    if let Some(cb) = super::item(r.game, gobj).collided
        && call_pred(r, gobj, cb)
    {
        item_mut(r.game, gobj).destroy_type = 1;
        destroy(r, gobj);
        return;
    }
    let pos = super::item(r.game, gobj).pos;
    let j = jobj_of(r.game, gobj).expect("an item without a model");
    set_translate(r.game, j, pos);
    super::hitbox::update_boxes(r.game, gobj);
    lib26b1::check_burst(r, gobj);
}

/// The item's accessory process (priority 9), `Item_80269A9C`: its queued effects, its
/// `on_accessory` callback, its camera box, and its ECB.
#[doc(alias = "Item_80269A9C")]
pub fn proc_accessory(r: &mut Running, gobj: GObjId) {
    lib26b1::flush_effects(r, gobj);
    if !super::item(r.game, gobj).xDC8_word.flags.x9
        && let Some(cb) = super::item(r.game, gobj).on_accessory
    {
        call(r, gobj, cb);
    }
    let ip = super::item(r.game, gobj);
    if ip.xDCD_flag.x0.b01 != 0
        && let Some(b) = ip.x520_cameraBox
    {
        let pos = ip.pos;
        let cb = &mut r.game.camera.boxes.boxes[b.0 as usize];
        cb.pos = pos;
        cb.bone_pos = pos;
    }
    coll::update_ecb(r.game, r.data, gobj);
    coll::clear_x1b(r.game, gobj);
    coll::track_fighters(r.game, gobj);
}

/// The item's process for being jumped on (priority 11), `Item_80269B60`.
#[doc(alias = "Item_80269B60")]
pub fn proc_jumped_on(r: &mut Running, gobj: GObjId) {
    let ip = super::item(r.game, gobj);
    if ip.xCFC.is_some()
        && let Some(cb) = ip.jumped_on
        && call_pred(r, gobj, cb)
    {
        item_mut(r.game, gobj).destroy_type = 2;
        destroy(r, gobj);
        return;
    }
    coll::update_hitboxes(r.game, r.data, gobj);
    coll::age_hit_victims(r.game, gobj);
}

/// The item's grab process (priority 12), `Item_80269BE4`: items that grab fighters (a
/// barrel cannon) look for one; a catch runs the item's grab callback, then the fighter's.
#[doc(alias = "Item_80269BE4")]
pub fn proc_grab(r: &mut Running, gobj: GObjId) {
    if !flag(super::item(r.game, gobj).xDD0_flag, 5) {
        return;
    }
    coll::grab_fighters(r, gobj);
    if super::item(r.game, gobj).grab_victim.is_none() {
        return;
    }
    let dealt = super::item(r.game, gobj)
        .grab_dealt
        .expect("an item armed to grab with no grab callback");
    call(r, gobj, dealt);
    let ip = super::item(r.game, gobj);
    let (victim, cb) = (
        ip.grab_victim.expect("a grab with no victim"),
        ip.grabbed_for_victim
            .expect("a grab with no victim callback"),
    );
    match cb.0 {
        super::kinds::tarucann::FT_BARREL_ENTER => r.enter_barrel(victim, gobj),
        a => code::unported_call(&format!("{a:#010x} (a grabbed fighter's callback)")),
    }
}

/// The item's hit process (priority 13), `Item_80269C5C`: its hitboxes against items' and
/// fighters' hurtboxes and hitboxes.
#[doc(alias = "Item_80269C5C")]
pub fn proc_hit(r: &mut Running, gobj: GObjId) {
    coll::clear_damage_log(r.game);
    coll::hit_by_fighters(r, gobj);
    coll::hit_by_items(r, gobj);
    coll::take_hits(r, gobj);
}

/// Adds damage the item took, up to 999, `Item_80269CA0`.
#[doc(alias = "Item_80269CA0")]
pub fn add_damage(ip: &mut Item, damage: i32) {
    ip.xC9C = ip.xC9C.wrapping_add(damage);
    if ip.xC9C > 999 {
        ip.xC9C = 999;
    }
}

/// Clears what this tick's hits recorded, `Item_80269CC4`.
#[doc(alias = "Item_80269CC4")]
pub fn clear_hit_record(ip: &mut Item) {
    ip.xCA0 = 0;
    ip.xCA4 = 0;
    ip.xC94 = 0;
    ip.xCC8_knockback = 0.0;
    ip.xC34_damageDealt = 0;
    ip.xC48 = 0;
    ip.xC4C = 0;
    ip.xC50 = 0;
    ip.xCCC_incDamageDirection = 0.0;
    ip.xCB8_outDamageDirection = 0.0;
    ip.xC68 = 0.0;
    ip.xCD0 = 0.0;
    ip.xCC0 = 0.0;
    set_flag(&mut ip.xDCF_flag, 6, false);
    ip.xCC4 = 0;
    ip.xCEC_fighterGObj = None;
    ip.xCF0_itemGObj = None;
    ip.xCF4_fighterGObjUnk = None;
    ip.toucher = None;
    ip.xC64_reflectGObj = None;
    ip.xC90_absorbGObj = None;
    ip.xCFC = None;
    ip.xCB4 = -1;
    ip.xC38 = -1;
    ip.xCE0 = Vec3::new(0.0, 0.0, 0.0);
    ip.xCD4 = Vec3::new(0.0, 0.0, 0.0);
    set_flag(&mut ip.xDCE_flag, 5, false);
    set_flag(&mut ip.xDCE_flag, 4, false);
    ip.xDCC_flag.b1 = false;
    ip.xC70 = 1.0;
    ip.xC6C = 1.0;
    ip.xDCC_flag.b2 = false;
    ip.xDC8_word.flags.xB = false;
    ip.xCA8 = 0;
    ip.xDC8_word.flags.x3 = false;
    ip.xDC8_word.flags.x4 = false;
    set_flag(&mut ip.xDCE_flag, 6, false);
}

/// Calls an event callback that can destroy the item (`processCallback`).
pub fn react_with(r: &mut Running, gobj: GObjId, cb: Option<CodeRef>) -> bool {
    if let Some(cb) = cb
        && call_pred(r, gobj, cb)
    {
        item_mut(r.game, gobj).destroy_type = 2;
        destroy(r, gobj);
        return true;
    }
    false
}

/// Bounces off or hits a shield, `Item_80269DC8`.
#[doc(alias = "Item_80269DC8")]
pub fn react_shield(r: &mut Running, gobj: GObjId) -> bool {
    let data = r.data;
    let degrees = common_data(r.game).f32(data, common::UNK_DEGREES);
    let ip = super::item(r.game, gobj);
    // `(f32) M_PI / 180 * (90 + degrees)`, single precision.
    let limit = fp::fmuls(
        fp::fdivs(fp::frsp(std::f64::consts::PI), 180.0),
        fp::fadds(90.0, reg(degrees)),
    );
    let bounce = flag(ip.xDCE_flag, 5)
        && ip.ground_or_air == 1
        && (flag(ip.xDCE_flag, 4) || reg(ip.xC54) < limit);
    if bounce {
        let ip = item_mut(r.game, gobj);
        if !flag(ip.xDCE_flag, 4) {
            ip.xC54 = mem(fp::frsp(fp::fsub(
                reg(ip.xC54),
                std::f64::consts::FRAC_PI_2,
            )));
            if reg(ip.xC54) < 0.0 {
                ip.xC54 = 0.0;
            }
        }
        let cb = logic(r.game, gobj).shield_bounced;
        react_with(r, gobj, cb)
    } else {
        let cb = logic(r.game, gobj).hit_shield;
        if lib2725::is_hit_counted(r.game, gobj) {
            let ip = item_mut(r.game, gobj);
            ip.xCA8 = ip.xC50;
        }
        react_with(r, gobj, cb)
    }
}

/// Changes hands on a reflection, `Item_80269F14`: the reflector owns the item, and its
/// hitboxes take the reflection's damage multiplier.
#[doc(alias = "Item_80269F14")]
pub fn react_reflect(r: &mut Running, gobj: GObjId) -> bool {
    let ip = super::item(r.game, gobj);
    if !ip.xDCC_flag.b1 && (ip.kind != M_BALL || !ip.xDCC_flag.b2) {
        let reflector = ip.xC64_reflectGObj;
        let team = super::owner::team(r.game, reflector);
        let ip = item_mut(r.game, gobj);
        ip.owner = reflector;
        ip.x20_team_id = team as u8;
    }
    let owner = super::item(r.game, gobj).owner;
    debug_owner_bubbles(r.game, gobj, owner);
    {
        let ip = item_mut(r.game, gobj);
        ip.xD90 = ip.xC74.to_be_bytes();
        ip.xD94 = ip.xC78.clone();
        ip.xD9C = ip.xC80.clone();
        ip.xDA4_word = ip.xC88 as u32;
        ip.xDA8_short = ip.xC8C;
    }
    let cb = logic(r.game, gobj).reflected;
    if react_with(r, gobj, cb) {
        return true;
    }
    if !super::item(r.game, gobj).xDCC_flag.b0 {
        let max = common_data(r.game).u32(r.data, common::XD8);
        for i in 0..4 {
            let ip = super::item(r.game, gobj);
            let hit = &ip.x5D4_hitboxes[i].hit;
            if hit.state != 0 {
                let d = fp::fmadds(reg(hit.damage), reg(ip.xC6C), f64::from(0.99_f32));
                let mut dmg = crate::ppc::cvt_fp2unsigned(d);
                if dmg > max {
                    dmg = max;
                }
                coll::set_hit_damage(r.game, r.data, gobj, i, dmg);
            }
        }
    }
    false
}

/// Enters hitlag if not in it (`EnterHitlagThink`).
pub fn enter_hitlag_if_free(r: &mut Running, gobj: GObjId) {
    if !super::item(r.game, gobj).xDC8_word.flags.x9 {
        enter_hitlag(r, gobj);
    }
}

/// Raises the hitlag to at least `frames` (`checkHitLag`).
pub fn raise_hitlag(ip: &mut Item, frames: f64) {
    if reg(ip.xCBC_hitlagFrames) < frames {
        ip.xCBC_hitlagFrames = mem(frames);
    }
    ip.xDC8_word.flags.xA = true;
}

/// The item's reaction process (priority 14), `Item_8026A294`: the kind's callbacks for what
/// this tick's hits did (damage taken, a shield, a clank, damage dealt, a reflection, an
/// absorption, a touch), hitlag, and clearing the record.
#[doc(alias = "Item_8026A294")]
pub fn proc_react(r: &mut Running, gobj: GObjId) {
    let ip = super::item(r.game, gobj);
    let destroyed = if reg(ip.xCC8_knockback) != 0.0 || ip.xCA0 != 0 {
        let ip = item_mut(r.game, gobj);
        add_damage(ip, ip.xCA0);
        ip.xCA8 = ip.xCA0;
        ip.xDC8_word.flags.xB = true;
        let cb = logic(r.game, gobj).dmg_received;
        react_with(r, gobj, cb)
    } else if ip.xC50 != 0 {
        react_shield(r, gobj)
    } else if ip.xC48 != 0 {
        let cb = logic(r.game, gobj).clanked;
        if lib2725::is_hit_counted(r.game, gobj) {
            let ip = item_mut(r.game, gobj);
            ip.xCA8 = ip.xC48;
        }
        react_with(r, gobj, cb)
    } else if ip.xC34_damageDealt != 0 || ip.xC4C != 0 {
        let cb = logic(r.game, gobj).dmg_dealt;
        if lib2725::is_hit_counted(r.game, gobj) {
            let ip = item_mut(r.game, gobj);
            ip.xCA8 = ip.xC34_damageDealt;
        }
        react_with(r, gobj, cb)
    } else if ip.xC64_reflectGObj.is_some() {
        react_reflect(r, gobj)
    } else if ip.xC90_absorbGObj.is_some() {
        let cb = logic(r.game, gobj).absorbed;
        react_with(r, gobj, cb)
    } else if flag(ip.xDCE_flag, 6) {
        let cb = ip.touched;
        react_with(r, gobj, cb)
    } else {
        false
    };
    if destroyed {
        return;
    }
    let ip = super::item(r.game, gobj);
    if ip.xDC8_word.flags.xD && reg(ip.xCC0) > 0.0 {
        let frames = reg(ip.xCC0);
        raise_hitlag(item_mut(r.game, gobj), frames);
        enter_hitlag_if_free(r, gobj);
    } else if ip.xCA8 != 0 {
        let frames = lib26b1::hitlag_frames(r.game, r.data, ip.xCA8);
        raise_hitlag(item_mut(r.game, gobj), frames);
        enter_hitlag_if_free(r, gobj);
    }
    let f = &super::item(r.game, gobj).xDC8_word.flags;
    if f.x4 || f.x3 {
        enter_hitlag_if_free(r, gobj);
        let ip = item_mut(r.game, gobj);
        if ip.xDC8_word.flags.x4 {
            ip.xDC8_word.flags.x8 = true;
            ip.xDC8_word.flags.x4 = false;
        } else {
            ip.xDC8_word.flags.x7 = true;
            ip.xDC8_word.flags.x3 = false;
        }
    }
    clear_hit_record(item_mut(r.game, gobj));
}

/// The item's dynamics process (priority 16), `Item_8026A788`.
#[doc(alias = "Item_8026A788")]
pub fn proc_dynamics(r: &mut Running, gobj: GObjId) {
    coll::update_dynamics_points(r.game, r.data, gobj);
    let ip = super::item(r.game, gobj);
    // The colliders are the item's dynamics points (`xB6C_t`, read as `lb_Collider`s).
    let count = usize::from(ip.xB68);
    let colliders: [Collider; 2] = std::array::from_fn(|k| Collider {
        radius: ip.xB6C_vars[k].xB78,
        center: ip.xB6C_vars[k].xB84,
    });
    let colliders = &colliders[..count];
    for i in 0..ip.x374_dynamicBonesNum.max(0) as usize {
        let b = &super::item(r.game, gobj).xD4_dynamicBones[i];
        let (desc, flags) = (b.dyn_desc.clone(), b.flags);
        r.game
            .swing_chain(r.data, &desc, colliders, None, BoneSlot::Id(flags), 0, true);
    }
}

/// Ends the hitlag a hit started, `Item_8026A0A0`, unless another hitlag holds it.
#[doc(alias = "Item_8026A0A0")]
pub fn end_hit_hitlag(r: &mut Running, gobj: GObjId) {
    let f = &item_mut(r.game, gobj).xDC8_word.flags;
    if f.x7 {
        if !f.xA && !f.x8 {
            exit_hitlag(r, gobj);
        }
        item_mut(r.game, gobj).xDC8_word.flags.x7 = false;
    }
}

/// Ends the hitlag a clank started, `Item_8026A0FC` (`func_8026A1E8_inline` inlines it too),
/// unless another hitlag holds it.
#[doc(alias = "Item_8026A0FC")]
pub fn end_clank_hitlag(r: &mut Running, gobj: GObjId) {
    let f = &item_mut(r.game, gobj).xDC8_word.flags;
    if f.x8 {
        if !f.xA && !f.x7 {
            exit_hitlag(r, gobj);
        }
        item_mut(r.game, gobj).xDC8_word.flags.x8 = false;
    }
}

/// Puts an item in hitlag, `Item_8026A158`, and the item it hit too (`func_8026A158_helper`).
#[doc(alias = "Item_8026A158")]
pub fn enter_hitlag(r: &mut Running, gobj: GObjId) {
    if let Some(cb) = item_mut(r.game, gobj).entered_hitlag {
        call(r, gobj, cb);
    }
    let ip = item_mut(r.game, gobj);
    ip.xDC8_word.flags.x9 = true;
    let (victim, x8) = (ip.atk_victim, ip.xDC8_word.flags.x8);
    if let Some(v) = victim
        && is_item(r.game, victim)
        && !x8
    {
        item_mut(r.game, v).xDC8_word.flags.x8 = true;
        enter_hitlag(r, v);
    }
}

/// Takes an item out of hitlag, `Item_8026A1E8`, and ends the hitlag of the item it hit.
#[doc(alias = "Item_8026A1E8")]
pub fn exit_hitlag(r: &mut Running, gobj: GObjId) {
    if let Some(cb) = item_mut(r.game, gobj).exited_hitlag {
        call(r, gobj, cb);
    }
    let ip = item_mut(r.game, gobj);
    ip.xDC8_word.flags.x9 = false;
    let (victim, x8) = (ip.atk_victim, ip.xDC8_word.flags.x8);
    if let Some(v) = victim
        && is_item(r.game, victim)
        && !x8
    {
        end_clank_hitlag(r, v);
    }
}

/// Whether a held special item of this kind is the fighter's held special item rather than
/// a held item (`DestroyItemInline` and `Item_8026A848`).
pub fn is_special_held(ip: &Item) -> bool {
    ip.hold_kind == 8
        && ip.kind != LINK_BOMB
        && ip.kind != CLINK_BOMB
        && ip.kind != PEACH_TURNIP
        && ip.kind != PEACH_PARASOL
}

/// Lets go of the item from a fighter's hand, `Item_8026A848`.
pub fn release_from(r: &mut Running, gobj: GObjId, fighter: GObjId) {
    super::owner::release(r, gobj, fighter, is_special_held(super::item(r.game, gobj)));
}

/// Destroys the item, `Item_8026A8EC`: its hitboxes, count, effects, destroy effect and
/// sound, the kind's `destroyed` callback, its holder's hand, its camera box, its sounds, and
/// its GObj.
#[doc(alias = "Item_8026A8EC")]
pub fn destroy(r: &mut Running, gobj: GObjId) {
    assert!(
        is_item(r.game, Some(gobj)),
        "===== Not Found Item_Struct!! ====="
    );
    lib2725::clear_hitboxes(r.game, r.data, gobj);
    uncount(r.game, gobj);
    // `func_8026A8EC_inline1`: a character item lets its character's data go.
    let ip = item_mut(r.game, gobj);
    if ip.xDC8_word.flags.xE {
        let k = ip.ecb_lock;
        ip.xDC8_word.flags.xE = false;
        ip.ecb_lock = -1;
        super::owner::dec_fighter_refs(r.game, k);
    }
    super::effect::destroy_all(r, gobj);
    let ip = super::item(r.game, gobj);
    if !flag(ip.xDCF_flag, 2) && (!ip.xDC8_word.flags.x13 || ip.owner.is_none()) {
        // `ItemSwitch`: the destroy effect and sound by how the item went.
        let a = attr_of(ip);
        let data = r.data;
        let (gfx, sfx) = match ip.destroy_type {
            0 => (Some(a.i32(data, attr::DESTROY_GFX)), ip.destroy_sfx),
            1 | 2 => (Some(a.i32(data, attr::X68)), ip.xD80),
            _ => (None, 0),
        };
        if let Some(gfx) = gfx {
            lib2725::destroy_effect(r, gobj, gfx, sfx);
        }
    }
    if let Some(cb) = logic(r.game, gobj).destroyed {
        call(r, gobj, cb);
    }
    let ip = super::item(r.game, gobj);
    if ip.xDC8_word.flags.x13 && is_fighter(r.game, ip.owner) {
        let owner = ip.owner.unwrap();
        let special = is_special_held(ip);
        super::owner::release(r, gobj, owner, special);
    }
    item_mut(r.game, gobj).owner = None;
    // `func_8026A8EC_inline3`: the camera box.
    remove_camera_box(r.game, gobj);
    let i = item_of(r.game, gobj);
    for k in 0..r.game.items[i.0].x374_dynamicBonesNum.max(0) as usize {
        let mut desc = r.game.items[i.0].xD4_dynamicBones[k].dyn_desc.clone();
        r.game.free_dynamics_chain(&mut desc);
        r.game.items[i.0].xD4_dynamicBones[k].dyn_desc = desc;
    }
    stop_all_sfx(r, gobj);
    lib26b1::clear_effect_queue(r.game, gobj);
    gobj::free(r, gobj);
}

/// Frees the item's camera box, `Item_80267454` (`func_8026A8EC_inline3` inlines it).
#[doc(alias = "Item_80267454")]
pub fn remove_camera_box(game: &mut Game, gobj: GObjId) {
    let ip = item_mut(game, gobj);
    if ip.xDCD_flag.x0.b01 != 0
        && let Some(b) = ip.x520_cameraBox
    {
        ip.x520_cameraBox = None;
        ip.xDCD_flag.x0.b01 = 0;
        camera::free_box(&mut game.camera.boxes, b);
    }
}

/// A new attack id, `Item_8026AE60`: never 0.
#[doc(alias = "Item_8026AE60")]
pub fn next_attack_id(game: &mut Game) -> u32 {
    let id = game.it.next_attack_id;
    game.it.next_attack_id = id.wrapping_add(1);
    if game.it.next_attack_id == 0 {
        game.it.next_attack_id = 1;
    }
    id
}

/// The silent sound id, and the id that stops a slot's sound instead.
const SFX_SILENT: i32 = 0x83D60;
const SFX_STOP: i32 = 0x83D61;

/// A sound slot without a sound (the audio driver's handles are never -1).
pub const NO_SFX: i32 = -1;

/// Plays a sound the item keeps only in `xD6C`, `Item_8026AE84`. (The original passes its
/// pan and volume to `lbAudioAx_800237A8` as volume and pan.)
#[doc(alias = "Item_8026AE84")]
pub fn play_sfx(r: &mut Running, gobj: GObjId, sfx: i32, pan: u8, volume: u8) {
    if sfx == SFX_SILENT {
        return;
    }
    let id = if sfx != SFX_STOP {
        r.game.audio.lb.kirby_voice(sfx)
    } else {
        SFX_SILENT
    };
    let h = r.sfx_play(id, pan.into(), volume.into());
    item_mut(r.game, gobj).xD6C = h;
}

/// Plays a sound in the item's first slot, `Item_8026AF0C`, stopping the one there.
#[doc(alias = "Item_8026AF0C")]
pub fn play_sfx1(r: &mut Running, gobj: GObjId, sfx: i32, pan: u8, volume: u8) {
    if sfx == SFX_SILENT {
        return;
    }
    if sfx == SFX_STOP {
        stop_sfx1(r, gobj);
        return;
    }
    let old = super::item(r.game, gobj).sfx_unk1;
    if old != NO_SFX {
        r.sfx_stop(old);
    }
    let id = r.game.audio.lb.kirby_voice(sfx);
    let h = r.sfx_play(id, pan.into(), volume.into());
    item_mut(r.game, gobj).sfx_unk1 = h;
}

/// Plays a sound in the item's second slot, `Item_8026AFA0`, stopping the one there.
#[doc(alias = "Item_8026AFA0")]
pub fn play_sfx2(r: &mut Running, gobj: GObjId, sfx: i32, pan: u8, volume: u8) {
    if sfx == SFX_SILENT {
        return;
    }
    if sfx == SFX_STOP {
        stop_sfx2(r, gobj);
        return;
    }
    let old = super::item(r.game, gobj).sfx_unk2;
    if old != NO_SFX {
        r.sfx_stop(old);
    }
    let id = r.game.audio.lb.kirby_voice(sfx);
    let h = r.sfx_play(id, pan.into(), volume.into());
    item_mut(r.game, gobj).sfx_unk2 = h;
}

/// Stops the sound in the item's first slot, `Item_8026B034`.
#[doc(alias = "Item_8026B034")]
pub fn stop_sfx1(r: &mut Running, gobj: GObjId) {
    let old = super::item(r.game, gobj).sfx_unk1;
    if old != NO_SFX {
        r.sfx_stop(old);
    }
    item_mut(r.game, gobj).sfx_unk1 = NO_SFX;
}

/// Stops the sound in the item's second slot, `Item_8026B074`.
#[doc(alias = "Item_8026B074")]
pub fn stop_sfx2(r: &mut Running, gobj: GObjId) {
    let old = super::item(r.game, gobj).sfx_unk2;
    if old != NO_SFX {
        r.sfx_stop(old);
    }
    item_mut(r.game, gobj).sfx_unk2 = NO_SFX;
}

/// Stops every sound the item made, `Item_8026B0B4`: the sound objects it owns, then its
/// three slots (a slot a sound object held is stopped there, the others by handle).
#[doc(alias = "Item_8026B0B4")]
pub fn stop_all_sfx(r: &mut Running, gobj: GObjId) {
    r.lb_stop_owned_sound_objects(gobj);
    let xd6c = super::item(r.game, gobj).xD6C;
    if xd6c != NO_SFX {
        r.lb_stop_owned_sound_object(gobj, xd6c);
        item_mut(r.game, gobj).xD6C = NO_SFX;
    }
    for second in [false, true] {
        let ip = super::item(r.game, gobj);
        let h = if second { ip.sfx_unk2 } else { ip.sfx_unk1 };
        if h != NO_SFX && r.lb_stop_owned_sound_object(gobj, h) {
            r.sfx_stop(h);
            let ip = item_mut(r.game, gobj);
            if second {
                ip.sfx_unk2 = NO_SFX;
            } else {
                ip.sfx_unk1 = NO_SFX;
            }
        }
    }
    for second in [false, true] {
        let ip = super::item(r.game, gobj);
        let h = if second { ip.sfx_unk2 } else { ip.sfx_unk1 };
        if h != NO_SFX {
            r.sfx_stop(h);
        }
        let ip = item_mut(r.game, gobj);
        if second {
            ip.sfx_unk2 = NO_SFX;
        } else {
            ip.sfx_unk1 = NO_SFX;
        }
    }
    item_mut(r.game, gobj).xD6C = NO_SFX;
}

/// A fighter takes the item in hand at its part `part`, `Item_8026AB54`: the pickup sound, the
/// item held, and the kind's `picked_up` callback.
#[doc(alias = "Item_8026AB54")]
pub fn pick_up(r: &mut Running, gobj: GObjId, holder: GObjId, part: i32) {
    lib2725::pickup_sfx(r, gobj);
    hold(r, gobj, holder, part);
    if let Some(cb) = logic(r.game, gobj).picked_up {
        call(r, gobj, cb);
    }
    stop_sfx2(r, gobj);
}

/// Puts the item in a fighter's hand, `it_802742F4`: its model reset, its holder and team
/// set, shown, and attached to the fighter's part.
#[doc(alias = "it_802742F4")]
pub fn hold(r: &mut Running, gobj: GObjId, holder: GObjId, part: i32) {
    let data = r.data;
    let root = jobj_of(r.game, gobj);
    let joint = super::item(r.game, gobj).xC8_joint;
    crate::game::gr::anime::reset_tree_srt(&mut r.game.objs, data, root, joint);
    apply_scale(r.game, gobj);
    if is_fighter(r.game, Some(holder)) {
        let f = super::owner::fighter_of(r.game, holder);
        let (slot, team) = {
            let fp = &r.game.fighters[f.0];
            (fp.player_idx, fp.team)
        };
        let ip = item_mut(r.game, gobj);
        ip.xDC8_word.flags.xF = (ip.xDC8_word.flags.xF | 1 << slot) & 0xF;
        if !ip.xDC8_word.flags.x0 {
            ip.owner = Some(holder);
        } else {
            ip.x51C = Some(holder);
        }
        ip.xDC4 = part as u32;
        ip.x20_team_id = team;
        ip.xD50_landNum = 0;
        ip.xDC8_word.flags.x13 = true;
        let child = root.and_then(|j| r.game.objs.jobjs[j].child);
        jobj::clear_flags_all(&mut r.game.objs.jobjs, child, jobj::HIDDEN);
        super::hitbox::start_blink(r.game, gobj);
        lib26b1::disable_pickup(r.game, gobj);
        debug_owner_bubbles(r.game, gobj, Some(holder));
    }
    let ip = item_mut(r.game, gobj);
    if flag(ip.xDD0_flag, 6) {
        set_flag(&mut ip.xDD0_flag, 6, false);
        ip.xD40 = 0.0;
        super::anim::clear_overlay(ip);
    }
    if super::item(r.game, gobj).kind < L_GUN_RAY {
        let lifetime = common_data(r.game).u32(data, common::LIFETIME);
        lib2725::set_lifetime(r.game, data, gobj, mem(fp::frsp(f64::from(lifetime))));
    }
    let bone = article(super::item(r.game, gobj))
        .model(data)
        .bone_attach_id(data);
    lib2725::attach_to_part(r.game, data, gobj, bone, holder, part);
    lib2725::clear_stomp(r.game, gobj);
}

/// Whether a fighter can pick the item up, `Item_IsGrabbable`.
#[doc(alias = "Item_IsGrabbable")]
pub fn is_grabbable(game: &Game, gobj: GObjId) -> bool {
    super::item(game, gobj).xDC8_word.flags.x15 && logic(game, gobj).picked_up.is_some()
}

/// Takes the items' common data from `ItCo.usd`, `Item_80266F70` (`it_8027870C`, whose
/// argument goes unread): the scene preloads it, so nothing loads from the disc; its
/// `itPublicData` points to the common data, the articles, the color animations and the
/// Pokémon's.
#[doc(alias = "Item_80266F70")]
#[doc(alias = "it_8027870C")]
///
/// `kept` says the file was set up by an earlier scene and stayed in memory (the preload
/// cache's): its table of character articles then holds what `it_8026B3F8` wrote into it since.
pub fn load_common_data(
    game: &mut Game,
    data: &Archives,
    it_co: melee_data::archive::ArchiveId,
    kept: bool,
) {
    let public = data
        .get(it_co)
        .public("itPublicData")
        .map(|offset| DataRef {
            archive: it_co,
            offset,
        })
        .expect("Cannot find symbol itPublicData.");
    let it = &mut game.it;
    it.common = data.ptr(public);
    it.common_articles = data.ptr(public.at(4));
    let table = data.ptr(public.at(8));
    let kept = kept && it.char_articles.len() == super::CHAR_ARTICLES;
    if !kept {
        it.char_articles = match table {
            Some(t) => (0..super::CHAR_ARTICLES as u32)
                .map(|k| data.ptr(t.at(4 * k)))
                .collect(),
            None => Vec::new(),
        };
    }
    it.pokemon_articles = data.ptr(public.at(0xC));
    it.zako = data.ptr(public.at(0x10));
    it.colanims = data.ptr(public.at(0x14));
}

/// Resets the item code's globals for a match, `Item_80266FCC`: empty item and joint-table
/// pools (the item links' pool, `item_link_alloc_data`, isn't modeled: no ported kind links),
/// no items out with the common data's limits, and empty spawn tables.
#[doc(alias = "Item_80266FCC")]
pub fn init_globals(game: &mut Game, data: &Archives) {
    {
        use crate::hsd::heap::Kind;
        crate::hsd::heap::init_pools(&[Kind::Item, Kind::ItemDynamicBones, Kind::ItemLink]);
    }
    game.items = Default::default();
    game.it.bone_tables = Default::default();
    let c = common_data(game);
    let w = |off: u32| c.i32(data, off);
    let n = &mut game.it.counts;
    n.x0 = 0;
    n.x8 = 0;
    n.x10 = 0;
    n.x18 = 0;
    n.x1C = 0;
    n.x24 = 0;
    n.x2C = 0;
    n.x34 = 0;
    n.x3C = 0;
    n.x40 = 0;
    n.x48 = 0;
    n.x50 = 0;
    n.x58 = 0;
    n.x60 = 0;
    n.x4 = w(0);
    n.xC = w(4);
    n.x14 = w(8);
    n.x20 = w(0xC);
    n.x28 = w(0x10);
    n.x30 = w(0x14);
    n.x38 = w(0x18);
    n.x44 = w(0x1C);
    n.x4C = w(0x24);
    n.x54 = w(0x20);
    n.x5C = w(0x28);
    n.x64 = w(0x148);
    game.it.ft_track.x154[0] |= 0x80;
    game.it.ft_track.count = 1;
    game.it.pokemon.last_kind = UNSELECTED;
    game.it.pokemon.previous_kind = UNSELECTED;
    game.it.pokemon.rare_spawned = 0;
    game.it.x6d00 = -1;
    game.it.next_attack_id = 1;
    game.it.made = 0;
    game.it.spawned = 0;
    game.it.x6d08 = 0;
    let s = &mut game.it.spawn;
    for t in [&mut s.monsters, &mut s.no_containers, &mut s.spawner.table] {
        t.total = 0;
    }
    for t in [&mut s.monsters, &mut s.no_containers, &mut s.spawner.table] {
        t.size = 0;
    }
}

/// `It_Kind_Unselected`.
const UNSELECTED: i32 = -1;

/// Hands the item to its kind's `entered_air` callback, if it has one, and marks it
/// skidding, `Item_8026ADC0`.
#[doc(alias = "Item_8026ADC0")]
pub fn enter_air_state(r: &mut Running, gobj: GObjId) {
    if let Some(cb) = logic(r.game, gobj).entered_air {
        call(r, gobj, cb);
        item_mut(r.game, gobj).xDC8_word.flags.x1F = true;
    }
}
