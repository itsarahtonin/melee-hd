//! The settings from a controller (docs/milestone-7.md, task 4, design call 2): two routes into
//! the settings panel that need no keyboard, both in Melee's main menu and neither changing game
//! code, so the session stays a console's and its recording takes the reads the game took.
//!
//! - **Z on the Options list.** While the Options list's think (`mn_8022D104`) runs, a Z press
//!   on any port opens the panel, and Z is held back from the game until it's let go, as the
//!   gates ([`crate::gates`]) hold A back. The player draws "Z: melee-hd settings" under the list.
//! - **Z held in the main menu's lists.** While any of the main menu's ten lists' thinks runs
//!   (Main, 1P, VS, Trophies, Options, Data, Regular Match, Stadium, Special Melee, Records), Z
//!   held for [`HOLD_TICKS`] on one port opens the panel, and from then on Z is held back until
//!   it's let go. Until then the game reads Z, so Z must be a button nothing reads there, which
//!   the code shows (each place the game reads the pads while a list runs):
//!   - `mn_80229624` (`Running::menu_inputs`), every list's input: A, Start, B, L, R, X and Y
//!     pressed, and `PAD_ANY_*` repeating (the D-pad and the stick's directions). Not Z.
//!   - `mn_802295AC` (`Running::note_confirm_port`): `PAD_CONFIRM`, A or Start.
//!   - `mnMain_Scene_OnFrame` (`Running::menu_on_frame`): L, R and Start together.
//!   - `mn_8022BA1C` (`Running::menu_camera_proc`): the C-stick turns the camera, so the C-stick
//!     can't be in the combo.
//!   - `gm_EvaluateAllControllerInputs` and its repeat hook `fn_801A396C`: the button maps
//!     widen every bit, and any press or release restarts a port's key repeat, so pressing Z
//!     while a direction is held delays that direction's next repeat, as any press on a console
//!     does. That's the one effect Z has there, and the combo's press has it once.
//!   - at the debug ROM's level and up (`DbLevel` 3, a developer setting, never the Slippi
//!     ruleset's), every scene's debug pause (`gm_801A4970`) steps a frame on Z while paused, so
//!     the held route is off there. The Options route holds its press back, so it works at any
//!     level.
//!
//!   The other buttons are read by the lists (A, B, X, Y, L, R, Start, the D-pad, the stick)
//!   or turn the camera (the C-stick); the analog triggers' travel short of the click and the
//!   analog A and B values are read by nothing there, but no player can hold those apart from
//!   the buttons. So the combo is Z alone, held. The sub-screens the lists open (the rules, the
//!   names, the Options' screens, the album, the sound test, the event list and the rest) and
//!   the selects read more of the pad, and no list's think runs while they show: neither route
//!   works there, and neither in a match.
//!
//! After the panel closes, a controller that worked the panel ([`crate::pad_nav`]) hands the
//! game nothing it still holds: its buttons are held back until each is let go, and its stick
//! reads centred until it comes back, so the B or Start that closed the panel doesn't reach the
//! Options list as a B.

use melee_sim::game::gm::DB_DEBUG_ROM;
use melee_sim::game::gm::GameMode;
use melee_sim::game::gm::machine::Phase;
use melee_sim::game::mn::main as mn;
use melee_sim::game::state::Game;
use melee_sim::input::controller::PadData;
use melee_sim::input::pad::{PadStatus, TRIGGER_Z};

/// How long Z is held in the main menu's lists before the panel opens: a second.
pub const HOLD_TICKS: u32 = 60;

/// What the player draws under the Options list.
pub const OPTIONS_LINE: &str = "Z: melee-hd settings";

/// The main menu's lists' thinks (`mn_803EB6B0`'s), the Options list's among them.
const LISTS: [u32; 10] = [
    mn::THINK_MAIN,
    mn::THINK_1P,
    mn::THINK_VS,
    mn::THINK_TOY,
    mn::THINK_SETTINGS,
    mn::THINK_DATA,
    mn::THINK_REG,
    mn::THINK_STADIUM,
    mn::THINK_SPECIAL,
    mn::THINK_RECORDS,
];

/// The process link a list's think runs on (`mn_80229894`'s and the scene entry's
/// `GObj_Create(0, 1, 0x80)`).
const THINK_LINK: u8 = 1;

/// Every GameCube button (`PAD_BUTTON_*`, `PAD_TRIGGER_*`).
const BUTTONS: u16 = 0x1F7F;

/// Where the game is, as the routes read it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    /// Neither route works: a match, a select, a sub-screen, another mode.
    Elsewhere,
    /// One of the main menu's lists other than Options.
    List,
    /// The Options list.
    Options,
}

/// Where the game is: which list's think runs in the main menu's scene, if one does.
pub fn place(game: &Game) -> Place {
    let Phase::Scene { mode, .. } = game.machine.phase else {
        return Place::Elsewhere;
    };
    if mode != GameMode::Menu {
        return Place::Elsewhere;
    }
    let w = &game.world;
    let mut g = w.first_in(THINK_LINK);
    while let Some(id) = g {
        let o = w.gobj(id);
        if let Some(p) = o.proc {
            let f = w.proc(p).on_invoke.0;
            if f == mn::THINK_SETTINGS {
                return Place::Options;
            }
            if LISTS.contains(&f) {
                return Place::List;
            }
        }
        g = o.next;
    }
    Place::Elsewhere
}

/// Whether the held route works at the game's debug level (below the debug ROM's).
fn holding_works(game: &Game) -> bool {
    game.gm.db_level < DB_DEBUG_ROM
}

/// A stick inside the pad library's dead zone for directions (`adc_th`, 30).
fn centred(p: &PadStatus) -> bool {
    let (x, y) = (i32::from(p.stick_x), i32::from(p.stick_y));
    x * x + y * y < 30 * 30
}

/// The routes' state, per port.
#[derive(Default)]
pub struct Hooks {
    /// Buttons held back until let go.
    held: [u16; 4],
    /// The stick reads centred until it comes back.
    stick: [bool; 4],
    /// Z held at the last read, as the controller holds it.
    z_before: [bool; 4],
    /// Reads Z has been held in the lists.
    z_ticks: [u32; 4],
    /// A controller asked for the settings; the player opens the panel and clears it.
    pub asked: bool,
}

impl Hooks {
    /// The controllers the game is handed in the state `game` is in.
    pub fn apply(&mut self, game: &Game, pads: PadData) -> PadData {
        self.apply_at(place(game), holding_works(game), pads)
    }

    /// [`Hooks::apply`] at `place`, with the held route on or off.
    fn apply_at(&mut self, place: Place, holding: bool, mut pads: PadData) -> PadData {
        for (i, p) in pads.iter_mut().enumerate() {
            let z = p.button & TRIGGER_Z != 0;
            let pressed = z && !self.z_before[i];
            self.z_before[i] = z;
            let back = self.held[i] & TRIGGER_Z != 0;
            if place == Place::Options && pressed {
                self.held[i] |= TRIGGER_Z;
                self.ask("Z on the Options list", i);
            } else if place != Place::Elsewhere && holding && z && !back {
                self.z_ticks[i] += 1;
                if self.z_ticks[i] >= HOLD_TICKS {
                    self.held[i] |= TRIGGER_Z;
                    self.ask("Z held in the main menu", i);
                }
            }
            if !z || place == Place::Elsewhere || self.held[i] & TRIGGER_Z != 0 {
                self.z_ticks[i] = 0;
            }
            // A button let go is the game's again.
            self.held[i] &= p.button;
            p.button &= !self.held[i];
            if self.stick[i] {
                if centred(p) {
                    self.stick[i] = false;
                } else {
                    (p.stick_x, p.stick_y) = (0, 0);
                }
            }
        }
        pads
    }

    fn ask(&mut self, how: &str, port: usize) {
        if !self.asked {
            eprintln!(
                "{how} (port {}) opens the settings (its Z held back)",
                port + 1
            );
        }
        self.asked = true;
    }

    /// The panel closed with the controllers at `pads`: the ports in `drove`, the ones that
    /// worked the panel, hand the game none of what they still hold.
    pub fn after_panel(&mut self, pads: &PadData, drove: [bool; 4]) {
        for (i, p) in pads.iter().enumerate() {
            self.z_before[i] = p.button & TRIGGER_Z != 0;
            if drove[i] && p.err == 0 {
                self.held[i] |= p.button & BUTTONS;
                self.stick[i] |= !centred(p);
            }
        }
        self.asked = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use melee_sim::input::pad::{BUTTON_A, BUTTON_B, BUTTON_START};

    fn pressing(port: usize, b: u16) -> PadData {
        let mut pads = [PadStatus::default(); 4];
        pads[port].button = b;
        pads
    }

    fn run(h: &mut Hooks, place: Place, pads: PadData) -> PadData {
        h.apply_at(place, true, pads)
    }

    /// Z pressed on the Options list asks for the panel and never reaches the game, until it's
    /// let go; other buttons pass; elsewhere Z passes and asks nothing.
    #[test]
    fn z_on_options_opens_the_panel_and_takes_no_z() {
        let mut h = Hooks::default();
        let out = run(&mut h, Place::Options, pressing(1, TRIGGER_Z | BUTTON_B));
        assert!(h.asked);
        assert_eq!(out[1].button, BUTTON_B);
        h.asked = false;
        // Still held, the list gone: still held back, nothing asked again.
        let out = run(&mut h, Place::Elsewhere, pressing(1, TRIGGER_Z));
        assert_eq!(out[1].button, 0);
        assert!(!h.asked);
        // Let go, then pressed elsewhere: the game takes it.
        run(&mut h, Place::Elsewhere, pressing(1, 0));
        let out = run(&mut h, Place::Elsewhere, pressing(1, TRIGGER_Z));
        assert_eq!(out[1].button, TRIGGER_Z);
        assert!(!h.asked);
        // A match, a select, a sub-screen: Z passes however long it's held.
        let mut h = Hooks::default();
        for _ in 0..3 * HOLD_TICKS {
            let out = run(&mut h, Place::Elsewhere, pressing(0, TRIGGER_Z));
            assert_eq!(out[0].button, TRIGGER_Z);
        }
        assert!(!h.asked);
    }

    /// Z held into the Options list from before passes until it's let go: only a press there
    /// asks at once (the held route still counts it).
    #[test]
    fn z_held_from_before_isnt_a_press() {
        let mut h = Hooks::default();
        run(&mut h, Place::List, pressing(0, TRIGGER_Z));
        let out = run(&mut h, Place::Options, pressing(0, TRIGGER_Z));
        assert_eq!(out[0].button, TRIGGER_Z);
        assert!(!h.asked);
        run(&mut h, Place::Options, pressing(0, 0));
        let out = run(&mut h, Place::Options, pressing(0, TRIGGER_Z));
        assert_eq!(out[0].button, 0);
        assert!(h.asked);
    }

    /// Z held a second in the lists asks for the panel on the second's last read, the game
    /// reading Z until then and never after, until it's let go; a shorter hold, or one broken by
    /// a list's sub-screen, asks nothing.
    #[test]
    fn z_held_a_second_in_the_lists_opens_the_panel() {
        let mut h = Hooks::default();
        for t in 1..=HOLD_TICKS {
            let out = run(&mut h, Place::List, pressing(2, TRIGGER_Z | BUTTON_A));
            if t < HOLD_TICKS {
                assert_eq!(out[2].button, TRIGGER_Z | BUTTON_A, "read {t}");
                assert!(!h.asked, "read {t}");
            } else {
                assert_eq!(out[2].button, BUTTON_A);
                assert!(h.asked);
            }
        }
        h.asked = false;
        for _ in 0..2 * HOLD_TICKS {
            let out = run(&mut h, Place::List, pressing(2, TRIGGER_Z));
            assert_eq!(out[2].button, 0);
        }
        assert!(!h.asked, "a held Z asks once");
        // Broken by a sub-screen, or let go: the count starts again.
        let mut h = Hooks::default();
        for _ in 0..HOLD_TICKS - 1 {
            run(&mut h, Place::List, pressing(0, TRIGGER_Z));
        }
        run(&mut h, Place::Elsewhere, pressing(0, TRIGGER_Z));
        for _ in 0..HOLD_TICKS - 1 {
            run(&mut h, Place::List, pressing(0, TRIGGER_Z));
        }
        run(&mut h, Place::List, pressing(0, 0));
        for _ in 0..HOLD_TICKS - 1 {
            run(&mut h, Place::List, pressing(0, TRIGGER_Z));
        }
        assert!(!h.asked);
        // Each port counts its own.
        let mut h = Hooks::default();
        for t in 0..HOLD_TICKS - 1 {
            let port = (t % 2) as usize;
            run(&mut h, Place::List, pressing(port, TRIGGER_Z));
        }
        assert!(!h.asked);
    }

    /// At the debug ROM's level the held route is off (the debug pause steps on Z); the
    /// Options route still works.
    #[test]
    fn the_held_route_is_off_at_the_debug_roms_level() {
        let mut h = Hooks::default();
        for _ in 0..2 * HOLD_TICKS {
            let out = h.apply_at(Place::List, false, pressing(0, TRIGGER_Z));
            assert_eq!(out[0].button, TRIGGER_Z);
        }
        assert!(!h.asked);
        h.apply_at(Place::Options, false, pressing(0, 0));
        let out = h.apply_at(Place::Options, false, pressing(0, TRIGGER_Z));
        assert_eq!(out[0].button, 0);
        assert!(h.asked);
    }

    /// After the panel, a port that worked it hands the game nothing it still holds, each
    /// button until it's let go and the stick until it's back in the dead zone; other ports
    /// pass as they are.
    #[test]
    fn what_closed_the_panel_stays_out_of_the_game() {
        let mut h = Hooks::default();
        let mut pads = pressing(0, BUTTON_B | BUTTON_START);
        pads[0].stick_y = -90;
        pads[1].button = BUTTON_A;
        h.asked = true;
        h.after_panel(&pads, [true, false, false, false]);
        assert!(!h.asked);
        let out = run(&mut h, Place::Options, pads);
        assert_eq!(out[0].button, 0);
        assert_eq!((out[0].stick_x, out[0].stick_y), (0, 0));
        assert_eq!(out[1].button, BUTTON_A);
        // B let go, Start still held, the stick back near the middle: B and the stick pass.
        let mut pads = pressing(0, BUTTON_START);
        pads[0].stick_y = -10;
        let out = run(&mut h, Place::Options, pads);
        assert_eq!(out[0].button, 0);
        assert_eq!(out[0].stick_y, -10);
        let mut pads = pressing(0, BUTTON_START | BUTTON_B);
        pads[0].stick_y = -90;
        let out = run(&mut h, Place::Options, pads);
        assert_eq!(out[0].button, BUTTON_B);
        assert_eq!(out[0].stick_y, -90);
        run(&mut h, Place::Options, pressing(0, 0));
        let out = run(&mut h, Place::Options, pressing(0, BUTTON_START));
        assert_eq!(out[0].button, BUTTON_START);
    }

    /// The place is the list whose think runs in the main menu's scene: Options for its own
    /// think, a list for the other nine, elsewhere for any other process (a sub-screen's) or
    /// another scene.
    #[test]
    fn the_place_is_the_running_lists() {
        use melee_sim::game::gm::machine::Phase;
        use melee_sim::game::handles::CodeRef;
        let at = |think: u32, mode: GameMode| {
            let mut g = Game::default();
            g.machine.phase = Phase::Scene {
                mode: mode.id(),
                scene_mode: mode.id(),
                state: 0,
                overridden: false,
            };
            // Something else on the link first, as the scene's other processes are.
            let other = g.world.create(0, THINK_LINK, 0x80);
            g.world.setup_proc(other, CodeRef(mn::LIST_PROC), 0);
            let o = g.world.create(0, THINK_LINK, 0x80);
            g.world.setup_proc(o, CodeRef(think), 0);
            place(&g)
        };
        assert_eq!(at(mn::THINK_SETTINGS, GameMode::Menu), Place::Options);
        for t in LISTS.into_iter().filter(|&t| t != mn::THINK_SETTINGS) {
            assert_eq!(at(t, GameMode::Menu), Place::List, "{t:#x}");
        }
        assert_eq!(at(mn::PANEL_PROC, GameMode::Menu), Place::Elsewhere);
        assert_eq!(at(mn::THINK_SETTINGS, GameMode::Vs), Place::Elsewhere);
        assert_eq!(place(&Game::default()), Place::Elsewhere);
    }

    /// The held route works below the debug ROM's level only.
    #[test]
    fn holding_works_below_the_debug_roms_level() {
        let mut g = Game::default();
        for (level, works) in [(0, true), (2, true), (3, false), (4, false)] {
            g.gm.db_level = level;
            assert_eq!(holding_works(&g), works, "level {level}");
        }
    }

    /// Z held through the panel isn't a new press when it closes.
    #[test]
    fn z_held_through_the_panel_asks_nothing_again() {
        let mut h = Hooks::default();
        run(&mut h, Place::Options, pressing(0, TRIGGER_Z));
        assert!(h.asked);
        h.after_panel(&pressing(0, TRIGGER_Z), [false; 4]);
        let out = run(&mut h, Place::Options, pressing(0, TRIGGER_Z));
        assert_eq!(out[0].button, 0);
        assert!(!h.asked);
    }
}
