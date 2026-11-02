//! Master Hand's entrance (`ftmasterhandentry.c`): he flies in from his place while the
//! cutscene camera circles him and the nearest opponent, then waits.
//!
//! The camera's script sets an orbit's distance and yaw without its pitch, which the
//! cutscene camera's orbit reads from stack slots nothing wrote (`Camera_8002E234`); that
//! stops the simulation by name until task 4's probe decides the model.

use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::{PART_WAIST_N, addr, attr, ms, mv, u};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// The entrance's roar, `ft_800881D8`'s sound.
const SFX_ENTRY: i32 = 320001;

impl Running<'_> {
    /// The entrance, `ftMh_MS_343_801510B0`: at his place, facing left, the camera's script
    /// as his accessory, the roar.
    #[doc(alias = "ftMh_MS_343_801510B0")]
    pub fn mh_entry_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.change_motion_state(gobj, ms::ENTRY, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        let pos = self.mh_da_vec2(f, attr::X30);
        let fp = &mut self.game.fighters[f.0];
        fp.cur_pos.x = pos.x;
        fp.cur_pos.y = pos.y;
        fp.cur_pos.z = 0.0;
        fp.accessory4_cb = Some(CodeRef(addr::MS_343_801511FC));
        fp.set_mv_i32(mv::X0, 0);
        self.fighter_play_track(f, super::super::sound::Track::Voice, SFX_ENTRY, 127, 64);
        self.boss_face(gobj, -1.0);
    }

    /// `ftMh_Entry_Anim`: waiting once it ends.
    #[doc(alias = "ftMh_Entry_Anim")]
    pub fn mh_entry_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.game.fighter_mut(gobj).set_u_i32(u::WAIT, ms::WAIT1_0);
            self.mh_return(gobj);
        }
    }

    /// The entrance's camera script, a step a frame (`mv.x0`, waits in `mv.x8`),
    /// `ftMh_MS_343_801511FC`; the interest follows his waist (`ftMh_MS_343_80151428`).
    #[doc(alias = "ftMh_MS_343_801511FC")]
    pub fn mh_entry_camera(&mut self, gobj: GObjId) {
        self.boss_entry_camera(gobj, addr::MS_343_80151428);
    }

    /// The camera's point on Master Hand's waist, `ftMh_MS_343_80151428`; whether he's there.
    #[doc(alias = "ftMh_MS_343_80151428")]
    pub fn mh_waist_point(&mut self) -> Option<Vec3> {
        let g = self.boss_gobj(super::KIND_MASTER_HAND)?;
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
        addr::MS_343_801510B0 => g.mh_entry_enter(gobj),
        addr::ENTRY_ANIM => g.mh_entry_anim(gobj),
        addr::MS_343_801511FC => g.mh_entry_camera(gobj),
        _ => return false,
    }
    true
}
