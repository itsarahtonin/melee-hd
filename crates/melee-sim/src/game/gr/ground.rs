//! The stage's core, `melee/gr/ground.c`: `stage_info`'s upkeep, the stage objects and their
//! processes, map collision, points of interest, camera limits and blast zones, fog and lights.

use gekko_fp as fp;
use melee_data::archive::{Archives, DataRef};

use super::super::handles::{CodeRef, GroundId};
use super::super::state::{Game, Obj, Running, UserData};
use super::super::structs::_GXColor;
use super::data::stage_data;
use super::datfiles::{self, GrJoint};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::hsd::{lobj, wobj};
use crate::mp::JointPose;
use crate::vec::Vec3;

/// A stage GObj's `Ground` (`gobj->user_data`).
pub fn ground_of(game: &Game, gobj: GObjId) -> GroundId {
    match game.world.gobjs[gobj.0].user_data {
        Some(UserData::Ground(g)) => g,
        other => panic!("GObj {gobj:?} holds {other:?}, not a stage object"),
    }
}

/// A GObj's joint (`gobj->hsd_obj`), if it holds one.
pub fn jobj_of(game: &Game, gobj: GObjId) -> Option<JObjId> {
    match game.world.gobjs[gobj.0].obj {
        Some(Obj::JObj(j)) => Some(j),
        None => None,
        other => panic!("GObj {gobj:?} holds {other:?}, not a joint"),
    }
}

impl Game {
    /// The stage's collision geometry, which a stage's code only reaches once it's loaded.
    pub fn geometry_mut(&mut self) -> &mut super::Geometry {
        self.geometry.as_mut().expect("no collision geometry")
    }
}

impl Running<'_> {
    /// A stage sound at full volume, centered, `Ground_801C53EC`.
    #[doc(alias = "Ground_801C53EC")]
    pub fn stage_sound(&mut self, id: i32) {
        self.sfx_play(id, 0x7F, 0x40);
    }
}

/// `mpColl_804D64AC += 1`: the collision geometry's update counter, which the stage code bumps
/// too.
pub fn bump_generation(game: &mut Game) {
    match &mut game.geometry {
        Some(g) => g.generation = g.generation.wrapping_add(1),
        None => game.gr.coll_generation = game.gr.coll_generation.wrapping_add(1),
    }
}

/// The collision geometry's update counter, `mpColl_804D64AC`.
pub fn generation(game: &Game) -> i32 {
    match &game.geometry {
        Some(g) => g.generation,
        None => game.gr.coll_generation,
    }
}

/// The stage's scale, `Ground_801C0498`: its parameters' `y`, or 1 without them.
#[doc(alias = "Ground_801C0498")]
pub fn scale(game: &Game) -> f32 {
    game.gr.param.as_ref().map_or(1.0, |p| p.y)
}

/// The stage object a map id names, `Ground_GetMapGObj`.
#[doc(alias = "Ground_GetMapGObj")]
pub fn map_gobj(game: &Game, map_id: i32) -> Option<GObjId> {
    game.stage_info.map_gobjs[map_id as usize]
}

/// Sets `stage_info.unk8C.b2`, which lets the stage's fog apply, `Ground_801C1E00`.
#[doc(alias = "Ground_801C1E00")]
pub fn set_fog_enabled(game: &mut Game, on: bool) {
    game.stage_info.unk8C.b2 = on;
}

/// Which of the stage parameters' colors (`GroundParam::xB8` through `xD8`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParamColor {
    B8,
    BC,
    C0,
    C4,
    C8,
    CC,
    D0,
    D4,
    D8,
}

/// Sets one of the stage parameters' colors (`Ground_801C052C` and its siblings).
#[doc(alias = "Ground_801C052C")]
#[doc(alias = "Ground_801C0544")]
#[doc(alias = "Ground_801C055C")]
#[doc(alias = "Ground_801C0574")]
#[doc(alias = "Ground_801C058C")]
#[doc(alias = "Ground_801C05A4")]
#[doc(alias = "Ground_801C05BC")]
#[doc(alias = "Ground_801C05D4")]
#[doc(alias = "Ground_801C05EC")]
pub fn set_param_color(game: &mut Game, which: ParamColor, c: _GXColor) {
    let p = game.gr.param.as_mut().expect("stage_info.param is null");
    let slot = match which {
        ParamColor::B8 => &mut p.xB8,
        ParamColor::BC => &mut p.xBC,
        ParamColor::C0 => &mut p.xC0,
        ParamColor::C4 => &mut p.xC4,
        ParamColor::C8 => &mut p.xC8,
        ParamColor::CC => &mut p.xCC,
        ParamColor::D0 => &mut p.xD0,
        ParamColor::D4 => &mut p.xD4,
        ParamColor::D8 => &mut p.xD8,
    };
    *slot = c;
}

/// The stage fog's color, `Ground_801C2090`, if there is a fog.
#[doc(alias = "Ground_801C2090")]
pub fn fog_color(game: &Game) -> Option<_GXColor> {
    let g = game.stage_info.x12C?;
    match game.world.gobjs[g.0].obj {
        Some(Obj::Fog(f)) => {
            let [r, g, b, a] = game.objs.fogs[f].color;
            Some(_GXColor { r, g, b, a })
        }
        _ => None,
    }
}

/// Sets the stage fog's color, `Ground_801C205C`.
#[doc(alias = "Ground_801C205C")]
pub fn set_fog_color(game: &mut Game, c: _GXColor) {
    let Some(g) = game.stage_info.x12C else {
        return;
    };
    if let Some(Obj::Fog(f)) = game.world.gobjs[g.0].obj {
        game.objs.fogs[f].color = [c.r, c.g, c.b, c.a];
        game.objs.fogs[f].stale_alpha = false;
    }
}

/// [`set_fog_color`] with a color whose alpha its caller never wrote: the alpha is whatever
/// that stack byte held (`docs/engine.md`, "Uninitialized values"), so the fog keeps its old
/// alpha, marked stale.
pub fn set_fog_rgb(game: &mut Game, r: u8, g: u8, b: u8) {
    let Some(gobj) = game.stage_info.x12C else {
        return;
    };
    if let Some(Obj::Fog(f)) = game.world.gobjs[gobj.0].obj {
        let fog = &mut game.objs.fogs[f];
        fog.color[..3].copy_from_slice(&[r, g, b]);
        fog.stale_alpha = true;
    }
}

/// The `depth`th joint below a stage object's root in depth-first order, not entering
/// instances' children, `Ground_801C3FA4`. Depth 0 is the root's first child.
#[doc(alias = "Ground_801C3FA4")]
pub fn nth_joint(game: &Game, gobj: GObjId, depth: i32) -> Option<JObjId> {
    let jobjs = &game.objs.jobjs;
    let root = jobj_of(game, gobj)?;
    let mut j = jobjs[root].child?;
    let mut depth = depth;
    while depth != 0 {
        depth -= 1;
        let o = &jobjs[j];
        if o.flags & jobj::INSTANCE == 0
            && let Some(c) = o.child
        {
            j = c;
            continue;
        }
        if let Some(n) = o.next {
            j = n;
            continue;
        }
        let mut up = j;
        loop {
            let p = jobjs[up].parent?;
            if let Some(n) = jobjs[p].next {
                j = n;
                break;
            }
            up = p;
        }
    }
    Some(j)
}

/// A point of interest's position (`Ground_801C2D24`): a stage joint `stage_info.x280`
/// lists, with fallbacks between related points; none if the stage has no such point.
#[doc(alias = "Ground_801C2D24")]
pub fn point(game: &mut Game, data: &Archives, id: i32) -> Option<Vec3> {
    let half = |a: Vec3, b: Vec3| {
        let h = |a: f32, b: f32| mem(fp::fmuls(fp::fadds(reg(a), reg(b)), 0.5));
        Vec3::new(h(a.x, b.x), h(a.y, b.y), h(a.z, b.z))
    };
    // The original adds into its out parameter whether or not the inner lookups found
    // anything; on a stage missing those points it would read whatever was there.
    let found = |game: &mut Game, i| {
        point(game, data, i).unwrap_or_else(|| panic!("point {i} reads a stale vector"))
    };
    if id == 8 {
        let a = found(game, 4);
        let b = found(game, 5);
        return Some(half(a, b));
    }
    if id == 9 {
        let a = found(game, 6);
        let b = found(game, 7);
        return Some(half(a, b));
    }
    if let Some(j) = game.stage_info.x280[id as usize] {
        return Some(crate::lb::joint::world_position(
            &mut game.objs,
            data,
            Some(j),
            None,
        ));
    }
    if (id as u32).wrapping_sub(1) <= 2 {
        return point(game, data, 0);
    }
    if (id as u32).wrapping_sub(5) <= 2 {
        return point(game, data, 4);
    }
    if id == 0x7F {
        if let Some(mut p) = point(game, data, 0x94) {
            p.y = mem(fp::fadds(reg(p.y), 50.0));
            return Some(p);
        }
        return point(game, data, 0);
    }
    if id == 4 {
        return Some(super::stage::cam_center(game));
    }
    None
}

/// Moves a collision joint with its model joint (`mpLib_80055E9C`), which reads the model
/// joint's hidden flag, else its world matrix brought up to date.
pub(super) fn update_coll_joint(game: &mut Game, data: &Archives, joint: i32) {
    let Game { geometry, objs, .. } = game;
    let g = geometry.as_mut().expect("no collision geometry");
    g.update_joint(joint, |j| {
        if objs.jobjs[j].flags & jobj::HIDDEN != 0 {
            return JointPose::Hidden;
        }
        jobj::setup_matrix(objs, data, Some(j));
        JointPose::Visible(objs.jobjs[j].mtx)
    });
}

/// The stage's collision joints for a map id: the stage's own table's, then where its
/// archive's are and how many (read as they're used, from data that doesn't change).
fn coll_joints(
    game: &Game,
    data: &Archives,
    map_id: i32,
) -> (&'static [GrJoint], Option<DataRef>, u32) {
    let own = stage_data(game.stage_info.grkind).joints;
    match datfiles::archive_for(game, data, map_id) {
        Some(head) => {
            let (list, n) = head.entry(data, map_id).coll_joints(data);
            (own, list, n.max(0) as u32)
        }
        None => (own, None, 0),
    }
}

/// Moves a map's collision joints with its model, `Ground_UpdateMapColl`, unless the map's
/// collision is switched off: the stage's own joints for the map, then the archive's not
/// already moved this time (each is stamped with a new value). Whether any moved.
#[doc(alias = "Ground_UpdateMapColl")]
pub fn update_map_coll(game: &mut Game, data: &Archives, gobj: GObjId) -> bool {
    let map_id = game.grounds[ground_of(game, gobj).0].map_id;
    let off = game
        .gr
        .coll_off
        .as_ref()
        .expect("Ground_804D6950 isn't allocated");
    if off[map_id as usize] != 0 {
        return false;
    }
    let mut moved = false;
    game.gr.coll_stamp = game.gr.coll_stamp.wrapping_add(1);
    let stamp = game.gr.coll_stamp;
    let (own, list, n) = coll_joints(game, data, map_id);
    for j in own.iter().filter(|j| i32::from(j.y) == map_id) {
        update_coll_joint(game, data, j.x.into());
        game.geometry.as_mut().unwrap().joints[j.x as usize].stamp = stamp;
        moved = true;
    }
    for i in 0..n {
        let j = &GrJoint::read(data, list.expect("collision joints").at(6 * i));
        let g = game.geometry.as_mut().unwrap();
        if stamp != g.joints[j.x as usize].stamp {
            update_coll_joint(game, data, j.x.into());
            game.geometry.as_mut().unwrap().joints[j.x as usize].stamp = stamp;
            moved = true;
        }
    }
    moved
}

/// A stage object's first process (priority 1), `Ground_801C1CD0`: animates its model and
/// color overlay, counts a collision update, and calls its `x8_callback`.
#[doc(alias = "Ground_801C1CD0")]
pub fn proc_anim(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj);
    jobj::anim_all(
        &mut r.game.objs,
        r.data,
        &mut crate::game::code::ParticleHooks,
        j,
    );
    super::material::run_ground_overlay(r.game, r.data, gobj);
    bump_generation(r.game);
    let gp = ground_of(r.game, gobj);
    if let Some(cb) = r.game.grounds[gp.0].x8_callback {
        super::super::code::run_proc(r, gobj, cb);
    }
}

/// A stage object's second process (priority 4), `Ground_801C1D38`: its `xC_callback`.
#[doc(alias = "Ground_801C1D38")]
pub fn proc_callback(r: &mut Running, gobj: GObjId) {
    let gp = ground_of(r.game, gobj);
    if let Some(cb) = r.game.grounds[gp.0].xC_callback {
        super::super::code::run_proc(r, gobj, cb);
    }
}

/// The stage's watcher process (priority 10), `Ground_801C0C2C`: finds the point of interest
/// player 1 stands in when asked (`unk8C.b6`, `b7`), and rains bob-ombs in that mode.
#[doc(alias = "Ground_801C0C2C")]
pub fn proc_watch(r: &mut Running, _gobj: GObjId) {
    let info = &r.game.stage_info;
    if info.unk8C.b6 || info.unk8C.b7 {
        unimplemented!("Ground_801C0C2C's point search needs the fighters (Player_GetEntity)");
    }
    // `gm_8016B238`: the rules' bob-omb rain.
    if r.game.vs_x6() != 0 {
        unimplemented!("bob-omb rain needs the items (it_8026BE84)");
    }
}

/// Animates a light list, keeping moved positions in the stage's scale, `Ground_801C445C`. The
/// original reads the first light's world positions for every light of the list.
#[doc(alias = "Ground_801C445C")]
pub fn anim_lights(game: &mut Game, data: &Archives, first: Option<lobj::LObjId>) {
    let Some(first) = first else { return };
    let mut cur = Some(first);
    let differs =
        |a: Vec3, b: Vec3| reg(a.x) != reg(b.x) || reg(a.y) != reg(b.y) || reg(a.z) != reg(b.z);
    while let Some(l) = cur {
        let pos_w = lobj::position_wobj(&game.objs.lobjs, Some(first));
        let pos0 = wobj::position(&mut game.objs, data, pos_w);
        let int_w = lobj::interest_wobj(&game.objs.lobjs, Some(first));
        let pos2 = wobj::position(&mut game.objs, data, int_w);
        lobj::anim(&mut game.objs, data, Some(l));
        let wobjs: [WObjOf; 2] = [lobj::position_wobj, lobj::interest_wobj];
        for (before, w) in [pos0, pos2].into_iter().zip(wobjs) {
            let w = w(&game.objs.lobjs, Some(first));
            if let Some(after) = wobj::position(&mut game.objs, data, w)
                && differs(before.expect("a world position appeared"), after)
            {
                let s = reg(scale(game));
                let m = |v: f32| mem(fp::fmuls(reg(v), s));
                let scaled = Vec3::new(m(after.x), m(after.y), m(after.z));
                wobj::set_position(&mut game.objs.wobjs, w, scaled);
            }
        }
        cur = game.objs.lobjs[l].next;
    }
}

/// A light's position or interest WObj.
type WObjOf = fn(&lobj::LObjs, Option<lobj::LObjId>) -> Option<wobj::WObjId>;

/// The stage lights' process, `Ground_801C461C`.
#[doc(alias = "Ground_801C461C")]
pub fn proc_lights(r: &mut Running, gobj: GObjId) {
    let first = match r.game.world.gobjs[gobj.0].obj {
        Some(Obj::LObj(l)) => Some(l),
        None => None,
        other => panic!("the stage lights' GObj holds {other:?}"),
    };
    anim_lights(r.game, r.data, first);
}

/// A stage object's process function, called as `HSD_GObjEvent`.
pub fn call(r: &mut Running, gobj: GObjId, f: CodeRef) {
    super::super::code::run_proc(r, gobj, f);
}

/// Ages the wind sources that sway jointed parts and sets whether wind blows
/// (`lb_804D63B4`), [`crate::lb::wind::tick`].
pub fn quake_list_tick(game: &mut Game) {
    game.gr.wind_level = crate::lb::wind::tick(&mut game.gr.winds, game.gr.wind_level);
}

/// How often items appear on stage `stkind`, `Ground_801C2AE8`: the stage parameters' and its
/// entry's percentages, multiplied, in a register.
#[doc(alias = "Ground_801C2AE8")]
pub fn item_freq_scale(game: &Game, data: &Archives, stkind: i32) -> f64 {
    let p = game.gr.param.as_ref().expect("stage_info.param is null");
    let pct = |v: i16| fp::fmuls(f64::from(0.01_f32), fp::frsp(f64::from(v)));
    for i in 0..p.stage_param_count.max(0) as u32 {
        let e = p.stage_params.expect("no stage parameters").at(0x64 * i);
        if data.u32(e) as i32 == stkind {
            return fp::fmuls(pct(p.x68), pct(data.u16(e.at(0x18)) as i16));
        }
    }
    panic!("stage {stkind} has no stage parameters (ground.c:2343)")
}
