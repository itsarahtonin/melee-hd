//! The enemies' common code, `melee/it/itzako.c`: spawning an enemy item, its common
//! variables (`itZako_ItemVars`, the first words of each enemy's member), its knockback and
//! its KO.

use gekko_fp as fp;

use super::item::{spawn_air, spawn_ground};
use super::{item, item_mut, owner};
use crate::fpu::{mem, reg};
use crate::game::code;
use crate::game::state::{Game, Running};
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::JObjId;
use crate::math;
use crate::vec::Vec3;

/// `it_2725_Logic9_Destroyed`, an enemy's destroy callback.
pub const DESTROYED: u32 = 0x8027_C8B0;

/// Runs the enemies' common events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        DESTROYED => destroyed(r, gobj),
        _ => return false,
    }
    true
}

/// Spawns an enemy item of `kind`, `it_8027B5B0`: at `pos`, or where `jobj` is; facing most
/// fighters; in the air with its setup (`use_init`) or on the ground; its spawner slot none.
///
/// Only the Shy Guys' variables are typed so far ([`super::kinds::heiho::HeihoVars`]), so
/// another kind stops.
#[doc(alias = "it_8027B5B0")]
pub fn spawn(
    r: &mut Running,
    kind: i32,
    pos: Option<Vec3>,
    jobj: Option<JObjId>,
    vel: Option<Vec3>,
    use_init: bool,
) -> Option<GObjId> {
    let prev = match (pos, jobj) {
        (Some(p), _) => p,
        (None, Some(j)) => {
            crate::lb::joint::world_position(&mut r.game.objs, r.data, Some(j), None)
        }
        (None, None) => panic!("can t init zako pos"),
    };
    let mut spawn = SpawnItem {
        kind,
        ..Default::default()
    };
    spawn.prev_pos = prev;
    spawn.pos = prev;
    spawn.facing_dir = owner::facing_for(r.game, r.data, prev);
    spawn.x3C_damage = 0;
    spawn.x44_flag[0] |= 0x80;
    spawn.vel = vel.unwrap_or(Vec3::new(0.0, 0.0, 0.0));
    spawn.x0_parent_gobj = None;
    spawn.x4_parent_gobj2 = None;
    spawn.x40 = 0;
    let gobj = if use_init {
        spawn_air(r, &mut spawn)
    } else {
        spawn_ground(r, &mut spawn)
    }?;
    if kind != super::kinds::HEIHO {
        code::unported_call("it_8027B5B0 (an enemy other than a Shy Guy)");
    }
    let v = item_mut(r.game, gobj).xDD4_itemVar.heiho();
    v.jobj.set(jobj);
    v.idx.set(-1);
    v.x14[2].set(0);
    item_mut(r.game, gobj).x378_itemColl.x34_flags.b1234 = 3;
    Some(gobj)
}

/// Resets an enemy's common variables, `it_8027B730`, and clears two of its flags
/// (`it_80275228`, `it_80275240`).
#[doc(alias = "it_8027B730")]
pub fn reset(game: &mut Game, gobj: GObjId) {
    let v = item_mut(game, gobj).xDD4_itemVar.heiho();
    v.jobj.set(None);
    v.idx.set(-1);
    v.x8.set(Vec3::new(0.0, 1.0, 0.0));
    for w in &mut v.x14 {
        w.set(0);
    }
    clear_x1c(game, gobj);
    clear_x1e(game, gobj);
}

/// `it_80275228`.
#[doc(alias = "it_80275228")]
pub fn clear_x1c(game: &mut Game, gobj: GObjId) {
    item_mut(game, gobj).xDC8_word.flags.x1C = false;
}

/// `it_80275240`.
#[doc(alias = "it_80275240")]
pub fn clear_x1e(game: &mut Game, gobj: GObjId) {
    item_mut(game, gobj).xDC8_word.flags.x1E = false;
}

/// An enemy's destroy callback, `it_2725_Logic9_Destroyed` (`grZakoGenerator_801CAC14`):
/// its spawner slot, if any, frees for a respawn.
#[doc(alias = "it_2725_Logic9_Destroyed")]
#[doc(alias = "grZakoGenerator_801CAC14")]
pub fn destroyed(r: &mut Running, gobj: GObjId) {
    let kind = item(r.game, gobj).kind;
    if kind == COIN {
        code::unported_call("grZakoGenerator_801CAC14 (a coin)");
    }
    let idx = item_mut(r.game, gobj).xDD4_itemVar.heiho().idx.get();
    if idx != -1 {
        code::unported_call("grZakoGenerator_801CAC14 (an enemy from a spawner slot)");
    }
}

/// `It_Kind_Coin`.
const COIN: i32 = 0xDB;

/// The knockback's launch angle (the inline `it_8027B798_CalcAngle`): the hit's own, or for
/// the Sakurai angle (361) `ftCommonData`'s in the air and on the ground a slope from 0.
fn launch_angle(r: &Running, ground_or_air: i32, angle: i32, kb: f64) -> f64 {
    let deg = |v: f64| fp::fmuls(0.017_453_292_384_743_69, v);
    if angle != 0x169 {
        return deg(fp::frsp(f64::from(angle)));
    }
    let c = r.common();
    if ground_or_air == GA_AIR {
        return reg(c.f32(0x144));
    }
    let (max, lo, hi) = (reg(c.f32(0x148)), reg(c.f32(0x14C)), reg(c.f32(0x150)));
    if kb < lo {
        return 0.0;
    }
    let t = deg(fp::fmadds(
        max,
        fp::fdivs(fp::fsubs(kb, lo), fp::fsubs(hi, lo)),
        1.0,
    ));
    if t > deg(max) { deg(max) } else { t }
}

/// `GA_Air`.
const GA_AIR: i32 = 1;

/// An enemy's knockback velocity, `it_8027B798`: the knockback scaled by `ftCommonData`'s
/// `x100` along its angle, away from the hit; on the ground, along the floor if it points
/// into it. Whether it points away from the floor.
#[doc(alias = "it_8027B798")]
pub fn knockback_vel(r: &mut Running, gobj: GObjId, out: &mut Vec3) -> bool {
    let ip = item(r.game, gobj);
    let kb = reg(ip.xCC8_knockback);
    let mag = fp::fmuls(kb, reg(r.common().f32(0x100)));
    let angle = launch_angle(r, ip.ground_or_air, ip.xCAC_angle, kb);
    let a = mem(angle);
    let dx = fp::fmuls(mag, reg(math::cos(a)));
    let dy = fp::fmuls(mag, reg(math::sin(a)));
    let ip = item_mut(r.game, gobj);
    ip.facing_dir = ip.xCCC_incDamageDirection;
    let x = mem(fp::fmuls(fp::fneg(dx), reg(ip.facing_dir)));
    if ip.ground_or_air == GA_AIR {
        *out = Vec3::new(x, mem(dy), 0.0);
        return false;
    }
    let normal = ip.x378_itemColl.floor.normal;
    let v = Vec3::new(x, mem(dy), 0.0);
    if reg(normal.angle(v)) < reg(1.570_796_4) {
        *out = Vec3::new(v.x, v.y, 0.0);
        true
    } else {
        *out = Vec3::new(
            mem(fp::fmuls(reg(normal.y), reg(v.x))),
            mem(fp::fmuls(fp::fneg(reg(normal.x)), reg(v.x))),
            0.0,
        );
        false
    }
}

/// A hit enemy's or Pokémon's launch, `it_8027B964`: off the surfaces it moves into
/// (`chk`), or along its knockback, never straight nowhere or level; then at the common data's
/// launch speed (`it_804D6D40->x4`).
#[doc(alias = "it_8027B964")]
pub fn launch(r: &mut Running, gobj: GObjId, chk: bool) {
    if chk {
        super::maplib::reflect_velocity(r.game, r.data, gobj);
    } else {
        let mut v = item(r.game, gobj).x40_vel;
        knockback_vel(r, gobj, &mut v);
        let ip = item_mut(r.game, gobj);
        ip.x40_vel = v;
        if ip.x40_vel.x == 0.0 && ip.x40_vel.y == 0.0 {
            ip.x40_vel.x = mem(fp::fmuls(
                reg(0.1f32),
                fp::fneg(reg(ip.xCCC_incDamageDirection)),
            ));
            ip.x40_vel.y = 0.1;
        }
        if ip.x40_vel.y == 0.0 {
            ip.x40_vel.y = 0.1;
        }
    }
    let mut v = item(r.game, gobj).x40_vel;
    v.normalize();
    let zako = r.game.it.zako.expect("no enemies' common data");
    let scale = reg(r.data.f32(zako.at(4)));
    v = Vec3::new(
        mem(fp::fmuls(reg(v.x), scale)),
        mem(fp::fmuls(reg(v.y), scale)),
        mem(fp::fmuls(reg(v.z), scale)),
    );
    item_mut(r.game, gobj).x40_vel = v;
}

/// An enemy KO'd, `it_8027CE44` (`grZakoGenerator_801CACB8`): its source player's KO bonus
/// (`it_8027CE18`), and its spawner slot.
#[doc(alias = "it_8027CE44")]
#[doc(alias = "grZakoGenerator_801CACB8")]
pub fn ko(r: &mut Running, gobj: GObjId) {
    let kind = item(r.game, gobj).kind;
    ko_bonus(r, gobj);
    if kind == COIN {
        code::unported_call("grZakoGenerator_801CACB8 (a coin)");
    }
    let idx = item_mut(r.game, gobj).xDD4_itemVar.heiho().idx.get();
    if idx != -1 {
        code::unported_call("grZakoGenerator_801CACB8 (an enemy from a spawner slot)");
    }
}

/// The enemy's source player's KO bonus, `it_8027CE18` (`pl_8004049C`).
#[doc(alias = "it_8027CE18")]
pub fn ko_bonus(r: &mut Running, gobj: GObjId) {
    let ip = item(r.game, gobj);
    let (player, kind) = (ip.xCB0_source_ply, ip.kind);
    r.zako_ko_bonus(player, kind);
}

impl Running<'_> {
    /// A player's bonus for KOing an enemy, `pl_8004049C`: none for no player (6).
    #[doc(alias = "pl_8004049C")]
    pub fn zako_ko_bonus(&mut self, player: i32, kind: i32) {
        if player == 6 {
            return;
        }
        assert!(
            (0..6).contains(&player),
            "zako ko player illegal ! :{player}"
        );
        let bonus = match kind {
            super::kinds::HEIHO => 0xD6,
            _ => code::unported_call("pl_8004049C (an enemy other than a Shy Guy)"),
        };
        let points = bonus_points(bonus);
        self.game.vs_add_score(player, points);
        self.count_bonus(player, bonus as usize);
    }
}

/// A point bonus's points, `gm_8016F2F8`: the bonus's entry in `lbl_803D5A4C` names its
/// points in `lbl_803D5648`. Transcribed for the enemies' KO bonuses, those `pl_8004049C`
/// counts (`main.dol`'s tables: entries 240 to 248).
#[doc(alias = "gm_8016F2F8")]
pub fn bonus_points(bonus: i32) -> i32 {
    match bonus {
        0xCE => 100,
        0xCF => 200,
        0xD0 | 0xD1 => 300,
        0xD2 => 500,
        0xD3 => 150,
        0xD4 => 200,
        0xD5 => 800,
        0xD6 => 20,
        _ => code::unported_call("gm_8016F2F8 (a bonus besides the enemies' KOs)"),
    }
}

impl Game {
    /// Adds to the first player's score in the VS scene's state (`unk_2E`), capped to 16
    /// bits, `gm_8016B6E8`; other players' scores aren't kept.
    #[doc(alias = "gm_8016B6E8")]
    pub fn vs_add_score(&mut self, player: i32, points: i32) {
        if player == 0 {
            let s = &mut self.vs.state;
            s.unk_2E = time_add(u32::from(s.unk_2E), points) as u16;
        }
    }
}

/// A 16-bit counter's sum, clamped to 0 and 0xFFFF in the low half, `lbTime_8000AF24`.
#[doc(alias = "lbTime_8000AF24")]
pub fn time_add(a: u32, b: i32) -> u32 {
    if b > 0 {
        if 0xFFFF_u32.wrapping_sub(a & 0xFFFF) > b as u32 {
            a.wrapping_add(b as u32)
        } else {
            0xFFFF
        }
    } else if (a & 0xFFFF) as i32 > b.wrapping_neg() {
        a.wrapping_add(b as u32)
    } else {
        0
    }
}
