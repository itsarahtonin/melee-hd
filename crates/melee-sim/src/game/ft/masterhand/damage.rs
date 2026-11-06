//! Master Hand beaten (`ftmasterhanddamage0.c`): a stamina hand out of HP (the
//! single-player setups; a debug VS match never sets his stamina) drops what he holds, cries
//! out and falls away, with the crowd's cheers, until he's knocked out.

use super::super::super::state::Running;
use super::{attr, ms, mv};
use crate::hsd::gobj::GObjId;

/// The beaten hand's cry and its sound.
const SFX_BEATEN_VOICE: i32 = 320023;
const SFX_BEATEN: i32 = 320024;
/// The crowd's cheers (`lbAudioAx_8002438C`): the first `x144` falls' and the next `x148`'s.
const SFX_CHEER: i32 = 530002;
const SFX_CHEER2: i32 = 530003;

impl Running<'_> {
    /// Beaten, `ftMh_MS_343_80151484` (`fn_800C8E74`'s for him): still, his lasers ended, the
    /// beams' hums stopped, a held fighter let go, falling (`x14C`, `x154`) for `x15C` frames,
    /// the cries.
    // Ports `ftMh_MS_343_80151484`; aliased once a check reaches it.
    pub fn mh_beaten(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.game.fighters[f.0].cmd_vars[0] = 0;
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel = crate::vec::Vec3::new(0.0, 0.0, 0.0);
        for off in [mv::X34, mv::X38, mv::X3C, mv::X40] {
            let l = self.game.fighters[f.0].mv.gobj_at(off);
            super::super::super::it::kinds::masterhandlaser::end(self, l);
        }
        let fp = &mut self.game.fighters[f.0];
        for off in [mv::X34, mv::X38, mv::X3C, mv::X40] {
            fp.mv.set_gobj_at(off, None);
        }
        for off in [mv::X28, mv::X2C, mv::X30] {
            let h = self.game.fighters[f.0].mv_i32(off);
            self.sfx_stop(h);
        }
        if self.game.fighters[f.0].mv_i32(mv::HOLDING) != 0 {
            let victim = self.game.fighters[f.0]
                .victim_gobj
                .expect("a beaten hand holding no one");
            self.mh_capture_cut(victim);
        }
        let (vy, vz, frames) = (
            self.mh_da_f32(f, attr::X14C),
            self.mh_da_f32(f, attr::X154),
            self.mh_da_f32(f, attr::X15C),
        );
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.y = vy;
        fp.self_vel.z = vz;
        fp.set_mv_i32(mv::X8, gekko_fp::fctiwz(crate::fpu::reg(frames)));
        self.mh_enter(gobj, ms::DAMAGE);
        self.fighter_play_track(
            f,
            super::super::sound::Track::Voice,
            SFX_BEATEN_VOICE,
            127,
            64,
        );
        self.fighter_play_sfx(f, SFX_BEATEN, 127, 64);
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_i32(mv::X74, 0);
        fp.set_mv_i32(mv::X78, 0);
    }

    /// Falling beaten, `ftMh_Damage_Anim` and `ftMh_MS_345_Anim`: a cheer on the script's
    /// flag, the second fall once the first ends, knocked out once the frames run out.
    // Ports `ftMh_Damage_Anim`; aliased once a check reaches it.
    // Ports `ftMh_MS_345_Anim`; aliased once a check reaches it.
    pub fn mh_damage_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[0] != 0 {
            let (first, second) = (self.mh_da_i32(f, attr::X144), self.mh_da_i32(f, attr::X148));
            let fp = &mut self.game.fighters[f.0];
            let n = fp.mv_i32(mv::X74).wrapping_add(1);
            fp.set_mv_i32(mv::X74, n);
            if n <= first {
                self.sfx_play_cheer(SFX_CHEER);
            } else {
                let m = fp.mv_i32(mv::X78).wrapping_add(1);
                fp.set_mv_i32(mv::X78, m);
                if m <= second {
                    self.sfx_play_cheer(SFX_CHEER2);
                }
            }
            self.game.fighters[f.0].cmd_vars[0] = 0;
        }
        if self.game.fighters[f.0].mv_i32(mv::X8) > 0 && !self.anim_frames_remaining(gobj) {
            self.mh_enter(gobj, ms::DAMAGE2);
        }
        let fp = &mut self.game.fighters[f.0];
        let n = fp.mv_i32(mv::X8).wrapping_sub(1);
        fp.set_mv_i32(mv::X8, n);
        if n == 0 {
            self.remove_with_partner(gobj, false);
        }
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        super::addr::MS_343_80151484 => g.mh_beaten(gobj),
        super::addr::DAMAGE_ANIM | super::addr::MS_345_ANIM => g.mh_damage_anim(gobj),
        _ => return false,
    }
    true
}
