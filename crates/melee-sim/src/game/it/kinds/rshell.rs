//! The red shell, `melee/it/kinds/itrshell.c`: a shell that chases the nearest fighter along
//! the ground, turning at ledges and walls, until it has hit `x54` times or its time runs out.
//! The green shell's code ([`super::shell`]) with its own chase, bounces and hits.

use crate::mp::coll::GroundOrAir;
use gekko_fp as fp;

use super::super::ground::ground_to_ledge;
use super::super::item::{change, change_state, common_data, play_sfx};
use super::super::vars::ItemVars;
use super::super::{coll, hitbox, item, item_mut, lib26b1, lib2725, maplib, owner};
use super::shell::{self, Shell, attr, to_f32};
use crate::fpu::{mem, reg};
use crate::game::handles::CodeRef;
use crate::game::state::{Game, Running};
use crate::game::structs::itRShell_ItemVars;
use crate::hsd::gobj::GObjId;
use crate::lb::hitcapsule;
use crate::vec::Vec3;

pub const TRAIL: u32 = 0x8028_CFE0;
pub const LANDED: u32 = 0x8028_D090;
pub const AT_LEDGE: u32 = 0x8028_D4A8;
pub const SPAWNED: u32 = 0x8028_D59C;
pub const STOPPED: u32 = 0x8028_D62C;
pub const ANIM0: u32 = 0x8028_D6C0;
pub const PHYS0: u32 = 0x8028_D6C8;
pub const COLL0: u32 = 0x8028_D6CC;
pub const FELL: u32 = 0x8028_D7F0;
pub const ANIM1: u32 = 0x8028_D818;
pub const PHYS1: u32 = 0x8028_D820;
pub const COLL1: u32 = 0x8028_D850;
pub const PICKED_UP: u32 = 0x8028_D898;
pub const ANIM2: u32 = 0x8028_D8D4;
pub const PHYS2: u32 = 0x8028_D8DC;
pub const THROWN: u32 = 0x8028_D8E0;
pub const ANIM3: u32 = 0x8028_D940;
pub const PHYS3: u32 = 0x8028_D998;
pub const COLL3: u32 = 0x8028_D9C8;
pub const DROPPED: u32 = 0x8028_D9F4;
pub const ANIM4: u32 = 0x8028_DA30;
pub const PHYS4: u32 = 0x8028_DA88;
pub const COLL4: u32 = 0x8028_DAB8;
pub const CHASE: u32 = 0x8028_DAE4;
pub const ANIM5: u32 = 0x8028_DC5C;
pub const PHYS5: u32 = 0x8028_DD88;
pub const COLL5: u32 = 0x8028_DFE8;
pub const CHASE_AIR: u32 = 0x8028_E170;
pub const ANIM6: u32 = 0x8028_E2D4;
pub const PHYS6: u32 = 0x8028_E370;
pub const COLL6: u32 = 0x8028_E3A0;
pub const ENTERED_AIR: u32 = 0x8028_E3CC;
pub const ANIM7: u32 = 0x8028_E408;
pub const PHYS7: u32 = 0x8028_E410;
pub const COLL7: u32 = 0x8028_E414;
pub const DMG_DEALT: u32 = 0x8028_E540;
pub const DMG_RECEIVED: u32 = 0x8028_E584;
pub const REFLECTED: u32 = 0x8028_E5C8;
pub const CLANKED: u32 = 0x8028_E600;
pub const HIT_SHIELD: u32 = 0x8028_E624;
pub const SHIELD_BOUNCED: u32 = 0x8028_E684;
pub const JUMPED_ON: u32 = 0x8028_E6C0;

/// The red shell: its dust every `x44` frames, its roll at `x38`, its hit delay (`x3C`)
/// started over by a throw or a drop, its skid state 7, and no pop-ups.
pub const RED: Shell = Shell {
    vars: shared,
    trail: 0x44,
    roll: 0x38,
    hit_delay_on_throw: Some(0x3C),
    skid: 7,
    stopped,
    fell,
    landed,
};

/// Runs one of the shell's predicates, if `addr` is one.
#[melee_callbacks::callbacks(Pred)]
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 | ANIM1 | ANIM2 | ANIM7 => shell::still(r, gobj),
        COLL0 => shell::coll0(r, gobj, &RED),
        COLL1 => shell::coll1(r, gobj, &RED),
        ANIM3 | ANIM4 => anim3(r, gobj),
        COLL3 | COLL4 | COLL6 => shell::coll3(r, gobj, &RED),
        ANIM5 => anim5(r, gobj),
        COLL5 => coll5(r, gobj),
        ANIM6 => anim6(r, gobj),
        COLL7 => shell::skid_coll(r, gobj, &RED),
        DMG_DEALT => dmg_dealt(r, gobj),
        DMG_RECEIVED => dmg_received(r, gobj),
        REFLECTED => shell::reflected(r, gobj, &RED),
        CLANKED => clanked(r, gobj),
        HIT_SHIELD => hit_shield(r, gobj),
        SHIELD_BOUNCED => shell::shield_bounced(r, gobj),
        JUMPED_ON => jumped_on(r, gobj),
        _ => return None,
    })
}

/// Runs one of the shell's events, if `addr` is one.
#[melee_callbacks::callbacks(Proc)]
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        TRAIL => shell::trail(r, gobj, &RED),
        LANDED => landed(r, gobj),
        AT_LEDGE => turn_back(r, gobj),
        SPAWNED => spawned(r, gobj),
        STOPPED => stopped(r, gobj),
        PHYS0 | PHYS2 | PHYS7 => shell::idle(r, gobj),
        FELL => fell(r, gobj),
        PHYS1 | PHYS3 | PHYS4 | PHYS6 => shell::phys_fall(r, gobj),
        PICKED_UP => shell::picked_up(r, gobj, &RED),
        THROWN => shell::thrown(r, gobj, &RED),
        DROPPED => shell::dropped(r, gobj, &RED),
        CHASE => chase(r, gobj),
        PHYS5 => phys5(r, gobj),
        CHASE_AIR => chase_air(r, gobj),
        ENTERED_AIR => shell::entered_air(r, gobj, &RED),
        _ => return false,
    }
    true
}

/// The shell's variables.
fn vars(game: &mut Game, gobj: GObjId) -> &mut itRShell_ItemVars {
    item_mut(game, gobj).vars.rshell()
}

/// The fields the green shell has too.
fn shared(v: &mut ItemVars) -> shell::Vars<'_> {
    let s = v.rshell();
    shell::Vars {
        life: &mut s.life,
        hit_delay: &mut s.hit_delay,
        trail: &mut s.trail,
        landings: &mut s.landings,
        bounced: &mut s.bounced,
        popped: None,
    }
}

/// The sign of `v` as the original takes it: -1 below zero, else 1.
fn sign(v: f64) -> i32 {
    if v < 0.0 { -1 } else { 1 }
}

/// Lands from a throw: chases at the attributes' launch speed, `it_8028D090`.
#[doc(alias = "it_8028D090")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    let launch = attr(r, gobj, 4);
    let b = item(r.game, gobj).kind_coll_box.clone();
    maplib::set_box(r.game, gobj, b);
    chase(r, gobj);
    let facing = item(r.game, gobj).facing_dir;
    item_mut(r.game, gobj).vel.x = launch_speed(r, gobj, launch, facing);
    lib2725::face_motion(r.game, gobj);
}

/// Where the hit leaves the shell's speed: `set` replaces it (`did_hit`), `add` adds to it.
enum Push {
    Set,
    Add,
    None,
}

/// Which push a state takes (the switch `it_8028D100` and `it_8028D26C` share).
fn push_for(msid: i32) -> Push {
    match msid {
        0 | 1 | 3 | 4 | 7 => Push::Set,
        5 | 6 => Push::Add,
        _ => Push::None,
    }
}

/// Goes on chasing when fast enough after a hit, else drops its hitboxes.
fn after_push(r: &mut Running, gobj: GObjId, did_hit: bool, take: fn(&mut Game, GObjId)) {
    let chase_speed = attr(r, gobj, 0x10);
    if reg(item(r.game, gobj).vel.x).abs() > reg(chase_speed) {
        take(r.game, gobj);
        if did_hit {
            hitbox::start_blink(r.game, gobj);
        }
        if item(r.game, gobj).ground_or_air == GroundOrAir::Air {
            chase_air(r, gobj);
        } else {
            chase(r, gobj);
        }
        return;
    }
    lib2725::clear_hitboxes(r.game, r.data, gobj);
}

/// Hit: knocked the way the hit pushed it, `it_8028D100`.
#[doc(alias = "it_8028D100")]
pub fn knocked(r: &mut Running, gobj: GObjId) {
    let msid = item(r.game, gobj).msid;
    let push = push_for(msid);
    let off = if matches!(push, Push::Set) {
        0x20
    } else {
        0x24
    };
    let mult = attr(r, gobj, off);
    if matches!(push, Push::Add) {
        count_hit(r, gobj);
    }
    let ip = item_mut(r.game, gobj);
    let p = fp::fmuls(to_f32(ip.hit_damage), reg(mult));
    let away = -reg(ip.damage_dir);
    match push {
        Push::Set => ip.vel.x = mem(fp::fmuls(away, p)),
        Push::Add => ip.vel.x = mem(fp::fmadds(away, p, reg(ip.vel.x))),
        Push::None => {}
    }
    after_push(r, gobj, matches!(push, Push::Set), |g, o| {
        coll::take_attacker(g, o);
    });
}

/// Jumped on: kicked away from the jumper, `it_8028D26C`.
#[doc(alias = "it_8028D26C")]
pub fn stomped(r: &mut Running, gobj: GObjId) {
    let msid = item(r.game, gobj).msid;
    let push = push_for(msid);
    let off = if matches!(push, Push::Set) {
        0x28
    } else {
        0x2C
    };
    let mult = attr(r, gobj, off);
    if matches!(push, Push::Add) {
        count_hit(r, gobj);
    }
    let ip = item_mut(r.game, gobj);
    let away = -reg(ip.stomp_dir);
    match push {
        Push::Set => ip.vel.x = mem(fp::fmuls(reg(mult), away)),
        Push::Add => ip.vel.x = mem(fp::fmadds(reg(mult), away, reg(ip.vel.x))),
        Push::None => {}
    }
    after_push(r, gobj, matches!(push, Push::Set), |g, o| {
        coll::take_stomper(g, o);
    });
}

/// Counts a hit off the ones it has left; at none it's spent (`xDF0`), `it_8028D390`.
#[doc(alias = "it_8028D390")]
pub fn count_hit(r: &mut Running, gobj: GObjId) {
    let v = vars(r.game, gobj);
    v.hits_left.update(|old| old.wrapping_sub(1));
    if v.hits_left.get() == 0 {
        v.hit.set(1);
    }
}

/// Hit something: unless spent, it bounces back off it, `it_8028D3B8`.
#[doc(alias = "it_8028D3B8")]
pub fn bounced(r: &mut Running, gobj: GObjId) {
    count_hit(r, gobj);
    if vars(r.game, gobj).hit.get() != 0 {
        return;
    }
    hitbox::end_blink(r.game, gobj);
    hitbox::set_hit_ids(r.game, gobj);
    let ip = item(r.game, gobj);
    let (vx, dir) = (ip.vel.x, ip.out_damage_dir);
    item_mut(r.game, gobj).vel.x = launch_speed(r, gobj, vx, dir);
    lib2725::face_motion(r.game, gobj);
    shell::draw_landings(r, gobj, &RED);
    let ip = item(r.game, gobj);
    if ip.msid != 6 && ip.msid != 5 {
        if ip.ground_or_air == GroundOrAir::Air {
            chase_air(r, gobj);
            return;
        }
        chase(r, gobj);
    }
}

/// Turns back at a ledge, `fn_8028D4A8`.
#[doc(alias = "fn_8028D4A8")]
pub fn turn_back(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    ip.vel.x = -ip.vel.x;
    let v = vars(r.game, gobj);
    v.accel.update(|old| -old);
    lib2725::face_motion(r.game, gobj);
}

/// Caps the speed at `xC`, `it_8028D4E4` (and the file's inline `itRshell_ClampVel`).
#[doc(alias = "it_8028D4E4")]
pub fn cap_speed(r: &mut Running, gobj: GObjId) {
    let max_speed = attr(r, gobj, 0xC);
    let ip = item_mut(r.game, gobj);
    let vx = reg(ip.vel.x);
    if vx.abs() > reg(max_speed) {
        ip.vel.x = mem(fp::fmuls(to_f32(sign(vx)), reg(max_speed)));
    }
}

/// A speed from `a` and `b` through the attributes' `x14` vector, `it_8028D56C`:
/// `(a * -x14.x + -x14.y * b) * x14.z`.
#[doc(alias = "it_8028D56C")]
pub fn launch_speed(r: &Running, gobj: GObjId, a: f32, b: f32) -> f32 {
    let (x, y, z) = (
        attr(r, gobj, 0x14),
        attr(r, gobj, 0x18),
        attr(r, gobj, 0x1C),
    );
    let t = fp::fmuls(-reg(y), reg(b));
    mem(fp::fmuls(fp::fmadds(reg(a), -reg(x), t), reg(z)))
}

/// `it_3F14_Logic15_Spawned`.
#[doc(alias = "it_3F14_Logic15_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    let hit_delay = attr(r, gobj, 0x3C);
    let hits = r.data.u32(shell::attr_ref(r, gobj, 0x54)) as i32;
    shell::start_life(r, gobj, &RED);
    let v = vars(r.game, gobj);
    v.hits_left.set(hits);
    v.hit.set(0);
    v.hit_delay.set(hit_delay);
    v.landings.set(1);
    v.bounced.set(false);
    v.accel.set(0.0);
    fell(r, gobj);
}

/// Comes to a stop, stompable, `it_8028D62C`.
#[doc(alias = "it_8028D62C")]
pub fn stopped(r: &mut Running, gobj: GObjId) {
    let hit_delay = attr(r, gobj, 0x3C);
    lib26b1::enable_pickup(r.game, gobj);
    hitbox::let_own_items_hit(r.game, gobj);
    hitbox::end_blink(r.game, gobj);
    item_mut(r.game, gobj).vel = Vec3::new(0.0, 0.0, 0.0);
    vars(r.game, gobj).hit_delay.set(hit_delay);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    lib2725::make_stompable(r.game, gobj);
    item_mut(r.game, gobj).jumped_on = Some(CodeRef(JUMPED_ON));
}

/// Falls (state 1), `it_8028D7F0`.
#[doc(alias = "it_8028D7F0")]
pub fn fell(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE);
}

/// `itRshell_UnkMotion3_Anim` and 4: its hits come back after its delay.
#[doc(alias = "itRshell_UnkMotion3_Anim")]
#[doc(alias = "itRshell_UnkMotion4_Anim")]
pub fn anim3(r: &mut Running, gobj: GObjId) -> bool {
    shell::count_hit_delay(r, gobj, &RED);
    false
}

/// Starts chasing in `msid` (the file's inline `itRshell_StopInit`, the speed capped).
fn start_chase(r: &mut Running, gobj: GObjId, msid: i32) {
    let (hit_delay, trail) = (attr(r, gobj, 0x3C), attr(r, gobj, 0x44));
    lib26b1::disable_pickup(r.game, gobj);
    lib2725::clear_stomp(r.game, gobj);
    let life = common_data(r.game).u32(r.data, super::super::data::common::LIFETIME);
    lib2725::set_lifetime(r.game, r.data, gobj, mem(fp::frsp(f64::from(life))));
    item_mut(r.game, gobj).skid = 0;
    let v = vars(r.game, gobj);
    v.hit_delay.set(hit_delay);
    v.chase_delay.set(0.0);
    v.trail.set(trail);
    hitbox::let_own_items_hit(r.game, gobj);
    cap_speed(r, gobj);
    lib2725::face_motion(r.game, gobj);
    let flags = if matches!(item(r.game, gobj).msid, 0 | 1 | 7) {
        change::ANIM_UPDATE
    } else {
        change::ANIM_UPDATE | change::HIT_PRESERVE
    };
    change_state(r, gobj, msid, flags);
}

/// Chases on the ground, `it_8028DAE4`.
#[doc(alias = "it_8028DAE4")]
pub fn chase(r: &mut Running, gobj: GObjId) {
    item_mut(r.game, gobj).vel.y = 0.0;
    start_chase(r, gobj, 5);
}

/// `itRshell_UnkMotion5_Anim` (6): its time runs out, its hits come back, and it trails dust
/// on the ground.
#[doc(alias = "itRshell_UnkMotion5_Anim")]
pub fn anim5(r: &mut Running, gobj: GObjId) -> bool {
    let v = vars(r.game, gobj);
    if reg(v.life.get()) <= 0.0 {
        return true;
    }
    v.life.update(|old| mem(fp::fsubs(reg(old), 1.0)));
    shell::count_hit_delay(r, gobj, &RED);
    if item(r.game, gobj).msid == 5 {
        shell::trail(r, gobj, &RED);
    }
    false
}

/// Speeds up by `xDE0`, capping the speed once it runs the same way.
fn accelerate(r: &mut Running, gobj: GObjId) {
    let accel = vars(r.game, gobj).accel.get();
    let ip = item_mut(r.game, gobj);
    ip.vel.x = mem(fp::fadds(reg(ip.vel.x), reg(accel)));
    if sign(reg(ip.vel.x)) == sign(reg(accel)) {
        cap_speed(r, gobj);
    }
}

/// `itRshell_UnkMotion5_Phys`: every `x40` frames it turns its acceleration towards the
/// nearest fighter.
#[doc(alias = "itRshell_UnkMotion5_Phys")]
pub fn phys5(r: &mut Running, gobj: GObjId) {
    if reg(vars(r.game, gobj).chase_delay.get()) <= 0.0 {
        let pos = item(r.game, gobj).pos;
        if let Some((_, target)) = owner::nearest_fighter(r.game, r.data, pos, None) {
            let accel = attr(r, gobj, 8);
            let dir = sign(fp::fsubs(reg(target.x), reg(pos.x)));
            vars(r.game, gobj)
                .accel
                .set(mem(fp::fmuls(reg(accel), to_f32(dir))));
            accelerate(r, gobj);
            let chase_speed = attr(r, gobj, 0x10);
            let ip = item(r.game, gobj);
            if ip.hitboxes[0].hit.state == hitcapsule::DISABLED
                && reg(ip.vel.x).abs() <= reg(chase_speed)
            {
                lib2725::start_hitbox(r.game, r.data, gobj, 0);
            }
            lib2725::face_motion(r.game, gobj);
        }
        let chase_delay = attr(r, gobj, 0x40);
        vars(r.game, gobj).chase_delay.set(chase_delay);
    } else {
        accelerate(r, gobj);
        lib2725::face_motion(r.game, gobj);
        let v = vars(r.game, gobj);
        v.chase_delay.update(|old| mem(fp::fsubs(reg(old), 1.0)));
    }
}

/// `itRshell_UnkMotion5_Coll`: turns back at ledges and walls, and chases through the air
/// off the ground.
#[doc(alias = "itRshell_UnkMotion5_Coll")]
pub fn coll5(r: &mut Running, gobj: GObjId) -> bool {
    if !ground_to_ledge(r, gobj, turn_back) {
        maplib::enter_air(item_mut(r.game, gobj));
        chase_air(r, gobj);
        return false;
    }
    if maplib::touched_wall_normal(r.game, gobj).0 != 0 {
        let ip = item_mut(r.game, gobj);
        ip.vel.x = -ip.vel.x;
        let v = vars(r.game, gobj);
        v.accel.update(|old| -old);
        lib2725::face_motion(r.game, gobj);
        hitbox::set_hit_ids(r.game, gobj);
    }
    shell::roll(r, gobj, &RED);
    false
}

/// Chases through the air, `it_8028E170`.
#[doc(alias = "it_8028E170")]
pub fn chase_air(r: &mut Running, gobj: GObjId) {
    start_chase(r, gobj, 6);
}

/// `itRshell_UnkMotion6_Anim`.
#[doc(alias = "itRshell_UnkMotion6_Anim")]
pub fn anim6(r: &mut Running, gobj: GObjId) -> bool {
    anim5(r, gobj);
    false
}

/// `it_3F14_Logic15_DmgDealt`: whether it's spent.
#[doc(alias = "it_3F14_Logic15_DmgDealt")]
pub fn dmg_dealt(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::show(r.game, gobj);
    bounced(r, gobj);
    vars(r.game, gobj).hit.get() != 0
}

/// `it_3F14_Logic15_DmgReceived`: whether it's spent.
#[doc(alias = "it_3F14_Logic15_DmgReceived")]
pub fn dmg_received(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::show(r.game, gobj);
    knocked(r, gobj);
    vars(r.game, gobj).hit.get() != 0
}

/// `itRShell_Logic15_Clanked`.
#[doc(alias = "itRShell_Logic15_Clanked")]
pub fn clanked(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::bounce_off_victim(r.game, r.data, gobj);
    false
}

/// `it_3F14_Logic15_HitShield`: a thrown shell bounces off; a chasing one counts a hit
/// (whether it's spent).
#[doc(alias = "it_3F14_Logic15_HitShield")]
pub fn hit_shield(r: &mut Running, gobj: GObjId) -> bool {
    match item(r.game, gobj).msid {
        3 | 4 => lib2725::bounce_off_victim(r.game, r.data, gobj),
        5 | 6 => {
            bounced(r, gobj);
            return vars(r.game, gobj).hit.get() != 0;
        }
        _ => {}
    }
    false
}

/// Jumped on, `it_8028E6C0`: whether it's spent.
#[doc(alias = "it_8028E6C0")]
pub fn jumped_on(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::show(r.game, gobj);
    stomped(r, gobj);
    play_sfx(r, gobj, 0xF1, 0x7F, 0x40);
    vars(r.game, gobj).hit.get() != 0
}
