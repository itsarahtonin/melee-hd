//! Crazy Hand's halves of the tag attacks (`ftcrazyhandtagrockpaper.c`,
//! `ftcrazyhandsqueezing1.c`, `ftcrazyhandsqueeze.c`, `ftcrazyhandthrow.c`) and his defeat
//! (`ftcrazyhandtaggrab.c`).
//!
//! For a tag attack he glides to its point and waits (`TagRockPaper`) until Master Hand's
//! attack timer runs out; Master Hand sees him waiting (`ftBossLib_8015C4C4`, his next
//! attack) and glides to his own point, then starts both halves (`ft::masterhand::tag`).
//! Alone (a debug VS match without Master Hand) he waits there for good: nothing else ends
//! the wait but Master Hand beaten or out.

use super::super::super::it::kinds::masterhandlaser;
use super::super::super::state::Running;
use super::{KIND_MASTER_HAND, addr, attr, ms, mv};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// The beaten hand's cry and its sound.
const SFX_BEATEN_VOICE: i32 = 0x4E217;
const SFX_BEATEN: i32 = 0x4E219;
/// The crowd's cheers (`lbAudioAx_8002438C`): the first `x124` falls' and the next `x128`'s.
const SFX_CHEER: i32 = 0x81652;
const SFX_CHEER2: i32 = 0x81653;

impl Running<'_> {
    /// Waiting for Master Hand at a tag attack's point, `fn_8015AAC8`: his animation's frame
    /// kept, its first frame not run.
    #[doc(alias = "fn_8015AAC8")]
    pub fn ch_tag_wait_enter(&mut self, gobj: GObjId) {
        let frame = self.game.fighter(gobj).cur_anim_frame;
        self.change_motion_state(gobj, ms::TAG_ROCK_PAPER, 0, frame, 1.0, 0.0, None);
    }

    /// Whether Master Hand is beaten or out (`ftBossLib_8015C2E0`, `8015C358`): the tag
    /// attacks' end.
    fn ch_partner_gone(&self) -> bool {
        self.boss_mh_beaten() || self.boss_mh_sleeping()
    }

    /// `ftCh_TagRockPaper_Anim`: back to his place if Master Hand is beaten or out; the wait's
    /// animation again when it ends (even just after starting back).
    #[doc(alias = "ftCh_TagRockPaper_Anim")]
    pub fn ch_tag_wait_anim(&mut self, gobj: GObjId) {
        if self.ch_partner_gone() {
            self.ch_return(gobj);
        }
        if !self.anim_frames_remaining(gobj) {
            self.ch_enter(gobj, ms::TAG_ROCK_PAPER);
        }
    }

    /// His half of a tag attack, from Master Hand's start (`ftMh_MS_380_80155194`,
    /// `382_801552F8`, `383_80155484`): `ftCh_Init_8015A2B0` (0x179), `8015A3F4` (0x17A, the
    /// cry's flag cleared) and `8015A560` (0x17B), Master Hand his partner (`x1A5C`).
    #[doc(alias = "ftCh_Init_8015A2B0")]
    #[doc(alias = "ftCh_Init_8015A3F4")]
    #[doc(alias = "ftCh_Init_8015A560")]
    pub fn ch_tag_enter(&mut self, gobj: GObjId, msid: i32) {
        if msid != ms::SQUEEZING1 {
            self.game.fighter_mut(gobj).cmd_vars[1] = 0;
        }
        let mh = self.boss_gobj(KIND_MASTER_HAND);
        self.game.fighter_mut(gobj).x1A5C = mh;
        self.ch_enter(gobj, msid);
    }

    /// `ftCh_Squeezing1_Anim`, `ftCh_Squeeze_Anim` and `ftCh_Throw_Anim`: a cry on the
    /// script's flag (the last two); back to his place once it ends or Master Hand is beaten
    /// or out, his partner forgotten.
    #[doc(alias = "ftCh_Squeezing1_Anim")]
    #[doc(alias = "ftCh_Squeeze_Anim")]
    #[doc(alias = "ftCh_Throw_Anim")]
    pub fn ch_tag_anim(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).motion_id != ms::SQUEEZING1
            && self.game.fighter(gobj).cmd_vars[1] != 0
        {
            self.boss_cry(gobj);
            self.game.fighter_mut(gobj).cmd_vars[1] = 0;
        }
        if self.ch_partner_gone() || !self.anim_frames_remaining(gobj) {
            self.ic_thaw(gobj);
            self.game.fighter_mut(gobj).x1A5C = None;
            self.ch_return(gobj);
        }
    }

    /// Beaten, `ftCh_GrabUnk1_8015ADD0` (`fn_800C8E74`'s for him): still, his lasers ended,
    /// the poke's hums stopped, a held fighter let go, falling (`x12C`, `x134`) for `x13C`
    /// frames, the cries.
    // Ports `ftCh_GrabUnk1_8015ADD0`; aliased once a check reaches it.
    pub fn ch_beaten(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        fp.cmd_vars[0] = 0;
        fp.self_vel = Vec3::new(0.0, 0.0, 0.0);
        for off in mv::LASERS {
            let l = self.game.fighters[f.0].mv.gobj_at(off);
            masterhandlaser::end(self, l);
        }
        let fp = &mut self.game.fighters[f.0];
        for off in mv::LASERS {
            fp.mv.set_gobj_at(off, None);
        }
        for off in mv::HUMS {
            let h = self.game.fighters[f.0].mv_i32(off);
            self.sfx_stop(h);
        }
        if self.game.fighters[f.0].mv_i32(mv::HOLDING) != 0 {
            let victim = self.game.fighters[f.0]
                .victim_gobj
                .expect("a beaten hand holding no one");
            self.ch_capture_cut(victim);
        }
        let (vy, vz, frames) = (
            self.ch_da_f32(f, attr::X12C),
            self.ch_da_f32(f, attr::X134),
            self.ch_da_f32(f, attr::X13C),
        );
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.y = vy;
        fp.self_vel.z = vz;
        fp.set_mv_i32(mv::X8, gekko_fp::fctiwz(reg(frames)));
        self.ch_enter(gobj, ms::TAG_GRAB);
        self.fighter_play_track(
            f,
            super::super::sound::Track::Voice,
            SFX_BEATEN_VOICE,
            127,
            64,
        );
        self.fighter_play_sfx(f, SFX_BEATEN, 127, 64);
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_i32(mv::CHEERS, 0);
        fp.set_mv_i32(mv::CHEERS2, 0);
    }

    /// Falling beaten, `ftCh_TagGrab_Anim` and `ftCh_TagSqueeze_Anim`: a cheer on the
    /// script's flag, the second fall once the first ends, knocked out once the frames run
    /// out.
    // Ports `ftCh_TagGrab_Anim`; aliased once a check reaches it.
    // Ports `ftCh_TagSqueeze_Anim`; aliased once a check reaches it.
    pub fn ch_beaten_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[0] != 0 {
            let (first, second) = (self.ch_da_i32(f, attr::X124), self.ch_da_i32(f, attr::X128));
            let fp = &mut self.game.fighters[f.0];
            let n = fp.mv_i32(mv::CHEERS).wrapping_add(1);
            fp.set_mv_i32(mv::CHEERS, n);
            if n <= first {
                self.sfx_play_cheer(SFX_CHEER);
            } else {
                let m = fp.mv_i32(mv::CHEERS2).wrapping_add(1);
                fp.set_mv_i32(mv::CHEERS2, m);
                if m <= second {
                    self.sfx_play_cheer(SFX_CHEER2);
                }
            }
            self.game.fighters[f.0].cmd_vars[0] = 0;
        }
        if self.game.fighters[f.0].mv_i32(mv::X8) > 0 && !self.anim_frames_remaining(gobj) {
            self.ch_enter(gobj, ms::TAG_SQUEEZE);
        }
        let fp = &mut self.game.fighters[f.0];
        let n = fp.mv_i32(mv::X8).wrapping_sub(1);
        fp.set_mv_i32(mv::X8, n);
        if n == 0 {
            self.remove_with_partner(gobj, false);
        }
    }

    /// `ftCh_TagGrab_Phys`: the beaten hand falls away (`x130`, `x138` each frame).
    // Ports `ftCh_TagGrab_Phys`; aliased once a check reaches it.
    pub fn ch_beaten_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let (dy, dz) = (self.ch_da_f32(f, attr::X130), self.ch_da_f32(f, attr::X138));
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.y = mem(gekko_fp::fadds(reg(fp.self_vel.y), reg(dy)));
        fp.self_vel.z = mem(gekko_fp::fadds(reg(fp.self_vel.z), reg(dz)));
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    use addr::*;
    match a {
        FN_8015AAC8 => g.ch_tag_wait_enter(gobj),
        TAG_ROCK_PAPER_ANIM => g.ch_tag_wait_anim(gobj),
        INIT_8015A2B0 => g.ch_tag_enter(gobj, ms::SQUEEZING1),
        INIT_8015A3F4 => g.ch_tag_enter(gobj, ms::SQUEEZE),
        INIT_8015A560 => g.ch_tag_enter(gobj, ms::THROW),
        SQUEEZING1_ANIM | SQUEEZE_ANIM | THROW_ANIM => g.ch_tag_anim(gobj),
        GRAB_UNK1_8015ADD0 => g.ch_beaten(gobj),
        TAG_GRAB_ANIM | TAG_SQUEEZE_ANIM => g.ch_beaten_anim(gobj),
        TAG_GRAB_PHYS => g.ch_beaten_phys(gobj),
        _ => return false,
    }
    true
}
