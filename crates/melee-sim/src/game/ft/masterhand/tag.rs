//! Master Hand's halves of the tag attacks with Crazy Hand (`ftmasterhandslam.c`'s start,
//! `ftmasterhandtagcrush.c`, `ftmasterhandtagapplaud.c`, `ftmasterhandtagrockpaper.c`'s
//! first state): once Crazy Hand waits at a tag attack's point and Master Hand's attack timer
//! runs out, he glides to his own point (`wait::mh_tag_attack`), then starts Crazy Hand's half
//! (unless Crazy Hand is beaten) and his own. Each ends with its animation, or as soon as
//! Crazy Hand is beaten or out.
//!
//! The decomp names his states 382 to 388 for tag moves; only 382 to 384 are entered. Nothing
//! enters 385 to 388 (`TagGrab` to `TagCancel`, `ledger/dead.txt`).

use super::super::super::state::Running;
use super::super::crazyhand::ms as ch_ms;
use super::{KIND_CRAZY_HAND, addr, ms};
use crate::hsd::gobj::GObjId;

impl Running<'_> {
    /// A tag attack's start, `ftMh_MS_380_80155194` (the crush with Crazy Hand's 0x179),
    /// `ftMh_MS_382_801552F8` (the clap with his 0x17A, the cry's flag cleared) and
    /// `ftMh_MS_383_80155484` (the third with his 0x17B): Crazy Hand's half unless he's
    /// beaten, Crazy Hand his partner (`x1A5C`), his own half.
    #[doc(alias = "ftMh_MS_380_80155194")]
    #[doc(alias = "ftMh_MS_382_801552F8")]
    #[doc(alias = "ftMh_MS_383_80155484")]
    pub fn mh_tag_enter(&mut self, gobj: GObjId, msid: i32) {
        let ch_msid = match msid {
            ms::TAG_CRUSH => ch_ms::SQUEEZING1,
            ms::TAG_APPLAUD => {
                self.game.fighter_mut(gobj).cmd_vars[1] = 0;
                ch_ms::SQUEEZE
            }
            _ => ch_ms::THROW,
        };
        let ch = self.boss_gobj(KIND_CRAZY_HAND);
        if !self.boss_ch_beaten() {
            let ch = ch.expect("ftCh_Init_8015A2B0 and its kin read a null Crazy Hand");
            self.ch_tag_enter(ch, ch_msid);
        }
        self.game.fighter_mut(gobj).x1A5C = ch;
        self.mh_enter(gobj, msid);
    }

    /// `ftMh_TagCrush_Anim`, `ftMh_TagApplaud_Anim` and `ftMh_TagRockPaper_Anim`: a cry on the
    /// script's flag (the clap); back to his place once it ends or Crazy Hand is beaten or out,
    /// his partner forgotten.
    #[doc(alias = "ftMh_TagCrush_Anim")]
    #[doc(alias = "ftMh_TagApplaud_Anim")]
    #[doc(alias = "ftMh_TagRockPaper_Anim")]
    pub fn mh_tag_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].motion_id == ms::TAG_APPLAUD
            && self.game.fighters[f.0].cmd_vars[1] != 0
        {
            self.boss_cry(gobj);
            self.game.fighters[f.0].cmd_vars[1] = 0;
        }
        if self.boss_ch_beaten() || self.boss_ch_sleeping() || !self.anim_frames_remaining(gobj) {
            self.ic_thaw(gobj);
            self.game.fighters[f.0].x1A5C = None;
            self.mh_return(gobj);
        }
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::MS_380_80155194 => g.mh_tag_enter(gobj, ms::TAG_CRUSH),
        addr::MS_382_801552F8 => g.mh_tag_enter(gobj, ms::TAG_APPLAUD),
        addr::MS_383_80155484 => g.mh_tag_enter(gobj, ms::TAG_ROCK_PAPER),
        addr::TAG_CRUSH_ANIM | addr::TAG_APPLAUD_ANIM | addr::TAG_ROCK_PAPER_ANIM => {
            g.mh_tag_anim(gobj)
        }
        _ => return false,
    }
    true
}
