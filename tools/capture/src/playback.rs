//! Slippi replay playback in a capture: whether Slippi's playback codes patch the game, their
//! state (the replay frame they counted and the frame they read last), and the Slippi
//! ruleset's codes with UCF's pad buffer, as `game::playback` and `game::slippi` model them.

use melee_sim::game::handles::CodeRef;
use melee_sim::game::playback::{
    FRAME_LEN, Frame, Playback, RECORD_FRAME_START_PROC, RECORD_ITEMS_PROC, SEED_PROC,
};
use melee_sim::game::slippi::placed;
use melee_sim::game::state::Rules;
use melee_sim::game::ucf::Pads;

use crate::MEM1;
use crate::compare::Comparer;
use crate::import::Importer;

/// `HSD_PadRenewRawStatus+0x8C`, which playback makes read the queue's read position.
const PAD_RAW_PATCH: u32 = 0x8037_6A88;
const PAD_RAW_PATCHED: u32 = 0x881F_0001;
/// `lbl_804D6660`: playback's buffers, the first its frame response.
const BUFFERS: u32 = 0x804D_6660;
/// `mnCharSel_804D6CF4`'s word, where playback counts the replay's frame.
const FRAME_INDEX: u32 = 0x804D_6CF4;
/// The playback and recording codes whose GObjs' processes run in the match, their entries'
/// offsets, and melee-sim's tokens for them.
const PROCS: [(u32, u32, u32); 3] = [
    (0x8016_E748, 0x4B0, SEED_PROC),
    (0x8016_E74C, 0x6E0, RECORD_FRAME_START_PROC),
    (0x8016_E74C, 0x788, RECORD_ITEMS_PROC),
];

fn word(mem: &[u8], a: u32) -> u32 {
    let i = (a - MEM1) as usize;
    u32::from_be_bytes(mem[i..i + 4].try_into().unwrap())
}

/// A function pointer as melee-sim names it: a playback or recording code's process by its
/// token, any other by its address.
pub fn code_ref(mem: &[u8], addr: u32) -> CodeRef {
    for (site, entry, token) in PROCS {
        if addr != 0 && placed(site, word(mem, site)).map(|p| p + entry) == Some(addr) {
            return CodeRef(token);
        }
    }
    CodeRef(addr)
}

/// The rules whose codes the game in `mem` runs, and UCF's pad buffer.
pub fn rules(mem: &[u8]) -> (Rules, Pads) {
    let read = |a: u32| word(mem, a);
    let rules =
        Rules::from_memory(&read).unwrap_or_else(|e| panic!("the captured game's codes: {e}"));
    (rules, Pads::from_memory(&read).unwrap_or_default())
}

/// Playback's state in `mem`, if the game runs Slippi's playback codes.
fn playback(mem: &[u8]) -> (bool, Option<Playback>) {
    if word(mem, PAD_RAW_PATCH) != PAD_RAW_PATCHED {
        return (false, None);
    }
    let buffers = word(mem, BUFFERS);
    if buffers == 0 {
        return (true, None);
    }
    let at = (word(mem, buffers) - MEM1) as usize;
    let mut bytes = [0; FRAME_LEN];
    bytes.copy_from_slice(&mem[at..at + FRAME_LEN]);
    let p = Playback {
        frame_index: word(mem, FRAME_INDEX) as i32,
        frame: Frame { bytes },
        ..Playback::default()
    };
    (true, Some(p))
}

impl Importer<'_> {
    /// Playback's state, the ruleset's codes and UCF's pad buffer.
    pub fn playback(&mut self) {
        let mem = &self.cap.call.mem1;
        let (patched, p) = playback(mem);
        let (rules, ucf) = rules(mem);
        self.game.pads.playback = patched;
        self.game.playback = p;
        self.game.rules = rules;
        self.game.ucf = ucf;
    }
}

impl Comparer<'_> {
    /// Playback's state, the ruleset and UCF's pad buffer, as [`Importer::playback`] imports
    /// them. The frame index and the response only compare while playback has made its
    /// buffers.
    pub fn playback(&mut self) {
        let (patched, p) = playback(self.mem);
        let (rules, ucf) = rules(self.mem);
        let g = self.game;
        self.check("playback", "pad library patched", g.pads.playback, patched);
        self.check("playback", "rules", g.rules.clone(), rules);
        self.check("ucf", "pad buffer", g.ucf, ucf);
        match (&g.playback, p) {
            (Some(o), Some(t)) => {
                self.check("playback", "frame index", o.frame_index, t.frame_index);
                if o.frame != t.frame {
                    let at = (0..FRAME_LEN)
                        .find(|&i| o.frame.bytes[i] != t.frame.bytes[i])
                        .unwrap();
                    self.diff(
                        "playback",
                        &format!("frame response byte {at:#x}"),
                        o.frame.bytes[at],
                        t.frame.bytes[at],
                    );
                }
            }
            (None, None) => {}
            (o, t) => self.diff("playback", "state", o.is_some(), t.is_some()),
        }
    }
}
