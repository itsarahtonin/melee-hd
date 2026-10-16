//! Pikachu's and Pichu's copies (`ftkirbyspecialpikachu.c`): the Thunder Jolt thrown ahead of
//! him when the script raises its flag, on the ground or in the air, as Pikachu's own
//! (`ft::pikachu::special_n`) with Kirby's states and attributes for each copy.

use gekko_fp as fp;

use super::super::super::it::kinds::pikachutjoltground;
use super::super::super::state::Running;
use super::{attr, ms, u};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// The copies' function addresses.
pub(super) mod addr {
    pub const ENTER: u32 = 0x800F_9FD4;
    pub const AIR_ENTER: u32 = 0x800F_A064;
    pub const N_ANIM: u32 = 0x800F_A0F4;
    pub const AIR_N_ANIM: u32 = 0x800F_A254;
    pub const N_IASA: u32 = 0x800F_A418;
    pub const AIR_N_IASA: u32 = 0x800F_A41C;
    pub const N_PHYS: u32 = 0x800F_A420;
    pub const AIR_N_PHYS: u32 = 0x800F_A440;
    pub const N_COLL: u32 = 0x800F_A460;
    pub const AIR_N_COLL: u32 = 0x800F_A4F0;
}

/// The copied kinds.
const KIND_PIKACHU: i32 = 12;
const KIND_PICHU: i32 = 23;
/// The flags the throw keeps between the ground and the air (`coll_mf`).
const COLL_FLAGS: u32 = 0x0C4C_5082;
/// The throws' sounds, Pikachu's and Pichu's.
const SFX_PK: i32 = 0x3A9CC;
const SFX_PC: i32 = 0x382B3;

/// A copy's attribute offsets for the throw: its offsets on the ground and in the air, the
/// aerial throw's landing lag, the jolt's item kind and the sound.
struct Throw {
    ground: (usize, usize),
    air: (usize, usize),
    landing_lag: usize,
    itkind: usize,
    sfx: i32,
}

const PK: Throw = Throw {
    ground: (attr::PK_OFFSET_X, attr::PK_OFFSET_Y),
    air: (attr::PK_AIR_OFFSET_X, attr::PK_AIR_OFFSET_Y),
    landing_lag: attr::PK_AIR_LANDING_LAG,
    itkind: attr::PK_ITKIND,
    sfx: SFX_PK,
};

const PC: Throw = Throw {
    ground: (attr::PC_OFFSET_X, attr::PC_OFFSET_Y),
    air: (attr::PC_AIR_OFFSET_X, attr::PC_AIR_OFFSET_Y),
    landing_lag: attr::PC_AIR_LANDING_LAG,
    itkind: attr::PC_ITKIND,
    sfx: SFX_PC,
};

/// Pikachu's copy's or Pichu's.
fn throw_of(kind: i32) -> Option<&'static Throw> {
    match kind {
        KIND_PIKACHU => Some(&PK),
        KIND_PICHU => Some(&PC),
        _ => None,
    }
}

impl Running<'_> {
    /// The throw's state for his copy: Pikachu's for any but Pichu's.
    fn kb_pk_state(&self, gobj: GObjId, pikachu: i32, pichu: i32) -> i32 {
        if self.game.fighter(gobj).u_i32(u::HAT_KIND) == KIND_PICHU {
            pichu
        } else {
            pikachu
        }
    }

    /// The throw's start (`doEnter`).
    fn kb_pk_start(&mut self, gobj: GObjId, pikachu: i32, pichu: i32) {
        let msid = self.kb_pk_state(gobj, pikachu, pichu);
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars = [0; 4];
        self.anim_update(gobj);
    }

    /// The throw on the ground, `ftKb_SpecialNPk_800F9FD4`.
    #[doc(alias = "ftKb_SpecialNPk_800F9FD4")]
    pub fn kb_pk_enter(&mut self, gobj: GObjId) {
        self.kb_pk_start(gobj, ms::PK_SPECIAL_N, ms::PC_SPECIAL_N);
    }

    /// In the air, `ftKb_SpecialNPk_800FA064`.
    #[doc(alias = "ftKb_SpecialNPk_800FA064")]
    pub fn kb_pk_air_enter(&mut self, gobj: GObjId) {
        self.kb_pk_start(gobj, ms::PK_SPECIAL_AIR_N, ms::PC_SPECIAL_AIR_N);
    }

    /// The throw's frame: when the script raises its flag the first time, his copy's jolt
    /// from its offset ahead (scaled with him) and its sound. Both throws spawn the ground
    /// throw's item kind.
    fn kb_pk_throw(&mut self, gobj: GObjId, air: bool) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        if fp.cmd_vars[0] != 1 {
            return;
        }
        fp.cmd_vars[0] = 0;
        if fp.cmd_vars[1] != 0 {
            return;
        }
        fp.cmd_vars[1] = 1;
        let Some(t) = throw_of(fp.u_i32(u::HAT_KIND)) else {
            return;
        };
        let (offset_x, offset_y) = if air { t.air } else { t.ground };
        let ox = reg(self.kb_attr_f32(f, offset_x));
        let oy = reg(self.kb_attr_f32(f, offset_y));
        let fp = &self.game.fighters[f.0];
        let scale = reg(fp.x34_scale.y);
        let pos = Vec3::new(
            mem(fp::fmadds(
                scale,
                fp::fmuls(ox, reg(fp.facing_dir)),
                reg(fp.cur_pos.x),
            )),
            mem(fp::fmadds(oy, scale, reg(fp.cur_pos.y))),
            0.0,
        );
        let facing = fp.facing_dir;
        let kind = self.kb_attr_i32(f, t.itkind);
        pikachutjoltground::spawn(self, gobj, pos, facing, kind);
        self.fighter_play_sfx(f, t.sfx, 127, 64);
    }

    /// `ftKb_PkSpecialN_Anim`: the throw, and waiting when it ends.
    #[doc(alias = "ftKb_PkSpecialN_Anim")]
    pub fn kb_pk_anim(&mut self, gobj: GObjId) {
        self.kb_pk_throw(gobj, false);
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftKb_PkSpecialAirN_Anim`: the throw, and when it ends a fall, helpless when his
    /// copy's landing lag isn't zero. Only Pikachu's and Pichu's copies reach these states:
    /// for another the original reads a register it never set (the landing lag), which the
    /// port refuses.
    #[doc(alias = "ftKb_PkSpecialAirN_Anim")]
    pub fn kb_pk_air_anim(&mut self, gobj: GObjId) {
        self.kb_pk_throw(gobj, true);
        let f = self.game.fighter_id(gobj);
        let kind = self.game.fighters[f.0].u_i32(u::HAT_KIND);
        let lag = throw_of(kind).map(|t| self.kb_attr_f32(f, t.landing_lag));
        if !self.anim_frames_remaining(gobj) {
            let lag = lag.unwrap_or_else(|| {
                panic!("ftKb_PkSpecialAirN_Anim reads an unset landing lag for copy kind {kind}")
            });
            if reg(lag) == 0.0 {
                self.enter_fall(gobj);
            } else {
                self.fall_special(gobj, 1.0, lag);
            }
        }
    }

    /// The copies' states' empty interrupts.
    #[doc(alias = "ftKb_PkSpecialN_IASA")]
    #[doc(alias = "ftKb_PkSpecialAirN_IASA")]
    pub fn kb_pk_no_iasa(&mut self, _gobj: GObjId) {}

    /// `ftKb_PkSpecialN_Phys`: friction.
    #[doc(alias = "ftKb_PkSpecialN_Phys")]
    pub fn kb_pk_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// `ftKb_PkSpecialAirN_Phys`: the air's physics.
    #[doc(alias = "ftKb_PkSpecialAirN_Phys")]
    pub fn kb_pk_air_phys(&mut self, gobj: GObjId) {
        self.air_physics(gobj);
    }

    /// `ftKb_PkSpecialN_Coll`: off a ledge, the aerial throw from the same frame.
    #[doc(alias = "ftKb_PkSpecialN_Coll")]
    pub fn kb_pk_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            let f = self.game.fighter_id(gobj);
            self.go_airborne(f);
            let frame = self.game.fighters[f.0].cur_anim_frame;
            let msid = self.kb_pk_state(gobj, ms::PK_SPECIAL_AIR_N, ms::PC_SPECIAL_AIR_N);
            self.change_motion_state(gobj, msid, COLL_FLAGS, frame, 1.0, 0.0, None);
        }
    }

    /// `ftKb_PkSpecialAirN_Coll`: landing, the grounded throw from the same frame, without
    /// falling speed.
    #[doc(alias = "ftKb_PkSpecialAirN_Coll")]
    pub fn kb_pk_air_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.land(gobj);
            let fp = self.game.fighter_mut(gobj);
            fp.self_vel.y = 0.0;
            let frame = fp.cur_anim_frame;
            let msid = self.kb_pk_state(gobj, ms::PK_SPECIAL_N, ms::PC_SPECIAL_N);
            self.change_motion_state(gobj, msid, COLL_FLAGS, frame, 1.0, 0.0, None);
        }
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::ENTER => g.kb_pk_enter(gobj),
        addr::AIR_ENTER => g.kb_pk_air_enter(gobj),
        addr::N_ANIM => g.kb_pk_anim(gobj),
        addr::AIR_N_ANIM => g.kb_pk_air_anim(gobj),
        addr::N_IASA | addr::AIR_N_IASA => g.kb_pk_no_iasa(gobj),
        addr::N_PHYS => g.kb_pk_phys(gobj),
        addr::AIR_N_PHYS => g.kb_pk_air_phys(gobj),
        addr::N_COLL => g.kb_pk_coll(gobj),
        addr::AIR_N_COLL => g.kb_pk_air_coll(gobj),
        _ => return false,
    }
    true
}
