//! Fighters' environment collision (`ft_081B.c`): each action state's collision callback runs
//! mpcoll on the fighter's `CollData` from its position, keeps the result, and changes state
//! when the fighter leaves the ground or lands.

use gekko_fp as fp;

use super::super::code::unported_call;
use super::super::gr::{DynamicAttrs, Geometry, StageColl};
use super::super::handles::{CodeRef, FighterId};
use super::super::state::Running;
use super::super::structs::CollData;
use crate::fpu::reg;
use crate::hsd::Objects;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::JObjId;
use crate::math;
use crate::mp::coll::{Coll, CollHooks};
use crate::vec::Vec3;
use melee_data::archive::Archives;

/// `CollData::env_flags` bit 23: the fighter is at a ledge it could teeter on.
const AT_LEDGE: i32 = 0x80_0000;

/// What mpcoll asks of the game during a fighter's pass.
pub struct FighterCollHooks<'a> {
    pub objs: &'a mut Objects,
    pub data: &'a Archives,
    pub dynamic_attrs: &'a DynamicAttrs,
}

impl CollHooks<StageColl> for FighterCollHooks<'_> {
    /// The terrain a dynamic attribute area gives a floor at a point,
    /// `grDynamicAttr_801CA284`: the first area on the in-use list on a floor connected to it
    /// whose circle holds the point, else 0.
    fn dynamic_terrain(&mut self, g: &Geometry, pos: Vec3, line_id: i32) -> i32 {
        dynamic_terrain(self.dynamic_attrs, g, pos, line_id)
    }

    fn joint_callback(
        &mut self,
        _g: &mut Geometry,
        callback: CodeRef,
        _user_data: Option<super::super::handles::GroundId>,
        _joint_id: i32,
        _coll: &mut CollData,
        _x50: i32,
        _kind: i32,
        _dy: f32,
    ) {
        unported_call(&format!("stage joint callback {:#010x}", callback.0))
    }

    fn floor_filter(&mut self, callback: CodeRef, _gobj: Option<GObjId>, _line_id: i32) -> bool {
        unported_call(&format!("floor filter {:#010x}", callback.0))
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

impl Running<'_> {
    /// Runs one mpcoll pass on fighter `f`'s collision data from its position, putting the
    /// fighter where the pass leaves it: the result of `pass`.
    fn fighter_coll(
        &mut self,
        f: FighterId,
        prepare: impl FnOnce(&mut CollData),
        pass: impl FnOnce(&mut Coll<'_, StageColl, FighterCollHooks<'_>>, &mut CollData) -> bool,
    ) -> bool {
        let game = &mut *self.game;
        let geometry = game.geometry.as_mut().expect("no collision geometry");
        let mut hooks = FighterCollHooks {
            objs: &mut game.objs,
            data: self.data,
            dynamic_attrs: &game.gr.dynamic_attrs,
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
        hit
    }

    /// Stays on the ground, pushed off ledges (`ft_800827A0`: mpcoll's grounded pass with
    /// ledges stopping nothing).
    #[doc(alias = "ft_800827A0")]
    pub(crate) fn ground_coll_slide(&mut self, f: FighterId) -> bool {
        self.fighter_coll(f, |_| {}, |c, cd| c.ground_frame(cd, 5, 2))
    }

    /// Stays on the ground, stopping at ledges it faces with the stick held toward them
    /// (`ft_80084280`'s inline: mpcoll's grounded pass with the stick and facing).
    fn ground_coll_ledges(&mut self, f: FighterId) -> bool {
        let fp = &self.game.fighters[f.0];
        let (stick, facing) = (
            fp.input.lstick[0].x,
            if fp.facing_dir < 0.0 { -1 } else { 1 },
        );
        self.fighter_coll(
            f,
            |cd| {
                cd.lstick_x = stick;
                cd.set_facing_dir(facing);
            },
            |c, cd| c.ground_frame(cd, 5, 1),
        )
    }

    /// A grounded state's collision that can teeter (`ft_80084280`): the fighter stays on the
    /// ground, stops at a ledge to teeter, or falls.
    #[doc(alias = "ft_80084280")]
    pub fn ground_coll_teeter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        let nudge = reg(fp.xF8_playerNudgeVel.x);
        let grounded = if nudge != 0.0 && fp::fmuls(nudge, reg(fp.facing_dir)) < 0.0 {
            self.ground_coll_slide(f)
        } else {
            self.ground_coll_ledges(f)
        };
        if grounded || self.teeter(gobj) {
            return;
        }
        self.enter_fall(gobj);
    }

    /// A grounded state's collision (`ft_80084104`): the fighter stays on the ground or falls.
    #[doc(alias = "ft_80084104")]
    pub fn ground_coll(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if !self.ground_coll_slide(f) {
            self.enter_fall(gobj);
        }
    }

    /// Teeters at the ledge the collision stopped the fighter at, `ftCo_8009A3C8` (entering
    /// the teeter isn't ported).
    #[doc(alias = "ftCo_8009A3C8")]
    fn teeter(&mut self, gobj: GObjId) -> bool {
        let fp = self.game.fighter(gobj);
        if fp.coll_data.env_flags & AT_LEDGE != 0 && !fp.is_sandbag {
            unported_call("ftCo_8009A410");
        }
        false
    }
}
