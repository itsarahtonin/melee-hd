//! A hand beaten (`ftmasterhanddamage0.c`, `ftcrazyhandtaggrab.c`): a stamina hand out of HP
//! (the single-player setups; a debug VS match never sets his stamina) drops what he holds,
//! cries out and falls away, with the crowd's cheers, until he's knocked out.

use gekko_fp as fp;

use super::super::super::state::Running;
use super::{HAND, Hand, mv};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;

/// The crowd's cheers (`lbAudioAx_8002438C`): the first falls' and the next's.
const SFX_CHEER: i32 = 530002;
const SFX_CHEER2: i32 = 530003;

/// A hand's defeat: its states, attributes, variables and sound.
pub struct Beaten {
    /// The two falls' states (`ftBossLib_8015C2E0` and `8015C31C` test for either).
    pub states: [i32; 2],
    /// The attributes: the fall's speed up and toward the screen and its frames; its
    /// acceleration each frame; how many falls cheer the first way and the second.
    pub fall: [usize; 3],
    pub accel: [usize; 2],
    pub cheer_counts: [usize; 2],
    /// The cheers so far, in `mv`.
    pub cheers: [usize; 2],
    /// The cry's sound (`ft_PlaySFX`; the voice's is the same for both).
    pub sfx: i32,
}

/// The beaten hand's cry.
const SFX_BEATEN_VOICE: i32 = 320023;

impl Running<'_> {
    /// Beaten, `ftMh_MS_343_80151484` and `ftCh_GrabUnk1_8015ADD0` (`fn_800C8E74`'s for the
    /// hands): still, his lasers ended, their hums stopped, a held fighter let go, falling
    /// for the fall's frames, the cries.
    #[doc(alias = "ftMh_MS_343_80151484")]
    #[doc(alias = "ftCh_GrabUnk1_8015ADD0")]
    pub fn hand_beaten_enter(&mut self, gobj: GObjId, h: &Hand) {
        let f = self.game.fighter_id(gobj);
        self.game.fighters[f.0].cmd_vars[0] = 0;
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel = crate::vec::Vec3::new(0.0, 0.0, 0.0);
        for off in h.lasers {
            let l = self.game.fighters[f.0].mv.gobj_at(off);
            super::super::super::it::kinds::masterhandlaser::end(self, l);
        }
        let fp = &mut self.game.fighters[f.0];
        for off in h.lasers {
            fp.mv.set_gobj_at(off, None);
        }
        for off in h.hums {
            let s = self.game.fighters[f.0].mv_i32(off);
            self.sfx_stop(s);
        }
        if self.game.fighters[f.0].mv_i32(mv::HOLDING) != 0 {
            let victim = self.game.fighters[f.0]
                .victim_gobj
                .expect("a beaten hand holding no one");
            self.hand_capture_cut(victim);
        }
        let [vy, vz, frames] = h.beaten.fall.map(|off| h.f32(self, f, off));
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.y = vy;
        fp.self_vel.z = vz;
        fp.set_mv_i32(mv::X8, fp::fctiwz(reg(frames)));
        self.hand_enter(gobj, h.beaten.states[0]);
        self.fighter_play_track(
            f,
            super::super::sound::Track::Voice,
            SFX_BEATEN_VOICE,
            127,
            64,
        );
        self.fighter_play_sfx(f, h.beaten.sfx, 127, 64);
        let fp = &mut self.game.fighters[f.0];
        for off in h.beaten.cheers {
            fp.set_mv_i32(off, 0);
        }
    }

    /// Falling beaten, `ftMh_Damage_Anim`, `ftMh_MS_345_Anim`, `ftCh_TagGrab_Anim` and
    /// `ftCh_TagSqueeze_Anim`: a cheer on the script's flag, the second fall once the first
    /// ends, knocked out once the frames run out.
    #[doc(alias = "ftMh_Damage_Anim")]
    #[doc(alias = "ftMh_MS_345_Anim")]
    #[doc(alias = "ftCh_TagGrab_Anim")]
    #[doc(alias = "ftCh_TagSqueeze_Anim")]
    pub fn hand_beaten_anim(&mut self, gobj: GObjId, h: &Hand) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[0] != 0 {
            let [first, second] = h.beaten.cheer_counts.map(|off| h.i32(self, f, off));
            let [c1, c2] = h.beaten.cheers;
            let fp = &mut self.game.fighters[f.0];
            let n = fp.mv_i32(c1).wrapping_add(1);
            fp.set_mv_i32(c1, n);
            if n <= first {
                self.sfx_play_cheer(SFX_CHEER);
            } else {
                let m = fp.mv_i32(c2).wrapping_add(1);
                fp.set_mv_i32(c2, m);
                if m <= second {
                    self.sfx_play_cheer(SFX_CHEER2);
                }
            }
            self.game.fighters[f.0].cmd_vars[0] = 0;
        }
        if self.game.fighters[f.0].mv_i32(mv::X8) > 0 && !self.anim_frames_remaining(gobj) {
            self.hand_enter(gobj, h.beaten.states[1]);
        }
        let fp = &mut self.game.fighters[f.0];
        let n = fp.mv_i32(mv::X8).wrapping_sub(1);
        fp.set_mv_i32(mv::X8, n);
        if n == 0 {
            self.remove_with_partner(gobj, false);
        }
    }

    /// `ftMh_Damage_Phys` and `ftCh_TagGrab_Phys`: the beaten hand falls away (the fall's
    /// acceleration each frame).
    #[doc(alias = "ftMh_Damage_Phys")]
    #[doc(alias = "ftCh_TagGrab_Phys")]
    pub fn hand_beaten_phys(&mut self, gobj: GObjId, h: &Hand) {
        let f = self.game.fighter_id(gobj);
        let [dy, dz] = h.beaten.accel.map(|off| h.f32(self, f, off));
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.y = mem(fp::fadds(reg(fp.self_vel.y), reg(dy)));
        fp.self_vel.z = mem(fp::fadds(reg(fp.self_vel.z), reg(dz)));
    }
}

#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        super::addr::MS_343_80151484 => g.hand_beaten_enter(gobj, &HAND),
        super::addr::DAMAGE_ANIM | super::addr::MS_345_ANIM => g.hand_beaten_anim(gobj, &HAND),
        super::addr::DAMAGE_PHYS => g.hand_beaten_phys(gobj, &HAND),
        _ => return false,
    }
    true
}
