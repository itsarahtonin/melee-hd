//! The Slippi ruleset: the Gecko codes Slippi tags as affecting gameplay
//! (`[affects-gameplay]` in Slippi Dolphin's injection lists), which Slippi's online play runs
//! and its replays record. A replay's code list says which it ran; [`Rules::from_codes`]
//! recognizes them, by where each injects and the instructions it runs, and refuses one it
//! doesn't know rather than play a different game.
//!
//! The codes, by the rule each belongs to:
//!
//! - **UCF 0.84** (`External/UCF 0.84/UCF`): dashback, the shield drop and its extension, the
//!   down-smash out of crouch fix, shield SDI, SDI, tumble, and the pad buffer with 1.0
//!   cardinals the others read. Its pad buffer keeps state in the code's own words, which the
//!   code list carries as the game left them when the replay started.
//! - **Neutral spawns** (`External/NeutralSpawn`).
//! - **Wobbling prevention** (`External/PreventWobbling`).
//! - **Off-screen damage from the stage's limits** (`Online/Core/BrawlOffscreenDamage`), not the
//!   camera's view.
//! - **The online determinism fixes** (`Common` and `Online/Core`): fighter and stage object
//!   data zeroed when made, Final Destination's background and Pokémon Stadium's
//!   transformations kept off the random seed, Stadium's monitor independent of the camera,
//!   Whispy's blow direction, Nana's inputs and the star KO's physics.
//! - **The Ice Climbers' freeze glitch fix** (`External/FreezeGlitchFix`).

// NaN takes the original's branches, which negated comparisons spell out.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use super::state::Rules;

/// One code of the ruleset: its type, where it injects, its first line's second word (its
/// length for `C2`), and the FNV-1a hash of its words with its data and final branch zeroed.
struct Known {
    kind: u8,
    addr: u32,
    first: u32,
    hash: u64,
    rule: Rule,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Rule {
    Ucf,
    NeutralSpawn,
    PreventWobbling,
    OffscreenLimits,
    Determinism,
    FreezeGlitchFix,
}

const fn code(kind: u8, addr: u32, first: u32, hash: u64, rule: Rule) -> Known {
    Known {
        kind,
        addr,
        first,
        hash,
        rule,
    }
}

/// The codes, as Slippi 3.19's online code set has them.
const KNOWN: &[Known] = &[
    code(
        0xC2,
        0x8016_E510,
        0x98,
        0xE8ED_8114_E454_548D,
        Rule::NeutralSpawn,
    ),
    code(0xC2, 0x800D_65EC, 0x10, 0x071D_D73A_9A8C_21CF, Rule::Ucf),
    code(0xC2, 0x800C_9A44, 0x20, 0xF571_8894_9314_2D79, Rule::Ucf),
    code(0xC2, 0x8006_B460, 0x54, 0xDD9B_FDB3_0B6D_6308, Rule::Ucf),
    code(0xC2, 0x8008_E54C, 0x1D, 0xFFBA_596D_3FAB_D386, Rule::Ucf),
    code(0xC2, 0x8009_A0B8, 0x0A, 0x6FFA_7A17_2601_81B7, Rule::Ucf),
    code(0xC2, 0x8009_98A4, 0x19, 0x657D_47BA_9BCF_1BAF, Rule::Ucf),
    code(0xC2, 0x8009_3294, 0x14, 0xC48A_A2D2_63A5_C13D, Rule::Ucf),
    code(0xC2, 0x8009_08F4, 0x13, 0xE43E_58DD_5EE2_94C0, Rule::Ucf),
    code(
        0x04,
        0x8012_39A8,
        0x6000_0000,
        0xCBF2_9CE4_8422_2325,
        Rule::FreezeGlitchFix,
    ),
    code(
        0xC2,
        0x801C_154C,
        0x04,
        0x4730_0777_B4D4_972E,
        Rule::Determinism,
    ),
    code(
        0xC2,
        0x8006_8EEC,
        0x06,
        0xB0F6_54A4_8B18_C237,
        Rule::Determinism,
    ),
    code(
        0xC2,
        0x8006_A880,
        0x1B,
        0xB851_262C_B2ED_2ED1,
        Rule::OffscreenLimits,
    ),
    code(
        0xC2,
        0x800D_4C1C,
        0x05,
        0xD2F5_8DF2_61F6_34E2,
        Rule::Determinism,
    ),
    code(
        0xC2,
        0x800D_4D68,
        0x0B,
        0xF74E_BDC9_76E3_6594,
        Rule::Determinism,
    ),
    code(
        0xC2,
        0x8008_0E80,
        0x11,
        0x0DBD_5729_4D66_DA2B,
        Rule::Determinism,
    ),
    code(
        0xC2,
        0x8021_AAE4,
        0x06,
        0x1A00_BD3E_6CCE_6332,
        Rule::Determinism,
    ),
    code(
        0xC2,
        0x801C_65C8,
        0x10,
        0xA75D_04E5_73A8_88E1,
        Rule::Determinism,
    ),
    code(
        0xC2,
        0x801D_4760,
        0x0A,
        0x6040_350C_9D48_D384,
        Rule::Determinism,
    ),
    code(
        0xC2,
        0x801D_457C,
        0x07,
        0x1C04_7DB3_06D1_437E,
        Rule::Determinism,
    ),
    code(
        0xC2,
        0x8001_65AC,
        0x24,
        0xDBC3_A5B6_BFB4_BCCE,
        Rule::Determinism,
    ),
    code(
        0xC2,
        0x8008_653C,
        0x06,
        0xB985_C71C_63D5_601A,
        Rule::Determinism,
    ),
    code(
        0xC2,
        0x800A_C5B8,
        0x02,
        0xB6F1_25FA_806B_2F5A,
        Rule::Determinism,
    ),
    code(
        0xC2,
        0x801D_24FC,
        0x18,
        0x2B3B_FC6E_7A27_F239,
        Rule::Determinism,
    ),
    code(
        0xC2,
        0x800D_B880,
        0x03,
        0x4BDB_DD62_8187_A11E,
        Rule::PreventWobbling,
    ),
    code(
        0xC2,
        0x800D_BBD4,
        0x03,
        0x4BDB_DD62_8187_A11E,
        Rule::PreventWobbling,
    ),
    code(
        0xC2,
        0x8008_F090,
        0x3D,
        0xA172_2F3A_96B2_D217,
        Rule::PreventWobbling,
    ),
];

/// UCF's pad buffer code (`UCF Pad Buffer + 1.0 Cardinals.asm`) and its data: four ports' raw
/// stick histories, from the word after its first.
pub const UCF_PAD_BUFFER: u32 = 0x8006_B460;

/// The words of a code that are data it keeps, by injection address: left out of its hash.
fn data_words(addr: u32) -> std::ops::Range<usize> {
    match addr {
        // The scratch double the stick distance check converts through.
        0x800D_65EC | 0x8009_98A4 => 4..6,
        // The pad buffer.
        UCF_PAD_BUFFER => 1..13,
        _ => 0..0,
    }
}

fn fnv(words: impl Iterator<Item = u32>) -> u64 {
    let mut h = 0xCBF2_9CE4_8422_2325u64;
    for w in words {
        for b in w.to_be_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01B3);
        }
    }
    h
}

/// One code of a list, as a code handler reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Code {
    pub kind: u8,
    pub addr: u32,
    pub first: u32,
    /// The words of its lines after the first.
    pub words: Vec<u32>,
}

/// A code list's codes, up to its terminator (the format Slippi's playback hands the game).
pub fn parse_codes(raw: &[u8]) -> Vec<Code> {
    let lines: Vec<(u32, u32)> = raw
        .as_chunks::<8>()
        .0
        .iter()
        .map(|c| {
            (
                u32::from_be_bytes(c[0..4].try_into().unwrap()),
                u32::from_be_bytes(c[4..8].try_into().unwrap()),
            )
        })
        .collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let (a, d) = lines[i];
        let kind = (a >> 24) as u8 & 0xFE;
        if kind == 0xF0 || kind == 0xFE {
            break;
        }
        let n = match kind {
            0xC0 | 0xC2 => d as usize,
            0x06 => (d as usize).div_ceil(8),
            0x08 => 1,
            _ => 0,
        };
        let end = (i + 1 + n).min(lines.len());
        out.push(Code {
            kind,
            addr: 0x8000_0000 | (a & 0x01FF_FFFF),
            first: d,
            words: lines[i + 1..end]
                .iter()
                .flat_map(|&(x, y)| [x, y])
                .collect(),
        });
        i += 1 + n;
    }
    out
}

/// What [`Rules::from_codes`] can't play.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CodeError {
    /// A code no rule covers.
    Unknown { kind: u8, addr: u32 },
    /// A code where a known one injects, running other instructions.
    Changed { addr: u32 },
    /// Some of a rule's codes without the rest.
    Partial { rule: &'static str },
}

impl std::fmt::Display for CodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            CodeError::Unknown { kind, addr } => {
                write!(f, "an unknown gameplay code ({kind:02X} at {addr:#010x})")
            }
            CodeError::Changed { addr } => {
                write!(
                    f,
                    "the code at {addr:#010x} isn't the version melee-hd knows"
                )
            }
            CodeError::Partial { rule } => write!(f, "only some of {rule}'s codes"),
        }
    }
}

impl std::error::Error for CodeError {}

impl Rules {
    /// The Slippi ruleset as Slippi's online play runs it.
    pub fn slippi_online() -> Rules {
        Rules {
            slippi: true,
            ucf: true,
            neutral_spawn: true,
            prevent_wobbling: true,
            offscreen_limits: true,
            freeze_glitch_fix: true,
        }
    }

    /// The rules a replay's gameplay codes make (its code list as Slippi's playback applies
    /// it, without the codes playback leaves out).
    pub fn from_codes(codes: &[Code]) -> Result<Rules, CodeError> {
        let mut seen = Vec::new();
        for c in codes {
            let k = KNOWN
                .iter()
                .find(|k| k.addr == c.addr && k.kind == c.kind)
                .ok_or(CodeError::Unknown {
                    kind: c.kind,
                    addr: c.addr,
                })?;
            let data = data_words(c.addr);
            let last = if c.kind == 0xC2 {
                c.words.len().checked_sub(1)
            } else {
                None
            };
            let words = c.words.iter().enumerate().map(|(i, &w)| {
                if data.contains(&i) || Some(i) == last {
                    0
                } else {
                    w
                }
            });
            if c.first != k.first || fnv(words) != k.hash {
                return Err(CodeError::Changed { addr: c.addr });
            }
            seen.push(k.rule);
        }
        let mut rules = Rules::default();
        for (rule, name, flag) in [
            (Rule::Ucf, "UCF 0.84", &mut rules.ucf),
            (
                Rule::NeutralSpawn,
                "neutral spawns",
                &mut rules.neutral_spawn,
            ),
            (
                Rule::PreventWobbling,
                "wobbling prevention",
                &mut rules.prevent_wobbling,
            ),
            (
                Rule::OffscreenLimits,
                "off-screen damage limits",
                &mut rules.offscreen_limits,
            ),
            (
                Rule::Determinism,
                "the determinism fixes",
                &mut rules.slippi,
            ),
            (
                Rule::FreezeGlitchFix,
                "the freeze glitch fix",
                &mut rules.freeze_glitch_fix,
            ),
        ] {
            let all = KNOWN.iter().filter(|k| k.rule == rule).count();
            let have = seen.iter().filter(|&&r| r == rule).count();
            match have {
                0 => {}
                n if n == all => *flag = true,
                _ => return Err(CodeError::Partial { rule: name }),
            }
        }
        Ok(rules)
    }
}

/// Where a `C2` code's branch at `site` leads: its instructions, wherever the code handler or
/// playback placed them, if `word` (what the site holds) is such a branch.
pub fn placed(site: u32, word: u32) -> Option<u32> {
    (word & 0xFC00_0003 == 0x4800_0000)
        .then(|| site.wrapping_add((((word & 0x03FF_FFFC) << 6) as i32 >> 6) as u32))
}

impl Rules {
    /// The rules whose codes a running game has applied, read from its memory (`read` gives
    /// the word at an address): each known code found at its site, its instructions where its
    /// branch leads.
    pub fn from_memory(read: &dyn Fn(u32) -> u32) -> Result<Rules, CodeError> {
        let mut codes = Vec::new();
        for k in KNOWN {
            let at = read(k.addr);
            match k.kind {
                0x04 if at == k.first => codes.push(Code {
                    kind: k.kind,
                    addr: k.addr,
                    first: k.first,
                    words: Vec::new(),
                }),
                0xC2 => {
                    if let Some(body) = placed(k.addr, at) {
                        let n = 2 * k.first;
                        codes.push(Code {
                            kind: k.kind,
                            addr: k.addr,
                            first: k.first,
                            words: (0..n).map(|i| read(body + 4 * i)).collect(),
                        });
                    }
                }
                _ => {}
            }
        }
        Rules::from_codes(&codes)
    }
}

impl super::ucf::Pads {
    /// The pad buffer's data in a running game's memory, if UCF's code is applied.
    pub fn from_memory(read: &dyn Fn(u32) -> u32) -> Option<super::ucf::Pads> {
        let body = placed(UCF_PAD_BUFFER, read(UCF_PAD_BUFFER))?;
        let words: Vec<u32> = (1..13).map(|i| read(body + 4 * i)).collect();
        Some(super::ucf::Pads::from_code_words(&words))
    }

    /// The pad buffer's data as a code list holds it, if the list has UCF.
    pub fn from_codes(codes: &[Code]) -> Option<super::ucf::Pads> {
        let buffer = codes.iter().find(|c| c.addr == UCF_PAD_BUFFER)?;
        Some(super::ucf::Pads::from_code_words(&buffer.words[1..13]))
    }
}

/// Neutral spawn points by stage (`StKind`), FFA's four then teams' four, as the code's table
/// holds them (x and y bits).
type Points = [[u32; 2]; 8];
const NEUTRAL_SPAWNS: [(i32, Points); 6] = [
    (
        32,
        [
            [0xC2700000, 0x41200000],
            [0x42700000, 0x41200000],
            [0xC1A00000, 0x41200000],
            [0x41A00000, 0x41200000],
            [0xC2700000, 0x41200000],
            [0xC1A00000, 0x41200000],
            [0x42700000, 0x41200000],
            [0x41A00000, 0x41200000],
        ],
    ),
    (
        31,
        [
            [0xC21B3333, 0x420CCCCD],
            [0x421B3333, 0x420CCCCD],
            [0x00000000, 0x41000000],
            [0x00000000, 0x4279999A],
            [0xC21B3333, 0x420CCCCD],
            [0xC21B3333, 0x40A00000],
            [0x421B3333, 0x420CCCCD],
            [0x421B3333, 0x40A00000],
        ],
    ),
    (
        8,
        [
            [0xC2280000, 0x41D4CCCD],
            [0x42280000, 0x41E00000],
            [0x00000000, 0x423B999A],
            [0x00000000, 0x409CCCCD],
            [0xC2280000, 0x41D4CCCD],
            [0xC2280000, 0x40A00000],
            [0x42280000, 0x41E00000],
            [0x42280000, 0x40A00000],
        ],
    ),
    (
        28,
        [
            [0xC23A6666, 0x4214CCCD],
            [0x423D999A, 0x42153333],
            [0x00000000, 0x40E00000],
            [0x00000000, 0x426A0000],
            [0xC23A6666, 0x4214CCCD],
            [0xC23A6666, 0x40A00000],
            [0x423D999A, 0x42153333],
            [0x423D999A, 0x40A00000],
        ],
    ),
    (
        2,
        [
            [0xC2250000, 0x41A80000],
            [0x42250000, 0x41D80000],
            [0x00000000, 0x40A80000],
            [0x00000000, 0x42400000],
            [0xC2250000, 0x41A80000],
            [0xC2250000, 0x40A00000],
            [0x42250000, 0x41D80000],
            [0x42250000, 0x40A00000],
        ],
    ),
    (
        3,
        [
            [0xC2200000, 0x42000000],
            [0x42200000, 0x42000000],
            [0x428C0000, 0x40E00000],
            [0xC28C0000, 0x40E00000],
            [0xC2200000, 0x42000000],
            [0xC2200000, 0x40A00000],
            [0x42200000, 0x42000000],
            [0x42200000, 0x40A00000],
        ],
    ),
];

/// The spawn point a team member takes when the stage has no neutral points: entry points by
/// position in the team order.
const TEAM_ENTRY_POINTS: [u8; 4] = [0, 3, 1, 2];

impl super::state::Running<'_> {
    /// Neutral spawns (`fn_8016E2BC+0x254`, `External/NeutralSpawn`): in a VS match of up to
    /// four players outside the single-player modes, after a slot's spawn point is set, the
    /// slot moves to its stage's neutral point for its place among the players (FFA) or in the
    /// team order (teams of two), facing the centre. A team match whose teams aren't all pairs
    /// keeps the stage's spawns.
    #[doc(alias = "code_8016E510")]
    pub fn neutral_spawn(&mut self, slot: i32) {
        const NONE: i32 = 3;
        let g = &*self.game;
        if super::gm::is_1p_mode(g) || slot >= 5 {
            return;
        }
        let scene = (g.game_mode, g.gm.scene_index);
        if scene == (0x1C, 2) || scene == (0x0F, 1) {
            return;
        }
        let teams = g.vs.start.is_teams == 1;
        let place = if !teams {
            let mut place = 0;
            for i in 0..=4 {
                if g.player_pkind(i) == NONE {
                    continue;
                }
                if i == slot {
                    break;
                }
                place += 1;
            }
            place
        } else {
            for team in 0..3 {
                let n = (0..4)
                    .filter(|&p| g.player_pkind(p) != NONE && g.player_team(p) == team)
                    .count();
                if n == 1 || n > 2 {
                    return;
                }
            }
            let order: Vec<i32> = (0..3)
                .flat_map(|team| {
                    (0..4).filter(move |&p| g.player_pkind(p) != NONE && g.player_team(p) == team)
                })
                .collect();
            // The code looks for the slot in the list's first four entries, past its end on
            // the stack it never wrote: a fifth slot in a team match isn't modeled.
            order
                .iter()
                .position(|&p| p == slot)
                .expect("a team match's fifth slot reads stack the neutral spawn never wrote")
                as i32
        };
        self.neutral_spawn_at(slot, place, teams);
    }

    fn neutral_spawn_at(&mut self, slot: i32, place: i32, teams: bool) {
        let stage = self.game.gr.stkind;
        let pos = match NEUTRAL_SPAWNS.iter().find(|(s, _)| *s == stage) {
            Some((_, points)) => {
                let [x, y] = points[4 * usize::from(teams) + place as usize];
                crate::vec::Vec3::new(f32::from_bits(x), f32::from_bits(y), 0.0)
            }
            None => {
                let no = if teams {
                    i32::from(TEAM_ENTRY_POINTS[place as usize])
                } else {
                    place
                };
                self.spawn_point(no)
            }
        };
        self.game.set_player_pos(slot, pos);
        let x = self.game.player_pos(slot).x;
        self.game
            .set_player_facing(slot, if !(x <= 0.0) { -1.0 } else { 1.0 });
    }
}

impl super::state::Game {
    /// Off-screen damage from the stage's limits (`Fighter_procAnim+0x520`,
    /// `Online/Core/BrawlOffscreenDamage`), in place of the magnifier's check: a fighter is
    /// off screen past the stage's camera limits, except in the scene 0x0120, while
    /// `x221F_b1` is set, and in actions 4 and 6.
    #[doc(alias = "code_8006A880")]
    pub fn offscreen_limits(&self, f: super::handles::FighterId) -> bool {
        use super::gr::stage::{cam_bottom, cam_left, cam_right, cam_top};
        use crate::fpu::reg;
        let fp = &self.fighters[f.0];
        if (self.game_mode, self.gm.scene_index) == (0x20, 1)
            || fp.x221F_b1
            || fp.motion_id == 4
            || fp.motion_id == 6
        {
            return false;
        }
        let (x, y) = (reg(fp.cur_pos.x), reg(fp.cur_pos.y));
        x < reg(cam_left(self))
            || x > reg(cam_right(self))
            || y > reg(cam_top(self))
            || y < reg(cam_bottom(self))
    }
}
