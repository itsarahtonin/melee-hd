//! The Bunny Hood, `melee/it/kinds/itrabbitc.c`: it falls and rests; the fighter that picks it
//! up wears it (`ftCommon_8007FA58`), its ears and parts fitted to the head
//! (`it_80294E78`, `it_80294EB0`), its shine shown or hidden (`it_802950D4`). Its rest, falls,
//! pickup, drop and skid are the power-ups' ([`powerup`]).

use super::super::lib2725;
use super::powerup::{self, PowerUp};
use crate::game::it::ItKind;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

pub const SHIELD_BOUNCED: u32 = 0x8029_4DA0;
pub const SPAWNED: u32 = 0x8029_5118;
pub const ANIM0: u32 = 0x8029_5188;
pub const PHYS0: u32 = 0x8029_5190;
pub const COLL0: u32 = 0x8029_5194;
pub const ANIM3: u32 = 0x8029_51E8;
pub const PHYS1: u32 = 0x8029_51F0;
pub const COLL1: u32 = 0x8029_5220;
pub const PICKED_UP: u32 = 0x8029_524C;
pub const ANIM2: u32 = 0x8029_5290;
pub const DROPPED: u32 = 0x8029_5298;
pub const PHYS3: u32 = 0x8029_52DC;
pub const COLL3: u32 = 0x8029_530C;
pub const ENTERED_AIR: u32 = 0x8029_5340;
pub const ANIM4: u32 = 0x8029_5368;
pub const PHYS4: u32 = 0x8029_5370;
pub const COLL4: u32 = 0x8029_5374;

/// The Bunny Hood: its shine hidden while it's held and shown again when it's let go of.
pub const BUNNY_HOOD: PowerUp = PowerUp {
    clears_hurt: false,
    on_spawn: None,
    tilts: false,
    on_pickup: Some(hide_shine),
    on_drop: Some(unhide_shine),
};

/// Runs one of the Bunny Hood's predicates, if `addr` is one.
#[melee_callbacks::callbacks(Pred)]
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        SHIELD_BOUNCED => shield_bounced(r, gobj),
        ANIM0 | ANIM3 | ANIM2 | ANIM4 => powerup::still(r, gobj),
        COLL0 => powerup::rests(r, gobj, &BUNNY_HOOD),
        COLL1 => powerup::lands(r, gobj),
        COLL3 => powerup::vanishes(r, gobj),
        COLL4 => powerup::skids(r, gobj),
        _ => return None,
    })
}

/// Runs one of the Bunny Hood's events, if `addr` is one.
#[melee_callbacks::callbacks(Proc)]
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => powerup::spawned(r, gobj, &BUNNY_HOOD),
        PHYS0 | PHYS4 => powerup::idle(r, gobj),
        PHYS1 | PHYS3 => powerup::gravity(r, gobj),
        PICKED_UP => powerup::picked_up(r, gobj, &BUNNY_HOOD),
        DROPPED => powerup::dropped(r, gobj, &BUNNY_HOOD),
        ENTERED_AIR => powerup::entered_air(r, gobj),
        _ => return false,
    }
    true
}

/// `itRabbitC_Logic30_ShieldBounced`: bounces off a shield.
#[doc(alias = "itRabbitC_Logic30_ShieldBounced")]
pub fn shield_bounced(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::bounce_off_shield(r.game, gobj)
}

/// Scales the ears (bone 1) to `scale`, `it_80294E78`.
#[doc(alias = "it_80294E78")]
pub fn scale_ears(r: &mut Running, gobj: GObjId, scale: f32) {
    let j = lib2725::bone(r.game, r.data, gobj, 1).expect("a Bunny Hood without its bones");
    lib2725::set_uniform_scale(r.game, j, scale);
}

/// Moves bone 8 by `a` and bone 4 by `b`, `it_80294EB0`.
#[doc(alias = "it_80294EB0")]
pub fn shift_parts(r: &mut Running, gobj: GObjId, a: Vec3, b: Vec3) {
    use crate::fpu::{mem, reg};
    use gekko_fp as fp;
    for (bone, d) in [(8, a), (4, b)] {
        let j = lib2725::bone(r.game, r.data, gobj, bone).expect("a Bunny Hood without its bones");
        let t = r.game.objs.jobjs[j].translate;
        let t = Vec3::new(
            mem(fp::fadds(reg(t.x), reg(d.x))),
            mem(fp::fadds(reg(t.y), reg(d.y))),
            mem(fp::fadds(reg(t.z), reg(d.z))),
        );
        super::super::item::set_translate(r.game, j, t);
    }
}

/// Shows (`on`) or hides its shine, bone 3, `it_802950D4`.
#[doc(alias = "it_802950D4")]
pub fn show_shine(r: &mut Running, gobj: GObjId, on: bool) {
    let j = lib2725::bone(r.game, r.data, gobj, 3);
    if on {
        lib2725::show_tree(r.game, j);
    } else {
        lib2725::hide_tree(r.game, j);
    }
}

/// Its shine hidden as it's picked up (`itRabbitC_Logic31_PickedUp` makes
/// `it_802950D4`'s calls itself).
fn hide_shine(r: &mut Running, gobj: GObjId) {
    show_shine(r, gobj, false);
}

/// Its shine shown as it's let go of (`itRabbitC_Logic31_Dropped`'s).
fn unhide_shine(r: &mut Running, gobj: GObjId) {
    show_shine(r, gobj, true);
}

/// A Bunny Hood resting at `pos`, `it_80294DC0`.
#[doc(alias = "it_80294DC0")]
pub fn spawn_at(r: &mut Running, pos: Vec3) -> Option<GObjId> {
    let mut spawn = super::super::item::drop_spawn(ItKind::BunnyHood.id(), pos, false);
    let gobj = super::super::item::spawn_ground(r, &mut spawn)?;
    powerup::landed(r, gobj);
    Some(gobj)
}
