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
//! The code list is the one the match runs, as Slippi's codes send it from the code handler's
//! list in memory: a replay's own, or under the Slippi ruleset Slippi 3.19.1's as melee-hd runs
//! it ([`ruleset_codes`]), with UCF's pad buffer as the game holds it at the game start. A
//! match the menus started takes its scene, its players' name tags and the saved language
//! from the game, where the codes read them; a match started some other way (a replay's, the
//! command line's, a test's) takes them from its [`Setup`].
//!
//! Where melee-hd writes what the codes don't: the UCF toggles say whether the match's rules
//! run UCF (3.19's codes write 1 whatever runs), a word the codes copy from memory that holds
//! a GObj (the motion state's first variable) is its address in the allocator model, 0 where
//! the model is off (a match of the player's own without an Ice Climber), and two UCF codes'
//! scratch doubles in the code list are their boot value (below, [`ruleset_codes`]). Under
//! Vanilla the metadata also carries the unlocks the match read ([`Setup::unlocks`]).
//!
//! Not recorded: the 3.18 stage events (Fountain of Dreams' platforms, Whispy's wind, Pokémon
//! Stadium's transformations), which hook stage code; their sizes are declared, as the codes
//! declare them, and parsers treat them as optional.

use super::state::{Game, Running, UserData};
use super::structs::{PlayerInitData, StartMeleeData, StartMeleeRules};
use crate::game::ft::CharacterKind;
use crate::game::pl::player::PlayerKind;
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
/// No nametag.
const NO_NAMETAG: u8 = 0x78;
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
    /// `d0c0de` header, without the terminator. melee-hd's Vanilla ruleset runs none. Where it
    /// has UCF's pad buffer, the recording writes the buffer as the game holds it at the game
    /// start.
    pub codes: Vec<u8>,
    /// Each port's nametag text, eight Shift JIS characters, as the memory card holds it
    /// (`GetNameText`): written for a human player with a nametag, in a match not started from
    /// the menus (a match the menus started reads the game's own name tags).
    pub nametags: [[u16; 8]; 4],
    /// Slippi's PAL toggle.
    pub pal: bool,
    /// Whether the frozen Pokémon Stadium code ran.
    pub frozen_ps: bool,
    /// The saved language (`lbLang_GetSavedLanguage`), in a match not started from the menus
    /// (one the menus started reads the game's): 1, English, as an NTSC disc starts.
    pub language: u8,
    /// Whether the metadata carries the unlocks the match read (`gm::Unlocks`, as the game
    /// held them at the game start): a Vanilla match's, whose save decides them, where Slippi's
    /// playback runs its own unlock codes (milestone 5's call of 2026-10-23).
    pub unlocks: bool,
    /// The scene, minor then major, for a match not started from the menus' own scene: a live
    /// match the Slippi ruleset builds as playback builds a replay's (`Playback::live`) records
    /// the scene the menus started it in; none is the VS mode's match.
    pub scene: Option<[u8; 2]>,
}

impl Setup {
    /// A match of melee-hd's Vanilla ruleset: no codes, no nametags and English unless the
    /// menus started it, and the unlocks in the metadata. The oracle's recordings take it; the
    /// player saves no Vanilla replays since 2026-11-19 (Sarah: "Slippi only").
    pub fn vanilla() -> Setup {
        Setup {
            language: 1,
            unlocks: true,
            ..Setup::default()
        }
    }

    /// A match of melee-hd's Slippi ruleset: Slippi 3.19.1's code list as melee-hd runs it
    /// ([`ruleset_codes`]), whose unlock codes stand for the save's.
    pub fn slippi() -> Setup {
        Setup {
            codes: ruleset_codes(),
            language: 1,
            ..Setup::default()
        }
    }
}

/// Slippi 3.19.1's code list as melee-hd runs it under the Slippi ruleset (design call 4 of
/// `docs/slippi-ruleset.md`): the 16 unlock and message writes of Slippi Dolphin's General
/// Codes, which Slippi's playback runs itself and leaves out of a replay's list, and the 27
/// codes it tags as affecting gameplay, which it applies from the list (`Rules::from_codes`
/// reads them as 3.19's with UCF 0.84), in Slippi Dolphin's order and with the bytes every
/// 3.19.1 replay records, each `C2` code's branch back included. Not Slippi Dolphin's other
/// 245 codes, which melee-hd doesn't run.
///
/// The data UCF keeps in its codes' words is the boot's: the pad buffer, which the recording
/// replaces with the game's at the game start, and the scratch doubles of the down-smash out
/// of crouch fix (`0x800D65EC`) and the shield drop (`0x800998A4`). Each of those two stores
/// its conversion's double (`stfd`) before it reads the word back, so no run reads what the
/// list holds there; Slippi Dolphin's list has the last conversion's, melee-hd's the boot's.
pub fn ruleset_codes() -> Vec<u8> {
    let mut out = Vec::with_capacity(5392);
    for line in RULESET_CODES.lines().filter(|l| !l.starts_with('#')) {
        for word in line.split_whitespace().take(2) {
            let w = u32::from_str_radix(word, 16).expect("a code word");
            out.extend_from_slice(&w.to_be_bytes());
        }
    }
    out
}

const RULESET_CODES: &str = include_str!("slippi-ruleset-3.19.1.txt");

/// The 16 writes at the list's head (two words each), which Slippi's playback leaves out of a
/// replay's list: it runs its own general codes.
const RULESET_WRITES: usize = 16 * 8;

/// The code list Slippi's playback applies from a recording of the ruleset's (its device's
/// `prepareGeckoList`): [`ruleset_codes`] with UCF's pad buffer as `pads` holds it, without
/// the 16 writes playback denies, and the terminator.
pub fn ruleset_playback_codes(pads: &super::ucf::Pads) -> Vec<u8> {
    let mut out = codes_as_they_stand(&ruleset_codes(), pads)[RULESET_WRITES..].to_vec();
    out.extend_from_slice(&[0xFF, 0, 0, 0, 0, 0, 0, 0]);
    out
}

/// A live match of the Slippi ruleset as the VS scene's entry would start it from `d` in `g`
/// (design call 3 of `docs/slippi-ruleset.md`): the game start the recording codes write there
/// (`fn_8016E730+0x1C`), as the replay start Slippi's playback builds the match from, and the
/// recorder's setup for what the match can't read from its own state (the menus' name tags,
/// saved language and scene). `vs_match::Settings::live` builds the match from it, so it is the
/// match a playback of its recording builds.
pub fn live_start(g: &Game, d: &StartMeleeData) -> (super::vs_match::ReplayStart, Setup) {
    let mut info = export_start(d);
    info[CALLBACKS].fill(0);
    let mut nametags = [[0u16; 8]; 4];
    for (port, tag) in nametags.iter_mut().enumerate() {
        let p = 0x60 + 0x24 * port;
        if info[p + 1] != PlayerKind::Human {
            continue;
        }
        // As the recording codes write it ([`Recorder::game_start`]): Zelda held A as the
        // match starts is Sheik, a costume past the character's count the first.
        if info[p] as i8 == CharacterKind::Zelda && g.pads.copy[port].button & PAD_A != 0 {
            info[p] = CharacterKind::Seak.raw() as u8;
        }
        if info[p + 3] > super::playback::costume_count(info[p]) {
            info[p + 3] = 0;
        }
        if info[p + 10] != NO_NAMETAG {
            *tag = name_tag_text(g, info[p + 10]);
        }
    }
    let seed = g.objs.rng.seed;
    let codes = ruleset_playback_codes(&g.ucf);
    // The device's whole answer, as `prepareGameInfo` queues it from the recording.
    let mut response = vec![1u8];
    response.extend_from_slice(&seed.to_be_bytes());
    response.extend_from_slice(&info);
    let ucf = u32::from(g.rules.ucf_on());
    for _ in 0..8 {
        response.extend_from_slice(&ucf.to_be_bytes());
    }
    for tag in nametags {
        for h in tag {
            response.extend_from_slice(&h.to_be_bytes());
        }
    }
    // Not PAL; Pokémon Stadium's transformations preload (a 1.3 or later recording); not
    // frozen; then the display names, online only.
    response.extend_from_slice(&[0, 1, 0, 0]);
    response.extend_from_slice(&[0; 4 * 31]);
    response.extend_from_slice(&(codes.len() as u32).to_be_bytes());
    let r = &g.machine.routing;
    let setup = Setup {
        nametags,
        language: g.game_data.saved_language(),
        scene: Some([r.curr_state_id, r.curr_mode]),
        ..Setup::slippi()
    };
    let start = super::vs_match::ReplayStart {
        info: info.to_vec(),
        seed,
        nametags,
        codes,
        response,
        unlocks: None,
    };
    (start, setup)
}

/// UCF's pad buffer code's injection (`game::slippi::UCF_PAD_BUFFER`) and where its data
/// starts in it: the twelve words after the body's first.
const PAD_BUFFER_DATA: core::ops::Range<usize> = 8 + 4..8 + 4 + 48;

/// `codes` with UCF's pad buffer, if the list has it, holding `pads`: the list as the code
/// handler's memory holds it.
fn codes_as_they_stand(codes: &[u8], pads: &super::ucf::Pads) -> Vec<u8> {
    let mut out = codes.to_vec();
    let word = |at: usize| u32::from_be_bytes(codes[at..at + 4].try_into().unwrap());
    let mut at = 0;
    while at + 8 <= codes.len() {
        let kind = codes[at] & 0xFE;
        let addr = (word(at) & 0x01FF_FFFF) | 0x8000_0000;
        let len = match kind {
            0xC0 | 0xC2 => 8 + 8 * word(at + 4) as usize,
            0x08 => 16,
            0x06 => 8 + ((word(at + 4) as usize + 7) & !7),
            0xF0 | 0xFE => break,
            _ => 8,
        };
        if kind == 0xC2 && addr == super::slippi::UCF_PAD_BUFFER && at + len <= codes.len() {
            out[at + PAD_BUFFER_DATA.start..at + PAD_BUFFER_DATA.end]
                .copy_from_slice(&pads.to_bytes());
        }
        at += len;
    }
    out
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
    /// The unlocks the match read, for the metadata, where the setup asks for them.
    unlocks: Option<super::gm::Unlocks>,
    /// At the game end, how many display-phase reads' outcomes differed and the game tick of
    /// the first, when some did (`live::DisplayPhase`, design call 9): the match may not play
    /// back exactly.
    display_phase: Option<(u32, u32)>,
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

    /// The unlocks the match read, for the metadata: a Vanilla match's ([`Setup::unlocks`]).
    pub fn unlocks(&self) -> Option<&super::gm::Unlocks> {
        self.unlocks.as_ref()
    }

    /// The note the metadata and the player's log carry when display-phase reads' outcomes
    /// differed or were made off playback's course (`live::DisplayPhase`, design call 9 of
    /// `docs/slippi-ruleset.md`; Sarah's wording of 2026-11-18).
    pub fn playback_note(&self) -> Option<String> {
        self.display_phase.map(|(n, first)| {
            format!(
                "may not play back exactly: {n} time{}, first at tick {first}, the match read something Slippi's playback can show differently (where the camera was, or a random number after Brinstar Depths' haze)",
                if n == 1 { "" } else { "s" }
            )
        })
    }

    fn game_start(&mut self, g: &Game, d: &StartMeleeData) {
        use command::*;
        self.frame = FIRST_FRAME - 1;
        self.unlocks = self.setup.unlocks.then_some(g.gm.unlocks);
        let s = &self.setup;
        // A match the menus started: the scenes' loop runs it, and the game's save data is the
        // session's.
        let from_menus = g.machine.phase != super::gm::machine::Phase::Boot;
        let codes = codes_as_they_stand(&s.codes, &g.ucf);
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
            (GECKO_LIST, codes.len()),
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
            if info[p + 1] != PlayerKind::Human {
                continue;
            }
            // Zelda holding A as the match starts is Sheik.
            if info[p] as i8 == CharacterKind::Zelda && g.pads.copy[port].button & PAD_A != 0 {
                info[p] = CharacterKind::Seak.raw() as u8;
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
        let ucf = u32::from(g.rules.ucf_on());
        for _ in 0..8 {
            w.u32(ucf);
        }
        for port in 0..4 {
            let p = 0x60 + 0x24 * port;
            let has = info[p + 1] == PlayerKind::Human && info[p + 10] != NO_NAMETAG;
            let tag = match (has, from_menus) {
                (false, _) => [0; 8],
                (true, true) => name_tag_text(g, info[p + 10]),
                (true, false) => s.nametags[port],
            };
            for c in tag {
                w.u16(c);
            }
        }
        w.u8(u8::from(s.pal));
        w.u8(u8::from(s.frozen_ps));
        // The scene controller's current minor and major (`gm_80479D30`).
        let r = &g.machine.routing;
        w.bytes(&if g.replay_drives() {
            SCENE_PLAYBACK
        } else if from_menus {
            [r.curr_state_id, r.curr_mode]
        } else {
            s.scene.unwrap_or(SCENE_VS)
        });
        // Display names, connect codes and Slippi UIDs: online only.
        w.zeros(4 * (31 + 10 + 29));
        w.u8(if from_menus {
            g.game_data.saved_language()
        } else {
            s.language
        });
        // The match ID, game number and tiebreak number: online only.
        w.zeros(51 + 4 + 4);
        debug_assert_eq!(w.0.len() - start, 1 + GAME_START_LEN);
        // The code list in split messages, through one buffer the codes reuse: past a short
        // last block it keeps the block before's bytes.
        let mut block = [0u8; SPLIT_DATA_LEN];
        let total = codes.len() as u16 as usize;
        let mut at = 0;
        loop {
            let left = total - at;
            let n = left.min(SPLIT_DATA_LEN);
            block[..n].copy_from_slice(&codes[at..at + n]);
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
        w.u32(g.scene.game_ticks);
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
        w.f32(fp.dmg.percent);
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
        w.f32(fp.dmg.percent);
        w.f32(fp.shield_health);
        w.u8(fp.combo_attack_id as u8);
        w.u8(fp.combo_count as u8);
        w.u8(fp.dmg.source_player as u8);
        w.u8(g.player_stocks(i32::from(fp.player_idx)) as u8);
        w.f32(fp.cur_anim_frame);
        for b in flag_bytes(fp) {
            w.u8(b);
        }
        w.bytes(&misc_as(g, fp));
        w.u8(fp.ground_or_air.raw() as u8);
        w.u16(fp.coll_data.floor.index as u16);
        w.u8((fp.co_attrs.max_jumps as u8).wrapping_sub(fp.jumps_used));
        w.u8(self.lcancel.get(f.0 as usize).copied().unwrap_or(0));
        // Move-induced intangibility before the game's.
        w.u8(if fp.hit_status != 0 {
            fp.hit_status
        } else {
            fp.timed_hit_status
        } as u8);
        w.f32(fp.self_vel.x);
        w.f32(fp.self_vel.y);
        w.f32(fp.kb_vel.x);
        w.f32(fp.kb_vel.y);
        w.f32(fp.gr_vel);
        w.f32(fp.dmg.hitlag_frames);
        w.u32(fp.anim_id as u32);
        w.u16(fp.dmg.instance_hit_by);
        w.u16(fp.attack_record.record_id);
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
            w.f32(it.vel.x);
            w.f32(it.vel.y);
            w.f32(it.pos.x);
            w.f32(it.pos.y);
            w.u16(it.damage_taken as u16);
            w.f32(it.life_timer);
            w.u32(it.spawn_num as u32);
            w.bytes(&item_misc(&it.vars));
            // The owner's player slot: byte 0xC of the owner GObj's user data.
            let owner = it.owner.and_then(|o| match g.world.gobj(o).user_data {
                Some(UserData::Fighter(f)) => Some(g.fighters[f.0].player_idx),
                Some(UserData::Item(i)) => Some((g.items[i.0].spawn_kind >> 24) as u8),
                _ => None,
            });
            w.u8(owner.unwrap_or(0xFF));
            w.u16(it.attack_record_short);
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
        let d = &g.display_phase;
        self.display_phase = d.first.map(|first| (d.differing, first));
        // The results as they stand, on a copy (the codes fill the results' global, which the
        // match doesn't read, from the controller's): each player's placement.
        let mut copy = g.clone();
        let r = &mut copy.vs.state.match_end;
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
        for p in &copy.vs.state.match_end.player_standings[..4] {
            w.u8(if p.pkind == PlayerKind::None {
                0xFF
            } else {
                p.is_small_loser
            });
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
    let first = g.scene.ticks == 0;
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
    if g.runs_playback_codes() {
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
    let playback = g.runs_playback_codes();
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

/// The 16 bytes the codes copy from name tag `slot`'s name (`GetNameText`), as halfwords: the
/// name's eight bytes, the four after them in its `NameTagData` (its terminator, rumble and
/// two more) and the next name tag's first four (its KO counts' first two). A slot with no
/// name gives a null name, which Slippi Dolphin reads as zeros.
#[doc(alias = "GetNameText")]
fn name_tag_text(g: &Game, slot: u8) -> [u16; 8] {
    use super::gm::main_lib::NAMETAG_BANK_SIZE;
    let banks = &g.game_data.card.nametag_banks;
    let tag = |s: usize| {
        banks
            .get(s / NAMETAG_BANK_SIZE)
            .map(|b| &b.inner[s % NAMETAG_BANK_SIZE])
    };
    let s = usize::from(slot);
    let Some(t) = tag(s).filter(|t| t.namedata[0] != 0) else {
        return [0; 8];
    };
    let mut b = [0u8; 16];
    for (o, &c) in b.iter_mut().zip(&t.namedata) {
        *o = c as u8;
    }
    b[8..12].copy_from_slice(&[
        t.name_end as u8,
        t.rumble_enabled,
        t.handicap as u8,
        t.pad_1a3,
    ]);
    if let Some(next) = tag(s + 1) {
        b[12..14].copy_from_slice(&next.vs_kos[0].to_be_bytes());
        b[14..16].copy_from_slice(&next.vs_kos[1].to_be_bytes());
    }
    core::array::from_fn(|i| u16::from_be_bytes([b[2 * i], b[2 * i + 1]]))
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
    let b = &fp.state_flags;
    [
        byte([
            fp.allow_interrupt,
            fp.jab_continue,
            fp.jab_rapid,
            fp.reflecting,
            fp.reflect_kept,
            fp.reflect_behavior,
            fp.absorbing,
            fp.absorb_any,
        ]),
        byte([
            fp.pending_hitlag,
            fp.thaw_size,
            fp.allow_sdi,
            fp.in_hitstun_sdi,
            fp.fall_fast,
            fp.hurtboxes_set,
            fp.hurtboxes_custom,
            fp.shield_was_hit,
        ]),
        byte([
            b.shield_bubble,
            b.shield_deflects,
            b.shield_no_entry,
            b.bubble_faces,
            b.bubble_reflectors_only,
            b.holds_grab,
            b.item_grab_set,
            b.grounded_catch,
        ]),
        byte([
            fp.held_hit_light,
            fp.powershield_late,
            fp.powershield,
            fp.powershield_early,
            fp.armor,
            fp.hit_this_frame,
            fp.in_hitstun,
            // The three-bit field's top bit; the rest is 0x221D's.
            fp.leg_reach & 4 != 0,
        ]),
        byte([
            fp.off_screen,
            fp.dead,
            fp.cloaked,
            fp.is_sleeping,
            fp.is_sub_fighter,
            fp.last_hit_by_sub,
            fp.shield_hit_by_sub,
            fp.unused_221f_b7,
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
    use super::it::Member;
    let low = |w: i32| w as u8;
    let b = &v.left.bytes;
    match &v.member {
        Member::Bytes => [b[3], b[7], b[0x17], b[0x1B]],
        Member::SamusMissile(m) => [low(*m.is_smash_missile.peek()), 0, 0, 0],
        Member::PeachTurnip(t) => [0, low(*t.face.peek()), 0, 0],
        Member::SamusChargeshot(c) => [0, 0, low(*c.fired.peek()), low(*c.charge.peek())],
        Member::MewtwoShadowball(s) => [0, 0, low(*s.fired.peek()), low(*s.charge.peek())],
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
    o.bits(0, 3, 3, u32::from(r.damage_display));
    let flags = [
        r.timer_enabled,
        r.timer_counts_up,
        r.exact_start_frame,
        r.counts_coins,
        r.long_countdown,
        r.alt_go,
        r.fixed_layout,
        r.keeps_rumble,
        r.timer_shows_hours,
        r.friendly_fire,
        r.is_stock,
        r.arrows_flag,
        r.unread_2_2,
        r.single_button,
        r.disable_pausing,
        r.damage_display_kos,
        r.damage_displays_on,
        r.damage_display_fades,
        r.shows_score,
        r.pause_shows_stick,
        r.pause_z_retries,
        r.pause_shows_z,
        r.pause_lras_quits,
        r.pause_shows_lras,
        r.pause_hides_controls,
        r.pause_shows_timers,
        r.pause_camera,
        r.is_vs,
        r.single_player,
        r.game_over_scene,
        r.clear_scores,
        r.allows_continue,
        r.time_bonus,
        r.damage_bonus,
        r.endless,
        r.goes_on,
        r.no_fighter_hits,
        r.stadium_zoom,
        r.unused_5_4,
        r.unused_5_5,
        r.unused_5_6,
        r.unused_5_7,
    ];
    for (i, &f) in flags.iter().enumerate() {
        o.bits(0, 6 + i as u32, 1, u32::from(f));
    }
    o.u8(0x6, r.sudden_death);
    o.u8(0x7, r.end_kind);
    o.u8(0x8, r.is_teams);
    o.u8(0x9, r.run_kind);
    o.u8(0xA, r.stage_layout);
    o.u8(0xB, r.item_freq as u8);
    o.u8(0xC, r.sd_penalty as u8);
    o.u8(0xD, r.end_wait);
    o.be(0xE, &r.stkind.to_be_bytes());
    o.be(0x10, &r.time_limit.to_be_bytes());
    o.u8(0x14, r.start_frame);
    o.be(0x18, &r.score.to_be_bytes());
    o.be(0x1C, &r.pad_1c[0].to_be_bytes());
    o.be(0x20, &r.item_switch.to_be_bytes());
    o.be(0x28, &r.forced_pokemon.to_be_bytes());
    o.be(0x2C, &r.quake_scale.to_bits().to_be_bytes());
    o.be(0x30, &r.damage_ratio.to_bits().to_be_bytes());
    o.be(0x34, &r.game_speed.to_bits().to_be_bytes());
    let code = |c: Option<super::handles::CodeRef>| c.map_or(0, |c| c.0).to_be_bytes();
    o.be(0x38, &code(r.on_unpause_override));
    o.be(0x3C, &code(r.on_pause_override));
    o.be(0x40, &code(r.check_for_pauser_override));
    o.be(0x44, &code(r.on_match_start));
    o.be(0x48, &code(r.on_frame_start));
    o.be(0x4C, &code(r.on_frame_end));
    o.be(0x50, &code(r.on_match_end));
    o.be(0x54, &r.opponent_list.0.to_be_bytes());
    o.be(0x58, &r.challenger_record.0.to_be_bytes());
    o.be(0x5C, &r.pad_5c);
}

fn export_player(o: &mut Bits<0x138>, at: usize, p: &PlayerInitData) {
    o.u8(at, p.ckind as u8);
    o.u8(at + 0x1, p.slot_type.raw());
    o.u8(at + 0x2, p.stocks as u8);
    o.u8(at + 0x3, p.color);
    o.u8(at + 0x4, p.slot);
    o.u8(at + 0x5, p.spawn_pos as u8);
    o.u8(at + 0x6, p.spawn_dir as u8);
    o.u8(at + 0x7, p.sub_color);
    o.u8(at + 0x8, p.handicap as u8);
    o.u8(at + 0x9, p.team);
    o.u8(at + 0xA, p.nametag);
    o.u8(at + 0xB, p.sfx_size);
    let flags = [
        p.rumble_enabled,
        p.enters,
        p.vs_metal,
        p.unread_c_b3,
        p.vs_invisible,
        p.attrs_scaled,
        p.voice_muted,
        p.stamina,
        p.hp_explodes,
        p.shows_hearts,
        p.no_offscreen_damage,
        p.no_magnifier,
        p.dies_up,
        p.unused_d_b5,
        p.unused_d_b6,
        p.unused_d_b7,
    ];
    for (i, &f) in flags.iter().enumerate() {
        o.bits(at, 96 + i as u32, 1, u32::from(f));
    }
    o.u8(at + 0xE, p.cpu_kind.raw());
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
    fn the_playback_note_counts_times() {
        let mut r = Recorder::new(Setup::slippi());
        assert_eq!(r.playback_note(), None);
        r.display_phase = Some((1, 640));
        assert_eq!(
            r.playback_note().as_deref(),
            Some(
                "may not play back exactly: 1 time, first at tick 640, the match read something Slippi's playback can show differently (where the camera was, or a random number after Brinstar Depths' haze)"
            )
        );
        r.display_phase = Some((3, 12));
        assert_eq!(
            r.playback_note().as_deref(),
            Some(
                "may not play back exactly: 3 times, first at tick 12, the match read something Slippi's playback can show differently (where the camera was, or a random number after Brinstar Depths' haze)"
            )
        );
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

    /// The unlock and message writes: `04` lines at these sites.
    const WRITES: [u32; 16] = [
        0x8015_EE98,
        0x8015_EDDC,
        0x8016_4B14,
        0x8016_48F4,
        0x8015_EE4C,
        0x8015_EE14,
        0x8015_D968,
        0x8015_D9D8,
        0x8017_229C,
        0x8017_37B0,
        0x8016_4658,
        0x8016_44E8,
        0x8030_490C,
        0x8030_44F0,
        0x8015_D94C,
        0x8015_D984,
    ];

    /// The ruleset's list is 43 codes: the 16 writes, and 27 codes that make Slippi's online
    /// rules, 3.19's with UCF 0.84. `game_start` puts UCF's pad buffer where the code keeps it.
    #[test]
    fn the_ruleset_list_is_the_writes_and_3_19s_rules() {
        let list = ruleset_codes();
        assert_eq!(list.len(), 5392);
        let codes = crate::game::slippi::parse_codes(&list);
        assert_eq!(codes.len(), 43);
        let (writes, rules): (Vec<_>, Vec<_>) = codes
            .into_iter()
            .partition(|c| c.kind == 0x04 && WRITES.contains(&c.addr));
        assert_eq!(writes.len(), 16);
        assert_eq!(rules.len(), 27);
        assert_eq!(
            crate::game::state::Rules::from_codes(&rules),
            Ok(crate::game::state::Rules::slippi_online())
        );
        let mut pads = super::super::ucf::Pads::default();
        pads.0[2].history[1] = [-96, 17];
        pads.0[3].index = 3;
        pads.0[3].flicked = 2;
        let stood = codes_as_they_stand(&list, &pads);
        let read = super::super::ucf::Pads::from_codes(&crate::game::slippi::parse_codes(&stood));
        assert_eq!(read, Some(pads));
        let differ = (0..list.len()).filter(|&i| list[i] != stood[i]).count();
        assert_eq!(differ, 4, "only the pads' bytes change");
    }

    /// A human player's name tag as the codes copy it from the game's save data, in a match the
    /// menus started: 16 bytes from the name on.
    #[test]
    fn a_name_tag_reads_from_the_game() {
        let mut g = Game::default();
        let t = g.game_data.name_tag(20);
        t.namedata = [0x82, 0x60, 0x82, 0x61, 0, 0, 0, 0].map(|b| b as i8);
        t.rumble_enabled = 1;
        g.game_data.name_tag(21).vs_kos[0] = 0x0102;
        assert_eq!(
            name_tag_text(&g, 20),
            [0x8260, 0x8261, 0, 0, 0x0001, 0, 0x0102, 0]
        );
        assert_eq!(name_tag_text(&g, 21), [0; 8], "a slot with no name");
    }

    /// One `.slp` file's raw events: the payload sizes' event, the game start's payload and
    /// the code list.
    fn raw_parts(file: &[u8]) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        let at = file
            .windows(10)
            .position(|w| w == b"U\x03raw[$U#l")
            .expect("a raw element");
        let len = u32::from_be_bytes(file[at + 10..at + 14].try_into().unwrap()) as usize;
        let raw = &file[at + 14..at + 14 + len];
        let n = usize::from(raw[1]);
        let mut sizes = [0usize; 256];
        for e in raw[2..1 + n].chunks(3) {
            sizes[usize::from(e[0])] = usize::from(u16::from_be_bytes([e[1], e[2]]));
        }
        let (mut start, mut codes) = (Vec::new(), Vec::new());
        let mut i = 1 + n;
        while i < raw.len() && raw[i] != command::PRE_FRAME {
            let c = raw[i];
            let p = &raw[i + 1..i + 1 + sizes[usize::from(c)]];
            match c {
                command::GAME_START => start = p.to_vec(),
                command::SPLIT_MESSAGE if p[514] == command::GECKO_LIST => {
                    let k = usize::from(u16::from_be_bytes([p[512], p[513]]));
                    codes.extend_from_slice(&p[..k]);
                }
                _ => {}
            }
            i += 1 + sizes[usize::from(c)];
        }
        (raw[..1 + n].to_vec(), start, codes)
    }

    /// Each code of a list, with its bytes.
    fn split_codes(list: &[u8]) -> Vec<(u8, u32, &[u8])> {
        let word = |at: usize| u32::from_be_bytes(list[at..at + 4].try_into().unwrap());
        let mut out = Vec::new();
        let mut at = 0;
        while at + 8 <= list.len() {
            let kind = list[at] & 0xFE;
            let len = match kind {
                0xC2 => 8 + 8 * word(at + 4) as usize,
                0x04 => 8,
                _ => panic!("a code of kind {kind:02X} in Slippi's list"),
            };
            out.push((
                kind,
                (word(at) & 0x01FF_FFFF) | 0x8000_0000,
                &list[at..at + len],
            ));
            at += len;
        }
        out
    }

    /// Sarah's four offline replays from Slippi Dolphin 3.19.1 (the VS mode's matches, one
    /// of them its Sudden Death), against the game start melee-hd's recorder writes for the
    /// same match under the Slippi ruleset, started from the menus, with the state those
    /// replays recorded (the start data, the seed, UCF's pad buffer, the scene): the game
    /// start is the same byte for byte, the payload sizes but for the code list's, and the
    /// code list is theirs without the 245 codes melee-hd doesn't run, but for the two UCF
    /// codes' scratch doubles (boot zeros in ours; one replay's down-smash fix holds its last
    /// conversion). Skips without `SLIPPI_REPLAYS`.
    #[test]
    fn offline_replays_game_starts() {
        let Some(dir) = std::env::var_os("SLIPPI_REPLAYS") else {
            melee_local_data::skip!("skipped: SLIPPI_REPLAYS isn't set");
            return;
        };
        let names = [
            "Game_20260524T173350",
            "Game_20260524T174154",
            "Game_20260524T174859",
            "Game_20260524T175101",
        ];
        let ours_list = ruleset_codes();
        let mut scratch_differs = Vec::new();
        for name in names {
            let path = std::path::Path::new(&dir)
                .join("2026-05")
                .join(format!("{name}.slp"));
            let Ok(file) = std::fs::read(&path) else {
                melee_local_data::skip!("skipped: {} isn't there", path.display());
                return;
            };
            let (sizes, start, list) = raw_parts(&file);
            let info = &start[4..4 + 0x138];
            let seed = u32::from_be_bytes(start[0x13C..0x140].try_into().unwrap());
            let (minor, major) = (start[0x1A2], start[0x1A3]);
            let mut g = Game {
                rules: crate::game::state::Rules::slippi_online(),
                ucf: super::super::ucf::Pads::from_codes(&crate::game::slippi::parse_codes(&list))
                    .expect("UCF's pad buffer"),
                ..Game::default()
            };
            g.objs.rng.seed = seed;
            g.game_data.set_saved_language(1);
            g.machine.routing.curr_mode = major;
            g.machine.routing.curr_state_id = minor;
            g.machine.phase = super::super::gm::machine::Phase::Scene {
                mode: major,
                scene_mode: minor,
                state: 0,
                overridden: false,
            };
            let d = StartMeleeData::import(
                &Mem {
                    bytes: info,
                    base: 0,
                },
                0,
                &mut NoPointers,
            );
            let mut r = Recorder::new(Setup::slippi());
            r.game_start(&g, &d);
            let mut file_ours = b"{U\x03raw[$U#l".to_vec();
            file_ours.extend_from_slice(&(r.raw().len() as u32).to_be_bytes());
            file_ours.extend_from_slice(r.raw());
            let (our_sizes, our_start, our_list) = raw_parts(&file_ours);
            assert_eq!(our_start, start, "{name}'s game start");
            let entries = |s: &[u8]| s[2..].chunks(3).map(<[u8]>::to_vec).collect::<Vec<_>>();
            let (a, b) = (entries(&our_sizes), entries(&sizes));
            assert_eq!(a.len(), b.len());
            for (x, y) in a.iter().zip(&b) {
                if x[0] == command::GECKO_LIST {
                    assert_eq!(u16::from_be_bytes([x[1], x[2]]), 5392);
                } else {
                    assert_eq!(x, y, "{name}'s payload sizes");
                }
            }
            // Theirs, without the codes melee-hd doesn't run.
            let ours = split_codes(&our_list);
            let theirs: Vec<_> = split_codes(&list)
                .into_iter()
                .filter(|&(k, a, _)| ours.iter().any(|&(k2, a2, _)| (k, a) == (k2, a2)))
                .collect();
            assert_eq!(ours.len(), 43);
            assert_eq!(theirs.len(), 43);
            for (&(_, a, x), &(_, _, y)) in ours.iter().zip(&theirs) {
                if x == y {
                    continue;
                }
                // The scratch double: the code's body words 4 and 5.
                assert!(
                    a == 0x800D_65EC || a == 0x8009_98A4,
                    "{name}: the code at {a:#010x} differs"
                );
                let scratch = 8 + 16..8 + 24;
                assert_eq!(x[..scratch.start], y[..scratch.start]);
                assert_eq!(x[scratch.end..], y[scratch.end..]);
                assert_eq!(x[scratch], [0; 8]);
                scratch_differs.push((name, a));
            }
            assert_eq!(our_list.len(), ours_list.len());
        }
        assert_eq!(
            scratch_differs,
            [("Game_20260524T175101", 0x800D_65EC)],
            "the replays' scratch doubles"
        );
    }
}
