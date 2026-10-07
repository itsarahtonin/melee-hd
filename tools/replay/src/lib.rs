//! Slippi replays as a melee-sim match's input: what the Slippi EXI device answers Slippi's
//! playback codes (Slippi Dolphin's `EXI_DeviceSlippi`, as ssbm-rs's `ssbm_slippi::Device`
//! reproduces it), so `vs_match::Match` plays a replay as a replay run of the original does.

use melee_sim::game::playback::{CharacterFrame, Frame, Source};
use melee_sim::game::vs_match::ReplayStart;
use ssbm_slippi::replay::{PlayerFrame, Replay};

/// The game info the device sends at the replay's start (`prepareGameInfo`): the game start's
/// `StartMeleeData`, a player who started as Sheik made Sheik, and the replay's code list
/// without the codes playback leaves out.
pub fn start(r: &Replay) -> ReplayStart {
    let mut header = r.header.clone();
    for port in 0..4 {
        if !r.player_exists(port) {
            continue;
        }
        let pos = 24 + 9 * port;
        let character = header[pos] >> 24;
        if (character == 0x12 || character == 0x13) && r.sheik_start[port] {
            header[pos] = (header[pos] & 0x00FF_FFFF) | (0x13 << 24);
        }
    }
    ReplayStart {
        info: header.iter().flat_map(|w| w.to_be_bytes()).collect(),
        seed: r.random_seed,
        nametags: r.nametags,
        codes: ssbm_slippi::gecko::playback_list(&r.gecko_codes),
    }
}

/// The device's answers from a replay.
pub struct Device<'a>(pub &'a Replay);

impl Source for Device<'_> {
    /// `prepareFrameData`: the frame's recorded seed and inputs once the replay holds all of
    /// it, the end past the replay's last frame.
    fn frame(&mut self, index: i32) -> Frame {
        let r = self.0;
        match r.frames.get(&index).filter(|f| f.complete) {
            Some(f) => {
                let chars = core::array::from_fn(|p| {
                    [
                        f.players[p].as_ref().map(character),
                        f.followers[p].as_ref().map(character),
                    ]
                });
                Frame::new(f.random_seed, chars)
            }
            None if r.game_ended || r.frames.keys().next_back() < Some(&index) => {
                Frame::terminate()
            }
            None => panic!("the replay has no complete frame {index}: one still being recorded"),
        }
    }

    /// `prepareIsStockSteal`: the replay has a leader in the port at that frame.
    fn has_player(&mut self, index: i32, port: u8) -> bool {
        self.0
            .frames
            .get(&index)
            .is_some_and(|f| f.players[usize::from(port & 3)].is_some())
    }
}

fn character(p: &PlayerFrame) -> CharacterFrame {
    CharacterFrame {
        seed: p.random_seed,
        stick_x: p.joystick_x,
        stick_y: p.joystick_y,
        cstick_x: p.cstick_x,
        cstick_y: p.cstick_y,
        trigger: p.trigger,
        buttons: p.buttons,
        pos_x: p.location_x,
        pos_y: p.location_y,
        facing: p.facing,
        action: u32::from(p.animation),
        raw_stick_x: p.joystick_x_raw,
        raw_stick_y: p.joystick_y_raw,
        percent: p.percent,
        raw_cstick_x: p.cstick_x_raw,
        raw_cstick_y: p.cstick_y_raw,
    }
}
