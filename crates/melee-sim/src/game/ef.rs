//! The effects library, `melee/ef`: the parts other objects reach so far. Effects themselves
//! (their GObjs on process links 11 and 12, and the particles they start) aren't ported.

use super::handles::EffectId;
use super::state::{Game, Running, UserData};
use super::structs::EF_Effect;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::hsd::pool::Pool;

/// The effects' process links (`HSD_GOBJ_PLINK_EFFECT_UNK0` and `UNK1`).
pub const P_LINK_EFFECTS: [u8; 2] = [11, 12];

/// An entry of `efLib_ParamTable`: an object an effect's parameters follow.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Param {
    pub gobj: Option<GObjId>,
    pub gfx_id: u16,
    pub alpha: u16,
}

/// The effects library's globals.
#[derive(Clone, Debug, Default)]
pub struct Globals {
    /// `efLib_ParamTable`.
    pub params: [Param; 8],
    /// The effects (`efLib_AllocData`).
    pub effects: Pool<EF_Effect>,
    /// How many effects that aren't async are out (`efLib_EffectCount`).
    pub count: i32,
}

/// Frees an effect, `efLib_remove_user_data`: an effect that isn't async counts out.
#[doc(alias = "efLib_remove_user_data")]
pub fn free_user_data(game: &mut Game, e: EffectId) {
    let fx = &game.ef.effects[e.0];
    assert!(fx.gobj.is_some(), "Duplicate Free");
    if fx.is_async == 0 {
        game.ef.count = game.ef.count.wrapping_sub(1);
    }
    game.ef.effects.free(e.0);
}

/// Destroys an effect's GObj: the generators following its joints, then the GObj (as
/// `efLib_DestroyAll` does for each effect it finds).
pub fn destroy_effect(r: &mut Running, gobj: GObjId) {
    detach_tree(r, gobj);
    crate::hsd::gobj::free(r, gobj);
}

/// Removes the generators following a GObj's joints (`HSD_JObjWalkTree` with `hsd_8039D688`).
pub fn detach_tree(r: &mut Running, gobj: GObjId) {
    let g = &r.game.world.gobjs[gobj.0];
    if g.obj_kind == super::state::OBJ_JOBJ
        && let Some(super::state::Obj::JObj(root)) = g.obj
    {
        let mut joints = Vec::new();
        jobj::walk_tree(&r.game.objs.jobjs, Some(root), &mut |j, _| joints.push(j));
        for j in joints {
            crate::hsd::generator::detach_jobj(
                &mut r.game.objs,
                r.data,
                &mut super::code::ParticleHooks,
                j,
            );
        }
    }
}

/// Destroys the effects an object owns, `efLib_DestroyAll`: forgets it in the parameter table,
/// frees the effects whose parent it is, and removes the particle generators following its
/// joints (`hsd_8039D688`).
#[doc(alias = "efLib_DestroyAll")]
pub fn destroy_all(r: &mut Running, gobj: GObjId) {
    for p in &mut r.game.ef.params {
        if p.gobj == Some(gobj) {
            p.gobj = None;
        }
    }
    // The effects' GObjs keep their effect as user data; those without (the effects' own
    // processes) are skipped.
    for link in P_LINK_EFFECTS {
        let mut cur = r.game.world.first_in(link);
        while let Some(g) = cur {
            let next = r.game.world.gobjs[g.0].next;
            match r.game.world.gobjs[g.0].user_data {
                Some(UserData::Effect(e)) => {
                    let fx = &r.game.ef.effects[e.0];
                    if fx.parent_gobj == Some(gobj) {
                        let fx_gobj = fx.gobj.expect("an effect without a GObj");
                        destroy_effect(r, fx_gobj);
                    }
                }
                Some(other) => panic!("an effect GObj holds {other:?}"),
                None => {}
            }
            cur = next;
        }
    }
    detach_tree(r, gobj);
}
