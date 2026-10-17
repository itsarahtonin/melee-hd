//! The mushrooms' states (`ftCo_KinokoGiantStart.c`, `ftCo_KinokoGiantEnd.c`,
//! `ftCo_KinokoSmallStart.c`, `ftCo_KinokoSmallEnd.c`, `ft_0D27.c`): a Super Mushroom grows
//! the fighter and a Poison Mushroom shrinks it, each through a state that scales it along the
//! mushroom's scale animation (`x2184`, a joint of its own animated by the Super Mushroom's
//! data), and shrinks or grows it back when its time runs out (`Fighter_procAnim`). The state
//! keeps the fighter's speeds while it scales (`mv.co.common`), the state to fall into
//! (`mv.co.mushroom.x0`) and its own end (`x4`), which a hit also runs.

use gekko_fp as fp;

use super::super::super::handles::CodeRef;
use super::super::super::it::{self, kinds};
use super::super::super::state::Running;
use super::super::FighterId;
use super::ids::ms;
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::{aobj, jobj};

/// `ftCommonData`'s mushroom fields: the frames a mushroom lasts (x688), the percent past
/// which a Super Mushroom lasts longer (x68C) and at most how much longer (x690), and the
/// scale targets, grown from normal or small (x678, x67C) and shrunk from normal or big (x680,
/// x684).
mod cd {
    pub const FRAMES: u32 = 0x688;
    pub const PERCENT_FROM: u32 = 0x68C;
    pub const PERCENT_MAX: u32 = 0x690;
    pub const GIANT: u32 = 0x678;
    pub const GIANT_FROM_SMALL: u32 = 0x67C;
    pub const SMALL: u32 = 0x680;
    pub const SMALL_FROM_GIANT: u32 = 0x684;
}

/// `mv.co.mushroom`: the state to fall into, the state's end, and whether the end starts
/// over the fighter's intangibility.
const MV_FALL: usize = 0x0;
const MV_END: usize = 0x4;
const MV_X8: usize = 0x8;
/// `mv.co.walk.middle_anim_frame` and `fast_anim_frame`: the scale it starts from and the
/// one it ends at.
const MV_SCALE_FROM: usize = 0xC;
const MV_SCALE_TO: usize = 0x10;
/// `mv.co.common`'s saved speeds.
mod mvc {
    pub const GR_ACCEL_1: usize = 0x14;
    pub const GR_ACCEL_2: usize = 0x18;
    pub const GR_VEL: usize = 0x1C;
    pub const GR_KB_VEL: usize = 0x20;
    pub const GR_SHIELD_KB_VEL: usize = 0x24;
    pub const SELF_ACCEL: usize = 0x28;
    pub const SELF_VEL: usize = 0x34;
    pub const KB_VEL: usize = 0x40;
    pub const SHIELD_KB: usize = 0x4C;
    pub const UNK_VEL: usize = 0x58;
}

/// The callbacks the states and their variables point to, by address.
pub mod addr {
    pub const FALL_ENTER: u32 = 0x800C_C730;
    pub const FALL_AERIAL_ENTER: u32 = 0x800C_CDA8;
    pub const FALL_SPECIAL_ENTER: u32 = 0x8009_68C8;
    pub const GIANT_START_DONE: u32 = 0x800D_15D0;
    pub const GIANT_START_LANDED: u32 = 0x800D_17FC;
    pub const GIANT_START_ANIM: u32 = 0x800D_1868;
    pub const GIANT_START_COLL: u32 = 0x800D_18A4;
    pub const GIANT_START_FELL: u32 = 0x800D_19BC;
    pub const GIANT_START_AIR_ANIM: u32 = 0x800D_1A28;
    pub const GIANT_START_AIR_COLL: u32 = 0x800D_1A64;
    pub const GIANT_END_DONE: u32 = 0x800D_1B04;
    pub const GIANT_END_LANDED: u32 = 0x800D_1C40;
    pub const GIANT_END_ANIM: u32 = 0x800D_1CAC;
    pub const GIANT_END_COLL: u32 = 0x800D_1CE8;
    pub const GIANT_END_FELL: u32 = 0x800D_1DB0;
    pub const GIANT_END_AIR_ANIM: u32 = 0x800D_1E1C;
    pub const GIANT_END_AIR_COLL: u32 = 0x800D_1E58;
    pub const SMALL_START_DONE: u32 = 0x800D_1F6C;
    pub const SMALL_START_LANDED: u32 = 0x800D_20EC;
    pub const SMALL_START_ANIM: u32 = 0x800D_2158;
    pub const SMALL_START_COLL: u32 = 0x800D_2194;
    pub const SMALL_START_FELL: u32 = 0x800D_22AC;
    pub const SMALL_START_AIR_ANIM: u32 = 0x800D_2318;
    pub const SMALL_START_AIR_COLL: u32 = 0x800D_2354;
    pub const SMALL_END_DONE: u32 = 0x800D_23F4;
    pub const SMALL_END_LANDED: u32 = 0x800D_2530;
    pub const SMALL_END_ANIM: u32 = 0x800D_259C;
    pub const SMALL_END_COLL: u32 = 0x800D_25D8;
    pub const SMALL_END_FELL: u32 = 0x800D_26A0;
    pub const SMALL_END_AIR_ANIM: u32 = 0x800D_270C;
    pub const SMALL_END_AIR_COLL: u32 = 0x800D_2748;
}

/// The descriptor `fn_800D299C` loads the scale joint from (`lbl_803C6FF0`, in the
/// executable's data): flags 8, no contents, identity transform.
const SCALE_JOINT_DESC: u32 = 0x803C_6FF0;

/// Runs one of the mushrooms' callbacks, if `cb` is one.
pub fn run(g: &mut Running, gobj: GObjId, cb: u32) -> bool {
    use addr::*;
    match cb {
        GIANT_START_DONE => g.giant_start_done(gobj),
        GIANT_END_DONE => g.giant_end_done(gobj),
        SMALL_START_DONE => g.small_start_done(gobj),
        SMALL_END_DONE => g.small_end_done(gobj),
        GIANT_START_ANIM | GIANT_START_AIR_ANIM => g.kinoko_anim(gobj, GIANT_START_DONE),
        GIANT_END_ANIM | GIANT_END_AIR_ANIM => g.kinoko_anim(gobj, GIANT_END_DONE),
        SMALL_START_ANIM | SMALL_START_AIR_ANIM => g.kinoko_anim(gobj, SMALL_START_DONE),
        SMALL_END_ANIM | SMALL_END_AIR_ANIM => g.kinoko_anim(gobj, SMALL_END_DONE),
        GIANT_START_COLL => g.kinoko_ground_coll(gobj, GIANT_START),
        GIANT_END_COLL => g.kinoko_ground_coll(gobj, GIANT_END),
        SMALL_START_COLL => g.kinoko_ground_coll(gobj, SMALL_START),
        SMALL_END_COLL => g.kinoko_ground_coll(gobj, SMALL_END),
        GIANT_START_AIR_COLL => g.kinoko_air_coll(gobj, GIANT_START),
        GIANT_END_AIR_COLL => g.kinoko_air_coll(gobj, GIANT_END),
        SMALL_START_AIR_COLL => g.kinoko_air_coll(gobj, SMALL_START),
        SMALL_END_AIR_COLL => g.kinoko_air_coll(gobj, SMALL_END),
        GIANT_START_FELL => g.kinoko_to_air(gobj, GIANT_START),
        GIANT_END_FELL => g.kinoko_to_air(gobj, GIANT_END),
        SMALL_START_FELL => g.kinoko_to_air(gobj, SMALL_START),
        SMALL_END_FELL => g.kinoko_to_air(gobj, SMALL_END),
        GIANT_START_LANDED => g.kinoko_to_ground(gobj, GIANT_START),
        GIANT_END_LANDED => g.kinoko_to_ground(gobj, GIANT_END),
        SMALL_START_LANDED => g.kinoko_to_ground(gobj, SMALL_START),
        SMALL_END_LANDED => g.kinoko_to_ground(gobj, SMALL_END),
        _ => return false,
    }
    true
}

/// Each mushroom state's ground and air versions, its flags into it, and its end.
#[derive(Clone, Copy)]
struct Kinoko {
    ground: i32,
    air: i32,
    end: u32,
}

const GIANT_START: Kinoko = Kinoko {
    ground: ms::KINOKO_GIANT_START,
    air: ms::KINOKO_GIANT_START_AIR,
    end: addr::GIANT_START_DONE,
};
const GIANT_END: Kinoko = Kinoko {
    ground: ms::KINOKO_GIANT_END,
    air: ms::KINOKO_GIANT_END_AIR,
    end: addr::GIANT_END_DONE,
};
const SMALL_START: Kinoko = Kinoko {
    ground: ms::KINOKO_SMALL_START,
    air: ms::KINOKO_SMALL_START_AIR,
    end: addr::SMALL_START_DONE,
};
const SMALL_END: Kinoko = Kinoko {
    ground: ms::KINOKO_SMALL_END,
    air: ms::KINOKO_SMALL_END_AIR,
    end: addr::SMALL_END_DONE,
};

/// The flags into a mushroom state from another (`0x90`, an end's `0x91`), and between its
/// ground and air versions (`0xC4C5080`, `0xC4C5081`).
const ENTER_FLAGS: u32 = 0x90;
const SWITCH_FLAGS: u32 = 0xC4C_5080;

impl Running<'_> {
    /// Whether the fighter is in a mushroom state.
    pub fn in_kinoko_state(&self, gobj: GObjId) -> bool {
        (ms::KINOKO_GIANT_START..=ms::KINOKO_SMALL_END_AIR)
            .contains(&self.game.fighter(gobj).motion_id)
    }

    /// Runs one of the callbacks the state's variables hold.
    fn run_kinoko_cb(&mut self, gobj: GObjId, cb: u32) {
        match cb {
            addr::FALL_ENTER => self.enter_fall(gobj),
            addr::FALL_AERIAL_ENTER => self.enter_fall_aerial(gobj),
            addr::FALL_SPECIAL_ENTER => self.enter_fall_special_basic(gobj),
            other => {
                assert!(run(self, gobj, other), "a mushroom callback {other:#010x}");
            }
        }
    }

    /// A Super Mushroom taken, `Fighter_SuperMushroomApply` (whether it started a state):
    /// not while frozen or in throws 12 and 13; a mushroom state under way ends at once; a
    /// grown fighter's time starts over; a shrunk one grows back; else it grows.
    #[doc(alias = "Fighter_SuperMushroomApply")]
    pub fn apply_super_mushroom(&mut self, gobj: GObjId) -> bool {
        self.apply_mushroom(gobj, true)
    }

    /// A Poison Mushroom taken, `Fighter_PoisonMushroomApply`: the Super Mushroom's, the other
    /// way round.
    #[doc(alias = "Fighter_PoisonMushroomApply")]
    pub fn apply_poison_mushroom(&mut self, gobj: GObjId) -> bool {
        self.apply_mushroom(gobj, false)
    }

    fn apply_mushroom(&mut self, gobj: GObjId, giant: bool) -> bool {
        if !self.can_take_mushroom(gobj) {
            return false;
        }
        if self.in_kinoko_state(gobj) {
            let end = self.game.fighter(gobj).mv_i32(MV_END) as u32;
            self.run_kinoko_cb(gobj, end);
        }
        let frames = self.common().i32(cd::FRAMES);
        let fp = self.game.fighter_mut(gobj);
        let (same, other) = if giant {
            (fp.x2220_b5, fp.x2220_b6)
        } else {
            (fp.x2220_b6, fp.x2220_b5)
        };
        if same {
            fp.x2008 = frames;
            return false;
        }
        let state = match (other, giant) {
            (true, true) => SMALL_END,
            (true, false) => GIANT_END,
            (false, true) => GIANT_START,
            (false, false) => SMALL_START,
        };
        let air = self.game.fighter(gobj).ground_or_air != 0;
        if other {
            self.enter_kinoko_end(gobj, state, air, true);
        } else {
            self.enter_kinoko_start(gobj, state, air);
        }
        true
    }

    /// A Super Mushroom's time out, `Fighter_SuperMushroomEnd`: unless it can't change now,
    /// back to its size.
    #[doc(alias = "Fighter_SuperMushroomEnd")]
    pub fn super_mushroom_end(&mut self, gobj: GObjId) -> bool {
        self.end_mushroom(gobj, GIANT_END)
    }

    /// A Poison Mushroom's time out, `Fighter_PoisonMushroomEnd`.
    #[doc(alias = "Fighter_PoisonMushroomEnd")]
    pub fn poison_mushroom_end(&mut self, gobj: GObjId) -> bool {
        self.end_mushroom(gobj, SMALL_END)
    }

    fn end_mushroom(&mut self, gobj: GObjId, k: Kinoko) -> bool {
        if !self.can_end_mushroom(gobj) {
            return false;
        }
        let air = self.game.fighter(gobj).ground_or_air != 0;
        self.enter_kinoko_end(gobj, k, air, false);
        true
    }

    /// Whether a mushroom takes, `ftCo_800D27C4`: not frozen, and not in throws 12 or 13.
    #[doc(alias = "ftCo_800D27C4")]
    pub fn can_take_mushroom(&self, gobj: GObjId) -> bool {
        let fp = self.game.fighter(gobj);
        if fp.motion_id == ms::DAMAGE_ICE {
            return false;
        }
        !matches!(fp.x2070[1] >> 4, 12 | 13)
    }

    /// Whether the mushroom's end can start now, `fn_800D2818`: not frozen, not in a mushroom
    /// state, and in a throw kind 0 or 5 or one with `x2071_b4`.
    #[doc(alias = "fn_800D2818")]
    pub fn can_end_mushroom(&self, gobj: GObjId) -> bool {
        let fp = self.game.fighter(gobj);
        if fp.motion_id == ms::DAMAGE_ICE || self.in_kinoko_state(gobj) {
            return false;
        }
        let kind = fp.x2070[1] >> 4;
        kind == 5 || (kind < 5 && kind == 0) || fp.x2070[1] & 0x08 != 0
    }

    /// Growing on the ground, `ftCo_800D170C`.
    #[doc(alias = "ftCo_800D170C")]
    pub fn enter_giant_start(&mut self, gobj: GObjId) {
        self.enter_kinoko_start(gobj, GIANT_START, false);
    }

    /// Growing in the air, `ftCo_800D18CC`.
    #[doc(alias = "ftCo_800D18CC")]
    pub fn enter_giant_start_air(&mut self, gobj: GObjId) {
        self.enter_kinoko_start(gobj, GIANT_START, true);
    }

    /// Shrinking on the ground, `ftCo_800D1FFC`.
    #[doc(alias = "ftCo_800D1FFC")]
    pub fn enter_small_start(&mut self, gobj: GObjId) {
        self.enter_kinoko_start(gobj, SMALL_START, false);
    }

    /// Shrinking in the air, `ftCo_800D21BC`.
    #[doc(alias = "ftCo_800D21BC")]
    pub fn enter_small_start_air(&mut self, gobj: GObjId) {
        self.enter_kinoko_start(gobj, SMALL_START, true);
    }

    /// Shrinking back from big on the ground, `ftCo_800D1BA0`: `x8` whether its end starts
    /// the intangibility over.
    #[doc(alias = "ftCo_800D1BA0")]
    pub fn enter_giant_end(&mut self, gobj: GObjId, x8: bool) {
        self.enter_kinoko_end(gobj, GIANT_END, false, x8);
    }

    /// Shrinking back from big in the air, `ftCo_800D1D10`.
    #[doc(alias = "ftCo_800D1D10")]
    pub fn enter_giant_end_air(&mut self, gobj: GObjId, x8: bool) {
        self.enter_kinoko_end(gobj, GIANT_END, true, x8);
    }

    /// Growing back from small on the ground, `ftCo_800D2490`.
    #[doc(alias = "ftCo_800D2490")]
    pub fn enter_small_end(&mut self, gobj: GObjId, x8: bool) {
        self.enter_kinoko_end(gobj, SMALL_END, false, x8);
    }

    /// Growing back from small in the air, `ftCo_800D2600`.
    #[doc(alias = "ftCo_800D2600")]
    pub fn enter_small_end_air(&mut self, gobj: GObjId, x8: bool) {
        self.enter_kinoko_end(gobj, SMALL_END, true, x8);
    }

    /// Into a growth or a shrink, its ground or air version (`ftCo_800D170C` and the like).
    fn enter_kinoko_start(&mut self, gobj: GObjId, k: Kinoko, air: bool) {
        let prev = self.game.fighter(gobj).motion_id;
        self.kinoko_let_go(gobj);
        let msid = if air { k.air } else { k.ground };
        self.change_motion_state(gobj, msid, ENTER_FLAGS, 0.0, 1.0, 0.0, None);
        self.save_kinoko_speeds(gobj);
        self.set_kinoko_fall(gobj, prev);
        let c = self.common();
        let fp = self.game.fighter(gobj);
        let giant = k.end == addr::GIANT_START_DONE;
        let target = if giant {
            if reg(fp.x34_scale.x) > 1.0 {
                c.f32(cd::GIANT_FROM_SMALL)
            } else {
                c.f32(cd::GIANT)
            }
        } else if reg(fp.x34_scale.x) < 1.0 {
            c.f32(cd::SMALL_FROM_GIANT)
        } else {
            c.f32(cd::SMALL)
        };
        let from = fp.x34_scale.y;
        let to = mem(fp::fmuls(reg(from), reg(target)));
        self.start_kinoko_scale(gobj, 0, from, to);
        let f = self.game.fighter_id(gobj);
        if giant {
            self.kinoko_grow_sfx(f);
        } else {
            self.kinoko_shrink_sfx(f);
        }
        self.start_rumble(f, 12, 0);
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(MV_END, k.end as i32);
        fp.take_dmg_cb = Some(CodeRef(k.end));
    }

    /// Into a mushroom's end, its ground or air version (`ftCo_800D1BA0` and the like).
    fn enter_kinoko_end(&mut self, gobj: GObjId, k: Kinoko, air: bool, x8: bool) {
        let prev = self.game.fighter(gobj).motion_id;
        self.kinoko_let_go(gobj);
        let msid = if air { k.air } else { k.ground };
        self.change_motion_state(gobj, msid, ENTER_FLAGS | 1, 0.0, 1.0, 0.0, None);
        self.save_kinoko_speeds(gobj);
        self.set_kinoko_fall(gobj, prev);
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(MV_X8, i32::from(x8));
        let (from, to) = (fp.x34_scale.y, fp.x34_scale.x);
        let giant = k.end == addr::GIANT_END_DONE;
        self.start_kinoko_scale(gobj, i32::from(giant), from, to);
        let f = self.game.fighter_id(gobj);
        if giant {
            self.kinoko_shrink_sfx(f);
        } else {
            self.kinoko_grow_sfx(f);
        }
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(MV_END, k.end as i32);
        fp.take_dmg_cb = Some(CodeRef(k.end));
    }

    /// The growth's sound, `fn_800D2AD8` (its id is `ftCo_MS_CaptureDamageKoopaAir`'s
    /// number).
    #[doc(alias = "fn_800D2AD8")]
    pub fn kinoko_grow_sfx(&mut self, f: FighterId) {
        self.fighter_play_sfx(f, ms::CAPTURE_DAMAGE_KOOPA_AIR, 0x7F, 0x40);
    }

    /// The shrink's sound, `fn_800D2B04` (`ftCo_MS_CaptureWaitKoopaAir`'s number).
    #[doc(alias = "fn_800D2B04")]
    pub fn kinoko_shrink_sfx(&mut self, f: FighterId) {
        self.fighter_play_sfx(f, ms::CAPTURE_WAIT_KOOPA_AIR, 0x7F, 0x40);
    }

    /// The growth's end, `ftCo_800D15D0`: the speeds back, grown for the mushroom's frames
    /// (and the percent past `x68C`, up to `x690`, more), the new scale's attributes, out of
    /// its intangibility, then waiting or falling.
    #[doc(alias = "ftCo_800D15D0")]
    pub fn giant_start_done(&mut self, gobj: GObjId) {
        self.restore_kinoko_speeds(gobj);
        let c = self.common();
        let (frames, from, max) = (
            c.i32(cd::FRAMES),
            fp::frsp(f64::from(c.i32(cd::PERCENT_FROM))),
            fp::frsp(f64::from(c.i32(cd::PERCENT_MAX))),
        );
        let fp = self.game.fighter_mut(gobj);
        fp.x2220_b5 = true;
        fp.x2008 = frames;
        let mut percent = reg(fp.dmg.x1830_percent);
        if percent > from {
            percent = fp::fsubs(percent, from);
            if percent >= max {
                percent = max;
            }
            fp.x2008 = fp::fctiwz(fp::fadds(fp::frsp(f64::from(fp.x2008)), percent));
        }
        let to = fp.mv_f32(MV_SCALE_TO);
        self.kinoko_done(gobj, to, false);
    }

    /// The shrink's end, `ftCo_800D1F6C`: as the growth's, without the percent.
    #[doc(alias = "ftCo_800D1F6C")]
    pub fn small_start_done(&mut self, gobj: GObjId) {
        self.restore_kinoko_speeds(gobj);
        let frames = self.common().i32(cd::FRAMES);
        let fp = self.game.fighter_mut(gobj);
        fp.x2220_b6 = true;
        fp.x2008 = frames;
        let to = fp.mv_f32(MV_SCALE_TO);
        self.kinoko_done(gobj, to, false);
    }

    /// The growth's end back to size, `ftCo_800D1B04`.
    #[doc(alias = "ftCo_800D1B04")]
    pub fn giant_end_done(&mut self, gobj: GObjId) {
        self.restore_kinoko_speeds(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.x2220_b5 = false;
        let to = fp.x34_scale.x;
        self.kinoko_done(gobj, to, true);
    }

    /// The shrink's end back to size, `ftCo_800D23F4`.
    #[doc(alias = "ftCo_800D23F4")]
    pub fn small_end_done(&mut self, gobj: GObjId) {
        self.restore_kinoko_speeds(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.x2220_b6 = false;
        let to = fp.x34_scale.x;
        self.kinoko_done(gobj, to, true);
    }

    /// The common end of the mushroom states: the new scale (`ftCo_800D2770`), out of the
    /// intangibility (an end's only when its `x8` says), then waiting or the fall it keeps.
    fn kinoko_done(&mut self, gobj: GObjId, scale: f32, end: bool) {
        self.set_kinoko_scale(gobj, scale);
        if !end {
            self.stop_dead(gobj);
        } else if self.game.fighter(gobj).mv_i32(MV_X8) != 0 {
            self.stop_dead(gobj);
            self.game.fighter_mut(gobj).set_mv_i32(MV_X8, 0);
        }
        if self.game.fighter(gobj).ground_or_air == 0 {
            self.enter_wait_blend(gobj, -1.0);
            return;
        }
        let fall = self.game.fighter(gobj).mv_i32(MV_FALL) as u32;
        self.run_kinoko_cb(gobj, fall);
    }

    /// The fighter at height scale `scale`, `ftCo_800D2770`: its attributes, camera box and
    /// ledge snap with it.
    #[doc(alias = "ftCo_800D2770")]
    pub fn set_kinoko_scale(&mut self, gobj: GObjId, scale: f32) {
        self.set_scale_y(gobj, scale);
        self.calc_attributes(gobj);
        let f = self.game.fighter_id(gobj);
        self.init_camera_box(f);
        let s = self.game.fighters[f.0].x34_scale.y;
        self.scale_ledge_snap(gobj, s);
    }

    /// The state to fall into when the mushroom state ends in the air, `fn_800D2890`: Peach's
    /// float (motion 0x172) and the helpless falls into a helpless fall, an aerial fall into
    /// one, else a fall.
    #[doc(alias = "fn_800D2890")]
    pub fn set_kinoko_fall(&mut self, gobj: GObjId, prev: i32) {
        let fp = self.game.fighter_mut(gobj);
        let fall = if fp.kind == super::super::peach::KIND_PEACH && prev == 0x172 {
            addr::FALL_SPECIAL_ENTER
        } else {
            match prev {
                ms::FALL_AERIAL => addr::FALL_AERIAL_ENTER,
                ms::FALL_SPECIAL | ms::ITEM_PARASOL_FALL_SPECIAL => addr::FALL_SPECIAL_ENTER,
                _ => addr::FALL_ENTER,
            }
        };
        fp.set_mv_i32(MV_FALL, fall as i32);
    }

    /// Lets go of what the fighter does before a mushroom state, `fn_800D290C`: its damage
    /// hooks, a heavy item, and a held fighter (who's cut free, or with a grab the fighter's
    /// own hold cut and the victim let go).
    #[doc(alias = "fn_800D290C")]
    pub fn kinoko_let_go(&mut self, gobj: GObjId) {
        self.damage_hooks(gobj);
        self.drop_heavy_item(gobj);
        let fp = self.game.fighter(gobj);
        let Some(victim) = fp.victim_gobj else {
            return;
        };
        if fp.x221B.x221B_b5 {
            self.enter_capture_cut(victim);
        } else {
            self.enter_capture_cut(gobj);
            self.enter_catch_cut(victim, false);
        }
        self.damage_hooks(victim);
    }

    /// The scale joint animated from `from` to `to` along the Super Mushroom's scale animation
    /// `idx` (0 growing, 1 shrinking), `fn_800D299C`: the joint made the first time.
    #[doc(alias = "fn_800D299C")]
    pub fn start_kinoko_scale(&mut self, gobj: GObjId, idx: i32, from: f32, to: f32) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].x2184.is_none() {
            let j = self.load_kinoko_scale_joint();
            self.game.fighters[f.0].x2184 = Some(j);
        }
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_f32(MV_SCALE_FROM, reg(from));
        fp.set_mv_f32(MV_SCALE_TO, reg(to));
        let j = fp.x2184;
        let anim = self.kinoko_scale_anim(idx);
        jobj::add_anim(&mut self.game.objs, self.data, j, anim, None, None);
        self.req_joint_anim(j, 0.0);
        jobj::anim(
            &mut self.game.objs,
            self.data,
            &mut crate::game::code::ParticleHooks,
            j,
        );
    }

    /// The Super Mushroom's scale animation `idx` (0 growing, 1 shrinking), `it_80293660`: its
    /// attributes' entry `idx + 2`.
    #[doc(alias = "it_80293660")]
    pub fn kinoko_scale_anim(&self, idx: i32) -> Option<melee_data::archive::DataRef> {
        let art = it::item::article_of(self.game, self.data, kinds::KINOKO)
            .expect("the Super Mushroom's article");
        let attrs = it::data::Article(art)
            .special(self.data)
            .expect("the Super Mushroom's attributes");
        self.data.ptr(attrs.at(4 * (idx as u32 + 2)))
    }

    /// A joint's animation set to `frame`, `ftAnim_80070734` (`HSD_JObjReqAnimByFlags` with
    /// the joint's own flag).
    #[doc(alias = "ftAnim_80070734")]
    pub fn req_joint_anim(&mut self, j: Option<jobj::JObjId>, frame: f32) {
        jobj::req_anim_by_flags(&mut self.game.objs, j, 1, frame);
    }

    /// `HSD_JObjLoadJoint` on the executable's `lbl_803C6FF0`: a joint with flags 8 and
    /// nothing in it, keyed in the ID table by the descriptor's address.
    fn load_kinoko_scale_joint(&mut self) -> jobj::JObjId {
        let objs = &mut self.game.objs;
        let j = jobj::alloc(objs);
        let o = &mut objs.jobjs[j];
        o.flags |= 8;
        o.rotate.x = 0.0;
        o.rotate.y = 0.0;
        o.rotate.z = 0.0;
        o.scale = crate::vec::Vec3::new(1.0, 1.0, 1.0);
        o.translate = crate::vec::Vec3::new(0.0, 0.0, 0.0);
        o.mtx = crate::mtx::Mtx34::IDENTITY;
        o.scl = None;
        let key = crate::hsd::id::address_key(SCALE_JOINT_DESC);
        objs.ids.insert(key, j);
        objs.jobjs[j].id = Some(key);
        j
    }

    /// One frame of the scale animation, `fn_800D2A3C`: the fighter's scale between where it
    /// started and its target by the joint's x scale; whether the animation runs on.
    #[doc(alias = "fn_800D2A3C")]
    pub fn kinoko_scale_frame(&mut self, gobj: GObjId) -> bool {
        let f = self.game.fighter_id(gobj);
        let j = self.game.fighters[f.0].x2184;
        jobj::anim(
            &mut self.game.objs,
            self.data,
            &mut crate::game::code::ParticleHooks,
            j,
        );
        let j = j.expect("no mushroom scale joint");
        let scale = reg(self.game.objs.jobjs[j].scale.x);
        let fp = &self.game.fighters[f.0];
        let (from, to) = (reg(fp.mv_f32(MV_SCALE_FROM)), reg(fp.mv_f32(MV_SCALE_TO)));
        self.set_scale_y(gobj, mem(fp::fmadds(scale, fp::fsubs(to, from), from)));
        let a = self.game.objs.jobjs[j].aobj;
        aobj::flags(&self.game.objs.aobjs, a) & 0x4000_0000 == 0
    }

    /// The mushroom states' animation, `ftCo_Kinoko_Anim`: the scale's frame, and the state's
    /// end `done` once the animation's over.
    #[doc(alias = "ftCo_KinokoGiantStart_Anim")]
    #[doc(alias = "ftCo_KinokoGiantStartAir_Anim")]
    #[doc(alias = "ftCo_KinokoGiantEnd_Anim")]
    #[doc(alias = "ftCo_KinokoGiantEndAir_Anim")]
    #[doc(alias = "ftCo_KinokoSmallStart_Anim")]
    #[doc(alias = "ftCo_KinokoSmallStartAir_Anim")]
    #[doc(alias = "ftCo_KinokoSmallEnd_Anim")]
    #[doc(alias = "ftCo_KinokoSmallEndAir_Anim")]
    fn kinoko_anim(&mut self, gobj: GObjId, done: u32) {
        if !self.kinoko_scale_frame(gobj) {
            self.run_kinoko_cb(gobj, done);
        }
    }

    /// The grounded states' collision, off a ledge into the air version (`ft_8008403C`).
    #[doc(alias = "ftCo_KinokoGiantStart_Coll")]
    #[doc(alias = "ftCo_KinokoGiantEnd_Coll")]
    #[doc(alias = "ftCo_KinokoSmallStart_Coll")]
    #[doc(alias = "ftCo_KinokoSmallEnd_Coll")]
    fn kinoko_ground_coll(&mut self, gobj: GObjId, k: Kinoko) {
        let cb: super::super::coll::Cb = match k.end {
            addr::GIANT_START_DONE => |g, gobj| g.kinoko_to_air(gobj, GIANT_START),
            addr::GIANT_END_DONE => |g, gobj| g.kinoko_to_air(gobj, GIANT_END),
            addr::SMALL_START_DONE => |g, gobj| g.kinoko_to_air(gobj, SMALL_START),
            _ => |g, gobj| g.kinoko_to_air(gobj, SMALL_END),
        };
        self.ground_coll_fall_off_then(gobj, cb);
    }

    /// The air states' collision, landing into the ground version (`ft_80082C74`).
    #[doc(alias = "ftCo_KinokoGiantStartAir_Coll")]
    #[doc(alias = "ftCo_KinokoGiantEndAir_Coll")]
    #[doc(alias = "ftCo_KinokoSmallStartAir_Coll")]
    #[doc(alias = "ftCo_KinokoSmallEndAir_Coll")]
    fn kinoko_air_coll(&mut self, gobj: GObjId, k: Kinoko) {
        let cb: super::super::coll::Cb = match k.end {
            addr::GIANT_START_DONE => |g, gobj| g.kinoko_to_ground(gobj, GIANT_START),
            addr::GIANT_END_DONE => |g, gobj| g.kinoko_to_ground(gobj, GIANT_END),
            addr::SMALL_START_DONE => |g, gobj| g.kinoko_to_ground(gobj, SMALL_START),
            _ => |g, gobj| g.kinoko_to_ground(gobj, SMALL_END),
        };
        self.air_coll_then(gobj, cb);
    }

    /// Off the ground in a mushroom state, `fn_800D19BC` (and the others' `fn_800D1DB0`,
    /// `fn_800D22AC`, `fn_800D26A0`): its saved ground speed and vertical accel gone
    /// (`fn_800D2CD4`), the air version at the same frame.
    #[doc(alias = "fn_800D19BC")]
    #[doc(alias = "fn_800D1DB0")]
    #[doc(alias = "fn_800D22AC")]
    #[doc(alias = "fn_800D26A0")]
    fn kinoko_to_air(&mut self, gobj: GObjId, k: Kinoko) {
        let f = self.game.fighter_id(gobj);
        self.kinoko_airborne(f);
        self.kinoko_switch(gobj, k.air, k);
    }

    /// Airborne in a mushroom state, its saved ground speed and vertical accel gone,
    /// `fn_800D2CD4`.
    #[doc(alias = "fn_800D2CD4")]
    pub fn kinoko_airborne(&mut self, f: FighterId) {
        self.go_airborne(f);
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_f32(mvc::GR_VEL, 0.0);
        fp.set_mv_f32(mvc::SELF_ACCEL + 4, 0.0);
    }

    /// Landed in a mushroom state, its saved ground speed from its saved air speed,
    /// `fn_800D2CA0`.
    #[doc(alias = "fn_800D2CA0")]
    pub fn kinoko_landed(&mut self, gobj: GObjId) {
        self.land(gobj);
        let fp = self.game.fighter_mut(gobj);
        let vx = fp.mv_i32(mvc::SELF_VEL);
        fp.set_mv_i32(mvc::GR_VEL, vx);
    }

    /// Landing in a mushroom state, `fn_800D17FC` (and the others' `fn_800D1C40`,
    /// `fn_800D20EC`, `fn_800D2530`): landed, its saved ground speed from its saved air speed
    /// (`fn_800D2CA0`), the ground version at the same frame.
    #[doc(alias = "fn_800D17FC")]
    #[doc(alias = "fn_800D1C40")]
    #[doc(alias = "fn_800D20EC")]
    #[doc(alias = "fn_800D2530")]
    fn kinoko_to_ground(&mut self, gobj: GObjId, k: Kinoko) {
        self.kinoko_landed(gobj);
        self.kinoko_switch(gobj, k.ground, k);
    }

    /// Into the state's other version at the same frame, its end as the damage hook.
    fn kinoko_switch(&mut self, gobj: GObjId, msid: i32, k: Kinoko) {
        let end = k.end == addr::GIANT_END_DONE || k.end == addr::SMALL_END_DONE;
        let fp = self.game.fighter(gobj);
        let (frame, rate) = (fp.cur_anim_frame, fp.frame_speed_mul);
        let flags = SWITCH_FLAGS | u32::from(end);
        self.change_motion_state(gobj, msid, flags, frame, rate, 0.0, None);
        self.game.fighter_mut(gobj).take_dmg_cb = Some(CodeRef(k.end));
    }

    /// The fighter's speeds kept while it scales, `fn_800D2B30`, and its intangibility from
    /// dying over (`ftCommon_8007E2FC`).
    #[doc(alias = "fn_800D2B30")]
    pub fn save_kinoko_speeds(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let w = |v: f32| v.to_bits() as i32;
        let (a1, a2, gv, gkb, gsk) = (
            fp.xE4_ground_accel_1,
            fp.xE8_ground_accel_2,
            fp.gr_vel,
            fp.xF0_ground_kb_vel,
            fp.xF4_ground_attacker_shield_kb_vel,
        );
        fp.set_mv_i32(mvc::GR_ACCEL_1, w(a1));
        fp.set_mv_i32(mvc::GR_ACCEL_2, w(a2));
        let sa = fp.x74_self_accel;
        set_mv_vec(fp, mvc::SELF_ACCEL, sa);
        fp.set_mv_i32(mvc::GR_VEL, w(gv));
        let sv = fp.self_vel;
        set_mv_vec(fp, mvc::SELF_VEL, sv);
        fp.set_mv_i32(mvc::GR_KB_VEL, w(gkb));
        let kb = fp.x8c_kb_vel;
        set_mv_vec(fp, mvc::KB_VEL, kb);
        fp.set_mv_i32(mvc::GR_SHIELD_KB_VEL, w(gsk));
        let sk = fp.x98_atk_shield_kb;
        set_mv_vec(fp, mvc::SHIELD_KB, sk);
        let uv = fp.xD4_unk_vel;
        set_mv_vec(fp, mvc::UNK_VEL, uv);
        self.stop_dead(gobj);
    }

    /// The fighter's speeds back from before it scaled, `fn_800D2BF8`.
    #[doc(alias = "fn_800D2BF8")]
    pub fn restore_kinoko_speeds(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let r =
            |fp: &super::super::super::structs::Fighter, off| f32::from_bits(fp.mv_i32(off) as u32);
        fp.xE4_ground_accel_1 = r(fp, mvc::GR_ACCEL_1);
        fp.xE8_ground_accel_2 = r(fp, mvc::GR_ACCEL_2);
        fp.x74_self_accel = mv_vec(fp, mvc::SELF_ACCEL);
        fp.gr_vel = r(fp, mvc::GR_VEL);
        fp.self_vel = mv_vec(fp, mvc::SELF_VEL);
        fp.xF0_ground_kb_vel = r(fp, mvc::GR_KB_VEL);
        fp.x8c_kb_vel = mv_vec(fp, mvc::KB_VEL);
        fp.xF4_ground_attacker_shield_kb_vel = r(fp, mvc::GR_SHIELD_KB_VEL);
        fp.x98_atk_shield_kb = mv_vec(fp, mvc::SHIELD_KB);
        fp.xD4_unk_vel = mv_vec(fp, mvc::UNK_VEL);
    }
}

/// Copies a vector into the state's variables word for word.
fn set_mv_vec(fp: &mut super::super::super::structs::Fighter, off: usize, v: crate::vec::Vec3) {
    fp.set_mv_i32(off, v.x.to_bits() as i32);
    fp.set_mv_i32(off + 4, v.y.to_bits() as i32);
    fp.set_mv_i32(off + 8, v.z.to_bits() as i32);
}

/// A vector of the state's variables, word for word.
fn mv_vec(fp: &super::super::super::structs::Fighter, off: usize) -> crate::vec::Vec3 {
    let w = |o| f32::from_bits(fp.mv_i32(o) as u32);
    crate::vec::Vec3::new(w(off), w(off + 4), w(off + 8))
}
