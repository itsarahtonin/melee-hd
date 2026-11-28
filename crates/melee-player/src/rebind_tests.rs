//! Rebinding changes only the inputs the game reads (milestone 7's task 1): a match played with
//! a rebound gamepad on port 1 and a rebound GameCube controller on port 2 is recorded from
//! the ports' reads, and its replay plays back exactly, event for event, as any recording does
//! (`record_tests`). Skips without `SSBM_DISC`.

use melee_input::bindings::{GamepadReader, GamepadState, GcReader};
use melee_input::gilrs::{Axis, Button};
use melee_input::{Bindings, Source, Target};
use melee_sim::game::record::Setup;
use melee_sim::game::state::Rules;
use melee_sim::game::vs_match::Settings;
use melee_sim::input::pad::PadStatus;

use crate::predict_tests::disc;
use crate::record_tests::{pads, record_and_play_back_with, settings};

/// A gamepad held as `p` asks: the face buttons by A and Y, the right trigger by R's travel,
/// the sticks by the sticks.
struct Held(PadStatus);

impl GamepadState for Held {
    fn is_pressed(&self, b: Button) -> bool {
        let bit = match b {
            Button::South => 0x0100,
            Button::North => 0x0800,
            Button::RightTrigger2 => 0x0020,
            _ => return false,
        };
        self.0.button & bit != 0
    }

    fn button_value(&self, b: Button) -> f32 {
        match b {
            Button::RightTrigger2 => f32::from(self.0.trigger_right) / 255.0,
            b => f32::from(u8::from(self.is_pressed(b))),
        }
    }

    fn axis_value(&self, a: Axis) -> f32 {
        f32::from(match a {
            Axis::LeftStickX => self.0.stick_x,
            Axis::LeftStickY => self.0.stick_y,
            Axis::RightStickX => self.0.substick_x,
            Axis::RightStickY => self.0.substick_y,
            _ => 0,
        }) / 127.0
    }
}

/// The gamepad rebound: A and Y swapped, the two sticks swapped.
fn gamepad() -> Bindings {
    let mut b = Bindings::gamepad();
    b.set(Target::A, Some(Source::Button(Button::North)));
    b.set(Target::Y, Some(Source::Button(Button::South)));
    for (t, a, pos) in [
        (Target::StickRight, Axis::RightStickX, true),
        (Target::StickLeft, Axis::RightStickX, false),
        (Target::StickUp, Axis::RightStickY, true),
        (Target::StickDown, Axis::RightStickY, false),
        (Target::CRight, Axis::LeftStickX, true),
        (Target::CLeft, Axis::LeftStickX, false),
        (Target::CUp, Axis::LeftStickY, true),
        (Target::CDown, Axis::LeftStickY, false),
    ] {
        b.set(t, Some(Source::Axis(a, pos)));
    }
    b
}

/// The GameCube controller rebound: Y jumps from X's place, R shields from L's, and the
/// stick's left and right are swapped.
fn gamecube() -> Bindings {
    let mut b = Bindings::gamecube();
    b.set(Target::Y, Some(Source::Gc(Target::X)));
    b.set(Target::X, Some(Source::Gc(Target::Y)));
    b.set(Target::RAnalog, Some(Source::Gc(Target::LAnalog)));
    b.set(Target::LAnalog, Some(Source::Gc(Target::RAnalog)));
    b.set(Target::RClick, Some(Source::Gc(Target::LClick)));
    b.set(Target::LClick, Some(Source::Gc(Target::RClick)));
    b.set(Target::StickLeft, Some(Source::Gc(Target::StickRight)));
    b.set(Target::StickRight, Some(Source::Gc(Target::StickLeft)));
    b
}

/// The ports as the game reads them at `tick` with `pad` and `gc` bound: port 1's gamepad and
/// port 2's GameCube controller held as `record_tests`' script asks.
fn read(tick: u32, pad: &Bindings, gc: &Bindings) -> [PadStatus; 4] {
    let held = pads(tick, false);
    let mut out = [PadStatus::default(); 4];
    out[0] = pad.pad(&GamepadReader {
        state: &Held(held[0]),
        taps: &[],
        exact: false,
    });
    out[1] = gc.pad(&GcReader(&held[1]));
    out
}

#[test]
fn a_rebound_match_plays_back() {
    crate::on_game_stack(|| {
        let Some(disc) = disc() else { return };
        melee_sim::gekko_fp::set_fp_mode(melee_sim::gekko_fp::FpMode::Slippi);
        let (pad, gc) = (gamepad(), gamecube());
        // The rebinding changes what the game reads, often.
        let changed = (0..3600)
            .filter(|&t| read(t, &pad, &gc) != read(t, &Bindings::gamepad(), &Bindings::gamecube()))
            .count();
        assert!(changed > 1000, "{changed} ticks rebound");
        let s = Settings {
            record: Some(Setup::slippi()),
            rules: Rules::slippi_online(),
            ..settings()
        };
        let (original, ours, _) =
            record_and_play_back_with(&disc, &s, "REBIND_TEST_OUT", &|t| read(t, &pad, &gc));
        melee_sim::gekko_fp::set_fp_mode(melee_sim::gekko_fp::FpMode::Hardware);
        assert_eq!(original.frames.len(), ours.frames.len());
        // What the game read was the rebound input: the script's first A press on port 1's
        // gamepad reached the game as Y.
        let tick = (0..3600)
            .find(|&t| pads(t, false)[0].button & 0x0900 == 0x0100)
            .expect("an A press");
        assert_eq!(read(tick, &pad, &gc)[0].button & 0x0900, 0x0800);
    });
}
