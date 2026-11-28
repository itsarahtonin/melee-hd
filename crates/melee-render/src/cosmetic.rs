//! Slippi's two optional cosmetic codes, done in the presentation (docs/milestone-7.md, task 23
//! and design call 9): the simulation runs as ever and only what's drawn changes, so neither
//! can reach gameplay, and neither goes into a `.slp`'s code list.
//!
//! **No screen shake.** Slippi's "Disable Screen Shake" makes `Camera_RequestQuake` return at
//! once: no quake starts, so the camera's translation (`Camera_ApplyQuake`) stays zero. Here the
//! quake runs in the simulation (its model, counters, log and translation are state) and the
//! drawn camera leaves its displacement out: `Camera_8002AF68` shook the game camera's CObj by
//! adding the translation to its eye and interest (the eye then kept above the stage's floor),
//! and [`Unshake`] takes that back out of the views the display pass recorded. The views the
//! pass derived from the camera's (an instance joint's, an app-SRT's, the lights') follow.
//!
//! **PAL stock icons.** `External/PAL/Core/PAL Stock Icons.asm` (slippi-ssbm-asm), injected at
//! 0x802F9A3C in `ifStock_802F98E8` just after `HSD_JObjSetTranslate` placed a player's row of
//! stock icons at its HUD position, sets the row's root joint's x and y scale to 0.85 and its
//! translate's y to -21, as the PAL version's HUD has them. Nothing animates the root's scale or
//! translate afterwards, so the row keeps them: [`pal_stock_row`] draws the row's joints as the
//! moved and scaled root would place them. A stolen stock's flight (`ifStock_802F7EFC`) is
//! placed relative to the root's translate at both ends, so its icons follow the root too.

use melee_sim::game::camera;
use melee_sim::game::state::{Game, Obj, UserData};
use melee_sim::hsd::cobj::CObjId;
use melee_sim::hsd::gobj::GObjId;
use melee_sim::mtx::Mtx34;
use melee_sim::vec::Vec3;

/// The PAL row's scale and height (`PAL Stock Icons.asm`'s floats).
pub const PAL_STOCK_SCALE: f32 = 0.85;
pub const PAL_STOCK_Y: f32 = -21.0;

/// What leaving the quake's displacement out changes of one camera's views: the view the pass
/// recorded (`V`) becomes `V · world`, and a matrix derived from it (`V · M`) becomes
/// `view · V · M`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Unshake {
    /// In world space: the shaken camera's points moved to where the still camera sees them.
    pub world: Mtx34,
    /// The same in the shaken camera's view space (`V · world · V⁻¹`).
    pub view: Mtx34,
}

impl Unshake {
    /// What camera `c`'s view `v` (as the display pass recorded it) loses without the quake:
    /// none for a camera other than the game camera, in a mode `Camera_8002A4AC` doesn't aim
    /// (the pause camera's view is unshaken), or while the camera isn't shaking.
    ///
    /// The shake is read off the CObj: its interest is the transform's shaken (`aimed` adds
    /// the translation in x and y), its eye too unless the stage's floor raised it. A raised
    /// eye's view is built again from the still eye and interest, its up taken from the shaken
    /// view's (the game camera has no roll).
    pub fn of(game: &Game, c: CObjId, v: &Mtx34) -> Option<Unshake> {
        if crate::blend::game_camera(game) != Some(c) {
            return None;
        }
        let mode = game.camera.mode();
        if mode != camera::STANDARD && mode != camera::FIXED && mode != camera::FREE {
            return None;
        }
        let o = game.objs.cobjs.pool.get(c.0)?;
        let wobj = |w: Option<melee_sim::hsd::wobj::WObjId>| {
            let w = w?;
            game.objs.wobjs.get(w.0).map(|w| w.pos)
        };
        let (eye_s, int_s) = (wobj(o.eyepos)?, wobj(o.interest)?);
        let (pos, int) = (game.camera.position(), game.camera.interest());
        let d = Vec3::new(int_s.x - int.x, int_s.y - int.y, 0.0);
        if d.x == 0.0 && d.y == 0.0 {
            return None;
        }
        let unshaken_y = pos.y + d.y;
        let raised = eye_s.y - unshaken_y > 1e-3 * unshaken_y.abs().max(1.0);
        if !raised {
            let world = Mtx34::translation(d.x, d.y, 0.0);
            let r = v.rotate(d);
            return Some(Unshake {
                world,
                view: Mtx34::translation(r.x, r.y, r.z),
            });
        }
        // The floor held the shaken eye up (`eye_floor`): the still eye is the transform's,
        // held up as far.
        let eye_u = Vec3::new(pos.x, pos.y.max(eye_s.y), pos.z);
        let up = Vec3::new(v.m[1][0], v.m[1][1], v.m[1][2]);
        let still = Mtx34::look_at(eye_u, up, int);
        let inv = v.inverse()?;
        Some(Unshake {
            world: inv.concat(&still),
            view: still.concat(&inv),
        })
    }

    /// Camera view `v` without the shake.
    pub fn camera(&self, v: &Mtx34) -> Mtx34 {
        v.concat(&self.world)
    }

    /// A matrix the pass derived from the shaken view (`V · M`), without the shake.
    pub fn derived(&self, m: &Mtx34) -> Mtx34 {
        self.view.concat(m)
    }
}

/// For a player's row of stock icons (`ifStock_802F98E8`'s GObj), what PAL stock icons does to
/// it, as a matrix in the HUD's world space each of the row's joints is drawn through: the
/// root's own world matrix taken to one with its x and y scaled to [`PAL_STOCK_SCALE`] and its
/// translate's y at [`PAL_STOCK_Y`], so every joint under it moves and scales with it. None for
/// any other GObj.
pub fn pal_stock_row(game: &Game, g: GObjId) -> Option<Mtx34> {
    if game.world.gobjs.is_free(g.0) {
        return None;
    }
    let o = game.world.gobjs.get(g.0)?;
    if !matches!(o.user_data, Some(UserData::StockIcons(_))) {
        return None;
    }
    let Some(Obj::JObj(root)) = o.obj else {
        return None;
    };
    let j = game.objs.jobjs.pool.get(root.0)?;
    if j.parent.is_some() || j.scale.x == 0.0 || j.scale.y == 0.0 {
        return None;
    }
    // A root's world matrix is T·R·S: its scale is in its columns, its translate the fourth.
    let m = j.mtx;
    let mut pal = m;
    let (kx, ky) = (PAL_STOCK_SCALE / j.scale.x, PAL_STOCK_SCALE / j.scale.y);
    for row in &mut pal.m {
        row[0] *= kx;
        row[1] *= ky;
    }
    pal.m[1][3] = PAL_STOCK_Y;
    Some(pal.concat(&m.inverse()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: &Mtx34, b: &Mtx34, tol: f32) -> bool {
        (0..3).all(|r| (0..4).all(|c| (a.m[r][c] - b.m[r][c]).abs() <= tol))
    }

    /// A game whose camera is in the standard mode, its CObj aimed at `int` from `eye` (as the
    /// display pass's `Camera_8002AF68` leaves it), its transform at `pos` and `t_int`.
    fn aimed(eye: Vec3, int: Vec3, pos: Vec3, t_int: Vec3) -> (Box<Game>, CObjId, Mtx34) {
        use melee_sim::hsd::cobj;
        let mut game: Box<Game> = Box::default();
        camera::create(&mut game);
        let c = crate::blend::game_camera(&game).expect("the game camera");
        {
            let parts = game.camera.raw_parts_mut();
            parts.cam.mode = camera::STANDARD;
            parts.cam.transform.position = pos;
            parts.cam.transform.interest = t_int;
        }
        cobj::set_eye_position(&mut game.objs, c, eye);
        cobj::set_interest(&mut game.objs, c, int);
        let v = Mtx34::look_at(eye, cobj::NO_UP_VECTOR, int);
        (game, c, v)
    }

    /// A shaken view loses the translation in x and y: its eye and interest go back to the
    /// transform's, and a still camera, the pause camera and the HUD's lose nothing.
    #[test]
    fn the_shake_comes_out_of_the_game_camera_only() {
        let (pos, int) = (Vec3::new(10.0, 20.0, 150.0), Vec3::new(10.0, 15.0, 0.0));
        let tr = Vec3::new(1.5, -2.25, 0.0);
        let (game, c, v) = aimed(pos.sdk_add(tr), int.sdk_add(tr), pos, int);
        let u = Unshake::of(&game, c, &v).expect("a shaking camera");
        let still = Mtx34::look_at(pos, melee_sim::hsd::cobj::NO_UP_VECTOR, int);
        assert!(close(&u.camera(&v), &still, 1e-4), "{:?}", u.camera(&v));
        // A matrix derived from the view (an instance joint's) moves with it.
        let model = Mtx34::translation(3.0, 4.0, 5.0);
        let derived = v.concat(&model);
        assert!(close(&u.derived(&derived), &still.concat(&model), 1e-4));
        // Still: nothing.
        let (game, c, v) = aimed(pos, int, pos, int);
        assert_eq!(Unshake::of(&game, c, &v), None);
        // Paused: the pause camera's view leaves the translation out.
        let (mut game, c, v) = aimed(pos.sdk_add(tr), int.sdk_add(tr), pos, int);
        game.camera.raw_parts_mut().cam.mode = camera::PAUSE;
        assert_eq!(Unshake::of(&game, c, &v), None);
        // Another camera (the HUD's).
        let hud = CObjId(
            game.objs
                .cobjs
                .pool
                .alloc(melee_sim::hsd::cobj::CObj::default()),
        );
        game.camera.raw_parts_mut().cam.mode = camera::STANDARD;
        assert_eq!(Unshake::of(&game, hud, &v), None);
    }

    /// An eye the stage's floor held up: the still view is built from the transform's eye,
    /// held up as far, and its interest.
    #[test]
    fn a_raised_eye_is_rebuilt() {
        let (pos, int) = (Vec3::new(0.0, -52.0, 120.0), Vec3::new(0.0, -40.0, 0.0));
        let tr = Vec3::new(2.0, -3.0, 0.0);
        let floor = -51.0;
        let eye_s = Vec3::new(pos.x + tr.x, floor, pos.z);
        let (game, c, v) = aimed(eye_s, int.sdk_add(tr), pos, int);
        let u = Unshake::of(&game, c, &v).expect("a shaking camera");
        let still = Mtx34::look_at(
            Vec3::new(pos.x, pos.y.max(floor), pos.z),
            melee_sim::hsd::cobj::NO_UP_VECTOR,
            int,
        );
        assert!(close(&u.camera(&v), &still, 1e-3), "{:?}", u.camera(&v));
    }

    /// A stock row's joints are drawn as the PAL root places them: scaled by 0.85 about the
    /// root and at its height, the root's x and depth kept.
    #[test]
    fn pal_stock_icons_scale_and_lower_the_row() {
        use melee_sim::hsd::jobj::{JObj, JObjId};
        let mut game = Game::default();
        let root = JObjId(game.objs.jobjs.pool.alloc(JObj {
            translate: Vec3::new(-120.0, -18.5, 5.0),
            scale: Vec3::new(1.0, 1.0, 1.0),
            mtx: Mtx34::translation(-120.0, -18.5, 5.0),
            ..JObj::default()
        }));
        let g = game.world.create(14, 15, 0);
        game.world.gobj_mut(g).user_data = Some(UserData::StockIcons(1));
        game.world.gobj_mut(g).obj = Some(Obj::JObj(root));
        let fix = pal_stock_row(&game, g).expect("a stock row");
        // The root lands at the PAL height; a joint 10 units right of it lands 8.5 right.
        let at = fix.transform(Vec3::new(-120.0, -18.5, 5.0));
        assert!((at.x + 120.0).abs() < 1e-4 && (at.y + 21.0).abs() < 1e-4);
        assert!((at.z - 5.0).abs() < 1e-4);
        let icon = fix.transform(Vec3::new(-110.0, -16.5, 5.0));
        assert!((icon.x + 111.5).abs() < 1e-4, "{icon:?}");
        assert!((icon.y + 19.3).abs() < 1e-4, "{icon:?}");
        // Another GObj: nothing.
        game.world.gobj_mut(g).user_data = None;
        assert_eq!(pal_stock_row(&game, g), None);
    }
}
