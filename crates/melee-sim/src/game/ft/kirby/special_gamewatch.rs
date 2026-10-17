//! Mr. Game & Watch's copy (`ftkirbyspecialgamewatch.c`): Chef as Kirby flips it, a pan of
//! his own in hand (`it::kinds::kirbygamewatchchefpan`, kept in `u` as `xDC`) and each flip
//! throwing a sausage (the copy's own kind), one of five but the last two (`u` `xD4`, `xD8`).
//! Mr. Game & Watch's own (`ft::gamewatch::special_n`) with his states, attributes and `mv`;
//! the items take their colors from the copy's data (`ftKb_Init_800EEB00`, `_800EEB1C`).

use gekko_fp as fp;

use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::{self as k, gamewatchchef, kirbygamewatchchefpan as pan};
use super::super::super::state::Running;
use super::super::super::structs::_GXColor;
use super::super::gamewatch::{mv, part};
use super::{addr as kb, ms};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// The copy's function addresses.
pub(super) mod addr {
    pub const SAUSAGE: u32 = 0x8010_CE5C;
    pub const PAN: u32 = 0x8010_CFB0;
    pub const PAN_GONE: u32 = 0x8010_D074;
    pub const CLEANUP: u32 = 0x8010_D0A8;
    pub const ON_ENTER_HITLAG: u32 = 0x8010_D100;
    pub const ON_EXIT_HITLAG: u32 = 0x8010_D130;
    pub const ENTER: u32 = 0x8010_D188;
    pub const AIR_ENTER: u32 = 0x8010_D204;
    pub const N_ANIM: u32 = 0x8010_D280;
    pub const AIR_N_ANIM: u32 = 0x8010_D320;
    pub const N_IASA: u32 = 0x8010_D3C0;
    pub const AIR_N_IASA: u32 = 0x8010_D444;
    pub const N_PHYS: u32 = 0x8010_D4C8;
    pub const AIR_N_PHYS: u32 = 0x8010_D4E8;
    pub const N_COLL: u32 = 0x8010_D508;
    pub const AIR_N_COLL: u32 = 0x8010_D544;
    pub const TO_AIR: u32 = 0x8010_D580;
    pub const TO_GROUND: u32 = 0x8010_D5F0;
}

/// `u` offsets: the last sausage and the one before (`xD4`, `xD8`), and the pan (`xDC`).
const LAST: usize = 0xD4;
const BEFORE: usize = 0xD8;
const PAN: usize = 0xDC;
/// `ftKb_DatAttrs`' Mr. Game & Watch entries: the frame a flip loops back to, and the most
/// sausages a use throws.
const GW_LOOP_FRAME: usize = 0x3E4;
const GW_MAX: usize = 0x3E8;
/// The copy's kind, whose data holds its colors (`hat_dynamics[4]`, `x1C`).
const KIND_GAME_WATCH: usize = 24;
/// `ftKb_MF_GwSpecialN_Coll`, also what a loop keeps.
const KEEP: u32 = 0x0C4C_508C;
/// `HSD_PAD_B`.
const PAD_B: u32 = 0x200;

impl Running<'_> {
    /// A color of his copy of Mr. Game & Watch, the word `off` bytes into the copy's last
    /// dynamics (`hat_dynamics[4]`): the items' and the costume's materials' (4,
    /// `ftKb_Init_800EEB00`), and theirs and his outline's (8, `ftKb_Init_800EEB1C`).
    #[doc(alias = "ftKb_Init_800EEB00")]
    #[doc(alias = "ftKb_Init_800EEB1C")]
    pub fn kb_gw_color(&self, off: u32) -> _GXColor {
        let copy = self.game.ft.kirby.copies[KIND_GAME_WATCH].expect("a copy without its data");
        let colors = self
            .data
            .ptr(copy.at(0x1C))
            .expect("the copy's colors (hat_dynamics[4])");
        let [r, g, b, a] = self.data.u32(colors.at(off)).to_be_bytes();
        _GXColor { r, g, b, a }
    }

    /// Whether he has thrown fewer sausages than the attributes allow.
    fn kb_gw_more(&self, gobj: GObjId) -> bool {
        let f = self.game.fighter_id(gobj);
        let n = self.game.fighters[f.0].mv_i32(mv::N_SAUSAGES);
        fp::frsp(f64::from(n)) < reg(self.kb_attr_f32(f, GW_MAX))
    }

    /// A sausage at the script's mark, `fn_8010CE5C`: one of the five but the last two at
    /// random, from his thumb.
    #[doc(alias = "fn_8010CE5C")]
    pub fn kb_gw_sausage(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).cmd_vars[0] == 0 {
            return;
        }
        self.game.fighter_mut(gobj).cmd_vars[0] = 0;
        if self.kb_gw_more(gobj) {
            let fp = self.game.fighter_mut(gobj);
            let n = fp.mv_i32(mv::N_SAUSAGES);
            fp.set_mv_i32(mv::N_SAUSAGES, n.wrapping_add(1));
            let bone = self.bone_index(self.game.fighter(gobj), part::L_THUMB_NB);
            let pos = self.gw_part_pos(gobj, bone, Some(Vec3::new(2.5, 6.5, 0.0)));
            let fp = self.game.fighter(gobj);
            let (last, before) = (fp.u_i32(LAST), fp.u_i32(BEFORE));
            let choices: Vec<i32> = (0..5).filter(|&i| i != last && i != before).collect();
            let pick = self.game.objs.rng.below(choices.len() as i32);
            let sausage = choices[pick as usize];
            let fp = self.game.fighter_mut(gobj);
            fp.set_u_i32(BEFORE, last);
            fp.set_u_i32(LAST, sausage);
            let facing = fp.facing_dir;
            gamewatchchef::spawn(self, gobj, pos, k::KIRBY_GAME_WATCH_CHEF, sausage, facing);
        }
        self.game.fighter_mut(gobj).accessory4_cb = None;
    }

    /// The pan in hand at the move's start, `fn_8010CFB0`: with his damage hooks while he
    /// holds it, his hitlag's, and the sausages' accessory next.
    #[doc(alias = "fn_8010CFB0")]
    pub fn kb_gw_pan(&mut self, gobj: GObjId) {
        let bone = self.bone_index(self.game.fighter(gobj), part::L_THUMB_NB);
        let pos = self.gw_part_pos(gobj, bone, None);
        let facing = self.game.fighter(gobj).facing_dir;
        let p = pan::spawn(self, gobj, pos, bone, facing);
        let fp = self.game.fighter_mut(gobj);
        fp.u.set_gobj_at(PAN, p);
        if p.is_some() {
            fp.death2_cb = Some(CodeRef(kb::DEATH3));
            fp.take_dmg_cb = Some(CodeRef(kb::DEATH1));
        }
        fp.pre_hitlag_cb = Some(CodeRef(addr::ON_ENTER_HITLAG));
        fp.post_hitlag_cb = Some(CodeRef(addr::ON_EXIT_HITLAG));
        fp.accessory4_cb = Some(CodeRef(addr::SAUSAGE));
    }

    /// He forgets the pan, `ftKb_SpecialNGw_8010D074`, the pan going on after his hitlag first.
    #[doc(alias = "ftKb_SpecialNGw_8010D074")]
    pub fn kb_gw_pan_gone(&mut self, gobj: GObjId) {
        self.kb_gw_exit_hitlag(gobj);
        self.game.fighter_mut(gobj).u.set_gobj_at(PAN, None);
    }

    /// The pan taken away when he's hit or KO'd, `ftKb_SpecialNGw_8010D0A8`.
    #[doc(alias = "ftKb_SpecialNGw_8010D0A8")]
    pub fn kb_gw_cleanup(&mut self, gobj: GObjId) {
        if let Some(p) = self.game.fighter(gobj).u.gobj_at(PAN) {
            pan::remove(self, p);
            self.kb_gw_pan_gone(gobj);
        }
    }

    /// The pan pauses in his hitlag, `fn_8010D100`.
    #[doc(alias = "fn_8010D100")]
    pub fn kb_gw_enter_hitlag(&mut self, gobj: GObjId) {
        if let Some(p) = self.game.fighter(gobj).u.gobj_at(PAN) {
            pan::hitlag_start(self, p);
        }
    }

    /// The pan goes on after his hitlag, `ftKb_SpecialNGw_8010D130`.
    #[doc(alias = "ftKb_SpecialNGw_8010D130")]
    pub fn kb_gw_exit_hitlag(&mut self, gobj: GObjId) {
        if let Some(p) = self.game.fighter(gobj).u.gobj_at(PAN) {
            pan::hitlag_end(self, p);
        }
    }

    /// Whether he has left the move, `ftKb_SpecialNGw_8010D160` (true outside its two
    /// states).
    #[doc(alias = "ftKb_SpecialNGw_8010D160")]
    pub fn kb_gw_done(&self, gobj: GObjId) -> bool {
        let msid = self.game.fighter(gobj).motion_id;
        !(ms::GW_SPECIAL_N..=ms::GW_SPECIAL_AIR_N).contains(&msid)
    }

    /// Chef on the ground or in the air (`ftKb_SpecialNGw_8010D188`, `_8010D204`): no vertical
    /// speed, the script's variables cleared, and the pan next.
    fn kb_gw_start(&mut self, gobj: GObjId, msid: i32) {
        self.game.fighter_mut(gobj).self_vel.y = 0.0;
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[0] = 0;
        fp.cmd_vars[1] = 0;
        fp.cmd_vars[2] = 0;
        fp.set_mv_i32(mv::N_LOOP_DISABLE, 0);
        fp.set_mv_i32(mv::N_SAUSAGES, 0);
        fp.accessory4_cb = Some(CodeRef(addr::PAN));
    }

    /// Chef on the ground, `ftKb_SpecialNGw_8010D188`.
    #[doc(alias = "ftKb_SpecialNGw_8010D188")]
    pub fn kb_gw_enter(&mut self, gobj: GObjId) {
        self.kb_gw_start(gobj, ms::GW_SPECIAL_N);
    }

    /// In the air, `ftKb_SpecialNGw_8010D204`.
    #[doc(alias = "ftKb_SpecialNGw_8010D204")]
    pub fn kb_gw_air_enter(&mut self, gobj: GObjId) {
        self.kb_gw_start(gobj, ms::GW_SPECIAL_AIR_N);
    }

    /// Chef from the attributes' frame again (`ftKb_SpecialNGw_8010D660`, `_8010D6D0`).
    #[doc(alias = "ftKb_SpecialNGw_8010D660")]
    #[doc(alias = "ftKb_SpecialNGw_8010D6D0")]
    fn kb_gw_loop(&mut self, gobj: GObjId, air: bool) {
        let f = self.game.fighter_id(gobj);
        let frame = self.kb_attr_f32(f, GW_LOOP_FRAME);
        let msid = if air {
            ms::GW_SPECIAL_AIR_N
        } else {
            ms::GW_SPECIAL_N
        };
        let start = mem(fp::fsubs(reg(frame), 1.0));
        self.change_motion_state(gobj, msid, KEEP, start, 1.0, 0.0, None);
        self.anim_update(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[1] = 0;
        fp.cmd_vars[2] = 0;
        fp.set_mv_i32(mv::N_LOOP_DISABLE, 0);
        fp.accessory4_cb = Some(CodeRef(addr::SAUSAGE));
    }

    /// The flip (`ftKb_GwSpecialN_Anim`, `ftKb_GwSpecialAirN_Anim`, testing in each one's
    /// order): at the script's mark, the loop with B still held and sausages left; waiting
    /// or a fall once its frames run out.
    #[doc(alias = "ftKb_GwSpecialN_Anim")]
    #[doc(alias = "ftKb_GwSpecialAirN_Anim")]
    fn kb_gw_anim(&mut self, gobj: GObjId, air: bool) {
        let fp = self.game.fighter_mut(gobj);
        if fp.cmd_vars[2] != 0 {
            fp.cmd_vars[2] = 0;
            let held = fp.mv_i32(mv::N_LOOP_DISABLE) == 0;
            if held && self.kb_gw_more(gobj) {
                self.kb_gw_loop(gobj, air);
            }
        }
        if !self.anim_frames_remaining(gobj) {
            if air {
                self.enter_fall(gobj);
            } else {
                self.enter_wait(gobj);
            }
        }
    }

    /// The flip's interrupts (`ftKb_GwSpecialN_IASA`, `ftKb_GwSpecialAirN_IASA`): B let go of
    /// ends the loop at the mark; B pressed again after the script allows it loops now.
    #[doc(alias = "ftKb_GwSpecialN_IASA")]
    #[doc(alias = "ftKb_GwSpecialAirN_IASA")]
    fn kb_gw_iasa(&mut self, gobj: GObjId, air: bool) {
        let fp = self.game.fighter_mut(gobj);
        if fp.input.held_buttons[0] & PAD_B == 0 {
            fp.set_mv_i32(mv::N_LOOP_DISABLE, 1);
        }
        let fp = self.game.fighter(gobj);
        if fp.cmd_vars[1] != 0 && fp.input.pressed_buttons & PAD_B != 0 && self.kb_gw_more(gobj) {
            self.kb_gw_loop(gobj, air);
        }
    }

    /// `ftKb_GwSpecialN_Phys`: friction.
    #[doc(alias = "ftKb_GwSpecialN_Phys")]
    fn kb_gw_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// `ftKb_GwSpecialAirN_Phys`: the air's physics without drift.
    #[doc(alias = "ftKb_GwSpecialAirN_Phys")]
    fn kb_gw_air_phys(&mut self, gobj: GObjId) {
        self.air_physics_no_drift(gobj);
    }

    /// `ftKb_GwSpecialN_Coll`: off an edge, into the air.
    #[doc(alias = "ftKb_GwSpecialN_Coll")]
    fn kb_gw_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_slide(gobj) {
            self.kb_gw_to_air(gobj);
        }
    }

    /// `ftKb_GwSpecialAirN_Coll`: landing, onto the ground.
    #[doc(alias = "ftKb_GwSpecialAirN_Coll")]
    fn kb_gw_air_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.kb_gw_to_ground(gobj);
        }
    }

    /// Off an edge, `ftKb_SpecialNGw_8010D580`: the aerial flip from the same frame.
    #[doc(alias = "ftKb_SpecialNGw_8010D580")]
    pub fn kb_gw_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::GW_SPECIAL_AIR_N, KEEP);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::SAUSAGE));
    }

    /// Landing, `ftKb_SpecialNGw_8010D5F0`: the grounded flip from the same frame.
    #[doc(alias = "ftKb_SpecialNGw_8010D5F0")]
    pub fn kb_gw_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::GW_SPECIAL_N, KEEP);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::SAUSAGE));
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::SAUSAGE => g.kb_gw_sausage(gobj),
        addr::PAN => g.kb_gw_pan(gobj),
        addr::PAN_GONE => g.kb_gw_pan_gone(gobj),
        addr::CLEANUP => g.kb_gw_cleanup(gobj),
        addr::ON_ENTER_HITLAG => g.kb_gw_enter_hitlag(gobj),
        addr::ON_EXIT_HITLAG => g.kb_gw_exit_hitlag(gobj),
        addr::ENTER => g.kb_gw_enter(gobj),
        addr::AIR_ENTER => g.kb_gw_air_enter(gobj),
        addr::N_ANIM => g.kb_gw_anim(gobj, false),
        addr::AIR_N_ANIM => g.kb_gw_anim(gobj, true),
        addr::N_IASA => g.kb_gw_iasa(gobj, false),
        addr::AIR_N_IASA => g.kb_gw_iasa(gobj, true),
        addr::N_PHYS => g.kb_gw_phys(gobj),
        addr::AIR_N_PHYS => g.kb_gw_air_phys(gobj),
        addr::N_COLL => g.kb_gw_coll(gobj),
        addr::AIR_N_COLL => g.kb_gw_air_coll(gobj),
        addr::TO_AIR => g.kb_gw_to_air(gobj),
        addr::TO_GROUND => g.kb_gw_to_ground(gobj),
        _ => return false,
    }
    true
}
