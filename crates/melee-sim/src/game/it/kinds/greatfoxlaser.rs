//! The Great Fox's laser, `melee/it/kinds/itgreatfoxlaser.c` (`It_Kind_GreatFox_Laser`):
//! Corneria's Great Fox fires it from its guns (`gr::corneria`), a big single shot (type 0) or a
//! volley's (type 1). It rides its gun while its animation charges it, then flies left at its
//! type's speed; reflected, it turns back from where its reflector stands. An enemy item
//! (`itZako_ItemVars` first in its variables) without collision of its own.

use gekko_fp as fp;

use super::super::item::{article, change, change_state, spawn_air};
use super::super::{Var, coll, item, item_mut, jobj_of, lib26b1, lib2725, zako};
use crate::fpu::{mem, reg};
use crate::game::it::ItKind;
use crate::game::mirror::{Cx, Mem, Mirror};
use crate::game::state::{Game, Running};
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::lb::joint::world_position;
use crate::vec::Vec3;

pub const SPAWNED: u32 = 0x802E_B1EC;
pub const ANIM: u32 = 0x802E_B2C0;
pub const PHYS: u32 = 0x802E_B3C0;
pub const DEALT: u32 = 0x802E_B444;
pub const CLANKED: u32 = 0x802E_B44C;
pub const ABSORBED: u32 = 0x802E_B454;
pub const REFLECTED: u32 = 0x802E_B45C;

/// `itGreatFoxLaser_Attrs`: a big shot's damage and speed, then a volley shot's.
mod attr {
    pub const BIG_DAMAGE: u32 = 0;
    pub const BIG_SPEED: u32 = 4;
    pub const DAMAGE: u32 = 8;
    pub const SPEED: u32 = 0xC;
}

/// Runs one of the laser's predicates, if `addr` is one.
#[melee_callbacks::callbacks(Pred)]
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM => animate(r, gobj),
        DEALT | CLANKED | ABSORBED => false,
        REFLECTED => reflected(r, gobj),
        _ => return None,
    })
}

/// Runs one of the laser's events, if `addr` is one.
#[melee_callbacks::callbacks(Proc)]
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        PHYS => physics(r, gobj),
        _ => return false,
    }
    true
}

/// `itGreatFoxLaser_ItemVars` over `itZako_ItemVars`, offsets from `xDD4`: the enemies' common
/// words, the Great Fox's object (`x20`) and the gun's offset in it (`x28`), whether a
/// reflection turned it (`x34`), its ticks left in its state (`x36`) and its type (`x38`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct GreatFoxLaserVars {
    pub jobj: Var<Option<JObjId>>,
    pub idx: Var<i32>,
    #[doc(alias("x8"))]
    pub normal: Var<Vec3>,
    #[doc(alias("x14"))]
    pub words: [Var<i32>; 3],
    #[doc(alias("x20"))]
    pub great_fox: Var<Option<GObjId>>,
    #[doc(alias("x28"))]
    pub gun_offset: Var<Vec3>,
    #[doc(alias("x34"))]
    pub reflected: Var<i16>,
    #[doc(alias("x36"))]
    pub state_ticks: Var<i16>,
    #[doc(alias("x38"))]
    pub shot_type: Var<i16>,
}

impl GreatFoxLaserVars {
    /// Marks every field stale, as a new item finds its variables.
    pub fn mark_stale(&mut self) {
        self.jobj.mark_stale();
        self.idx.mark_stale();
        self.normal.mark_stale();
        for v in &mut self.words {
            v.mark_stale();
        }
        self.great_fox.mark_stale();
        self.gun_offset.mark_stale();
        for v in [
            &mut self.reflected,
            &mut self.state_ticks,
            &mut self.shot_type,
        ] {
            v.mark_stale();
        }
    }
}

impl Mirror for GreatFoxLaserVars {
    const SIZE: u32 = 0x3C;

    fn import(m: &Mem, at: u32, cx: &mut dyn Cx) -> Self {
        let f = |a: u32| f32::from_bits(m.u32(a));
        let i = |a: u32| Var::new(m.u32(a) as i32);
        let h = |a: u32| Var::new(m.u16(a) as i16);
        GreatFoxLaserVars {
            jobj: Var::new(cx.import_handle("JObjId", m.u32(at)).map(JObjId)),
            idx: i(at + 4),
            normal: Var::new(Vec3::new(f(at + 8), f(at + 0xC), f(at + 0x10))),
            words: std::array::from_fn(|k| i(at + 0x14 + 4 * k as u32)),
            great_fox: Var::new(cx.import_handle("GObjId", m.u32(at + 0x20)).map(GObjId)),
            gun_offset: Var::new(Vec3::new(f(at + 0x28), f(at + 0x2C), f(at + 0x30))),
            reflected: h(at + 0x34),
            state_ticks: h(at + 0x36),
            shot_type: h(at + 0x38),
        }
    }

    fn compare(&self, m: &Mem, at: u32, path: &str, cx: &mut dyn Cx) {
        let p = |n: &str| format!("{path}.{n}");
        if self.jobj.live(&p("jobj"), cx) {
            let ours = self.jobj.peek().map(|j| j.0);
            cx.compare_handle(&p("jobj"), "JObjId", ours, m.u32(at));
        }
        let ints: [(&str, &Var<i32>, u32); 4] = [
            ("idx", &self.idx, 4),
            ("x14.x", &self.words[0], 0x14),
            ("x14.y", &self.words[1], 0x18),
            ("x14.z", &self.words[2], 0x1C),
        ];
        for (name, v, off) in ints {
            if v.live(&p(name), cx) && *v.peek() != m.u32(at + off) as i32 {
                cx.mismatch(
                    &p(name),
                    format!("{:?}", v.peek()),
                    format!("{:?}", m.u32(at + off) as i32),
                );
            }
        }
        self.normal.compare(m, at + 8, &p("x8"), cx);
        if self.great_fox.live(&p("x20"), cx) {
            let ours = self.great_fox.peek().map(|g| g.0);
            cx.compare_handle(&p("x20"), "GObjId", ours, m.u32(at + 0x20));
        }
        self.gun_offset.compare(m, at + 0x28, &p("x28"), cx);
        let shorts: [(&str, &Var<i16>, u32); 3] = [
            ("x34", &self.reflected, 0x34),
            ("x36", &self.state_ticks, 0x36),
            ("x38", &self.shot_type, 0x38),
        ];
        for (name, v, off) in shorts {
            if v.live(&p(name), cx) && *v.peek() != m.u16(at + off) as i16 {
                cx.mismatch(
                    &p(name),
                    format!("{:?}", v.peek()),
                    format!("{:?}", m.u16(at + off) as i16),
                );
            }
        }
    }
}

fn vars(game: &mut Game, gobj: GObjId) -> &mut GreatFoxLaserVars {
    item_mut(game, gobj).vars.greatfox_laser()
}

/// The laser's attribute at `off`.
fn attr_f(r: &Running, gobj: GObjId, off: u32) -> f32 {
    let a = article(item(r.game, gobj))
        .special(r.data)
        .expect("the Great Fox's laser without attributes");
    r.data.f32(a.at(off))
}

/// Its hitbox's damage from an attribute.
fn set_damage(r: &mut Running, gobj: GObjId, off: u32) {
    let d = crate::ppc::cvt_fp2unsigned(reg(attr_f(r, gobj, off)));
    coll::set_hit_damage(r.game, r.data, gobj, 0, d);
}

/// Fires a laser from the Great Fox's `owner` at `offset` in its model, `it_802EAF34`: of type
/// `ty`, facing left, at the stage's scale, its type's damage.
#[doc(alias = "it_802EAF34")]
pub fn spawn(r: &mut Running, owner: GObjId, offset: Vec3, ty: i32) -> Option<GObjId> {
    let j = super::super::super::gr::ground::jobj_of(r.game, owner);
    let at = world_position(&mut r.game.objs, r.data, j, Some(offset));
    let mut spawn = SpawnItem {
        kind: ItKind::GreatFoxLaser.id(),
        prev_pos: at,
        pos: at,
        facing_dir: -1.0,
        x3C_damage: 0,
        vel: Vec3::new(0.0, 0.0, 0.0),
        x0_parent_gobj: None,
        x4_parent_gobj2: None,
        x40: 0,
        ..Default::default()
    };
    spawn.x44_flag[0] &= !0x80;
    let gobj = spawn_air(r, &mut spawn)?;
    let s = super::super::super::gr::ground::scale(r.game);
    item_mut(r.game, gobj).scl = s;
    let mj = jobj_of(r.game, gobj).expect("the Great Fox's laser without a model");
    jobj::set_srt(&mut r.game.objs.jobjs, mj, |o| o.scale.x = s);
    jobj::set_srt(&mut r.game.objs.jobjs, mj, |o| o.scale.y = s);
    jobj::set_srt(&mut r.game.objs.jobjs, mj, |o| o.scale.z = s);
    let v = vars(r.game, gobj);
    v.great_fox.set(Some(owner));
    v.gun_offset.set(offset);
    v.shot_type.set(ty as i16);
    let off = if vars(r.game, gobj).shot_type.get() == 0 {
        attr::BIG_DAMAGE
    } else {
        attr::DAMAGE
    };
    set_damage(r, gobj, off);
    enter(r, gobj);
    Some(gobj)
}

/// The laser's spawned callback, `it_802EB1EC`: an enemy's reset, facing its way, flags
/// cleared, and its state.
#[doc(alias = "it_802EB1EC")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    zako::reset(r.game, gobj);
    let face = item(r.game, gobj).facing_dir;
    zako::face(r, gobj, face);
    let ip = item_mut(r.game, gobj);
    ip.state_flags.checks_blast_zones = false;
    ip.skid = 0;
    ip.flags.flags.pickup = false;
    super::super::hitbox::clear_hit_ids_b7(r.game, gobj);
    vars(r.game, gobj).state_ticks.set(1);
    enter(r, gobj);
}

/// Puts the laser in its charging state, `it_802EB268`: 60 ticks, not reflected, not to be
/// picked up.
#[doc(alias = "it_802EB268")]
pub fn enter(r: &mut Running, gobj: GObjId) {
    lib26b1::disable_pickup(r.game, gobj);
    let v = vars(r.game, gobj);
    v.state_ticks.set(60);
    v.reflected.set(0);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// The laser's animation callback, `itGreatfoxlaser_UnkMotion1_Anim`: once its animation
/// ends it flies at its type's speed with its type's damage, until then it rides its gun; it
/// ends when its ticks run out.
#[doc(alias = "itGreatfoxlaser_UnkMotion1_Anim")]
pub fn animate(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        let big = vars(r.game, gobj).shot_type.get() == 0;
        let speed = reg(attr_f(
            r,
            gobj,
            if big { attr::BIG_SPEED } else { attr::SPEED },
        ));
        let ip = item_mut(r.game, gobj);
        ip.vel.x = mem(fp::fmuls(speed, reg(ip.facing_dir)));
        set_damage(r, gobj, if big { attr::BIG_DAMAGE } else { attr::DAMAGE });
    } else {
        let owner = vars(r.game, gobj).great_fox.get();
        let j = owner.and_then(|o| super::super::super::gr::ground::jobj_of(r.game, o));
        let off = vars(r.game, gobj).gun_offset.get();
        let at = world_position(&mut r.game.objs, r.data, j, Some(off));
        item_mut(r.game, gobj).pos = at;
    }
    if vars(r.game, gobj).state_ticks.get() == 0 {
        return true;
    }
    vars(r.game, gobj).state_ticks.update(|n| n.wrapping_sub(1));
    false
}

/// The laser's physics callback, `itGreatfoxlaser_UnkMotion1_Phys`: reflected, it turns and
/// charges again in its second state.
#[doc(alias = "itGreatfoxlaser_UnkMotion1_Phys")]
pub fn physics(r: &mut Running, gobj: GObjId) {
    if vars(r.game, gobj).reflected.get() == 0 {
        return;
    }
    let ip = item_mut(r.game, gobj);
    ip.facing_dir = mem(fp::fneg(reg(ip.facing_dir)));
    let face = ip.facing_dir;
    zako::face(r, gobj, face);
    lib26b1::disable_pickup(r.game, gobj);
    let v = vars(r.game, gobj);
    v.state_ticks.set(60);
    v.reflected.set(0);
    change_state(r, gobj, 1, change::ANIM_UPDATE);
}

/// `itGreatFoxLaser_Logic27_DmgDealt`, `Clanked` and `Absorbed`: it goes on.
#[doc(alias = "itGreatFoxLaser_Logic27_DmgDealt")]
#[doc(alias = "itGreatFoxLaser_Logic27_Clanked")]
#[doc(alias = "itGreatFoxLaser_Logic27_Absorbed")]
pub fn goes_on(_r: &mut Running, _gobj: GObjId) -> bool {
    false
}

/// The laser reflected while charging, `it_2725_Logic27_Reflected`: its gun's offset moves
/// back by its reflector's distance, it stands there, and it turns at its next physics.
#[doc(alias = "it_2725_Logic27_Reflected")]
pub fn reflected(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).msid != 0 {
        return false;
    }
    let by = item(r.game, gobj).reflector.expect("a reflector");
    let at = r.game.ft_pos(by);
    let owner = vars(r.game, gobj).great_fox.get();
    let j = owner.and_then(|o| super::super::super::gr::ground::jobj_of(r.game, o));
    let off = vars(r.game, gobj).gun_offset.get();
    let laser = world_position(&mut r.game.objs, r.data, j, Some(off));
    let d = fp::fsubs(reg(laser.x), reg(at.x));
    let d = if d < 0.0 { fp::fneg(d) } else { d };
    let mut off = off;
    off.x = mem(fp::fsubs(reg(off.x), d));
    vars(r.game, gobj).gun_offset.set(off);
    let pos = world_position(&mut r.game.objs, r.data, j, Some(off));
    item_mut(r.game, gobj).pos = pos;
    let t = item(r.game, gobj)
        .bone_table
        .expect("the Great Fox's laser without bones");
    let b2 = r.game.it.bone_tables[t.0].bones[2].expect("a bone");
    jobj::set_translate(&mut r.game.objs.jobjs, b2, pos);
    vars(r.game, gobj).reflected.set(1);
    false
}
