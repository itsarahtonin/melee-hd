//! Fighters' environment collision (`ft_081B.c`, `ft_0819.c`): each action state's collision
//! callback runs mpcoll on the fighter's `CollData` from its position, keeps the result, and
//! changes state when the fighter leaves the ground, lands, meets a wall or catches a ledge.

use gekko_fp as fp;

use super::super::code::unported_call;
use super::super::gr::{DynamicAttrs, Geometry, StageColl};
use super::super::handles::{CodeRef, FighterId};
use super::super::state::Running;
use super::super::structs::{CollData, ftCollisionBox};
use super::common::GA_AIR;
use crate::fpu::{mem, reg};
use crate::hsd::Objects;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::JObjId;
use crate::math;
use crate::mp::LINE_PLATFORM;
use crate::mp::coll::{
    CEILING_HUG, Coll, CollHooks, ECB_SOURCE_JOBJ, FLOOR_HUG, FLOOR_PUSH, LEFT_LEDGE_GRAB,
    LEFT_LEDGE_SLIP, LEFT_WALL_HUG, RIGHT_LEDGE_GRAB, RIGHT_LEDGE_SLIP, RIGHT_WALL_HUG,
};
use crate::vec::Vec3;
use melee_data::archive::Archives;

/// `CollData::env_flags` bit 23: the fighter is at a ledge it could teeter on.
const AT_LEDGE: i32 = 0x80_0000;

/// `Collide_FloorMask`.
const FLOOR_MASK: i32 = FLOOR_PUSH | FLOOR_HUG;

/// `Collide_LedgeGrabMask`.
const LEDGE_GRAB_MASK: i32 = LEFT_LEDGE_GRAB | RIGHT_LEDGE_GRAB;

/// The item P-link (`HSD_GOBJ_PLINK_ITEM`).
const ITEM_P_LINK: u8 = 9;

/// `ftCo_80096CC8`: the floor filter that lets a fighter holding down drop through platforms.
pub const PLATFORM_FILTER: CodeRef = CodeRef(0x8009_6CC8);

/// `fn_800DD6E4`: the floor filter of a throw in the air, which lands on any floor but a
/// platform.
pub const THROW_FILTER: CodeRef = CodeRef(0x800D_D6E4);

/// A callback the original passes a collision function: what happens on landing.
pub type Cb = fn(&mut Running<'_>, GObjId);

/// What a fighter's floor filter reads: its stick, `ftCommonData` x25C, and which lines are
/// platforms.
pub struct FilterInput<'a> {
    pub stick_y: f32,
    pub threshold: f32,
    pub platforms: &'a [bool],
}

/// What mpcoll asks of the game during an object's pass (a fighter's or an item's).
pub struct ObjCollHooks<'a> {
    pub objs: &'a mut Objects,
    pub data: &'a Archives,
    /// What a landing on Yoshi's Island's blocks reads and gives.
    pub landing: crate::game::gr::yorster::Landing,
    /// The planks Kongo Jungle's bridge's landings kicked.
    pub kicks: crate::game::gr::kongo::Kicks,
    /// The stage objects, which the stage's collision joints' callbacks write.
    pub grounds: &'a mut crate::hsd::pool::Pool<super::super::structs::Ground>,
    /// The stage objects by map id (`Ground_GetMapGObj`), for callbacks that write another
    /// object than their own.
    pub stage_maps: StageMaps<'a>,
    /// The fighter's floor filter's inputs, for a filtered pass.
    pub filter: Option<FilterInput<'a>>,
}

/// The stage objects by map id, as a collision joint's callback reaches them
/// (`Ground_GetMapGObj`).
#[derive(Clone, Copy)]
pub struct StageMaps<'a> {
    world: &'a crate::hsd::gobj::World<super::super::state::GameKinds>,
    info: &'a super::super::structs::StageInfo,
    /// The stage code's globals (the dynamic attribute areas, the stage archives, the stages'
    /// parameters), which the collision joints' callbacks read.
    pub gr: &'a super::super::gr::Globals,
}

impl<'a> StageMaps<'a> {
    pub fn of_parts(
        world: &'a crate::hsd::gobj::World<super::super::state::GameKinds>,
        info: &'a super::super::structs::StageInfo,
        gr: &'a super::super::gr::Globals,
    ) -> Self {
        StageMaps { world, info, gr }
    }

    /// The stage's kind (`stage_info.grkind`).
    pub fn grkind(&self) -> i32 {
        self.info.grkind
    }

    /// A GObj's joint (`hsd_obj`), if it holds one.
    pub fn jobj(&self, g: GObjId) -> Option<JObjId> {
        match self.world.gobjs[g.0].obj {
            Some(super::super::state::Obj::JObj(j)) => Some(j),
            _ => None,
        }
    }

    /// A stage object's `Ground` from its GObj (`HSD_GObjGetUserData`).
    pub fn ground_of(&self, g: GObjId) -> super::super::handles::GroundId {
        match self.world.gobjs[g.0].user_data {
            Some(super::super::state::UserData::Ground(gr)) => gr,
            other => panic!("GObj {g:?} holds {other:?}, not a stage object"),
        }
    }

    /// The GObj of map `map_id`, `Ground_GetMapGObj`.
    pub fn gobj(&self, map_id: i32) -> Option<GObjId> {
        self.info.map_gobjs[map_id as usize]
    }

    /// The stage object of map `map_id`, `Ground_GetMapGObj(map_id)->user_data`.
    pub fn ground(&self, map_id: i32) -> Option<super::super::handles::GroundId> {
        let g = self.info.map_gobjs[map_id as usize]?;
        match self.world.gobjs[g.0].user_data {
            Some(super::super::state::UserData::Ground(gr)) => Some(gr),
            _ => None,
        }
    }
}

impl CollHooks<StageColl> for ObjCollHooks<'_> {
    /// The terrain a dynamic attribute area gives a floor at a point,
    /// `grDynamicAttr_801CA284`: the first area on the in-use list on a floor connected to it
    /// whose circle holds the point, else 0.
    fn dynamic_terrain(&mut self, g: &Geometry, pos: Vec3, line_id: i32) -> i32 {
        dynamic_terrain(&self.stage_maps.gr.dynamic_attrs, g, pos, line_id)
    }

    fn joint_callback(
        &mut self,
        g: &mut Geometry,
        callback: CodeRef,
        user_data: Option<super::super::gr::CollUser>,
        joint_id: i32,
        coll: &mut CollData,
        x50: i32,
        kind: i32,
        dy: f32,
    ) {
        match callback.0 {
            super::super::gr::kongo::BRIDGE_LANDED => self.kicks.landed(g, coll, kind),
            super::super::gr::kongo::PLATFORM_LANDED => {
                if let Some(super::super::gr::CollUser::Ground(gr)) = user_data {
                    super::super::gr::kongo::platform_landed(&mut self.grounds[gr.0], kind);
                }
            }
            super::super::gr::yorster::BLOCK_LANDED => {
                if let Some(super::super::gr::CollUser::Ground(g)) = user_data {
                    let (gr, grkind, data) =
                        (self.stage_maps.gr, self.stage_maps.grkind(), self.data);
                    let part_joint = |m: i32, p: i32| {
                        super::super::gr::setup::coll_joint_id_in(gr, grkind, data, m, p)
                    };
                    super::super::gr::yorster::block_landed(
                        &mut self.grounds[g.0],
                        gr.yorster_yakumono,
                        data,
                        &part_joint,
                        joint_id,
                        coll,
                        &mut self.landing,
                        dy,
                    );
                }
            }
            super::super::gr::oldyoshi::CLOUD_TOUCHED => {
                if let Some(super::super::gr::CollUser::Ground(g)) = user_data {
                    super::super::gr::oldyoshi::touch(
                        &mut self.grounds[g.0],
                        joint_id,
                        coll.x34_flags.b1234.into(),
                    );
                }
            }
            super::super::gr::fourside::ROOF_TOUCHED => super::super::gr::fourside::touch(
                self.grounds,
                self.stage_maps.ground(5),
                coll.x34_flags.b1234.into(),
            ),
            super::super::gr::greatbay::PLATFORM_TOUCHED => {
                if let Some(super::super::gr::CollUser::Ground(g)) = user_data {
                    super::super::gr::greatbay::platform_touched(
                        self.objs,
                        self.data,
                        self.grounds,
                        self.stage_maps.gr.greatbay_yakumono,
                        g,
                        coll,
                        x50,
                        kind,
                    );
                }
            }
            super::super::gr::greatbay::BALLOON_TOUCHED => {
                if let Some(super::super::gr::CollUser::Ground(g)) = user_data {
                    super::super::gr::greatbay::balloon_touched(&mut self.grounds[g.0], coll, x50);
                }
            }
            super::super::gr::greens::BLOCK_LANDED => {
                if let Some(super::super::gr::CollUser::Ground(g)) = user_data {
                    super::super::gr::greens::block_landed(&mut self.grounds[g.0], joint_id);
                }
            }
            super::super::gr::onett::ROOF_LANDED => {
                if let Some(super::super::gr::CollUser::Ground(g)) = user_data {
                    let param = self.stage_maps.gr.onett_yakumono;
                    super::super::gr::onett::roof_landed(
                        &mut self.grounds[g.0],
                        param,
                        self.data,
                        kind,
                    );
                }
            }
            super::super::gr::onett::AWNING_LANDED => {
                if let Some(super::super::gr::CollUser::Ground(g)) = user_data {
                    let param = self.stage_maps.gr.onett_yakumono;
                    super::super::gr::onett::awning_landed(
                        &mut self.grounds[g.0],
                        param,
                        self.data,
                        joint_id,
                        coll.x34_flags.b1234.into(),
                        kind,
                    );
                }
            }
            super::super::gr::rcruise::BLOCK_TOUCHED => {
                if let Some(super::super::gr::CollUser::GObj(go)) = user_data {
                    let (g, root) = (self.stage_maps.ground_of(go), self.stage_maps.jobj(go));
                    let kind = coll.x34_flags.b1234.into();
                    super::super::gr::rcruise::block_touched(
                        self.objs,
                        self.grounds,
                        root,
                        g,
                        joint_id,
                        kind,
                    );
                }
            }
            super::super::gr::rcruise::DECK_TOUCHED => {
                if let Some(super::super::gr::CollUser::GObj(go)) = user_data {
                    let (g, root) = (self.stage_maps.ground_of(go), self.stage_maps.jobj(go));
                    super::super::gr::rcruise::deck_touched(
                        self.objs,
                        self.data,
                        self.grounds,
                        root,
                        g,
                        coll,
                        x50,
                    );
                }
            }
            super::super::gr::rcruise::PLANK_TOUCHED => {
                if let Some(super::super::gr::CollUser::GObj(go)) = user_data {
                    let g = self.stage_maps.ground_of(go);
                    let kind = coll.x34_flags.b1234.into();
                    super::super::gr::rcruise::plank_touched(self.grounds, g, joint_id, kind);
                }
            }
            super::super::gr::icemt::ICE_A_TOUCHED
            | super::super::gr::icemt::ICE_B_TOUCHED
            | super::super::gr::icemt::ICE_C_TOUCHED => {
                if let Some(super::super::gr::CollUser::Ground(g)) = user_data {
                    let map = if callback.0 == super::super::gr::icemt::ICE_C_TOUCHED {
                        4
                    } else {
                        2
                    };
                    let root = self
                        .stage_maps
                        .gobj(map)
                        .and_then(|go| self.stage_maps.jobj(go));
                    let climb = self.stage_maps.ground(10);
                    super::super::gr::icemt::ice_touched(
                        self.objs,
                        self.grounds,
                        root,
                        g,
                        callback.0,
                        coll,
                        climb,
                    );
                }
            }
            _ => unported_call(&format!("stage joint callback {:#010x}", callback.0)),
        }
    }

    /// A fighter's floor filter. `ftCo_80096CC8`: any floor but a platform, and a platform too
    /// unless the stick is held down past `ftCommonData` x25C. `fn_800DD6E4`: any floor but a
    /// platform.
    #[doc(alias = "ftCo_80096CC8")]
    #[doc(alias = "fn_800DD6E4")]
    fn floor_filter(&mut self, callback: CodeRef, _gobj: Option<GObjId>, line_id: i32) -> bool {
        match callback {
            PLATFORM_FILTER => {
                let f = self
                    .filter
                    .as_ref()
                    .expect("a floor filter without its inputs");
                line_id != -1 && (!f.platforms[line_id as usize] || f.stick_y > f.threshold)
            }
            THROW_FILTER => {
                let f = self
                    .filter
                    .as_ref()
                    .expect("a floor filter without its inputs");
                line_id != -1 && !f.platforms[line_id as usize]
            }
            _ => unported_call(&format!("floor filter {:#010x}", callback.0)),
        }
    }

    fn joint_position(&mut self, jobj: Option<JObjId>) -> Vec3 {
        crate::lb::joint::world_position(self.objs, self.data, jobj, None)
    }
}

/// `grDynamicAttr_801CA284`.
#[doc(alias = "grDynamicAttr_801CA284")]
pub fn dynamic_terrain(attrs: &DynamicAttrs, g: &Geometry, pos: Vec3, line_id: i32) -> i32 {
    let mut cur = attrs.active;
    while let Some(i) = cur {
        let a = &attrs.areas[i as usize];
        if g.lines_connected(line_id, a.floor) {
            let dx = fp::fsubs(reg(pos.x), reg(a.pos.x));
            let dy = fp::fsubs(reg(pos.y), reg(a.pos.y));
            let d = fp::fadds(fp::fmuls(dx, dx), fp::fmuls(dy, dy));
            if math::sqrt(crate::fpu::mem(d)) < a.radius {
                return a.attr;
            }
        }
        cur = a.next;
    }
    0
}

/// The box an ECB description gives for the way the fighter faces, `ft_80082838`: mirrored
/// when facing left.
#[doc(alias = "ft_80082838")]
pub fn facing_box(b: &ftCollisionBox, facing: f32) -> ftCollisionBox {
    if facing < 0.0 {
        let mut m = b.clone();
        m.right.x = mem(fp::fneg(reg(b.left.x)));
        m.left.x = mem(fp::fneg(reg(b.right.x)));
        m
    } else {
        b.clone()
    }
}

impl Running<'_> {
    /// Runs one mpcoll pass on fighter `f`'s collision data from its position, putting the
    /// fighter where the pass leaves it: the result of `pass`.
    fn fighter_coll(
        &mut self,
        f: FighterId,
        prepare: impl FnOnce(&mut CollData),
        pass: impl FnOnce(&mut Coll<'_, StageColl, ObjCollHooks<'_>>, &mut CollData) -> bool,
    ) -> bool {
        self.fighter_coll_filtered(f, false, prepare, pass)
    }

    /// A collision pass from `last` to `cur`, the ECB zeroed first (`mpColl_80043670`), as code
    /// that moves a fighter itself runs it; the fighter ends where it does.
    pub(crate) fn fighter_coll_placed(
        &mut self,
        f: FighterId,
        last: Vec3,
        cur: Vec3,
        pass: impl FnOnce(&mut Coll<'_, StageColl, ObjCollHooks<'_>>, &mut CollData) -> bool,
    ) -> bool {
        self.fighter_coll(
            f,
            |cd| {
                cd.last_pos = last;
                cd.request_clear();
                cd.cur_pos = cur;
            },
            pass,
        )
    }

    /// [`Self::fighter_coll`], with the inputs of the fighter's floor filter when `filtered`.
    fn fighter_coll_filtered(
        &mut self,
        f: FighterId,
        filtered: bool,
        prepare: impl FnOnce(&mut CollData),
        pass: impl FnOnce(&mut Coll<'_, StageColl, ObjCollHooks<'_>>, &mut CollData) -> bool,
    ) -> bool {
        let threshold = if filtered {
            self.common().f32(0x25C)
        } else {
            0.0
        };
        let game = &mut *self.game;
        let landing = crate::game::gr::yorster::Landing::of(game);
        let geometry = game.geometry.as_mut().expect("no collision geometry");
        let platforms = &mut game.ft.platforms;
        platforms.clear();
        if filtered {
            platforms.extend(
                (0..geometry.lines.len() as i32)
                    .map(|l| geometry.line_surface(l) & LINE_PLATFORM != 0),
            );
        }
        let filter = filtered.then(|| FilterInput {
            stick_y: game.fighters[f.0].input.lstick[0].y,
            threshold,
            platforms: &game.ft.platforms,
        });
        let mut hooks = ObjCollHooks {
            objs: &mut game.objs,
            data: self.data,
            landing,
            kicks: Default::default(),
            grounds: &mut game.grounds,
            stage_maps: StageMaps::of_parts(&game.world, &game.stage_info, &game.gr),
            filter,
        };
        let mut coll = Coll {
            g: geometry,
            s: &mut game.coll,
            hooks: &mut hooks,
        };
        let fp = &mut game.fighters[f.0];
        let cd = &mut fp.coll_data;
        cd.last_pos = cd.cur_pos;
        cd.cur_pos = fp.cur_pos;
        prepare(cd);
        let hit = pass(&mut coll, cd);
        fp.cur_pos = fp.coll_data.cur_pos;
        let landing = hooks.landing;
        let kicks = std::mem::take(&mut hooks.kicks);
        game.ft.platforms.clear();
        landing.apply(game);
        kicks.apply(game, self.data);
        hit
    }

    /// An airborne pass, then whether the fighter stepped onto an item (`ft_80081A00`): what
    /// the pass returned, unless an item took the fighter.
    fn air_coll(
        &mut self,
        gobj: GObjId,
        filtered: bool,
        prepare: impl FnOnce(&mut CollData),
        pass: impl FnOnce(&mut Coll<'_, StageColl, ObjCollHooks<'_>>, &mut CollData) -> bool,
    ) -> bool {
        let f = self.game.fighter_id(gobj);
        let hit = self.fighter_coll_filtered(f, filtered, prepare, pass);
        !self.stepped_on_item(gobj) && hit
    }

    /// Whether the fighter's feet landed on an item it can step on, `ft_80081A00`: its feet's
    /// span is measured against every steppable item's (`inlineA0`), and an item that takes it
    /// bounces it up (`ftCo_800C703C`).
    #[doc(alias = "ft_80081A00")]
    pub fn stepped_on_item(&mut self, gobj: GObjId) -> bool {
        let fp = self.game.fighter_mut(gobj);
        if fp.x2219_b1 || fp.x2223_b4 {
            return false;
        }
        fp.x2223_b5 = true;
        let p = fp.coll_data.cur_pos;
        let (lx, rx) = (reg(fp.coll_data.ecb.left.x), reg(fp.coll_data.ecb.right.x));
        fp.dmg.x1930.xC.x = mem(fp::fadds(lx, reg(p.x)));
        fp.dmg.x1930.xC.y = mem(fp::fadds(rx, reg(p.x)));
        fp.dmg.x1930.xC.z = mem(fp::fadds(reg(fp.cur_pos.y), reg(fp.co_attrs.x17C)));
        let mut cur = self.game.world.first_in(ITEM_P_LINK);
        while let Some(item) = cur {
            cur = self.game.world.gobjs[item.0].next;
            if crate::game::it::lib26b1::is_stomp(self.game, item) {
                let feet = self.game.fighter(gobj).dmg.x1930.clone();
                let i = crate::game::it::item_of(self.game, item);
                if crate::lb::stomp::stepped_on(&feet, &self.game.items[i.0].xB54) {
                    let (fx, slot) = {
                        let fp = self.game.fighter(gobj);
                        (fp.cur_pos.x, i32::from(fp.player_idx))
                    };
                    let ip = &mut self.game.items[i.0];
                    ip.xCD0 = if ip.pos.x > fx { -1.0 } else { 1.0 };
                    ip.xCFC = Some(gobj);
                    ip.xCB0_source_ply = slot;
                    self.game.fighter_mut(gobj).dmg.x1950 = 1;
                }
            }
        }
        let fp = self.game.fighter_mut(gobj);
        if fp.dmg.x1950 != 0 {
            self.bounce_off_item(gobj);
            self.game.fighter_mut(gobj).dmg.x1950 = 0;
            return true;
        }
        false
    }

    /// `ftCo_800C703C`: an item stepped on bounces the fighter up.
    #[doc(alias = "ftCo_800C703C")]
    pub fn bounce_off_item(&mut self, gobj: GObjId) {
        let c = self.common();
        let (y, frames) = (c.f32(0x6D0), c.i32(0x6CC));
        let fp = self.game.fighter_mut(gobj);
        fp.xA4_unk_vel.x = fp.self_vel.x;
        fp.xA4_unk_vel.y = y;
        fp.xA4_unk_vel.z = 0.0;
        fp.dmg.x194C = frames;
        fp.dmg.x1948 = frames;
    }

    /// One fighter's collision state copied to another's, `ft_800849EC` (`mpCopyCollData`
    /// without the ECBs' first copy).
    #[doc(alias = "ft_800849EC")]
    pub fn copy_coll_data(&mut self, from: FighterId, to: FighterId) {
        let cd = self.game.fighters[from.0].coll_data.clone();
        self.game.fighters[to.0].coll_data.copy_from(&cd, 2);
    }

    /// The fighter's facing as mpcoll takes it (`ftGetFacingDirInt`).
    fn facing_int(&self, f: FighterId) -> i32 {
        if self.game.fighters[f.0].facing_dir < 0.0 {
            -1
        } else {
            1
        }
    }

    /// Whether a fighter that can't catch ledges now (on cooldown, or out of stamina).
    fn no_ledges(&self, f: FighterId) -> bool {
        let fp = &self.game.fighters[f.0];
        fp.x2064_ledgeCooldown != 0 || fp.stamina_dead
    }

    /// The airborne pass, landing on any floor, `ft_80081D0C`: true when the fighter landed.
    #[doc(alias = "ft_80081D0C")]
    pub fn air_coll_land(&mut self, gobj: GObjId) -> bool {
        self.air_coll(gobj, false, |_| {}, |c, cd| c.air_frame(cd, 6, 0))
    }

    /// The airborne pass that catches ledges in the fighter's ECB height, or, in hitstun,
    /// stays airborne on walls, `ft_80081DD4`.
    #[doc(alias = "ft_80081DD4")]
    pub fn air_coll_ledges_snap(&mut self, gobj: GObjId) -> bool {
        self.air_coll_snap(gobj, 6, None)
    }

    /// `ft_80081F2C`: [`Self::air_coll_ledges_snap`] with the ECB's lower flags.
    #[doc(alias = "ft_80081F2C")]
    pub fn air_coll_ledges_snap_low(&mut self, gobj: GObjId) -> bool {
        self.air_coll_snap(gobj, 10, None)
    }

    /// `ft_80082084`: [`Self::air_coll_ledges_snap`] with an unlocked ECB bottom.
    #[doc(alias = "ft_80082084")]
    pub fn air_coll_ledges_snap_unlocked(&mut self, gobj: GObjId) -> bool {
        self.air_coll_snap(gobj, 0x12, Some(()))
    }

    /// The body of `ft_80081DD4`, `ft_80081F2C` and `ft_80082084`: ECB flags `ecb` (0x12 with
    /// `unlocked`), pass flags 1 in hitstun, 0 off ledges, else 4 with the ledge snap height
    /// scaled by `ftCommonData` x1CC for the pass.
    fn air_coll_snap(&mut self, gobj: GObjId, ecb: u32, unlocked: Option<()>) -> bool {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        let pass =
            |c: &mut Coll<'_, StageColl, ObjCollHooks<'_>>, cd: &mut CollData, flags: u32| {
                if unlocked.is_some() {
                    c.air_frame_unlocked(cd, flags)
                } else {
                    c.air_frame(cd, ecb, flags)
                }
            };
        if fp.allow_sdi {
            return self.air_coll(gobj, false, |_| {}, |c, cd| pass(c, cd, 1));
        }
        if self.no_ledges(f) {
            return self.air_coll(gobj, false, |_| {}, |c, cd| pass(c, cd, 0));
        }
        let x44 = self.ft_data_x44(f);
        let snap = reg(self.data.f32(x44.at(0x18)));
        let scale = reg(fp.x34_scale.y);
        let mul = reg(self.common().f32(0x1CC));
        let scaled = mem(fp::fmuls(mul, fp::fmuls(snap, scale)));
        let plain = mem(fp::fmuls(snap, scale));
        self.air_coll(
            gobj,
            false,
            |cd| {
                if cd.ecb_source.kind == ECB_SOURCE_JOBJ {
                    cd.ledge_snap_height = scaled;
                }
            },
            |c, cd| {
                let hit = pass(c, cd, 4);
                if cd.ecb_source.kind == ECB_SOURCE_JOBJ {
                    cd.ledge_snap_height = plain;
                }
                hit
            },
        )
    }

    /// `ftData::x44`: the character's ECB joints and ledge snap.
    fn ft_data_x44(&self, f: FighterId) -> melee_data::archive::DataRef {
        let ft_data = self.game.fighters[f.0]
            .ft_data
            .expect("a fighter without ftData");
        self.data.ptr(ft_data.at(0x44)).expect("ftData without x44")
    }

    /// The airborne pass with the lower ECB flags, landing on any floor, `ft_800821DC`.
    #[doc(alias = "ft_800821DC")]
    pub fn air_coll_land_low(&mut self, gobj: GObjId) -> bool {
        self.air_coll(gobj, false, |_| {}, |c, cd| c.air_frame(cd, 10, 0))
    }

    /// The airborne pass catching ledges the fighter faces `dir`, `ft_CheckGroundAndLedge`
    /// (without ledges when it can't catch them).
    #[doc(alias = "ft_CheckGroundAndLedge")]
    pub fn air_coll_ground_and_ledge(&mut self, gobj: GObjId, dir: i32) -> bool {
        let f = self.game.fighter_id(gobj);
        if self.no_ledges(f) {
            self.air_coll(gobj, false, |_| {}, |c, cd| c.air_frame(cd, 6, 0))
        } else {
            self.air_coll(
                gobj,
                false,
                |cd| cd.set_facing_dir(dir),
                |c, cd| c.air_frame(cd, 6, 4),
            )
        }
    }

    /// The airborne pass with the ECB `b`, catching ledges facing `dir`, `ft_8008239C`.
    #[doc(alias = "ft_8008239C")]
    pub fn air_coll_box_ledges(&mut self, gobj: GObjId, dir: i32, b: &ftCollisionBox) -> bool {
        let f = self.game.fighter_id(gobj);
        let b = facing_box(b, self.game.fighters[f.0].facing_dir);
        if self.no_ledges(f) {
            self.air_coll(gobj, false, |_| {}, |c, cd| c.air_frame_box(cd, &b, 0))
        } else {
            self.air_coll(
                gobj,
                false,
                |cd| cd.set_facing_dir(dir),
                |c, cd| c.air_frame_box(cd, &b, 4),
            )
        }
    }

    /// The airborne pass with the ECB `b`, `ft_800824A0`.
    #[doc(alias = "ft_800824A0")]
    pub fn air_coll_box(&mut self, gobj: GObjId, b: &ftCollisionBox) -> bool {
        let f = self.game.fighter_id(gobj);
        let b = facing_box(b, self.game.fighters[f.0].facing_dir);
        self.air_coll(gobj, false, |_| {}, |c, cd| c.air_frame_box(cd, &b, 0))
    }

    /// The airborne pass with the ECB `b` that stays airborne (mpcoll's pass flags 1),
    /// `ft_80082638`: whether it touched a floor, unless the fighter stepped onto an item.
    #[doc(alias = "ft_80082638")]
    pub fn air_coll_box_floor(&mut self, gobj: GObjId, b: &ftCollisionBox) -> bool {
        let f = self.game.fighter_id(gobj);
        let b = facing_box(b, self.game.fighters[f.0].facing_dir);
        self.fighter_coll(f, |_| {}, |c, cd| c.air_frame_box(cd, &b, 1));
        if self.stepped_on_item(gobj) {
            return false;
        }
        self.game.fighters[f.0].coll_data.env_flags & FLOOR_MASK != 0
    }

    /// The grounded pass with the ECB `b` (mirrored facing left), `ft_80082888`: whether the
    /// fighter stays on the ground.
    #[doc(alias = "ft_80082888")]
    pub fn ground_coll_box(&mut self, gobj: GObjId, b: &ftCollisionBox) -> bool {
        let f = self.game.fighter_id(gobj);
        let b = facing_box(b, self.game.fighters[f.0].facing_dir);
        self.fighter_coll(f, |_| {}, |c, cd| c.ground_frame_box(cd, &b, 0))
    }

    /// [`Self::ground_coll_box`] with mpcoll's pass flags 2 (`mpColl_8004B3F0`), `ft_80082978`.
    #[doc(alias = "ft_80082978")]
    pub fn ground_coll_box_ledges(&mut self, gobj: GObjId, b: &ftCollisionBox) -> bool {
        let f = self.game.fighter_id(gobj);
        let b = facing_box(b, self.game.fighters[f.0].facing_dir);
        self.fighter_coll(f, |_| {}, |c, cd| c.ground_frame_box(cd, &b, 2))
    }

    /// The airborne pass that stays airborne, `ft_80082578`: whether a floor was touched.
    #[doc(alias = "ft_80082578")]
    pub fn air_coll_touch(&mut self, gobj: GObjId) -> bool {
        let f = self.game.fighter_id(gobj);
        self.fighter_coll(f, |_| {}, |c, cd| c.air_frame(cd, 6, 1));
        if self.stepped_on_item(gobj) {
            return false;
        }
        self.game.fighters[f.0].coll_data.env_flags & FLOOR_MASK != 0
    }

    /// The grounded pass that lets the fighter off ledges, `ft_80082708`: true when it went
    /// off (`GA_Air`).
    #[doc(alias = "ft_80082708")]
    pub fn ground_coll_off_ledges(&mut self, gobj: GObjId) -> bool {
        let f = self.game.fighter_id(gobj);
        self.fighter_coll(f, |_| {}, |c, cd| c.ground_frame(cd, 5, 0))
    }

    /// Stays on the ground, pushed off ledges (`ft_800827A0`: mpcoll's grounded pass with
    /// ledges stopping nothing).
    #[doc(alias = "ft_800827A0")]
    pub fn ground_coll_slide(&mut self, gobj: GObjId) -> bool {
        let f = self.game.fighter_id(gobj);
        self.fighter_coll(f, |_| {}, |c, cd| c.ground_frame(cd, 5, 2))
    }

    /// Stays on the ground, stopping at ledges it faces with the stick held toward them
    /// (`ft_80084280`'s inline: mpcoll's grounded pass with the stick and facing; `ecb` 9 is
    /// `ft_800843FC`'s).
    fn ground_coll_ledges(&mut self, f: FighterId, ecb: u32) -> bool {
        let fp = &self.game.fighters[f.0];
        let (stick, facing) = (fp.input.lstick[0].x, self.facing_int(f));
        self.fighter_coll(
            f,
            |cd| {
                cd.lstick_x = stick;
                cd.set_facing_dir(facing);
            },
            |c, cd| c.ground_frame(cd, ecb, 1),
        )
    }

    /// Landing from the air, `ft_80082B1C`: still rising past the landing threshold
    /// (`ftCo_800D0EC8`) goes straight to waiting, else the normal landing.
    #[doc(alias = "ft_80082B1C")]
    pub fn land_normally(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let threshold = self.landing_threshold(f);
        if reg(self.game.fighters[f.0].self_vel.y) > threshold {
            self.enter_wait(gobj);
        } else {
            self.enter_landing_basic(gobj);
        }
    }

    /// The airborne pass, landing normally, `ftCo_AirCatchHit_Coll`.
    #[doc(alias = "ftCo_AirCatchHit_Coll")]
    pub fn co_air_catch_hit_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.land_normally(gobj);
        }
    }

    /// The airborne pass, `cb` on landing, `ft_80082C74`.
    #[doc(alias = "ft_80082C74")]
    pub fn air_coll_then(&mut self, gobj: GObjId, cb: Cb) {
        if self.air_coll_land(gobj) {
            cb(self, gobj);
        }
    }

    /// The unlocked airborne pass, landing normally, `ft_80082D40` (its float argument is
    /// unused).
    #[doc(alias = "ft_80082D40")]
    pub fn air_coll_unlocked_land(&mut self, gobj: GObjId) {
        if self.air_coll(gobj, false, |_| {}, |c, cd| c.air_frame_unlocked(cd, 0)) {
            self.land_normally(gobj);
        }
    }

    /// The fighter on the ledge `gobj` would catch, `ft_80082E3C`: another fighter already
    /// hanging (`x221D_b7`) from a ledge connected to it, facing it.
    #[doc(alias = "ft_80082E3C")]
    pub fn ledge_taken_by(&self, gobj: GObjId) -> Option<GObjId> {
        let me = self.game.fighter(gobj);
        let g = self.game.geometry.as_ref().expect("no collision geometry");
        let mut cur = self.game.world.first_in(super::common::FIGHTER_P_LINK);
        while let Some(o) = cur {
            cur = self.game.world.gobjs[o.0].next;
            if o == gobj {
                continue;
            }
            let other = self.game.fighter(o);
            if !other.x221D_b7 {
                continue;
            }
            let floor = other.mv_i32(0);
            if other.facing_dir > 0.0
                && me.coll_data.env_flags & LEFT_LEDGE_GRAB != 0
                && g.lines_connected(floor, me.coll_data.ledge_id_left)
            {
                return Some(o);
            }
            if other.facing_dir < 0.0
                && me.coll_data.env_flags & RIGHT_LEDGE_GRAB != 0
                && g.lines_connected(floor, me.coll_data.ledge_id_right)
            {
                return Some(o);
            }
        }
        None
    }

    /// An airborne state's collision, `ft_80082F28`: landing normally, else a wall jump or a
    /// ledge.
    #[doc(alias = "ft_80082F28")]
    pub fn air_coll_full(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let dir = self.facing_int(f);
        if self.air_coll_ground_and_ledge(gobj, dir) {
            self.land_normally(gobj);
            return;
        }
        if self.wall_jump_check(gobj) {
            return;
        }
        self.catch_ledge_check(gobj);
    }

    /// The filtered airborne pass (`ft_80083090_inline`, ECB flags 6) and the ledge-less one
    /// below it: whether the fighter landed on a floor the filter let it.
    fn air_coll_filtered(&mut self, gobj: GObjId, filter: CodeRef, ecb: Option<u32>) -> bool {
        let f = self.game.fighter_id(gobj);
        let dir = self.facing_int(f);
        // Pass flags 2 off ledges, else 6 catching them.
        let off = self.no_ledges(f);
        self.air_coll(
            gobj,
            true,
            |cd| {
                if !off {
                    cd.set_facing_dir(dir);
                }
            },
            |c, cd| {
                let flags = if off { 2 } else { 6 };
                c.air_frame_filtered(cd, ecb, flags, Some(filter), Some(gobj))
            },
        )
    }

    /// A throw's airborne pass with its floor filter (`ft_80083CE4`, `mpColl_800476B4` with ECB
    /// flags 6 and pass flags 3): whether the fighter touched a floor, unless an item took it.
    #[doc(alias = "ft_80083CE4")]
    pub fn air_coll_throw(&mut self, gobj: GObjId) -> bool {
        let f = self.game.fighter_id(gobj);
        self.fighter_coll_filtered(
            f,
            true,
            |_| {},
            |c, cd| c.air_frame_filtered(cd, Some(6), 3, Some(THROW_FILTER), Some(gobj)),
        );
        if self.stepped_on_item(gobj) {
            return false;
        }
        self.game.fighters[f.0].coll_data.env_flags & FLOOR_MASK != 0
    }

    /// The filtered airborne pass, `cb` on landing, else a ledge, `ft_80083090`.
    #[doc(alias = "ft_80083090")]
    pub fn air_coll_filtered_ledge(&mut self, gobj: GObjId, filter: CodeRef, cb: Cb) {
        if self.air_coll_filtered(gobj, filter, Some(6)) {
            cb(self, gobj);
            return;
        }
        self.catch_ledge_check(gobj);
    }

    /// The filtered airborne pass, `cb` on landing, else a wall jump or a ledge,
    /// `ft_800831CC`.
    #[doc(alias = "ft_800831CC")]
    pub fn air_coll_filtered_full(&mut self, gobj: GObjId, filter: CodeRef, cb: Cb) {
        if self.air_coll_filtered(gobj, filter, Some(6)) {
            cb(self, gobj);
            return;
        }
        if self.wall_jump_check(gobj) {
            return;
        }
        self.catch_ledge_check(gobj);
    }

    /// [`Self::air_coll_filtered_full`] with the lower ECB flags, `ft_80083318`.
    #[doc(alias = "ft_80083318")]
    pub fn air_coll_filtered_full_low(&mut self, gobj: GObjId, filter: CodeRef, cb: Cb) {
        if self.air_coll_filtered(gobj, filter, Some(10)) {
            cb(self, gobj);
            return;
        }
        if self.wall_jump_check(gobj) {
            return;
        }
        self.catch_ledge_check(gobj);
    }

    /// [`Self::air_coll_filtered_full`] with an unlocked ECB bottom, `ft_80083464`.
    #[doc(alias = "ft_80083464")]
    pub fn air_coll_filtered_full_unlocked(&mut self, gobj: GObjId, filter: CodeRef, cb: Cb) {
        if self.air_coll_filtered(gobj, filter, None) {
            cb(self, gobj);
            return;
        }
        if self.wall_jump_check(gobj) {
            return;
        }
        self.catch_ledge_check(gobj);
    }

    /// The jumps' collision, `ft_800835B0`: [`Self::air_coll_filtered_full`], then hitting a
    /// ceiling (`ftCo_8009EF68`).
    #[doc(alias = "ft_800835B0")]
    pub fn air_coll_filtered_ceiling(&mut self, gobj: GObjId, filter: CodeRef, cb: Cb) {
        if self.air_coll_filtered(gobj, filter, Some(6)) {
            cb(self, gobj);
            return;
        }
        if self.wall_jump_check(gobj) {
            return;
        }
        if self.catch_ledge_check(gobj) {
            return;
        }
        self.stop_at_ceiling_check(gobj);
    }

    /// The airborne pass catching ledges, `cb` on landing, else a wall jump or a ledge,
    /// `ft_8008370C`.
    #[doc(alias = "ft_8008370C")]
    pub fn air_coll_ledges_then(&mut self, gobj: GObjId, cb: Cb) {
        let f = self.game.fighter_id(gobj);
        let dir = self.facing_int(f);
        if self.air_coll_ground_and_ledge(gobj, dir) {
            cb(self, gobj);
            return;
        }
        if self.wall_jump_check(gobj) {
            return;
        }
        self.catch_ledge_check(gobj);
    }

    /// The airborne pass with the grounded ECB flags and no callback (`ft_80083DCC`'s).
    pub fn air_coll_plain(&mut self, gobj: GObjId) {
        self.air_coll(gobj, false, |_| {}, |c, cd| c.air_frame(cd, 5, 1));
    }

    /// The airborne pass with the grounded ECB flags, `cb` on landing, `ft_80083844`.
    #[doc(alias = "ft_80083844")]
    pub fn air_coll_grounded_ecb_then(&mut self, gobj: GObjId, cb: Cb) {
        if self.air_coll(gobj, false, |_| {}, |c, cd| c.air_frame(cd, 5, 0)) {
            cb(self, gobj);
        }
    }

    /// [`Self::air_coll_ledges_then`] with the grounded ECB flags, `ft_80083910`.
    #[doc(alias = "ft_80083910")]
    pub fn air_coll_grounded_ecb_ledges_then(&mut self, gobj: GObjId, cb: Cb) {
        let f = self.game.fighter_id(gobj);
        let dir = self.facing_int(f);
        let landed = if self.no_ledges(f) {
            self.air_coll(gobj, false, |_| {}, |c, cd| c.air_frame(cd, 5, 0))
        } else {
            self.air_coll(
                gobj,
                false,
                |cd| cd.set_facing_dir(dir),
                |c, cd| c.air_frame(cd, 5, 4),
            )
        };
        if landed {
            cb(self, gobj);
            return;
        }
        if self.wall_jump_check(gobj) {
            return;
        }
        self.catch_ledge_check(gobj);
    }

    /// The airborne pass catching ledges either way, `cb` on landing, `ft_80083A48`.
    #[doc(alias = "ft_80083A48")]
    pub fn air_coll_any_ledge_then(&mut self, gobj: GObjId, cb: Cb) {
        if self.air_coll_ground_and_ledge(gobj, 0) {
            cb(self, gobj);
            return;
        }
        if self.wall_jump_check(gobj) {
            return;
        }
        self.catch_ledge_check(gobj);
    }

    /// The airborne pass that stays airborne, `ft_80083B68`.
    #[doc(alias = "ft_80083B68")]
    pub fn air_coll_stay(&mut self, gobj: GObjId) {
        self.air_coll_touch(gobj);
    }

    /// The airborne pass that stays airborne, `cb` when it touched a floor, `ft_80083C00`.
    #[doc(alias = "ft_80083C00")]
    pub fn air_coll_touch_then(&mut self, gobj: GObjId, cb: Cb) {
        if self.air_coll_touch(gobj) {
            cb(self, gobj);
        }
    }

    /// A grounded state's collision that falls off ledges, `ft_80083F88`: off a ledge, the
    /// fighter falls.
    #[doc(alias = "ft_80083F88")]
    pub fn ground_coll_fall_off(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// `ft_8008403C`: [`Self::ground_coll_fall_off`] with `cb` instead of falling.
    #[doc(alias = "ft_8008403C")]
    pub fn ground_coll_fall_off_then(&mut self, gobj: GObjId, cb: Cb) {
        if !self.ground_coll_off_ledges(gobj) {
            cb(self, gobj);
        }
    }

    /// A grounded state's collision (`ft_80084104`): the fighter stays on the ground or falls.
    #[doc(alias = "ft_80084104")]
    pub fn ground_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_slide(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// `ft_800841B8`: [`Self::ground_coll`] with `cb` instead of falling.
    #[doc(alias = "ft_800841B8")]
    pub fn ground_coll_then(&mut self, gobj: GObjId, cb: Cb) {
        if !self.ground_coll_slide(gobj) {
            cb(self, gobj);
        }
    }

    /// A grounded state's collision that can teeter (`ft_80084280`): the fighter stays on the
    /// ground, stops at a ledge to teeter, or falls.
    #[doc(alias = "ft_80084280")]
    pub fn ground_coll_teeter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        let nudge = reg(fp.xF8_playerNudgeVel.x);
        let grounded = if nudge != 0.0 && fp::fmuls(nudge, reg(fp.facing_dir)) < 0.0 {
            self.ground_coll_slide(gobj)
        } else {
            self.ground_coll_ledges(f, 5)
        };
        if grounded || self.teeter(gobj) {
            return;
        }
        self.enter_fall(gobj);
    }

    /// `ft_800843FC`: [`Self::ground_coll_teeter`] with the low ECB flags and no nudge.
    #[doc(alias = "ft_800843FC")]
    pub fn ground_coll_teeter_low(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.ground_coll_ledges(f, 9) || self.teeter(gobj) {
            return;
        }
        self.enter_fall(gobj);
    }

    /// The dash's and run's collision, `ft_800844EC`: off a ledge the fighter falls; on the
    /// ground, a wall it runs into stops it (`ftCo_8009EDA4`).
    #[doc(alias = "ft_800844EC")]
    pub fn ground_coll_run(&mut self, gobj: GObjId) {
        if self.ground_coll_off_ledges(gobj) {
            self.stop_at_wall_check(gobj);
        } else {
            self.enter_fall(gobj);
        }
    }

    /// Going off a ledge it backs off slips (`ftCo_8009F39C`), else `cb`: the body of
    /// `ft_800845B4` and `ft_800848DC`.
    fn ground_coll_slip(&mut self, gobj: GObjId, cb: Cb) {
        if self.ground_coll_off_ledges(gobj) {
            return;
        }
        let fp = self.game.fighter(gobj);
        let flags = fp.coll_data.env_flags;
        if (fp.facing_dir < 0.0 && flags & RIGHT_LEDGE_SLIP != 0)
            || (fp.facing_dir > 0.0 && flags & LEFT_LEDGE_SLIP != 0)
        {
            self.enter_miss_foot(gobj);
            return;
        }
        cb(self, gobj);
    }

    /// `ft_800845B4`: off a ledge backwards the fighter slips, else falls.
    #[doc(alias = "ft_800845B4")]
    pub fn ground_coll_slip_fall(&mut self, gobj: GObjId) {
        self.ground_coll_slip(gobj, |g, gobj| g.enter_fall(gobj));
    }

    /// `ft_800848DC`: off a ledge backwards the fighter slips, else `cb`.
    #[doc(alias = "ft_800848DC")]
    pub fn ground_coll_slip_then(&mut self, gobj: GObjId, cb: Cb) {
        self.ground_coll_slip(gobj, cb);
    }

    /// The airborne pass with the ECB `b`, `cb` on landing, `ft_80083E64`.
    #[doc(alias = "ft_80083E64")]
    pub fn air_coll_box_then(&mut self, gobj: GObjId, b: &ftCollisionBox, cb: Cb) {
        let f = self.game.fighter_id(gobj);
        let b = facing_box(b, self.game.fighters[f.0].facing_dir);
        if self.air_coll(gobj, false, |_| {}, |c, cd| c.air_frame_box(cd, &b, 0)) {
            cb(self, gobj);
        }
    }

    /// The grounded pass with the ECB `b`, `cb` off the ground, `ft_800846B0`.
    #[doc(alias = "ft_800846B0")]
    pub fn ground_coll_box_then(&mut self, gobj: GObjId, b: &ftCollisionBox, cb: Cb) {
        let f = self.game.fighter_id(gobj);
        let b = facing_box(b, self.game.fighters[f.0].facing_dir);
        if !self.fighter_coll(f, |_| {}, |c, cd| c.ground_frame_box(cd, &b, 0)) {
            cb(self, gobj);
        }
    }

    /// The grounded pass with the ECB `b`, falling off the ground, `ft_800847D0`.
    #[doc(alias = "ft_800847D0")]
    pub fn ground_coll_box_fall(&mut self, gobj: GObjId, b: &ftCollisionBox) {
        let f = self.game.fighter_id(gobj);
        let b = facing_box(b, self.game.fighters[f.0].facing_dir);
        if !self.fighter_coll(f, |_| {}, |c, cd| c.ground_frame_box(cd, &b, 2)) {
            self.enter_fall(gobj);
        }
    }

    /// Whether the fighter's floor is live, `ft_80084A18`.
    #[doc(alias = "ft_80084A18")]
    pub fn on_live_floor(&self, gobj: GObjId) -> bool {
        let g = self.game.geometry.as_ref().expect("no collision geometry");
        g.line_live(self.game.fighter(gobj).coll_data.floor.index)
    }

    /// Teeters at the ledge the collision stopped the fighter at, `ftCo_8009A3C8`.
    #[doc(alias = "ftCo_8009A3C8")]
    pub fn teeter(&mut self, gobj: GObjId) -> bool {
        let fp = self.game.fighter(gobj);
        if fp.coll_data.env_flags & AT_LEDGE != 0 && !fp.is_sandbag {
            self.enter_ottotto(gobj);
            return true;
        }
        false
    }

    /// Running into a wall at more than walking speed stops the fighter, `ftCo_8009EDA4`.
    #[doc(alias = "ftCo_8009EDA4")]
    pub fn stop_at_wall_check(&mut self, gobj: GObjId) -> bool {
        let fp = self.game.fighter(gobj);
        let flags = fp.coll_data.env_flags;
        let right = fp.facing_dir == -1.0 && flags & RIGHT_WALL_HUG != 0;
        if right || (fp.facing_dir == 1.0 && flags & LEFT_WALL_HUG != 0) {
            let v = reg(fp.gr_vel);
            let abs = if v < 0.0 { fp::fneg(v) } else { v };
            if abs > reg(fp.co_attrs.walk_max_vel) {
                self.enter_stop_wall(gobj, right);
                return true;
            }
        }
        false
    }

    /// Rising into a ceiling stops the fighter, `ftCo_8009EF68`.
    #[doc(alias = "ftCo_8009EF68")]
    pub fn stop_at_ceiling_check(&mut self, gobj: GObjId) -> bool {
        if self.game.fighter(gobj).coll_data.env_flags & CEILING_HUG != 0 {
            self.enter_stop_ceil(gobj);
            return true;
        }
        false
    }

    /// Catches the ledge the collision found, unless the stick is held down or another fighter
    /// hangs there, `ftCliffCommon_80081298`.
    #[doc(alias = "ftCliffCommon_80081298")]
    pub fn catch_ledge_check(&mut self, gobj: GObjId) -> bool {
        let down = reg(self.common().f32(0x480));
        let fp = self.game.fighter(gobj);
        if reg(fp.input.lstick[0].y) <= fp::fneg(down) {
            return false;
        }
        if fp.coll_data.env_flags & LEDGE_GRAB_MASK == 0 || fp.is_sandbag {
            return false;
        }
        match self.ledge_taken_by(gobj) {
            None => {
                let (slot, sub) = (fp.player_idx, fp.is_sub_fighter);
                self.record_ledge_catch(i32::from(slot), sub);
                self.enter_cliff_catch(gobj);
                true
            }
            Some(other) => {
                let o = self.game.fighter(other);
                let ledge = o.mv_i32(0);
                let (os, osub) = (i32::from(o.player_idx), i32::from(o.is_sub_fighter));
                let (ms, msub) = (i32::from(fp.player_idx), i32::from(fp.is_sub_fighter));
                self.record_ledge_steal(os, osub, ms, msub, ledge);
                self.game.fighter_mut(gobj).x213C = ledge;
                false
            }
        }
    }

    /// The landing threshold, `ftCo_800D0EC8`: `ftCommonData` x310 scaled by the fighter's
    /// height, negated.
    #[doc(alias = "ftCo_800D0EC8")]
    pub fn landing_threshold(&self, f: FighterId) -> f64 {
        let x310 = reg(self.common().f32(0x310));
        let k = reg(self.data.f32(self.common_ptr(0x30).at(0x30)));
        let scale = reg(self.game.fighters[f.0].x34_scale.y);
        fp::fneg(scaled_by_height(x310, scale, k))
    }

    /// Whether the fighter is airborne.
    pub fn is_airborne(&self, gobj: GObjId) -> bool {
        self.game.fighter(gobj).ground_or_air == GA_AIR
    }
}

/// A value scaled by a fighter's height, `ftCo_CalcYScaledKnockback`: `k` 0 leaves it, a
/// negative `k` divides by the scale for `-k`, else the scale's distance from 1 times `k`
/// (or, shrunk with `k` above 1, the scale over `k`).
#[doc(alias = "ftCo_CalcYScaledKnockback")]
pub fn scaled_by_height(v: f64, scale: f64, k: f64) -> f64 {
    assert!(scale != 0.0, "ftchangeparam.c:30: scale != 0.0F");
    if k == 0.0 {
        return v;
    }
    if k < 0.0 {
        return fp::fdivs(v, scaled_by_height(1.0, scale, fp::fneg(k)));
    }
    if scale >= 1.0 || k <= 1.0 {
        return fp::fmadds(k, fp::fmuls(fp::fsubs(scale, 1.0), v), v);
    }
    fp::fdivs(fp::fmuls(v, scale), k)
}
