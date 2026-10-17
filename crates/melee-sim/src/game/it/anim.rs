//! Item scripts and color animations, `melee/it/itanimlist.c`: each state's command script
//! (hitboxes, sounds, effects, rumble and the item's own variables), run each frame against its
//! animation, and the item's color overlay.

use gekko_fp as fp;
use melee_data::archive::{Archives, DataRef};

use super::{item, item_mut, jobj_of, set_flag};
use crate::fpu::{mem, reg};
use crate::game::code;
use crate::game::state::{Game, Running};
use crate::game::structs::{ColorOverlay, Item};
use crate::hsd::gobj::GObjId;
use crate::lb::command::{self, WAIT_FOR_FRAME};
use crate::lb::hitcapsule;
use crate::vec::Vec3;

/// `0.003906f`, the scale of a command's fixed-point sizes and offsets.
const FIXED: f32 = 0.003906;

/// A command word's unsigned field of `bits` bits, `at` bits from its top.
pub fn field(w: u32, at: u32, bits: u32) -> u32 {
    w >> (32 - at - bits) & ((1u64 << bits) - 1) as u32
}

/// A signed field likewise.
pub fn sfield(w: u32, at: u32, bits: u32) -> i32 {
    ((w << at) as i32) >> (32 - bits)
}

/// A fixed-point command value as a float.
pub fn fixed(v: i32) -> f32 {
    mem(fp::fmuls(f64::from(FIXED), fp::frsp(f64::from(v))))
}

/// The word at the item's script cursor, `n` words on.
pub fn word(r: &Running, gobj: GObjId, n: u32) -> u32 {
    let s = &item(r.game, gobj).x524_cmd.script;
    s.word_at(r.data, n)
}

/// Moves the item's script on `n` words.
pub fn skip(r: &mut Running, gobj: GObjId, n: u32) {
    item_mut(r.game, gobj).x524_cmd.script.skip(n);
}

/// Makes or updates a hitbox from the script, `it_802790C0`.
#[doc(alias = "it_802790C0")]
pub fn create_hitbox(r: &mut Running, gobj: GObjId) {
    let data = r.data;
    let w0 = word(r, gobj, 0);
    let idx = field(w0, 6, 3) as usize;
    let group = field(w0, 9, 3);
    {
        let ip = item_mut(r.game, gobj);
        let hit = &mut ip.x5D4_hitboxes[idx].hit;
        if hit.state == hitcapsule::DISABLED || hit.x4 != group {
            hit.x4 = group;
            hit.state = hitcapsule::ENABLED;
            ip.xDC8_word.flags.x16 = true;
            set_flag(&mut ip.xDAA, 2, false);
            super::coll::share_group_victims(r.game, gobj, idx);
        }
    }
    let bone = field(w0, 12, 7);
    let j = if bone != 0 {
        let t = item(r.game, gobj)
            .xBBC_dynamicBoneTable
            .expect("item can't set attack!");
        r.game.it.bone_tables[t.0].bones[bone as usize]
    } else {
        jobj_of(r.game, gobj)
    };
    item_mut(r.game, gobj).x5D4_hitboxes[idx].hit.jobj.set(j);
    let ip = item(r.game, gobj);
    let dmg = fp::fmuls(
        reg(ip.xC3C),
        fp::fmuls(fp::frsp(f64::from(field(w0, 19, 13))), reg(ip.xC40)),
    );
    super::coll::set_hit_damage(r.game, data, gobj, idx, crate::ppc::cvt_fp2unsigned(dmg));
    skip(r, gobj, 1);
    let w1 = word(r, gobj, 0);
    {
        let ip = item_mut(r.game, gobj);
        let hit = &mut ip.x5D4_hitboxes[idx].hit;
        hit.scale = fixed(field(w1, 0, 16) as i32);
        ip.x3C = ip.x5D4_hitboxes[idx].hit.scale;
    }
    let inv = fp::fdivs(1.0, reg(item(r.game, gobj).scl));
    super::hitbox::scale_hitbox(r.game, gobj, idx, inv);
    item_mut(r.game, gobj).x5D4_hitboxes[idx].hit.b_offset.x = fixed(sfield(w1, 16, 16));
    skip(r, gobj, 1);
    let w2 = word(r, gobj, 0);
    {
        let hit = &mut item_mut(r.game, gobj).x5D4_hitboxes[idx].hit;
        hit.b_offset.y = fixed(sfield(w2, 0, 16));
        hit.b_offset.z = fixed(sfield(w2, 16, 16));
    }
    skip(r, gobj, 1);
    let w3 = word(r, gobj, 0);
    {
        let hit = &mut item_mut(r.game, gobj).x5D4_hitboxes[idx].hit;
        hit.kb_angle = field(w3, 0, 9) as i32;
        hit.x24 = field(w3, 9, 9);
        hit.x28 = field(w3, 18, 9);
        hit.x43_b1 = false;
    }
    skip(r, gobj, 1);
    let w4 = word(r, gobj, 0);
    {
        let hit = &mut item_mut(r.game, gobj).x5D4_hitboxes[idx].hit;
        hit.x2C = field(w4, 0, 9);
        hit.element = field(w4, 9, 5);
        hit.x40_b0 = field(w4, 14, 1) != 0;
        hit.x40_b1 = false;
        hit.x34 = sfield(w4, 15, 8);
        hit.sfx_severity = field(w4, 23, 3) as i32;
        hit.sfx_kind = field(w4, 26, 4) as i32;
        hit.x40_b2 = field(w4, 31, 1) != 0;
        hit.x40_b3 = field(w4, 30, 1) != 0;
    }
    skip(r, gobj, 1);
    let w5 = word(r, gobj, 0);
    {
        let ip = item_mut(r.game, gobj);
        let hb = &mut ip.x5D4_hitboxes[idx];
        let hit = &mut hb.hit;
        hit.x40_b4 = field(w5, 0, 8) as u8;
        hit.x41_b4 = field(w5, 8, 1) != 0;
        hit.x41_b5 = field(w5, 9, 1) != 0;
        hit.x41_b6 = field(w5, 10, 1) != 0;
        hit.x41_b7 = field(w5, 11, 1) != 0;
        hit.x42_b0 = field(w5, 12, 1) != 0;
        hit.x42_b1 = field(w5, 13, 1) != 0;
        hit.x42_b2 = field(w5, 14, 1) != 0;
        hit.x42_b3 = field(w5, 15, 1) != 0;
        let b2 = (w5 >> 8) as u8;
        hit.x42_b4 = b2 >> 7 & 1 != 0;
        hit.x42_b5 = b2 >> 6 & 1 != 0;
        hit.x42_b6 = b2 >> 5 & 1 != 0;
        hit.x42_b7 = b2 >> 4 & 1 != 0;
        hit.x43_b0 = b2 >> 3 & 1 != 0;
        hb.x138 = b2 >> 2 & 1 != 0;
    }
    skip(r, gobj, 1);
    item_mut(r.game, gobj).x5D4_hitboxes[idx].hit.x43_b2 = false;
    let w = &r.game.world;
    if let Some(p) = w.current_proc()
        && w.procs[p.0].s_link > 11
    {
        super::coll::update_hitbox(r.game, data, gobj, idx);
    }
}

/// Sets a hitbox's damage from the script, `it_80279544`.
#[doc(alias = "it_80279544")]
pub fn set_hitbox_damage(r: &mut Running, gobj: GObjId) {
    let w = word(r, gobj, 0);
    let idx = field(w, 6, 3) as usize;
    let val = w & 0x1FFF;
    let ip = item(r.game, gobj);
    let dmg = fp::fmuls(
        reg(ip.xC3C),
        fp::fmuls(fp::frsp(f64::from(val)), reg(ip.xC40)),
    );
    super::coll::set_hit_damage(r.game, r.data, gobj, idx, crate::ppc::cvt_fp2unsigned(dmg));
    skip(r, gobj, 1);
}

/// Sets a hitbox's size from the script, `it_802795EC`.
pub fn set_hitbox_scale(r: &mut Running, gobj: GObjId) {
    let w = word(r, gobj, 0);
    let idx = field(w, 6, 3) as usize;
    {
        let ip = item_mut(r.game, gobj);
        ip.x5D4_hitboxes[idx].hit.scale = fixed(field(w, 9, 23) as i32);
        ip.x3C = ip.x5D4_hitboxes[idx].hit.scale;
    }
    let inv = fp::fdivs(1.0, reg(item(r.game, gobj).scl));
    super::hitbox::scale_hitbox(r.game, gobj, idx, inv);
    skip(r, gobj, 1);
}

/// A sound command, `it_8027978C`: plays in one of the item's slots, or stops one.
pub fn sound(r: &mut Running, gobj: GObjId) {
    let w = word(r, gobj, 0);
    let op = w >> 18 & 0xFF;
    skip(r, gobj, 1);
    match op {
        0..=2 => {
            let id = word(r, gobj, 0) as i32;
            skip(r, gobj, 1);
            let b = word(r, gobj, 0).to_be_bytes();
            let (pan, volume) = (b[2], b[3]);
            let f = [
                super::item::play_sfx,
                super::item::play_sfx1,
                super::item::play_sfx2,
            ];
            f[op as usize](r, gobj, id, pan, volume);
        }
        10 | 11 => {
            skip(r, gobj, 1);
            if op == 10 {
                super::item::stop_sfx1(r, gobj);
            } else {
                super::item::stop_sfx2(r, gobj);
            }
        }
        _ => {}
    }
    skip(r, gobj, 1);
}

/// Queues an effect from the script, `it_80278F2C`: its bone, id, and offset and spread in
/// 1/256ths (`it_80278800`).
#[doc(alias = "it_80278F2C")]
pub fn script_effect(r: &mut Running, gobj: GObjId) {
    let w: [u32; 5] = std::array::from_fn(|i| word(r, gobj, i as u32));
    let hi = |v: u32| (v >> 16) as u16;
    let lo = |v: u32| v as u16;
    let bone = i32::from(hi(w[0]) & 0x3FF);
    let ef_id = i32::from(hi(w[1]));
    let unit = |h: u16| {
        mem(fp::fmuls(
            f64::from(0.003906_f32),
            fp::frsp(f64::from(h as i16)),
        ))
    };
    let offset = Vec3::new(unit(hi(w[2])), unit(lo(w[2])), unit(hi(w[3])));
    let spread = Vec3::new(unit(lo(w[3])), unit(hi(w[4])), unit(lo(w[4])));
    skip(r, gobj, 5);
    super::effect::spawn(r, gobj, ef_id, bone, offset, spread, false);
}

/// Runs one of the items' own script commands (`it_803F22A8`, by opcode from 10).
pub fn run_command(r: &mut Running, gobj: GObjId, opcode: u32) {
    match opcode {
        10 => script_effect(r, gobj),
        11 => create_hitbox(r, gobj),
        12 => set_hitbox_damage(r, gobj),
        13 => set_hitbox_scale(r, gobj),
        // it_80279680: one hitbox off.
        14 => {
            let idx = (word(r, gobj, 0) & 0x3FF_FFFF) as usize;
            super::lib2725::clear_hitbox(r.game, r.data, gobj, idx);
            skip(r, gobj, 1);
        }
        // it_802796C4: every hitbox off.
        15 => {
            super::lib2725::clear_hitboxes(r.game, r.data, gobj);
            skip(r, gobj, 1);
        }
        16 => sound(r, gobj),
        // it_802796FC, it_80279720 and it_80279744: the item's script variables.
        17..=19 => {
            let v = word(r, gobj, 0) & 0x3FF_FFFF;
            let ip = item_mut(r.game, gobj);
            match opcode {
                17 => ip.xDAC_itcmd_var0 = v,
                18 => ip.xDB0_itcmd_var1 = v,
                _ => ip.xDB4_itcmd_var2 = v,
            }
            skip(r, gobj, 1);
        }
        // it_80279768: the script's flag for this frame.
        20 => {
            item_mut(r.game, gobj).xDBC[0] |= 0x80;
            skip(r, gobj, 1);
        }
        // it_80279888, it_802798D4 and it_8027990C: rumble (13-bit kind and frames).
        21..=23 => {
            let w = word(r, gobj, 0);
            let (kind, frames) = ((w >> 13 & 0x1FFF) as i32, (w & 0x1FFF) as i32);
            match opcode {
                21 => super::lib2725::rumble_owner(r, gobj, kind, frames),
                22 => super::lib2725::stop_owner_rumble(r, gobj),
                _ => super::lib2725::rumble_all(r, kind, frames),
            }
            skip(r, gobj, 1);
        }
        // it_80279958: a color animation (8-bit id, 18-bit frames).
        24 => {
            let w = word(r, gobj, 0);
            let (id, frames) = ((w >> 18 & 0xFF) as i32, (w & 0x3_FFFF) as i32);
            start_colanim(r.game, r.data, gobj, id, frames);
            skip(r, gobj, 1);
        }
        // it_802799A8: the color animation off.
        25 => {
            clear_overlay(item_mut(r.game, gobj));
            skip(r, gobj, 1);
        }
        _ => panic!("item script command {opcode} isn't in it_803F22A8"),
    }
}

/// Runs the item's script up to this frame of its animation, `it_802799E4`.
#[doc(alias = "it_802799E4")]
pub fn run_script(r: &mut Running, gobj: GObjId) {
    {
        let ip = item_mut(r.game, gobj);
        ip.x524_cmd.script.frame_count = ip.x5CC_currentAnimFrame.to_bits();
        ip.xDBC = [0; 4];
        if ip.x524_cmd.script.cursor.is_none() {
            return;
        }
        let s = &mut ip.x524_cmd.script;
        if reg(s.timer_f32()) != reg(WAIT_FOR_FRAME) {
            s.set_timer_f32(mem(fp::fsubs(
                reg(s.timer_f32()),
                reg(ip.x5D0_animFrameSpeed),
            )));
        }
    }
    loop {
        let ip = item_mut(r.game, gobj);
        let speed = reg(ip.x5D0_animFrameSpeed);
        let s = &mut ip.x524_cmd.script;
        if s.cursor.is_none() {
            return;
        }
        if reg(s.timer_f32()) == reg(WAIT_FOR_FRAME) {
            if reg(s.frame_count_f32()) >= speed {
                return;
            }
            s.set_timer_f32(mem(fp::fneg(reg(s.frame_count_f32()))));
        } else if reg(s.timer_f32()) > 0.0 {
            return;
        }
        let opcode = s.opcode(r.data);
        if !command::execute(s, r.data, opcode) {
            run_command(r, gobj, opcode);
        }
    }
}

/// Clears the item's color overlay, `it_80279B64` and `it_80279BBC` (`lb_80014498`).
#[doc(alias = "it_80279B64")]
#[doc(alias = "it_80279BBC")]
pub fn clear_overlay(ip: &mut Item) {
    crate::lb::colanim::clear(&mut ip.x548_colorOverlay);
}

/// The items' own color script commands (`fn_80279B30`, through `it_804D51C8`).
pub fn overlay_command(_data: &Archives, _co: &mut ColorOverlay, opcode: u32) {
    match opcode {
        21 => code::unported_call("it_80279AF0 (an item color script's effect)"),
        _ => code::unported_call("it_80279B10 (an item color script's sound)"),
    }
}

/// Runs the item's color overlay for this frame, clearing it whenever it ends,
/// `it_80279BE0`.
#[doc(alias = "it_80279BE0")]
pub fn run_overlay(r: &mut Running, gobj: GObjId) {
    let data = r.data;
    loop {
        let co = &mut item_mut(r.game, gobj).x548_colorOverlay;
        let ended = crate::lb::colanim::step(data, co, &mut |co, op| overlay_command(data, co, op));
        if !ended {
            return;
        }
        clear_overlay(item_mut(r.game, gobj));
    }
}

/// A script's start in an item state description (`ItemStateDesc::xC_script`), number
/// `index` of the item's article: for the random stage item, the stage's own where it gives
/// one (`Ground_801C0800` writes them into the article's descriptions).
pub fn state_script(
    game: &crate::game::state::Game,
    data: &Archives,
    kind: i32,
    index: i32,
    desc: DataRef,
) -> Option<DataRef> {
    if kind == super::kinds::PKIND_RANDOM
        && let Some(&(_, s)) = game
            .gr
            .yaku_scripts
            .iter()
            .find(|&&(i, _)| i32::from(i) == index)
    {
        return Some(s);
    }
    data.ptr(desc.at(0xC))
}

/// Starts the item color animation `id` (`it_804D6D04`'s) on the item's overlay, unless one
/// of higher priority plays, `it_80279B88`.
#[doc(alias = "it_80279B88")]
pub fn start_colanim(game: &mut Game, data: &Archives, gobj: GObjId, id: i32, frames: i32) -> bool {
    let table = game.it.colanims.expect("no item color animations");
    let co = &mut item_mut(game, gobj).x548_colorOverlay;
    crate::lb::colanim::start(co, data, table, id, frames)
}
