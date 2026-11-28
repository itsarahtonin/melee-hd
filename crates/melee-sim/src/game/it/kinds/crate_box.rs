//! The box (crate), `melee/it/kinds/itbox.c`: a container that breaks open when thrown hard
//! enough, hit for enough damage or when it hits something, dropping items or nothing. Kongo
//! Jungle's barrels carry floating ones (`it_80286088`). Its rest, falls, pickup, throw and drop,
//! its breaking and its opening are the containers' ([`container`]).

use gekko_fp as fp;

use super::super::item::{change, change_state};
use super::super::vars::ItemVars;
use super::super::{drop, item, item_mut, jobj_of, lib26b1, maplib};
use super::container::{self, Container, Opening};
use crate::fpu::{mem, reg};
use crate::game::handles::CodeRef;
use crate::game::it::ItKind;
use crate::game::state::{Game, Running};
use crate::game::structs::{SpawnItem, itBox_ItemVars};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

pub const SPAWNED: u32 = 0x8028_61CC;
pub const DESTROYED: u32 = 0x8028_6208;
pub const LANDED: u32 = 0x8028_6480;
pub const ANIM0: u32 = 0x8028_64D8;
pub const PHYS0: u32 = 0x8028_6518;
pub const COLL0: u32 = 0x8028_651C;
pub const FELL: u32 = 0x8028_655C;
pub const ANIM4: u32 = 0x8028_6598;
pub const PHYS1: u32 = 0x8028_65A0;
pub const COLL1: u32 = 0x8028_6754;
pub const PICKED_UP: u32 = 0x8028_6780;
pub const ANIM2: u32 = 0x8028_67A8;
pub const PHYS2: u32 = 0x8028_67B0;
pub const THROWN: u32 = 0x8028_67B4;
pub const PHYS4: u32 = 0x8028_67F0;
pub const COLL3: u32 = 0x8028_683C;
pub const DROPPED: u32 = 0x8028_6964;
pub const COLL4: u32 = 0x8028_69A0;
pub const ANIM6: u32 = 0x8028_6B74;
pub const PHYS6: u32 = 0x8028_6B94;
pub const COLL6: u32 = 0x8028_6B98;
pub const ANIM7: u32 = 0x8028_6C7C;
pub const PHYS7: u32 = 0x8028_6CA8;
pub const COLL7: u32 = 0x8028_6CAC;
pub const DMG_DEALT: u32 = 0x8028_6CB4;
pub const CLANKED: u32 = 0x8028_6D48;
pub const HIT_SHIELD: u32 = 0x8028_6DDC;
pub const REFLECTED: u32 = 0x8028_6E70;
pub const DMG_RECEIVED: u32 = 0x8028_6F04;
pub const ENTERED_AIR: u32 = 0x8028_6FC8;
pub const ANIM5: u32 = 0x8028_7044;
pub const PHYS5: u32 = 0x8028_704C;
pub const COLL5: u32 = 0x8028_7050;
pub const ANIM8: u32 = 0x8028_7104;
pub const PHYS8: u32 = 0x8028_710C;
pub const COLL8: u32 = 0x8028_7334;
pub const FOLLOW: u32 = 0x8028_733C;

/// The box: resting, it tilts with the floor; held in 2, thrown into 3 and dropped into 4.
pub const CRATE: Container = Container {
    fell: container::fell,
    landed: container::landed,
    tilts: true,
    held: 2,
    thrown: 3,
    dropped: 4,
    let_go_disables_pickup: true,
};

/// The box's opening: it breaks on a hit, a smash or `x14` damage, a drop past `x1C` smashing
/// it; empty (by its weights, `x0` to `xC`) it bursts in state 6 with the wind, else it opens
/// in 7 with a crash (0xF6), spilling its items by its weights.
pub const OPENING: Opening = Opening {
    name: "box",
    vars: shared,
    breaks: break_open,
    smashes: break_open,
    damage_limit: 0x14,
    drop_limit: 0x1C,
    settles: container::landed,
    has_items,
    burst_state: 6,
    burst_wind: true,
    open_sfx: 0xF6,
    spills: Some(spill),
    open_frames: 40,
    open_state: 7,
};

/// Runs one of the box's predicates, if `addr` is one.
#[melee_callbacks::callbacks(Pred)]
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 => container::smokes(r, gobj),
        COLL0 => container::rests(r, gobj, &CRATE),
        ANIM4 | ANIM2 | ANIM5 | ANIM8 | COLL6 | COLL7 | COLL8 => container::still(r, gobj),
        COLL1 => container::lands(r, gobj, &CRATE),
        COLL3 => container::flies(r, gobj, &OPENING),
        COLL4 => container::drops(r, gobj, &OPENING),
        ANIM6 => container::blast_lasts(r, gobj),
        ANIM7 => container::gone_after(r, gobj, &OPENING),
        DMG_DEALT | CLANKED | HIT_SHIELD | REFLECTED => container::hit(r, gobj, &OPENING),
        DMG_RECEIVED => container::damaged(r, gobj, &OPENING),
        COLL5 => container::skids(r, gobj, &CRATE),
        _ => return None,
    })
}

/// Runs one of the box's events, if `addr` is one.
#[melee_callbacks::callbacks(Proc)]
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        DESTROYED => destroyed(r, gobj),
        LANDED => container::landed(r, gobj),
        PHYS0 | PHYS2 | PHYS6 | PHYS7 | PHYS5 => container::idle(r, gobj),
        FELL => container::fell(r, gobj),
        PHYS1 => phys1(r, gobj),
        PICKED_UP => container::picked_up(r, gobj, &CRATE),
        THROWN => container::thrown(r, gobj, &CRATE),
        PHYS4 => container::thrown_phys(r, gobj),
        DROPPED => container::dropped(r, gobj, &CRATE),
        ENTERED_AIR => entered_air(r, gobj),
        PHYS8 => phys8(r, gobj),
        FOLLOW => follow(r, gobj),
        _ => return false,
    }
    true
}

/// The box's variables.
fn vars(game: &mut Game, gobj: GObjId) -> &mut itBox_ItemVars {
    item_mut(game, gobj).vars.crate_box()
}

/// The fields the barrel and the party ball have too.
fn shared(v: &mut ItemVars) -> container::Vars<'_> {
    let b = v.crate_box();
    container::Vars {
        opened: &mut b.opened,
        timer: &mut b.despawn_timer,
    }
}

/// `itBoxAttributes`' float at `off`.
fn attr_f(r: &Running, gobj: GObjId, off: u32) -> f32 {
    container::attr_f(r, gobj, &OPENING, off)
}

/// Makes a box floating with a barrel (`parent`), `it_80286088`: it spins at a random rate
/// and follows the barrel.
#[doc(alias = "it_80286088")]
pub fn spawn_floating(r: &mut Running, parent: Option<GObjId>) -> Option<GObjId> {
    let pj = parent.and_then(|p| jobj_of(r.game, p))?;
    let pos = crate::lb::joint::world_position(&mut r.game.objs, r.data, Some(pj), None);
    let mut spawn = SpawnItem {
        kind: ItKind::Crate,
        ..Default::default()
    };
    // `Item_InitSpawnPosition`.
    spawn.prev_pos = pos;
    spawn.pos = pos;
    spawn.facing_dir = 1.0;
    spawn.damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.owner = None;
    spawn.maker = None;
    spawn.flags[0] &= !0x80;
    spawn.ignore_id = 0;
    let gobj = super::super::item::spawn_air(r, &mut spawn)?;
    vars(r.game, gobj).spawned_gobj.set(parent);
    let scale = 0.034_906_584_769_487_38;
    for i in 0..2 {
        let f = r.game.objs.rng.draw_f32();
        let v = mem(fp::frsp(fp::fmul(fp::fsubs(reg(f), 0.5), scale)));
        if i == 0 {
            vars(r.game, gobj).rot_vel_x.set(v);
        } else {
            vars(r.game, gobj).rot_vel_y.set(v);
        }
    }
    item_mut(r.game, gobj).flags.flags.turns_to_face = false;
    lib26b1::stop_keeping_clear(r.game, gobj);
    float(r, gobj);
    Some(gobj)
}

/// `itBox_Logic1_Spawned`.
#[doc(alias = "itBox_Logic1_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    super::super::set_flag(&mut item_mut(r.game, gobj).hurt_flags, 7, false);
    let v = vars(r.game, gobj);
    v.opened.set(0);
    v.spawned_gobj.set(None);
    container::fell(r, gobj);
}

/// `itBox_Logic1_Destroyed`: a barrel's box lets go of it.
#[doc(alias = "itBox_Logic1_Destroyed")]
pub fn destroyed(r: &mut Running, gobj: GObjId) {
    if let Some(path) = vars(r.game, gobj).spawned_gobj.get() {
        crate::game::gr::kongo_jungle::destroy_path(r, path);
        vars(r.game, gobj).spawned_gobj.set(None);
    }
}

/// Drops what's in the box, `it_80286248`: coins in a coin match, else a monster if one comes
/// out, else 1, 2 or 3 items by the weights (all one kind on a 1 in `special` draw).
#[doc(alias = "it_80286248")]
pub fn drop_items(r: &mut Running, gobj: GObjId, w1: i32, w2: i32, w3: i32, special: i32) {
    let pos = item(r.game, gobj).pos;
    if drop::drop_coins(r, gobj, pos, None) {
        return;
    }
    if drop::drop_monster(r, Some(gobj), pos, None).is_some() {
        return;
    }
    let same = r.game.objs.rng.below(special) == 0;
    let sum = w1.wrapping_add(w2);
    let roll = r.game.objs.rng.below(w3.wrapping_add(sum));
    let n = if roll < w1 {
        1
    } else if roll < sum {
        2
    } else {
        3
    };
    drop::drop_contents(r, gobj, n, same);
}

/// Its items as it opens (`it_80286248` over the attributes' weights `x0`, `x4`, `x8` and the
/// all-one-kind draw `x10`).
fn spill(r: &mut Running, gobj: GObjId) {
    let w = [0, 4, 8, 0x10].map(|o| container::attr_i(r, gobj, &OPENING, o));
    drop_items(r, gobj, w[0], w[1], w[2], w[3]);
}

/// Whether it breaks with items in it: [`container::has_items`] over the attributes' weights
/// `x0` to `x8` and the empty one `xC`.
fn has_items(r: &mut Running, gobj: GObjId) -> bool {
    let w: [i32; 4] = std::array::from_fn(|i| container::attr_i(r, gobj, &OPENING, 4 * i as u32));
    container::has_items(r, w[0], w[1], w[2], w[3])
}

/// Breaks: its splinters, then opens with items or empty (the file's inline
/// `itBox_TryOpen_inline`).
fn break_open(r: &mut Running, gobj: GObjId) {
    container::break_open(r, gobj, &OPENING);
}

/// Whether it landed on a floor near enough upright to stop, `it_802863BC`, stopping its
/// tilting if so.
#[doc(alias = "it_802863BC")]
pub fn lands_upright(r: &mut Running, gobj: GObjId) -> bool {
    let limit = attr_f(r, gobj, 0x18);
    let ip = item_mut(r.game, gobj);
    if ip.coll.env_flags & (crate::mp::coll::FLOOR_PUSH | crate::mp::coll::FLOOR_HUG) == 0 {
        return false;
    }
    let dir = maplib::surface_normal(&ip.coll).expect("a floor's normal");
    if reg(dir.angle_xy(Vec3::new(0.0, 1.0, 0.0))) < reg(limit) {
        let z = Vec3::new(0.0, 0.0, 0.0);
        ip.skid_vel = z;
        ip.slide_vel = z;
        ip.last_slide_vel = z;
        ip.unread_a0 = z;
        ip.skid = 1;
        return true;
    }
    false
}

/// Wraps a rotation past a turn either way.
fn wrap_turn(v: f32) -> f32 {
    let tau = std::f64::consts::TAU;
    if reg(v) > tau {
        mem(fp::frsp(fp::fsub(reg(v), tau)))
    } else if reg(v) < -tau {
        mem(fp::frsp(fp::fadd(reg(v), tau)))
    } else {
        v
    }
}

/// Turns the model by the spin rates about x and y.
fn spin(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj).expect("a box without a model");
    let (vx, vy) = {
        let v = vars(r.game, gobj);
        (v.rot_vel_x.get(), v.rot_vel_y.get())
    };
    let mut q = r.game.objs.jobjs[j].rotate;
    q.x = wrap_turn(mem(fp::fadds(reg(q.x), reg(vx))));
    q.y = wrap_turn(mem(fp::fadds(reg(q.y), reg(vy))));
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.rotate = q);
}

/// `itBox_UnkMotion1_Phys`: gravity, and a barrel's box tumbles.
#[doc(alias = "itBox_UnkMotion1_Phys")]
pub fn phys1(r: &mut Running, gobj: GObjId) {
    super::gravity(r, gobj);
    if vars(r.game, gobj).spawned_gobj.get().is_some() {
        let ip = item_mut(r.game, gobj);
        ip.vel.z = mem(fp::fmuls(reg(ip.vel.z), f64::from(0.95_f32)));
        spin(r, gobj);
    }
}

/// `itBox_Logic1_EnteredAir`: stops if it lands upright, else skids (state 5).
#[doc(alias = "itBox_Logic1_EnteredAir")]
pub fn entered_air(r: &mut Running, gobj: GObjId) {
    if lands_upright(r, gobj) {
        lib26b1::enable_pickup(r.game, gobj);
        item_mut(r.game, gobj).vel = Vec3::new(0.0, 0.0, 0.0);
        change_state(r, gobj, 0, 2);
    } else {
        change_state(r, gobj, 5, 2);
    }
}

/// Floats with its barrel (state 8), `it_802870A4`.
#[doc(alias = "it_802870A4")]
pub fn float(r: &mut Running, gobj: GObjId) {
    maplib::enter_ground(item_mut(r.game, gobj));
    lib26b1::disable_pickup(r.game, gobj);
    change_state(r, gobj, 8, change::ANIM_UPDATE);
    item_mut(r.game, gobj).on_accessory = Some(CodeRef(FOLLOW));
}

/// Nudges a spin rate at random, capped at 3 degrees a frame.
fn nudge(r: &mut Running, v: f32) -> f32 {
    let deg = 0.017_453_292_384_743_69;
    let max = 0.052_359_877_154_231_07;
    let f = r.game.objs.rng.draw_f32();
    let n = mem(fp::frsp(fp::fmadd(
        deg,
        fp::fsubs(reg(f), f64::from(0.2_f32)),
        reg(v),
    )));
    if reg(n) > max {
        mem(fp::frsp(max))
    } else if reg(n) < -max {
        mem(fp::frsp(-max))
    } else {
        n
    }
}

/// `itBox_UnkMotion8_Phys`: floating, it tumbles at randomly changing rates.
#[doc(alias = "itBox_UnkMotion8_Phys")]
pub fn phys8(r: &mut Running, gobj: GObjId) {
    let vx = vars(r.game, gobj).rot_vel_x.get();
    let vx = nudge(r, vx);
    vars(r.game, gobj).rot_vel_x.set(vx);
    let vy = vars(r.game, gobj).rot_vel_y.get();
    let vy = nudge(r, vy);
    vars(r.game, gobj).rot_vel_y.set(vy);
    spin(r, gobj);
}

/// Follows its barrel, `it_8028733C`, falling once the barrel comes out in front (or is gone).
#[doc(alias = "it_8028733C")]
// NaN keeps following, as the original's `>= 0` test has it.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
pub fn follow(r: &mut Running, gobj: GObjId) {
    let parent = vars(r.game, gobj).spawned_gobj.get();
    if let Some(p) = parent {
        let j = jobj_of(r.game, p).expect("a barrel without a model");
        let target = r.game.objs.jobjs[j].translate;
        let ip = item_mut(r.game, gobj);
        let vel = Vec3::new(
            mem(fp::fsubs(reg(target.x), reg(ip.pos.x))),
            mem(fp::fsubs(reg(target.y), reg(ip.pos.y))),
            mem(fp::fsubs(reg(target.z), reg(ip.pos.z))),
        );
        ip.pos = target;
        if !(reg(target.z) >= 0.0) {
            return;
        }
        ip.vel = vel;
    }
    let ip = item_mut(r.game, gobj);
    let pos = ip.pos;
    ip.coll.set_position(pos);
    lib26b1::disable_pickup(r.game, gobj);
    change_state(r, gobj, 1, 2);
}
