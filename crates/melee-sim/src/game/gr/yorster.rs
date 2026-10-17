//! Yoshi's Island, `melee/gr/gryorster.c`: the stage (map 1) with its nine blocks, and the
//! background (map 0).
//!
//! Each block is a collision joint of the stage's and a hurtbox item (`it::kinds::yaku`) on its
//! model. Hit hard enough, or landed on fast enough, a block flips over after a moment, its
//! collision gone, and turns back once it has spun a while, unless something stands where it
//! would come back, when it tries again shortly after.

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;
use melee_data::archive::{Archives, DataRef};

use super::super::handles::{CodeRef, GroundId};
use super::super::state::{Game, Running};
use super::data::{StageCallbacks, YORSTER_CALLBACKS};
use super::ground::{ground_of, jobj_of, nth_joint, update_map_coll};
use super::{anime, grlib, setup, zako};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// `grYorster_802024F0`, the blocks' collision joints' callback (`mpJointSetCb2`).
pub const BLOCK_LANDED: u32 = 0x8020_24F0;
/// `grYorster_80202428`, a block's hurtbox item's callback for a hit it takes.
pub const BLOCK_HIT: u32 = 0x8020_2428;
/// `grYorster_80202160`, the stage's start hook.
const START_HOOK: u32 = 0x8020_2160;

/// The blocks' parts (`grYorster_802022A4`'s `joints`).
const BLOCK_PARTS: [i32; 9] = [0xA, 0xB, 0xC, 0xD, 0xE, 0xF, 0x10, 0x11, 0x12];

/// `YorsterParams`, by offset.
#[derive(Clone, Copy)]
struct Param(DataRef);

impl Param {
    fn f(self, data: &Archives, off: u32) -> f32 {
        data.f32(self.0.at(off))
    }
    fn i(self, data: &Archives, off: u32) -> i32 {
        data.u32(self.0.at(off)) as i32
    }
}

/// The fall speed a landing flips a block at, and what it subtracts.
const LAND_MIN: u32 = 0x0;
/// The landing speed past which a block flips at once.
const LAND_FULL: u32 = 0x4;
/// The damage a hit flips a block at, and what it subtracts.
const HIT_MIN: u32 = 0x8;
/// The damage past which a block flips at once.
const HIT_FULL: u32 = 0xC;
/// The hitlag a landing or a hit gives (`x10` and `x14`).
const LAND_HITLAG: u32 = 0x10;
const HIT_HITLAG: u32 = 0x14;
/// The ticks before a struck block flips.
const DELAY: u32 = 0x18;
/// The flip's longest start frame.
const END_FRAME: u32 = 0x1C;

fn param(game: &Game) -> Param {
    Param(
        game.gr
            .yorster_yakumono
            .expect("gryorster.c: grYt_804D6A20"),
    )
}

/// A stage object's process or callback, by address: Yoshi's Island's.
pub fn run(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        0x8020_2158 | 0x8020_215C | 0x8020_22A0 => empty(r, gobj),
        0x8020_2124 => init_back(r, gobj),
        0x8020_21AC => init_stage(r, gobj),
        0x8020_2254 => proc_stage(r, gobj),
        START_HOOK => start_blocks(r, gobj),
        _ => return false,
    }
    true
}

/// The stage's hooks without arguments, by address.
pub fn run_event(r: &mut Running, f: u32) -> bool {
    match f {
        0x8020_1FA4 => on_init(r),
        0x8020_1FA0 => on_demo_init(r),
        0x8020_200C => on_load(r),
        0x8020_2010 => on_start(r),
        _ => return false,
    }
    true
}

/// The objects' processes and destroy callbacks that do nothing.
#[doc(alias = "grYorster_80202158")]
#[doc(alias = "grYorster_8020215C")]
#[doc(alias = "grYorster_802022A0")]
pub fn empty(_r: &mut Running, _gobj: GObjId) {}

/// Creates one of the stage's objects and sets it up, `grYorster_8020203C`.
#[doc(alias = "grYorster_8020203C")]
pub fn create(r: &mut Running, map_id: i32, frame: u32) -> Option<GObjId> {
    setup::create_from_callbacks(r, map_id, callbacks(map_id), frame)
}

/// The stage's objects' setup functions and processes, by map id (`grYt_StageCallbacks`).
pub fn callbacks(map_id: i32) -> &'static StageCallbacks {
    &YORSTER_CALLBACKS[map_id as usize]
}

/// Yoshi's Island's setup, `grYorster_80201FA4`: its parameters, maps 0 and 1, camera limits
/// and blast zones.
#[doc(alias = "grYorster_80201FA4")]
pub fn on_init(r: &mut Running) {
    r.game.gr.yorster_yakumono = super::ground::yakumono_param(r.game);
    r.game.stage_info.unk8C.b4 = false;
    r.game.stage_info.unk8C.b5 = true;
    create(r, 0, setup::SETUP_FRAME);
    create(r, 1, setup::SETUP_FRAME);
    setup::measure_cam_bounds(r.game, r.data);
    setup::measure_blast_zones(r.game, r.data);
}

/// `grYorster_80201FA0` does nothing (only a demo runs it).
#[doc(alias = "grYorster_80201FA0")]
pub fn on_demo_init(_r: &mut Running) {}

/// `grYorster_8020200C` does nothing.
#[doc(alias = "grYorster_8020200C")]
pub fn on_load(_r: &mut Running) {}

/// Yoshi's Island's start, `grYorster_80202010`: the enemy spawner, without enemies.
#[doc(alias = "grYorster_80202010")]
pub fn on_start(r: &mut Running) {
    zako::start(r.game, None);
}

fn map_id(game: &Game, gobj: GObjId) -> i32 {
    game.grounds[ground_of(game, gobj).0].map_id
}

/// The background's setup (map 0), `stageGObj0_OnInit`: its animation.
#[doc(alias = "stageGObj0_OnInit@80202124")]
pub fn init_back(r: &mut Running, gobj: GObjId) {
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
}

/// The stage's setup (map 1), `grYorster_802021AC`: its collision, its animation 4, the
/// animation of parts 23 and 26, the blocks set up when the match starts, and the blocks
/// still waiting for that.
#[doc(alias = "grYorster_802021AC")]
pub fn init_stage(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    anime::start_map(r.game, r.data, gobj, m, 4);
    anime::start(r.game, r.data, gobj, 23, 7, 3, 0.0, 1.0, true);
    anime::start(r.game, r.data, gobj, 26, 7, 3, 0.0, 1.0, true);
    setup::add_start_hook(r.game, gobj, CodeRef(START_HOOK));
    r.game.grounds[g.0].u.yorster().xC4 = 1;
}

/// The stage's start hook, `grYorster_80202160`: the blocks, the collision moved to the
/// model, the joints' links made, and the blocks running.
#[doc(alias = "grYorster_80202160")]
pub fn start_blocks(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    init_blocks(r, gobj);
    update_map_coll(r.game, r.data, gobj);
    r.game.geometry_mut().link_all_joints();
    r.game.grounds[g.0].u.yorster().xC4 = 0;
}

/// The stage's process (map 1), `grYorster_80202254`: the blocks once running, then the wind
/// sources age and the collision follows the model.
#[doc(alias = "grYorster_80202254")]
pub fn proc_stage(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    if r.game.grounds[g.0].u.yorster().xC4 == 0 {
        proc_blocks(r, gobj);
    }
    let level = r.game.gr.wind_level;
    r.game.gr.wind_level = crate::lb::wind::tick(&mut r.game.gr.winds, level);
    update_map_coll(r.game, r.data, gobj);
}

/// The blocks' setup, `grYorster_802022A4`: each block at rest with its part, a hurtbox item
/// on it (on the ground, a capsule of radius 5 at the part, shown at once), its collision
/// joint telling it what lands on it, and its animation 1 held at its first frame.
#[doc(alias = "grYorster_802022A4")]
pub fn init_blocks(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    for (i, &part) in BLOCK_PARTS.iter().enumerate() {
        let joint = nth_joint(r.game, gobj, part);
        let b = &mut r.game.grounds[g.0].u.yorster().blocks[i];
        b.x00 = i as i8;
        b.x01 = 0;
        b.x04 = 0.0;
        b.x08 = 0.0;
        b.x14 = part;
        b.x18 = joint;
        let item =
            super::material::stage_item(r, 0, 0, g, joint, None, Some(CodeRef(BLOCK_HIT)), None);
        r.game.grounds[g.0].u.yorster().blocks[i].x1C = item;
        let item = item.expect("gryorster.c: a block's item");
        super::material::item_ground_or_air(r.game, item, 0);
        let coll = setup::coll_joint_id(r.game, r.data, m, part);
        r.game.geometry_mut().set_callback(
            coll,
            Some(super::CollUser::Ground(g)),
            Some(CodeRef(BLOCK_LANDED)),
        );
        super::material::set_item_hurtbox(r.game, item, Vec3::default(), Vec3::default(), 5.0);
        super::material::item_hurtbox_on(r.game, item);
        anime::start(r.game, r.data, gobj, part, 7, 1, 0.0, 0.0, true);
    }
}

/// A hit a block's item took, `grYorster_80202428`: the fighter that hit it gets the
/// parameters' hitlag, and the block counts the damage.
#[doc(alias = "grYorster_80202428")]
pub fn block_hit(
    r: &mut Running,
    item: GObjId,
    g: GroundId,
    _dir: Vec3,
    fighter: Option<GObjId>,
    damage: f32,
) {
    if let Some(f) = fighter
        && super::super::it::item::is_fighter(r.game, Some(f))
    {
        let lag = fp::frsp(f64::from(param(r.game).i(r.data, HIT_HITLAG))) as f32;
        r.game.ft_80086a4c(f, lag);
    }
    let blocks = &mut r.game.grounds[g.0].u.yorster().blocks;
    if let Some(b) = blocks.iter_mut().find(|b| b.x1C == Some(item)) {
        b.x04 = mem(fp::fadds(reg(b.x04), reg(damage)));
    }
}

/// What the blocks' collision callback reads of a fighter that lands, and the hitlag it gives
/// the object that landed. mpcoll runs the callback in the middle of the object's pass, with
/// the fighters and items out of reach, so a pass on Yoshi's Island first notes each fighter's
/// entrance (`ftLib_IsEntry`) and its position's change (`ftLib_GetPosDelta`), and the pass's
/// caller gives the hitlag (`ftLib_80086A4C` for a fighter, `it_8026B718` for an item) once
/// mpcoll returns; nothing in a pass reads either.
#[derive(Clone, Copy, Debug, Default)]
pub struct Landing {
    fighters: [(Option<GObjId>, bool, f32); 8],
    pub hitlag: Option<(Option<GObjId>, f32)>,
}

impl Landing {
    /// The fighters' entrances and moves, on Yoshi's Island (no other stage reads them).
    pub fn of(game: &Game) -> Landing {
        let mut l = Landing::default();
        if game.stage_info.grkind != super::GR_KIND_YORSTER {
            return l;
        }
        let mut cur = game.world.first_in(8);
        let mut i = 0;
        while let Some(g) = cur {
            let fp = game.fighter(g);
            let entering = (super::super::ft::co::ids::ms::ENTRY
                ..=super::super::ft::co::ids::ms::ENTRY_END)
                .contains(&fp.motion_id);
            if i < l.fighters.len() {
                l.fighters[i] = (Some(g), entering, fp.pos_delta.y);
                i += 1;
            }
            cur = game.world.gobj(g).next;
        }
        l
    }

    fn fighter(&self, g: Option<GObjId>) -> (bool, f32) {
        self.fighters
            .iter()
            .find(|f| f.0.is_some() && f.0 == g)
            .map_or((false, 0.0), |f| (f.1, f.2))
    }

    /// Gives the hitlag a landing asked for to the object that landed.
    pub fn apply(self, game: &mut Game) {
        let Some((Some(g), lag)) = self.hitlag else {
            return;
        };
        if super::super::it::item::is_fighter(game, Some(g)) {
            game.ft_80086a4c(g, lag);
        } else {
            super::super::it::lib26b1::set_hitlag(game, g, lag);
        }
    }
}

/// The blocks' collision callback, `grYorster_802024F0`: a fighter (not in its entrance), an
/// item or a fighter's item coming down at the parameters' speed or faster gets their hitlag,
/// and the block counts how fast it came down (a fighter's position's change, otherwise the
/// height the pass moved).
#[doc(alias = "grYorster_802024F0")]
#[allow(clippy::too_many_arguments)]
pub fn block_landed(
    gp: &mut super::super::structs::Ground,
    yaku: Option<DataRef>,
    data: &Archives,
    part_joint: &dyn Fn(i32, i32) -> i32,
    joint_id: i32,
    coll: &crate::game::structs::CollData,
    landing: &mut Landing,
    delta_y: f32,
) {
    let env = i32::from(coll.x34_flags.b1234);
    if env != 1 && env != 2 && env != 3 {
        return;
    }
    let (entering, moved) = landing.fighter(coll.x0_gobj);
    if env == 1 && entering {
        return;
    }
    let p = Param(yaku.expect("gryorster.c: grYt_804D6A20"));
    let dy = if env == 1 { moved } else { delta_y };
    if reg(dy) >= reg(p.f(data, LAND_MIN)) {
        let lag = fp::frsp(f64::from(p.i(data, LAND_HITLAG))) as f32;
        landing.hitlag = Some((coll.x0_gobj, lag));
    }
    let map = gp.map_id;
    let blocks = &mut gp.u.yorster().blocks;
    for b in blocks.iter_mut() {
        if joint_id == part_joint(map, b.x14) {
            b.x08 = mem(fp::fadds(reg(b.x08), reg(dy)));
            break;
        }
    }
}

/// [`block_landed`] as a pass would run it on the stage's state, with the hitlag given at once
/// (for the checks, which run the callback on its own).
pub fn block_landed_now(
    r: &mut Running,
    g: GroundId,
    joint_id: i32,
    coll: &crate::game::structs::CollData,
    delta_y: f32,
) {
    let mut landing = Landing::of(r.game);
    let data = r.data;
    let Game {
        grounds,
        gr,
        stage_info,
        ..
    } = &mut *r.game;
    let grkind = stage_info.grkind;
    let part_joint = |m: i32, p: i32| setup::coll_joint_id_in(gr, grkind, data, m, p);
    block_landed(
        &mut grounds[g.0],
        gr.yorster_yakumono,
        data,
        &part_joint,
        joint_id,
        coll,
        &mut landing,
        delta_y,
    );
    landing.apply(r.game);
}

/// The blocks' process, `grYorster_8020266C`. A block at rest (0, or 1 once it has turned
/// back) flips when it has taken the parameters' damage (keeping the rest, its flip starting
/// later in its animation the more over it was) or been landed on that fast (likewise, with
/// a sound): its item blinks and it waits its delay (2). Then it spins from that frame, its
/// collision gone (3); after 323 ticks it turns back, its collision and item back, unless
/// something is near where it would be, when it spins slowly (4) and tries again once its
/// animation rewinds or 25 ticks have passed. Each tick a landing's speed is forgotten.
#[doc(alias = "grYorster_8020266C")]
pub fn proc_blocks(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    let p = param(r.game);
    let data = r.data;
    for i in 0..9 {
        let b = r.game.grounds[g.0].u.yorster().blocks[i].clone();
        let joint = setup::coll_joint_id(r.game, r.data, m, b.x14);
        let item = b.x1C.expect("gryorster.c: a block's item");
        match b.x01 as i8 {
            0 | 1 => {
                let (hit_min, land_min) = (p.f(data, HIT_MIN), p.f(data, LAND_MIN));
                let mut flip = None;
                if reg(b.x04) >= reg(hit_min) {
                    super::material::item_hurtbox_off(r.game, item);
                    let full = p.f(data, HIT_FULL);
                    let x04 = mem(fp::fsubs(reg(b.x04), reg(hit_min)));
                    let left = if reg(x04) >= reg(full) {
                        0
                    } else {
                        fp::fctiwz(fp::fsubs(reg(full), reg(x04)))
                    };
                    let end = flip_end(r, p, b.x18);
                    let x10 = fp::fctiwz(fp::fmuls(
                        fp::frsp(f64::from(left)),
                        fp::fdivs(reg(end), reg(full)),
                    ));
                    r.game.grounds[g.0].u.yorster().blocks[i].x04 = x04;
                    flip = Some(x10);
                } else if reg(b.x08) >= reg(land_min) {
                    super::material::item_hurtbox_off(r.game, item);
                    let full = p.f(data, LAND_FULL);
                    let mut x08 = mem(fp::fsubs(reg(b.x08), reg(land_min)));
                    if reg(x08) <= reg(land_min) {
                        x08 = land_min;
                    }
                    let left = if reg(x08) >= reg(full) {
                        0
                    } else {
                        fp::fctiwz(fp::fmuls(10.0, fp::fsubs(reg(full), reg(x08))))
                    };
                    let end = flip_end(r, p, b.x18);
                    let x10 = fp::fctiwz(fp::fmuls(
                        fp::frsp(f64::from(left)),
                        fp::fdivs(reg(end), fp::fmuls(10.0, reg(full))),
                    ));
                    r.game.grounds[g.0].u.yorster().blocks[i].x08 = 0.0;
                    r.stage_sound(0xE);
                    flip = Some(x10);
                }
                if let Some(x10) = flip {
                    let delay = p.i(data, DELAY);
                    let v = &mut r.game.grounds[g.0].u.yorster().blocks[i];
                    v.x10 = x10;
                    v.x0C = delay;
                    v.x01 = 2;
                }
            }
            2 => {
                if b.x0C <= 0 {
                    let from = fp::frsp(f64::from(b.x10)) as f32;
                    anime::start(r.game, r.data, gobj, b.x14, 7, 0, from, 1.0, true);
                    r.game.geometry_mut().disable_joint(joint);
                    let v = &mut r.game.grounds[g.0].u.yorster().blocks[i];
                    v.x01 = 3;
                    v.x0C = b.x10;
                } else {
                    r.game.grounds[g.0].u.yorster().blocks[i].x0C = b.x0C - 1;
                }
            }
            3 => {
                if b.x0C >= 0x143 {
                    let pos = translation(r.game, b.x18);
                    let scale = reg(super::ground::scale(r.game));
                    let near = mem(fp::fsubs(fp::fmuls(10.0, scale), 2.0));
                    if grlib::point_near_object(r.game, pos, near) {
                        anime::start(r.game, r.data, gobj, b.x14, 7, 2, 0.0, 0.3, true);
                        let v = &mut r.game.grounds[g.0].u.yorster().blocks[i];
                        v.x0C = 0;
                        v.x01 = 4;
                    } else {
                        turn_back(r, gobj, g, i, b.x14, joint, item);
                    }
                } else {
                    r.game.grounds[g.0].u.yorster().blocks[i].x0C = b.x0C + 1;
                }
            }
            4 => {
                if anime::rewound(r.game, gobj, b.x14, 7) || b.x0C >= 0x19 {
                    r.game.grounds[g.0].u.yorster().blocks[i].x0C = 0;
                    let pos = translation(r.game, b.x18);
                    let scale = reg(super::ground::scale(r.game));
                    let near = mem(fp::fdivs(fp::fmuls(10.0, scale), 2.0));
                    if !grlib::point_near_object(r.game, pos, near) {
                        turn_back(r, gobj, g, i, b.x14, joint, item);
                    }
                } else {
                    r.game.grounds[g.0].u.yorster().blocks[i].x0C = b.x0C + 1;
                }
            }
            _ => {}
        }
        r.game.grounds[g.0].u.yorster().blocks[i].x08 = 0.0;
    }
}

/// The flip's start frame's top: the parameters' end frame, or the block's animation's end if
/// that comes first.
fn flip_end(r: &Running, p: Param, model: Option<crate::hsd::jobj::JObjId>) -> f32 {
    let mut end = fp::frsp(f64::from(p.i(r.data, END_FRAME))) as f32;
    let last = crate::lb::anim::end_frame(&r.game.objs, model);
    if reg(end) >= reg(last) {
        end = last;
    }
    end
}

/// A block turns back: its animation 1, its collision back on, its item's hurtbox back, and at
/// rest with nothing counted (`*** End Frame = %d` reported, at state 3).
fn turn_back(
    r: &mut Running,
    gobj: GObjId,
    g: GroundId,
    i: usize,
    part: i32,
    joint: i32,
    item: GObjId,
) {
    anime::start(r.game, r.data, gobj, part, 7, 1, 0.0, 1.0, true);
    r.game.geometry_mut().enable_joint(joint);
    super::material::item_hurtbox_on(r.game, item);
    let v = &mut r.game.grounds[g.0].u.yorster().blocks[i];
    v.x04 = 0.0;
    v.x08 = 0.0;
    v.x01 = 1;
}

/// A joint's translation (`HSD_JObjGetTranslation`).
fn translation(game: &Game, j: Option<crate::hsd::jobj::JObjId>) -> Vec3 {
    game.objs.jobjs[j.expect("gryorster.c: a block's part")].translate
}

/// The stage's shadow check, `grYorster_80202B64`: every joint takes the shadows.
#[doc(alias = "grYorster_80202B64")]
pub fn check_shadow_render() -> bool {
    true
}
