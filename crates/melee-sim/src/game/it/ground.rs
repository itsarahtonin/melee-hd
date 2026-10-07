//! Items on the stage's lines, `melee/it/itgroundcoll.c`.

use super::item;
use crate::game::state::Game;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// Whether the line the item keeps in `xC30` is live, `it_8026D604`.
#[doc(alias = "it_8026D604")]
pub fn on_line(game: &Game, gobj: GObjId) -> bool {
    let line = item(game, gobj).xC30;
    game.geometry
        .as_ref()
        .expect("no collision geometry")
        .line_live(line)
}

/// Where the move from `from` to `to` first crosses the stage, if it does, `it_8026E9A4`
/// (`mpCheckAllRemap` on every surface; the original writes the point over `to`).
#[doc(alias = "it_8026E9A4")]
pub fn stage_crossing(game: &mut Game, from: Vec3, to: Vec3) -> Option<Vec3> {
    let g = game.geometry.as_mut().expect("no collision geometry");
    g.check_all_remap(-1, -1, from.x, from.y, to.x, to.y)
        .map(|hit| hit.pos)
}
