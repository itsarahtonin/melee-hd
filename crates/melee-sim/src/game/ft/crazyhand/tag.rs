//! Crazy Hand's halves of the tag attacks (`ftcrazyhandtagrockpaper.c`,
//! `ftcrazyhandsqueezing1.c`, `ftcrazyhandsqueeze.c`, `ftcrazyhandthrow.c`) and his defeat
//! (`ftcrazyhandtaggrab.c`, the hands' shared one, `ft::masterhand::damage`).
//!
//! For a tag attack he glides to its point and waits (`TagRockPaper`) until Master Hand's
//! attack timer runs out; Master Hand sees him waiting (`ftBossLib_8015C4C4`, his next
//! attack) and glides to his own point, then starts both halves (`ft::masterhand::tag`).
//! Alone (a debug VS match without Master Hand) he waits there for good: nothing else ends
//! the wait but Master Hand beaten or out.

use super::super::super::state::Running;
use super::{HAND, addr, ms};
use crate::game::ft::FighterKind;
use crate::hsd::gobj::GObjId;

impl Running<'_> {
    /// Waiting for Master Hand at a tag attack's point, `fn_8015AAC8`: his animation's frame
    /// kept, its first frame not run.
    #[doc(alias = "fn_8015AAC8")]
    pub fn ch_tag_wait_enter(&mut self, gobj: GObjId) {
        let frame = self.game.fighter(gobj).cur_anim_frame;
        self.change_motion_state(gobj, ms::TAG_ROCK_PAPER, 0, frame, 1.0, 0.0, None);
    }

    /// `ftCh_TagRockPaper_Anim`: back to his place if Master Hand is beaten or out; the wait's
    /// animation again when it ends (even just after starting back).
    #[doc(alias = "ftCh_TagRockPaper_Anim")]
    pub fn ch_tag_wait_anim(&mut self, gobj: GObjId) {
        if self.boss_hand_partner_gone(&HAND) {
            self.hand_return(gobj, &super::HAND);
        }
        if !self.anim_frames_remaining(gobj) {
            self.hand_enter(gobj, ms::TAG_ROCK_PAPER);
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
        let mh = self.boss_gobj(FighterKind::MasterHand.id());
        self.game.fighter_mut(gobj).x1A5C = mh;
        self.hand_enter(gobj, msid);
    }
}

#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    use addr::*;
    match a {
        TAG_WAIT_ENTER => g.ch_tag_wait_enter(gobj),
        TAG_ROCK_PAPER_ANIM => g.ch_tag_wait_anim(gobj),
        TAG_HALF_CRUSH_ENTER => g.ch_tag_enter(gobj, ms::SQUEEZING1),
        TAG_HALF_APPLAUD_ENTER => g.ch_tag_enter(gobj, ms::SQUEEZE),
        TAG_HALF_ROCK_PAPER_ENTER => g.ch_tag_enter(gobj, ms::THROW),
        SQUEEZING1_ANIM | SQUEEZE_ANIM | THROW_ANIM => g.hand_tag_anim(gobj, &HAND),
        BEATEN_ENTER => g.hand_beaten_enter(gobj, &HAND),
        TAG_GRAB_ANIM | TAG_SQUEEZE_ANIM => g.hand_beaten_anim(gobj, &HAND),
        TAG_GRAB_PHYS => g.hand_beaten_phys(gobj, &HAND),
        _ => return false,
    }
    true
}
