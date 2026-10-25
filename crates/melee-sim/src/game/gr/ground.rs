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

/// A stage GObj's `Ground` (`gobj->user_data`; Pokémon Stadium's transformations have it as a
/// function of their own, `grStadium_801D4354`).
#[doc(alias = "grStadium_801D4354")]
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

/// The stage's `yakumono_param`, `Ground_GetYakumonoParam`.
#[doc(alias = "Ground_GetYakumonoParam")]
pub fn yakumono_param(game: &Game) -> Option<DataRef> {
    game.stage_info.yakumono_param
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
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
    nth_joint_below(&game.objs.jobjs, jobj_of(game, gobj)?, depth)
}

/// [`nth_joint`] from the object's root joint, for callers that hold the joints alone (a
/// collision joint's callback).
pub fn nth_joint_below(jobjs: &jobj::JObjs, root: JObjId, depth: i32) -> Option<JObjId> {
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

/// The joint after `j` in depth-first order, not entering an instance's children, or none
/// past the tree's end, `Ground_801C4100`.
#[doc(alias = "Ground_801C4100")]
pub fn next_in_tree(jobjs: &jobj::JObjs, j: JObjId) -> Option<JObjId> {
    let o = &jobjs[j];
    if o.flags & jobj::INSTANCE == 0
        && let Some(c) = o.child
    {
        return Some(c);
    }
    if let Some(n) = o.next {
        return Some(n);
    }
    let mut up = j;
    loop {
        let p = jobjs[up].parent?;
        if let Some(n) = jobjs[p].next {
            return Some(n);
        }
        up = p;
    }
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
        set_stamp(game, j.x as u16, stamp);
        moved = true;
    }
    for i in 0..n {
        let j = &GrJoint::read(data, list.expect("collision joints").at(6 * i));
        let g = game.geometry.as_mut().unwrap();
        // A stamp the load left over the GPU's pixels: what the console compares here is a
        // rendered texture's bytes (docs/melee-bugs.md).
        assert!(
            !g.render_stamps.contains(&(j.x as u16)),
            "Ground_UpdateMapColl reads joint {}'s stamp, which the GPU's pixels left",
            j.x
        );
        if stamp != g.joints[j.x as usize].stamp {
            update_coll_joint(game, data, j.x.into());
            set_stamp(game, j.x as u16, stamp);
            moved = true;
        }
    }
    moved
}

/// Stamps a collision joint with this update's value.
fn set_stamp(game: &mut Game, joint: u16, stamp: i16) {
    let g = game.geometry.as_mut().unwrap();
    g.joints[joint as usize].stamp = stamp;
    g.render_stamps.retain(|&j| j != joint);
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
/// player 1 stands in when asked (`unk8C.b6`, `b7`), and rains bob-ombs in that mode (not
/// ported past a bomb's starting point: the bomb is an item, `it_8026BE84`).
#[doc(alias = "Ground_801C0C2C")]
pub fn proc_watch(r: &mut Running, _gobj: GObjId) {
    let info = &r.game.stage_info;
    if info.unk8C.b6 || info.unk8C.b7 {
        unimplemented!("Ground_801C0C2C's point search needs the fighters (Player_GetEntity)");
    }
    // `gm_8016B238`: the rules' bob-omb rain, a bomb every 30 frames after the first 1200.
    if r.game.vs_x6() != 0 {
        let frame = r.game.vs_frame_count();
        if frame as i32 > 0x4B0 && frame.wrapping_sub(r.game.stage_info.x9C) > 0x1E {
            r.game.stage_info.x9C = frame;
            // `sp2C` is a stack vector the point fills whenever it finds one; it isn't read
            // otherwise.
            let (found, _at) = bomb_rain_point(r.game, r.data, Vec3::default());
            if found {
                super::super::code::unported_call("it_8026BE84 (a bob-omb of the rain)");
            }
        }
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

// `stage_info`'s settings and queries, the stage parameters' colors, and helpers other code
// calls with the stage's objects.

/// Sets the stage's scale, `Ground_SetParamY`.
#[doc(alias = "Ground_SetParamY")]
pub fn set_scale(game: &mut Game, y: f32) {
    game.gr.param.as_mut().expect("ground.c: param").y = y;
}

/// The stage parameters' `x4`, or 128 without them, `Ground_801C0508`.
#[doc(alias = "Ground_801C0508")]
pub fn param_x4(game: &Game) -> i32 {
    game.gr.param.as_ref().map_or(128, |p| i32::from(p.x4))
}

/// One of the stage parameters' colors (`Ground_801C0604` and its siblings hand back where it
/// is).
#[doc(alias = "Ground_801C0604")]
#[doc(alias = "Ground_801C0618")]
#[doc(alias = "Ground_801C062C")]
#[doc(alias = "Ground_801C0640")]
#[doc(alias = "Ground_801C0654")]
#[doc(alias = "Ground_801C0668")]
#[doc(alias = "Ground_801C067C")]
#[doc(alias = "Ground_801C0690")]
#[doc(alias = "Ground_801C06A4")]
pub fn param_color(game: &Game, which: ParamColor) -> _GXColor {
    let p = game.gr.param.as_ref().expect("stage_info.param is null");
    match which {
        ParamColor::B8 => p.xB8.clone(),
        ParamColor::BC => p.xBC.clone(),
        ParamColor::C0 => p.xC0.clone(),
        ParamColor::C4 => p.xC4.clone(),
        ParamColor::C8 => p.xC8.clone(),
        ParamColor::CC => p.xCC.clone(),
        ParamColor::D0 => p.xD0.clone(),
        ParamColor::D4 => p.xD4.clone(),
        ParamColor::D8 => p.xD8.clone(),
    }
}

/// Runs the stage's demo setup, `Ground_DemoInit`.
#[doc(alias = "Ground_DemoInit")]
pub fn demo_init(r: &mut Running, grkind: i32, arg: i32) {
    let f = stage_data(grkind).on_demo_init;
    super::super::code::run_demo_init(r, f, arg);
}

/// `Ground_801C1154` does nothing.
#[doc(alias = "Ground_801C1154")]
pub fn ready(_game: &mut Game) {}

/// Sets a stage object's model's scale back to 1, `Ground_801C11AC`.
#[doc(alias = "Ground_801C11AC")]
pub fn reset_scale(game: &mut Game, gobj: GObjId) {
    let j = jobj_of(game, gobj).expect("HSD_JObjSetScale: jobj");
    jobj::set_srt(&mut game.objs.jobjs, j, |o| {
        o.scale = Vec3::new(1.0, 1.0, 1.0)
    });
}

/// The joint descriptor `depth` steps into a model, depth first, `Ground_801C126C`: none past
/// the model's end.
#[doc(alias = "Ground_801C126C")]
pub fn nth_joint_desc(data: &Archives, node: DataRef, depth: &mut i32) -> Option<DataRef> {
    *depth -= 1;
    if *depth < 0 {
        return Some(node);
    }
    if let Some(c) = data.ptr(node.at(8))
        && let Some(found) = nth_joint_desc(data, c, depth)
    {
        return Some(found);
    }
    if let Some(n) = data.ptr(node.at(0xC))
        && let Some(found) = nth_joint_desc(data, n, depth)
    {
        return Some(found);
    }
    None
}

/// Loads a joint of a map's model (`depth` steps in, depth first) as a tree of its own,
/// `Ground_801C13D0`.
#[doc(alias = "Ground_801C13D0")]
pub fn load_map_joint(game: &mut Game, data: &Archives, map_id: i32, depth: i32) -> Option<JObjId> {
    let head = datfiles::archive_for(game, data, map_id)?;
    if map_id >= head.count(data) {
        return None;
    }
    let root = head.entry(data, map_id).joint(data);
    let joint = if depth == 0 {
        root
    } else {
        let mut d = depth;
        nth_joint_desc(data, root.expect("ground.c: joint"), &mut d)
    };
    jobj::load_joint(&mut game.objs, data, joint)
}

/// The stage kinds the bob-omb rain falls on players over (`Ground_801C0A70`'s
/// `enabled_stages`): every VS stage but Icicle Mountain and Flat Zone.
const RAIN_OVER_PLAYERS: [i32; 27] = [
    2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 24, 25, 28, 29, 30, 36,
    37,
];

/// Where a bob-omb of the rain starts, `Ground_801C0A70`: half the time, on a stage that
/// allows it, above a random player who is there (five below the top blast zone, up to 50 to
/// either side); otherwise a random item point, or the players' start points, through
/// `Stage_80224FDC`. Whether it found a place, and the place (`pos` as it was when it found
/// none).
#[doc(alias = "Ground_801C0A70")]
pub fn bomb_rain_point(game: &mut Game, data: &Archives, pos: Vec3) -> (bool, Vec3) {
    if game.objs.rng.below(2) != 0 && RAIN_OVER_PLAYERS.contains(&game.stage_info.grkind) {
        let slot = game.objs.rng.below(4);
        if game.player_entity(slot).is_some() {
            let mut p = game.player_pos(slot);
            p.y = mem(fp::fadds(fp::fneg(5.0), reg(super::stage::blast_top(game))));
            let xoff = game.objs.rng.below(100).wrapping_sub(50);
            p.x = mem(fp::fadds(reg(p.x), fp::frsp(f64::from(xoff))));
            return (true, p);
        }
    }
    match super::stage::random_item_point(game, data) {
        Some(p) => (true, p),
        None => (false, pos),
    }
}

/// Sets bits of `stage_info.flags`, `Ground_801C1D6C`.
#[doc(alias = "Ground_801C1D6C")]
pub fn set_flags(game: &mut Game, bits: u32) {
    game.stage_info.flags |= bits;
}

/// `stage_info.flags` under a mask: `Ground_801C1D84` (a bonus stage's end, 0x330),
/// `Ground_801C1D98` (0x20, every target broken), `Ground_801C1DAC` (0x100) and
/// `Ground_801C1DC0` (0x80).
#[doc(alias = "Ground_801C1D84")]
#[doc(alias = "Ground_801C1D98")]
#[doc(alias = "Ground_801C1DAC")]
#[doc(alias = "Ground_801C1DC0")]
pub fn flags(game: &Game, mask: u32) -> u32 {
    game.stage_info.flags & mask
}

/// `stage_info.x6D0`, `Ground_801C1DD4`.
#[doc(alias = "Ground_801C1DD4")]
pub fn x6d0(game: &Game) -> i32 {
    i32::from(game.stage_info.x6D0)
}

/// Whether the stage's fog applies (`unk8C.b2`), `Ground_801C1E18`.
#[doc(alias = "Ground_801C1E18")]
pub fn fog_enabled(game: &Game) -> bool {
    game.stage_info.unk8C.b2
}

/// The stage's fog GObj, `Ground_801C1E84`.
#[doc(alias = "Ground_801C1E84")]
pub fn fog_gobj(game: &Game) -> Option<GObjId> {
    game.stage_info.x12C
}

/// Gives the camera the stage fog's color as its clear color (black without a fog),
/// `Ground_ApplyStageBackgroundColor`.
#[doc(alias = "Ground_ApplyStageBackgroundColor")]
pub fn apply_background_color(game: &mut Game) {
    let (r, g, b) = fog_color(game).map_or((0, 0, 0), |c| (c.r, c.g, c.b));
    super::super::camera::set_background_color(&mut game.camera.cam, r, g, b);
}

/// The camera's vertical tilt, `Ground_801C20D0`.
#[doc(alias = "Ground_801C20D0")]
pub fn cam_vertical_tilt(game: &Game) -> f32 {
    game.stage_info.cam_info.cam_vertical_tilt
}

/// Puts a light list's positions and interests in the stage's scale, `Ground_801C2374`.
#[doc(alias = "Ground_801C2374")]
pub fn scale_lights(game: &mut Game, data: &Archives, first: Option<lobj::LObjId>) {
    let s = reg(scale(game));
    let scaled = |v: Vec3| {
        Vec3::new(
            mem(fp::fmuls(reg(v.x), s)),
            mem(fp::fmuls(reg(v.y), s)),
            mem(fp::fmuls(reg(v.z), s)),
        )
    };
    let mut cur = first;
    while let Some(l) = cur {
        if let Some(p) = lobj::position(&mut game.objs, data, Some(l)) {
            lobj::set_position(&mut game.objs, l, scaled(p));
        }
        if let Some(i) = lobj::interest(&mut game.objs, data, Some(l)) {
            lobj::set_interest(&mut game.objs, l, scaled(i));
        }
        cur = game.objs.lobjs[l].next;
    }
}

/// A map's spline (`unk10[i]` of the archive's `map_head`, if it has that many),
/// `Ground_801C247C`.
#[doc(alias = "Ground_801C247C")]
pub fn spline(game: &Game, data: &Archives, map_id: i32, i: i32) -> Option<DataRef> {
    let head = datfiles::archive_for(game, data, map_id).expect("ground.c: archive");
    if i < data.u32(head.0.at(0x14)) as i32 {
        data.ptr(data.ptr(head.0.at(0x10))?.at(4 * i as u32))
    } else {
        None
    }
}

/// A point of interest's joint, `Ground_801C2CF4`.
#[doc(alias = "Ground_801C2CF4")]
pub fn point_joint(game: &Game, i: i32) -> Option<JObjId> {
    game.stage_info.x280[i as usize]
}

/// Sets a point of interest's joint, `Ground_801C2D0C`.
#[doc(alias = "Ground_801C2D0C")]
pub fn set_point_joint(game: &mut Game, i: i32, j: Option<JObjId>) {
    game.stage_info.x280[i as usize] = j;
}

/// Sets the camera limits' top edge, `Ground_801C3880`.
#[doc(alias = "Ground_801C3880")]
pub fn set_cam_top(game: &mut Game, v: f32) {
    game.stage_info.cam_info.cam_bounds.top = v;
}

/// Sets the camera limits' bottom edge, `Ground_801C3890`.
#[doc(alias = "Ground_801C3890")]
pub fn set_cam_bottom(game: &mut Game, v: f32) {
    game.stage_info.cam_info.cam_bounds.bottom = v;
}

/// Sets the camera limits' left edge, `Ground_801C38A0`.
#[doc(alias = "Ground_801C38A0")]
pub fn set_cam_left(game: &mut Game, v: f32) {
    game.stage_info.cam_info.cam_bounds.left = v;
}

/// Sets the camera limits' right edge, `Ground_801C38AC`.
#[doc(alias = "Ground_801C38AC")]
pub fn set_cam_right(game: &mut Game, v: f32) {
    game.stage_info.cam_info.cam_bounds.right = v;
}

/// Sets the camera's offset, `Ground_801C38BC`.
#[doc(alias = "Ground_801C38BC")]
pub fn set_cam_offset(game: &mut Game, x: f32, y: f32) {
    game.stage_info.cam_info.cam_x_offset = x;
    game.stage_info.cam_info.cam_y_offset = y;
}

/// Sets the top blast zone, `Ground_801C3980`.
#[doc(alias = "Ground_801C3980")]
pub fn set_blast_top(game: &mut Game, v: f32) {
    game.stage_info.blast_zone.top = v;
}

/// Sets the bottom blast zone, `Ground_801C3990`.
#[doc(alias = "Ground_801C3990")]
pub fn set_blast_bottom(game: &mut Game, v: f32) {
    game.stage_info.blast_zone.bottom = v;
}

/// Sets the left blast zone, `Ground_801C39A0`.
#[doc(alias = "Ground_801C39A0")]
pub fn set_blast_left(game: &mut Game, v: f32) {
    game.stage_info.blast_zone.left = v;
}

/// Sets the right blast zone, `Ground_801C39B0`.
#[doc(alias = "Ground_801C39B0")]
pub fn set_blast_right(game: &mut Game, v: f32) {
    game.stage_info.blast_zone.right = v;
}

/// Asks the stage's watcher to find the point player 1 stands in (`unk8C.b6`), with half the
/// box's sizes and what to call, `Ground_801C3D44`: the point found before, if any.
#[doc(alias = "Ground_801C3D44")]
pub fn watch_points(game: &mut Game, cb: Option<CodeRef>, w: f32, h: f32) -> i32 {
    let i = &mut game.stage_info;
    i.unk8C.b6 = true;
    i.x70C = mem(fp::fmuls(0.5, reg(w)));
    i.x710 = mem(fp::fmuls(0.5, reg(h)));
    i.x90 = cb;
    if i.x714 != -1 {
        i.flags |= 0x10;
        i.x6D0 = i.x714 as i16;
    } else {
        i.flags &= !0x10;
    }
    i.x714
}

/// The second such search (`unk8C.b7`), `Ground_801C3DB4`.
#[doc(alias = "Ground_801C3DB4")]
pub fn watch_points2(game: &mut Game, cb: Option<CodeRef>, w: f32, h: f32) -> i32 {
    let i = &mut game.stage_info;
    i.unk8C.b7 = true;
    i.x718 = mem(fp::fmuls(0.5, reg(w)));
    i.x71C = mem(fp::fmuls(0.5, reg(h)));
    i.x94 = cb;
    if i.x720 != -1 {
        i.flags |= 0x40;
    } else {
        i.flags &= !0x40;
    }
    i.x720
}

/// The first animation in a joint tree, depth first, `Ground_801C3E18`.
#[doc(alias = "Ground_801C3E18")]
pub fn first_aobj(game: &Game, j: Option<JObjId>) -> Option<crate::hsd::aobj::AObjId> {
    let j = j?;
    let o = &game.objs.jobjs[j];
    if o.aobj.is_some() {
        return o.aobj;
    }
    first_aobj(game, o.child).or_else(|| first_aobj(game, game.objs.jobjs[j].next))
}

/// The frame of a joint tree's first animation, or 0, `Ground_801C3F20`.
#[doc(alias = "Ground_801C3F20")]
pub fn anim_frame(game: &Game, j: Option<JObjId>) -> f32 {
    first_aobj(game, j).map_or(0.0, |a| game.objs.aobjs[a].curr_frame)
}

/// Counts a target down, `Ground_801C4338`: the last sets `flags` 0x20.
#[doc(alias = "Ground_801C4338")]
pub fn target_broken(game: &mut Game) {
    let i = &mut game.stage_info;
    i.x6D4 = i.x6D4.wrapping_sub(1);
    if i.x6D4 == 0 {
        i.flags |= 0x20;
    }
}

/// `x724` and how far it moved last (`x724 - x728`), `Ground_801C4368`.
#[doc(alias = "Ground_801C4368")]
pub fn x724_and_step(game: &Game) -> (f32, f32) {
    let i = &game.stage_info;
    (i.x724, mem(fp::fsubs(reg(i.x724), reg(i.x728))))
}

/// Sets `x724`, keeping its last value in `x728`, `Ground_801C438C`.
#[doc(alias = "Ground_801C438C")]
pub fn set_x724(game: &mut Game, v: f32) {
    game.stage_info.x728 = game.stage_info.x724;
    game.stage_info.x724 = v;
}

/// The stage's light GObj (the first of class 13 on the lights' link), `Ground_801C498C`.
#[doc(alias = "Ground_801C498C")]
pub fn light_gobj(game: &Game) -> Option<GObjId> {
    let mut cur = game.world.first_in(3);
    while let Some(g) = cur {
        if game.world.gobj(g).classifier == 13 {
            return Some(g);
        }
        cur = game.world.gobj(g).next;
    }
    None
}

/// Remembers a barrel's object, place and value for the stages that throw it,
/// `Ground_801C4D70`.
#[doc(alias = "Ground_801C4D70")]
pub fn set_barrel(game: &mut Game, gobj: Option<GObjId>, pos: Vec3, v: f32) -> bool {
    let i = &mut game.stage_info;
    i.x72C = gobj;
    i.x730 = pos;
    i.x73C = v;
    true
}

/// The barrel's place and value, `Ground_801C4DA0`.
#[doc(alias = "Ground_801C4DA0")]
pub fn barrel(game: &Game) -> (Vec3, f32) {
    (game.stage_info.x730, game.stage_info.x73C)
}

/// Six joints' world positions as the stage's guide points (`x130` to `x16C`, `unk8C.b3`),
/// `Ground_801C4E70`.
#[doc(alias = "Ground_801C4E70")]
pub fn set_guide_points(game: &mut Game, data: &Archives, joints: [Option<JObjId>; 6]) {
    game.stage_info.unk8C.b3 = true;
    for (k, j) in joints.into_iter().enumerate() {
        let v = crate::lb::joint::world_position(&mut game.objs, data, j, None);
        let i = &mut game.stage_info;
        let slot = match k {
            0 => &mut i.x130,
            1 => &mut i.x13C,
            2 => &mut i.x148,
            3 => &mut i.x154,
            4 => &mut i.x160,
            _ => &mut i.x16C,
        };
        *slot = v;
    }
}

/// A point on a spline and the Euler angles that turn an object along it, `Ground_801C4B50`:
/// the spline's direction (normalized) is the object's x axis, the side axis its cross with up
/// (or with z where the direction is nearly vertical) and the up axis theirs, read back as
/// rotations about x, y and z. The point is `None` where `splGetSplinePoint` leaves it alone.
#[doc(alias = "Ground_801C4B50")]
pub fn spline_pose(data: &Archives, spline: DataRef, u: f32) -> (Option<Vec3>, Vec3) {
    use crate::math::{asin, cos, sin};
    let at = crate::hsd::spline::point(data, spline, u);
    let mut dir = crate::hsd::spline::tangent(data, spline, u)
        .expect("Ground_801C4B50: a spline parameter outside [0, 1] leaves its tangent unset");
    dir.normalize();
    let ay = if reg(dir.y) < 0.0 {
        fp::fneg(reg(dir.y))
    } else {
        reg(dir.y)
    };
    let up = if fp::frsp(ay) > fp::frsp(0.9) {
        Vec3::new(0.0, 0.0, 1.0)
    } else {
        Vec3::new(0.0, 1.0, 0.0)
    };
    let side = dir.cross_normalized(up);
    let up = side.cross_normalized(dir);
    let pi = std::f64::consts::PI;
    let ry = asin(side.z);
    let az = if reg(side.z) < 0.0 {
        fp::fneg(reg(side.z))
    } else {
        reg(side.z)
    };
    let (rx, rz) = if az >= f64::from(0.99999_f32) {
        let mut rx = asin(mem(fp::fneg(reg(dir.y))));
        if fp::fmuls(fp::fmuls(reg(dir.x), reg(cos(rx))), reg(sin(ry))) < 0.0 {
            rx = mem(fp::frsp(fp::fsub(pi, reg(rx))));
        }
        (rx, 0.0)
    } else {
        let mut rx = asin(mem(fp::fdivs(reg(up.z), reg(cos(ry)))));
        if fp::fmuls(fp::fmuls(reg(dir.z), reg(cos(rx))), reg(cos(ry))) < 0.0 {
            rx = mem(fp::frsp(fp::fsub(pi, reg(rx))));
        }
        let mut rz = asin(mem(fp::fdivs(fp::fneg(reg(side.y)), reg(cos(ry)))));
        if fp::fmuls(fp::fmuls(fp::fneg(reg(side.x)), reg(cos(ry))), reg(cos(rz))) < 0.0 {
            rz = mem(fp::frsp(fp::fsub(pi, reg(rz))));
        }
        (rx, rz)
    };
    (at, Vec3::new(rx, ry, rz))
}

/// Sets `x6D8`, `Ground_801C5740`.
#[doc(alias = "Ground_801C5740")]
pub fn set_x6d8(game: &mut Game, v: i32) {
    game.stage_info.x6D8 = v;
}

/// Clears `x6D8`, `Ground_801C5750`.
#[doc(alias = "Ground_801C5750")]
pub fn clear_x6d8(game: &mut Game) {
    game.stage_info.x6D8 = 0;
}

/// `x6D8`, `Ground_801C5764`.
#[doc(alias = "Ground_801C5764")]
pub fn x6d8(game: &Game) -> i32 {
    game.stage_info.x6D8
}

/// `x6DC`, `Ground_801C5774`.
#[doc(alias = "Ground_801C5774")]
pub fn x6dc(game: &Game) -> i32 {
    i32::from(game.stage_info.x6DC)
}

/// Sets Onett's car warning (`x740`), which the CPU reads, `Ground_801C5784`.
#[doc(alias = "Ground_801C5784")]
pub fn set_x740(game: &mut Game, v: i32) {
    game.stage_info.x740 = v;
}

/// Onett's car warning, `Ground_801C5794`.
#[doc(alias = "Ground_801C5794")]
pub fn x740(game: &Game) -> i32 {
    game.stage_info.x740
}

/// Player 1's fighter, `Ground_GetP1Fighter`.
#[doc(alias = "Ground_GetP1Fighter")]
pub fn p1_fighter(game: &Game) -> Option<GObjId> {
    game.player_entity(0)
}

/// Player 1's second fighter, `Ground_GetP1Fighter2`.
#[doc(alias = "Ground_GetP1Fighter2")]
pub fn p1_fighter2(game: &Game) -> Option<GObjId> {
    game.player_entity_at(0, 1)
}

/// `x6E0`, `Ground_801C57F0` (its argument unused).
#[doc(alias = "Ground_801C57F0")]
pub fn x6e0(game: &Game, _arg: i32) -> f32 {
    game.stage_info.x6E0
}

/// One of `x6E4`'s two entries drawn at random, `Ground_801C5840`: `HSD_Randi(1)`, so always
/// the first, the draw still taken.
#[doc(alias = "Ground_801C5840")]
pub fn x6e4_drawn(game: &mut Game) -> i32 {
    let i = game.objs.rng.below(1);
    game.stage_info.x6E4[i as usize]
}

/// Sets `x98`, `Ground_801C5A84`.
#[doc(alias = "Ground_801C5A84")]
pub fn set_x98(game: &mut Game, v: i32) {
    game.stage_info.x98 = v;
}

/// `x98`, `Ground_801C5A94`.
#[doc(alias = "Ground_801C5A94")]
pub fn x98(game: &Game) -> i32 {
    game.stage_info.x98
}

/// Sets `unk8C.b1`, `Ground_801C5AA4`.
#[doc(alias = "Ground_801C5AA4")]
pub fn set_b1(game: &mut Game, v: bool) {
    game.stage_info.unk8C.b1 = v;
}

/// `unk8C.b1`, `Ground_801C5ABC`.
#[doc(alias = "Ground_801C5ABC")]
pub fn b1(game: &Game) -> bool {
    game.stage_info.unk8C.b1
}

/// A stage kind's `flags2`, `Ground_801C5AD0`.
#[doc(alias = "Ground_801C5AD0")]
pub fn flags2(grkind: i32) -> u32 {
    stage_data(grkind).flags2
}

/// Euler angles from three axes, any not under 30000 in size (NaN too) zeroed,
/// `Ground_801C5AEC`.
#[doc(alias = "Ground_801C5AEC")]
pub fn euler_from_axes(a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
    let mut v = Vec3::euler_angles_from_basis(a, b, c);
    let small = |x: f32| {
        let x = reg(x);
        let abs = if x < 0.0 { fp::fneg(x) } else { x };
        abs < 30000.0
    };
    if !small(v.x) {
        v.x = 0.0;
    }
    if !small(v.y) {
        v.y = 0.0;
    }
    if !small(v.z) {
        v.z = 0.0;
    }
    v
}

/// A stage object's sound ids that mean no sound (`540000`) and stop the slot's (`540001`).
pub const SOUND_NONE: u32 = 540_000;
pub const SOUND_STOP: u32 = 540_001;

// A stage object's sounds: eight slots of handles (`Ground::x20`, -1 when empty).
impl Running<'_> {
    /// A stage sound at full volume, centered, on a track, `Ground_801C5414`.
    #[doc(alias = "Ground_801C5414")]
    pub fn stage_sound_on_track(&mut self, id: i32, track: i32) {
        self.sfx_play_track(id, 0x7F, 0x40, track);
    }

    /// Plays a sound in one of a stage object's slots, `Ground_801C5440`, stopping the
    /// slot's last; `SOUND_STOP` only stops it and `SOUND_NONE` does nothing.
    #[doc(alias = "Ground_801C5440")]
    pub fn object_sound(&mut self, g: Option<GroundId>, i: i32, id: u32) {
        if !(0..8).contains(&i) {
            return;
        }
        let Some(g) = g else { return };
        if id == SOUND_NONE {
            return;
        }
        if id != SOUND_STOP {
            let h = self.game.grounds[g.0].x20[i as usize];
            if h != -1 {
                self.sfx_stop(h);
            }
            let h = self.sfx_play(id as i32, 0x7F, 0x40);
            self.game.grounds[g.0].x20[i as usize] = h;
        } else {
            self.stop_object_sound(Some(g), i);
        }
    }

    /// Whether a stage object's slot's sound still plays, `Ground_801C54DC`.
    #[doc(alias = "Ground_801C54DC")]
    pub fn object_sound_playing(&self, g: Option<GroundId>, i: i32) -> bool {
        if !(0..8).contains(&i) {
            return false;
        }
        let Some(g) = g else { return false };
        let h = self.game.grounds[g.0].x20[i as usize];
        h != -1 && self.sfx_playing(h)
    }

    /// Stops a stage object's slot's sound, `Ground_801C5544`.
    #[doc(alias = "Ground_801C5544")]
    pub fn stop_object_sound(&mut self, g: Option<GroundId>, i: i32) {
        if !(0..8).contains(&i) {
            return;
        }
        let Some(g) = g else { return };
        let h = self.game.grounds[g.0].x20[i as usize];
        if h != -1 {
            self.sfx_stop(h);
        }
        self.game.grounds[g.0].x20[i as usize] = -1;
    }

    /// Stops every one of a stage object's sounds, `Ground_801C55AC`.
    #[doc(alias = "Ground_801C55AC")]
    pub fn stop_object_sounds(&mut self, g: Option<GroundId>) {
        if g.is_some() {
            for i in 0..8 {
                self.stop_object_sound(g, i);
            }
        }
    }

    /// Sets a stage object's slot's sound's volume (0 to 1), `Ground_801C5630`.
    #[doc(alias = "Ground_801C5630")]
    pub fn object_sound_volume(&mut self, g: Option<GroundId>, i: i32, v: f32) {
        if !(0..8).contains(&i) {
            return;
        }
        let Some(g) = g else { return };
        let h = self.game.grounds[g.0].x20[i as usize];
        if h != -1 {
            let vol = fp::fctiwz(fp::fmuls(fp::frsp(127.0), reg(v)));
            self.sfx_volume(h, vol);
        }
    }

    /// Sets a stage object's slot's sound's pan (-1 to 1), `Ground_801C5694`.
    #[doc(alias = "Ground_801C5694")]
    pub fn object_sound_pan(&mut self, g: Option<GroundId>, i: i32, v: f32) {
        if !(0..8).contains(&i) {
            return;
        }
        let Some(g) = g else { return };
        let h = self.game.grounds[g.0].x20[i as usize];
        if h != -1 {
            let pan = fp::fctiwz(fp::fmuls(63.5, fp::fadds(fp::frsp(1.0), reg(v))));
            self.sfx_pan(h, pan);
        }
    }
}
