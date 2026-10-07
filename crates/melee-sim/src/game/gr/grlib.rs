//! Shared stage helpers, `melee/gr/grlib.c`.

use melee_data::archive::Archives;

use super::super::state::Game;
use super::ground::ground_of;
use crate::hsd::gobj::GObjId;

/// Measures the players' start points (points of interest 0 to 3), `grLib_801C9A10`: a point
/// the stage lacks keeps its last measure.
#[doc(alias = "grLib_801C9A10")]
pub fn measure_start_points(game: &mut Game, data: &Archives) {
    for i in 0..4 {
        if let Some(p) = super::ground::point(game, data, i) {
            game.gr.start_points[i as usize] = p;
        }
    }
}

/// A player's start point as last measured, `grLib_801C9A70`.
#[doc(alias = "grLib_801C9A70")]
pub fn start_point(game: &Game, i: i32) -> crate::vec::Vec3 {
    assert!((0..4).contains(&i), "grlib.c: a start point");
    game.gr.start_points[i as usize]
}

/// Whether a stage object's color overlay has ended (`x10_flags.b4`), `grLib_801C96E8`.
#[doc(alias = "grLib_801C96E8")]
pub fn overlay_done(game: &Game, gobj: GObjId) -> bool {
    game.grounds[ground_of(game, gobj).0].x10_flags.b4
}

/// Removes the particle generators bound to a joint tree, `grLib_801C9854` (`hsd_8039D5DC`).
#[doc(alias = "grLib_801C9854")]
pub fn kill_joint_generators(
    game: &mut Game,
    data: &melee_data::archive::Archives,
    j: Option<crate::hsd::jobj::JObjId>,
) {
    let mut hooks = crate::game::code::ParticleHooks;
    crate::hsd::generator::kill_on_tree(&mut game.objs, data, &mut hooks, j);
}
