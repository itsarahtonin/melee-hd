//! What the Slippi EXI device answers Slippi's playback codes from a replay (Slippi Dolphin's
//! `EXI_DeviceSlippi`, as ssbm-rs's `ssbm_slippi::Device` reproduces it), so
//! `vs_match::Match` plays a replay as a replay run of the original does.

use melee_sim::game::playback::{CharacterFrame, Frame, Source};
use melee_sim::game::vs_match::ReplayStart;

use crate::denylist::DENYLIST;
use crate::replay::{PlayerFrame, Replay};

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
        codes: playback_codes(&r.gecko_codes),
    }
}

/// The replay's Gecko code list without the codes playback leaves out, as
/// `CEXISlippi::prepareGeckoList` builds it: every code whose injection address is on the
/// denylist is dropped, and the list ends with the terminator.
pub fn playback_codes(source: &[u8]) -> Vec<u8> {
    let word = |at: usize| {
        u32::from_be_bytes([source[at], source[at + 1], source[at + 2], source[at + 3]])
    };
    let mut out = Vec::new();
    let mut at = 0;
    while at + 8 <= source.len() {
        let kind = source[at] & 0xFE;
        let address = (word(at) & 0x01FF_FFFF) | 0x8000_0000;
        let len = match kind {
            0xC0 | 0xC2 => 8 + 8 * word(at + 4) as usize,
            0x08 => 16,
            0x06 => 8 + ((word(at + 4) as usize + 7) & !7),
            _ => 8,
        };
        let end = (at + len).min(source.len());
        if DENYLIST.binary_search(&address).is_err() {
            out.extend_from_slice(&source[at..end]);
        }
        at += len;
    }
    out.extend_from_slice(&[0xFF, 0, 0, 0, 0, 0, 0, 0]);
    out
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

#[cfg(test)]
mod tests {
    use super::*;

    fn list(lines: &[(u32, u32)]) -> Vec<u8> {
        lines
            .iter()
            .flat_map(|&(a, d)| a.to_be_bytes().into_iter().chain(d.to_be_bytes()))
            .collect()
    }

    #[test]
    fn playback_leaves_out_denied_codes_whole() {
        // A denied C2 code (Slippi's 0x8016E9AC injection) of two lines between two writes.
        let source = list(&[
            (0x0400_1000, 1),
            (0xC216_E9AC, 2),
            (0x6000_0000, 0x6000_0000),
            (0x6000_0000, 0),
            (0x0400_2000, 2),
        ]);
        let expected = list(&[(0x0400_1000, 1), (0x0400_2000, 2), (0xFF00_0000, 0)]);
        assert_eq!(playback_codes(&source), expected);
    }
}
