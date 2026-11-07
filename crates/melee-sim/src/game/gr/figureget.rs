//! Snag the Trophies, `melee/gr/grfigureget.c` (ground kind 38, `/GrNFg.dat`): Classic's
//! fifth stage. An animated backdrop (map 0) and the stand (map 1, with the stage's lights and
//! fog), over which three trophy coins (`it::kinds::trophycoin`'s kinds 1 to 3, `it_802F2094`)
//! fall one at a time from above the camera, each at a random place left or right of the
//! middle. A coin that lands on the stand's collision joint (joint 0's callback) within 4.5 of
//! its middle is snagged: it's knocked off its stand (`it_802F202C`) and its trophy kept. Once
//! the three are out and each is snagged or gone, the stand counts the snagged trophies as the
//! match's (`gm_8017E280`) and ends the stage: flags 0x100 with one or more (0x20 with all
//! three), else 0x200.

use gekko_fp as fp;
use melee_data::archive::DataRef;

use super::super::state::{Game, Running};
use super::data::{FIGUREGET_CALLBACKS, StageCallbacks};
use super::ground::{ground_of, yakumono_param};
use super::{CollUser, anime, setup, stage, zako};
use crate::fpu::{mem, reg};
use crate::game::handles::{CodeRef, GroundId};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// `HSD_GOBJ_PLINK_FIGHTER`.
const P_LINK_FIGHTER: u8 = 8;

/// `grFigureGet_80219B10`, the stand's collision joint's callback (`mpJointSetCb1`).
pub const COIN_TOUCHED: u32 = 0x8021_9B10;
/// `grFigureGet_OnTouchLine`, the stage's line-touch dynamics.
pub const ON_TOUCH_LINE: u32 = 0x8021_9C88;
/// `grFigureGet_OnCheckShadowRender`, the stage's shadow check.
pub const CHECK_SHADOW_RENDER: u32 = 0x8021_9C90;
/// `grFigureGet_OnDemoInit`.
pub const ON_DEMO_INIT: u32 = 0x8021_9530;

/// A stage object's process or callback, by address: Snag the Trophies'.
pub fn run(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        0x8021_96E8 | 0x8021_96EC | 0x8021_9B0C => empty(r, gobj),
        0x8021_96B4 => init_backdrop(r, gobj),
        0x8021_96F0 => init_stand(r, gobj),
        0x8021_9898 => proc_stand(r, gobj),
        _ => return false,
    }
    true
}

/// The stage's hooks without arguments, by address.
pub fn run_event(r: &mut Running, f: u32) -> bool {
    match f {
        0x8021_9534 => on_init(r),
        0x8021_959C => on_load(r),
        0x8021_95A0 => on_start(r),
        _ => return false,
    }
    true
}

/// The backdrop's process and the objects' destroy callbacks, which do nothing.
// Ports grFigureGet_802196E8; aliased once a check reaches it.
// Ports grFigureGet_802196EC; aliased once a check reaches it.
// Ports grFigureGet_80219B0C; aliased once a check reaches it.
pub fn empty(_r: &mut Running, _gobj: GObjId) {}

/// The objects' predicates (`callback1`), which nothing calls and answer false.
// Ports grFigureGet_802196E0; aliased once a check reaches it.
// Ports grFigureGet_80219890; aliased once a check reaches it.
pub fn answers_false(_gobj: GObjId) -> bool {
    false
}

/// The stage's `callback4`, `grFigureGet_802195C4`: false.
// Ports grFigureGet_802195C4; aliased once a check reaches it.
pub fn callback4() -> bool {
    false
}

/// The stage's demo setup does nothing (only a cutscene's `Stage_8022532C` runs one).
// Ports grFigureGet_OnDemoInit; aliased once a check reaches it.
pub fn on_demo_init(_r: &mut Running) {}

/// The stage's line-touch dynamics: none.
// Ports grFigureGet_OnTouchLine; aliased once a check reaches it.
pub fn on_touch_line(_line: i32) -> Option<DataRef> {
    None
}

/// The stage's shadow check: every shadow is drawn.
// Ports grFigureGet_OnCheckShadowRender; aliased once a check reaches it.
pub fn check_shadow_render() -> bool {
    true
}

/// The stage's objects' setup functions and processes, by map id
/// (`grFigureGet_StageCallbacks`).
fn callbacks(map_id: i32) -> &'static StageCallbacks {
    &FIGUREGET_CALLBACKS[map_id as usize]
}

/// Creates one of the stage's objects and sets it up, `grFigureGet_802195CC`, its scale
/// joint's descriptor at `frame` on the stack.
#[doc(alias = "grFigureGet_802195CC")]
pub fn create(r: &mut Running, map_id: i32, frame: u32) -> Option<GObjId> {
    setup::create_from_callbacks(r, map_id, callbacks(map_id), frame)
}

fn param(game: &Game) -> DataRef {
    game.gr
        .figureget_yakumono
        .expect("grfigureget.c: yakumono_param")
}

/// The stage's setup, `grFigureGet_OnInit`: its parameters, the stage's `unk8C` flags (b4
/// off, b5 on), maps 0 and 1, then the camera limits and blast zones measured.
#[doc(alias = "grFigureGet_OnInit")]
pub fn on_init(r: &mut Running) {
    r.game.gr.figureget_yakumono = yakumono_param(r.game);
    r.game.stage_info.unk8C.b4 = false;
    r.game.stage_info.unk8C.b5 = true;
    create(r, 0, setup::SETUP_FRAME);
    create(r, 1, setup::SETUP_FRAME);
    setup::measure_cam_bounds(r.game, r.data);
    setup::measure_blast_zones(r.game, r.data);
}

/// The stage's load does nothing, `grFigureGet_OnLoad`.
#[doc(alias = "grFigureGet_OnLoad")]
pub fn on_load(_r: &mut Running) {}

/// The stage's start, `grFigureGet_OnStart`: the enemy spawner, with no table.
#[doc(alias = "grFigureGet_OnStart")]
pub fn on_start(r: &mut Running) {
    zako::start(r.game, None);
}

fn map_id(game: &Game, gobj: GObjId) -> i32 {
    game.grounds[ground_of(game, gobj).0].map_id
}

/// The backdrop's setup (map 0), `stageGObj0_OnInit`: its animation.
#[doc(alias = "stageGObj0_OnInit@802196B4")]
pub fn init_backdrop(r: &mut Running, gobj: GObjId) {
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
}

/// The parameters' ticks between coins: `x0` and up to `x4` more at random (no draw for 0).
fn next_wait(r: &mut Running) -> i32 {
    let p = param(r.game);
    let n = r.data.u32(p.at(4)) as i32;
    let extra = if n != 0 { r.game.objs.rng.below(n) } else { 0 };
    (r.data.u32(p) as i32).wrapping_add(extra)
}

/// The stand's setup (map 1), `grFigureGet_802196F0`: its animation, no callbacks, the
/// trophies' bookkeeping (`tyDisplay_8031C2CC`), and its three coins' trophies, from a slot
/// drawn at random on: a trophy of any class (65% a new one, `Toy_80305058(0, 0x63, 0, 65)`),
/// the stage's display trophy (`Ground_801C5840`), and one of the display trophy's kin of class
/// 2 (`tyDisplay_8031C354`), or with none a marked trophy of class 2 (75% a new one) other than
/// the display trophy; each loaded (`tyDisplay_8031C454`). Then the wait to the first coin, the counts
/// and lists cleared, and joint 0's callback.
#[doc(alias = "grFigureGet_802196F0")]
#[doc(alias = "tyDisplay_8031C354")]
pub fn init_stand(r: &mut Running, gobj: GObjId) {
    let m = map_id(r.game, gobj);
    let g = ground_of(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    let gp = &mut r.game.grounds[g.0];
    gp.x8_callback = None;
    gp.xC_callback = None;
    r.stage_trophies();
    let i0 = r.game.objs.rng.below(3) as usize;
    let t = r.toy_draw(0, 0x63, 0, 65.0);
    r.game.grounds[g.0].u.figureget().x10[i0] = t;
    let i1 = if i0 + 1 >= 3 { 0 } else { i0 + 1 };
    let shown = super::ground::x6e4_drawn(r.game);
    r.game.grounds[g.0].u.figureget().x10[i1] = shown;
    let mut kin = [0; 1];
    let third = if r.display_kin(shown, &mut kin, 1, 2) == 0 {
        loop {
            let t = r.toy_draw(2, 0x63, 1, 75.0);
            if t != shown {
                break t;
            }
        }
    } else {
        kin[0]
    };
    let i2 = if i1 + 1 >= 3 { 0 } else { i1 + 1 };
    r.game.grounds[g.0].u.figureget().x10[i2] = third;
    for i in 0..3 {
        let id = r.game.grounds[g.0].u.figureget().x10[i];
        r.stage_trophy_load(id);
    }
    let wait = next_wait(r);
    let v = r.game.grounds[g.0].u.figureget();
    v.x0 = wait;
    v.x4 = 0;
    v.x8 = 0;
    v.xC = 0;
    v.x1C = [0; 3];
    v.x28 = [None; 3];
    v.x34 = [None; 3];
    r.game.geometry_mut().set_ledge_callback(
        0,
        Some(CollUser::Ground(g)),
        Some(CodeRef(COIN_TOUCHED)),
    );
}

/// The stand's process (map 1), `grFigureGet_80219898`: every fighter's collision joint skip
/// cleared; while coins are left to drop, at the wait's end a coin of the next trophy (kind
/// 1, 2, 3 in turn) at a random place 20 to half the camera's width less 20 from the middle,
/// left or right, at the parameters' `x14` above the camera's top, colliding with joint 0
/// alone, and the next wait; once all are out and each is snagged or gone
/// (`grFigureGet_80219C50`), the snagged trophies counted as the match's and the stage's
/// end flags. Then the wind sources age.
#[doc(alias = "grFigureGet_80219898")]
pub fn proc_stand(r: &mut Running, gobj: GObjId) {
    let mut at = r.game.world.first_in(P_LINK_FIGHTER);
    while let Some(f) = at {
        let id = r.game.fighter_id(f);
        r.game.fighters[id.0].coll_data.joint_id_skip = 0;
        at = r.game.world.gobjs[f.0].next;
    }
    let g = ground_of(r.game, gobj);
    let p = param(r.game);
    let total = r.data.u32(p.at(8)) as i32;
    let v = r.game.grounds[g.0].u.figureget();
    if v.x4 < total {
        if v.x0 == 0 {
            v.x4 += 1;
            let x4 = v.x4;
            let (xc, x10, x14) = (
                r.data.f32(p.at(0xC)),
                r.data.f32(p.at(0x10)),
                r.data.f32(p.at(0x14)),
            );
            let mut w = fp::fsubs(reg(x10), reg(xc));
            if w < 0.0 {
                w = fp::fneg(w);
            }
            let half = fp::fsubs(fp::fmuls(w, 0.5), 20.0);
            let n = fp::fctiwz(half);
            let off = if n != 0 { r.game.objs.rng.below(n) } else { 0 };
            let mut x = mem(fp::fadds(20.0, fp::frsp(f64::from(off))));
            if r.game.objs.rng.below(2) == 0 {
                x = mem(fp::fmuls(reg(x), fp::fneg(1.0)));
            }
            let y = mem(fp::fadds(reg(x14), reg(stage::cam_top(r.game))));
            let trophy = r.game.grounds[g.0].u.figureget().x10[(x4 - 1) as usize];
            let coin = super::super::it::kinds::trophycoin::spawn(
                r,
                Some(gobj),
                Vec3::new(x, y, 0.0),
                trophy,
                x4,
            );
            r.game.grounds[g.0].u.figureget().x34[(x4 - 1) as usize] = coin;
            if let Some(c) = coin {
                super::super::it::kinds::trophycoin::set_joint_only(r, c, 0);
            } else {
                super::super::code::unported_call(
                    "grFigureGet_80219898 with no coin (it_802F2014 on a null item)",
                );
            }
            let wait = next_wait(r);
            r.game.grounds[g.0].u.figureget().x0 = wait;
        } else {
            v.x0 -= 1;
        }
    } else if done(r.game, Some(gobj)) {
        let v = r.game.grounds[g.0].u.figureget();
        let (n, kept) = (v.x8, v.x1C);
        for &t in kept.iter().take(n.clamp(0, 3) as usize) {
            r.count_match_trophy(t as u16, 1);
        }
        if n != 0 {
            if n == 3 {
                r.game.stage_info.flags |= 0x20;
            }
            r.game.stage_info.flags |= 0x100;
        } else {
            r.game.stage_info.flags |= 0x200;
        }
    }
    super::ground::quake_list_tick(r.game);
}

/// A coin gone (its destroy callback), `grFigureGet_80219C34`: one more gone.
#[doc(alias = "grFigureGet_80219C34")]
pub fn coin_gone(game: &mut Game, stand: Option<GObjId>) {
    let Some(gobj) = stand else { return };
    let g = ground_of(game, gobj);
    game.grounds[g.0].u.figureget().xC += 1;
}

/// Whether every coin is snagged or gone, `grFigureGet_80219C50`: their counts' sum at least
/// 3; false without a stand.
#[doc(alias = "grFigureGet_80219C50")]
pub fn done(game: &mut Game, stand: Option<GObjId>) -> bool {
    let Some(gobj) = stand else { return false };
    let g = ground_of(game, gobj);
    let v = game.grounds[g.0].u.figureget();
    v.x8.wrapping_add(v.xC) >= 3
}

/// The coins' touches of the stand's collision joint in a collision pass, applied after it in
/// order ([`CoinTouch::apply`]).
#[derive(Default)]
pub struct CoinTouch(Vec<(GroundId, GObjId, f32)>);

impl CoinTouch {
    /// An item's collision (`x34_flags.b1234` 5) at `x` touching the stand `g`'s joint.
    pub fn touched(
        &mut self,
        g: GroundId,
        coll_kind: u8,
        owner: Option<GObjId>,
        item: bool,
        x: f32,
    ) {
        if let (5, Some(o), true) = (coll_kind, owner, item) {
            self.0.push((g, o, x));
        }
    }

    /// The stand's collision joint's callback, `grFigureGet_80219B10`: a coin touching while
    /// fewer than three are snagged, not snagged already, within 4.5 of the middle, is knocked
    /// off its stand (`it_802F202C`) and snagged, its trophy kept.
    #[doc(alias = "grFigureGet_80219B10")]
    pub fn apply(self, r: &mut Running) {
        use super::super::it::kinds::{COIN, trophycoin};
        for (g, coin, x) in self.0 {
            if super::super::it::item(r.game, coin).kind != COIN
                || r.game.grounds[g.0].u.figureget().x8 >= 3
            {
                continue;
            }
            let value = trophycoin::trophy(r, coin);
            let v = r.game.grounds[g.0].u.figureget();
            let n = v.x8.clamp(0, 3) as usize;
            if v.x28[..n].contains(&Some(coin)) {
                continue;
            }
            if reg(x) <= 4.5 && reg(x) >= fp::fneg(4.5) {
                trophycoin::knocked_off(r, coin);
                let v = r.game.grounds[g.0].u.figureget();
                v.x28[n] = Some(coin);
                v.x1C[n] = value;
                v.x8 += 1;
            }
        }
    }
}
