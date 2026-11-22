//! What the items read of fighters: their teams, positions and scale, the stale-move table
//! their owner's attacks weaken by, and the fighter-side bookkeeping items do (the count of
//! objects using a character's data, the fighters the items keep clear of). The fighter core
//! is ported separately; these read the generated `Fighter` directly.

use gekko_fp as fp;
use melee_data::archive::Archives;

use crate::fpu::{mem, reg};
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
#[doc(alias = "ftData_80085560")]
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
    assert!(
        scale != 0.0,
        "the original's assertion failed: ftchangeparam.c:30 (scale != 0.0F)"
    );
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
    fighter(game, gobj).special_item
}

/// The fighter lets go of its held special item, `ftLib_ClearHeldSpecialItem` (with
/// `ftCommon_8007E82C`).
#[doc(alias = "ftLib_ClearHeldSpecialItem")]
#[doc(alias = "ftCommon_8007E82C")]
pub fn clear_held_special_item(game: &mut Game, gobj: GObjId) {
    let f = fighter_of(game, gobj);
    let fp = &mut game.fighters[f.0];
    if fp.special_item.is_some() {
        fp.special_item = None;
    }
}

/// Whether the fighter holds `item` (in hand, or as `x1978`), `ftLib_IsHoldingItem`.
#[doc(alias = "ftLib_IsHoldingItem")]
pub fn is_holding(game: &Game, gobj: GObjId, item: GObjId) -> bool {
    let fp = fighter(game, gobj);
    fp.item_gobj == Some(item) || fp.second_item == Some(item)
}

/// Lets go of an item from a fighter's hand: its held special item (`ftLib_ClearHeldSpecialItem`)
/// or a held item (`ftLib_ReleaseItem`), whichever the item is (`Item_8026A848`'s choice).
#[doc(alias = "ftLib_ReleaseItem")]
pub fn release(r: &mut Running, gobj: GObjId, fighter_gobj: GObjId, special: bool) {
    if special {
        if held_special_item(r.game, fighter_gobj) == Some(gobj) {
            clear_held_special_item(r.game, fighter_gobj);
        }
    } else if is_holding(r.game, fighter_gobj, gobj) {
        // `ftLib_ReleaseItem`.
        let fp = fighter(r.game, fighter_gobj);
        if fp.item_gobj.is_some() || fp.second_item.is_some() {
            r.release_item(fighter_gobj, gobj, true);
        }
    }
}

/// A fighter's camera bone's position (`ftLib_GetCameraBonePos`, whose port is the fighters'
/// `camera_bone_pos`) from the game and data alone, as the item code reaches it.
pub fn camera_bone_pos(game: &mut Game, data: &Archives, gobj: GObjId) -> Vec3 {
    let fp = fighter(game, gobj);
    let attrs = data
        .ptr(fp.ft_data.expect("a fighter without data"))
        .expect("a fighter without attributes");
    let bone = data.u32(attrs.at(0x16C)) as usize;
    let offset = fp.co_attrs.camera_bone_offset;
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
        // The Slippi ruleset's WhispyBlowDirFix (`8008653C`, not in 3.14's codes): a fighter in
        // a dead or sleeping state (motion 11 or below) counts for neither side.
        if game.rules.whispy_blow_dir_fix() && fighter(game, g).motion_id <= 11 {
            continue;
        }
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

/// The opponent of `gobj` (anyone, for None) whose camera bone is nearest `pos`,
/// `ftLib_FindNearestOpponent`: none when every fighter is out.
#[doc(alias = "ftLib_FindNearestOpponent")]
pub fn nearest_opponent(
    game: &mut Game,
    data: &Archives,
    pos: Vec3,
    gobj: Option<GObjId>,
) -> Option<GObjId> {
    nearest_by_bone(game, data, pos, gobj, None)
}

/// [`nearest_opponent`], and with `facing_dir` [`nearest_opponent_in_dir`]: the candidates'
/// camera bones read in the fighters' order, then the nearest taken; under Slippi's rules the
/// outcome is also taken with the other display-phase placement of each fighter the camera
/// places ([`crate::game::live::DisplayPhase`]), and a different one noted.
fn nearest_by_bone(
    game: &mut Game,
    data: &Archives,
    pos: Vec3,
    gobj: Option<GObjId>,
    facing_dir: Option<f32>,
) -> Option<GObjId> {
    let me = gobj.map(|g| (g, fighter(game, g).player_idx, fighter(game, g).team));
    // Up to six players and their followers.
    let mut bones = [(GObjId(0), Vec3::default()); 12];
    let mut n = 0;
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
        bones[n] = (g, camera_bone_pos(game, data, g));
        n += 1;
    }
    let result = nearest_of(pos, &bones[..n], facing_dir);
    if game.rules.notes_playback() {
        // The bones with each fighter the camera places at the other display-phase placement;
        // while playback's camera is followed apart from this one, at the logic's placement and
        // at the pass's through playback's camera instead (`DisplayPhase::place`).
        let following = game.follows_playback_camera();
        let (mut other, mut logic, mut theirs) = (bones, bones, Some(bones));
        let (mut moved, mut by_camera) = (false, false);
        for i in 0..n {
            let (g, v) = bones[i];
            let fp = fighter(game, g);
            if !fp.camera_placed {
                continue;
            }
            let f = fighter_of(game, g);
            let Some(crate::game::state::Obj::JObj(j)) = game.world.gobjs[g.0].obj else {
                continue;
            };
            let root = game.objs.jobjs[j].translate;
            // The pass placed it through the camera's second CObj.
            by_camera |= placed_by_pass(root, fp.cur_pos);
            let placed = game.display_phase.placed(f.0 as usize);
            if following {
                moved = true;
                logic[i].1 = shifted(v, fp.cur_pos, root);
                match (placed.and_then(|p| p.1), &mut theirs) {
                    (Some(at), Some(t)) => t[i].1 = shifted(v, at, root),
                    // Not placed through playback's camera yet: its outcome isn't known.
                    _ => theirs = None,
                }
            } else if let Some(w) = other_placement(v, root, fp.cur_pos, placed.map(|p| p.0)) {
                other[i].1 = w;
                moved = true;
            }
        }
        let lost = game.display_phase.lost;
        if read_noted(lost, by_camera, || {
            moved
                && if following {
                    followed_differs(
                        pos,
                        facing_dir,
                        result,
                        &logic[..n],
                        theirs.as_ref().map(|t| &t[..n]),
                    )
                } else {
                    nearest_of(pos, &other[..n], facing_dir) != result
                }
        }) {
            let tick = game.scene.game_ticks;
            game.display_phase.note(tick);
        }
    }
    result
}

/// Whether a nearest-opponent read that found `result` decides otherwise in playback's run while
/// playback's camera is followed apart from this one: at either display phase, each fighter the
/// camera places at the logic's placement (`logic`) or at the pass's through playback's camera
/// (`theirs`, none where one isn't known, which counts as otherwise).
fn followed_differs(
    pos: Vec3,
    facing_dir: Option<f32>,
    result: Option<GObjId>,
    logic: &[(GObjId, Vec3)],
    theirs: Option<&[(GObjId, Vec3)]>,
) -> bool {
    nearest_of(pos, logic, facing_dir) != result
        || theirs.is_none_or(|t| nearest_of(pos, t, facing_dir) != result)
}

/// Camera bone `v` of a fighter whose root is at `root`, with the root at `at` instead.
fn shifted(v: Vec3, at: Vec3, root: Vec3) -> Vec3 {
    let shift = |v: f32, a: f32, r: f32| mem(fp::fadds(reg(v), fp::fsubs(reg(a), reg(r))));
    Vec3::new(
        shift(v.x, at.x, root.x),
        shift(v.y, at.y, root.y),
        shift(v.z, at.z, root.z),
    )
}

/// Whether a fighter KO'd toward the screen, its root at `root` and its position `cur_pos`,
/// stands where the display pass placed it through the camera's second CObj
/// (`Camera_800310B8`), not where the logic put it back (a doubled round's second tick).
fn placed_by_pass(root: Vec3, cur_pos: Vec3) -> bool {
    root != cur_pos
}

/// Whether a nearest-opponent read is noted (design call 9): where playback's camera is lost
/// (`DisplayPhase::lost`), through a fighter the pass placed by the camera, whatever it
/// decides; otherwise where the other placements (`differs`) decide otherwise.
fn read_noted(lost: bool, by_camera: bool, differs: impl FnOnce() -> bool) -> bool {
    (lost && by_camera) || differs()
}

/// A camera bone `v` as it would read with the other display-phase placement of its
/// fighter's root, now at `root`: at the logic's, its position `cur_pos`, when the display
/// pass placed it (at `pass`, its last placement); at the pass's when the logic did (a doubled
/// round's second tick). None where both are the same, or the pass hasn't placed it yet.
fn other_placement(v: Vec3, root: Vec3, cur_pos: Vec3, pass: Option<Vec3>) -> Option<Vec3> {
    let alt = if root == cur_pos { pass } else { Some(cur_pos) };
    let a = alt.filter(|&a| a != root)?;
    Some(shifted(v, a, root))
}

/// The candidate of `bones` (each a fighter and its camera bone) nearest `pos`, the first on
/// a tie, on the side `facing_dir` looks toward when given.
fn nearest_of(pos: Vec3, bones: &[(GObjId, Vec3)], facing_dir: Option<f32>) -> Option<GObjId> {
    let mut best = f64::from(f32::MAX);
    let mut result = None;
    for &(g, v) in bones {
        if let Some(d) = facing_dir
            && ((d == -1.0 && v.x > pos.x) || (d == 1.0 && v.x < pos.x))
        {
            continue;
        }
        let dx = fp::fsubs(reg(pos.x), reg(v.x));
        let dy = fp::fsubs(reg(pos.y), reg(v.y));
        let dist = fp::fmadds(dx, dx, fp::fmuls(dy, dy));
        if dist < best {
            best = dist;
            result = Some(g);
        }
    }
    result
}

/// The opponent of `gobj` (anyone, for None) with the lowest percent, the first of them on a
/// tie, `ftLib_FindLowestPercentOpponent`: none when every fighter is out. The percents
/// compare against the lowest so far truncated (it starts at 999).
#[doc(alias = "ftLib_FindLowestPercentOpponent")]
pub fn lowest_percent_opponent(game: &Game, gobj: Option<GObjId>) -> Option<GObjId> {
    let me = gobj.map(|g| (g, fighter(game, g).player_idx, fighter(game, g).team));
    let mut lowest = 999i32;
    let mut result = None;
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
        let percent = reg(other.dmg.percent);
        if percent < fp::frsp(f64::from(lowest)) {
            lowest = fp::fctiwz(percent);
            result = Some(g);
        }
    }
    result
}

/// The opponent of `gobj` (anyone, for None) whose camera bone is nearest `pos` on the side
/// `facing_dir` looks toward, `ftLib_FindNearestOpponentInDir`.
#[doc(alias = "ftLib_FindNearestOpponentInDir")]
pub fn nearest_opponent_in_dir(
    game: &mut Game,
    data: &Archives,
    pos: Vec3,
    gobj: Option<GObjId>,
    facing_dir: f32,
) -> Option<GObjId> {
    nearest_by_bone(game, data, pos, gobj, Some(facing_dir))
}

/// The nearest opponent of `gobj` on the side `facing_dir` looks toward and its camera bone's
/// position, `it_8026B634`.
#[doc(alias = "it_8026B634")]
pub fn nearest_opponent_ahead(
    game: &mut Game,
    data: &Archives,
    pos: Vec3,
    gobj: Option<GObjId>,
    facing_dir: f32,
) -> Option<(GObjId, Vec3)> {
    let g = nearest_opponent_in_dir(game, data, pos, gobj, facing_dir)?;
    Some((g, camera_bone_pos(game, data, g)))
}

/// The nearest fighter to `pos` and its camera bone's position, `it_8026B5E4`.
#[doc(alias = "it_8026B5E4")]
pub fn nearest_fighter(
    game: &mut Game,
    data: &Archives,
    pos: Vec3,
    gobj: Option<GObjId>,
) -> Option<(GObjId, Vec3)> {
    let g = nearest_opponent(game, data, pos, gobj)?;
    Some((g, camera_bone_pos(game, data, g)))
}

/// Which way most fighters are from `pos`, `it_8026B684`.
#[doc(alias = "it_8026B684")]
pub fn facing_for(game: &mut Game, data: &Archives, pos: Vec3) -> f32 {
    opponents_dir(game, data, pos, None)
}

#[cfg(test)]
mod display_phase_tests {
    use super::*;

    /// A fighter KO'd toward the screen, its root where the display pass placed it, is the
    /// nearest opponent of an item at the origin; with its root at its position (the logic's
    /// placement, a doubled round's second tick) the other fighter is: the read is one whose
    /// outcome the display pass's phase decides. Both ways round, and a facing that skips it.
    #[test]
    fn a_read_through_a_placed_joint_goes_either_way() {
        let (a, b) = (GObjId(1), GObjId(2));
        let pass = Vec3::new(10.0, 2.0, 0.0);
        let ko_spot = Vec3::new(100.0, 2.0, 0.0);
        // The bone 3 above the root.
        let bone = |root: Vec3| Vec3::new(root.x, root.y + 3.0, 0.0);
        let other = (b, Vec3::new(30.0, 5.0, 0.0));
        let at = Vec3::default();
        // After a display pass: the root at the pass's placement.
        let mut bones = [(a, bone(pass)), other];
        assert_eq!(nearest_of(at, &bones, None), Some(a));
        bones[0].1 = other_placement(bones[0].1, pass, ko_spot, Some(pass)).unwrap();
        assert_eq!(bones[0].1, bone(ko_spot));
        assert_eq!(nearest_of(at, &bones, None), Some(b));
        // A doubled round's second tick: the root at the fighter's position.
        let mut bones = [(a, bone(ko_spot)), other];
        assert_eq!(nearest_of(at, &bones, None), Some(b));
        bones[0].1 = other_placement(bones[0].1, ko_spot, ko_spot, Some(pass)).unwrap();
        assert_eq!(nearest_of(at, &bones, None), Some(a));
        // Not yet placed, or placed where it stands: nothing to compare.
        assert_eq!(other_placement(bone(ko_spot), ko_spot, ko_spot, None), None);
        assert_eq!(other_placement(bone(pass), pass, pass, Some(pass)), None);
        // Facing left, the item sees neither.
        assert_eq!(nearest_of(at, &[(a, bone(pass)), other], Some(-1.0)), None);
    }

    /// While playback's camera is followed apart from this one, a read through a fighter the
    /// camera places is noted where it decides otherwise with the fighter at the logic's
    /// placement or at the pass's through playback's camera, or where that one isn't known; not
    /// where both decide the same.
    #[test]
    fn a_read_through_playbacks_followed_camera_is_decided_both_ways() {
        let (a, b) = (GObjId(1), GObjId(2));
        let at = Vec3::default();
        let other = (b, Vec3::new(30.0, 5.0, 0.0));
        // Ours: the KO'd fighter nearest, its root where the pass placed it.
        let ours = [(a, Vec3::new(10.0, 5.0, 0.0)), other];
        let result = nearest_of(at, &ours, None);
        assert_eq!(result, Some(a));
        let near = [(a, Vec3::new(12.0, 5.0, 0.0)), other];
        let far = [(a, Vec3::new(100.0, 5.0, 0.0)), other];
        assert!(!followed_differs(at, None, result, &near, Some(&near)));
        assert!(followed_differs(at, None, result, &far, Some(&near)));
        assert!(followed_differs(at, None, result, &near, Some(&far)));
        assert!(followed_differs(at, None, result, &near, None));
    }

    /// Where playback's camera is lost a read through a fighter the pass placed by the camera
    /// is noted whatever it decides; one through a fighter the logic put back, or on course,
    /// only where the other placement decides otherwise.
    #[test]
    fn a_read_through_the_camera_lost_is_noted() {
        let (pass, ko_spot) = (Vec3::new(10.0, 2.0, 0.0), Vec3::new(100.0, 2.0, 0.0));
        assert!(placed_by_pass(pass, ko_spot));
        assert!(!placed_by_pass(ko_spot, ko_spot));
        let unreached = || -> bool { panic!("the other placement taken") };
        assert!(read_noted(true, true, unreached));
        for (off_course, by_camera) in [(true, false), (false, true), (false, false)] {
            assert!(!read_noted(off_course, by_camera, || false));
            assert!(read_noted(off_course, by_camera, || true));
        }
    }
}
