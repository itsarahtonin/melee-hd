//! Slippi's recording codes (slippi-ssbm-asm's `Recording/`, version 3.19.1): the replay a match
//! makes, its raw event stream as the codes send it over EXI, read from the state where the
//! codes read the game's memory.
//!
//! The recorder only reads. It is presentation output, as the mix log is (`Audio::mix`): the
//! frontend turns it on before the match starts (`vs_match::Settings::record`), simulated code
//! never reads it, a clone starts without it and a rollback restore leaves it alone. Its hooks
//! sit where the codes inject:
//!
//! - **Game start** (`SendGameInfo.asm`, at `fn_8016E730+0x1C`, after playback's own code): the
//!   payload sizes, the match's `StartMeleeData` with its callbacks cleared, the seed, the
//!   rules' toggles and the Gecko code list in split messages ([`game_start`]).
//! - **Frame index** (`Common/IncrementFrameIndex.asm`, at `fn_8016CFE0+0x2B4`): -123 at the
//!   scene's first tick, then one more each frame the match isn't paused ([`count_frame`]).
//! - **Frame start** and **items**: the processes the codes give two GObjs at the match's start
//!   (`GObj_Create(4, 7, 0)`, priorities 0 and 15). In a replay's match playback makes them
//!   (`playback::RECORD_FRAME_START_PROC`); in a match of the player's own, which has no such
//!   GObjs, the recorder takes their place in the run order: in priorities 0 and 15, right
//!   before the first process of a GObj on link 7 or later ([`before_proc`]), or at the
//!   priority's end, so the seed it records is the one playback's seed process restores there.
//! - **Pre-frame** (`SendGamePreFrame.asm`, at `Fighter_procInput+0x3D0`, after playback's
//!   inputs) and **post-frame** (`SendGamePostFrame.asm`, at `Fighter_procCamera`'s end)
//!   updates for each fighter, and the L-cancel status the codes keep in the bytes they add to
//!   `Fighter` (`GetLCancelStatus.asm`, `ResetLCancelStatus.asm`).
//! - **Frame bookend** (`FlushFrameBuffer.asm`): the frame's buffered events go out together
//!   once its processes have run ([`end_of_procs`]).
//! - **Game end** (`SendGameEnd.asm`, after the VS scene's frame handler): once, when the match
//!   has an outcome, with each player's placement from the results as they stand
//!   ([`game_end`]). It is the stream's last event: Slippi Dolphin closes the file there, so
//!   the rest of that tick isn't recorded.
//!
//! Where melee-hd writes what the codes don't: the UCF toggles say whether the match's rules
//! run UCF (3.19's codes write 1 whatever runs), and a word the codes copy from memory that
//! holds a GObj (the motion state's first variable) is its address in the allocator model, 0
//! where the model is off (a match of the player's own without an Ice Climber).
//!
//! Not recorded: the 3.18 stage events (Fountain of Dreams' platforms, Whispy's wind, Pokémon
//! Stadium's transformations), which hook stage code; their sizes are declared, as the codes
//! declare them, and parsers treat them as optional.

use super::state::{Game, Running, UserData};
use super::structs::{PlayerInitData, StartMeleeData, StartMeleeRules};
use crate::hsd::gobj::GObjId;

/// The version the recording writes, `CURRENT_VERSION` (major, minor, build, unused).
pub const VERSION: [u8; 4] = [3, 19, 1, 0];

/// Event command bytes (`Recording.s`).
pub mod command {
    pub const SPLIT_MESSAGE: u8 = 0x10;
    pub const PAYLOAD_SIZES: u8 = 0x35;
    pub const GAME_START: u8 = 0x36;
    pub const PRE_FRAME: u8 = 0x37;
    pub const POST_FRAME: u8 = 0x38;
    pub const GAME_END: u8 = 0x39;
    pub const FRAME_START: u8 = 0x3A;
    pub const ITEM: u8 = 0x3B;
    pub const FRAME_BOOKEND: u8 = 0x3C;
    pub const GECKO_LIST: u8 = 0x3D;
    pub const FOD_PLATFORMS: u8 = 0x3F;
    pub const WHISPY: u8 = 0x40;
    pub const STADIUM: u8 = 0x41;
}

/// Each event's payload size, after its command byte (`Recording.s`).
pub const GAME_START_LEN: usize = 760;
pub const PRE_FRAME_LEN: usize = 66;
pub const POST_FRAME_LEN: usize = 84;
pub const GAME_END_LEN: usize = 6;
pub const FRAME_START_LEN: usize = 12;
pub const ITEM_LEN: usize = 44;
pub const FRAME_BOOKEND_LEN: usize = 8;
pub const SPLIT_MESSAGE_LEN: usize = 516;
const FOD_PLATFORMS_LEN: usize = 9;
const WHISPY_LEN: usize = 5;
const STADIUM_LEN: usize = 8;
/// A split message's data block.
const SPLIT_DATA_LEN: usize = 512;
/// Items recorded a frame, at most.
const MAX_ITEMS: usize = 15;

/// The first frame's number (`CONST_FirstFrameIdx`).
pub const FIRST_FRAME: i32 = -123;

/// `StartMeleeData`'s callbacks, which the codes clear in what they send (0x40 to 0x5C).
const CALLBACKS: core::ops::Range<usize> = 0x40..0x5C;
/// `Gm_PKind_Human`, a player's slot type the codes test for.
const PKIND_HUMAN: u8 = 0;
/// No nametag.
const NO_NAMETAG: u8 = 0x78;
/// `CharacterKind`s of Zelda and Sheik.
const CKIND_ZELDA: i8 = 0x12;
const CKIND_SHEIK: i8 = 0x13;
/// The A button in a pad status.
const PAD_A: u32 = 0x100;
/// The link the codes' GObjs are made on, and their processes' priorities.
const CODES_P_LINK: u8 = 7;
const FRAME_START_PRIORITY: usize = 0;
const ITEMS_PRIORITY: usize = 15;
/// The items' process link (`HSD_GObjPLinkHead[9]`, which `SendItemInfo` walks).
const ITEMS_P_LINK: u8 = super::it::P_LINK_ITEM;
/// Scenes as the codes write them, minor and then major: the VS mode's match, and Slippi
/// playback's.
const SCENE_VS: [u8; 2] = [2, 2];
const SCENE_PLAYBACK: [u8; 2] = [1, 14];

/// What a recording needs beyond the state: what the codes read from outside the match.
#[derive(Clone, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Setup {
    /// The Gecko code list the match ran with, as Slippi records it: the list past its
    /// `d0c0de` header, without the terminator. melee-hd's Vanilla ruleset runs none.
    pub codes: Vec<u8>,
    /// Each port's nametag text, eight Shift JIS characters, as the memory card holds it
    /// (`Nametag_LoadSlotText`): written for a human player with a nametag.
    pub nametags: [[u16; 8]; 4],
    /// Slippi's PAL toggle.
    pub pal: bool,
    /// Whether the frozen Pokémon Stadium code ran.
    pub frozen_ps: bool,
    /// The game's language (`Language_GetLanguage`): 1, English, as an NTSC disc starts.
    pub language: u8,
}

impl Setup {
    /// A match of melee-hd's Vanilla ruleset: no codes and no nametags, in English.
    pub fn vanilla() -> Setup {
        Setup {
            language: 1,
            ..Setup::default()
        }
    }
}

/// The recording: the event stream sent so far, and what the codes keep between events.
#[derive(Debug, Default)]
pub struct Recorder {
    setup: Setup,
    /// The event stream the codes sent, as a replay's raw element holds it.
    raw: Vec<u8>,
    /// The frame's events, which the codes buffer and send at its end (`bufferOffset`).
    frame_buf: Vec<u8>,
    /// The game start went out.
    started: bool,
    /// `frameIndex`.
    frame: i32,
    /// `RDB_GAME_END_SENT`.
    game_end_sent: bool,
    /// The L-cancel status the codes keep past each fighter's `Fighter` (`LCancelStatus`,
    /// 0x25FF), by the fighter's slot.
    lcancel: Vec<u8>,
    /// In a match of the player's own, whether this tick's frame start and items ran.
    frame_start_done: bool,
    items_done: bool,
    /// The last frame index with a frame start or a fighter's update.
    last_frame: Option<i32>,
    /// Each port's leader's post-frame updates by internal character, for the metadata.
    characters: [Vec<(u8, u32)>; 4],
}

impl Recorder {
    pub fn new(setup: Setup) -> Recorder {
        Recorder {
            setup,
            frame: FIRST_FRAME - 1,
            ..Recorder::default()
        }
    }

    /// The event stream sent so far: a replay's raw element. A frame whose processes are still
    /// running is buffered and not yet in it.
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }

    /// The last frame recorded, the metadata's `lastFrame`.
    pub fn last_frame(&self) -> Option<i32> {
        self.last_frame
    }

    /// How many frames each port's player (its leader, not Nana) was each internal
    /// character, the metadata's `characters`; none for an empty port.
    pub fn characters(&self, port: usize) -> &[(u8, u32)] {
        &self.characters[port]
    }

    /// The game end went out.
    pub fn ended(&self) -> bool {
        self.game_end_sent
    }

    fn game_start(&mut self, g: &Game, d: &StartMeleeData) {
        use command::*;
        let s = &self.setup;
        self.frame = FIRST_FRAME - 1;
        let mut w = W::default();
        // The payload sizes: the size byte counts itself and three bytes for each command.
        let sizes: [(u8, usize); 12] = [
            (GAME_START, GAME_START_LEN),
            (PRE_FRAME, PRE_FRAME_LEN),
            (POST_FRAME, POST_FRAME_LEN),
            (GAME_END, GAME_END_LEN),
            (FRAME_START, FRAME_START_LEN),
            (ITEM, ITEM_LEN),
            (FRAME_BOOKEND, FRAME_BOOKEND_LEN),
            (GECKO_LIST, s.codes.len()),
            (SPLIT_MESSAGE, SPLIT_MESSAGE_LEN),
            (FOD_PLATFORMS, FOD_PLATFORMS_LEN),
            (WHISPY, WHISPY_LEN),
            (STADIUM, STADIUM_LEN),
        ];
        w.u8(PAYLOAD_SIZES);
        w.u8((3 * sizes.len() + 1) as u8);
        for (c, n) in sizes {
            w.u8(c);
            // `sth`: a list longer than a halfword keeps its low half.
            w.u16(n as u16);
        }
        let start = w.0.len();
        w.u8(GAME_START);
        w.bytes(&VERSION);
        let mut info = export_start(d);
        info[CALLBACKS].fill(0);
        for port in 0..4 {
            let p = 0x60 + 0x24 * port;
            if info[p + 1] != PKIND_HUMAN {
                continue;
            }
            // Zelda holding A as the match starts is Sheik.
            if info[p] as i8 == CKIND_ZELDA && g.pads.copy[port].button & PAD_A != 0 {
                info[p] = CKIND_SHEIK as u8;
            }
            // A costume past the character's count is the first (`ble`: the count itself
            // stays).
            if info[p + 3] > super::playback::costume_count(info[p]) {
                info[p + 3] = 0;
            }
        }
        w.bytes(&info);
        w.u32(g.objs.rng.seed);
        // Dashback and shield drop fixes for each port, both UCF while it runs.
        let ucf = u32::from(g.rules.ucf != super::ucf::Ucf::Off);
        for _ in 0..8 {
            w.u32(ucf);
        }
        for (port, tag) in s.nametags.iter().enumerate() {
            let p = 0x60 + 0x24 * port;
            let has = info[p + 1] == PKIND_HUMAN && info[p + 10] != NO_NAMETAG;
            for &c in tag {
                w.u16(if has { c } else { 0 });
            }
        }
        w.u8(u8::from(s.pal));
        w.u8(u8::from(s.frozen_ps));
        w.bytes(if g.playback.is_some() {
            &SCENE_PLAYBACK
        } else {
            &SCENE_VS
        });
        // Display names, connect codes and Slippi UIDs: online only.
        w.zeros(4 * (31 + 10 + 29));
        w.u8(s.language);
        // The match ID, game number and tiebreak number: online only.
        w.zeros(51 + 4 + 4);
        debug_assert_eq!(w.0.len() - start, 1 + GAME_START_LEN);
        // The code list in split messages, through one buffer the codes reuse: past a short
        // last block it keeps the block before's bytes.
        let mut block = [0u8; SPLIT_DATA_LEN];
        let total = s.codes.len() as u16 as usize;
        let mut at = 0;
        loop {
            let left = total - at;
            let n = left.min(SPLIT_DATA_LEN);
            block[..n].copy_from_slice(&s.codes[at..at + n]);
            w.u8(SPLIT_MESSAGE);
            w.bytes(&block);
            w.u16(n as u16);
            w.u8(GECKO_LIST);
            w.u8(u8::from(left <= SPLIT_DATA_LEN));
            at += SPLIT_DATA_LEN;
            if at >= total {
                break;
            }
        }
        self.raw.extend_from_slice(&w.0);
        self.started = true;
    }

    fn frame_start(&mut self, g: &Game) {
        let mut w = W(std::mem::take(&mut self.frame_buf));
        w.u8(command::FRAME_START);
        w.i32(self.frame);
        w.u32(g.objs.rng.seed);
        w.u32(g.scene.unk_8);
        self.frame_buf = w.0;
        self.last_frame = Some(self.frame);
    }

    fn pre_frame(&mut self, g: &Game, gobj: GObjId) {
        let fp = g.fighter(gobj);
        let slot = usize::from(fp.player_idx);
        let mut w = W(std::mem::take(&mut self.frame_buf));
        w.u8(command::PRE_FRAME);
        w.i32(self.frame);
        w.u8(fp.player_idx);
        w.u8(u8::from(is_follower(g, gobj)));
        w.u32(g.objs.rng.seed);
        w.u16(fp.motion_id as u16);
        w.f32(fp.cur_pos.x);
        w.f32(fp.cur_pos.y);
        w.f32(fp.facing_dir);
        let i = &fp.input;
        w.f32(i.lstick[0].x);
        w.f32(i.lstick[0].y);
        w.f32(i.cstick[0].x);
        w.f32(i.cstick[0].y);
        w.f32(i.triggers[0]);
        w.u32(i.held_buttons[0]);
        // The master status of the player's slot (`HSD_PadMasterStatus`).
        let m = &g.pads.master[slot & 3];
        w.u16(m.button as u16);
        w.f32(m.nml_analog_l);
        w.f32(m.nml_analog_r);
        // The raw sticks of the poll the tick took, from the pad library's five-poll queue.
        let lib = &g.pads.lib;
        let mut q = i32::from(lib.qread) - 1;
        if q < 0 {
            q += 5;
        }
        let raw = lib
            .queue
            .get(q as usize)
            .map_or(Default::default(), |d| d[slot & 3]);
        w.u8(raw.stick_x as u8);
        w.f32(fp.dmg.x1830_percent);
        w.u8(raw.stick_y as u8);
        w.u8(raw.substick_x as u8);
        w.u8(raw.substick_y as u8);
        self.frame_buf = w.0;
        self.last_frame = Some(self.frame);
    }

    fn post_frame(&mut self, g: &Game, gobj: GObjId) {
        let f = g.fighter_id(gobj);
        let fp = &g.fighters[f.0];
        if fp.is_sleeping {
            return;
        }
        let follower = is_follower(g, gobj);
        let mut w = W(std::mem::take(&mut self.frame_buf));
        w.u8(command::POST_FRAME);
        w.i32(self.frame);
        w.u8(fp.player_idx);
        w.u8(u8::from(follower));
        w.u8(fp.kind as u8);
        w.u16(fp.motion_id as u16);
        w.f32(fp.cur_pos.x);
        w.f32(fp.cur_pos.y);
        w.f32(fp.facing_dir);
        w.f32(fp.dmg.x1830_percent);
        w.f32(fp.shield_health);
        w.u8(fp.x208C as u8);
        w.u8(fp.x2090 as u8);
        w.u8(fp.dmg.x18c4_source_ply as u8);
        w.u8(g.player_stocks(i32::from(fp.player_idx)) as u8);
        w.f32(fp.cur_anim_frame);
        for b in flag_bytes(fp) {
            w.u8(b);
        }
        w.bytes(&misc_as(g, fp));
        w.u8(fp.ground_or_air as u8);
        w.u16(fp.coll_data.floor.index as u16);
        w.u8((fp.co_attrs.max_jumps as u8).wrapping_sub(fp.x1968_jumpsUsed));
        w.u8(self.lcancel.get(f.0 as usize).copied().unwrap_or(0));
        // Move-induced intangibility before the game's.
        w.u8(if fp.x1988 != 0 { fp.x1988 } else { fp.x198C } as u8);
        w.f32(fp.self_vel.x);
        w.f32(fp.self_vel.y);
        w.f32(fp.x8c_kb_vel.x);
        w.f32(fp.x8c_kb_vel.y);
        w.f32(fp.gr_vel);
        w.f32(fp.dmg.x195c_hitlag_frames);
        w.u32(fp.anim_id as u32);
        w.u16(fp.dmg.x18ec_instancehitby);
        w.u16(fp.x2074.x2088);
        self.frame_buf = w.0;
        self.last_frame = Some(self.frame);
        if !follower {
            let port = usize::from(fp.player_idx & 3);
            let kind = fp.kind as u8;
            let c = &mut self.characters[port];
            match c.iter_mut().find(|e| e.0 == kind) {
                Some(e) => e.1 += 1,
                None => c.push((kind, 1)),
            }
        }
    }

    fn items(&mut self, g: &Game) {
        let mut w = W(std::mem::take(&mut self.frame_buf));
        let mut count = 0;
        let mut cur = g.world.first_in(ITEMS_P_LINK);
        while let Some(gobj) = cur {
            count += 1;
            if count > MAX_ITEMS {
                break;
            }
            let o = g.world.gobj(gobj);
            cur = o.next;
            let Some(UserData::Item(id)) = o.user_data else {
                continue;
            };
            let it = &g.items[id.0];
            w.u8(command::ITEM);
            w.i32(self.frame);
            w.u16(it.kind as u16);
            w.u8(it.msid as u8);
            w.f32(it.facing_dir);
            w.f32(it.x40_vel.x);
            w.f32(it.x40_vel.y);
            w.f32(it.pos.x);
            w.f32(it.pos.y);
            w.u16(it.xC9C as u16);
            w.f32(it.xD44_lifeTimer);
            w.u32(it.x1C as u32);
            w.bytes(&item_misc(&it.xDD4_itemVar));
            // The owner's player slot: byte 0xC of the owner GObj's user data.
            let owner = it.owner.and_then(|o| match g.world.gobj(o).user_data {
                Some(UserData::Fighter(f)) => Some(g.fighters[f.0].player_idx),
                Some(UserData::Item(i)) => Some((g.items[i.0].spawn_kind >> 24) as u8),
                _ => None,
            });
            w.u8(owner.unwrap_or(0xFF));
            w.u16(it.xDA8_short);
        }
        self.frame_buf = w.0;
    }

    fn flush(&mut self) {
        if self.frame_buf.is_empty() {
            return;
        }
        let mut w = W(std::mem::take(&mut self.frame_buf));
        w.u8(command::FRAME_BOOKEND);
        w.i32(self.frame);
        // The latest finalized frame: this one, without rollback.
        w.i32(self.frame);
        self.raw.extend_from_slice(&w.0);
        w.0.clear();
        self.frame_buf = w.0;
    }

    fn game_end(&mut self, g: &Game) {
        let method = g.vs.state.match_result;
        if method == 0 || self.game_end_sent {
            return;
        }
        // The results as they stand, on a copy (the codes fill the results' global, which the
        // match doesn't read, from the controller's): each player's placement.
        let mut copy = g.clone();
        let r = &mut copy.vs.state.x24C;
        r.outcome = method;
        r.is_teams = g.vs.start.is_teams;
        copy.compile_results();
        let mut w = W::default();
        w.u8(command::GAME_END);
        w.u8(method);
        // Who quit with L+R+A+Start.
        w.u8(if method == 7 {
            g.vs.state.pauser as u8
        } else {
            0xFF
        });
        for p in &copy.vs.state.x24C.player_standings[..4] {
            w.u8(if p.pkind == 3 { 0xFF } else { p.is_small_loser });
        }
        self.frame_buf.clear();
        self.raw.extend_from_slice(&w.0);
        self.game_end_sent = true;
    }

    fn lcancel_slot(&mut self, f: u32) -> &mut u8 {
        let i = f as usize;
        if self.lcancel.len() <= i {
            self.lcancel.resize(i + 1, 0);
        }
        &mut self.lcancel[i]
    }
}

impl Game {
    /// Runs `f` on the recorder, when the frontend turned it on, with the state to read: from
    /// the game start until the game end, the stream's last event, as Slippi Dolphin closes the
    /// file there (a replay's match that ends when its inputs run out still runs that tick).
    fn record(&mut self, f: impl FnOnce(&mut Recorder, &Game)) {
        if let Some(mut r) = self.recorder.take() {
            if r.started && !r.game_end_sent {
                f(&mut r, self);
            }
            self.recorder = Some(r);
        }
    }
}

/// The game start, `SendGameInfo.asm`, from the `StartMeleeData` the match starts from.
pub(crate) fn game_start(r: &mut Running, d: &StartMeleeData) {
    let g = &mut *r.game;
    if let Some(mut rec) = g.recorder.take() {
        rec.game_start(g, d);
        g.recorder = Some(rec);
    }
}

/// The frame index, `Common/IncrementFrameIndex.asm`: the first frame at the scene's first
/// tick, else the next.
pub(crate) fn count_frame(g: &mut Game) {
    let first = g.scene.unk_0 == 0;
    g.record(|r, _| {
        r.frame = if first {
            FIRST_FRAME
        } else {
            r.frame.wrapping_add(1)
        };
    });
}

/// A fighter's pre-frame update, `SendGamePreFrame.asm`.
pub(crate) fn pre_frame(g: &mut Game, gobj: GObjId) {
    g.record(|r, g| r.pre_frame(g, gobj));
}

/// A fighter's post-frame update, `SendGamePostFrame.asm`: none for a sleeping fighter.
pub(crate) fn post_frame(g: &mut Game, gobj: GObjId) {
    g.record(|r, g| r.post_frame(g, gobj));
}

/// `ResetLCancelStatus.asm`, at `Fighter_procMap`'s start for a fighter awake.
pub(crate) fn reset_lcancel(g: &mut Game, gobj: GObjId) {
    g.record(|r, g| *r.lcancel_slot(g.fighter_id(gobj).0) = 0);
}

/// `GetLCancelStatus.asm`, where an aerial's landing tests the L-cancel window: 1 for an
/// L-cancel, 2 for none.
pub(crate) fn lcancel(g: &mut Game, gobj: GObjId, cancelled: bool) {
    g.record(|r, g| *r.lcancel_slot(g.fighter_id(gobj).0) = if cancelled { 1 } else { 2 });
}

/// The codes' frame start process, in a replay's match.
pub(crate) fn frame_start_proc(g: &mut Game) {
    g.record(|r, g| r.frame_start(g));
}

/// The codes' item process, in a replay's match.
pub(crate) fn items_proc(g: &mut Game) {
    g.record(|r, g| r.items(g));
}

/// In a match of the player's own, which has no GObjs of the codes: before each process, the
/// frame start and items where the codes' processes would run, on link 7 at priorities 0 and
/// 15, made at the match's start before any other GObj on that link.
pub(crate) fn before_proc(g: &mut Game, gobj: GObjId) {
    if g.playback.is_some() {
        return;
    }
    let s_link = g.world.current_s_link;
    let p_link = g.world.gobj(gobj).p_link;
    g.record(|r, g| r.virtual_procs(g, s_link, p_link >= CODES_P_LINK));
}

/// After a tick's processes: the frame start and items the run order didn't reach (in a match
/// of the player's own), then the frame's events go out with its bookend
/// (`FlushFrameBuffer.asm`).
pub(crate) fn end_of_procs(g: &mut Game) {
    let playback = g.playback.is_some();
    g.record(|r, g| {
        if !playback {
            r.virtual_procs(g, usize::MAX, true);
        }
        r.frame_start_done = false;
        r.items_done = false;
        r.flush();
    });
}

/// The game end, `SendGameEnd.asm`.
pub(crate) fn game_end(g: &mut Game) {
    g.record(|r, g| r.game_end(g));
}

impl Recorder {
    /// The codes' processes a run reaches before a process at `s_link` (on link 7 or later
    /// when `late`), where the scheduler runs link 7 at all.
    fn virtual_procs(&mut self, g: &Game, s_link: usize, late: bool) {
        let runs = g.world.skip_links & (1u64 << CODES_P_LINK) == 0;
        let due = |p: usize| s_link > p || (s_link == p && late);
        if !self.frame_start_done && due(FRAME_START_PRIORITY) {
            self.frame_start_done = true;
            if runs {
                self.frame_start(g);
            }
        }
        if !self.items_done && due(ITEMS_PRIORITY) {
            self.items_done = true;
            if runs {
                self.items(g);
            }
        }
    }
}

/// `FN_GetIsFollower`: a sub-fighter whose character has no transformation (Nana).
fn is_follower(g: &Game, gobj: GObjId) -> bool {
    let fp = g.fighter(gobj);
    fp.is_sub_fighter
        && super::pl::player::mapping(g.player_ckind(i32::from(fp.player_idx))).has_transformation
            == 0
}

/// A byte from eight flags, the first the most significant bit.
fn byte(bits: [bool; 8]) -> u8 {
    bits.iter().fold(0, |b, &f| b << 1 | u8::from(f))
}

/// The state flag bytes the post-frame update copies: `Fighter`'s 0x2218, 0x221A, 0x221B,
/// 0x221C and 0x221F.
fn flag_bytes(fp: &super::structs::Fighter) -> [u8; 5] {
    let b = &fp.x221B;
    [
        byte([
            fp.allow_interrupt,
            fp.x2218_b1,
            fp.x2218_b2,
            fp.reflecting,
            fp.x2218_b4,
            fp.x2218_b5,
            fp.x2218_b6,
            fp.x2218_b7,
        ]),
        byte([
            fp.x221A_b0,
            fp.x221A_b1,
            fp.allow_sdi,
            fp.x221A_b3,
            fp.fall_fast,
            fp.x221A_b5,
            fp.x221A_b6,
            fp.x221A_b7,
        ]),
        byte([
            b.x221B_b0, b.x221B_b1, b.x221B_b2, b.x221B_b3, b.x221B_b4, b.x221B_b5, b.x221B_b6,
            b.x221B_b7,
        ]),
        byte([
            fp.x221C_b0,
            fp.x221C_b1,
            fp.x221C_b2,
            fp.x221C_b3,
            fp.x221C_b4,
            fp.x221C_b5,
            fp.x221C_b6,
            // The three-bit field's top bit; the rest is 0x221D's.
            fp.x221C_u16_y & 4 != 0,
        ]),
        byte([
            fp.x221F_b0,
            fp.x221F_b1,
            fp.x221F_b2,
            fp.is_sleeping,
            fp.is_sub_fighter,
            fp.x221F_b5,
            fp.x221F_b6,
            fp.x221F_b7,
        ]),
    ]
}

/// The motion state's first variable word (`Fighter`'s 0x2340), as memory holds it: a GObj it
/// holds by the address the allocator model gives it (0 without the model).
fn misc_as(g: &Game, fp: &super::structs::Fighter) -> [u8; 4] {
    use super::ft::vars::Handle;
    match fp.mv.peek_word(0) {
        Ok(b) => b,
        Err(Handle::GObj(o)) => g.world.gobjs.addr(o.0).to_be_bytes(),
        Err(_) => [0; 4],
    }
}

/// An item's variables' bytes the codes copy (`0xDD7`, `0xDDB`, `0xDEB`, `0xDEF`): a Samus
/// missile's kind, a turnip's face, and a charge shot's launch and power. The members melee-sim
/// keeps as types give the fields those bytes end; others' are zero.
fn item_misc(v: &super::it::ItemVars) -> [u8; 4] {
    use super::it::ItemVars;
    let low = |w: i32| w as u8;
    match v {
        ItemVars::Bytes(b) => [b[3], b[7], b[0x17], b[0x1B]],
        ItemVars::SamusMissile(m) => [low(*m.is_smash_missile.peek()), 0, 0, 0],
        ItemVars::PeachTurnip(t) => [0, low(*t.xDD8.peek()), 0, 0],
        ItemVars::SamusChargeshot(c) => [0, 0, low(*c.xDE8.peek()), low(*c.xDEC.peek())],
        ItemVars::MewtwoShadowball(s) => [0, 0, low(*s.x14.peek()), low(*s.x18.peek())],
        _ => [0; 4],
    }
}

/// A big-endian event writer.
#[derive(Default)]
struct W(Vec<u8>);

impl W {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_be_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_be_bytes());
    }
    fn i32(&mut self, v: i32) {
        self.0.extend_from_slice(&v.to_be_bytes());
    }
    fn f32(&mut self, v: f32) {
        self.u32(v.to_bits());
    }
    fn bytes(&mut self, b: &[u8]) {
        self.0.extend_from_slice(b);
    }
    fn zeros(&mut self, n: usize) {
        self.0.resize(self.0.len() + n, 0);
    }
}

/// `StartMeleeData` as the game keeps it in memory, 0x138 bytes: the inverse of its mirror's
/// import (bytes no field models are zero).
pub fn export_start(d: &StartMeleeData) -> [u8; 0x138] {
    let mut o = Bits([0; 0x138]);
    export_rules(&mut o, &d.rules);
    for (i, p) in d.players.iter().enumerate() {
        export_player(&mut o, 0x60 + 0x24 * i, p);
    }
    o.0
}

fn export_rules(o: &mut Bits<0x138>, r: &StartMeleeRules) {
    o.bits(0, 0, 3, u32::from(r.match_kind));
    o.bits(0, 3, 3, u32::from(r.x0_3));
    let flags = [
        r.timer_enabled,
        r.timer_counts_up,
        r.x1_0,
        r.x1_1,
        r.x1_2,
        r.x1_3,
        r.x1_4,
        r.x1_5,
        r.timer_shows_hours,
        r.friendly_fire,
        r.is_stock,
        r.x2_1,
        r.x2_2,
        r.single_button,
        r.disable_pausing,
        r.x2_5,
        r.x2_6,
        r.x2_7,
        r.x3_0,
        r.x3_1,
        r.x3_2,
        r.x3_3,
        r.x3_4,
        r.x3_5,
        r.x3_6,
        r.x3_7,
        r.x4_0,
        r.is_vs,
        r.x4_2,
        r.x4_3,
        r.x4_4,
        r.x4_5,
        r.x4_6,
        r.x4_7,
        r.x5_0,
        r.x5_1,
        r.x5_2,
        r.x5_3,
        r.x5_4,
        r.x5_5,
        r.x5_6,
        r.x5_7,
    ];
    for (i, &f) in flags.iter().enumerate() {
        o.bits(0, 6 + i as u32, 1, u32::from(f));
    }
    o.u8(0x6, r.x6);
    o.u8(0x7, r.x7);
    o.u8(0x8, r.is_teams);
    o.u8(0x9, r.x9);
    o.u8(0xA, r.xA);
    o.u8(0xB, r.item_freq as u8);
    o.u8(0xC, r.sd_penalty as u8);
    o.u8(0xD, r.xD);
    o.be(0xE, &r.stkind.to_be_bytes());
    o.be(0x10, &r.time_limit.to_be_bytes());
    o.u8(0x14, r.x14);
    o.be(0x18, &r.x18.to_be_bytes());
    o.be(0x1C, &r.x1C_pad[0].to_be_bytes());
    o.be(0x20, &r.x20.to_be_bytes());
    o.be(0x28, &r.x28.to_be_bytes());
    o.be(0x2C, &r.x2C.to_bits().to_be_bytes());
    o.be(0x30, &r.x30.to_bits().to_be_bytes());
    o.be(0x34, &r.game_speed.to_bits().to_be_bytes());
    let code = |c: Option<super::handles::CodeRef>| c.map_or(0, |c| c.0).to_be_bytes();
    o.be(0x38, &code(r.on_unpause_override));
    o.be(0x3C, &code(r.on_pause_override));
    o.be(0x40, &code(r.check_for_pauser_override));
    o.be(0x44, &code(r.on_match_start));
    o.be(0x48, &code(r.on_frame_start));
    o.be(0x4C, &code(r.on_frame_end));
    o.be(0x50, &code(r.on_match_end));
    o.be(0x54, &r.x54.0.to_be_bytes());
    o.be(0x58, &r.x58.0.to_be_bytes());
    o.be(0x5C, &r.pad_x5C);
}

fn export_player(o: &mut Bits<0x138>, at: usize, p: &PlayerInitData) {
    o.u8(at, p.ckind as u8);
    o.u8(at + 0x1, p.slot_type);
    o.u8(at + 0x2, p.stocks as u8);
    o.u8(at + 0x3, p.color);
    o.u8(at + 0x4, p.slot);
    o.u8(at + 0x5, p.spawn_pos as u8);
    o.u8(at + 0x6, p.spawn_dir as u8);
    o.u8(at + 0x7, p.sub_color);
    o.u8(at + 0x8, p.handicap as u8);
    o.u8(at + 0x9, p.team);
    o.u8(at + 0xA, p.nametag);
    o.u8(at + 0xB, p.xB);
    let flags = [
        p.rumble_enabled,
        p.xC_b1,
        p.vs_metal,
        p.xC_b3,
        p.vs_invisible,
        p.xC_b5,
        p.xC_b6,
        p.xC_b7,
        p.xD_b0,
        p.xD_b1,
        p.xD_b2,
        p.xD_b3,
        p.xD_b4,
        p.xD_b5,
        p.xD_b6,
        p.xD_b7,
    ];
    for (i, &f) in flags.iter().enumerate() {
        o.bits(at, 96 + i as u32, 1, u32::from(f));
    }
    o.u8(at + 0xE, p.cpu_kind);
    o.u8(at + 0xF, p.cpu_level);
    o.be(at + 0x10, &p.damage.to_be_bytes());
    o.be(at + 0x12, &p.damage1.to_be_bytes());
    o.be(at + 0x14, &p.hp.to_be_bytes());
    o.be(at + 0x18, &p.attack_ratio.to_bits().to_be_bytes());
    o.be(at + 0x1C, &p.defense_ratio.to_bits().to_be_bytes());
    o.be(at + 0x20, &p.model_scale.to_bits().to_be_bytes());
}

/// Bytes written as the mirrors read them (`Mem::bits`: bits counted from each byte's most
/// significant).
struct Bits<const N: usize>([u8; N]);

impl<const N: usize> Bits<N> {
    fn u8(&mut self, at: usize, v: u8) {
        self.0[at] = v;
    }
    fn be(&mut self, at: usize, b: &[u8]) {
        self.0[at..at + b.len()].copy_from_slice(b);
    }
    fn bits(&mut self, at: usize, bit: u32, bits: u32, v: u32) {
        for i in 0..bits {
            let b = bit + i;
            let on = v >> (bits - 1 - i) & 1 != 0;
            let byte = &mut self.0[at + b as usize / 8];
            let mask = 0x80 >> (b % 8);
            if on {
                *byte |= mask;
            } else {
                *byte &= !mask;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::mirror::{Mem, Mirror};

    struct NoPointers;
    impl crate::game::mirror::Cx for NoPointers {
        fn import_handle(&mut self, _: &'static str, _: u32) -> Option<u32> {
            None
        }
        fn import_data(&mut self, _: u32) -> Option<melee_data::archive::DataRef> {
            None
        }
        fn compare_handle(&mut self, _: &str, _: &'static str, _: Option<u32>, _: u32) {}
        fn compare_data(&mut self, _: &str, _: Option<melee_data::archive::DataRef>, _: u32) {}
        fn mismatch(&mut self, path: &str, ours: String, theirs: String) {
            panic!("{path}: {ours} vs {theirs}");
        }
    }

    #[test]
    fn start_data_round_trips() {
        // Every modeled byte set, the unmodeled ones (0x15..0x18 and each player's 0x16..0x18)
        // left zero.
        let mut bytes = [0u8; 0x138];
        for (i, b) in bytes.iter_mut().enumerate() {
            *b = (i as u8).wrapping_mul(37).wrapping_add(11);
        }
        bytes[0x15..0x18].fill(0);
        for p in 0..6 {
            let at = 0x60 + 0x24 * p;
            bytes[at + 0x16..at + 0x18].fill(0);
        }
        let m = Mem {
            bytes: &bytes,
            base: 0,
        };
        let d = StartMeleeData::import(&m, 0, &mut NoPointers);
        assert_eq!(export_start(&d), bytes);
        d.compare(&m, 0, "start", &mut NoPointers);
    }
}
