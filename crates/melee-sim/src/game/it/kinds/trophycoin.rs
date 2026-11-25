//! The single-player coin, `melee/it/kinds/itcoin.c` (`It_Kind_Coin`): a trophy on its stand
//! that a beaten enemy drops in a single-player mode (`it::zako::trophy_roll`), a trophy point
//! of an adventure stage (`Ground_801C58E0`), the rest area or Snag the Trophies sets out. It
//! falls (state 0) and lies on the floor (1) until a fighter picks it up, which counts the
//! trophy as the match's (`gm_8017E280`) as it goes; hit, it flies off (3) and falls again.
//! Its model is the trophy's display model (`tyDisplay_8031C5E4`), which its render callback
//! trades for the question mark when the single-player camera zooms out past the attributes'
//! `x48`.
//!
//! Its kind (`x4`) is 0 for a trophy; Snag the Trophies' coins (2 and 3, `grfigureget.c`'s,
//! with their stage object in `x14`) fall at their own speeds and show the question mark
//! only until the stand's coins are all snagged or gone (`gr::snag_the_trophies`).

use gekko_fp as fp;
use melee_data::archive::DataRef;

use super::super::ground::{fall, fall_to_floor, ground_or_fall, ground_skid};
use super::super::item::{article, attr_of, change, change_state};
use super::super::{data, hitbox, item, item_mut, jobj_of, lib26b1, lib2725, maplib, zako};
use crate::fpu::{mem, reg};
use crate::game::it::ItKind;
use crate::game::state::{OBJ_JOBJ, Obj, Running};
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::{GObjId, Host};
use crate::hsd::jobj;
use crate::vec::Vec3;

pub const RENDER: u32 = 0x802F_13B4;
pub const SPAWNED: u32 = 0x802F_14E4;
pub const DESTROYED: u32 = 0x802F_1518;
pub const ANIM0: u32 = 0x802F_15B0;
pub const PHYS0: u32 = 0x802F_15B8;
pub const COLL0: u32 = 0x802F_1604;
pub const ANIM1: u32 = 0x802F_1850;
pub const PHYS1: u32 = 0x802F_1858;
pub const COLL1: u32 = 0x802F_185C;
pub const PICKED_UP: u32 = 0x802F_1888;
pub const ANIM2: u32 = 0x802F_18E4;
pub const PHYS2: u32 = 0x802F_18EC;
#[doc(alias("EVT_UNK"))]
pub const FORGET: u32 = 0x802F_18F0;
pub const DMG_RECEIVED: u32 = 0x802F_1910;
pub const ANIM3: u32 = 0x802F_1C68;
pub const PHYS3: u32 = 0x802F_1C70;
pub const COLL3: u32 = 0x802F_1CB4;
pub const THROWN: u32 = 0x802F_1CE0;
pub const ANIM4: u32 = 0x802F_1EE4;
pub const PHYS4: u32 = 0x802F_1EEC;
pub const COLL4: u32 = 0x802F_1F1C;
pub const ENTERED_AIR: u32 = 0x802F_1F48;
pub const ANIM5: u32 = 0x802F_1FD4;
pub const PHYS5: u32 = 0x802F_1FDC;
pub const COLL5: u32 = 0x802F_1FE0;

/// `ITEM_UNK_0x1`, a state change's first flag.
const UNK_0X1: u32 = 1;

/// Runs one of the coin's predicates, if `addr` is one.
#[melee_callbacks::callbacks(Pred)]
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 | ANIM1 | ANIM2 | ANIM3 | ANIM4 | ANIM5 => still(r, gobj),
        COLL0 | COLL4 => coll0(r, gobj),
        COLL1 => coll1(r, gobj),
        COLL3 => coll3(r, gobj),
        COLL5 => coll5(r, gobj),
        DMG_RECEIVED => damaged(r, gobj),
        _ => return None,
    })
}

/// Runs one of the coin's events, if `addr` is one.
#[melee_callbacks::callbacks(Proc)]
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        DESTROYED => destroyed(r, gobj),
        PHYS0 => phys0(r, gobj),
        PHYS1 | PHYS2 | PHYS5 => idle(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        PHYS3 => phys3(r, gobj),
        THROWN => thrown(r, gobj),
        PHYS4 => phys4(r, gobj),
        ENTERED_AIR => entered_air(r, gobj),
        _ => return false,
    }
    true
}

/// `itCoinAttributes`' float at `off`.
fn attr(r: &Running, gobj: GObjId, off: u32) -> f32 {
    let a: DataRef = article(item(r.game, gobj))
        .special(r.data)
        .expect("a 1P coin without attributes");
    r.data.f32(a.at(off))
}

/// Makes a coin holding trophy `trophy` at `pos`, `it_802F2094`: in the air, facing nowhere,
/// still; a trophy (`kind` 0) blinking, with its camera box gone and to be picked up; Snag
/// the Trophies' (2, 3, from its stage object `from`) not blinking, not to be picked up, at
/// its kind's fall speeds. Its model becomes the trophy's display model, its scale the stage's
/// times the attributes' `x44`. None if the items are at their limit.
#[doc(alias = "it_802F2094")]
pub fn spawn(
    r: &mut Running,
    from: Option<GObjId>,
    pos: Vec3,
    trophy: i32,
    kind: i32,
) -> Option<GObjId> {
    let mut spawn = SpawnItem {
        kind: ItKind::Trophy.id(),
        ..Default::default()
    };
    spawn.prev_pos = pos;
    spawn.pos = pos;
    spawn.facing_dir = 0.0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.damage = 0;
    spawn.owner = None;
    spawn.maker = None;
    spawn.flags[0] |= 0x80;
    spawn.ignore_id = 0;
    let gobj = super::super::item::spawn_air(r, &mut spawn)?;
    let v = item_mut(r.game, gobj).vars.trophy_coin();
    v.trophy.set(trophy);
    v.trophy_kind.set(kind);
    v.stand.set(from);
    if kind != 0 {
        hitbox::end_blink(r.game, gobj);
        lib26b1::disable_pickup(r.game, gobj);
        let (fall_at, max_at) = match kind {
            2 => (0xC, 0x10),
            3 => (0x18, 0x1C),
            _ => (0x0, 0x4),
        };
        let (f, m) = (attr(r, gobj, fall_at), attr(r, gobj, max_at));
        let v = item_mut(r.game, gobj).vars.trophy_coin();
        v.spin.set(f);
        v.spin_max.set(m);
    } else {
        super::super::item::remove_camera_box(r.game, gobj);
        hitbox::start_blink(r.game, gobj);
        lib26b1::enable_pickup(r.game, gobj);
        item_mut(r.game, gobj).skid = 0;
    }
    let model = r.coin_trophy_model(trophy);
    // `HSD_GObjObject_80390B0C`, then `HSD_GObjObject_80390A70` with the trophy's model.
    if let Some((k, obj)) = r.game.world.take_obj(gobj) {
        r.remove_obj(k, obj);
    }
    let model = model.unwrap_or_else(|| {
        panic!(
            "Melee crashes: the 1P coin's trophy {trophy} has no display model loaded \
             (tyDisplay_8031C5E4 returns null), and its scale asserts on the null joint"
        )
    });
    r.game.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(model));
    super::super::coll::init_hurtboxes(r, gobj);
    let scale = crate::game::gr::ground::scale(r.game);
    let size = attr(r, gobj, 0x44);
    let ip = item_mut(r.game, gobj);
    ip.scl = scale;
    ip.scl = mem(fp::fmuls(reg(ip.scl), reg(size)));
    let s = ip.scl;
    jobj::set_scale(&mut r.game.objs.jobjs, model, Vec3::new(s, s, s));
    Some(gobj)
}

/// `itCoin_Logic116_Spawned`: a trophy's coin, falling.
#[doc(alias = "itCoin_Logic116_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    let v = item_mut(r.game, gobj).vars.trophy_coin();
    v.trophy_kind.set(0);
    v.trophy.set(0);
    v.unread_10.set(0);
    fell(r, gobj);
}

/// `itCoin_Logic116_Destroyed`: Snag the Trophies' coin tells its stage object; a trophy's,
/// gone from a fighter's hand, counts the trophy as the match's and the coin as an enemy KO'd
/// (`it_8027CE44`); then the enemies' destroy (`it_2725_Logic9_Destroyed`).
#[doc(alias = "itCoin_Logic116_Destroyed")]
pub fn destroyed(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    let held = ip.flags.flags.held;
    let v = ip.vars.trophy_coin();
    if v.trophy_kind.get() != 0 {
        let stand = v.stand.get();
        crate::game::gr::snag_the_trophies::coin_gone(r.game, stand);
    } else if held {
        let trophy = v.trophy.get();
        r.count_match_trophy(trophy as u16, 1);
        zako::ko(r, gobj);
    }
    zako::destroyed(r, gobj);
}

/// Falls, `it_802F1588`.
#[doc(alias = "it_802F1588")]
pub fn fell(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 0, UNK_0X1);
}

/// The states' animations, which do nothing (`itCoin_UnkMotion0_Anim` to 5).
#[doc(alias = "itCoin_UnkMotion0_Anim")]
#[doc(alias = "itCoin_UnkMotion1_Anim")]
#[doc(alias = "itCoin_UnkMotion2_Anim")]
#[doc(alias = "itCoin_UnkMotion3_Anim")]
#[doc(alias = "itCoin_UnkMotion4_Anim")]
#[doc(alias = "itCoin_UnkMotion5_Anim")]
pub fn still(_: &mut Running, _: GObjId) -> bool {
    false
}

/// The states that move nothing, `itCoin_UnkMotion1_Phys`, `2` and `5`.
#[doc(alias = "itCoin_UnkMotion1_Phys")]
#[doc(alias = "itCoin_UnkMotion2_Phys")]
#[doc(alias = "itCoin_UnkMotion5_Phys")]
pub fn idle(_: &mut Running, _: GObjId) {}

/// Falling's physics, `itCoin_UnkMotion0_Phys`: a trophy's at its attributes' fall speeds,
/// Snag the Trophies' at its own.
#[doc(alias = "itCoin_UnkMotion0_Phys")]
pub fn phys0(r: &mut Running, gobj: GObjId) {
    let v = item_mut(r.game, gobj).vars.trophy_coin();
    if v.trophy_kind.get() == 0 {
        phys4(r, gobj);
        return;
    }
    let (f, m) = (v.spin.get(), v.spin_max.get());
    lib2725::fall(r.game, gobj, f, m);
}

/// Falling's collision, `itCoin_UnkMotion0_Coll` and `itCoin_UnkMotion4_Coll`: it lands
/// on a floor (`it_8026E414`).
#[doc(alias = "itCoin_UnkMotion0_Coll")]
#[doc(alias = "itCoin_UnkMotion4_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    fall_to_floor(r, gobj, landed);
    false
}

/// Lands, `it_802F1630`: still, unturned, lying (state 1), blinking.
#[doc(alias = "it_802F1630")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    lib2725::clear_velocity(r.game, gobj);
    unturn(r, gobj);
    change_state(r, gobj, 1, UNK_0X1);
    hitbox::start_blink(r.game, gobj);
}

/// The inline `itCoin_ResetRotation`: the model's turn cleared.
fn unturn(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj).expect("a 1P coin without a model");
    let jobjs = &mut r.game.objs.jobjs;
    jobj::set_rotation_x(jobjs, j, 0.0);
    jobj::set_rotation_y(jobjs, j, 0.0);
    jobj::set_rotation_z(jobjs, j, 0.0);
}

/// Lying's collision, `itCoin_UnkMotion1_Coll`: off a ledge, it falls (`it_8026D62C`).
#[doc(alias = "itCoin_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, fell);
    false
}

/// Picked up, `itCoin_Logic116_PickedUp`: the model shown, held (state 2).
#[doc(alias = "itCoin_Logic116_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    let child = jobj_of(r.game, gobj).and_then(|j| r.game.objs.jobjs[j].child);
    jobj::clear_flags_all(&mut r.game.objs.jobjs, child, jobj::HIDDEN);
    change_state(r, gobj, 2, UNK_0X1);
}

/// Hit, `itCoin_Logic116_DmgReceived`: in the air, its damage taken plus the element's, it
/// flies off along the hit's knockback (or up and away from the hit), normalized and scaled
/// by the attributes: across the stage's middle by `x30` toward it or `x2C` away (each times
/// `x24` plus the damage), capped at `x3C`, and up by `x38` times `x28` plus the damage,
/// capped at `x40`; the damage cleared, its fall speed `x8` up by its kind's, and off it flies
/// (state 3).
#[doc(alias = "itCoin_Logic116_DmgReceived")]
pub fn damaged(r: &mut Running, gobj: GObjId) -> bool {
    let a = |r: &Running, off| reg(attr(r, gobj, off));
    maplib::enter_air(item_mut(r.game, gobj));
    let extra = reg(zako::element_damage(r, gobj));
    let ip = item_mut(r.game, gobj);
    ip.damage_taken = fp::fctiwz(fp::fadds(fp::frsp(f64::from(ip.damage_taken)), extra));
    let mut vel = item(r.game, gobj).vel;
    zako::knockback_vel(r, gobj, &mut vel);
    item_mut(r.game, gobj).vel = vel;
    let ip = item_mut(r.game, gobj);
    let zero = 0.0;
    if reg(ip.vel.x) == zero && reg(ip.vel.y) == zero {
        ip.vel.x = mem(fp::fmuls(reg(0.1), fp::fneg(reg(ip.damage_dir))));
        ip.vel.y = 0.1;
    }
    ip.vel.normalize();
    let minus = fp::frsp(-1.0);
    let plus = fp::frsp(1.0);
    let dir = reg(ip.damage_dir);
    let x = reg(ip.pos.x);
    if x > zero && minus == dir {
        ip.vel.x = mem(fp::fmuls(reg(ip.vel.x), minus));
    }
    if x < zero && plus == dir {
        ip.vel.x = mem(fp::fmuls(reg(ip.vel.x), minus));
    }
    let damage = fp::frsp(f64::from(item(r.game, gobj).damage_taken));
    let (base_x, base_y) = (a(r, 0x24), a(r, 0x28));
    let (scale_ahead, scale_behind, scale_y) = (a(r, 0x2C), a(r, 0x30), a(r, 0x38));
    let (max_vx, max_vy) = (a(r, 0x3C), a(r, 0x40));
    let ip = item_mut(r.game, gobj);
    let scaled = |m: f64| fp::fmuls(m, fp::fadds(base_x, damage));
    if x > zero {
        let m = if dir == minus {
            scale_behind
        } else {
            scale_ahead
        };
        ip.vel.x = mem(fp::fmuls(reg(ip.vel.x), scaled(m)));
        if reg(ip.vel.x) <= fp::fneg(max_vx) {
            ip.vel.x = mem(fp::fneg(max_vx));
        }
    } else {
        let m = if minus == dir {
            scale_ahead
        } else {
            scale_behind
        };
        ip.vel.x = mem(fp::fmuls(reg(ip.vel.x), scaled(m)));
        if reg(ip.vel.x) >= max_vx {
            ip.vel.x = mem(max_vx);
        }
    }
    ip.vel.y = mem(fp::fmuls(
        reg(ip.vel.y),
        fp::fmuls(scale_y, fp::fadds(base_y, damage)),
    ));
    if reg(ip.vel.y) >= max_vy {
        ip.vel.y = mem(max_vy);
    }
    ip.vel.z = mem(zero);
    ip.damage_taken = 0;
    let off = match ip.vars.trophy_coin().trophy_kind.get() {
        2 => 0x14,
        3 => 0x20,
        _ => 0x8,
    };
    let add = a(r, off);
    let v = item_mut(r.game, gobj).vars.trophy_coin();
    let spin = reg(v.spin.get());
    v.spin.set(mem(fp::fadds(spin, add)));
    change_state(r, gobj, 3, UNK_0X1);
    false
}

/// Flying off's physics, `itCoin_UnkMotion3_Phys`: its sideways speed kept by the attributes'
/// `x34`, falling at its own speeds.
#[doc(alias = "itCoin_UnkMotion3_Phys")]
pub fn phys3(r: &mut Running, gobj: GObjId) {
    let keep = reg(attr(r, gobj, 0x34));
    let ip = item_mut(r.game, gobj);
    ip.vel.x = mem(fp::fmuls(reg(ip.vel.x), keep));
    let v = ip.vars.trophy_coin();
    let (f, m) = (v.spin.get(), v.spin_max.get());
    lib2725::fall(r.game, gobj, f, m);
}

/// Flying off's collision, `itCoin_UnkMotion3_Coll`: it bounces and lands (`it_8026E15C`).
#[doc(alias = "itCoin_UnkMotion3_Coll")]
pub fn coll3(r: &mut Running, gobj: GObjId) -> bool {
    fall(r, gobj, landed);
    false
}

/// Thrown or dropped, `itCoin_Logic116_Thrown`: unturned, falling (state 4).
#[doc(alias = "itCoin_Logic116_Thrown")]
pub fn thrown(r: &mut Running, gobj: GObjId) {
    unturn(r, gobj);
    change_state(r, gobj, 4, UNK_0X1);
}

/// Falling's physics at the attributes' speeds, `itCoin_UnkMotion4_Phys` (and a trophy's
/// state 0).
#[doc(alias = "itCoin_UnkMotion4_Phys")]
pub fn phys4(r: &mut Running, gobj: GObjId) {
    let a = attr_of(item(r.game, gobj));
    let accel = a.f32(r.data, data::attr::FALL_SPEED);
    let max = a.f32(r.data, data::attr::FALL_SPEED_MAX);
    lib2725::fall(r.game, gobj, accel, max);
}

/// Skidding off a slope, `itCoin_Logic116_EnteredAir`: facing the way it moves (state 5).
#[doc(alias = "itCoin_Logic116_EnteredAir")]
pub fn entered_air(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 5, change::ANIM_UPDATE);
    let ip = item_mut(r.game, gobj);
    let v = fp::fadds(
        reg(ip.slide_vel.x),
        fp::fadds(reg(ip.vel.x), reg(ip.skid_vel.x)),
    );
    ip.facing_dir = if v >= 0.0 { 1.0 } else { -1.0 };
    let dir = if fp::frsp(-1.0) == reg(ip.facing_dir) {
        -1
    } else {
        1
    };
    ip.coll.set_facing_dir(dir);
}

/// Skidding's collision, `itCoin_UnkMotion5_Coll`: it lands or falls (`it_8026E8C4`).
#[doc(alias = "itCoin_UnkMotion5_Coll")]
pub fn coll5(r: &mut Running, gobj: GObjId) -> bool {
    ground_skid(r, gobj, landed, fell);
    false
}

/// Snag the Trophies' coin's collision joint alone, `it_802F2014`.
#[doc(alias = "it_802F2014")]
pub fn set_joint_only(r: &mut Running, gobj: GObjId, joint: i32) {
    item_mut(r.game, gobj).coll.joint_id_only = joint;
}

/// The coin's trophy, `it_802F2020` (Snag the Trophies').
#[doc(alias = "it_802F2020")]
pub fn trophy(r: &mut Running, gobj: GObjId) -> i32 {
    item_mut(r.game, gobj).vars.trophy_coin().trophy.get()
}

/// Snag the Trophies' coin knocked off its stand, `it_802F202C`: the sound 0xAA, no sideways
/// speed, falling no faster than 1.5, blinking.
#[doc(alias = "it_802F202C")]
pub fn knocked_off(r: &mut Running, gobj: GObjId) {
    r.sfx_play(0xAA, 0x7F, 0x40);
    let ip = item_mut(r.game, gobj);
    ip.vel.x = 0.0;
    if reg(ip.vel.y) < fp::fneg(1.5) {
        ip.vel.y = mem(fp::fneg(1.5));
    }
    hitbox::start_blink(r.game, gobj);
}

impl Running<'_> {
    /// The coin's render callback, `it_802F13B4`: its trophy hidden for the question mark
    /// (its model's second child) while the debug pause's second bit is set outside a bonus
    /// stage's end (`Ground_801C1D84`) or the single-player camera's zoom (`Camera_80031144`)
    /// is under the attributes' `x48`, else the question mark hidden; then drawn as an item
    /// (`it_8026EECC`). Snag the Trophies' coins ask their stage object as well
    /// (`grFigureGet_80219C50`).
    #[doc(alias = "it_802F13B4")]
    pub fn render_trophy_coin(&mut self, gobj: GObjId, pass: i32) {
        let min_zoom = reg(attr(self, gobj, 0x48));
        let kind = item_mut(self.game, gobj)
            .vars
            .trophy_coin()
            .trophy_kind
            .get();
        let j = jobj_of(self.game, gobj).and_then(|j| self.game.objs.jobjs[j].child);
        let mark = if kind != 0 {
            // The question mark while the stand's coins aren't all snagged or gone.
            let stand = item_mut(self.game, gobj).vars.trophy_coin().stand.get();
            let zoom = reg(self.game.camera.zoom());
            (self.game.db_pause_flag(1) || zoom < min_zoom)
                && !crate::game::gr::snag_the_trophies::done(self.game, stand)
        } else {
            let zoom = reg(self.game.camera.zoom());
            (self.game.db_pause_flag(1) && crate::game::gr::ground::flags(self.game, 0x330) == 0)
                || zoom < min_zoom
        };
        let next = j.and_then(|j| self.game.objs.jobjs[j].next);
        let jobjs = &mut self.game.objs.jobjs;
        if mark {
            jobj::set_flags_all(jobjs, j, jobj::HIDDEN);
            jobj::clear_flags_all(jobjs, next, jobj::HIDDEN);
        } else {
            jobj::set_flags_all(jobjs, next, jobj::HIDDEN);
            jobj::clear_flags_all(jobjs, j, jobj::HIDDEN);
        }
        self.render_item(gobj, pass);
    }
}
