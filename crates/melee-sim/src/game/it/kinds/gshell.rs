//! The green shell, `melee/it/kinds/itgshell.c`: kicked, jumped on or hit, it slides along the
//! ground hitting what it meets, bouncing off walls, until it slows to a stop or its time runs
//! out. The red shell's code ([`super::shell`]) with its own slide, pop-ups and hits.

use gekko_fp as fp;

use super::super::ground::ground_or_fall;
use super::super::item::{change, change_state, common_data, play_sfx};
use super::super::vars::ItemVars;
use super::super::{coll, hitbox, item, item_mut, lib26b1, lib2725, maplib};
use super::shell::{self, Shell, attr, to_f32};
use crate::fpu::{mem, reg};
use crate::game::handles::CodeRef;
use crate::game::state::{Game, Running};
use crate::game::structs::itGShell_ItemVars;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

pub const TRAIL: u32 = 0x8028_B8D8;
pub const SPAWNED: u32 = 0x8028_BDAC;
pub const STOPPED: u32 = 0x8028_BE54;
pub const ANIM0: u32 = 0x8028_BEE8;
pub const PHYS0: u32 = 0x8028_BEF0;
pub const COLL0: u32 = 0x8028_BEF4;
pub const FELL: u32 = 0x8028_C018;
pub const ANIM1: u32 = 0x8028_C054;
pub const PHYS1: u32 = 0x8028_C0D0;
pub const COLL1: u32 = 0x8028_C100;
pub const PICKED_UP: u32 = 0x8028_C148;
pub const ANIM2: u32 = 0x8028_C1A0;
pub const PHYS2: u32 = 0x8028_C1A8;
pub const THROWN: u32 = 0x8028_C1AC;
pub const ANIM3: u32 = 0x8028_C1FC;
pub const PHYS3: u32 = 0x8028_C2A4;
pub const COLL3: u32 = 0x8028_C2D4;
pub const DROPPED: u32 = 0x8028_C300;
pub const ANIM4: u32 = 0x8028_C328;
pub const PHYS4: u32 = 0x8028_C34C;
pub const COLL4: u32 = 0x8028_C37C;
pub const SLIDE: u32 = 0x8028_C3A8;
pub const ANIM6: u32 = 0x8028_C52C;
pub const PHYS6: u32 = 0x8028_C6E4;
pub const COLL6: u32 = 0x8028_C75C;
pub const SLIDE_AIR: u32 = 0x8028_C898;
pub const ANIM8: u32 = 0x8028_CA14;
pub const PHYS8: u32 = 0x8028_CAC8;
pub const COLL8: u32 = 0x8028_CAF8;
pub const ENTERED_AIR: u32 = 0x8028_CB24;
pub const ANIM9: u32 = 0x8028_CB70;
pub const PHYS9: u32 = 0x8028_CB78;
pub const COLL9: u32 = 0x8028_CB7C;
pub const DMG_DEALT: u32 = 0x8028_CCA8;
pub const DMG_RECEIVED: u32 = 0x8028_CCCC;
pub const REFLECTED: u32 = 0x8028_CD04;
pub const CLANKED: u32 = 0x8028_CD3C;
pub const HIT_SHIELD: u32 = 0x8028_CE1C;
pub const SHIELD_BOUNCED: u32 = 0x8028_CF2C;
pub const JUMPED_ON: u32 = 0x8028_CF68;

/// The green shell: its dust every `x30` frames, its roll at `x20`, its hit delay started only
/// as it slides, its skid state 9, and it pops up (`xDEC` bit 3).
pub const GREEN: Shell = Shell {
    vars: shared,
    trail: 0x30,
    roll: 0x20,
    hit_delay_on_throw: None,
    skid: 9,
    stopped,
    fell,
    landed,
};

/// Runs one of the shell's predicates, if `addr` is one.
#[melee_callbacks::callbacks(Pred)]
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 | ANIM2 | ANIM9 => shell::still(r, gobj),
        COLL0 => shell::coll0(r, gobj, &GREEN),
        ANIM1 => anim1(r, gobj),
        COLL1 => shell::coll1(r, gobj, &GREEN),
        ANIM3 | ANIM4 => anim3(r, gobj),
        COLL3 | COLL4 | COLL8 => shell::coll3(r, gobj, &GREEN),
        ANIM6 => anim6(r, gobj),
        COLL6 => coll6(r, gobj),
        ANIM8 => anim8(r, gobj),
        COLL9 => shell::skid_coll(r, gobj, &GREEN),
        DMG_DEALT => dmg_dealt(r, gobj),
        DMG_RECEIVED => dmg_received(r, gobj),
        REFLECTED => shell::reflected(r, gobj, &GREEN),
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
        TRAIL => shell::trail(r, gobj, &GREEN),
        SPAWNED => spawned(r, gobj),
        STOPPED => stopped(r, gobj),
        PHYS0 | PHYS2 | PHYS9 => shell::idle(r, gobj),
        FELL => fell(r, gobj),
        PHYS1 | PHYS3 | PHYS4 | PHYS8 => shell::phys_fall(r, gobj),
        PICKED_UP => shell::picked_up(r, gobj, &GREEN),
        THROWN => shell::thrown(r, gobj, &GREEN),
        DROPPED => shell::dropped(r, gobj, &GREEN),
        SLIDE => slide(r, gobj),
        PHYS6 => phys6(r, gobj),
        SLIDE_AIR => slide_air(r, gobj),
        ENTERED_AIR => shell::entered_air(r, gobj, &GREEN),
        _ => return false,
    }
    true
}

/// The shell's variables.
fn vars(game: &mut Game, gobj: GObjId) -> &mut itGShell_ItemVars {
    item_mut(game, gobj).vars.gshell()
}

/// The fields the red shell has too.
fn shared(v: &mut ItemVars) -> shell::Vars<'_> {
    let g = v.gshell();
    shell::Vars {
        life: &mut g.life,
        hit_delay: &mut g.hit_delay,
        trail: &mut g.trail,
        landings: &mut g.landings,
        bounced: &mut g.bounced,
        popped: Some(&mut g.popped),
    }
}

/// Counts down how long a hit shell can't hurt its owner, `it_8028B988`.
#[doc(alias = "it_8028B988")]
pub fn count_owner_grace(r: &mut Running, gobj: GObjId) {
    let owner_grace = attr(r, gobj, 0x28);
    let v = vars(r.game, gobj);
    if !v.grace_counting.get() {
        return;
    }
    v.owner_grace.update(|old| mem(fp::fsubs(reg(old), 1.0)));
    if reg(v.owner_grace.get()) <= 0.0 {
        v.grace_counting.set(false);
        v.owner_grace.set(owner_grace);
        let blinking = v.blinking.get();
        hitbox::set_hit_flag0(r.game, gobj, 0, true);
        if blinking {
            hitbox::end_blink(r.game, gobj);
        }
    }
}

/// Lands from a throw or a hit, `it_8028BA2C`: stops when slow (or slides off), else
/// slides on.
#[doc(alias = "it_8028BA2C")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    let stop_speed = attr(r, gobj, 8);
    let b = item(r.game, gobj).kind_coll_box.clone();
    maplib::set_box(r.game, gobj, b);
    if reg(item(r.game, gobj).vel.x).abs() < reg(stop_speed) {
        item_mut(r.game, gobj).vel = Vec3::new(0.0, 0.0, 0.0);
        if !maplib::slides(r.game, r.data, gobj) {
            stopped(r, gobj);
        } else {
            shell::entered_air(r, gobj, &GREEN);
        }
    } else {
        slide(r, gobj);
    }
}

/// Sent sliding the way the hit pushed it, `it_8028BAD8`.
#[doc(alias = "it_8028BAD8")]
pub fn knocked(r: &mut Running, gobj: GObjId) {
    let push_scale = attr(r, gobj, 0x14);
    let ip = item_mut(r.game, gobj);
    let push = fp::fmuls(to_f32(ip.hit_damage), reg(push_scale));
    let away = -reg(ip.damage_dir);
    match ip.msid {
        0 | 1 | 3 | 4 | 9 => {
            ip.vel.x = mem(fp::fmuls(away, push));
            vars(r.game, gobj).blinking.set(true);
        }
        5..=8 => {
            ip.vel.x = mem(fp::fmadds(away, push, reg(ip.vel.x)));
            vars(r.game, gobj).blinking.set(true);
        }
        _ => {}
    }
    coll::take_attacker(r.game, gobj);
    hitbox::start_blink(r.game, gobj);
    if item(r.game, gobj).ground_or_air == 1 {
        slide_air(r, gobj);
    } else {
        slide(r, gobj);
    }
    vars(r.game, gobj).grace_counting.set(true);
    hitbox::set_hit_flag0(r.game, gobj, 0, false);
}

/// Bounces up off a fighter who hit a sliding shell (the file's inline `shellHit`, and the
/// sliding half of `it_8028BC2C`), up to `fell` (state 1).
fn pop_up(r: &mut Running, gobj: GObjId) {
    let (bounce_back, pop_up_vel, pop_timer) =
        (attr(r, gobj, 0xC), attr(r, gobj, 0x10), attr(r, gobj, 0x2C));
    hitbox::start_blink(r.game, gobj);
    hitbox::set_hit_ids(r.game, gobj);
    let f = r.game.objs.rng.draw_f32();
    let ip = item_mut(r.game, gobj);
    let vx = fp::fmuls(-reg(ip.vel.x), reg(bounce_back));
    ip.vel.x = mem(fp::fmuls(vx, reg(f)));
    ip.vel.y = pop_up_vel;
    shell::draw_landings(r, gobj, &GREEN);
    maplib::enter_air(item_mut(r.game, gobj));
    fell(r, gobj);
    let v = vars(r.game, gobj);
    v.popped.set(true);
    v.pop_timer.set(pop_timer);
}

/// Jumped on: a sliding shell pops up, a still one is kicked away from the jumper,
/// `it_8028BC2C`.
#[doc(alias = "it_8028BC2C")]
pub fn stomped(r: &mut Running, gobj: GObjId) {
    match item(r.game, gobj).msid {
        5..=8 => {
            pop_up(r, gobj);
            vars(r.game, gobj).blinking.set(true);
        }
        0 | 1 | 3 | 4 | 9 => {
            let kick_speed = attr(r, gobj, 0x1C);
            let ip = item_mut(r.game, gobj);
            ip.vel.x = mem(fp::fmuls(reg(kick_speed), -reg(ip.stomp_dir)));
            coll::take_stomper(r.game, gobj);
            vars(r.game, gobj).blinking.set(true);
            if item(r.game, gobj).ground_or_air == 1 {
                slide_air(r, gobj);
            } else {
                slide(r, gobj);
            }
        }
        _ => {}
    }
    hitbox::start_blink(r.game, gobj);
    vars(r.game, gobj).grace_counting.set(true);
    hitbox::set_hit_flag0(r.game, gobj, 0, false);
}

/// `itGShell_Logic14_Spawned`.
#[doc(alias = "itGShell_Logic14_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    let (owner_grace, pop_timer) = (attr(r, gobj, 0x28), attr(r, gobj, 0x2C));
    shell::start_life(r, gobj, &GREEN);
    let v = vars(r.game, gobj);
    v.landings.set(1);
    v.bounced.set(false);
    v.grace_counting.set(false);
    v.owner_grace.set(owner_grace);
    v.popped.set(false);
    v.pop_timer.set(pop_timer);
    v.blinking.set(true);
    fell(r, gobj);
}

/// Comes to a stop, stompable, `it_8028BE54`.
#[doc(alias = "it_8028BE54")]
pub fn stopped(r: &mut Running, gobj: GObjId) {
    lib26b1::enable_pickup(r.game, gobj);
    hitbox::set_hit_ids(r.game, gobj);
    hitbox::set_hit_ids2(r.game, gobj);
    hitbox::end_blink(r.game, gobj);
    item_mut(r.game, gobj).vel = Vec3::new(0.0, 0.0, 0.0);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    vars(r.game, gobj).popped.set(false);
    lib2725::make_stompable(r.game, gobj);
    item_mut(r.game, gobj).jumped_on = Some(CodeRef(JUMPED_ON));
}

/// Falls (state 1), not stompable, `it_8028C018`.
#[doc(alias = "it_8028C018")]
pub fn fell(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    lib2725::clear_stomp(r.game, gobj);
}

/// `itGshell_UnkMotion1_Anim`: a popped-up shell turns stompable again after `xDE4` frames.
#[doc(alias = "itGshell_UnkMotion1_Anim")]
pub fn anim1(r: &mut Running, gobj: GObjId) -> bool {
    let stompable = super::super::flag(item(r.game, gobj).status_flags, 0);
    let v = vars(r.game, gobj);
    if v.popped.get() && !stompable {
        if reg(v.pop_timer.get()) <= 0.0 {
            lib2725::make_stompable(r.game, gobj);
            item_mut(r.game, gobj).jumped_on = Some(CodeRef(JUMPED_ON));
        } else {
            v.pop_timer.update(|old| mem(fp::fsubs(reg(old), 1.0)));
        }
    }
    false
}

/// `itGshell_UnkMotion3_Anim` and 4.
#[doc(alias = "itGshell_UnkMotion3_Anim")]
#[doc(alias = "itGshell_UnkMotion4_Anim")]
pub fn anim3(r: &mut Running, gobj: GObjId) -> bool {
    count_owner_grace(r, gobj);
    false
}

/// Starts sliding: the speed capped, its time and hit delays set, then sliding (`slow` when
/// all but stopped, `fast` else).
fn start_slide(r: &mut Running, gobj: GObjId, fast: i32, slow: i32, ground: bool) {
    let (max_speed, stop_speed, hit_delay, trail) = (
        attr(r, gobj, 4),
        attr(r, gobj, 8),
        attr(r, gobj, 0x24),
        attr(r, gobj, 0x30),
    );
    lib26b1::disable_pickup(r.game, gobj);
    hitbox::clear_hit_ids(r.game, gobj);
    hitbox::let_own_items_hit(r.game, gobj);
    let life = common_data(r.game).u32(r.data, super::super::data::common::LIFETIME);
    lib2725::set_lifetime(r.game, r.data, gobj, mem(fp::frsp(f64::from(life))));
    item_mut(r.game, gobj).skid = 0;
    let v = vars(r.game, gobj);
    v.hit_delay.set(hit_delay);
    v.trail.set(trail);
    v.popped.set(false);
    let ip = item_mut(r.game, gobj);
    if reg(ip.vel.x).abs() > reg(max_speed) {
        let dir = if reg(ip.vel.x) < 0.0 { -1.0 } else { 1.0 };
        ip.vel.x = mem(fp::fmuls(dir, reg(max_speed)));
    }
    lib2725::face_motion(r.game, gobj);
    if reg(item(r.game, gobj).vel.x).abs() < reg(stop_speed) {
        change_state(r, gobj, slow, change::ANIM_UPDATE);
        lib26b1::enable_pickup(r.game, gobj);
        if ground {
            hitbox::set_hit_ids2(r.game, gobj);
        }
    } else {
        change_state(r, gobj, fast, change::ANIM_UPDATE);
    }
    lib2725::make_stompable(r.game, gobj);
    item_mut(r.game, gobj).jumped_on = Some(CodeRef(JUMPED_ON));
}

/// Slides on the ground, `it_8028C3A8`.
#[doc(alias = "it_8028C3A8")]
pub fn slide(r: &mut Running, gobj: GObjId) {
    start_slide(r, gobj, 5, 6, true);
}

/// `itGshell_UnkMotion6_Anim` (5, 7, 8): its hits come back after `xDD8` frames, it trails
/// dust on the ground, and its time runs out.
#[doc(alias = "itGshell_UnkMotion6_Anim")]
pub fn anim6(r: &mut Running, gobj: GObjId) -> bool {
    shell::count_hit_delay(r, gobj, &GREEN);
    count_owner_grace(r, gobj);
    if matches!(item(r.game, gobj).msid, 5 | 6) {
        shell::trail(r, gobj, &GREEN);
    }
    let v = vars(r.game, gobj);
    if reg(v.life.get()) <= 0.0 {
        return true;
    }
    v.life.update(|old| mem(fp::fsubs(reg(old), 1.0)));
    false
}

/// `itGshell_UnkMotion6_Phys`: when all but stopped (and not held), it can be picked up.
#[doc(alias = "itGshell_UnkMotion6_Phys")]
pub fn phys6(r: &mut Running, gobj: GObjId) {
    let stop_speed = attr(r, gobj, 8);
    let ip = item(r.game, gobj);
    if reg(ip.vel.x).abs() < reg(stop_speed) && !ip.flags.flags.pickup {
        lib26b1::enable_pickup(r.game, gobj);
        lib2725::clear_hitboxes(r.game, r.data, gobj);
        hitbox::set_hit_ids2(r.game, gobj);
    }
}

/// `itGshell_UnkMotion6_Coll`: rolls, and turns around off walls.
#[doc(alias = "itGshell_UnkMotion6_Coll")]
pub fn coll6(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, slide_air);
    shell::roll(r, gobj, &GREEN);
    if maplib::bounce_off_wall(r.game, r.data, gobj) {
        lib2725::face_motion(r.game, gobj);
    }
    false
}

/// Slides through the air, `it_8028C898`.
#[doc(alias = "it_8028C898")]
pub fn slide_air(r: &mut Running, gobj: GObjId) {
    start_slide(r, gobj, 7, 8, false);
}

/// `itGshell_UnkMotion8_Anim`.
#[doc(alias = "itGshell_UnkMotion8_Anim")]
pub fn anim8(r: &mut Running, gobj: GObjId) -> bool {
    anim6(r, gobj);
    false
}

/// `itGShell_Logic14_DmgDealt`.
#[doc(alias = "itGShell_Logic14_DmgDealt")]
pub fn dmg_dealt(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::show(r.game, gobj);
    false
}

/// `itGShell_Logic14_DmgReceived`.
#[doc(alias = "itGShell_Logic14_DmgReceived")]
pub fn dmg_received(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::show(r.game, gobj);
    knocked(r, gobj);
    false
}

/// `itGShell_Logic14_Clanked`.
#[doc(alias = "itGShell_Logic14_Clanked")]
pub fn clanked(r: &mut Running, gobj: GObjId) -> bool {
    pop_up(r, gobj);
    false
}

/// `itGShell_Logic14_HitShield`: a thrown shell bounces off, a sliding one pops up.
#[doc(alias = "itGShell_Logic14_HitShield")]
pub fn hit_shield(r: &mut Running, gobj: GObjId) -> bool {
    match item(r.game, gobj).msid {
        3 | 4 => lib2725::bounce_off_victim(r.game, r.data, gobj),
        5..=8 => pop_up(r, gobj),
        _ => {}
    }
    false
}

/// Jumped on, `it_8028CF68`.
#[doc(alias = "it_8028CF68")]
pub fn jumped_on(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::show(r.game, gobj);
    stomped(r, gobj);
    play_sfx(r, gobj, 241, 127, 64);
    false
}
