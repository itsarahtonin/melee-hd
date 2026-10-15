//! Put to sleep by a hit (`ftCo_DamageSong.c`): Jigglypuff's Sing (`Nap`) and Rest (`Sleep`)
//! hold a fighter asleep (`DamageSong`, then `DamageSongWait`) until the mash timer runs out,
//! then wake it (`DamageSongRv`).

use gekko_fp as fp;

use super::super::super::state::Running;
use super::ids::ms;
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;

/// `Ft_MF_SkipMatAnim | Ft_MF_SkipColAnim`.
const WAIT_FLAGS: u32 = 0x80 | 0x1000;

impl Running<'_> {
    /// Asleep from a hit, `ftCo_800C318C`: the held item and victim let go, the state, and
    /// the mash timer by the fighter's damage, rank and handicap (`ftCommonData` x624 to
    /// x638), longer by x644 for Rest's (`sleep`).
    #[doc(alias = "ftCo_800C318C")]
    pub fn enter_damage_song(&mut self, gobj: GObjId, sleep: bool) {
        // `ftCo_ReleaseItemAndVictim`.
        self.damage_hooks(gobj);
        self.drop_heavy_item(gobj);
        self.release_victim(gobj);
        self.change_motion_state(gobj, ms::DAMAGE_SONG, 0, 0.0, 1.0, 0.0, None);
        let slot = i32::from(self.game.fighter(gobj).player_idx);
        let c = self.common();
        let rank = self.game.player_rank(slot).wrapping_add(1);
        let by_rank = fp::fmuls(
            reg(c.f32(0x630)),
            fp::fsubs(reg(c.f32(0x634)), fp::frsp(f64::from(rank))),
        );
        let handicap = self.game.player_handicap(slot);
        let mut s = fp::fsubs(reg(c.f32(0x62C)), fp::frsp(f64::from(handicap)));
        s = fp::fmadds(reg(c.f32(0x628)), s, reg(c.f32(0x624)));
        s = fp::fadds(s, by_rank);
        let fp = self.game.fighter_mut(gobj);
        let mut timer = fp::fmadds(reg(fp.dmg.x1830_percent), reg(c.f32(0x638)), s);
        if sleep {
            timer = fp::fmuls(timer, reg(c.f32(0x644)));
        }
        // `ftCommon_InitGrab(fp, 0, timer)`.
        fp.grab_timer = mem(timer);
        fp.x1A51 = 0;
        fp.x1A50 = 0;
        fp.x2224_b6 = false;
    }

    /// The timer runs down by `ftCommonData` x63C and mashing (x640) takes more off; once
    /// it's out, waking (`inlineB0`).
    fn song_tick(&mut self, gobj: GObjId) -> bool {
        let (dec, mash) = {
            let c = self.common();
            (c.f32(0x63C), c.f32(0x640))
        };
        let fp = self.game.fighter_mut(gobj);
        fp.grab_timer = mem(fp::fsubs(reg(fp.grab_timer), reg(dec)));
        self.grab_mash(gobj, mash);
        if self.game.fighter(gobj).grab_timer <= 0.0 {
            self.enter_damage_song_rv(gobj);
            return true;
        }
        false
    }

    /// `ftCo_DamageSong_Anim`: the timer, then asleep once the fall plays out.
    #[doc(alias = "ftCo_DamageSong_Anim")]
    pub fn co_damage_song_anim(&mut self, gobj: GObjId) {
        if !self.song_tick(gobj) && !self.anim_frames_remaining(gobj) {
            self.enter_damage_song_wait(gobj);
        }
    }

    /// Asleep, `ftCo_800C3390`.
    #[doc(alias = "ftCo_800C3390")]
    pub fn enter_damage_song_wait(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::DAMAGE_SONG_WAIT, WAIT_FLAGS, 0.0, 1.0, 0.0, None);
    }

    /// `ftCo_DamageSongWait_Anim`: the timer.
    #[doc(alias = "ftCo_DamageSongWait_Anim")]
    pub fn co_damage_song_wait_anim(&mut self, gobj: GObjId) {
        self.song_tick(gobj);
    }

    /// Waking, `ftCo_800C3480`.
    #[doc(alias = "ftCo_800C3480")]
    pub fn enter_damage_song_rv(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::DAMAGE_SONG_RV, 0, 0.0, 1.0, 0.0, None);
    }

    /// `ftCo_DamageSongRv_Anim`: standing once it plays out.
    #[doc(alias = "ftCo_DamageSongRv_Anim")]
    pub fn co_damage_song_rv_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }
}
