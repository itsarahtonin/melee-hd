//! Yoshi's copy (`ftkirbyspecialyoshi.c`): the egg lay as Kirby does it. Yoshi's own
//! (`ft::yoshi::special_n`) with Kirby's states, his attributes and his copy's hat, whose
//! tongue animations (the copy data's dynamics 1 to 4) play on it through the move. A fighter
//! caught goes into his copy's own capture and egg states (`ft::co::yoshi_egg`), an item onto
//! his copy's own tongue item (`it::kinds::yoshitongue`) and into his copy's egg item.

use gekko_fp as fp;

use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::{self as it_kinds, yoshiegglay, yoshitongue};
use super::super::super::state::Running;
use super::super::co::yoshi_egg::KB_CAPTURE;
use super::super::parts::TRANS_N2;
use super::{ms, u};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

/// The copy's function addresses.
pub(super) mod addr {
    pub const HAT_RESET: u32 = 0x8010_93B4;
    pub const ENTER: u32 = 0x8010_941C;
    pub const AIR_ENTER: u32 = 0x8010_94FC;
    pub const GRAB_ITEM: u32 = 0x8010_95DC;
    pub const GRAB_FIGHTER: u32 = 0x8010_9680;
    pub const AIR_GRAB_ITEM: u32 = 0x8010_9714;
    pub const AIR_GRAB_FIGHTER: u32 = 0x8010_97B8;
    pub const N1_TO_AIR: u32 = 0x8010_984C;
    pub const AIR_N_TO_GROUND: u32 = 0x8010_98EC;
    pub const ITEM_PULL_TO_AIR: u32 = 0x8010_998C;
    pub const PULL_TO_AIR: u32 = 0x8010_9A08;
    pub const AIR_ITEM_PULL_TO_GROUND: u32 = 0x8010_9A84;
    pub const AIR_PULL_TO_GROUND: u32 = 0x8010_9B00;
    pub const ITEM_SWALLOW_TO_AIR: u32 = 0x8010_9B7C;
    pub const SWALLOW_TO_AIR: u32 = 0x8010_9BF8;
    pub const AIR_ITEM_SWALLOW_TO_GROUND: u32 = 0x8010_9C74;
    pub const AIR_SWALLOW_TO_GROUND: u32 = 0x8010_9CF0;
    pub const N1_ANIM: u32 = 0x8010_9D6C;
    pub const AIR_N_ANIM: u32 = 0x8010_9DB8;
    pub const ITEM_PULL_ANIM: u32 = 0x8010_9E04;
    pub const PULL_ANIM: u32 = 0x8010_9EE8;
    pub const AIR_ITEM_PULL_ANIM: u32 = 0x8010_9FD4;
    pub const AIR_PULL_ANIM: u32 = 0x8010_A0B8;
    pub const ITEM_SWALLOW_ANIM: u32 = 0x8010_A1A4;
    pub const SWALLOW_ANIM: u32 = 0x8010_A304;
    pub const AIR_ITEM_SWALLOW_ANIM: u32 = 0x8010_A3C8;
    pub const AIR_SWALLOW_ANIM: u32 = 0x8010_A528;
    pub const N1_PHYS: u32 = 0x8010_A5EC;
    pub const AIR_N_PHYS: u32 = 0x8010_A60C;
    pub const ITEM_PULL_PHYS: u32 = 0x8010_A62C;
    pub const PULL_PHYS: u32 = 0x8010_A64C;
    pub const AIR_PULL_PHYS: u32 = 0x8010_A66C;
    pub const AIR_ITEM_PULL_PHYS: u32 = 0x8010_A68C;
    pub const ITEM_SWALLOW_PHYS: u32 = 0x8010_A6AC;
    pub const SWALLOW_PHYS: u32 = 0x8010_A6CC;
    pub const AIR_ITEM_SWALLOW_PHYS: u32 = 0x8010_A6EC;
    pub const AIR_SWALLOW_PHYS: u32 = 0x8010_A70C;
    pub const N1_COLL: u32 = 0x8010_A72C;
    pub const AIR_N_COLL: u32 = 0x8010_A754;
    pub const ITEM_PULL_COLL: u32 = 0x8010_A77C;
    pub const PULL_COLL: u32 = 0x8010_A7A4;
    pub const AIR_ITEM_PULL_COLL: u32 = 0x8010_A7CC;
    pub const AIR_PULL_COLL: u32 = 0x8010_A7F4;
    pub const ITEM_SWALLOW_COLL: u32 = 0x8010_A81C;
    pub const SWALLOW_COLL: u32 = 0x8010_A844;
    pub const AIR_ITEM_SWALLOW_COLL: u32 = 0x8010_A86C;
    pub const AIR_SWALLOW_COLL: u32 = 0x8010_A894;
    pub const RELEASE_ITEM: u32 = 0x8010_A8BC;
}

/// Yoshi's copied kind.
const KIND_YOSHI: usize = 14;
/// The swallow's flags (keeping the tongue's effects and models), the catch's, and the
/// switches between the ground and the air (`ftYs_MF_SpecialN_Coll`, `_CollHit`).
const FLAGS0: u32 = 0x80012;
const FLAGS1: u32 = 0x81092;
const COLL: u32 = 0xC4C_5092;
const COLL_HIT: u32 = 0xC4C_509A;
/// Kirby's attributes: the egg's speed back and up, its damage behavior and its lifetime;
/// from his loaded attributes the damage's share of an egg item's life.
const VEL_X: usize = 0x3AC;
const VEL_Y: usize = 0x3B0;
const BEHAVIOR: usize = 0x3B4;
const LIFETIME: usize = 0x3C0;
const DAMAGE: usize = 0x3E0;

impl Running<'_> {
    /// One of the copy's tongue animations (its data's dynamics `idx`) put on his hat and
    /// started, and played a frame when `play`.
    fn kb_ys_hat_anim(&mut self, gobj: GObjId, idx: u32, play: bool) {
        let copy = self.game.ft.kirby.copies[KIND_YOSHI].expect("a copy without its data");
        let anim = self.data.ptr(copy.at(0xC + 4 * idx));
        let hat = self.game.fighter(gobj).u.jobj_at(u::HAT_JOBJ);
        jobj::add_anim_all(&mut self.game.objs, self.data, hat, anim, None, None);
        jobj::req_anim_all(&mut self.game.objs, hat, 0.0);
        if play {
            self.kb_ys_hat_play(gobj);
        }
    }

    /// His hat's animation a frame on (`HSD_JObjAnimAll`).
    fn kb_ys_hat_play(&mut self, gobj: GObjId) {
        let hat = self.game.fighter(gobj).u.jobj_at(u::HAT_JOBJ);
        jobj::anim_all(
            &mut self.game.objs,
            self.data,
            &mut crate::game::code::ParticleHooks,
            hat,
        );
    }

    /// The tongue as a grab box (`ftCommon_8007E2D0(fp, 4, ...)`): a fighter caught runs
    /// `grab`, an item `item`, and the fighter's side his copy's capture.
    fn kb_ys_grab_cbs(&mut self, gobj: GObjId, grab: u32, item: u32) {
        let fp = self.game.fighter_mut(gobj);
        fp.x221E_b6 = true;
        fp.x1A68 = 4;
        fp.grab_cb = Some(CodeRef(grab));
        fp.grabbed_cb = Some(CodeRef(KB_CAPTURE));
        fp.x2194 = Some(CodeRef(item));
    }

    /// The hat's tongue back at its rest (`ftKb_SpecialNYs_801093B4`, with the copy's
    /// cleanups): its first animation played a frame and stopped.
    #[doc(alias = "ftKb_SpecialNYs_801093B4")]
    pub fn kb_ys_hat_reset(&mut self, gobj: GObjId) {
        self.kb_ys_hat_anim(gobj, 1, true);
        let hat = self.game.fighter(gobj).u.jobj_at(u::HAT_JOBJ);
        crate::lb::anim::set_framerate(&mut self.game.objs, hat, 0.0);
    }

    /// The tongue's start: its state, the hat's animation, the grab box and his cleanups.
    fn kb_ys_start(&mut self, gobj: GObjId, msid: i32, anim: u32, grab: u32, item: u32) {
        self.game.fighter_mut(gobj).cmd_vars[0] = 0;
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.kb_ys_hat_anim(gobj, anim, true);
        self.kb_ys_grab_cbs(gobj, grab, item);
        self.kb_set_cleanup_cbs(gobj);
    }

    /// The egg lay on the ground, `ftKb_SpecialNYs_8010941C`.
    #[doc(alias = "ftKb_SpecialNYs_8010941C")]
    pub fn kb_ys_enter(&mut self, gobj: GObjId) {
        self.kb_ys_start(
            gobj,
            ms::YS_SPECIAL_N1,
            1,
            addr::GRAB_FIGHTER,
            addr::GRAB_ITEM,
        );
    }

    /// In the air, `ftKb_SpecialNYs_801094FC`.
    #[doc(alias = "ftKb_SpecialNYs_801094FC")]
    pub fn kb_ys_air_enter(&mut self, gobj: GObjId) {
        self.kb_ys_start(
            gobj,
            ms::YS_SPECIAL_AIR_N_CAPTURE2,
            3,
            addr::AIR_GRAB_FIGHTER,
            addr::AIR_GRAB_ITEM,
        );
    }

    /// The tongue caught something: the pull's state from the same frame, the catch done
    /// (`x2222_b2`) and still.
    fn kb_ys_caught(&mut self, gobj: GObjId, msid: i32) {
        let frame = self.game.fighter(gobj).cur_anim_frame;
        self.change_motion_state(gobj, msid, FLAGS1, frame, 1.0, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        fp.x2222_b2 = true;
        fp.x1A6A = 0x1FF;
        self.stop_dead(gobj);
        self.kb_set_cleanup_cbs(gobj);
    }

    /// An item caught: onto his copy's tongue (`it_802F2D98`).
    fn kb_ys_caught_item(&mut self, gobj: GObjId, msid: i32) {
        let item = self
            .game
            .fighter(gobj)
            .target_item_gobj
            .expect("no item caught");
        yoshitongue::catch_kirby(self, item, gobj);
        self.kb_ys_caught(gobj, msid);
    }

    /// The tongue caught an item on the ground, `fn_801095DC`.
    // Ports `fn_801095DC`; aliased once a check reaches it.
    pub fn kb_ys_grab_item(&mut self, gobj: GObjId) {
        self.kb_ys_caught_item(gobj, ms::YS_SPECIAL_N_CAPTURE1_1);
    }

    /// The tongue caught a fighter on the ground, `fn_80109680`.
    #[doc(alias = "fn_80109680")]
    pub fn kb_ys_grab_fighter(&mut self, gobj: GObjId) {
        self.kb_ys_caught(gobj, ms::YS_SPECIAL_N_CAPTURE1_0);
    }

    /// The tongue caught an item in the air, `fn_80109714`.
    // Ports `fn_80109714`; aliased once a check reaches it.
    pub fn kb_ys_air_grab_item(&mut self, gobj: GObjId) {
        self.kb_ys_caught_item(gobj, ms::YS_SPECIAL_AIR_N_CAPTURE1_1);
    }

    /// The tongue caught a fighter in the air, `fn_801097B8`.
    #[doc(alias = "fn_801097B8")]
    pub fn kb_ys_air_grab_fighter(&mut self, gobj: GObjId) {
        self.kb_ys_caught(gobj, ms::YS_SPECIAL_AIR_N_CAPTURE1_0);
    }

    /// Off the ground with the tongue out, `fn_8010984C`: the aerial tongue from the same
    /// frame and its grab box.
    #[doc(alias = "fn_8010984C")]
    pub fn kb_ys_n1_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::YS_SPECIAL_AIR_N_CAPTURE2, COLL_HIT);
        self.kb_ys_grab_cbs(gobj, addr::AIR_GRAB_FIGHTER, addr::AIR_GRAB_ITEM);
        self.kb_set_cleanup_cbs(gobj);
    }

    /// Landing with the tongue out, `fn_801098EC`.
    #[doc(alias = "fn_801098EC")]
    pub fn kb_ys_air_n_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::YS_SPECIAL_N1, COLL_HIT);
        self.kb_ys_grab_cbs(gobj, addr::GRAB_FIGHTER, addr::GRAB_ITEM);
        self.kb_set_cleanup_cbs(gobj);
    }

    /// The pulls and swallows off the ground (`fn_8010998C`, `fn_80109A08`, `fn_80109B7C`,
    /// `fn_80109BF8`) and landing (`fn_80109A84`, `fn_80109B00`, `fn_80109C74`,
    /// `fn_80109CF0`): the counterpart from the same frame, and his cleanups.
    #[doc(alias = "fn_8010998C")]
    #[doc(alias = "fn_80109A08")]
    #[doc(alias = "fn_80109B7C")]
    #[doc(alias = "fn_80109BF8")]
    #[doc(alias = "fn_80109A84")]
    #[doc(alias = "fn_80109B00")]
    #[doc(alias = "fn_80109C74")]
    #[doc(alias = "fn_80109CF0")]
    pub fn kb_ys_switch(&mut self, gobj: GObjId, msid: i32, to_air: bool) {
        if to_air {
            self.fox_ground_to_air(gobj, msid, COLL);
        } else {
            self.fox_air_to_ground(gobj, msid, COLL);
        }
        self.kb_set_cleanup_cbs(gobj);
    }

    /// `ftKb_YsSpecialN1_Anim` and `ftKb_YsSpecialAirNCapture2_Anim`: the hat's tongue a
    /// frame on, and waiting or falling once it plays out.
    #[doc(alias = "ftKb_YsSpecialN1_Anim")]
    #[doc(alias = "ftKb_YsSpecialAirNCapture2_Anim")]
    pub fn kb_ys_tongue_anim(&mut self, gobj: GObjId, air: bool) {
        self.kb_ys_hat_play(gobj);
        if !self.anim_frames_remaining(gobj) {
            if air {
                self.enter_fall(gobj);
            } else {
                self.enter_wait(gobj);
            }
        }
    }

    /// The pull's end: once the script says and the tongue still holds what it caught, the
    /// swallow's state with the hat's swallow (dynamics `anim`). The fighter's pulls play the
    /// hat's tongue a frame on first; the items' don't.
    #[doc(alias = "ftKb_YsSpecialNCapture1_Anim")]
    #[doc(alias = "ftKb_YsSpecialAirNCapture1_Anim")]
    #[doc(alias = "ftKb_YsSpecialAirCapture1_Anim")]
    #[doc(alias = "ftKb_YsSpecialAirCapture2_Anim")]
    pub fn kb_ys_pull_anim(&mut self, gobj: GObjId, msid: i32, item: bool, anim: u32) {
        if !item {
            self.kb_ys_hat_play(gobj);
        }
        let fp = self.game.fighter(gobj);
        let holds = if item {
            fp.target_item_gobj.is_some()
        } else {
            fp.victim_gobj.is_some()
        };
        if fp.cmd_vars[0] == 0 || !holds {
            return;
        }
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[0] = 0;
        fp.cmd_vars[1] = 0;
        self.change_motion_state(gobj, msid, FLAGS0, 0.0, 1.0, 0.0, None);
        self.kb_ys_hat_anim(gobj, anim, false);
        self.kb_set_cleanup_cbs(gobj);
        self.game.fighter_mut(gobj).x1A6A = 0x1FF;
    }

    /// Waiting or falling once the swallow plays out.
    fn kb_ys_swallow_end(&mut self, gobj: GObjId, air: bool) {
        if !self.anim_frames_remaining(gobj) {
            if air {
                self.enter_fall(gobj);
            } else {
                self.enter_wait(gobj);
            }
        }
    }

    /// Swallowing an item (`ftKb_YsSpecialNCapture2_0_Anim`, `ftKb_YsSpecialAirN2_1_Anim`):
    /// the hat a frame on; the script's second flag lets the item go; its first lays his
    /// copy's egg at the tongue's tip, item or not.
    #[doc(alias = "ftKb_YsSpecialNCapture2_0_Anim")]
    #[doc(alias = "ftKb_YsSpecialAirN2_1_Anim")]
    pub fn kb_ys_swallow_item_anim(&mut self, gobj: GObjId, air: bool) {
        self.kb_ys_hat_play(gobj);
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        if fp.cmd_vars[1] != 0
            && let Some(item) = fp.target_item_gobj
        {
            yoshitongue::release(self, item, false);
            let fp = &mut self.game.fighters[f.0];
            fp.x1A64 = Default::default();
            fp.target_item_gobj = None;
            fp.cmd_vars[1] = 0;
        }
        if self.game.fighters[f.0].cmd_vars[0] != 0 {
            self.game.fighters[f.0].x1A6A = 0;
            let tip = {
                let fp = &self.game.fighters[f.0];
                self.bone_index(fp, TRANS_N2)
            };
            let j = self.bone_joint(gobj, tip);
            let pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None);
            let facing = self.game.fighters[f.0].facing_dir;
            let vx = mem(fp::fmuls(
                fp::fneg(reg(facing)),
                reg(self.kb_attr_f32(f, VEL_X)),
            ));
            let (a, b) = (self.kb_ext_attr_f32(DAMAGE), self.kb_ext_attr_f32(BEHAVIOR));
            let laid = yoshiegglay::Laid {
                kind: it_kinds::KIRBY_YOSHI_EGG_LAY,
                pos,
                vel: Vec3::new(vx, self.kb_attr_f32(f, VEL_Y), 0.0),
                facing_dir: facing,
                lifetime: self.kb_attr_f32(f, LIFETIME),
                x24: self.kb_attr_f32(f, BEHAVIOR),
                damage_mul: mem(fp::fdivs(reg(a), reg(b))),
            };
            yoshiegglay::spawn(self, &laid);
            self.game.fighters[f.0].cmd_vars[0] = 0;
        }
        self.kb_ys_swallow_end(gobj, air);
    }

    /// Swallowing a fighter (`ftKb_YsSpecialNCapture2_1_Anim`, `ftKb_YsSpecialAirN2_0_Anim`):
    /// the hat a frame on; the script's second flag swallows it; its first lets it go from
    /// his mouth into his copy's egg.
    #[doc(alias = "ftKb_YsSpecialNCapture2_1_Anim")]
    #[doc(alias = "ftKb_YsSpecialAirN2_0_Anim")]
    pub fn kb_ys_swallow_anim(&mut self, gobj: GObjId, air: bool) {
        self.kb_ys_hat_play(gobj);
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[1] != 0
            && let Some(victim) = self.game.fighters[f.0].victim_gobj
        {
            self.yoshi_swallowed(victim);
            self.game.fighters[f.0].cmd_vars[1] = 0;
        }
        if self.game.fighters[f.0].cmd_vars[0] != 0
            && let Some(victim) = self.game.fighters[f.0].victim_gobj
        {
            self.game.fighters[f.0].x1A6A = 0;
            self.throw_hit(gobj, victim, false);
            self.enter_kirby_yoshi_egg(victim, gobj);
            self.game.fighters[f.0].cmd_vars[0] = 0;
        }
        self.kb_ys_swallow_end(gobj, air);
    }

    /// An item on his copy's tongue let go, `ftKb_SpecialNYs_8010A8BC` (with the copy's
    /// cleanups): out of the item's pull or swallow, its destruction kind cleared.
    #[doc(alias = "ftKb_SpecialNYs_8010A8BC")]
    pub fn kb_ys_release_item(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        let Some(item) = fp.target_item_gobj else {
            return;
        };
        let clear = matches!(
            fp.motion_id,
            ms::YS_SPECIAL_N_CAPTURE1_1
                | ms::YS_SPECIAL_N_CAPTURE2_1
                | ms::YS_SPECIAL_AIR_N_CAPTURE1_1
                | ms::YS_SPECIAL_AIR_N2_1
        );
        yoshitongue::release(self, item, clear);
        let fp = self.game.fighter_mut(gobj);
        fp.x1A64 = Default::default();
        fp.target_item_gobj = None;
    }

    /// The copy's cleanup when hit or KO'd: the hat's tongue back at rest and an item let go.
    pub(super) fn kb_ys_cleanup(&mut self, gobj: GObjId) {
        self.kb_ys_hat_reset(gobj);
        self.kb_ys_release_item(gobj);
    }

    /// The states' physics: friction on the ground, gravity without drift in the air.
    #[doc(alias = "ftKb_YsSpecialN1_Phys")]
    #[doc(alias = "ftKb_YsSpecialAirNCapture1_Phys")]
    #[doc(alias = "ftKb_YsSpecialNCapture1_Phys")]
    #[doc(alias = "ftKb_YsSpecialNCapture2_0_Phys")]
    #[doc(alias = "ftKb_YsSpecialNCapture2_1_Phys")]
    #[doc(alias = "ftKb_YsSpecialAirNCapture2_Phys")]
    #[doc(alias = "ftKb_YsSpecialAirCapture1_Phys")]
    #[doc(alias = "ftKb_YsSpecialAirCapture2_Phys")]
    #[doc(alias = "ftKb_YsSpecialAirN2_1_Phys")]
    #[doc(alias = "ftKb_YsSpecialAirN2_0_Phys")]
    pub fn kb_ys_phys(&mut self, gobj: GObjId, air: bool) {
        if air {
            self.air_physics_no_drift(gobj);
        } else {
            self.ground_friction(gobj);
        }
    }
}

/// The states' collisions: off the ground or landing, the counterpart.
#[doc(alias = "ftKb_YsSpecialN1_Coll")]
#[doc(alias = "ftKb_YsSpecialAirNCapture2_Coll")]
#[doc(alias = "ftKb_YsSpecialAirNCapture1_Coll")]
#[doc(alias = "ftKb_YsSpecialNCapture1_Coll")]
#[doc(alias = "ftKb_YsSpecialAirCapture2_Coll")]
#[doc(alias = "ftKb_YsSpecialAirCapture1_Coll")]
#[doc(alias = "ftKb_YsSpecialNCapture2_0_Coll")]
#[doc(alias = "ftKb_YsSpecialNCapture2_1_Coll")]
#[doc(alias = "ftKb_YsSpecialAirN2_1_Coll")]
#[doc(alias = "ftKb_YsSpecialAirN2_0_Coll")]
fn coll(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    use addr::*;
    match a {
        N1_COLL => g.ground_coll_fall_off_then(gobj, |r, g| r.kb_ys_n1_to_air(g)),
        AIR_N_COLL => g.air_coll_then(gobj, |r, g| r.kb_ys_air_n_to_ground(g)),
        ITEM_PULL_COLL => g.ground_coll_fall_off_then(gobj, |r, g| {
            r.kb_ys_switch(g, ms::YS_SPECIAL_AIR_N_CAPTURE1_1, true)
        }),
        PULL_COLL => g.ground_coll_fall_off_then(gobj, |r, g| {
            r.kb_ys_switch(g, ms::YS_SPECIAL_AIR_N_CAPTURE1_0, true)
        }),
        ITEM_SWALLOW_COLL => g.ground_coll_fall_off_then(gobj, |r, g| {
            r.kb_ys_switch(g, ms::YS_SPECIAL_AIR_N2_1, true)
        }),
        SWALLOW_COLL => g.ground_coll_fall_off_then(gobj, |r, g| {
            r.kb_ys_switch(g, ms::YS_SPECIAL_AIR_N2_0, true)
        }),
        AIR_ITEM_PULL_COLL => g.air_coll_then(gobj, |r, g| {
            r.kb_ys_switch(g, ms::YS_SPECIAL_N_CAPTURE1_1, false)
        }),
        AIR_PULL_COLL => g.air_coll_then(gobj, |r, g| {
            r.kb_ys_switch(g, ms::YS_SPECIAL_N_CAPTURE1_0, false)
        }),
        AIR_ITEM_SWALLOW_COLL => g.air_coll_then(gobj, |r, g| {
            r.kb_ys_switch(g, ms::YS_SPECIAL_N_CAPTURE2_1, false)
        }),
        AIR_SWALLOW_COLL => g.air_coll_then(gobj, |r, g| {
            r.kb_ys_switch(g, ms::YS_SPECIAL_N_CAPTURE2_0, false)
        }),
        _ => return false,
    }
    true
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    use addr::*;
    match a {
        HAT_RESET => g.kb_ys_hat_reset(gobj),
        ENTER => g.kb_ys_enter(gobj),
        AIR_ENTER => g.kb_ys_air_enter(gobj),
        GRAB_ITEM => g.kb_ys_grab_item(gobj),
        GRAB_FIGHTER => g.kb_ys_grab_fighter(gobj),
        AIR_GRAB_ITEM => g.kb_ys_air_grab_item(gobj),
        AIR_GRAB_FIGHTER => g.kb_ys_air_grab_fighter(gobj),
        N1_TO_AIR => g.kb_ys_n1_to_air(gobj),
        AIR_N_TO_GROUND => g.kb_ys_air_n_to_ground(gobj),
        ITEM_PULL_TO_AIR => g.kb_ys_switch(gobj, ms::YS_SPECIAL_AIR_N_CAPTURE1_1, true),
        PULL_TO_AIR => g.kb_ys_switch(gobj, ms::YS_SPECIAL_AIR_N_CAPTURE1_0, true),
        ITEM_SWALLOW_TO_AIR => g.kb_ys_switch(gobj, ms::YS_SPECIAL_AIR_N2_1, true),
        SWALLOW_TO_AIR => g.kb_ys_switch(gobj, ms::YS_SPECIAL_AIR_N2_0, true),
        AIR_ITEM_PULL_TO_GROUND => g.kb_ys_switch(gobj, ms::YS_SPECIAL_N_CAPTURE1_1, false),
        AIR_PULL_TO_GROUND => g.kb_ys_switch(gobj, ms::YS_SPECIAL_N_CAPTURE1_0, false),
        AIR_ITEM_SWALLOW_TO_GROUND => g.kb_ys_switch(gobj, ms::YS_SPECIAL_N_CAPTURE2_1, false),
        AIR_SWALLOW_TO_GROUND => g.kb_ys_switch(gobj, ms::YS_SPECIAL_N_CAPTURE2_0, false),
        N1_ANIM => g.kb_ys_tongue_anim(gobj, false),
        AIR_N_ANIM => g.kb_ys_tongue_anim(gobj, true),
        ITEM_PULL_ANIM => g.kb_ys_pull_anim(gobj, ms::YS_SPECIAL_N_CAPTURE2_1, true, 2),
        PULL_ANIM => g.kb_ys_pull_anim(gobj, ms::YS_SPECIAL_N_CAPTURE2_0, false, 2),
        AIR_ITEM_PULL_ANIM => g.kb_ys_pull_anim(gobj, ms::YS_SPECIAL_AIR_N2_1, true, 4),
        AIR_PULL_ANIM => g.kb_ys_pull_anim(gobj, ms::YS_SPECIAL_AIR_N2_0, false, 4),
        ITEM_SWALLOW_ANIM => g.kb_ys_swallow_item_anim(gobj, false),
        AIR_ITEM_SWALLOW_ANIM => g.kb_ys_swallow_item_anim(gobj, true),
        SWALLOW_ANIM => g.kb_ys_swallow_anim(gobj, false),
        AIR_SWALLOW_ANIM => g.kb_ys_swallow_anim(gobj, true),
        N1_PHYS | ITEM_PULL_PHYS | PULL_PHYS | ITEM_SWALLOW_PHYS | SWALLOW_PHYS => {
            g.kb_ys_phys(gobj, false)
        }
        AIR_N_PHYS
        | AIR_PULL_PHYS
        | AIR_ITEM_PULL_PHYS
        | AIR_ITEM_SWALLOW_PHYS
        | AIR_SWALLOW_PHYS => g.kb_ys_phys(gobj, true),
        RELEASE_ITEM => g.kb_ys_release_item(gobj),
        _ => return coll(g, gobj, a),
    }
    true
}
