//! Classic's stage states (`gmclassic.c`): each stage's intro (the intro scene's enter data,
//! `gmClassic_801B3500`) and match (`gmClassic_801B3A34`, its exit `gmClassic_801B3B40`), and
//! the run's tables' getters (`gm_17EB.c`: `lbl_803D85F0` by stage and difficulty).

use super::super::handles::CodeRef;
use super::super::state::Running;
use super::one_player::{CLASSIC_STAGES, Unk1PData, matchup};

/// `ChKind_None`, `CKind_Zelda`, `CKind_Seak`, `CKind_Kirby`.
const NONE: i8 = 0x21;
const CKIND_ZELDA: i8 = 0x12;
const CKIND_SEAK: u8 = 0x13;
const CKIND_KIRBY: i8 = 4;

/// An entry of `lbl_803D85F0` (`AllstarStageEntry`, 0x1A bytes): the item frequency, the
/// opponents' attack and defense ratios in percent, three opponents' level, costume and
/// computer kind, the allies' ratios and two allies' level, computer kind and costume.
#[derive(Clone, Copy, Debug)]
pub struct Level {
    pub items: u8,
    pub attack: u16,
    pub defense: u16,
    pub enemies: [u8; 10],
    pub ally_attack: u16,
    pub ally_defense: u16,
    pub allies: [u8; 6],
}

#[allow(clippy::too_many_arguments)]
const fn ce(
    items: u8,
    _x1: u8,
    attack: u16,
    defense: u16,
    enemies: [u8; 10],
    ally_attack: u16,
    ally_defense: u16,
    allies: [u8; 6],
) -> Level {
    Level {
        items,
        attack,
        defense,
        enemies,
        ally_attack,
        ally_defense,
        allies,
    }
}

/// `lbl_803D85F0`: Classic's eleven stages before the last, five difficulties each (the
/// executable's data; the bonus stage's rows are 0xFF).
pub const LEVELS: [Level; 55] = [
    ce(
        0x04,
        0,
        60,
        175,
        [0, 0, 9, 0, 0, 0, 0, 0, 0, 0],
        0,
        0,
        [0, 0, 0, 0, 0, 0],
    ),
    ce(
        0x03,
        0,
        85,
        120,
        [2, 0, 4, 0, 0, 0, 0, 0, 0, 0],
        0,
        0,
        [0, 0, 0, 0, 0, 0],
    ),
    ce(
        0x03,
        0,
        100,
        100,
        [4, 0, 4, 0, 0, 0, 0, 0, 0, 0],
        0,
        0,
        [0, 0, 0, 0, 0, 0],
    ),
    ce(
        0x02,
        0,
        100,
        100,
        [7, 0, 4, 0, 0, 0, 0, 0, 0, 0],
        0,
        0,
        [0, 0, 0, 0, 0, 0],
    ),
    ce(
        0x01,
        0,
        120,
        90,
        [9, 3, 8, 0, 0, 0, 0, 0, 0, 0],
        0,
        0,
        [0, 0, 0, 0, 0, 0],
    ),
    ce(
        0x04,
        0,
        70,
        160,
        [1, 0, 4, 2, 0, 9, 0, 0, 0, 0],
        80,
        90,
        [8, 1, 4, 0, 0, 0],
    ),
    ce(
        0x04,
        0,
        85,
        120,
        [3, 0, 4, 3, 0, 4, 0, 0, 0, 0],
        80,
        100,
        [7, 1, 4, 0, 0, 0],
    ),
    ce(
        0x03,
        0,
        100,
        100,
        [5, 0, 4, 4, 0, 4, 0, 0, 0, 0],
        80,
        115,
        [6, 1, 4, 0, 0, 0],
    ),
    ce(
        0x03,
        0,
        100,
        90,
        [6, 0, 4, 6, 0, 8, 0, 0, 0, 0],
        80,
        130,
        [4, 1, 4, 0, 0, 0],
    ),
    ce(
        0x01,
        0,
        115,
        80,
        [8, 3, 4, 8, 0, 8, 0, 0, 0, 0],
        80,
        150,
        [3, 1, 4, 0, 0, 0],
    ),
    ce(
        0xff,
        0,
        0,
        0,
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        0,
        0,
        [0, 0, 0, 0, 0, 0],
    ),
    ce(
        0xff,
        0,
        0,
        0,
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        0,
        0,
        [0, 0, 0, 0, 0, 0],
    ),
    ce(
        0xff,
        0,
        0,
        0,
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        0,
        0,
        [0, 0, 0, 0, 0, 0],
    ),
    ce(
        0xff,
        0,
        0,
        0,
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        0,
        0,
        [0, 0, 0, 0, 0, 0],
    ),
    ce(
        0xff,
        0,
        0,
        0,
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        0,
        0,
        [0, 0, 0, 0, 0, 0],
    ),
    ce(
        0x04,
        0,
        75,
        140,
        [2, 0, 4, 0, 0, 0, 0, 0, 0, 0],
        0,
        0,
        [0, 0, 0, 0, 0, 0],
    ),
    ce(
        0x04,
        0,
        90,
        110,
        [5, 0, 4, 0, 0, 0, 0, 0, 0, 0],
        0,
        0,
        [0, 0, 0, 0, 0, 0],
    ),
    ce(
        0x04,
        0,
        100,
        100,
        [8, 0, 4, 0, 0, 0, 0, 0, 0, 0],
        0,
        0,
        [0, 0, 0, 0, 0, 0],
    ),
    ce(
        0x04,
        0,
        115,
        85,
        [9, 0, 4, 0, 0, 0, 0, 0, 0, 0],
        0,
        0,
        [0, 0, 0, 0, 0, 0],
    ),
    ce(
        0x04,
        0,
        134,
        70,
        [9, 0, 4, 0, 0, 0, 0, 0, 0, 0],
        0,
        0,
        [0, 0, 0, 0, 0, 0],
    ),
    ce(
        0x03,
        0,
        70,
        360,
        [2, 0, 27, 0, 0, 0, 0, 0, 0, 0],
        100,
        100,
        [6, 2, 4, 6, 1, 4],
    ),
    ce(
        0x03,
        0,
        80,
        240,
        [3, 0, 27, 0, 0, 0, 0, 0, 0, 0],
        100,
        90,
        [5, 2, 4, 5, 1, 4],
    ),
    ce(
        0x02,
        0,
        90,
        180,
        [5, 0, 27, 0, 0, 0, 0, 0, 0, 0],
        100,
        80,
        [2, 2, 4, 2, 1, 9],
    ),
    ce(
        0x02,
        0,
        100,
        140,
        [7, 0, 4, 0, 0, 0, 0, 0, 0, 0],
        100,
        80,
        [1, 2, 4, 1, 1, 9],
    ),
    ce(
        0x01,
        0,
        120,
        100,
        [8, 0, 4, 0, 0, 0, 0, 0, 0, 0],
        100,
        80,
        [0, 2, 4, 0, 1, 9],
    ),
    ce(
        0xff,
        0,
        0,
        0,
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        0,
        0,
        [0, 0, 0, 0, 0, 0],
    ),
    ce(
        0xff,
        0,
        0,
        0,
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        0,
        0,
        [0, 0, 0, 0, 0, 0],
    ),
    ce(
        0xff,
        0,
        0,
        0,
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        0,
        0,
        [0, 0, 0, 0, 0, 0],
    ),
    ce(
        0xff,
        0,
        0,
        0,
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        0,
        0,
        [0, 0, 0, 0, 0, 0],
    ),
    ce(
        0xff,
        0,
        0,
        0,
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        0,
        0,
        [0, 0, 0, 0, 0, 0],
    ),
    ce(
        0x02,
        0,
        85,
        130,
        [3, 0, 4, 0, 0, 0, 0, 0, 0, 0],
        0,
        0,
        [0, 0, 0, 0, 0, 0],
    ),
    ce(
        0x02,
        0,
        100,
        110,
        [6, 0, 4, 0, 0, 0, 0, 0, 0, 0],
        0,
        0,
        [0, 0, 0, 0, 0, 0],
    ),
    ce(
        0x02,
        0,
        90,
        130,
        [5, 0, 4, 0, 0, 0, 0, 0, 0, 0],
        0,
        0,
        [0, 0, 0, 0, 0, 0],
    ),
    ce(
        0x02,
        0,
        100,
        110,
        [6, 0, 18, 0, 0, 0, 0, 0, 0, 0],
        0,
        0,
        [0, 0, 0, 0, 0, 0],
    ),
    ce(
        0x01,
        0,
        110,
        90,
        [8, 0, 18, 0, 0, 0, 0, 0, 0, 0],
        0,
        0,
        [0, 0, 0, 0, 0, 0],
    ),
    ce(
        0x03,
        0,
        60,
        400,
        [1, 0, 25, 0, 0, 0, 0, 0, 0, 0],
        0,
        0,
        [0, 0, 0, 0, 0, 0],
    ),
    ce(
        0x03,
        0,
        70,
        260,
        [3, 0, 23, 0, 0, 0, 0, 0, 0, 0],
        0,
        0,
        [0, 0, 0, 0, 0, 0],
    ),
    ce(
        0x02,
        0,
        80,
        200,
        [4, 0, 23, 0, 0, 0, 0, 0, 0, 0],
        0,
        0,
        [0, 0, 0, 0, 0, 0],
    ),
    ce(
        0x02,
        0,
        90,
        155,
        [5, 0, 23, 0, 0, 0, 0, 0, 0, 0],
        0,
        0,
        [0, 0, 0, 0, 0, 0],
    ),
    ce(
        0x01,
        0,
        110,
        120,
        [7, 0, 23, 0, 0, 0, 0, 0, 0, 0],
        0,
        0,
        [0, 0, 0, 0, 0, 0],
    ),
    ce(
        0x02,
        0,
        100,
        100,
        [1, 0, 4, 0, 0, 4, 0, 0, 4, 0],
        100,
        100,
        [0, 0, 0, 0, 0, 0],
    ),
    ce(
        0x02,
        0,
        100,
        100,
        [1, 0, 4, 0, 0, 4, 0, 0, 4, 0],
        100,
        100,
        [0, 0, 0, 0, 0, 0],
    ),
    ce(
        0x02,
        0,
        100,
        100,
        [1, 0, 4, 0, 0, 4, 0, 0, 4, 0],
        100,
        100,
        [0, 0, 0, 0, 0, 0],
    ),
    ce(
        0x02,
        0,
        100,
        100,
        [1, 0, 4, 0, 0, 4, 0, 0, 4, 0],
        100,
        100,
        [0, 0, 0, 0, 0, 0],
    ),
    ce(
        0x02,
        0,
        100,
        100,
        [1, 0, 4, 0, 0, 4, 0, 0, 4, 0],
        100,
        100,
        [0, 0, 0, 0, 0, 0],
    ),
    ce(
        0x02,
        0,
        80,
        180,
        [1, 0, 27, 0, 0, 0, 0, 0, 0, 0],
        0,
        0,
        [0, 0, 0, 0, 0, 0],
    ),
    ce(
        0x02,
        0,
        90,
        130,
        [3, 0, 27, 0, 0, 0, 0, 0, 0, 0],
        0,
        0,
        [0, 0, 0, 0, 0, 0],
    ),
    ce(
        0x02,
        0,
        100,
        100,
        [5, 0, 27, 0, 0, 0, 0, 0, 0, 0],
        0,
        0,
        [0, 0, 0, 0, 0, 0],
    ),
    ce(
        0x01,
        0,
        110,
        85,
        [8, 0, 27, 0, 0, 0, 0, 0, 0, 0],
        0,
        0,
        [0, 0, 0, 0, 0, 0],
    ),
    ce(
        0xff,
        0,
        125,
        68,
        [9, 0, 27, 0, 0, 0, 0, 0, 0, 0],
        0,
        0,
        [0, 0, 0, 0, 0, 0],
    ),
    ce(
        0x02,
        0,
        50,
        100,
        [5, 0, 4, 0, 0, 0, 0, 0, 0, 0],
        0,
        0,
        [0, 0, 0, 0, 0, 0],
    ),
    ce(
        0x02,
        0,
        75,
        100,
        [6, 0, 4, 0, 0, 0, 0, 0, 0, 0],
        0,
        0,
        [0, 0, 0, 0, 0, 0],
    ),
    ce(
        0x02,
        0,
        100,
        100,
        [7, 0, 4, 0, 0, 0, 0, 0, 0, 0],
        0,
        0,
        [0, 0, 0, 0, 0, 0],
    ),
    ce(
        0x02,
        0,
        150,
        100,
        [8, 0, 4, 0, 0, 0, 0, 0, 0, 0],
        0,
        0,
        [0, 0, 0, 0, 0, 0],
    ),
    ce(
        0xff,
        0,
        200,
        100,
        [9, 0, 4, 0, 0, 0, 0, 0, 0, 0],
        0,
        0,
        [0, 0, 0, 0, 0, 0],
    ),
];

/// A stage's row for a difficulty (`lbl_803D85F0[level + stage * 5]`).
fn level(stage: u8, difficulty: u8) -> &'static Level {
    &LEVELS[usize::from(difficulty) + usize::from(stage) * 5]
}

/// Classic's run's `u8` getters (`Unk1PData::x48` to `x60`), by their addresses, for a stage,
/// difficulty and index; `None` for another getter.
#[doc(alias = "gm_8017EB3C")]
#[doc(alias = "gm_8017EB64")]
#[doc(alias = "gm_8017EB98")]
#[doc(alias = "gm_8017EBCC")]
#[doc(alias = "gm_8017ECA0")]
// Ports gm_8017ECD4 and gm_8017ED08 (the allies'); aliased once a check reaches them.
pub fn getter(getter: CodeRef, stage: u8, difficulty: u8, i: u8) -> Option<u8> {
    let e = level(stage, difficulty);
    let i = usize::from(i) * 3;
    Some(match getter.0 {
        0x8017_EB3C => e.items,
        0x8017_EB64 => e.enemies[i],
        0x8017_EB98 => e.enemies[i + 1],
        0x8017_EBCC => e.enemies[i + 2],
        0x8017_ECA0 => e.allies[i],
        0x8017_ECD4 => e.allies[i + 1],
        0x8017_ED08 => e.allies[i + 2],
        _ => return None,
    })
}

/// Classic's run's ratio getters (`x64` to `x70`), a hundredth of the row's percent: the
/// opponents' attack (`gm_8017ED3C`) and defense (`gm_8017ED8C`), the allies' (`gm_8017EC00`,
/// `gm_8017EC50`); `None` for another getter.
#[doc(alias = "gm_8017ED3C")]
#[doc(alias = "gm_8017ED8C")]
// Ports gm_8017EC00 and gm_8017EC50 (the allies'); aliased once a check reaches them.
pub fn ratio_percent(getter: CodeRef, stage: u8, difficulty: u8) -> Option<u16> {
    let e = level(stage, difficulty);
    Some(match getter.0 {
        0x8017_ED3C => e.attack,
        0x8017_ED8C => e.defense,
        0x8017_EC00 => e.ally_attack,
        0x8017_EC50 => e.ally_defense,
        _ => return None,
    })
}

/// How many allies a stage's flags give (`fn_8017DE54`): none on a stage with the flag 0x20,
/// on a team stage (0x10) as many as make the opponents' count three, two on a giant's
/// stage (2).
#[doc(alias = "fn_8017DE54")]
fn ally_count(flags: u8, chars: &[i8; 3]) -> usize {
    if flags & 0x20 != 0 {
        return 0;
    }
    if flags & 0x10 != 0 {
        return 3 - chars.iter().filter(|&&c| c != NONE).count();
    }
    if flags & 2 != 0 {
        return 2;
    }
    0
}

/// `gmClassicIntroData` (`gmClassicIntroDataBuffer`, 0x20 bytes): the intro scene's enter
/// data, its offsets.
pub mod intro_data {
    pub const KIND: usize = 0x00;
    pub const GAME_TYPE: usize = 0x04;
    pub const SLOT: usize = 0x08;
    pub const NAMETAG: usize = 0x09;
    pub const STAGE: usize = 0x0A;
    pub const ALLIES: usize = 0x0B;
    pub const ENEMIES: usize = 0x0C;
    pub const ALLY_KINDS: usize = 0x0D;
    pub const ENEMY_KINDS: usize = 0x10;
    pub const ALLY_COLORS: usize = 0x13;
    pub const ENEMY_COLORS: usize = 0x16;
    pub const ENEMY_METAL: usize = 0x1C;
}

impl Running<'_> {
    /// The allies of a stage with them, `gm_8017DB88`: `count` of them (`fn_8017DE54`) into
    /// the run's (`x24`), each a character drawn at random, the rest none.
    #[doc(alias = "gm_8017DB88")]
    fn classic_allies(&mut self, run: &mut Unk1PData, flags: u8, chars: &[i8; 3]) {
        let count = ally_count(flags, chars);
        if count != 0 {
            crate::game::code::unported_call("Classic's allies drawn (fn_8017D9C0)");
        }
        for a in run.opponents.iter_mut().skip(count) {
            a.ckind = NONE;
        }
    }

    /// Classic's stage's intro state's entry, `gmClassic_801B3500`: the intro scene's enter
    /// data (its kind by the stage's flags, the stage's number, the opponents and their
    /// costumes kept apart from the player's, the metal flag, the player and allies), the
    /// game cache's player first and waited for, then the opponents, the allies and the
    /// stage, and the match's sound banks.
    #[doc(alias = "gmClassic_801B3500")]
    #[doc(alias = "gm_8017CD94")]
    #[doc(alias = "gm_8017DB6C")]
    #[doc(alias = "gm_8017DB78")]
    pub fn classic_enter_intro(&mut self) {
        use intro_data as d;
        let id = self.game.machine.routing.curr_state_id;
        let round = usize::from((id >> 3) & 0x1F);
        let entry = CLASSIC_STAGES[round];
        let (stkind, chars) =
            matchup(self.game.one_player.classic_picks[round]).expect("a Classic stage's matchup");
        let mut run = self.game.one_player.classic.base;
        run.player.mode = id;
        let mut sd = self.game.one_player.classic_intro;
        sd[d::STAGE] = entry.x0.wrapping_add(1);
        sd[d::SLOT] = run.player.slot;
        let kind: i32 = if entry.x1 & 0x80 != 0 {
            3
        } else if entry.x1 & 8 != 0 {
            4
        } else if entry.x1 & 0x10 != 0 {
            1
        } else if entry.x1 & 2 != 0 {
            2
        } else {
            0
        };
        sd[d::KIND..d::KIND + 4].copy_from_slice(&kind.to_be_bytes());
        if kind == 3 {
            if let 1..=3 = entry.x2 {
                sd[d::GAME_TYPE..d::GAME_TYPE + 4]
                    .copy_from_slice(&i32::from(entry.x2).to_be_bytes());
            }
        } else {
            sd[d::GAME_TYPE..d::GAME_TYPE + 4].fill(0);
        }
        let mut enemies = 0;
        for i in 0..3 {
            sd[d::ENEMY_KINDS + i] = chars[i] as u8;
            // `gm_8017CD94`: the run's costume getter's, within the character's costumes.
            let n = crate::game::playback::costume_count(chars[i] as u8);
            let c = match run.getters[3] {
                Some(g) => {
                    let v =
                        Self::one_player_getter(Some(g), entry.x0, run.player.cpu_level, i as u8);
                    if n != 0 { v % n } else { 0 }
                }
                None => 0,
            };
            sd[d::ENEMY_COLORS + i] = c;
            let mut colors = [
                sd[d::ENEMY_COLORS],
                sd[d::ENEMY_COLORS + 1],
                sd[d::ENEMY_COLORS + 2],
            ];
            super::one_player::enemy_color_table(
                run.player.ckind,
                run.player.color,
                &chars,
                &mut colors,
            );
            sd[d::ENEMY_COLORS..d::ENEMY_COLORS + 3].copy_from_slice(&colors);
            sd[d::ENEMY_METAL + i] = u8::from(entry.x1 & 4 != 0);
            if chars[i] != NONE {
                enemies += 1;
            }
        }
        sd[d::ENEMIES] = enemies;
        let ckind = run.player.ckind;
        sd[d::ALLY_KINDS] = if ckind == CKIND_ZELDA && run.xc[6] != 0 {
            CKIND_SEAK
        } else {
            ckind as u8
        };
        sd[d::ALLY_COLORS] = run.player.color;
        self.classic_allies(&mut run, entry.x1, &chars);
        let mut allies = 1;
        for i in 1..3 {
            sd[d::ALLY_KINDS + i] = run.opponents[i - 1].ckind as u8;
            sd[d::ALLY_COLORS + i] = run.opponents[i - 1].color;
            if run.opponents[i - 1].ckind != NONE {
                allies += 1;
            }
        }
        sd[d::ALLIES] = allies;
        sd[d::NAMETAG] = run.player.nametag;
        self.game.one_player.classic_intro = sd;
        self.game.one_player.classic.base = run;
        self.dvd_mut().keep_class();
        let gc = &mut self.dvd_mut().cache.scene.game_cache;
        gc.entries[0].char_id = i32::from(sd[d::ALLY_KINDS]);
        gc.entries[0].color = run.player.color;
        self.preload_refresh();
        self.wait_preload_class(0xC7);
        self.wait_preload_stale_released(4);
        let gc = &mut self.dvd_mut().cache.scene.game_cache;
        let mut slot = 1;
        for i in 0..3 {
            if chars[i] != NONE {
                gc.entries[slot].char_id = i32::from(chars[i]);
                gc.entries[slot].color = if entry.x1 & 8 != 0 {
                    0xFF
                } else {
                    sd[d::ENEMY_COLORS + i]
                };
                slot += 1;
            }
        }
        for a in run.opponents {
            if a.ckind != NONE {
                gc.entries[slot].char_id = i32::from(a.ckind);
                gc.entries[slot].color = a.color;
                slot += 1;
            }
        }
        self.preload_refresh();
        let target = entry.x1 == 0x80 && entry.x2 == 1;
        let cache_stage = if target {
            super::one_player::target_stage(ckind as u8)
        } else if entry.x1 == 4 {
            0xAF
        } else {
            stkind
        };
        self.dvd_mut().cache.scene.game_cache.stkind = i32::from(cache_stage);
        self.preload_refresh();
        use crate::lb::audio::{character_banks, stage_banks};
        let mut mask = character_banks(i32::from(ckind));
        for a in run.opponents {
            if a.ckind != NONE {
                mask |= character_banks(i32::from(a.ckind));
            }
        }
        for c in chars {
            if c != NONE {
                mask |= character_banks(i32::from(c));
                if c == CKIND_KIRBY {
                    mask |= (2 << 32) | 0x4000;
                }
            }
        }
        // `gmClassic_GetStKind`: the target stage's, else the matchup's (a stage with the
        // flag 4 alone keeps its matchup's here).
        let bank_stage = if target {
            super::one_player::target_stage(ckind as u8)
        } else {
            stkind
        };
        mask |= stage_banks(i32::from(bank_stage));
        self.game.audio.lb.unwant_banks(0x1C);
        self.game.audio.lb.want_banks(0xC, mask);
        self.game.audio.lb_start_bank_loads(self.data);
    }

    /// Classic's stage's match, `gmClassic_801B3A34`: the matchup's stage (the character's
    /// target stage on the target bonus stage, 0xAF on a stage with the flag 4 alone), the
    /// stage's flags into the run, and the stage's start data (`gm_8017CE34`) with its time
    /// and the color kind the run's last stage left, then the rumble setting.
    ///
    /// The color kind is `gm_804908A0[stage - 1]`: on the first stage, the byte before the
    /// table, the last of the intro data's (`gmClassicIntroDataBuffer`'s padding, which
    /// nothing writes).
    #[doc(alias = "gmClassic_801B3A34")]
    pub fn classic_enter_vs(&mut self) {
        let id = self.game.machine.routing.curr_state_id;
        let round = usize::from((id >> 3) & 0x1F);
        let entry = CLASSIC_STAGES[round];
        let (stkind, chars) =
            matchup(self.game.one_player.classic_picks[round]).expect("a Classic stage's matchup");
        let mut run = self.game.one_player.classic.base;
        let mut stage = i32::from(stkind);
        if entry.x1 == 0x80 && entry.x2 == 1 {
            stage = i32::from(super::one_player::target_stage(run.player.ckind as u8));
        }
        if entry.x1 == 4 {
            stage = 0xAF;
        }
        run.x8[0] = entry.x1;
        run.x8[1] = entry.x2;
        run.x8[3] = entry.x8;
        let color_kind = if round == 0 {
            self.game.one_player.classic_intro[0x1F]
        } else {
            self.game.one_player.classic_order[round - 1]
        };
        let mut start = self.game.one_player.match_start.clone();
        self.one_player_stage_start(
            &mut start,
            &mut run,
            &chars,
            entry.x6 as u8,
            1,
            0,
            i32::from(entry.x4),
            stage,
            round as i32,
            i32::from(color_kind),
        );
        self.game.one_player.classic.base = run;
        let (_, prefs) = super::attract::saved_rules(&self.game.game_data);
        let tags = self.nametag_rumbles();
        super::vs_mode::load_rumble_enabled_with(&mut start, &prefs, &tags);
        self.game.one_player.match_start = start;
    }

    /// Classic's stage's match's exit, `gmClassic_801B3B40`: the stage marked won (2) or not
    /// (1) in `gm_804908A0`, and the run's exit (`gm_8017D7AC`, the game over next at no
    /// stock); the last stage's clear and the target stage's records aren't ported.
    #[doc(alias = "gmClassic_801B3B40")]
    pub fn classic_exit_vs(&mut self) {
        let id = self.game.machine.routing.curr_state_id;
        let round = usize::from((id >> 3) & 0x1F);
        let entry = CLASSIC_STAGES[round];
        let won = self.game.one_player.match_exit.x8 != 0;
        // `gm_804908A0[stage - 1]`: the first stage's is the byte before the table.
        let mark = if won { 2 } else { 1 };
        if round == 0 {
            self.game.one_player.classic_intro[0x1F] = mark;
        } else {
            self.game.one_player.classic_order[round - 1] = mark;
        }
        let mut run = self.game.one_player.classic.base;
        let on = self.one_player_match_exit(&mut run, 0x69);
        self.game.one_player.classic.base = run;
        if on && CLASSIC_STAGES.get(round + 1).is_some_and(|e| e.x0 == 0xD) {
            crate::game::code::unported_call("gm_8017CBAC (Classic's clear)");
        }
        if entry.x1 == 0x80 && entry.x2 == 1 {
            crate::game::code::unported_call("gmClassic_801B3B40's target stage records");
        }
    }
}
