//! Fox's and Falco's blasters, `melee/it/kinds/itfoxblaster.c`: the gun a fighter holds
//! through its neutral special and throws, whose state follows the fighter's action and whose
//! model recoils with each shot.

use super::super::item::{animate, change_state, destroy, logic, play_sfx, spawn_air};
use super::super::{item, item_mut, jobj_of, lib26b1, lib2725};
use super::{FALCO_BLASTER, FOX_BLASTER, KIRBY_FALCO_BLASTER, KIRBY_FOX_BLASTER};
use crate::fpu::{mem, reg};
use crate::game::code;
use crate::game::state::{Game, Running};
use crate::game::structs::{Item, SpawnItem};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::vec::Vec3;
use gekko_fp as fp;

pub const MOTION8_ANIM: u32 = 0x802A_EBCC;
pub const MOTION8_PHYS: u32 = 0x802A_EED4;
pub const MOTION8_COLL: u32 = 0x802A_EF08;
pub const MOTION9_ANIM: u32 = 0x802A_EF10;
pub const MOTION9_PHYS: u32 = 0x802A_EFB8;
pub const MOTION9_COLL: u32 = 0x802A_EFBC;
pub const MOTION10_ANIM: u32 = 0x802A_F064;
pub const MOTION10_PHYS: u32 = 0x802A_F178;
pub const MOTION10_COLL: u32 = 0x802A_F17C;
pub const PICKED_UP: u32 = 0x802A_EB00;
pub const EVT_UNK: u32 = 0x802A_F184;
/// `it_802ADEF0`, the accessory callback a shot sets.
pub const ON_SHOT: u32 = 0x802A_DEF0;

/// Runs one of the blaster's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        MOTION8_ANIM => follow_fighter(r, gobj),
        MOTION8_COLL | MOTION10_COLL => false,
        MOTION9_ANIM | MOTION9_COLL => {
            crate::game::ef::destroy_all(r, gobj);
            release(r, gobj);
            true
        }
        MOTION10_ANIM => scale_with_owner(r, gobj),
        _ => return None,
    })
}

/// Runs one of the blaster's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        MOTION8_PHYS => {
            recoil(r.game, gobj);
            pulse(r.game, gobj);
        }
        MOTION9_PHYS | MOTION10_PHYS => {}
        PICKED_UP => picked_up(r, gobj),
        ON_SHOT => on_shot(r, gobj),
        _ => return false,
    }
    true
}

/// A fighter's action, `Fighter::motion_id`.
pub fn motion_id(game: &Game, gobj: Option<GObjId>) -> Option<i32> {
    gobj.map(|g| super::super::owner::fighter(game, g).motion_id)
}

/// `ftFx_MS_SpecialNStart`, and the common throws.
const SPECIAL_N_START: i32 = 341;
const CATCH_DASH: i32 = 214;
const THROW_B: i32 = 220;
const THROW_LW: i32 = 222;

/// Which blaster state the owner's action calls for, `ftFx_SpecialN_GetBlasterAction`: its
/// neutral special's six actions and the three throws that shoot, else 9 (none).
#[doc(alias = "ftFx_SpecialN_GetBlasterAction")]
pub fn blaster_action(game: &Game, owner: Option<GObjId>) -> i32 {
    match motion_id(game, owner) {
        Some(m) if (SPECIAL_N_START..SPECIAL_N_START + 6).contains(&m) => m - SPECIAL_N_START,
        Some(m) if (THROW_B..=THROW_LW).contains(&m) => m - CATCH_DASH,
        _ => 9,
    }
}

/// Whether the owner has let go of its blaster, `ftFx_SpecialN_CheckRemoveBlaster`
/// (`Fighter::u.fx.x222C_blasterGObj` is null).
#[doc(alias = "ftFx_SpecialN_CheckRemoveBlaster")]
pub fn blaster_removed(game: &Game, owner: GObjId) -> bool {
    super::super::owner::fighter(game, owner)
        .u
        .gobj_at(crate::game::ft::vars::FOX_BLASTER)
        .is_none()
}

/// Whether the owner's action no longer needs the blaster, `ftFx_SpecialN_CheckBlasterAction`:
/// in one of its blaster actions, when its action flags say so (`x2070.x2071_b6`).
#[doc(alias = "ftFx_SpecialN_CheckBlasterAction")]
pub fn blaster_not_needed(game: &Game, owner: Option<GObjId>) -> bool {
    let Some(o) = owner else { return true };
    let fp = super::super::owner::fighter(game, o);
    let m = fp.motion_id;
    if (SPECIAL_N_START..SPECIAL_N_START + 6).contains(&m) || (THROW_B..=THROW_LW).contains(&m) {
        fp.x2070[1] & 0x02 != 0
    } else {
        true
    }
}

/// The owner forgets its blaster, `ftFx_SpecialN_ClearBlaster`: its blaster GObj and its damage
/// callbacks (`Fighter_SetDamageCallback` with none).
#[doc(alias = "ftFx_SpecialN_ClearBlaster")]
pub fn clear_from_owner(game: &mut Game, owner: GObjId) {
    let f = super::super::owner::fighter_of(game, owner);
    let fp = &mut game.fighters[f.0];
    fp.u.set_gobj_at(crate::game::ft::vars::FOX_BLASTER, None);
    fp.take_dmg_cb = None;
    fp.death2_cb = None;
}

/// Plays the Fox or Falco version of a sound (`itFoxBlaster_PlaySFX`).
pub fn play_kind_sfx(r: &mut Running, gobj: GObjId, fox: i32, falco: i32) {
    match item(r.game, gobj).kind {
        FOX_BLASTER | KIRBY_FOX_BLASTER => play_sfx(r, gobj, fox, 0x7F, 0x40),
        FALCO_BLASTER | KIRBY_FALCO_BLASTER => play_sfx(r, gobj, falco, 0x7F, 0x40),
        _ => {}
    }
}

/// Shows or hides the blaster's model with the owner's script, `it_802ADDD0`, with a sound
/// when it's put away.
#[doc(alias = "it_802ADDD0")]
pub fn set_visible(r: &mut Running, gobj: GObjId, visibility: i32) {
    let game = &mut *r.game;
    let root = jobj_of(game, gobj);
    let child = root.and_then(|j| game.objs.jobjs[j].child);
    let v = item_mut(game, gobj).xDD4_itemVar.fox_blaster();
    if v.set_sfx_var2.get() != visibility {
        v.set_sfx_var2.set(visibility);
        if visibility == 2 {
            play_kind_sfx(r, gobj, 0x1AE14, 0x18700);
        }
    }
    let game = &mut *r.game;
    match item_mut(game, gobj)
        .xDD4_itemVar
        .fox_blaster()
        .set_sfx_var2
        .get()
    {
        0 | 2 => jobj::set_flags_all(&mut game.objs.jobjs, child, jobj::HIDDEN),
        1 => jobj::clear_flags_all(&mut game.objs.jobjs, child, jobj::HIDDEN),
        _ => {}
    }
}

/// The accessory callback after a shot, `it_802ADEF0` (`it_802ADF10`): the muzzle flash, and
/// the shots' history moves on.
///
/// The original keeps each flash as the address `efSync_Spawn` returned, cast to an `s32`
/// (`xDE4`), and only ever moves it along the history: the flash is an effect handle here,
/// which moves the same way (see docs/engine.md's uninitialized values).
#[doc(alias = "it_802ADEF0")]
#[doc(alias = "it_802ADF10")]
pub fn on_shot(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    let owner = ip.owner;
    let kind = ip.kind;
    let v = ip.xDD4_itemVar.fox_blaster();
    if owner != v.owner.get() || v.gfx_spawn_var.get() == 0 {
        return;
    }
    v.gfx_spawn_var.set(0);
    let o = v.owner.get().expect("a blaster's flash without an owner");
    let pos = r.game.ft_pos(o);
    // The muzzle and grip, and the flash: Fox's and Falco's own, or Kirby's copies'
    // (`ftKb_SpecialNFx_800FDC00`, `ftKb_SpecialNFx_800FDC70`).
    let kirby = match kind {
        FOX_BLASTER | FALCO_BLASTER => false,
        KIRBY_FOX_BLASTER | KIRBY_FALCO_BLASTER => true,
        _ => return shift_history(r, gobj),
    };
    let muzzle = if kirby {
        r.kb_fx_muzzle(Some(o))
    } else {
        r.fox_muzzle(o)
    };
    let v = item_mut(r.game, gobj).xDD4_itemVar.fox_blaster();
    v.xE14[0].set(Vec3::new(
        mem(fp::fsubs(reg(muzzle.x), reg(pos.x))),
        mem(fp::fsubs(reg(muzzle.y), reg(pos.y))),
        mem(fp::fsubs(reg(muzzle.z), reg(pos.z))),
    ));
    let grip = if kirby {
        r.kb_fx_grip(Some(o))
    } else {
        r.fox_grip(o)
    };
    let v = item_mut(r.game, gobj).xDD4_itemVar.fox_blaster();
    v.angle[0].set(crate::math::atan2(
        mem(fp::fsubs(reg(muzzle.y), reg(grip.y))),
        mem(fp::fsubs(reg(muzzle.x), reg(grip.x))),
    ));
    v.xDFC[0].set(0);
    let angle = v.angle[0].get();
    let flash = crate::game::ef::spawn(
        r,
        if kirby { 1196 } else { 1166 },
        Some(gobj),
        &[
            crate::game::ef::Arg::Vec(muzzle),
            crate::game::ef::Arg::Float(angle),
        ],
    );
    item_mut(r.game, gobj).xDD4_itemVar.fox_blaster().xDE4[0].set(match flash {
        crate::game::ef::Spawned::Effect(e) => Some(e),
        crate::game::ef::Spawned::None => None,
        other => panic!("the blaster's flash spawned {other:?}"),
    });
    shift_history(r, gobj);
}

/// The shots' history moves on, the newest first.
fn shift_history(r: &mut Running, gobj: GObjId) {
    let v = item_mut(r.game, gobj).xDD4_itemVar.fox_blaster();
    for i in (1..6).rev() {
        let x = v.xDE4[i - 1].get();
        v.xDE4[i].set(x);
        let x = v.xDFC[i - 1].get();
        v.xDFC[i].set(x);
        let x = v.xE14[i - 1].get();
        v.xE14[i].set(x);
        let x = v.angle[i - 1].get();
        v.angle[i].set(x);
    }
    v.xDE4[0].set(None);
    v.xDFC[0].set(0);
    v.xE14[0].set(Vec3::new(0.0, 0.0, 0.0));
    v.angle[0].set(0.0);
}

/// Starts a shot's flash and recoil, `it_802AE1D0`.
#[doc(alias = "it_802AE1D0")]
pub fn shot(game: &mut Game, gobj: GObjId) {
    let ip = item_mut(game, gobj);
    ip.xDD4_itemVar.fox_blaster().xDD4.set(1);
    ip.on_accessory = Some(crate::game::handles::CodeRef(ON_SHOT));
    ip.xDD4_itemVar.fox_blaster().gfx_spawn_var.set(1);
}

/// The first child `n` levels down (null-safe, as the original walks it).
pub fn down(game: &Game, j: Option<JObjId>, n: u32) -> Option<JObjId> {
    let mut j = j;
    for _ in 0..n {
        j = j.and_then(|j| game.objs.jobjs[j].child);
    }
    j
}

/// The muzzle's translation, the flash's scale and the barrel's turn, by recoil frame
/// (`it_803F6D58`, `it_803F6D90`, `it_803F6DC8`).
const RECOIL_Z: [f32; 14] = [
    0.0, -0.51, -1.02, -1.53, -1.39, -1.251, -1.112, -0.973, -0.834, -0.695, -0.556, -0.417,
    -0.278, -0.139,
];
const RECOIL_SCALE: [f32; 14] = [
    0.5, 0.5, 1.75, 3.0, 2.375, 1.75, 1.125, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5,
];
const RECOIL_TURN: [f32; 14] = [
    0.0, -42.0, -20.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
];

/// Plays a frame of the shot's recoil on the model, `it_802AE200`.
#[doc(alias = "it_802AE200")]
pub fn recoil(game: &mut Game, gobj: GObjId) {
    let root = jobj_of(game, gobj);
    let j4 = down(game, root, 4);
    let Some(j1) = j4.and_then(|j| game.objs.jobjs[j].next) else {
        return;
    };
    let f = item_mut(game, gobj).xDD4_itemVar.fox_blaster().xDD4.get() as usize;
    jobj::set_srt(&mut game.objs.jobjs, j1, |o| o.translate.z = RECOIL_Z[f]);
    let j2 = game.objs.jobjs[j1].next.expect("jobj");
    jobj::set_srt(&mut game.objs.jobjs, j2, |o| o.scale.y = RECOIL_SCALE[f]);
    jobj::set_srt(&mut game.objs.jobjs, j2, |o| o.scale.z = RECOIL_SCALE[f]);
    let j3 = game.objs.jobjs[j2].next.expect("jobj");
    lib2725::set_rotation_x(game, j3, RECOIL_TURN[f]);
    let v = item_mut(game, gobj).xDD4_itemVar.fox_blaster();
    if v.xDD4.get() > 0 && v.xDD4.get() < 14 {
        v.xDD4.update(|old| old + 1);
        if v.xDD4.get() >= 14 {
            v.xDD4.set(0);
        }
    }
}

/// Starts the barrel's pulse with the first shot, `it_802AE538`.
#[doc(alias = "it_802AE538")]
pub fn start_pulse(r: &mut Running, gobj: GObjId) {
    let game = &mut *r.game;
    let v = item_mut(game, gobj).xDD4_itemVar.fox_blaster();
    if v.xDD8.get() != 4 {
        v.xDD8.set(1);
        v.xDDC.set(1);
        if v.xDE0.get() == 0 {
            play_kind_sfx(r, gobj, 0x1AE05, 0x186F1);
            item_mut(r.game, gobj)
                .xDD4_itemVar
                .fox_blaster()
                .xDE0
                .set(1);
        }
    }
}

/// Runs the barrel's pulse back down as the blaster is put away, `it_802AE608`.
#[doc(alias = "it_802AE608")]
pub fn end_pulse(game: &mut Game, gobj: GObjId) {
    let v = item_mut(game, gobj).xDD4_itemVar.fox_blaster();
    if v.xDD8.get() != 0 {
        v.xDD8.set(3);
        v.xDDC.set(-1);
    }
}

/// The barrel's height by pulse frame (`it_803F6E28`).
const PULSE_Y: [f32; 5] = [-0.425, -0.595, -0.765, -0.935, -0.85];

/// Plays a frame of the barrel's pulse, `it_802AE63C`.
#[doc(alias = "it_802AE63C")]
pub fn pulse(game: &mut Game, gobj: GObjId) {
    let root = jobj_of(game, gobj);
    let Some(target) = down(game, root, 4) else {
        return;
    };
    let f = item_mut(game, gobj).xDD4_itemVar.fox_blaster().xDD8.get() as usize;
    jobj::set_srt(&mut game.objs.jobjs, target, |o| o.translate.y = PULSE_Y[f]);
    let v = item_mut(game, gobj).xDD4_itemVar.fox_blaster();
    if v.xDD8.get() > 0 && v.xDD8.get() < 4 {
        v.xDD8.update(|old| old.wrapping_add(v.xDDC.get()));
        if v.xDD8.get() >= 5 {
            v.xDD8.set(4);
            v.xDDC.set(0);
        } else if v.xDD8.get() <= 0 {
            v.xDD8.set(0);
            v.xDDC.set(0);
        }
    }
}

/// Sets the item's script variables (`itFoxBlaster_SetCommandVars`).
pub fn set_command_vars(ip: &mut Item, value: u32) {
    ip.xDB8_itcmd_var3 = value;
    ip.xDB4_itcmd_var2 = value;
    ip.xDB0_itcmd_var1 = value;
    ip.xDAC_itcmd_var0 = value;
}

/// Clears the blaster's variables, `it_802AE7B8`.
#[doc(alias = "it_802AE7B8")]
pub fn reset(game: &mut Game, gobj: GObjId) {
    let ip = item_mut(game, gobj);
    set_command_vars(ip, 0);
    ip.xDCC_flag.b3 = false;
    let v = ip.xDD4_itemVar.fox_blaster();
    v.xDD4.set(0);
    v.xDD8.set(0);
    v.xDDC.set(0);
    v.xDE0.set(0);
    v.gfx_spawn_var.set(0);
    v.set_sfx_var2.set(1);
    for i in 0..6 {
        v.xDE4[i].set(None);
        v.xDFC[i].set(0);
        v.xE14[i].set(Vec3::new(0.0, 0.0, 0.0));
        v.angle[i].set(0.0);
    }
}

/// A blaster in a fighter's hand, `it_802AE8A8`: made at `pos` facing `facing_dir`, then held
/// at the fighter's part `part`.
#[doc(alias = "it_802AE8A8")]
pub fn spawn(
    r: &mut Running,
    facing_dir: f32,
    fighter: Option<GObjId>,
    pos: Vec3,
    part: i32,
    kind: i32,
) -> Option<GObjId> {
    let fighter = fighter?;
    let mut spawn = SpawnItem {
        kind,
        ..Default::default()
    };
    // `Item_InitSpawnPosition` on the plane.
    spawn.prev_pos = pos;
    spawn.prev_pos.z = 0.0;
    spawn.pos = spawn.prev_pos;
    // `Item_InitSpawnCommonFields`, without a first collision.
    spawn.facing_dir = facing_dir;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = Some(fighter);
    spawn.x4_parent_gobj2 = Some(fighter);
    spawn.x44_flag[0] &= !0x80;
    spawn.x40 = 0;
    let gobj = spawn_air(r, &mut spawn)?;
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .fox_blaster()
        .owner
        .set(Some(fighter));
    reset(r.game, gobj);
    super::super::item::pick_up(r, gobj, fighter, part);
    Some(gobj)
}

/// Destroys a blaster its owner lets go of, `it_802AEAB4`.
pub fn remove(r: &mut Running, gobj: Option<GObjId>) {
    let Some(gobj) = gobj else { return };
    let ip = item_mut(r.game, gobj);
    ip.owner = None;
    ip.xDD4_itemVar.fox_blaster().owner.set(None);
    crate::game::ef::destroy_all(r, gobj);
    destroy(r, gobj);
}

/// The state each blaster action puts the blaster in (`it_803F6E68`).
const STATE_FOR_ACTION: [i32; 11] = [0, 9, 9, 3, 9, 9, 6, 7, 8, 9, 10];

/// The blaster's pickup callback, `itFoxBlaster_Logic96_PickedUp`: the state its owner's action
/// calls for (10 when its script variables are set), animated a frame.
#[doc(alias = "itFoxBlaster_Logic96_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    let ip = item(r.game, gobj);
    let vars = ip.xDB8_itcmd_var3 | ip.xDB4_itcmd_var2 | ip.xDAC_itcmd_var0 | ip.xDB0_itcmd_var1;
    let flags = super::super::item::change::ANIM_UPDATE;
    if vars != 0 {
        change_state(r, gobj, 0xA, flags);
    } else {
        let owner = item_mut(r.game, gobj)
            .xDD4_itemVar
            .fox_blaster()
            .owner
            .get();
        let action = match item(r.game, gobj).kind {
            FOX_BLASTER | FALCO_BLASTER => blaster_action(r.game, owner),
            KIRBY_FOX_BLASTER | KIRBY_FALCO_BLASTER => r.kb_fx_blaster_action(owner),
            _ => 9,
        };
        change_state(r, gobj, STATE_FOR_ACTION[action as usize], flags);
    }
    animate(r, gobj);
}

/// The owner and the blaster forget each other (`clear_blaster_references`), and its effects
/// go.
pub fn release(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    let owner = ip.owner;
    let vowner = ip.xDD4_itemVar.fox_blaster().owner.get();
    if owner == vowner {
        match item(r.game, gobj).kind {
            FOX_BLASTER | FALCO_BLASTER => {
                if let Some(o) = vowner {
                    clear_from_owner(r.game, o);
                }
            }
            KIRBY_FOX_BLASTER | KIRBY_FALCO_BLASTER => r.kb_fx_forget_blaster(vowner),
            _ => {}
        }
    }
    let ip = item_mut(r.game, gobj);
    ip.owner = None;
    ip.xDD4_itemVar.fox_blaster().owner.set(None);
    crate::game::ef::destroy_all(r, gobj);
}

/// The blaster's animation callback in its fighter's hands, `itFoxblaster_UnkMotion8_Anim`:
/// its state follows the owner's action, and it goes when the owner's done with it.
#[doc(alias = "itFoxblaster_UnkMotion8_Anim")]
pub fn follow_fighter(r: &mut Running, gobj: GObjId) -> bool {
    let msid = item(r.game, gobj).msid;
    let kind = item(r.game, gobj).kind;
    if !matches!(msid, 2 | 5..=8) {
        let owner = item_mut(r.game, gobj)
            .xDD4_itemVar
            .fox_blaster()
            .owner
            .get();
        let action = match kind {
            FOX_BLASTER | FALCO_BLASTER => blaster_action(r.game, owner),
            KIRBY_FOX_BLASTER | KIRBY_FALCO_BLASTER => r.kb_fx_blaster_action(owner),
            _ => 9,
        };
        if msid != action {
            change_state(r, gobj, action, super::super::item::change::ANIM_UPDATE);
            animate(r, gobj);
        }
    }
    let owner = item_mut(r.game, gobj)
        .xDD4_itemVar
        .fox_blaster()
        .owner
        .get();
    let Some(o) = owner else {
        clear(r, gobj);
        return false;
    };
    let (action, removed) = match kind {
        FOX_BLASTER | FALCO_BLASTER => (blaster_action(r.game, owner), blaster_removed(r.game, o)),
        KIRBY_FOX_BLASTER | KIRBY_FALCO_BLASTER => {
            (r.kb_fx_blaster_action(owner), r.kb_fx_no_blaster(owner))
        }
        _ => (9, true),
    };
    if action == 9 || removed {
        clear(r, gobj);
    } else {
        let not_needed = match kind {
            FOX_BLASTER | FALCO_BLASTER => blaster_not_needed(r.game, owner),
            KIRBY_FOX_BLASTER | KIRBY_FALCO_BLASTER => r.kb_fx_blaster_not_needed(owner),
            _ => true,
        };
        if not_needed {
            item_mut(r.game, gobj)
                .xDD4_itemVar
                .fox_blaster()
                .owner
                .set(None);
            destroy(r, gobj);
            // The original then destroys the effects of the GObj it just freed
            // (`efLib_DestroyAll`), which finds none: destroying the item destroyed them.
        }
    }
    false
}

/// Forgets the owner and destroys the blaster (`clear_blaster`).
pub fn clear(r: &mut Running, gobj: GObjId) {
    release(r, gobj);
    let ip = item_mut(r.game, gobj);
    ip.owner = None;
    ip.xDD4_itemVar.fox_blaster().owner.set(None);
    crate::game::ef::destroy_all(r, gobj);
    destroy(r, gobj);
}

/// Matches the owner's scale when the owner's been resized, `itFoxblaster_UnkMotion10_Anim`.
pub fn scale_with_owner(r: &mut Running, gobj: GObjId) -> bool {
    if item_mut(r.game, gobj)
        .xDD4_itemVar
        .fox_blaster()
        .owner
        .get()
        .is_some()
    {
        code::unported_call("ftCo_800BF228 (a resized blaster owner)");
    }
    false
}

/// Forgets a GObj the blaster points to, `itFoxBlaster_Logic96_EvtUnk` (`it_8026B894`).
#[doc(alias = "itFoxBlaster_Logic96_EvtUnk")]
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}

/// Whether the blaster's logic table is the one ported here (Fox's or Falco's).
pub fn is_blaster(game: &Game, gobj: GObjId) -> bool {
    logic(game, gobj).picked_up == Some(crate::game::handles::CodeRef(PICKED_UP))
}
