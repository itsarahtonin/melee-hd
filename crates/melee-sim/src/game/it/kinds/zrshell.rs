//! The Koopas' red shell, `melee/it/kinds/itzrshell.c` (`It_Kind_ZRShell`): the green shell's
//! code ([`super::zgshell`], which dispatches the red one's wrappers too) with its own spawn.

use gekko_fp as fp;

use super::super::item::{change, change_state};
use super::super::{item_mut, jobj_of, zako};
use super::zgshell::{off, set, settle};
use crate::fpu::mem;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

/// Spawns a red shell, `it_802E0488`: an enemy item at `pos` facing `facing` (its model turned
/// that way, its collision left facing as it spawned), the Koopa that wakes from it of `kind`,
/// posed in its state 12 first, then settled as the green one.
// Ports it_802E0488; aliased once a check reaches it.
pub fn spawn(r: &mut Running, kind: i32, pos: Vec3, facing: i32) -> Option<GObjId> {
    let gobj = zako::spawn(
        r,
        super::ZRSHELL,
        Some(pos),
        None,
        Some(Vec3::new(0.0, 0.0, 0.0)),
        true,
    )?;
    let f = mem(fp::frsp(f64::from(facing)));
    item_mut(r.game, gobj).facing_dir = f;
    zako::face(r, gobj, f);
    set(r, gobj, off::KOOPA, kind);
    change_state(r, gobj, 0xC, change::ANIM_UPDATE);
    let j = jobj_of(r.game, gobj);
    jobj::anim_all(
        &mut r.game.objs,
        r.data,
        &mut crate::game::code::ParticleHooks,
        j,
    );
    settle(r, gobj);
    Some(gobj)
}
