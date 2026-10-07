//! Color animations (`ftcolanim.c`, `lb_013B.c`): the tints and lights a fighter flashes (a
//! hit, a heal, an invincibility blink), each a command script on one of three overlays.

use super::super::code::unported_call;
use super::super::state::Running;
use super::super::structs::ColorOverlay;
use crate::hsd::gobj::GObjId;
use crate::lb::colanim::{self, Stop, clear};
use melee_data::archive::DataRef;

/// Which of the fighter's overlays: `x408` (most), `x488` (those `Fighter_804D653C` marks)
/// and `x508` (Spycloak's).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Overlay {
    X408,
    X488,
    X508,
}

impl Running<'_> {
    fn overlay(&mut self, gobj: GObjId, which: Overlay) -> &mut ColorOverlay {
        let fp = self.game.fighter_mut(gobj);
        match which {
            Overlay::X408 => &mut fp.x408,
            Overlay::X488 => &mut fp.x488,
            Overlay::X508 => &mut fp.x508,
        }
    }

    /// Steps the fighter's color animations, `ftCo_800C0408`, restarting the defaults of
    /// those that end.
    #[doc(alias = "ftCo_800C0408")]
    pub fn color_anim_update(&mut self, gobj: GObjId) {
        while self.color_overlay_step(gobj, Overlay::X508, false) {
            // `ft_800C0098`.
            clear(self.overlay(gobj, Overlay::X508));
            if self.game.fighter(gobj).x2226_b4 {
                self.start_color_anim(gobj, 0x80, 0);
            }
        }
        while self.color_overlay_step(gobj, Overlay::X408, false) {
            self.reset_x408(gobj);
        }
        let alt = i32::from_be_bytes(self.game.fighter(gobj).x408.x28_colanim) != 0;
        while self.color_overlay_step(gobj, Overlay::X488, alt) {
            self.reset_x488(gobj);
        }
        self.game.fighter_mut(gobj).x2221_b3 = true;
    }

    /// `resetColAnimX408`: back to whichever lasting state's color applies.
    fn reset_x408(&mut self, gobj: GObjId) {
        clear(self.overlay(gobj, Overlay::X408));
        let fp = self.game.fighter(gobj);
        let (dead, healing, b6) = (fp.stamina_dead, fp.dmg.x18F0 != 0, fp.x221D_b6);
        let x9 = fp.x1994 != 0 || fp.x1990 != 0 || fp.x2221_b0;
        if dead {
            self.start_color_anim(gobj, 0x7A, 0);
        }
        if healing {
            self.start_color_anim(gobj, 8, 0);
        }
        if b6 {
            self.start_color_anim(gobj, 0x6B, 0);
        }
        if x9 {
            self.start_color_anim(gobj, 9, 0);
        }
    }

    /// `ftCo_800C0134`.
    #[doc(alias = "ftCo_800C0134")]
    #[doc(alias = "ftSk_Init_UnkMotionStates4")]
    pub(super) fn reset_x488(&mut self, gobj: GObjId) {
        clear(self.overlay(gobj, Overlay::X488));
        let fp = self.game.fighter(gobj);
        // `ftData_UnkMotionStates4`: Donkey Kong's, Kirby's, Sheik's, Samus's, Mewtwo's and
        // Game & Watch's.
        match fp.kind {
            // `ftSk_Init_UnkMotionStates4`: Sheik's charged needles glow.
            7 => {
                if i32::from_be_bytes(fp.u[..4].try_into().unwrap()) == 6 {
                    self.start_color_anim(gobj, 86, 0);
                }
            }
            3 | 4 | 13 | 16 | 24 => unported_call("ftData_UnkMotionStates4"),
            _ => {}
        }
        let fp = self.game.fighter(gobj);
        // `ftCo_800C53E4`: holding the hammer.
        if (307..=313).contains(&fp.motion_id) {
            self.start_color_anim(gobj, 0x6A, 0);
        }
    }

    /// `Fighter_804D653C[i]` (or `Fighter_804D6538`'s, for Spycloak's): a color animation's
    /// script and priority, and whether it plays on `x488`.
    fn color_anim_entry(&self, spycloak: bool, i: i32) -> (Option<DataRef>, u8, u8) {
        let table = self.load_common(if spycloak { 0x1C } else { 0x18 });
        let e = table.at(8 * i as u32);
        (
            self.data.ptr(e),
            self.data.u8(e.at(4)),
            self.data.u8(e.at(5)),
        )
    }

    /// Starts color animation `id` unless one of higher priority plays, `ftCo_800BFFD0`;
    /// `frames` limits it (0 for its script's length).
    #[doc(alias = "ftCo_800BFFD0")]
    pub fn start_color_anim(&mut self, gobj: GObjId, id: i32, frames: i32) -> bool {
        let (which, spycloak, i) = if id >= SPYCLOAK_START {
            (Overlay::X508, true, id - SPYCLOAK_START)
        } else if self.color_anim_entry(false, id).2 != 0 {
            (Overlay::X488, false, id)
        } else {
            (Overlay::X408, false, id)
        };
        let table = self.load_common(if spycloak { 0x1C } else { 0x18 });
        let data = self.data;
        colanim::start(self.overlay(gobj, which), data, table, i, frames)
    }

    /// The color animation playing: `x408`'s, or `x488`'s when `x408` plays none,
    /// `ftCo_800C0694`.
    #[doc(alias = "ftCo_800C0694")]
    pub fn color_anim_id(&self, gobj: GObjId) -> i32 {
        let fp = self.game.fighter(gobj);
        let x408 = i32::from_be_bytes(fp.x408.x28_colanim);
        if x408 != 0 {
            x408
        } else {
            i32::from_be_bytes(fp.x488.x28_colanim)
        }
    }

    /// Stops color animation `id`'s overlay and restarts what lasts there, `ftCo_800C0200`.
    #[doc(alias = "ftCo_800C0200")]
    pub fn end_color_anim(&mut self, gobj: GObjId, id: i32) {
        assert!(id < SPYCLOAK_START, "don't reset spycloak colanim!");
        if self.color_anim_entry(false, id).2 != 0 {
            self.reset_x488(gobj);
        } else {
            self.reset_x408(gobj);
        }
    }

    /// Runs one overlay's script for the frame and blends its colors (`lb_80014258`, the
    /// shared [`colanim`] runner with the fighter's own commands); whether it ended (its frames,
    /// `x4_pri`, ran out, or a command ended it). `alt` picks the fighter command table
    /// `ft_800BFF70` over `ft_800BFF34`.
    fn color_overlay_step(&mut self, gobj: GObjId, which: Overlay, alt: bool) -> bool {
        let data = self.data;
        colanim::tick_timer(self.overlay(gobj, which));
        loop {
            match colanim::run_commands(data, self.overlay(gobj, which)) {
                None => break,
                Some(Stop::Ended) => return true,
                Some(Stop::Owner(op)) => self.overlay_command(gobj, which, op - 21, alt),
            }
        }
        colanim::blend(self.overlay(gobj, which))
    }
}

/// `FtColAnim_SpycloakStart`: Sheik's Spycloak animations, numbered from here on `x508`.
const SPYCLOAK_START: i32 = 123;
