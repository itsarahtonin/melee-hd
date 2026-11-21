//! The Super Scope, `melee/it/kinds/itsscope.c`: a shooting item that falls, lands and is
//! held, with its ammunition (`xD4C`), the cost of each charge, and its shots
//! ([`sscopebeam`](super::sscopebeam)), which the fighter's scope states fire. Its callbacks
//! are the held items' ([`super::held`]).

use gekko_fp as fp;

use super::super::item::article;
use super::super::{item, item_mut};
use super::held::{self, Held};
use super::sscopebeam;
use crate::game::it::ItKind;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

pub const SPAWNED: u32 = 0x8029_1C98;
pub const ANIM0: u32 = 0x8029_2080;
pub const PHYS0: u32 = 0x8029_2088;
pub const COLL0: u32 = 0x8029_208C;
pub const FELL: u32 = 0x8029_20B8;
pub const ANIM1: u32 = 0x8029_20E0;
pub const PHYS1: u32 = 0x8029_20E8;
pub const COLL1: u32 = 0x8029_2118;
pub const PICKED_UP: u32 = 0x8029_215C;
pub const ANIM2: u32 = 0x8029_2184;
pub const PHYS2: u32 = 0x8029_218C;
pub const DROPPED: u32 = 0x8029_2190;
pub const THROWN: u32 = 0x8029_21B8;
pub const PHYS3: u32 = 0x8029_21E0;
pub const DMG_DEALT: u32 = 0x8029_222C;
pub const CLANKED: u32 = 0x8029_2250;
pub const HIT_SHIELD: u32 = 0x8029_2274;
pub const REFLECTED: u32 = 0x8029_2298;
pub const SHIELD_BOUNCED: u32 = 0x8029_22B8;
pub const ENTERED_AIR: u32 = 0x8029_22D8;
pub const ANIM4: u32 = 0x8029_2300;
pub const PHYS4: u32 = 0x8029_2308;
pub const COLL4: u32 = 0x8029_230C;

/// The Super Scope: its ammunition, let go of or thrown into state 3, skidding in 4.
pub const SUPER_SCOPE: Held = Held {
    name: "Super Scope",
    charges: true,
    on_spawn: None,
    fell: held::fell,
    dropped: 3,
    thrown: 3,
    drop_update: true,
    skid: 4,
    bounces_only_let_go: false,
    hits_disarm: false,
    skid_faces_motion: false,
};

/// Runs one of the Super Scope's predicates, if `addr` is one.
#[melee_callbacks::callbacks(Pred)]
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 | ANIM1 | ANIM2 | ANIM4 => held::still(r, gobj),
        COLL0 => held::grounded(r, gobj, &SUPER_SCOPE),
        COLL1 => held::lands_loaded(r, gobj),
        COLL4 => held::skid(r, gobj, &SUPER_SCOPE),
        DMG_DEALT | CLANKED | HIT_SHIELD => held::bounce(r, gobj, &SUPER_SCOPE),
        REFLECTED => held::reflected(r, gobj, &SUPER_SCOPE),
        SHIELD_BOUNCED => held::shield_bounced(r, gobj, &SUPER_SCOPE),
        _ => return None,
    })
}

/// Runs one of the Super Scope's events, if `addr` is one.
#[melee_callbacks::callbacks(Proc)]
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => held::spawned(r, gobj, &SUPER_SCOPE),
        FELL => held::fell(r, gobj),
        PHYS0 | PHYS2 | PHYS4 => held::idle(r, gobj),
        PHYS1 => held::gravity(r, gobj),
        PICKED_UP => held::picked_up(r, gobj),
        DROPPED | THROWN => held::dropped(r, gobj, &SUPER_SCOPE),
        PHYS3 => held::thrown_phys(r, gobj),
        ENTERED_AIR => held::entered_air(r, gobj, &SUPER_SCOPE),
        _ => return false,
    }
    true
}

/// `itSScopeAttributes`: the ammunition (0), the charge below which a shot is the smallest
/// (4), the charges a level takes, by eighths (8), each level's cost (0xC, ten floats) and
/// the muzzle's offset (0x34).
fn attrs(r: &Running, gobj: GObjId) -> melee_data::archive::DataRef {
    article(item(r.game, gobj))
        .special(r.data)
        .expect("a Super Scope without attributes")
}

/// Where the shots leave the muzzle, in the hand's space, `it_80291CCC`.
#[doc(alias = "it_80291CCC")]
pub fn muzzle(r: &Running, gobj: GObjId) -> Vec3 {
    crate::game::mirror::read_vec3(r.data, attrs(r, gobj).at(0x34))
}

/// The level a charge of `charge` ticks makes, 0 to 9, `it_80291CF4`.
#[doc(alias = "it_80291CF4")]
pub fn charge_level(r: &Running, gobj: GObjId, charge: i32) -> i32 {
    let a = attrs(r, gobj);
    let (low, step) = (r.data.u32(a.at(4)) as i32, r.data.u32(a.at(8)) as i32);
    if charge <= low {
        0
    } else if charge < step.wrapping_mul(8) {
        charge / 8
    } else {
        9
    }
}

/// What a shot of `level` costs, `it_80291D38`: its float, truncated. Its switch has no
/// case for another level, and the machine code then returns its first argument untouched:
/// the scope's GObj address.
#[doc(alias = "it_80291D38")]
pub fn cost(r: &Running, gobj: GObjId, level: i32) -> i32 {
    if !(0..=9).contains(&level) {
        // Nothing in the game calls it so: the address it returns would be read as a cost
        // only here, which the stand-in doesn't cover.
        assert!(
            r.game.world.gobjs.addr(gobj.0) != 0,
            "it_80291D38 read a Super Scope's GObj address as a cost in a match without the allocator model"
        );
        return gobj_address(r, gobj);
    }
    fp::fctiwz(f64::from(
        r.data.f32(attrs(r, gobj).at(0xC + 4 * level as u32)),
    ))
}

/// The scope's GObj address, which `it_80291DAC` and `it_80291D38` return where they set no
/// level: the allocator model's ([`crate::hsd::heap`]). A Vanilla match that can spawn a Super
/// Scope runs with the model (`vs_match::Settings`), so the address is the original's.
///
/// Without the model, under the Slippi ruleset, it's [`ADDRESS_STAND_IN`]: every address the
/// heap can give is a negative word that takes the same branches (no level case matches it, a
/// charged shot of it clicks, and spending it leaves over two billion shots), so only the
/// ammunition word and the fighter's state word that hold it differ, by the address, and
/// nothing else reads them as numbers ([`spend`] checks the one that could matter). A `.slp`
/// records the state word (post-frame "hitstun"), which already differs between two machines'
/// layouts and which playback never reads. Vanilla stops by name instead.
fn gobj_address(r: &Running, gobj: GObjId) -> i32 {
    let addr = r.game.world.gobjs.addr(gobj.0);
    if addr != 0 {
        return addr as i32;
    }
    assert!(
        r.game.rules.sscope_stand_in(),
        "a Super Scope's GObj address reached gameplay in a Vanilla match without the allocator model"
    );
    ADDRESS_STAND_IN as i32
}

/// The Super Scope's GObj address under the Slippi ruleset without the allocator model
/// ([`gobj_address`]): an address in the scene heap's range (Slippi playback's main heap is
/// `0x80BD8B20` to `0x817F8AC0`, a console's VS match's lies within the same bounds).
pub const ADDRESS_STAND_IN: u32 = 0x8100_0000;

/// The level a shot of `charge` ticks fires with what's left, `it_80291DAC`: none (-1) when
/// empty; the charge's level when it can afford it; else, going down from that level, the
/// first level it can afford exactly, or the one above the first it can afford with some
/// left. When even level 1 is too dear (or the charge's level is 0 and too dear), the
/// original falls off its end and returns its first argument, the scope's GObj address,
/// which its callers take as a level: the infinite-ammunition glitch (`docs/melee-bugs.md`).
#[doc(alias = "it_80291DAC")]
pub fn affordable(r: &Running, gobj: GObjId, charge: i32) -> i32 {
    let ammo = item(r.game, gobj).xD4C;
    if ammo <= 0 {
        return -1;
    }
    let mut level = charge_level(r, gobj, charge);
    if ammo.wrapping_sub(cost(r, gobj, level)) >= 0 {
        return level;
    }
    while level > 0 {
        let left = ammo.wrapping_sub(cost(r, gobj, level));
        if left == 0 {
            return level;
        }
        if left > 0 {
            return level + 1;
        }
        level -= 1;
    }
    gobj_address(r, gobj)
}

/// Spends a shot of `level`, to no less than none, `it_80291F14`. Its inline copy of
/// `it_80291D38` keeps the cost in the level's register, so a level no case covers (the GObj
/// address `affordable` returns) costs itself, which leaves the scope more than two billion
/// shots.
#[doc(alias = "it_80291F14")]
pub fn spend(r: &mut Running, gobj: GObjId, level: i32) {
    let c = if (0..=9).contains(&level) {
        cost(r, gobj, level)
    } else {
        level
    };
    let stand_in = !(0..=9).contains(&level) && r.game.world.gobjs.addr(gobj.0) == 0;
    if stand_in {
        // A level outside the cases is only `affordable`'s address (its callers never spend
        // none); without the model, the stand-in, which must leave the glitch's refill.
        assert!(
            level == ADDRESS_STAND_IN as i32,
            "it_80291F14 spent a level outside its cases that isn't the Super Scope's address stand-in"
        );
    }
    let ip = item_mut(r.game, gobj);
    ip.xD4C = ip.xD4C.wrapping_sub(c);
    if ip.xD4C < 0 {
        ip.xD4C = 0;
    }
    assert!(
        !stand_in || ip.xD4C > 0,
        "the Super Scope's address stand-in left it no ammunition"
    );
}

/// Spends and fires a shot of `level` from `pos`, facing `facing`, `it_80291FA8`.
#[doc(alias = "it_80291FA8")]
pub fn shoot(r: &mut Running, gobj: GObjId, pos: Vec3, level: i32, facing: f32) {
    spend(r, gobj, level);
    let owner = item(r.game, gobj)
        .owner
        .expect("a Super Scope fired without an owner");
    sscopebeam::fire(r, owner, pos, level, facing);
}

/// A Super Scope standing at `pos`, `it_80291BE0`.
#[doc(alias = "it_80291BE0")]
pub fn spawn_at(r: &mut Running, pos: Vec3) -> Option<GObjId> {
    let mut spawn = super::super::item::drop_spawn(ItKind::SuperScope.id(), pos, false);
    let gobj = super::super::item::spawn_ground(r, &mut spawn)?;
    held::landed(r, gobj);
    Some(gobj)
}
