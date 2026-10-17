//! Peach's copy (`ftkirbyspecialpeach.c`): Toad held out from his hand (the copy's own kind,
//! kept in `u` as `xD0`), whose bubble turns an attack into the counter's spore. Peach's own
//! (`ft::peach::special_n`) with his states and attributes, the bubble's shape among them
//! (`specialn_pe_absorbdesc`); his first aerial Toad of a jump lifts him (`u` `xCC`).

use gekko_fp as fp;

use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::{self as k, peachtoad, peachtoadspore};
use super::super::super::state::Running;
use super::super::co::guard::set_shield_hit;
use super::super::common::GA_GROUND;
use super::{addr as kb, ms};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// The copy's function addresses.
pub(super) mod addr {
    pub const ON_ACCESSORY4: u32 = 0x8010_C288;
    pub const ON_HIT_ACCESSORY4: u32 = 0x8010_C344;
    pub const TOAD_GONE: u32 = 0x8010_C3C0;
    pub const CLEANUP: u32 = 0x8010_C3F4;
    pub const ON_ENTER_HITLAG: u32 = 0x8010_C44C;
    pub const ON_EXIT_HITLAG: u32 = 0x8010_C47C;
    pub const ENTER: u32 = 0x8010_C4D4;
    pub const AIR_ENTER: u32 = 0x8010_C560;
    pub const LW_ANIM: u32 = 0x8010_C5FC;
    pub const AIR_LW_ANIM: u32 = 0x8010_C6B8;
    pub const LW_IASA: u32 = 0x8010_C774;
    pub const AIR_LW_IASA: u32 = 0x8010_C778;
    pub const LW_PHYS: u32 = 0x8010_C77C;
    pub const AIR_LW_PHYS: u32 = 0x8010_C7B0;
    pub const LW_COLL: u32 = 0x8010_C860;
    pub const AIR_LW_COLL: u32 = 0x8010_C89C;
    pub const TO_AIR: u32 = 0x8010_C8D8;
    pub const TO_GROUND: u32 = 0x8010_C9CC;
    pub const LW_HIT_ANIM: u32 = 0x8010_CAB4;
    pub const AIR_LW_HIT_ANIM: u32 = 0x8010_CB18;
    pub const LW_HIT_IASA: u32 = 0x8010_CB7C;
    pub const AIR_LW_HIT_IASA: u32 = 0x8010_CB80;
    pub const LW_HIT_PHYS: u32 = 0x8010_CB84;
    pub const AIR_LW_HIT_PHYS: u32 = 0x8010_CBA4;
    pub const LW_HIT_COLL: u32 = 0x8010_CBF4;
    pub const AIR_LW_HIT_COLL: u32 = 0x8010_CC30;
    pub const HIT_TO_AIR: u32 = 0x8010_CC6C;
    pub const HIT_TO_GROUND: u32 = 0x8010_CCD4;
    pub const HIT_HOOKS: u32 = 0x8010_CD44;
    pub const ON_HIT: u32 = 0x8010_CD88;
}

/// `u` offsets: his first aerial Toad of a jump spent (`xCC`) and Toad (`xD0`).
const AIR_USED: usize = 0xCC;
const TOAD: usize = 0xD0;
/// `mv` offset of his facing when the move started (`specialn_pe.facing_dir`, an int).
const MV_FACING: usize = 0x0;
/// `ftKb_DatAttrs`' Peach entries: the aerial Toad's speed divisor and air friction, its lift,
/// gravity and top falling speed, the bubble's push, and the bubble (bone, offset, size).
const PE_AIR_DIV: usize = 0x178;
const PE_AIR_FRICTION: usize = 0x17C;
const PE_AIR_LIFT: usize = 0x180;
const PE_GRAVITY: usize = 0x184;
const PE_TERMINAL: usize = 0x188;
const PE_PUSH: usize = 0x18C;
const PE_BUBBLE_BONE: usize = 0x3EC;
const PE_BUBBLE_OFFSET: usize = 0x3F0;
const PE_BUBBLE_SIZE: usize = 0x3FC;
/// `FtPart_R3rdNa`, Toad's bone (by index).
const PART_R3RD_NA: i32 = 44;
/// The script's variables (`cmd_vars`): the air lift's, the bubble's, Toad's and the
/// counter's spore's.
const CMD_PHYS: usize = 0;
const CMD_ANIM: usize = 1;
const CMD_ACCESSORY4: usize = 2;
const CMD_HIT_ANIM: usize = 3;
/// What the move keeps changing between the ground and the air.
const COLL_MF: u32 = 0x0C4C_508C;

impl Running<'_> {
    /// Toad made in his hand, `fn_8010C288` (once): with his damage hooks while he holds it,
    /// and his hitlag's.
    #[doc(alias = "fn_8010C288")]
    pub fn kb_pe_on_accessory4(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[CMD_ACCESSORY4] != 0 {
            return;
        }
        self.game.fighters[f.0].cmd_vars[CMD_ACCESSORY4] = 1;
        let hand = self.bone_joint(gobj, PART_R3RD_NA);
        let pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, hand, None);
        let facing = self.game.fighters[f.0].facing_dir;
        let toad = peachtoad::spawn(self, gobj, pos, PART_R3RD_NA, k::KIRBY_PEACH_TOAD, facing);
        let fp = &mut self.game.fighters[f.0];
        fp.u.set_gobj_at(TOAD, toad);
        if toad.is_some() {
            fp.death2_cb = Some(CodeRef(kb::DEATH3));
            fp.take_dmg_cb = Some(CodeRef(kb::DEATH1));
        }
        fp.accessory4_cb = None;
        fp.pre_hitlag_cb = Some(CodeRef(addr::ON_ENTER_HITLAG));
        fp.post_hitlag_cb = Some(CodeRef(addr::ON_EXIT_HITLAG));
    }

    /// The counter's spore, `fn_8010C344`: from his hand, a little above it and on the stage's
    /// plane.
    #[doc(alias = "fn_8010C344")]
    pub fn kb_pe_on_hit_accessory4(&mut self, gobj: GObjId) {
        let hand = self.bone_joint(gobj, PART_R3RD_NA);
        let mut pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, hand, None);
        pos.y = mem(fp::fadds(reg(pos.y), 2.5));
        pos.z = 0.0;
        let facing = self.game.fighter(gobj).facing_dir;
        peachtoadspore::spawn(self, gobj, pos, k::KIRBY_PEACH_TOAD_SPORE, facing);
        self.game.fighter_mut(gobj).accessory4_cb = None;
    }

    /// He forgets Toad, `ftKb_SpecialNPe_8010C3C0`, Toad going on after his hitlag first (his
    /// damage hooks stay).
    #[doc(alias = "ftKb_SpecialNPe_8010C3C0")]
    pub fn kb_pe_toad_gone(&mut self, gobj: GObjId) {
        self.kb_pe_exit_hitlag(gobj);
        self.game.fighter_mut(gobj).u.set_gobj_at(TOAD, None);
    }

    /// Toad taken away when he's hit or KO'd, `ftKb_SpecialNPe_8010C3F4`.
    #[doc(alias = "ftKb_SpecialNPe_8010C3F4")]
    pub fn kb_pe_cleanup(&mut self, gobj: GObjId) {
        if let Some(toad) = self.game.fighter(gobj).u.gobj_at(TOAD) {
            peachtoad::remove(self, toad);
            self.kb_pe_toad_gone(gobj);
        }
    }

    /// Toad pauses in his hitlag, `fn_8010C44C`.
    #[doc(alias = "fn_8010C44C")]
    pub fn kb_pe_enter_hitlag(&mut self, gobj: GObjId) {
        if let Some(toad) = self.game.fighter(gobj).u.gobj_at(TOAD) {
            peachtoad::hitlag_start(self, toad);
        }
    }

    /// Toad goes on after his hitlag, `ftKb_SpecialNPe_8010C47C`.
    #[doc(alias = "ftKb_SpecialNPe_8010C47C")]
    pub fn kb_pe_exit_hitlag(&mut self, gobj: GObjId) {
        if let Some(toad) = self.game.fighter(gobj).u.gobj_at(TOAD) {
            peachtoad::hitlag_end(self, toad);
        }
    }

    /// Whether he has left the move, `ftKb_SpecialNPe_8010C4AC` (true outside its four
    /// states).
    #[doc(alias = "ftKb_SpecialNPe_8010C4AC")]
    pub fn kb_pe_done(&self, gobj: GObjId) -> bool {
        let msid = self.game.fighter(gobj).motion_id;
        !(ms::PE_SPECIAL_LW..=ms::PE_SPECIAL_AIR_LW_HIT).contains(&msid)
    }

    /// The move's start: the state, the script's variables cleared, his facing kept, and
    /// Toad made when the script says.
    fn kb_pe_start(&mut self, gobj: GObjId, msid: i32) {
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars = [0; 4];
        let facing = fp::fctiwz(reg(fp.facing_dir));
        fp.set_mv_i32(MV_FACING, facing);
        fp.accessory4_cb = Some(CodeRef(addr::ON_ACCESSORY4));
    }

    /// Toad on the ground, `ftKb_SpecialNPe_8010C4D4`: no vertical speed.
    #[doc(alias = "ftKb_SpecialNPe_8010C4D4")]
    pub fn kb_pe_enter(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).self_vel.y = 0.0;
        self.kb_pe_start(gobj, ms::PE_SPECIAL_LW);
    }

    /// In the air, `ftKb_SpecialNPe_8010C560`: his horizontal speed divided by the
    /// attributes'.
    #[doc(alias = "ftKb_SpecialNPe_8010C560")]
    pub fn kb_pe_air_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let div = reg(self.kb_attr_f32(f, PE_AIR_DIV));
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.x = mem(fp::fdivs(reg(fp.self_vel.x), div));
        self.kb_pe_start(gobj, ms::PE_SPECIAL_AIR_LW);
    }

    /// Toad's bubble, `ftColl_8007B1B8` with the attributes' absorb shape: an attack meeting
    /// it calls [`addr::ON_HIT`].
    fn kb_pe_bubble(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let bone = self.kb_attr_i32(f, PE_BUBBLE_BONE);
        let offset = Vec3::new(
            self.kb_attr_f32(f, PE_BUBBLE_OFFSET),
            self.kb_attr_f32(f, PE_BUBBLE_OFFSET + 4),
            self.kb_attr_f32(f, PE_BUBBLE_OFFSET + 8),
        );
        let size = self.kb_attr_f32(f, PE_BUBBLE_SIZE);
        let joint = self.bone_joint(gobj, bone);
        let fp = &mut self.game.fighters[f.0];
        set_shield_hit(fp, CodeRef(addr::ON_HIT), joint, size, offset);
    }

    /// The stance (`ftKb_PeSpecialLw_Anim`, `ftKb_PeSpecialAirLw_Anim`): the bubble once the
    /// script opens it (`cmd_vars[1]` 1), with its push; none before; then waiting or a fall
    /// when the animation ends.
    #[doc(alias = "ftKb_PeSpecialLw_Anim")]
    #[doc(alias = "ftKb_PeSpecialAirLw_Anim")]
    fn kb_pe_stance(&mut self, gobj: GObjId, air: bool) {
        let f = self.game.fighter_id(gobj);
        match self.game.fighters[f.0].cmd_vars[CMD_ANIM] {
            1 => {
                self.game.fighters[f.0].cmd_vars[CMD_ANIM] = 2;
                self.kb_pe_bubble(gobj);
                let push = self.kb_attr_f32(f, PE_PUSH);
                let fp = &mut self.game.fighters[f.0];
                fp.x221B.x221B_b3 = true;
                fp.shield_unk0 = push;
                fp.shield_unk1 = push;
            }
            0 => self.game.fighters[f.0].x221B.x221B_b0 = false,
            _ => {}
        }
        if !self.anim_frames_remaining(gobj) {
            if air {
                self.enter_fall(gobj);
            } else {
                self.enter_wait(gobj);
            }
        }
    }

    /// `ftKb_PeSpecialAirLw_Phys`: once the script says (`cmd_vars[0]`), the first Toad of a
    /// jump lifts him and later ones stop his fall, then the move's gravity; before, plain
    /// gravity. Air friction, and the bubble follows.
    #[doc(alias = "ftKb_PeSpecialAirLw_Phys")]
    pub fn kb_pe_air_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let lift = self.kb_attr_f32(f, PE_AIR_LIFT);
        let (g, t) = (
            reg(self.kb_attr_f32(f, PE_GRAVITY)),
            reg(self.kb_attr_f32(f, PE_TERMINAL)),
        );
        let friction = reg(self.kb_attr_f32(f, PE_AIR_FRICTION));
        let fp = &mut self.game.fighters[f.0];
        if fp.cmd_vars[CMD_PHYS] >= 1 {
            if fp.cmd_vars[CMD_PHYS] == 1 {
                fp.cmd_vars[CMD_PHYS] = 2;
                if fp.u_i32(AIR_USED) == 0 {
                    fp.set_u_i32(AIR_USED, 1);
                    fp.self_vel.y = lift;
                } else {
                    fp.self_vel.y = 0.0;
                }
            }
            Self::fall(fp, g, t);
        } else {
            Self::fall_basic(fp);
        }
        Self::self_accel_deaccel(fp, friction);
        self.shield_follow(gobj);
    }

    /// The move's hooks again after a change between the ground and the air, with the bubble
    /// if it was open.
    fn kb_pe_hooks(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        if fp.u.gobj_at(TOAD).is_some() {
            fp.death2_cb = Some(CodeRef(kb::DEATH3));
            fp.take_dmg_cb = Some(CodeRef(kb::DEATH1));
        }
        fp.accessory4_cb = Some(CodeRef(addr::ON_ACCESSORY4));
        fp.pre_hitlag_cb = Some(CodeRef(addr::ON_ENTER_HITLAG));
        fp.post_hitlag_cb = Some(CodeRef(addr::ON_EXIT_HITLAG));
        if fp.cmd_vars[CMD_ANIM] == 2 {
            self.kb_pe_bubble(gobj);
            self.game.fighters[f.0].x221B.x221B_b3 = true;
        }
    }

    /// Off an edge, `ftKb_SpecialNPe_8010C8D8`: the air state from the same frame, the lift
    /// spent if the script had just asked for it.
    #[doc(alias = "ftKb_SpecialNPe_8010C8D8")]
    pub fn kb_pe_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::PE_SPECIAL_AIR_LW, COLL_MF);
        let fp = self.game.fighter_mut(gobj);
        if fp.cmd_vars[CMD_PHYS] == 1 {
            fp.cmd_vars[CMD_PHYS] = 2;
        }
        self.kb_pe_hooks(gobj);
    }

    /// Landing, `ftKb_SpecialNPe_8010C9CC`: his air Toad's lift back, the ground state from
    /// the same frame.
    #[doc(alias = "ftKb_SpecialNPe_8010C9CC")]
    pub fn kb_pe_to_ground(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).set_u_i32(AIR_USED, 0);
        self.fox_air_to_ground(gobj, ms::PE_SPECIAL_LW, COLL_MF);
        self.kb_pe_hooks(gobj);
    }

    /// The counter (`ftKb_PeSpecialLwHit_Anim`, `ftKb_PeSpecialAirLwHit_Anim`): the spore when
    /// the script says (`cmd_vars[3]`), then waiting or a fall when the animation ends.
    #[doc(alias = "ftKb_PeSpecialLwHit_Anim")]
    #[doc(alias = "ftKb_PeSpecialAirLwHit_Anim")]
    fn kb_pe_counter(&mut self, gobj: GObjId, air: bool) {
        let fp = self.game.fighter_mut(gobj);
        if fp.cmd_vars[CMD_HIT_ANIM] != 0 {
            fp.cmd_vars[CMD_HIT_ANIM] = 0;
            fp.accessory4_cb = Some(CodeRef(addr::ON_HIT_ACCESSORY4));
        }
        if !self.anim_frames_remaining(gobj) {
            if air {
                self.enter_fall(gobj);
            } else {
                self.enter_wait(gobj);
            }
        }
    }

    /// The states' empty interrupts.
    #[doc(alias = "ftKb_PeSpecialLw_IASA")]
    #[doc(alias = "ftKb_PeSpecialAirLw_IASA")]
    #[doc(alias = "ftKb_PeSpecialLwHit_IASA")]
    #[doc(alias = "ftKb_PeSpecialAirLwHit_IASA")]
    fn kb_pe_no_iasa(&mut self, _gobj: GObjId) {}

    /// `ftKb_PeSpecialLw_Phys`: friction, and the bubble follows.
    #[doc(alias = "ftKb_PeSpecialLw_Phys")]
    fn kb_pe_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
        self.shield_follow(gobj);
    }

    /// `ftKb_PeSpecialLwHit_Phys`: friction.
    #[doc(alias = "ftKb_PeSpecialLwHit_Phys")]
    fn kb_pe_hit_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// `ftKb_PeSpecialLw_Coll`: into the air off an edge.
    #[doc(alias = "ftKb_PeSpecialLw_Coll")]
    fn kb_pe_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_slide(gobj) {
            self.kb_pe_to_air(gobj);
        }
    }

    /// `ftKb_PeSpecialAirLw_Coll`: onto the ground on landing.
    #[doc(alias = "ftKb_PeSpecialAirLw_Coll")]
    fn kb_pe_air_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.kb_pe_to_ground(gobj);
        }
    }

    /// `ftKb_PeSpecialLwHit_Coll`: into the air off an edge in the counter.
    #[doc(alias = "ftKb_PeSpecialLwHit_Coll")]
    fn kb_pe_hit_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_slide(gobj) {
            self.kb_pe_hit_to_air(gobj);
        }
    }

    /// `ftKb_PeSpecialAirLwHit_Coll`: onto the ground on landing in the counter.
    #[doc(alias = "ftKb_PeSpecialAirLwHit_Coll")]
    fn kb_pe_air_hit_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.kb_pe_hit_to_ground(gobj);
        }
    }

    /// `ftKb_PeSpecialAirLwHit_Phys`: the move's gravity and air friction.
    #[doc(alias = "ftKb_PeSpecialAirLwHit_Phys")]
    pub fn kb_pe_air_hit_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let (g, t) = (
            reg(self.kb_attr_f32(f, PE_GRAVITY)),
            reg(self.kb_attr_f32(f, PE_TERMINAL)),
        );
        let friction = reg(self.kb_attr_f32(f, PE_AIR_FRICTION));
        let fp = &mut self.game.fighters[f.0];
        Self::fall(fp, g, t);
        Self::self_accel_deaccel(fp, friction);
    }

    /// Off an edge in the counter, `ftKb_SpecialNGw_8010CC6C`.
    #[doc(alias = "ftKb_SpecialNGw_8010CC6C")]
    pub fn kb_pe_hit_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::PE_SPECIAL_AIR_LW_HIT, COLL_MF);
        self.kb_pe_hit_hooks(gobj);
    }

    /// Landing in the counter, `ftKb_SpecialNGw_8010CCD4`: his air Toad's lift back.
    #[doc(alias = "ftKb_SpecialNGw_8010CCD4")]
    pub fn kb_pe_hit_to_ground(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).set_u_i32(AIR_USED, 0);
        self.fox_air_to_ground(gobj, ms::PE_SPECIAL_LW_HIT, COLL_MF);
        self.kb_pe_hit_hooks(gobj);
    }

    /// The counter's hooks, `ftKb_SpecialNGw_8010CD44`: for his hit while Toad is out, and his
    /// hitlag.
    #[doc(alias = "ftKb_SpecialNGw_8010CD44")]
    pub fn kb_pe_hit_hooks(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.u.gobj_at(TOAD).is_some() {
            fp.death2_cb = Some(CodeRef(kb::DEATH3));
            fp.take_dmg_cb = Some(CodeRef(kb::DEATH1));
        }
        fp.pre_hitlag_cb = Some(CodeRef(addr::ON_ENTER_HITLAG));
        fp.post_hitlag_cb = Some(CodeRef(addr::ON_EXIT_HITLAG));
    }

    /// An attack met Toad's bubble, `fn_8010CD88`: the counter from frame 9, facing where the
    /// hit came from, and Toad's own reaction.
    #[doc(alias = "fn_8010CD88")]
    pub fn kb_pe_on_hit(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let facing = fp::fctiwz(reg(fp.specialn_facing_dir));
        fp.set_mv_i32(MV_FACING, facing);
        let msid = if fp.ground_or_air == GA_GROUND {
            ms::PE_SPECIAL_LW_HIT
        } else {
            ms::PE_SPECIAL_AIR_LW_HIT
        };
        self.change_motion_state(gobj, msid, 0, 9.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        if let Some(toad) = self.game.fighter(gobj).u.gobj_at(TOAD) {
            peachtoad::hit(self, toad);
        }
        self.kb_pe_hit_hooks(gobj);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::ON_ACCESSORY4 => g.kb_pe_on_accessory4(gobj),
        addr::ON_HIT_ACCESSORY4 => g.kb_pe_on_hit_accessory4(gobj),
        addr::TOAD_GONE => g.kb_pe_toad_gone(gobj),
        addr::CLEANUP => g.kb_pe_cleanup(gobj),
        addr::ON_ENTER_HITLAG => g.kb_pe_enter_hitlag(gobj),
        addr::ON_EXIT_HITLAG => g.kb_pe_exit_hitlag(gobj),
        addr::ENTER => g.kb_pe_enter(gobj),
        addr::AIR_ENTER => g.kb_pe_air_enter(gobj),
        addr::LW_ANIM => g.kb_pe_stance(gobj, false),
        addr::AIR_LW_ANIM => g.kb_pe_stance(gobj, true),
        addr::LW_IASA | addr::AIR_LW_IASA | addr::LW_HIT_IASA | addr::AIR_LW_HIT_IASA => {
            g.kb_pe_no_iasa(gobj)
        }
        addr::LW_PHYS => g.kb_pe_phys(gobj),
        addr::AIR_LW_PHYS => g.kb_pe_air_phys(gobj),
        addr::LW_COLL => g.kb_pe_coll(gobj),
        addr::AIR_LW_COLL => g.kb_pe_air_coll(gobj),
        addr::TO_AIR => g.kb_pe_to_air(gobj),
        addr::TO_GROUND => g.kb_pe_to_ground(gobj),
        addr::LW_HIT_ANIM => g.kb_pe_counter(gobj, false),
        addr::AIR_LW_HIT_ANIM => g.kb_pe_counter(gobj, true),
        addr::LW_HIT_PHYS => g.kb_pe_hit_phys(gobj),
        addr::AIR_LW_HIT_PHYS => g.kb_pe_air_hit_phys(gobj),
        addr::LW_HIT_COLL => g.kb_pe_hit_coll(gobj),
        addr::AIR_LW_HIT_COLL => g.kb_pe_air_hit_coll(gobj),
        addr::HIT_TO_AIR => g.kb_pe_hit_to_air(gobj),
        addr::HIT_TO_GROUND => g.kb_pe_hit_to_ground(gobj),
        addr::HIT_HOOKS => g.kb_pe_hit_hooks(gobj),
        addr::ON_HIT => g.kb_pe_on_hit(gobj),
        _ => return false,
    }
    true
}
