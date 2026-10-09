//! Slippi replay playback: what Slippi's playback code set (Slippi Dolphin's playback Gecko
//! codes, `crates/slippi/data/playback.ini` in ssbm-rs) does to a VS match, with the replay's
//! settings and frames as explicit inputs instead of the Slippi EXI device's answers.
//!
//! In a replay run the codes patch the VS scene's entry, frame and setup and the pad library:
//!
//! - **Entry** (`fn_8016E730`, after `db_Setup`): the replay's game start replaces the match's
//!   `StartMeleeData` and random seed, and a GObj is made whose process restores each frame's
//!   recorded seed. [`Settings::from_replay`](super::vs_match::Settings::from_replay) builds the
//!   settings; [`Running::playback_start`] makes the GObj.
//! - **Frame** (`fn_8016CFE0`): the replay's frame number is counted from -123, each frame's
//!   recorded inputs are read, a team player out of stocks steals one when the replay has them
//!   back, and the match ends when the replay does.
//! - **Inputs** (`Fighter_procInput`): each fighter's sticks, trigger and buttons become the
//!   recorded ones, after the game has read its own controller, and the raw stick values go
//!   back into the pad library's queue, where UCF reads them.
//! - **Pad library** (`HSD_PadRenewRawStatus`, `HSD_PadRenewMasterStatus`): every engine tick
//!   renews the master statuses from the newest poll, whether or not one arrived.
//!
//! Other codes in the set don't change the simulation: the debug menu and scene transitions that
//! boot into the match, the recording codes (which send the replay being made over EXI), the
//! sound bookkeeping that undoes sounds a rollback replays (it never acts on a replay played
//! forward), the mixer volumes the scene loop resets each tick, and the HUD's player names. The
//! general codes' unlocks don't reach a VS match. Codes that patch fighter code no port covers
//! yet are listed in `docs/oracle.md` for those ports to follow.

use super::handles::CodeRef;
use super::state::{Game, Running};
use crate::hsd::gobj::GObjId;

/// The size of a frame response: code, rollback flag, seed flag and seed, then each port's
/// leader and follower.
pub const FRAME_LEN: usize = 7 + 4 * 2 * CHARACTER_LEN;
/// One character's data in a frame response (`CHARACTER_DATA_LEN`).
pub const CHARACTER_LEN: usize = 52;

/// A frame response's first byte (`FRAME_RESP_*`).
pub const RESP_WAIT: u8 = 0;
pub const RESP_CONTINUE: u8 = 1;
pub const RESP_TERMINATE: u8 = 2;

/// The replay's first frame, which the scene's first tick reads (`GAME_FIRST_FRAME`).
pub const FIRST_FRAME: i32 = -123;

/// The process playback gives its GObj: the code at `0x8016E748`'s entry at 0x4B0, which
/// playback places on the heap (ssbm-rs's `code_8016E748_4B0`), named by its site and offset.
pub const SEED_PROC: u32 = 0x8016_E748 + 0x4B0;
/// The recording codes' processes (the code at `0x8016E74C`): one sends each frame's start
/// and seed, the other the items, over EXI. Neither changes the game.
pub const RECORD_FRAME_START_PROC: u32 = 0x8016_E74C + 0x6E0;
pub const RECORD_ITEMS_PROC: u32 = 0x8016_E74C + 0x788;

/// What a pre-frame update recorded for one character, as the frame response carries it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CharacterFrame {
    pub seed: u32,
    pub stick_x: f32,
    pub stick_y: f32,
    pub cstick_x: f32,
    pub cstick_y: f32,
    pub trigger: f32,
    pub buttons: u32,
    pub pos_x: f32,
    pub pos_y: f32,
    pub facing: f32,
    pub action: u32,
    pub raw_stick_x: u8,
    pub raw_stick_y: u8,
    pub percent: f32,
    pub raw_cstick_x: u8,
    pub raw_cstick_y: u8,
}

impl CharacterFrame {
    /// The bytes a frame response holds for it.
    pub fn to_bytes(&self) -> [u8; CHARACTER_LEN] {
        let mut b = [0; CHARACTER_LEN];
        let words = [
            self.seed,
            self.stick_x.to_bits(),
            self.stick_y.to_bits(),
            self.cstick_x.to_bits(),
            self.cstick_y.to_bits(),
            self.trigger.to_bits(),
            self.buttons,
            self.pos_x.to_bits(),
            self.pos_y.to_bits(),
            self.facing.to_bits(),
            self.action,
        ];
        for (i, w) in words.iter().enumerate() {
            b[4 * i..4 * i + 4].copy_from_slice(&w.to_be_bytes());
        }
        b[0x2C] = self.raw_stick_x;
        b[0x2D] = self.raw_stick_y;
        b[0x2E..0x32].copy_from_slice(&self.percent.to_bits().to_be_bytes());
        b[0x32] = self.raw_cstick_x;
        b[0x33] = self.raw_cstick_y;
        b
    }
}

/// A frame response as playback keeps it: the bytes the codes read words and bytes from.
#[derive(melee_derive::CloneFrom, Debug, PartialEq)]
pub struct Frame {
    pub bytes: [u8; FRAME_LEN],
}

impl Default for Frame {
    /// The zeroed buffer playback allocates before its first read.
    fn default() -> Self {
        Frame {
            bytes: [0; FRAME_LEN],
        }
    }
}

/// Where a replay's match gets what the Slippi EXI device would answer.
pub trait Source {
    /// The response to a request for frame `index`: the frame, or [`Frame::terminate`] past
    /// the replay's end.
    fn frame(&mut self, index: i32) -> Frame;
    /// Whether the replay has a player in `port` at frame `index` (a stock steal's answer).
    fn has_player(&mut self, index: i32, port: u8) -> bool;
}

/// What a tick of a replay's match may ask the device, answered before it runs: the frame it
/// reads if it reads one, and which ports have a player at the frame it ends up counted at
/// (the one it reads, or the last if it reads none).
#[derive(melee_derive::CloneFrom, Debug, PartialEq)]
pub struct Answers {
    pub index: i32,
    pub frame: Frame,
    pub players: [(i32, [bool; 4]); 2],
}

impl Frame {
    /// A frame of the replay: its recorded seed, if the replay has one, and each port's
    /// leader and follower.
    pub fn new(seed: Option<u32>, players: [[Option<CharacterFrame>; 2]; 4]) -> Frame {
        let mut bytes = [0; FRAME_LEN];
        bytes[0] = RESP_CONTINUE;
        bytes[2] = u8::from(seed.is_some());
        bytes[3..7].copy_from_slice(&seed.unwrap_or(0).to_be_bytes());
        for (port, chars) in players.iter().enumerate() {
            for (follower, c) in chars.iter().enumerate() {
                if let Some(c) = c {
                    let at = 7 + port * 2 * CHARACTER_LEN + follower * CHARACTER_LEN;
                    bytes[at..at + CHARACTER_LEN].copy_from_slice(&c.to_bytes());
                }
            }
        }
        Frame { bytes }
    }

    /// The answer past the replay's end: the response code, and zeros where the frame's data
    /// would be (the device's read fills the rest of the buffer with zeros).
    pub fn terminate() -> Frame {
        let mut bytes = [0; FRAME_LEN];
        bytes[0] = RESP_TERMINATE;
        Frame { bytes }
    }

    pub fn code(&self) -> u8 {
        self.bytes[0]
    }

    fn word(&self, at: usize) -> u32 {
        u32::from_be_bytes(self.bytes[at..at + 4].try_into().unwrap())
    }

    /// The word at `off` in the character data of `port`'s leader (0) or follower (1).
    fn char_word(&self, port: usize, follower: usize, off: usize) -> u32 {
        self.word(7 + port * 2 * CHARACTER_LEN + follower * CHARACTER_LEN + off)
    }

    fn char_byte(&self, port: usize, follower: usize, off: usize) -> u8 {
        self.bytes[7 + port * 2 * CHARACTER_LEN + follower * CHARACTER_LEN + off]
    }
}

/// Playback's state in a match (`lbl_804D6660`'s buffers and the frame number the codes keep
/// in `mnCharSel_804D6CF4`'s word).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq)]
pub struct Playback {
    /// The replay's random seed, which the match's start restores.
    pub start_seed: u32,
    /// The replay frame last counted.
    pub frame_index: i32,
    /// The last frame read: its inputs are what the fighters get until the next read.
    pub frame: Frame,
    /// What the frontend answered for this tick, while it runs.
    pub answers: Option<Answers>,
}

impl Playback {
    /// The frame a tick would read next: the first at the scene's first tick, else the one
    /// after the last counted.
    pub fn next_index(game: &Game) -> i32 {
        let p = game.playback.as_ref().expect("a replay's match");
        if game.scene.unk_0 == 0 {
            FIRST_FRAME
        } else {
            p.frame_index.wrapping_add(1)
        }
    }

    /// The device's answers for the next tick, from `source`.
    pub fn answers(game: &Game, source: &mut dyn Source) -> Answers {
        let next = Playback::next_index(game);
        let now = game
            .playback
            .as_ref()
            .expect("a replay's match")
            .frame_index;
        let players = |source: &mut dyn Source, i| {
            (i, core::array::from_fn(|p| source.has_player(i, p as u8)))
        };
        Answers {
            index: next,
            frame: source.frame(next),
            players: [players(source, now), players(source, next)],
        }
    }
}

impl Game {
    fn playback_mut(&mut self) -> &mut Playback {
        self.playback.as_mut().expect("a replay's match")
    }

    /// Counts the replay's frame (`fn_8016CFE0+0x2B4`): the first frame at the scene's first
    /// tick, else the next.
    ///
    /// The Gecko code at `0x8016D294`.
    pub fn playback_count_frame(&mut self) {
        let first = self.scene.unk_0 == 0;
        let p = self.playback_mut();
        p.frame_index = if first {
            FIRST_FRAME
        } else {
            p.frame_index.wrapping_add(1)
        };
    }

    /// Reads the counted frame's response (`fn_8016CFE0+0x2B8`, which asks the device until
    /// it answers): the frame the frontend supplied for it.
    ///
    /// The Gecko code at `0x8016D298`.
    pub fn playback_read_frame(&mut self) {
        let p = self.playback_mut();
        let index = p.frame_index;
        let a = p
            .answers
            .as_ref()
            .unwrap_or_else(|| panic!("the replay's frame {index} wasn't supplied"));
        assert_eq!(
            a.index, index,
            "the frontend supplied frame {}, the match read frame {index}",
            a.index
        );
        let frame = a.frame.clone();
        assert!(
            frame.code() != RESP_WAIT,
            "the replay has no data for frame {index} yet"
        );
        p.frame = frame;
    }

    /// Whether the replay has ended (`fn_8016CFE0+0x324`): the last read answered that it
    /// terminated.
    pub fn playback_terminated(&self) -> bool {
        self.playback
            .as_ref()
            .is_some_and(|p| p.frame.code() == RESP_TERMINATE)
    }

    /// Whether a team player out of stocks takes one from a teammate this frame
    /// (`fn_8016B918+0xA8`): the replay has `port`'s player at the counted frame.
    ///
    /// The Gecko code at `0x8016B9C0`.
    pub fn playback_stock_steal(&self, port: u8) -> bool {
        let p = self.playback.as_ref().expect("a replay's match");
        let a = p.answers.as_ref().expect("the device's answers");
        let (_, players) = a
            .players
            .iter()
            .find(|(i, _)| *i == p.frame_index)
            .expect("an answer for the counted frame");
        players[usize::from(port & 3)]
    }

    /// Playback's per-frame process (`code_8016E748_4B0`): the frame's recorded seed, if it
    /// has one, becomes the random seed.
    ///
    /// The Gecko code at `0x8016E748`, from its entry at 0x4B0.
    pub fn playback_restore_seed(&mut self) {
        let f = &self.playback.as_ref().expect("a replay's match").frame;
        if f.bytes[2] != 0 {
            self.objs.rng.seed = f.word(3);
        }
    }
}

/// How many costumes each character has (`lbl_803D51A0`'s first column), by `CharacterKind`.
const COSTUMES: [u8; 26] = [
    6, 5, 4, 4, 6, 4, 5, 4, 5, 5, 4, 4, 5, 4, 4, 5, 5, 6, 5, 5, 4, 5, 5, 5, 4, 5,
];

/// A character's costume count, `gm_GetNumCostumesForCKind`: none past the playable ones.
#[doc(alias = "gm_GetNumCostumesForCKind")]
pub fn costume_count(ckind: u8) -> u8 {
    COSTUMES.get(usize::from(ckind)).copied().unwrap_or(0)
}

impl Game {
    /// The general codes' costume rules at the end of the match's setup (`fn_8016DCC0+0x214`,
    /// in playback's `Required: General Codes`): a costume the character doesn't have becomes
    /// its first, and of players with the same character, costume and shade, each later one
    /// gets the next shade.
    ///
    /// The Gecko code at `0x8016DED4`.
    pub fn playback_settle_costumes(&mut self) {
        const NONE: i32 = 3;
        for i in 0..6 {
            if self.player_pkind(i) == NONE {
                continue;
            }
            let costume = self.player_costume(i) as i32;
            if costume >= i32::from(costume_count(self.player_ckind(i) as u8)) {
                self.set_player_costume(i, 0);
            }
        }
        for i in 0..6 {
            if self.player_pkind(i) == NONE {
                continue;
            }
            let (ckind, costume, shade) = (
                self.player_ckind(i),
                self.player_costume(i),
                self.player_sub_color(i),
            );
            for j in 0..6 {
                if j == i || self.player_pkind(j) == NONE {
                    continue;
                }
                if self.player_ckind(j) == ckind
                    && self.player_costume(j) == costume
                    && self.player_sub_color(j) == shade
                {
                    self.set_player_sub_color(j, shade.wrapping_add(1) as i8);
                }
            }
        }
    }
}

impl Running<'_> {
    /// Playback's part of the match's start (`fn_8016E730+0x18`, after `db_Setup`): the
    /// replay's seed, and the GObj whose process restores each frame's seed (`GObj_Create(4,
    /// 7, 0)`, its process at priority 0); then the recording codes' (`fn_8016E730+0x1C`):
    /// two more such GObjs, whose processes (priorities 0 and 15) send each frame's start and
    /// items over EXI. The replay's `StartMeleeData` and settings are
    /// [`Settings::from_replay`](super::vs_match::Settings::from_replay)'s.
    ///
    /// The Gecko codes at `0x8016E748` and `0x8016E74C`.
    pub fn playback_start(&mut self, seed: u32) {
        let g = &mut *self.game;
        g.objs.rng.seed = seed;
        for (proc, priority) in [
            (SEED_PROC, 0),
            (RECORD_FRAME_START_PROC, 0),
            (RECORD_ITEMS_PROC, 15),
        ] {
            let gobj = g.world.create(4, 7, 0);
            g.world.setup_proc(gobj, CodeRef(proc), priority);
        }
    }

    /// The recorded inputs replace what a fighter read from its controller
    /// (`Fighter_procInput+0x3CC`): sticks, trigger and buttons from the frame read last, and
    /// the raw stick values into the pad library's queue at the poll this tick took. A
    /// follower (Nana) whose character isn't a transformation keeps its own. At the replay's
    /// first frame the fighter's position is settled: its first part's rotation, its
    /// collision positions, its player block's position, its camera box and the camera.
    ///
    /// The Gecko code at `0x8006B0DC`.
    pub fn playback_inject(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        // `stub_800055F8`: a sub-fighter whose character has no transformation.
        if fp.is_sub_fighter {
            let ckind = self.game.player_ckind(i32::from(fp.player_idx));
            if super::pl::player::mapping(ckind).has_transformation == 0 {
                return;
            }
        }
        let port = usize::from(fp.player_idx);
        let pad_port = usize::from(fp.pad_port);
        let p = self.game.playback.as_ref().expect("a replay's match");
        let frame = &p.frame;
        let w = |off| frame.char_word(port, 0, off);
        let (sx, sy, cx, cy, trigger, buttons) = (w(4), w(8), w(0xC), w(0x10), w(0x14), w(0x18));
        let b = |off| frame.char_byte(port, 0, off);
        let raw = [b(0x2C), b(0x2D), b(0x32), b(0x33)];
        let first = p.frame_index == FIRST_FRAME;
        let fp = &mut self.game.fighters[f.0];
        fp.input.lstick[0].x = f32::from_bits(sx);
        fp.input.lstick[0].y = f32::from_bits(sy);
        fp.input.cstick[0].x = f32::from_bits(cx);
        fp.input.cstick[0].y = f32::from_bits(cy);
        fp.input.triggers[0] = f32::from_bits(trigger);
        fp.input.held_buttons[0] = buttons;
        let lib = &mut self.game.pads.lib;
        let mut slot = i32::from(lib.qread) - 1;
        if slot < 0 {
            slot += 5;
        }
        let s = &mut lib.queue[slot as usize][pad_port];
        s.stick_x = raw[0] as i8;
        s.stick_y = raw[1] as i8;
        s.substick_x = raw[2] as i8;
        s.substick_y = raw[3] as i8;
        if first {
            self.playback_settle_first_frame(gobj);
        }
    }

    /// The first frame's settling of a fighter (`Fighter_procInput+0x3CC`'s tail), which
    /// would apply a resynced position: the codes never resync a replay played back.
    fn playback_settle_first_frame(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        // `@282`, the float the codes rotate the fighter's first part by.
        self.game.part_set_rot_x(f, 0, 0.0);
        let generation = super::gr::ground::generation(self.game);
        let fp = &mut self.game.fighters[f.0];
        let pos = fp.cur_pos;
        fp.coll_data.cur_pos = pos;
        fp.coll_data.last_pos = pos;
        fp.mv[4..8].copy_from_slice(&pos.y.to_bits().to_be_bytes());
        fp.coll_data.x38 = generation;
        let (slot, which) = (i32::from(fp.player_idx), i32::from(fp.is_sub_fighter));
        self.game.set_player_entity_pos(slot, which, pos);
        self.update_camera_box(gobj);
        let fp = &self.game.fighters[f.0];
        let b = fp.x890_cameraBox.expect("a fighter's camera box");
        let cb = &mut self.game.camera.boxes[b];
        cb.ext.h.x = cb.target_ext.h.x;
        cb.ext.h.y = cb.target_ext.h.y;
        super::camera::snap(self);
    }
}
