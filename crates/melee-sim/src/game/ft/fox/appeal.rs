//! The Corneria taunt (`ftfoxappeals.c`): on Corneria, Fox's and Falco's side taunt calls
//! their team in three parts, once per match.

use super::super::super::code::Val;
use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::{KIND_FALCO, KIND_FOX, PAD_DPAD_DOWN, addr, ext, ms, mv};
use crate::hsd::gobj::GObjId;

/// The attack id the taunt counts under (`FTFOX_APPEALS_ATTACKID`).
const ATTACK_ID: i32 = 0x72;

/// The taunt's three parts, facing right and left (`ASID_AppealS`).
const STATES: [[i32; 3]; 2] = [
    [ms::APPEAL_S_START_R, ms::APPEAL_S_R, ms::APPEAL_S_END_R],
    [ms::APPEAL_S_START_L, ms::APPEAL_S_L, ms::APPEAL_S_END_L],
];

impl Running<'_> {
    /// How often a player used the taunt (`pl_800386D8` on their action stats). The stats
    /// are the player block's, which a stand-in's log can't name.
    fn fox_appeal_count(&mut self, slot: i32) -> i32 {
        let _ = self.game.player_action_stats(slot);
        self.ext(ext::PL_800386D8, &[Val::None, Val::Int(ATTACK_ID)])
            .int()
    }

    /// Whether the fighter's player used the taunt and Corneria lets it count,
    /// `ftFx_AppealS_CheckIfUsed`.
    #[doc(alias = "ftFx_AppealS_CheckIfUsed")]
    pub fn fox_appeal_s_used(&mut self, f: super::super::super::handles::FighterId) -> bool {
        let slot = i32::from(self.game.fighters[f.0].player_idx);
        self.fox_appeal_count(slot) != 0 && self.ext(ext::GRCORNERIA_801E2D14, &[]).int() != 0
    }

    /// The taunt on Corneria, `ftFx_AppealS_CheckInput`: Fox or Falco, the side taunt, and
    /// nobody has used it yet.
    #[doc(alias = "ftFx_AppealS_CheckInput")]
    pub fn fox_appeal_s_check_input(&mut self, gobj: GObjId) -> bool {
        let kind = self.game.fighter(gobj).kind;
        if !(kind == KIND_FOX || kind == KIND_FALCO) {
            return false;
        }
        if !self.corneria_taunt_stage() {
            return false;
        }
        let fp = self.game.fighter(gobj);
        if fp.input.held_buttons[0] & PAD_DPAD_DOWN != 0 || fp.x682 != 1 {
            return false;
        }
        for i in 0..6 {
            if self.fox_appeal_count(i) != 0 {
                return false;
            }
        }
        self.fox_appeal_s_enter(gobj);
        let fp = self.game.fighter(gobj);
        let (slot, sub) = (i32::from(fp.player_idx), i32::from(fp.is_sub_fighter));
        self.ext(ext::PL_80040120, &[Val::Int(slot), Val::Int(sub)]);
        true
    }

    /// `ftFx_AppealS_Enter`: the taunt's first part, by facing.
    #[doc(alias = "ftFx_AppealS_Enter")]
    pub fn fox_appeal_s_enter(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(mv::APPEAL_COUNT, 0);
        let left = i32::from(fp.facing_dir != 1.0);
        fp.set_mv_i32(mv::APPEAL_FACING, left);
        fp.x2210 = [0; 4];
        let msid = STATES[left as usize][0];
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
    }

    /// `ftFx_AppealS_Anim`: the team's call on the script's cue, then each part in turn.
    #[doc(alias = "ftFx_AppealS_Anim")]
    pub fn fox_appeal_s_anim(&mut self, gobj: GObjId) {
        // `ftCheckThrowB3`.
        if self.game.fighter(gobj).throw_flag(3) {
            self.game.fighter_mut(gobj).set_throw_flag(3, false);
            let check = match self.game.fighter(gobj).kind {
                KIND_FOX => Some(ext::GRCORNERIA_801E2B80),
                KIND_FALCO => Some(ext::GRCORNERIA_801E2C34),
                _ => None,
            };
            if let Some(c) = check
                && self.ext(c, &[]).int() != 0
            {
                self.game.fighter_mut(gobj).death1_cb =
                    Some(CodeRef(addr::APPEAL_S_ON_TAKE_DAMAGE));
            }
        }
        if !self.anim_frames_remaining(gobj) {
            let fp = self.game.fighter_mut(gobj);
            let n = fp.mv_i32(mv::APPEAL_COUNT).wrapping_add(1);
            fp.set_mv_i32(mv::APPEAL_COUNT, n);
            if n >= 3 {
                self.ext_gobj(ext::FT_8008A324, gobj);
                return;
            }
            let facing = fp.mv_i32(mv::APPEAL_FACING);
            let msid = STATES[facing as usize][n as usize];
            self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        }
    }

    /// `ftFx_AppealS_Coll`.
    #[doc(alias = "ftFx_AppealS_Coll")]
    pub fn fox_appeal_s_coll(&mut self, gobj: GObjId) {
        self.ground_coll_teeter(gobj);
    }

    /// Hit during the call, `ftFx_AppealS_OnTakeDamage` (`death1_cb`): the call is cut off.
    #[doc(alias = "ftFx_AppealS_OnTakeDamage")]
    pub fn fox_appeal_s_on_take_damage(&mut self, gobj: GObjId) {
        self.ext(ext::GRCORNERIA_801E2AF4, &[]);
        self.game.fighter_mut(gobj).death1_cb = None;
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::APPEAL_S_ENTER => g.fox_appeal_s_enter(gobj),
        0x800E_5B18 => g.fox_appeal_s_anim(gobj),
        0x800E_5C38 => g.fox_no_iasa(gobj),
        0x800E_5C3C => g.fox_ground_phys(gobj),
        0x800E_5C5C => g.fox_appeal_s_coll(gobj),
        addr::APPEAL_S_ON_TAKE_DAMAGE => g.fox_appeal_s_on_take_damage(gobj),
        _ => return false,
    }
    true
}

pub(super) fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    Some(match f {
        addr::APPEAL_S_CHECK_INPUT => {
            let g = args[0].gobj().expect("a null fighter");
            Val::Int(i32::from(r.fox_appeal_s_check_input(g)))
        }
        addr::APPEAL_S_CHECK_IF_USED => {
            let Val::Fighter(fid) = args[0] else {
                panic!(
                    "ftFx_AppealS_CheckIfUsed takes a fighter, not {:?}",
                    args[0]
                );
            };
            Val::Int(i32::from(r.fox_appeal_s_used(fid)))
        }
        _ => return None,
    })
}
