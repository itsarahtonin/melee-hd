//! Gecko codes players add to their own Slippi setups, which their replays record next to
//! Slippi's ruleset (`game::slippi`). Slippi's playback applies a replay's whole code list
//! (but the denylisted injections) when the match's scene starts (`fn_8016E730+0x18`,
//! `Playback/Core/RestoreGameInfo.asm`), taking only `04`, `06` and `C2` codes, so these run in
//! a replay from the match's entry on. [`recognize`] knows each by where it injects and the
//! instructions it runs, as the ruleset's codes are known; a code it doesn't know still
//! refuses the replay.
//!
//! The codes that change something the match keeps are ported where they inject, keyed off
//! [`PlayerCodes`] in [`Rules`](super::state::Rules):
//!
//! - **X/Z swap** (`C2 8034E2AC`, in `SPEC2_MakeStatus`): the decoder swaps the X and Z
//!   buttons (and keeps the response's two error bits). [`PadStatus::swap_xz`].
//! - **Invisible techs** (`C2 80080A30`, in `ftDrawCommon_800805C8`): a fighter teching or
//!   tech-rolling (`Passive`, `PassiveStandF`, `PassiveStandB`) isn't drawn while its
//!   animation frame's top half is from `0x40A0` (5.0) up to `0x41A0` (20.0).
//! - **Fast-fall spark** (`C2 8007D5A0`, in `ftCommon_CheckFallFast`): starting a fast fall
//!   spawns effect 1011 on the fighter's part 3, 8 units behind it (`ftCo_8009F834`, whose
//!   spread draws three random numbers). The code takes the fighter from `r28` and the facing
//!   from `r31`'s word at `0x2C`, which its callers leave: the GObj and the fighter, but in
//!   `ftCo_FallSpecial_Phys`, where `r31` is the fighter's attributes and the word is
//!   `run_animation_scaling`.
//! - **Quiet announcer** (`04 800249A4`, in `fn_80024654`): the announcer's volume setting is
//!   read as 1 instead of `lbl_804D3884`.
//! - **Camera** (`C2 801C0A50`, at the end of `Ground_801C0800`): every stage's camera tilt,
//!   pan, `x20`, `x24`, tracking ratio, fixed zoom, tracking smoothness, zoom rate and depth
//!   become 30, -10, 0.05, 0.05, 1.5, 1.5, 1.8, 83 and 1000.
//! - **Match rules at the scene's entry** (`C2 8016E750`, after playback's code): in the VS
//!   mode's match (scene 2.2) or a replay's (14.1), the match's `StartMeleeData` is changed
//!   before the match reads it ([`StartRules`]); some variants test the scene 14.1 only past
//!   Slippi's online mode byte (`r13-0x5060`, which only Slippi's online scene sets: 0 in a
//!   replay and the VS mode), so they skip a replay.
//! - **Stocks at the scene's entry's end** (`C2 8016E91C`, the end of `fn_8016E730`): likewise,
//!   every player slot's stocks set ([`EndRules`]).
//! - **SDI colors** (`C2 8008E588` and `C2 8008E734`, in `ftCo_Damage_OnEveryHitlag` and
//!   `ftCo_Damage_OnExitHitlag`): each smash DI input sets the second color overlay (`x488`)
//!   yellow, then blue, then magenta, counting in the overlay's padding byte (`x488 + 0x7D`,
//!   [`State::sdi_count`]), which leaving hitlag clears.
//! - **Dash grab color** (`C2 8006A138`, in `Fighter_ChangeMotionState`): entering
//!   `CatchDash` or `CatchDashPull` sets the second color overlay red. In Slippi's online scene
//!   (2.8) only for the local player; a replay runs it for every fighter.
//! - **Fountain of Dreams' reflection** (`04 801CCE4C`, `801CCE50` and the reflection camera's
//!   descriptor words `803E0F3C` to `803E0F48`): the reflection renders 640x480 instead of
//!   80x60, so its texture buffer from the scene's heap is that much bigger. And
//!   (`04 801CCECC`, `801CD078`) the reflection's render keeps the fighters' own render
//!   callbacks instead of the magnifier's.
//! - **Stadium monitor weights** (`04 801D2AC0`, `801D2AC8`, `801D2B98`, in
//!   `grStadium_801D2A60`): Pokémon Stadium's big screen picks its next mode with the weights
//!   of modes 8 and 7 (the fighters' close-ups) zeroed and no check against the last two
//!   modes, so one random number per pick.
//!
//! Codes that change nothing in a replay played forward are accepted without effect:
//!
//! - Codes in the menus' and the title's scenes, which a replay never runs after its codes
//!   apply: the character select screen's (`04 80263384`, `C2 80262F48`, `C2 80263064`,
//!   `C2 802668BC`, `04 804DC578`), the title's (`04 801A1CC4`, `04 801B08B4`), the main menu's
//!   and the sound test's (`C2 8022E934`, `C2 8024B4D8`), the debug menus' (`C2 80227784`,
//!   `04 8022877C` in `db_RunEveryFrame`'s paths, which return at once below the debug ROM's
//!   level) and the debug text menus' (`04 803030DC`, `80303158`, `803031D4`, `80303244`,
//!   `80304254`, `8030425C`, `C2 803039A4`, in `textlib_1.c`).
//! - An empty `00 80000000 00000000` line: playback applies no `00` code.
//!
//! Marth's sword trail colors by costume (`C2 80136510`, in `ftMs_Init_LoadSpecialAttrs`)
//! reads the costume from a fixed distance before Marth's attribute block, wherever the heap
//! put it; it only runs for Marth, so a match without him accepts it and one with him is
//! refused (`vs_match::Settings::from_replay`).

use super::slippi::{Code, CodeError};
use super::state::{Game, Running};
use super::structs::StartMeleeData;
use crate::hsd::gobj::GObjId;

/// The players' codes a match runs (`game::player_codes`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct PlayerCodes {
    pub swap_xz: bool,
    pub invisible_techs: bool,
    pub fall_fast_spark: bool,
    pub quiet_announcer: bool,
    pub camera: bool,
    pub start_rules: Option<StartRules>,
    pub end_rules: Option<EndRules>,
    pub sdi_colors: bool,
    pub sdi_colors_reset: bool,
    pub dash_grab_color: bool,
    pub izumi_reflection_size: bool,
    pub izumi_reflection_callbacks: bool,
    /// Marth's sword trail colors (`C2 80136510`): accepted only without Marth.
    pub marth_trail: bool,
    /// Pokémon Stadium's monitor weights.
    pub stadium_weights: bool,
}

/// What `C2 8016E750` writes into the match's `StartMeleeData`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub enum StartRules {
    /// Item frequency 3, a 120-second time limit and items `xFDFFFBF8FF` in the item
    /// switch's low five bytes; in the VS mode's match or a replay's.
    Items,
    /// A 180-second time limit, in the VS mode's match only (a replay's skips it).
    Time,
}

/// What `C2 8016E91C` writes at the end of the scene's entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub enum EndRules {
    /// Two stocks each, in the VS mode's match only.
    TwoStocks,
    /// One stock each and `0x2C` into the match's time limit's top byte, in the VS mode's
    /// match only.
    OneStock,
}

/// State the codes keep outside the game's structs.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct State {
    /// The SDI colors' count, in each fighter slot's second color overlay's padding byte
    /// (`x488 + 0x7D`), by pool slot: a fighter block comes back with what its last
    /// occupant left; a new one holds 0, as the match's fresh heap does.
    pub sdi_count: Vec<u8>,
}

impl State {
    /// Fighter pool slot `slot`'s count.
    pub fn sdi_count(&mut self, slot: usize) -> &mut u8 {
        if self.sdi_count.len() <= slot {
            self.sdi_count.resize(slot + 1, 0);
        }
        &mut self.sdi_count[slot]
    }
}

/// What a recognized code does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Effect {
    /// Nothing a replay played forward reaches.
    Inert,
    SwapXz,
    InvisibleTechs,
    FallFastSpark,
    QuietAnnouncer,
    Camera,
    Start(StartRules),
    End(EndRules),
    SdiColors,
    SdiColorsReset,
    DashGrabColor,
    IzumiSize,
    IzumiCallbacks,
    MarthTrail,
    StadiumWeights,
}

impl Effect {
    /// Codes that only work together, by the name a partial list's error gives.
    fn group(self) -> Option<&'static str> {
        match self {
            Effect::IzumiSize => Some("Fountain of Dreams' reflection size"),
            Effect::IzumiCallbacks => Some("Fountain of Dreams' reflection callbacks"),
            Effect::StadiumWeights => Some("Pokémon Stadium's monitor weights"),
            _ => None,
        }
    }
}

/// One known code: its type, where it injects, its first line's second word, the FNV-1a hash
/// of its words with a `C2` code's final branch zeroed (`game::slippi`'s), and its effect.
struct Known {
    kind: u8,
    addr: u32,
    first: u32,
    hash: u64,
    effect: Effect,
}

/// The hash of no words: every `04` code's.
const EMPTY: u64 = 0xCBF2_9CE4_8422_2325;

const fn write(addr: u32, value: u32, effect: Effect) -> Known {
    Known {
        kind: 0x04,
        addr,
        first: value,
        hash: EMPTY,
        effect,
    }
}

const fn inject(addr: u32, lines: u32, hash: u64, effect: Effect) -> Known {
    Known {
        kind: 0xC2,
        addr,
        first: lines,
        hash,
        effect,
    }
}

const KNOWN: &[Known] = &[
    inject(0x8034_E2AC, 2, 0x28BE_09A8_2E44_42E0, Effect::SwapXz),
    inject(
        0x8008_0A30,
        6,
        0xDF54_E6D1_1F89_2ED7,
        Effect::InvisibleTechs,
    ),
    inject(
        0x8007_D5A0,
        0x0E,
        0x8A34_C1B8_4C6D_D537,
        Effect::FallFastSpark,
    ),
    write(0x8002_49A4, 0x3860_0001, Effect::QuietAnnouncer),
    inject(0x801C_0A50, 0x0B, 0xC287_8B70_0FCA_C941, Effect::Camera),
    inject(
        0x8016_E750,
        0x11,
        0xFBDC_E192_9FD7_F132,
        Effect::Start(StartRules::Items),
    ),
    inject(
        0x8016_E750,
        0x0C,
        0x4146_AB83_9482_B491,
        Effect::Start(StartRules::Time),
    ),
    inject(
        0x8016_E91C,
        0x11,
        0x7ED0_124A_B50F_6C9C,
        Effect::End(EndRules::TwoStocks),
    ),
    inject(
        0x8016_E91C,
        0x13,
        0x3F5A_86FD_D105_D381,
        Effect::End(EndRules::OneStock),
    ),
    inject(0x8008_E588, 0x18, 0xEE1B_E083_8891_EE3B, Effect::SdiColors),
    inject(
        0x8008_E734,
        2,
        0x3373_E4D2_E894_1BD7,
        Effect::SdiColorsReset,
    ),
    inject(
        0x8006_A138,
        0x0F,
        0xC384_FCA5_73B6_A8F3,
        Effect::DashGrabColor,
    ),
    write(0x801C_CE4C, 0x3880_0280, Effect::IzumiSize),
    write(0x801C_CE50, 0x38A0_01E0, Effect::IzumiSize),
    write(0x803E_0F3C, 0x0000_0280, Effect::IzumiSize),
    write(0x803E_0F40, 0x0000_01E0, Effect::IzumiSize),
    write(0x803E_0F44, 0x0000_0280, Effect::IzumiSize),
    write(0x803E_0F48, 0x0000_01E0, Effect::IzumiSize),
    write(0x801C_CECC, 0x6000_0000, Effect::IzumiCallbacks),
    write(0x801C_D078, 0x6000_0000, Effect::IzumiCallbacks),
    inject(0x8013_6510, 0x0F, 0xC85E_04C6_430A_ABD1, Effect::MarthTrail),
    write(0x801D_2AC0, 0x38A0_0000, Effect::StadiumWeights),
    write(0x801D_2AC8, 0x3880_0000, Effect::StadiumWeights),
    write(0x801D_2B98, 0x4800_0018, Effect::StadiumWeights),
    // The character select screen's.
    write(0x8026_3384, 0x4800_0010, Effect::Inert),
    inject(0x8026_2F48, 0x2E, 0x3056_1C9C_BC89_6830, Effect::Inert),
    inject(0x8026_3064, 0x0C, 0x88C1_E74D_2B6F_FF5A, Effect::Inert),
    inject(0x8026_68BC, 0x0B, 0xF32C_EA9E_8FD5_AC83, Effect::Inert),
    write(0x804D_C578, 0, Effect::Inert),
    // The title's, the main menu's and the sound test's.
    write(0x801A_1CC4, 0x6000_0000, Effect::Inert),
    write(0x801B_08B4, 0x6000_0000, Effect::Inert),
    inject(0x8022_E934, 8, 0x0D1D_3E2E_6A58_2F01, Effect::Inert),
    inject(0x8024_B4D8, 7, 0x5651_67CA_BCA5_1A28, Effect::Inert),
    // The debug menus' and the debug text menus'.
    inject(0x8022_7784, 0x0D, 0xA6F5_13A5_4BDC_AB34, Effect::Inert),
    write(0x8022_877C, 0x2C00_0006, Effect::Inert),
    write(0x8030_30DC, 0x3C00_0000, Effect::Inert),
    write(0x8030_3158, 0x3C00_0000, Effect::Inert),
    write(0x8030_31D4, 0x3C00_0000, Effect::Inert),
    write(0x8030_3244, 0x3C00_0000, Effect::Inert),
    write(0x8030_4254, 0x3860_0000, Effect::Inert),
    write(0x8030_425C, 0x38A0_0006, Effect::Inert),
    inject(0x8030_39A4, 0x16, 0xCB7A_2F06_91D9_630D, Effect::Inert),
];

/// Whether `c` is an empty `00 80000000 00000000` line, which playback doesn't apply.
fn empty_line(c: &Code) -> bool {
    c.kind == 0x00 && c.addr == 0x8000_0000 && c.first == 0
}

/// The player code `c` is, if melee-hd knows it.
fn recognize(c: &Code) -> Option<Effect> {
    if empty_line(c) {
        return Some(Effect::Inert);
    }
    let hash = super::slippi::code_hash(c);
    KNOWN
        .iter()
        .find(|k| k.kind == c.kind && k.addr == c.addr && k.first == c.first && k.hash == hash)
        .map(|k| k.effect)
}

/// The known player codes a running game has applied, read from its memory (`read` gives the
/// word at an address), as [`Rules::from_memory`](super::state::Rules::from_memory) reads the
/// ruleset's: an `04` code whose word is at its address, a `C2` code whose instructions are
/// where the branch at its address leads.
pub(crate) fn in_memory(read: &dyn Fn(u32) -> u32) -> Vec<Code> {
    let mut out: Vec<Code> = Vec::new();
    for k in KNOWN {
        let at = read(k.addr);
        let code = match k.kind {
            0x04 if at == k.first => Code {
                kind: k.kind,
                addr: k.addr,
                first: k.first,
                words: Vec::new(),
            },
            0xC2 => match super::slippi::placed(k.addr, at) {
                Some(body) => Code {
                    kind: k.kind,
                    addr: k.addr,
                    first: k.first,
                    words: (0..2 * k.first).map(|i| read(body + 4 * i)).collect(),
                },
                None => continue,
            },
            _ => continue,
        };
        if recognize(&code).is_some() && !out.iter().any(|c| c.addr == k.addr && c.kind == k.kind) {
            out.push(code);
        }
    }
    out
}

/// Whether some known player code injects where `c` does.
pub(crate) fn known_site(c: &Code) -> bool {
    KNOWN.iter().any(|k| k.kind == c.kind && k.addr == c.addr)
}

/// Collects the player codes of a list as their effects, or `None` for a code that isn't one.
pub(crate) struct Collect {
    codes: PlayerCodes,
    seen: Vec<(u8, u32, Effect)>,
}

impl Collect {
    pub(crate) fn new() -> Collect {
        Collect {
            codes: PlayerCodes::default(),
            seen: Vec::new(),
        }
    }

    /// Takes `c` if it's a known player code.
    pub(crate) fn take(&mut self, c: &Code) -> bool {
        let Some(e) = recognize(c) else {
            return false;
        };
        let p = &mut self.codes;
        match e {
            Effect::Inert => {}
            Effect::SwapXz => p.swap_xz = true,
            Effect::InvisibleTechs => p.invisible_techs = true,
            Effect::FallFastSpark => p.fall_fast_spark = true,
            Effect::QuietAnnouncer => p.quiet_announcer = true,
            Effect::Camera => p.camera = true,
            Effect::Start(s) => p.start_rules = Some(s),
            Effect::End(s) => p.end_rules = Some(s),
            Effect::SdiColors => p.sdi_colors = true,
            Effect::SdiColorsReset => p.sdi_colors_reset = true,
            Effect::DashGrabColor => p.dash_grab_color = true,
            Effect::IzumiSize => p.izumi_reflection_size = true,
            Effect::IzumiCallbacks => p.izumi_reflection_callbacks = true,
            Effect::MarthTrail => p.marth_trail = true,
            Effect::StadiumWeights => p.stadium_weights = true,
        }
        self.seen.push((c.kind, c.addr, e));
        true
    }

    /// The codes, once every group present is whole.
    pub(crate) fn finish(self) -> Result<PlayerCodes, CodeError> {
        for k in KNOWN {
            let Some(name) = k.effect.group() else {
                continue;
            };
            let present = self.seen.iter().any(|&(_, _, e)| e == k.effect);
            let this = self
                .seen
                .iter()
                .any(|&(kind, addr, _)| kind == k.kind && addr == k.addr);
            if present && !this {
                return Err(CodeError::Partial { rule: name });
            }
        }
        Ok(self.codes)
    }
}

impl crate::input::pad::PadStatus {
    /// `SPEC2_MakeStatus` with the X/Z swap (`C2 8034E2AC`), which replaces its first
    /// instruction: the buttons are the response's whole top half (its two error bits kept)
    /// with X and Z swapped.
    pub fn decode_swapping_xz(&mut self, data: [u32; 2], origin: &crate::input::pad::PadStatus) {
        let err = self.err;
        self.decode(data, origin);
        self.button = (data[0] >> 16) as u16;
        self.err = crate::input::pad::ERR_NONE;
        self.swap_xz();
        self.err = err;
    }

    /// The X/Z swap's decoding (`C2 8034E2AC`) of a decoded status: the X and Z buttons trade
    /// places. A status the decoder never made (an error's) is left alone.
    pub fn swap_xz(&mut self) {
        use crate::input::pad::{BUTTON_X, ERR_NONE, TRIGGER_Z};
        if self.err != ERR_NONE {
            return;
        }
        let b = self.button;
        let mut out = b & !(BUTTON_X | TRIGGER_Z);
        if b & TRIGGER_Z != 0 {
            out |= BUTTON_X;
        }
        if b & BUTTON_X != 0 {
            out |= TRIGGER_Z;
        }
        self.button = out;
    }
}

/// The scene the codes test, `(minor << 8) | major` from `state_machine`: the VS mode's match
/// (2.2), a replay's (the debug VS mode's 14.1), or neither. A match melee-sim runs is its
/// mode's match scene, so the mode tells them apart. Slippi's online scene (8.2), whose test
/// reads its online mode byte, is never melee-sim's.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Scene {
    Vs,
    Replay,
    Other,
}

fn scene(g: &Game) -> Scene {
    match g.game_mode {
        super::gm::GM_VS => Scene::Vs,
        super::gm::GM_DEBUG_VS => Scene::Replay,
        _ => Scene::Other,
    }
}

impl Running<'_> {
    /// `C2 8016E750`: the match's `StartMeleeData` as the code leaves it.
    pub fn player_start_rules(&self, d: &mut StartMeleeData) {
        let Some(rules) = self.game.rules.player.start_rules else {
            return;
        };
        let at = scene(self.game);
        let r = &mut d.rules;
        match rules {
            StartRules::Items if at != Scene::Other => {
                r.item_freq = 3;
                r.time_limit = 120;
                r.x20 = r.x20 & 0xFFFF_FF00_0000_0000 | 0x0000_00FD_FFFB_F8FF;
            }
            StartRules::Time if at == Scene::Vs => r.time_limit = 180,
            _ => {}
        }
    }

    /// `C2 8016E91C`, at the end of the scene's entry: every slot's stocks.
    pub fn player_end_rules(&mut self) {
        let Some(rules) = self.game.rules.player.end_rules else {
            return;
        };
        let at = scene(self.game);
        let stocks = match rules {
            EndRules::TwoStocks if at == Scene::Vs => 2,
            EndRules::OneStock if at == Scene::Vs => 1,
            _ => return,
        };
        for slot in 0..4 {
            self.game.set_player_stocks(slot, stocks);
        }
        if rules == EndRules::OneStock {
            // `stb` of 300 into `controller.start.time_limit`'s first byte.
            let t = &mut self.game.vs.start.time_limit;
            *t = *t & 0x00FF_FFFF | 0x2C00_0000;
        }
    }

    /// `C2 8007D5A0`, after a fast fall starts: effect 1011 on part 3, 8 units behind the
    /// facing `word` (the caller's `r31 + 0x2C`) says.
    pub fn fall_fast_spark(&mut self, gobj: GObjId, word: f32) {
        let x = if (word.to_bits() as i32) > 0 {
            -8.0
        } else {
            8.0
        };
        let offset = crate::vec::Vec3::new(x, 0.0, 0.0);
        let range = crate::vec::Vec3::new(0.0, 0.0, 0.0);
        self.script_effect(gobj, 1011, 3, false, false, offset, range);
    }

    /// `C2 8008E588`, at a smash DI input: the second overlay's color by the count.
    pub fn sdi_colors(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let n = self.game.player_code_state.sdi_count(f.0 as usize);
        *n = n.wrapping_add(1);
        let n = *n;
        let o = &mut self.game.fighters[f.0].x488;
        let (r, g, b) = match n {
            1 => (255.0, 255.0, 0.0),
            2 => (0.0, 0.0, 255.0),
            _ => (255.0, 0.0, 255.0),
        };
        o.x30_color_red = r;
        o.x34_color_green = g;
        o.x38_color_blue = b;
        o.x3C_color_alpha = 255.0;
        o.x40_colorblend_red = 0.0;
        o.x44_colorblend_green = 0.0;
        o.x48_colorblend_blue = 0.0;
        o.x4C_colorblend_alpha = 0.0;
        set_overlay_flags(o, 0x80);
    }

    /// `C2 8008E734`, leaving hitlag: the count cleared.
    pub fn sdi_colors_reset(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        *self.game.player_code_state.sdi_count(f.0 as usize) = 0;
    }

    /// `C2 8006A138`, as an action state starts: red for a dash grab. Its test of the local
    /// player runs only in Slippi's online scene, never melee-sim's.
    pub fn dash_grab_color(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        if fp.motion_id != 0xD6 && fp.motion_id != 0xD7 {
            return;
        }
        let o = &mut fp.x488;
        o.x30_color_red = 255.0;
        o.x3C_color_alpha = 255.0;
        o.x34_color_green = 0.0;
        o.x38_color_blue = 0.0;
        o.x40_colorblend_red = 0.0;
        o.x44_colorblend_green = 0.0;
        o.x48_colorblend_blue = 0.0;
        o.x4C_colorblend_alpha = 0.0;
        set_overlay_flags(o, 0x91);
    }
}

/// A color overlay's flag byte (`x7C`) as one store sets it.
fn set_overlay_flags(o: &mut super::structs::ColorOverlay, b: u8) {
    o.x7C_color_enable = b & 0x80 != 0;
    o.x7C_flag2 = b & 0x40 != 0;
    o.x7C_light_enable = b & 0x20 != 0;
    o.x7C_flag4 = b & 0x10 != 0;
    o.x7C_flag5 = b & 0x08 != 0;
    o.x7C_flag6 = b & 0x04 != 0;
    o.x7C_flag7 = b & 0x02 != 0;
    o.x7C_flag8 = b & 0x01 != 0;
}

/// The camera code's values for the stage's camera, from the tilt (`x18`) to the depth
/// (`x38`), as its table holds them: 30, -10, 0.05, 0.05, 1.5, 1.5, 1.8, 83 and 1000.
const CAMERA: [u32; 9] = [
    0x41F0_0000,
    0xC120_0000,
    0x3D4C_CCCD,
    0x3D4C_CCCD,
    0x3FC0_0000,
    0x3FC0_0000,
    0x3FE6_6666,
    0x42A6_0000,
    0x447A_0000,
];

impl Game {
    /// `C2 801C0A50`: the camera's values after the stage's setup.
    pub fn player_camera(&mut self) {
        if !self.rules.player.camera {
            return;
        }
        let c = &mut self.stage_info.cam_info;
        let [tilt, pan, x20, x24, ratio, zoom, smooth, rate, depth] = CAMERA.map(f32::from_bits);
        c.cam_vertical_tilt = tilt;
        c.cam_pan_degrees = pan;
        c.x20 = x20;
        c.x24 = x24;
        c.cam_track_ratio = ratio;
        c.cam_fixed_zoom = zoom;
        c.cam_track_smooth = smooth;
        c.cam_zoom_rate = rate;
        c.cam_max_depth = depth;
    }

    /// Whether the invisible techs code hides the fighter now.
    pub fn tech_hidden(&self, f: super::handles::FighterId) -> bool {
        let fp = &self.fighters[f.0];
        self.rules.player.invisible_techs
            && (199..=201).contains(&fp.motion_id)
            && (0x40A0..0x41A0).contains(&(fp.cur_anim_frame.to_bits() >> 16))
    }
}
