//! Sandbag (`ft/kinds/ftSandbag`, fighter kind 32): the Home-Run Contest's bag, a fighter
//! whose knockback slows by its own attributes (the two decelerations), whose joints are held
//! together by constraints, and that lands face down into its own wait (`WaitReverse`).

use gekko_fp as fp;

use super::super::state::Running;
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::JObjId;
use crate::hsd::{jobj, robj};

/// `FTKIND_SANDBAG`.
pub const KIND_SANDBAG: i32 = 32;

/// `ftSandbagAttributes`: the knockback's x and y deceleration.
const ATTRS_SIZE: usize = 8;

/// `ftSb_MS_WaitReverse`.
const MS_WAIT_REVERSE: i32 = 0x155;

/// The parts `ftSb_Init_8014FA30` constrains (`FtPart_WaistN` and the others).
const WAIST_N: usize = 5;
const L_LEG_JA: usize = 6;
const L_LEG_J: usize = 7;
const R_LEG_J: usize = 12;
const L_SHOULDER_N: usize = 17;
const R_SHOULDER_J: usize = 37;

/// Sandbag's functions, by address.
pub mod addr {
    pub const ON_DEATH: u32 = 0x8014_F9CC;
    pub const ON_LOAD: u32 = 0x8014_F9D0;
    pub const LOAD_SPECIAL_ATTRS: u32 = 0x8014_FB78;
    pub const ON_KNOCKBACK_ENTER: u32 = 0x8014_FB9C;
    pub const ON_KNOCKBACK_EXIT: u32 = 0x8014_FBA0;
    pub const WAIT_REVERSE_ANIM: u32 = 0x8014_FC20;
    pub const WAIT_REVERSE_IASA: u32 = 0x8014_FC24;
    pub const WAIT_REVERSE_PHYS: u32 = 0x8014_FC28;
    pub const WAIT_REVERSE_COLL: u32 = 0x8014_FC48;
}

/// Gives `j` a limit on its rotation's `axis` (1 x, 2 y, 3 z) at `limit`, `lb_8000C420`.
#[doc(alias = "lb_8000C420")]
pub fn limit_rotation(objs: &mut crate::hsd::Objects, j: Option<JObjId>, axis: u32, limit: f32) {
    let r = robj::alloc(&mut objs.robjs);
    robj::set_flags(&mut objs.robjs, Some(r), axis | 0xA000_0000);
    objs.robjs[r].u = robj::Ref::Limit(limit);
    jobj::prepend_robj(objs, j, Some(r));
}

impl Running<'_> {
    /// Its spawn, `ftSb_Init_OnDeath`: nothing.
    #[doc(alias = "ftSb_Init_OnDeath")]
    pub fn sb_on_death(&mut self, _gobj: GObjId) {}

    /// Its creation, `ftSb_Init_OnLoad`: its attributes pushed, marked the bag
    /// (`is_sandbag`) and lying on its z axis (`x2226_b0`), and its joints constrained.
    #[doc(alias = "ftSb_Init_OnLoad")]
    pub fn sb_on_load(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.push_attrs(f, ATTRS_SIZE);
        let fp = &mut self.game.fighters[f.0];
        fp.is_sandbag = true;
        fp.x2226_b0 = true;
        self.sb_constrain(gobj);
    }

    /// `ftSb_Init_8014FA30`: on both of each part's joints, the waist kept at the right leg and
    /// the left shoulder, the left leg aimed at the right shoulder with its x and y turns held
    /// at -90°, and the left leg's base aimed at the waist with them held at -86°.
    #[doc(alias = "ftSb_Init_8014FA30")]
    #[doc(alias = "lb_8000C228")]
    fn sb_constrain(&mut self, gobj: GObjId) {
        use super::super::ef::eflib::constrain;
        const POSITION: u32 = 0x9000_0001;
        const DIRECTION: u32 = 0x9000_0002;
        let f = self.game.fighter_id(gobj);
        let b = self.bones(f).to_vec();
        let both = |p: usize| [b[p].joint, b[p].x4_jobj2];
        let pairs = [
            (WAIST_N, R_LEG_J, POSITION),
            (WAIST_N, L_SHOULDER_N, POSITION),
            (L_LEG_J, R_SHOULDER_J, DIRECTION),
        ];
        let objs = &mut self.game.objs;
        for (j, t, kind) in pairs {
            for (jj, tt) in both(j).into_iter().zip(both(t)) {
                constrain(
                    objs,
                    jj.expect("a constrained joint"),
                    tt.expect("a constraint joint"),
                    kind,
                );
            }
        }
        let quarter = mem(fp::frsp(fp::fdiv(fp::fneg(std::f64::consts::PI), 2.0)));
        for j in both(L_LEG_J) {
            limit_rotation(objs, j, 1, quarter);
            limit_rotation(objs, j, 2, quarter);
        }
        for (jj, tt) in both(L_LEG_JA).into_iter().zip(both(WAIST_N)) {
            constrain(
                objs,
                jj.expect("a constrained joint"),
                tt.expect("a constraint joint"),
                DIRECTION,
            );
        }
        let angle = mem(fp::fmuls(-86.0, 0.017_453_292_384_743_69));
        for j in both(L_LEG_JA) {
            limit_rotation(objs, j, 1, angle);
            limit_rotation(objs, j, 2, angle);
        }
    }

    /// `ftSb_Init_LoadSpecialAttrs`: its attributes copied again.
    #[doc(alias = "ftSb_Init_LoadSpecialAttrs")]
    pub fn sb_load_special_attrs(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.copy_attrs(f, ATTRS_SIZE);
    }

    /// Its knockback's decelerations, `ftCommon_SandbagGetKnockbackDeaccelX` and `Y` (the
    /// original asserts the fighter is the bag).
    #[doc(alias = "ftCommon_SandbagGetKnockbackDeaccelX")]
    #[doc(alias = "ftCommon_SandbagGetKnockbackDeaccelY")]
    pub fn sb_kb_deaccel(&self, gobj: GObjId) -> (f32, f32) {
        let f = self.game.fighter_id(gobj);
        assert!(
            self.game.fighters[f.0].kind == KIND_SANDBAG,
            "the original's assertion failed: fp->kind == Ft_Kind_Sandbag"
        );
        let a = self.game.fighters[f.0]
            .dat_attrs
            .expect("a fighter without attributes");
        let b = &self.game.fighter_attrs[a.0];
        let w = |o: usize| f32::from_bits(u32::from_be_bytes(b[o..o + 4].try_into().unwrap()));
        (w(0), w(4))
    }

    /// Lying face down after a knockdown, `ftSb_Init_8014FBA4`: landed, its own wait.
    // Ports ftSb_Init_8014FBA4; aliased once a check reaches it.
    pub fn sb_enter_wait_reverse(&mut self, gobj: GObjId) {
        if self.is_airborne(gobj) {
            self.land(gobj);
        }
        self.change_motion_state(gobj, MS_WAIT_REVERSE, 0, 0.0, 1.0, 0.0, None);
        let x5f0 = self.common().u32(0x5F0) as u16;
        self.game.fighter_mut(gobj).x209A = x5f0;
    }
}

/// The knockback slowed by `dec` toward zero, stopping there, `ftCommon_SandbagKnockbackDeaccel`.
#[doc(alias = "ftCommon_SandbagKnockbackDeaccel")]
pub fn kb_deaccel(kb: f32, dec: f32) -> f32 {
    let mut result = reg(kb);
    if kb > 0.0 {
        result = fp::fsubs(result, reg(dec));
        if result < 0.0 {
            return 0.0;
        }
    } else if kb < 0.0 {
        result = fp::fadds(result, reg(dec));
        if result > 0.0 {
            return 0.0;
        }
    }
    mem(result)
}

/// Runs one of Sandbag's callbacks, if `a` is one.
pub fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        // `ftSb_Init_OnKnockbackEnter`, `_OnKnockbackExit`, `ftSb_WaitReverse_Anim` and
        // `_IASA`: nothing.
        addr::ON_KNOCKBACK_ENTER
        | addr::ON_KNOCKBACK_EXIT
        | addr::WAIT_REVERSE_ANIM
        | addr::WAIT_REVERSE_IASA => {}
        addr::WAIT_REVERSE_PHYS => g.sb_wait_reverse_phys(gobj),
        addr::WAIT_REVERSE_COLL => g.sb_wait_reverse_coll(gobj),
        _ => return false,
    }
    true
}

impl Running<'_> {
    /// `ftSb_WaitReverse_Phys`: the ground's friction (`ft_80084F3C`).
    // Ports ftSb_WaitReverse_Phys; aliased once a check reaches it.
    fn sb_wait_reverse_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// `ftSb_WaitReverse_Coll`: kept on the ground, falling off its edge (`ft_80083F88`).
    // Ports ftSb_WaitReverse_Coll; aliased once a check reaches it.
    fn sb_wait_reverse_coll(&mut self, gobj: GObjId) {
        self.ground_coll_fall_off(gobj);
    }
}

/// The empty callbacks: `ftSb_Init_OnKnockbackEnter` and `ftSb_Init_OnKnockbackExit` do
/// nothing.
#[doc(alias = "ftSb_Init_OnKnockbackEnter")]
#[doc(alias = "ftSb_Init_OnKnockbackExit")]
pub fn nothing() {}

/// `ftSb_WaitReverse_Anim` and `ftSb_WaitReverse_IASA` do nothing either.
// Ports ftSb_WaitReverse_Anim; aliased once a check reaches it.
// Ports ftSb_WaitReverse_IASA; aliased once a check reaches it.
pub fn nothing_lying() {}
