//! The settings panel from a controller (docs/milestone-7.md, task 4): every port's controller
//! read while the panel shows, its presses turned into the panel's moves. The D-pad or the
//! stick moves the focus (repeating while held), A activates what has it, B goes back (a
//! drop-down closes, else the panel) and Start closes the panel. [`crate::ui::Window::pad`]
//! hands them to egui as its keys.

use std::time::{Duration, Instant};

use melee_sim::input::controller::PadData;
use melee_sim::input::pad::{
    BUTTON_A, BUTTON_B, BUTTON_DOWN, BUTTON_LEFT, BUTTON_RIGHT, BUTTON_START, BUTTON_UP, PadStatus,
};

/// How often the controllers are read while the panel shows.
pub const POLL: Duration = Duration::from_millis(8);

/// A held direction moves again after this, then every [`REPEAT`].
const FIRST_REPEAT: Duration = Duration::from_millis(400);
const REPEAT: Duration = Duration::from_millis(120);

/// How far the stick must lean to move the focus, of its 127.
const STICK: i32 = 56;

/// A move in the panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nav {
    Up,
    Down,
    Left,
    Right,
    /// A: what has the focus is pressed.
    Activate,
    /// B: a drop-down closes, else the panel.
    Back,
    /// Start: the panel closes.
    Close,
}

/// The direction a controller holds: the D-pad's first, else the stick's, along its larger
/// axis.
fn direction(p: &PadStatus) -> Option<Nav> {
    let b = p.button;
    if b & BUTTON_UP != 0 {
        return Some(Nav::Up);
    }
    if b & BUTTON_DOWN != 0 {
        return Some(Nav::Down);
    }
    if b & BUTTON_LEFT != 0 {
        return Some(Nav::Left);
    }
    if b & BUTTON_RIGHT != 0 {
        return Some(Nav::Right);
    }
    let (x, y) = (i32::from(p.stick_x), i32::from(p.stick_y));
    if x.abs().max(y.abs()) < STICK {
        None
    } else if y.abs() >= x.abs() {
        Some(if y > 0 { Nav::Up } else { Nav::Down })
    } else {
        Some(if x > 0 { Nav::Right } else { Nav::Left })
    }
}

/// A port's controller as last read.
#[derive(Clone, Copy, Default)]
struct Port {
    buttons: u16,
    /// The direction held and when it moves again (none: not until it's let go, as for a
    /// direction held when the panel opened).
    held: Option<(Nav, Option<Instant>)>,
}

/// The controllers' moves in the panel.
#[derive(Default)]
pub struct PadNav {
    ports: [Port; 4],
    /// The ports that moved anything since the panel opened.
    pub drove: [bool; 4],
    /// Rebind… waited for an input: the next read starts again from what's held.
    resync_next: bool,
}

impl PadNav {
    /// The panel opened with the controllers at `pads`: what they hold already does nothing
    /// until it's let go.
    pub fn reset(&mut self, pads: &PadData) {
        self.drove = [false; 4];
        self.resync(pads);
    }

    /// The controllers read again after Rebind… took their presses: what they hold does
    /// nothing until it's let go, and a port holding something counts as one that worked the
    /// panel (what it holds stays out of the game as the panel closes).
    pub fn resync(&mut self, pads: &PadData) {
        for (i, (port, p)) in self.ports.iter_mut().zip(pads).enumerate() {
            let p = present(p);
            port.buttons = p.button;
            port.held = direction(&p).map(|d| (d, None));
            if p.button != 0 || port.held.is_some() {
                self.drove[i] = true;
            }
        }
    }

    /// The panel's moves now, the controllers read by `read`. While Rebind… waits for an input
    /// (`capturing`) a controller's press is its binding, not a move: nothing is read (a read
    /// would drop the gamepads' quick presses Rebind… looks for) and nothing moves; afterwards
    /// what the controllers hold does nothing until it's let go.
    pub fn poll(
        &mut self,
        capturing: bool,
        read: impl FnOnce() -> PadData,
        now: Instant,
    ) -> Vec<Nav> {
        if capturing {
            self.resync_next = true;
            return Vec::new();
        }
        let pads = read();
        if std::mem::take(&mut self.resync_next) {
            self.resync(&pads);
            return Vec::new();
        }
        self.read(&pads, now)
    }

    /// The moves since the last read, every port's in port order.
    pub fn read(&mut self, pads: &PadData, now: Instant) -> Vec<Nav> {
        let mut out = Vec::new();
        for (i, (port, p)) in self.ports.iter_mut().zip(pads).enumerate() {
            let p = present(p);
            let before = out.len();
            let pressed = p.button & !port.buttons;
            port.buttons = p.button;
            let dir = direction(&p);
            match (dir, port.held) {
                (None, _) => port.held = None,
                (Some(d), Some((h, next))) if d == h => {
                    if let Some(t) = next
                        && now >= t
                    {
                        out.push(d);
                        port.held = Some((d, Some(now + REPEAT)));
                    }
                }
                (Some(d), _) => {
                    out.push(d);
                    port.held = Some((d, Some(now + FIRST_REPEAT)));
                }
            }
            for (bit, nav) in [
                (BUTTON_A, Nav::Activate),
                (BUTTON_B, Nav::Back),
                (BUTTON_START, Nav::Close),
            ] {
                if pressed & bit != 0 {
                    out.push(nav);
                }
            }
            if out.len() > before {
                self.drove[i] = true;
            }
        }
        out
    }
}

/// A port as the panel takes it: nothing from a port with no controller.
fn present(p: &PadStatus) -> PadStatus {
    if p.err == 0 { *p } else { PadStatus::default() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use melee_sim::input::pad::{ERR_NO_CONTROLLER, TRIGGER_Z};

    fn pads(port: usize, b: u16, stick: (i8, i8)) -> PadData {
        let mut p = [PadStatus::default(); 4];
        p[port].button = b;
        (p[port].stick_x, p[port].stick_y) = stick;
        p
    }

    /// Each press moves once; a held direction repeats after its first wait, then faster.
    #[test]
    fn presses_move_and_held_directions_repeat() {
        let t0 = Instant::now();
        let ms = |n: u64| t0 + Duration::from_millis(n);
        let mut n = PadNav::default();
        n.reset(&pads(0, 0, (0, 0)));
        assert_eq!(n.read(&pads(0, BUTTON_DOWN, (0, 0)), ms(0)), [Nav::Down]);
        assert_eq!(n.read(&pads(0, BUTTON_DOWN, (0, 0)), ms(300)), []);
        assert_eq!(n.read(&pads(0, BUTTON_DOWN, (0, 0)), ms(400)), [Nav::Down]);
        assert_eq!(n.read(&pads(0, BUTTON_DOWN, (0, 0)), ms(500)), []);
        assert_eq!(n.read(&pads(0, BUTTON_DOWN, (0, 0)), ms(520)), [Nav::Down]);
        assert_eq!(n.read(&pads(0, 0, (0, 0)), ms(530)), []);
        // The stick, along its larger axis, past its threshold.
        assert_eq!(n.read(&pads(0, 0, (-70, 30)), ms(540)), [Nav::Left]);
        assert_eq!(n.read(&pads(0, 0, (20, 80)), ms(550)), [Nav::Up]);
        assert_eq!(n.read(&pads(0, 0, (40, -40)), ms(560)), []);
        assert_eq!(n.read(&pads(0, 0, (0, -127)), ms(570)), [Nav::Down]);
        assert_eq!(n.read(&pads(0, 0, (127, 0)), ms(580)), [Nav::Right]);
        assert_eq!(n.drove, [true, false, false, false]);
    }

    /// A, B and Start act on their press only; Z and the rest do nothing.
    #[test]
    fn a_b_and_start_act_once_a_press() {
        let t = Instant::now();
        let mut n = PadNav::default();
        n.reset(&pads(1, 0, (0, 0)));
        assert_eq!(n.read(&pads(1, BUTTON_A, (0, 0)), t), [Nav::Activate]);
        assert_eq!(n.read(&pads(1, BUTTON_A, (0, 0)), t), []);
        assert_eq!(
            n.read(&pads(1, BUTTON_A | BUTTON_B | BUTTON_START, (0, 0)), t),
            [Nav::Back, Nav::Close]
        );
        assert_eq!(n.read(&pads(1, TRIGGER_Z, (0, 0)), t), []);
        assert_eq!(n.drove, [false, true, false, false]);
    }

    /// While Rebind… waits, the controllers aren't read and nothing moves (B binds, it doesn't
    /// close the panel); after it, the press it took, still held, moves nothing, and that port
    /// counts as one that worked the panel; a new press moves again.
    #[test]
    fn rebind_takes_the_presses_not_the_panel() {
        let t = Instant::now();
        let mut n = PadNav::default();
        n.reset(&pads(3, 0, (0, 0)));
        let unread = || -> PadData { panic!("read while Rebind… waits") };
        assert_eq!(n.poll(true, unread, t), []);
        assert_eq!(n.poll(true, unread, t), []);
        assert_eq!(n.poll(false, || pads(3, BUTTON_B, (0, 90)), t), []);
        assert_eq!(n.poll(false, || pads(3, BUTTON_B, (0, 90)), t), []);
        assert_eq!(n.drove, [false, false, false, true]);
        n.poll(false, || pads(3, 0, (0, 0)), t);
        assert_eq!(n.poll(false, || pads(3, BUTTON_B, (0, 0)), t), [Nav::Back]);
    }

    /// What's held as the panel opens (the Z that opened it, a direction) does nothing until
    /// it's let go; a port with no controller does nothing.
    #[test]
    fn what_was_held_at_the_opening_waits_for_its_release() {
        let t0 = Instant::now();
        let mut n = PadNav::default();
        n.reset(&pads(0, TRIGGER_Z | BUTTON_A | BUTTON_LEFT, (0, 0)));
        for s in 0..5 {
            let at = t0 + Duration::from_secs(s);
            assert_eq!(n.read(&pads(0, BUTTON_A | BUTTON_LEFT, (0, 0)), at), []);
        }
        n.read(&pads(0, 0, (0, 0)), t0);
        assert_eq!(n.read(&pads(0, BUTTON_LEFT, (0, 0)), t0), [Nav::Left]);
        let mut gone = pads(2, BUTTON_A, (0, 0));
        gone[2].err = ERR_NO_CONTROLLER;
        assert_eq!(n.read(&gone, t0), []);
        assert_eq!(n.drove, [true, false, false, false]);
    }
}
