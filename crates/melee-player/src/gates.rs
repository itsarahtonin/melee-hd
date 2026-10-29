//! The menus' gates (docs/player.md, "The menus the player offers"): while the main menu's
//! cursor is on an entry that leads into unported code (melee-sim's `menus_audit::GATES`, the
//! single-player modes), the confirming buttons (A and Start) of every port are held back from
//! the game, and stay held back until they're let go, so a held press can't arrive on another
//! entry. The game runs as a console's on which nobody confirmed there, and the session's
//! recording, which takes the reads the game took, is still a console's.

use std::time::{Duration, Instant};

use melee_sim::game::menus_audit::{self, Gate};
use melee_sim::game::state::Game;
use melee_sim::input::controller::PadData;
use melee_sim::input::pad::{BUTTON_A, BUTTON_START};

/// How long the note that a gated entry was refused shows.
pub const NOTE: Duration = Duration::from_secs(3);

/// The confirming buttons.
const CONFIRM: u16 = BUTTON_A | BUTTON_START;

/// The gates' state: the ports whose confirming buttons are held back, and the last refusal.
#[derive(Default)]
pub struct Gates {
    held: [bool; 4],
    /// The entry last refused and when, for the note over the game.
    pub refused: Option<(&'static Gate, Instant)>,
}

impl Gates {
    /// The controllers the game is handed in the state `game` is in: with the confirming
    /// buttons held back on a gated entry, and while they stay pressed after.
    pub fn apply(&mut self, game: &Game, mut pads: PadData) -> PadData {
        let gate = menus_audit::gate(game);
        for (p, held) in pads.iter_mut().zip(&mut self.held) {
            let pressed = p.button & CONFIRM;
            if pressed == 0 {
                *held = false;
                continue;
            }
            if let Some(g) = gate
                && !*held
            {
                *held = true;
                self.refused = Some((g, Instant::now()));
                eprintln!("{} (its press held back)", note_text(g));
            }
            if *held {
                p.button &= !CONFIRM;
            }
        }
        pads
    }
}

/// What the player reads when a gated entry is refused.
pub fn note_text(g: &Gate) -> String {
    format!(
        "{} isn't in melee-hd yet. The single-player modes come in a later version.",
        g.what
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use melee_sim::game::gm::machine::{GM_MENU, Phase};
    use melee_sim::input::pad::PadStatus;

    fn menu(list: u8, entry: u16) -> Game {
        let mut g = Game::default();
        g.machine.phase = Phase::Scene {
            mode: GM_MENU,
            scene_mode: 1,
            state: 0,
            overridden: false,
        };
        g.menu.flow.cur_menu = list;
        g.menu.flow.hovered_selection = entry;
        g
    }

    fn pressing(b: u16) -> PadData {
        let p = PadStatus {
            button: b,
            ..PadStatus::default()
        };
        [p; 4]
    }

    /// A and Start are held back on a gated entry, and stay held back until let go, wherever
    /// the cursor goes; other buttons pass, and an entry that isn't gated takes the press.
    #[test]
    fn a_gated_entry_takes_no_confirm() {
        let g = &menus_audit::GATES[0];
        let gated = menu(g.list, g.entry);
        let open = menu(g.list, 99);
        let mut gates = Gates::default();
        let b = melee_sim::input::pad::BUTTON_B;
        let out = gates.apply(&gated, pressing(BUTTON_A | b));
        assert!(out.iter().all(|p| p.button == b));
        assert_eq!(gates.refused.map(|r| r.0), Some(g));
        // Still held, now elsewhere: still held back.
        let out = gates.apply(&open, pressing(BUTTON_START));
        assert!(out.iter().all(|p| p.button == 0));
        // Let go, then pressed elsewhere: the game takes it.
        gates.apply(&open, pressing(0));
        let out = gates.apply(&open, pressing(BUTTON_A));
        assert!(out.iter().all(|p| p.button == BUTTON_A));
        // Outside the main menu's scene nothing is gated.
        let mut other = gated;
        other.machine.phase = Phase::Boot;
        let mut gates = Gates::default();
        let out = gates.apply(&other, pressing(BUTTON_A));
        assert!(out.iter().all(|p| p.button == BUTTON_A));
        assert!(note_text(g).starts_with("Classic isn't in melee-hd yet."));
    }
}
