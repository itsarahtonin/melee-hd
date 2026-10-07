//! What the items read of fighters: their teams, positions and scale, the stale-move table
//! their owner's attacks weaken by, and the fighter-side bookkeeping items do (the count of
//! objects using a character's data, the fighters the items keep clear of). The fighter core
//! is ported separately; these read the generated `Fighter` directly.

use gekko_fp as fp;
use melee_data::archive::Archives;

use crate::fpu::{mem, reg};
use crate::game::code;
use crate::game::handles::FighterId;
use crate::game::state::{Game, Running, UserData};
use crate::game::structs::{_itECB, Fighter};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// The fighters' process link (`HSD_GOBJ_PLINK_FIGHTER`).
pub const P_LINK_FIGHTER: u8 = 8;

/// A fighter GObj's `Fighter`.
pub fn fighter_of(game: &Game, gobj: GObjId) -> FighterId {
    match game.world.gobjs[gobj.0].user_data {
        Some(UserData::Fighter(f)) => f,
        other => panic!("GObj {gobj:?} holds {other:?}, not a fighter"),
    }
}

/// A fighter by its GObj.
pub fn fighter(game: &Game, gobj: GObjId) -> &Fighter {
    &game.fighters[fighter_of(game, gobj).0]
}

/// A fighter's team, `ftLib_GetTeam`.
#[doc(alias = "ftLib_GetTeam")]
pub fn team(game: &Game, gobj: Option<GObjId>) -> i32 {
    i32::from(fighter(game, gobj.expect("no fighter")).team)
}

/// The center of a fighter's ECB, `ftLib_GetECBCenter`.
#[doc(alias = "ftLib_GetECBCenter")]
pub fn ecb_center(game: &Game, gobj: GObjId) -> Vec3 {
    let fp = fighter(game, gobj);
    let e = &fp.coll_data.ecb;
    let dy = fp::fmuls(0.5, fp::fadds(reg(e.top.y), reg(e.bottom.y)));
    let p = fp.cur_pos;
    Vec3::new(
        mem(fp::fadds(reg(p.x), fp::frsp(0.0))),
        mem(fp::fadds(reg(p.y), dy)),
        mem(fp::fadds(reg(p.z), fp::frsp(0.0))),
    )
}

/// Counts an object in that needs a character's data loaded, `ftLib_IncFighterRefCount`
/// (`ftData_80085560` with 1).
#[doc(alias = "ftLib_IncFighterRefCount")]
pub fn inc_fighter_refs(game: &mut Game, kind: i32) {
    let r = &mut game.ft.data.ref_counts[kind as usize];
    *r = r.wrapping_add(1);
}

/// Counts such an object out, `ftLib_DecFighterRefCount`; the original asserts the count stays
/// at or above zero (`ftData_80085560`).
pub fn dec_fighter_refs(game: &mut Game, kind: i32) {
    let r = &mut game.ft.data.ref_counts[kind as usize];
    *r = r.wrapping_sub(1);
    assert!(*r >= 0, "fighter reference counter error!");
}

/// Scales knockback or damage by a fighter's height scale, `ftCo_CalcYScaledKnockback`.
#[doc(alias = "ftCo_CalcYScaledKnockback")]
pub fn y_scaled(arg0: f64, scale: f64, arg2: f64) -> f64 {
    assert!(scale != 0.0, "scale != 0.0F");
    if arg2 == 0.0 {
        return arg0;
    }
    if arg2 < 0.0 {
        return fp::fdivs(arg0, y_scaled(1.0, scale, fp::fneg(arg2)));
    }
    if scale >= 1.0 || arg2 <= 1.0 {
        return fp::fmadds(arg2, fp::fmuls(fp::fsubs(scale, 1.0), arg0), arg0);
    }
    fp::fdivs(fp::fmuls(arg0, scale), arg2)
}

/// How much a move has gone stale for a player, `ft_80089118`: 1, less each weight of
/// `Fighter_804D6548` for each of its recent uses.
#[doc(alias = "ft_80089118")]
pub fn stale_mul(game: &Game, data: &Archives, slot: u8, move_id: i32) -> f64 {
    let table = &game.player(i32::from(slot)).stale_moves;
    let mut mul = 1.0;
    let mut k = if table.current_index != 0 {
        table.current_index.wrapping_sub(1)
    } else {
        9
    };
    if move_id == 1 {
        return 1.0;
    }
    let weights = crate::game::ft::common_ptr(game, data, crate::game::ft::STALE_WEIGHTS)
        .expect("no stale-move weights");
    for i in 0..9 {
        let m = i32::from(table.StaleMoves[k as usize].move_id);
        if m == 0 {
            return mul;
        }
        if move_id == m {
            mul = fp::fsubs(mul, reg(data.f32(weights.at(4 * i))));
        }
        k = if k != 0 { k - 1 } else { 9 };
    }
    mul
}

/// A hit's damage, weakened by how stale the attack is for the fighter, `ft_80089228`.
#[doc(alias = "ft_80089228")]
pub fn stale_damage(game: &Game, data: &Archives, f: FighterId, attack: i32, damage: f64) -> f64 {
    if game.gm.db_level >= crate::game::gm::DB_DEBUG_ROM {
        return damage;
    }
    let slot = game.fighters[f.0].player_idx;
    let mul = stale_mul(game, data, slot, attack);
    if mul != 1.0 {
        fp::fmuls(damage, mul)
    } else {
        damage
    }
}

/// The ECB each fighter gets in the items' copy (`ecb_offset` in `ftCo_DownAttack.c`, which
/// nothing writes).
const TRACK_ECB: _itECB = _itECB {
    top: 14.0,
    bottom: 0.0,
    right: -3.0,
    left: 3.0,
};

/// Copies the fighters' positions for the items to keep clear of, `ftCo_80098634`.
#[doc(alias = "ftCo_80098634")]
pub fn track_fighters(game: &mut Game) {
    let mut cur = game.world.first_in(P_LINK_FIGHTER);
    let mut i = 0;
    while let Some(g) = cur {
        let pos = fighter(game, g).cur_pos;
        let t = &mut game.it.ft_track;
        t.ecb_offset_arr[i] = TRACK_ECB;
        t.ft_pos_arr[i] = pos;
        cur = game.world.gobjs[g.0].next;
        i += 1;
    }
    game.it.ft_track.count = i as u32;
}

/// The fighter's held special item, `ftLib_GetHeldSpecialItem`.
#[doc(alias = "ftLib_GetHeldSpecialItem")]
pub fn held_special_item(game: &Game, gobj: GObjId) -> Option<GObjId> {
    fighter(game, gobj).x1984_heldItemSpec
}

/// Whether the fighter holds `item` (in hand, or as `x1978`), `ftLib_IsHoldingItem`.
#[doc(alias = "ftLib_IsHoldingItem")]
pub fn is_holding(game: &Game, gobj: GObjId, item: GObjId) -> bool {
    let fp = fighter(game, gobj);
    fp.item_gobj == Some(item) || fp.x1978 == Some(item)
}

/// Lets go of an item from a fighter's hand: its held special item (`ftLib_ClearHeldSpecialItem`)
/// or a held item (`ftLib_ReleaseItem`), whichever the item is (`Item_8026A848`'s choice).
pub fn release(r: &mut Running, gobj: GObjId, fighter_gobj: GObjId, special: bool) {
    if special {
        if held_special_item(r.game, fighter_gobj) == Some(gobj)
            && fighter(r.game, fighter_gobj).x1984_heldItemSpec.is_some()
        {
            code::unported_call("ftCommon_8007E82C (ftLib_ClearHeldSpecialItem)");
        }
    } else if is_holding(r.game, fighter_gobj, gobj) {
        let fp = fighter(r.game, fighter_gobj);
        if fp.item_gobj.is_some() || fp.x1978.is_some() {
            code::unported_call("ftCommon_8007E6DC (ftLib_ReleaseItem)");
        }
    }
}

/// A fighter's camera bone's position, `ftLib_GetCameraBonePos`: the joint its attributes name
/// for the camera to follow, offset by `co_attrs.x170`.
#[doc(alias = "ftLib_GetCameraBonePos")]
pub fn camera_bone_pos(game: &mut Game, data: &Archives, gobj: GObjId) -> Vec3 {
    let fp = fighter(game, gobj);
    let attrs = data
        .ptr(fp.ft_data.expect("a fighter without data"))
        .expect("a fighter without attributes");
    let bone = data.u32(attrs.at(0x16C)) as usize;
    let offset = fp.co_attrs.x170;
    let parts = fp.parts.expect("a fighter without parts");
    let j = game.fighter_parts[parts.0][bone].joint;
    crate::lb::joint::world_position(&mut game.objs, data, j, Some(offset))
}

/// Which way most of a fighter's opponents are from `pos`, `ftLib_GetOpponentsDir`: 1 or -1
/// (a random one on a tie). With no fighter, every awake fighter counts.
#[doc(alias = "ftLib_GetOpponentsDir")]
pub fn opponents_dir(game: &mut Game, data: &Archives, pos: Vec3, gobj: Option<GObjId>) -> f32 {
    let me = gobj.map(|g| (g, fighter(game, g).player_idx, fighter(game, g).team));
    let mut result = 0i32;
    let mut cur = game.world.first_in(P_LINK_FIGHTER);
    while let Some(g) = cur {
        cur = game.world.gobjs[g.0].next;
        if me.is_some_and(|(m, slot, _)| m == g || fighter(game, g).player_idx == slot) {
            continue;
        }
        let other = fighter(game, g);
        if other.is_sleeping
            || (game.vs_is_teams() && me.is_some_and(|(_, _, team)| other.team == team))
        {
            continue;
        }
        let v = camera_bone_pos(game, data, g);
        result = result.wrapping_add(if fp::fsubs(reg(v.x), reg(pos.x)) < 0.0 {
            -1
        } else {
            1
        });
    }
    if result == 0 {
        result = if game.objs.rng.below(2) != 0 { 1 } else { -1 };
    }
    if result < 0 { -1.0 } else { 1.0 }
}

/// Which way most fighters are from `pos`, `it_8026B684`.
#[doc(alias = "it_8026B684")]
pub fn facing_for(game: &mut Game, data: &Archives, pos: Vec3) -> f32 {
    opponents_dir(game, data, pos, None)
}
