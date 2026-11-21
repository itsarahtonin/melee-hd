//! The hands' entrance (`ftmasterhandentry.c`, `ftcrazyhandgrabunk1b174.c`): he flies in from
//! his place while the cutscene camera circles him and the nearest opponent, then waits.
//! Crazy Hand's camera script is Master Hand's, its interest on his own waist.
//!
//! The camera's script sets an orbit's distance and yaw without its pitch, which the
//! cutscene camera's orbit reads from stack slots nothing wrote (`Camera_8002E234`,
//! `camera::orbit_leftover`): Crazy Hand's waist callback (`fn_8015B4EC`) has Master Hand's
//! frame (`ftMh_MS_343_80151428`'s 0x18 bytes, both calling `ftBossLib_GetFighterGObj`), so the
//! words are the same for both.

use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::{HAND, Hand, PART_WAIST_N, addr, mv, u};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

impl Running<'_> {
    /// The entrance, `ftMh_MS_343_801510B0` and `ftCh_GrabUnk1_8015B174`: at his place,
    /// facing his way, the camera's script as his accessory, the roar (`ft_800881D8`'s).
    #[doc(alias = "ftMh_MS_343_801510B0")]
    #[doc(alias = "ftCh_GrabUnk1_8015B174")]
    pub fn hand_entry_enter(&mut self, gobj: GObjId, h: &Hand) {
        let f = self.game.fighter_id(gobj);
        self.hand_enter(gobj, h.entry);
        let pos = h.point(self, f, h.home);
        let fp = &mut self.game.fighters[f.0];
        fp.cur_pos = pos;
        fp.accessory4_cb = Some(CodeRef(h.entry_camera));
        fp.set_mv_i32(mv::X0, 0);
        self.fighter_play_track(f, super::super::sound::Track::Voice, h.entry_sfx, 127, 64);
        self.boss_face(gobj, h.entry_facing);
    }

    /// `ftMh_Entry_Anim` and `ftCh_TagFail_Anim` (the entrance's): waiting once it ends.
    #[doc(alias = "ftMh_Entry_Anim")]
    #[doc(alias = "ftCh_TagFail_Anim")]
    pub fn hand_entry_anim(&mut self, gobj: GObjId, h: &Hand) {
        if !self.anim_frames_remaining(gobj) {
            self.game.fighter_mut(gobj).set_u_i32(u::WAIT, h.wait1);
            self.hand_return(gobj, h);
        }
    }

    /// The entrance's camera script, a step a frame (`mv.x0`, waits in `mv.x8`),
    /// `ftMh_MS_343_801511FC` and `fn_8015B2C0`; the interest follows his waist.
    #[doc(alias = "ftMh_MS_343_801511FC")]
    #[doc(alias = "ftLib_GetPlayerIndex")]
    #[doc(alias = "fn_8015B2C0")]
    pub fn hand_entry_camera(&mut self, gobj: GObjId, h: &Hand) {
        self.boss_entry_camera(gobj, h.waist);
    }

    /// The camera's point on his waist, `ftMh_MS_343_80151428` and `fn_8015B4EC`; whether he's
    /// there.
    #[doc(alias = "ftMh_MS_343_80151428")]
    #[doc(alias = "fn_8015B4EC")]
    pub fn hand_waist_point(&mut self, h: &Hand) -> Option<Vec3> {
        let g = self.boss_gobj(h.kind.id())?;
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

#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::ENTRY_ENTER => g.hand_entry_enter(gobj, &HAND),
        addr::ENTRY_ANIM => g.hand_entry_anim(gobj, &HAND),
        addr::ENTRY_CAMERA => g.hand_entry_camera(gobj, &HAND),
        _ => return false,
    }
    true
}
