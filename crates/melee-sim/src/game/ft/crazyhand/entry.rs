//! Crazy Hand's entrance (`ftcrazyhandgrabunk1b174.c`): he flies in from his place while the
//! cutscene camera circles him and the nearest opponent (Master Hand's script, its interest
//! on Crazy Hand's waist), then waits.
//!
//! The script sets orbits without all their parts, which the cutscene camera's orbit reads
//! from stack words nothing wrote (`camera::orbit_leftover`): his waist's callback
//! (`fn_8015B4EC`) has Master Hand's frame (`ftMh_MS_343_80151428`'s 0x18 bytes, both calling
//! `ftBossLib_GetFighterGObj`), so the words are Master Hand's.

use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::super::masterhand::{PART_WAIST_N, u};
use super::{KIND_CRAZY_HAND, addr, attr, ms, mv};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// The entrance's roar, `ft_800881D8`'s sound.
const SFX_ENTRY: i32 = 320002;

impl Running<'_> {
    /// The entrance, `ftCh_GrabUnk1_8015B174`: at his place, facing right, the camera's
    /// script as his accessory, the roar.
    #[doc(alias = "ftCh_GrabUnk1_8015B174")]
    pub fn ch_entry_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.ch_enter(gobj, ms::TAG_FAIL);
        let pos = self.ch_da_point(f, attr::X18, attr::X1C);
        let fp = &mut self.game.fighters[f.0];
        fp.cur_pos = pos;
        fp.accessory4_cb = Some(CodeRef(addr::FN_8015B2C0));
        fp.set_mv_i32(mv::X0, 0);
        self.fighter_play_track(f, super::super::sound::Track::Voice, SFX_ENTRY, 127, 64);
        self.boss_face(gobj, 1.0);
    }

    /// `ftCh_TagFail_Anim` (the entrance's): waiting once it ends.
    #[doc(alias = "ftCh_TagFail_Anim")]
    pub fn ch_entry_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.game.fighter_mut(gobj).set_u_i32(u::WAIT, ms::WAIT1_0);
            self.ch_return(gobj);
        }
    }

    /// The entrance's camera script, `fn_8015B2C0`: Master Hand's, its interest on Crazy
    /// Hand's waist (`fn_8015B4EC`).
    #[doc(alias = "fn_8015B2C0")]
    pub fn ch_entry_camera(&mut self, gobj: GObjId) {
        self.boss_entry_camera(gobj, addr::FN_8015B4EC);
    }

    /// The camera's point on Crazy Hand's waist, `fn_8015B4EC`; whether he's there.
    #[doc(alias = "fn_8015B4EC")]
    pub fn ch_waist_point(&mut self) -> Option<Vec3> {
        let g = self.boss_gobj(KIND_CRAZY_HAND)?;
        let parts = self.game.fighter(g).parts.expect("a fighter without parts");
        let j = self.game.fighter_parts[parts.0][PART_WAIST_N].joint;
        Some(crate::lb::joint::world_position(
            &mut self.game.objs,
            self.data,
            j,
            None,
        ))
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::GRAB_UNK1_8015B174 => g.ch_entry_enter(gobj),
        addr::TAG_FAIL_ANIM => g.ch_entry_anim(gobj),
        addr::FN_8015B2C0 => g.ch_entry_camera(gobj),
        _ => return false,
    }
    true
}
