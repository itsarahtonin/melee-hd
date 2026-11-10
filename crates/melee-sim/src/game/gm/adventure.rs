//! Adventure's stage states (`gmadventure.c`) and its stage table (`gm_17E4.c`'s
//! `gm_803DE650`): each stage's intro (the intro scene's enter data, `gm_801B3F40`).

use super::super::handles::CodeRef;
use super::super::state::Running;

/// `ChKind_None`, `CKind_Kirby`, `CKind_Mario`, `CKind_Luigi`.
const NONE: u8 = 0x21;
const CKIND_KIRBY: u8 = 4;
const CKIND_MARIO: u8 = 8;
const CKIND_LUIGI: u8 = 7;
const CKIND_FALCO: u8 = 0x14;

/// An entry of `gm_803DE650` (`gm_803DE650_t`, 0x10 bytes): its state's id, the stage's
/// flags, its time, `x4` (a stage start's flag), the stage kind, `x8` and `x9`, and up to
/// three opponents.
#[derive(Clone, Copy, Debug)]
pub struct AdventureStage {
    pub id: u8,
    pub x1: u8,
    pub x2: u16,
    pub x4: u16,
    pub stkind: u16,
    pub x8: u8,
    pub x9: i8,
    pub chars: [u8; 3],
}

#[allow(clippy::too_many_arguments)]
const fn ae(
    id: u8,
    x1: u8,
    x2: u16,
    x4: u16,
    stkind: u16,
    x8: u8,
    x9: i8,
    chars: [u8; 3],
) -> AdventureStage {
    AdventureStage {
        id,
        x1,
        x2,
        x4,
        stkind,
        x8,
        x9,
        chars,
    }
}

/// `gm_803DE650`: Adventure's states' stages, by their ids (the executable's data; ended by
/// id 0xFF).
pub const ADVENTURE_TABLE: [AdventureStage; super::one_player::ADVENTURE_STAGES] = [
    ae(0x00, 0x00, 0, 0, 0x3B, 4, 0, [0x11, 0x11, 0x11]),
    ae(0x01, 0x09, 420, 0, 0x3B, 4, 0, [0x11, 0x11, 0x11]),
    ae(0x02, 0x00, 0, 0, 0x3B, 4, 0, [0x21, 0x21, 0x21]),
    ae(0x03, 0x00, 240, 0, 0x3C, 3, 0, [0x08, 0x0C, 0x21]),
    ae(0x08, 0x00, 0, 0, 0x3D, 4, 0, [0x01, 0x21, 0x21]),
    ae(0x09, 0x00, 240, 0, 0x3D, 3, 0, [0x01, 0x01, 0x21]),
    ae(0x0A, 0x02, 240, 0, 0x3E, 2, 0, [0x01, 0x21, 0x21]),
    ae(0x10, 0x00, 0, 0, 0x3F, 4, 0, [0x06, 0x21, 0x21]),
    ae(0x11, 0x09, 420, 0, 0x3F, 4, 0, [0x06, 0x21, 0x21]),
    ae(0x12, 0x00, 240, 0, 0x40, 2, 0, [0x12, 0x21, 0x21]),
    ae(0x18, 0x00, 0, 0, 0x41, 4, 0, [0x10, 0x21, 0x21]),
    ae(0x19, 0x00, 240, 0, 0x41, 2, 0, [0x10, 0x21, 0x21]),
    ae(0x1A, 0x00, 0, 0, 0x42, 4, 0, [0x21, 0x21, 0x21]),
    ae(0x1B, 0x41, 40, 0, 0x42, 4, -1, [0x21, 0x21, 0x21]),
    ae(0x1C, 0x00, 0, 0, 0x42, 4, 0, [0x21, 0x21, 0x21]),
    ae(0x20, 0x00, 0, 0, 0x43, 4, 0, [0x04, 0x04, 0x04]),
    ae(0x21, 0x00, 240, 0, 0x43, 2, 0, [0x04, 0x21, 0x21]),
    ae(0x22, 0x00, 0, 15, 0x44, 4, 0, [0x04, 0x04, 0x04]),
    ae(0x23, 0x08, 240, 15, 0x44, 4, 0, [0x04, 0x04, 0x04]),
    ae(0x24, 0x00, 0, 0, 0x45, 4, 0, [0x21, 0x21, 0x21]),
    ae(0x25, 0x02, 240, 0, 0x45, 2, 0, [0x04, 0x21, 0x21]),
    ae(0x28, 0x00, 0, 0, 0x46, 4, 0, [0x02, 0x21, 0x21]),
    ae(0x29, 0x00, 240, 0, 0x46, 2, 0, [0x02, 0x21, 0x21]),
    ae(0x2A, 0x00, 0, 0, 0x47, 4, 0, [0x02, 0x21, 0x21]),
    ae(0x2B, 0x00, 240, 0, 0x47, 2, 0, [0x02, 0x21, 0x21]),
    ae(0x30, 0x00, 0, 0, 0x48, 4, 0, [0x0D, 0x21, 0x21]),
    ae(0x31, 0x08, 240, 12, 0x48, 4, 0, [0x0D, 0x18, 0x0F]),
    ae(0x38, 0x00, 0, 0, 0x49, 4, 0, [0x21, 0x21, 0x21]),
    ae(0x39, 0x00, 0, 0, 0x49, 4, 0, [0x00, 0x21, 0x21]),
    ae(0x3A, 0x41, 240, 0, 0x49, 4, 0, [0x21, 0x21, 0x21]),
    ae(0x3B, 0x00, 240, 0, 0x4A, 2, 0, [0x00, 0x21, 0x21]),
    ae(0x40, 0x00, 0, 0, 0x4B, 4, 0, [0x0B, 0x21, 0x21]),
    ae(0x41, 0x00, 240, 0, 0x4B, 4, 0, [0x0B, 0x0B, 0x0B]),
    ae(0x48, 0x00, 0, 0, 0x4C, 4, 0, [0x0E, 0x21, 0x21]),
    ae(0x49, 0x09, 240, 0, 0x4C, 4, 0, [0x0E, 0x21, 0x21]),
    ae(0x50, 0x00, 0, 0, 0x4E, 4, 0, [0x1B, 0x1C, 0x21]),
    ae(0x51, 0x08, 240, 15, 0x4E, 6, 0, [0x1B, 0x1C, 0x21]),
    ae(0x52, 0x00, 0, 0, 0x4F, 4, 0, [0x08, 0x07, 0x21]),
    ae(0x53, 0x04, 240, 0, 0x4F, 3, 0, [0x08, 0x07, 0x21]),
    ae(0x58, 0x00, 0, 0, 0x50, 4, 0, [0x05, 0x21, 0x21]),
    ae(0x59, 0x02, 240, 0, 0x50, 2, 0, [0x05, 0x21, 0x21]),
    ae(0x5A, 0x00, 0, 0, 0x50, 4, 0, [0x05, 0x21, 0x21]),
    ae(0x5B, 0x00, 0, 0, 0x51, 4, 0, [0x1D, 0x21, 0x21]),
    ae(0x5C, 0x20, 240, 0, 0x51, 2, 0, [0x1D, 0x21, 0x21]),
    ae(0x5D, 0x00, 240, 0, 0x51, 2, 0, [0x1D, 0x21, 0x21]),
    ae(0xFF, 0x00, 0, 0, 0x00, 0, 0, [0x00, 0x00, 0x00]),
];

/// The characters whose copies Kirby's hats load (`ftLib_LoadKirbyHats`): every unlocked one
/// of the 26 (`SELKIND_COUNT` and the one past it).
pub fn kirby_hat_characters(game: &super::super::state::Game) -> Vec<u8> {
    (0..=25u8)
        .filter(|&c| game.gm.unlocks.ckind_unlocked(c))
        .collect()
}

/// How many of a character's fighters Kirby's hats load: Zelda's and Sheik's two.
pub fn hat_fighters(ckind: u8) -> i32 {
    const CKIND_ZELDA: u8 = 0x12;
    const CKIND_SEAK: u8 = 0x13;
    if ckind == CKIND_ZELDA || ckind == CKIND_SEAK {
        2
    } else {
        1
    }
}

/// Each playing slot's fighter kind and second kind (-1 for none), `Player_80031DC8`'s walk.
pub fn playing_kinds(game: &super::super::state::Game) -> Vec<(i32, i32)> {
    (0..6)
        .filter(|&s| game.player_state(s) != 0)
        .map(|s| {
            let m = super::super::pl::player::mapping(game.player_ckind(s));
            (i32::from(m.internal_id), i32::from(m.extra_internal_id))
        })
        .collect()
}

/// A state's entry in Adventure's table, `gm_8017E4C4`.
#[doc(alias = "gm_8017E4C4")]
pub fn stage_entry(id: u8) -> Option<&'static AdventureStage> {
    ADVENTURE_TABLE.iter().find(|e| e.id == id)
}

impl Running<'_> {
    /// Adventure's stage's intro state's entry, `gm_801B3F40`: the intro scene's enter data
    /// (`gm_804D68E0`: the player's port, the stage's number, its stage kind) and the
    /// stage's sound banks: the player's, the allies', the opponents' (Kirby's copies' too)
    /// and the stage's.
    #[doc(alias = "gm_801B3F40")]
    pub fn adventure_enter_intro(&mut self) {
        use crate::lb::audio::{character_banks, stage_banks};
        let id = self.game.machine.routing.curr_state_id;
        let entry = stage_entry(id).expect("an Adventure state's stage");
        let run = &mut self.game.one_player.adventure.base;
        run.player.mode = id;
        let run = *run;
        let d = &mut self.game.one_player.adventure_intro;
        d.stage = (id >> 3) & 0x1F;
        d.stkind = entry.stkind;
        d.slot = run.player.slot;
        let mut mask = character_banks(i32::from(run.player.ckind));
        for a in run.opponents {
            if a.ckind as u8 != NONE {
                mask |= character_banks(i32::from(a.ckind));
            }
        }
        for c in entry.chars {
            if c != NONE {
                mask |= character_banks(i32::from(c));
                if c == CKIND_KIRBY {
                    mask |= (2 << 32) | 0x4000;
                }
            }
        }
        mask |= stage_banks(i32::from(entry.stkind));
        self.game.audio.lb.unwant_banks(0x1C);
        self.game.audio.lb.want_banks(0xC, mask);
        self.game.audio.lb_start_bank_loads(self.data);
    }
}

/// `IntroData` (`gm_804D68E0`): Adventure's intro scene's enter data.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct IntroData {
    pub slot: u8,
    pub stage: u8,
    pub stkind: u16,
}

/// An entry of `lbl_803D7AC0` (`AdventureStageEntry`, 0x1A bytes): the item frequency, the
/// opponents' attack and defense ratios in percent, then three opponents' level, costume and
/// computer kind, and from `x14` two allies' level, computer kind and costume.
#[derive(Clone, Copy, Debug)]
pub struct Level {
    pub items: u8,
    pub attack: u16,
    pub defense: u16,
    pub t: [u8; 20],
}

const fn ae2(items: u8, attack: u16, defense: u16, t: [u8; 20]) -> Level {
    Level {
        items,
        attack,
        defense,
        t,
    }
}

/// `lbl_803D7AC0`: Adventure's 22 stages, five difficulties each (the executable's data).
const LEVELS: [Level; 110] = [
    ae2(
        0x04,
        40,
        450,
        [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x03,
        50,
        400,
        [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x03,
        60,
        320,
        [4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x02,
        70,
        240,
        [5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x01,
        80,
        170,
        [6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x04,
        80,
        140,
        [0, 0, 16, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x03,
        94,
        112,
        [3, 0, 16, 3, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x03,
        100,
        107,
        [5, 0, 16, 4, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x03,
        110,
        105,
        [7, 0, 16, 7, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x02,
        120,
        95,
        [8, 0, 16, 8, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x04,
        80,
        140,
        [1, 0, 4, 1, 1, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x04,
        100,
        90,
        [2, 0, 4, 2, 1, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x03,
        110,
        70,
        [4, 0, 4, 3, 1, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x02,
        120,
        50,
        [7, 0, 4, 6, 1, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x01,
        130,
        40,
        [9, 0, 4, 9, 1, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x04,
        60,
        380,
        [
            1, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 100, 0, 100, 0, 0, 4, 0, 0, 4,
        ],
    ),
    ae2(
        0x04,
        80,
        250,
        [
            3, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 100, 0, 100, 0, 0, 4, 0, 0, 4,
        ],
    ),
    ae2(
        0x03,
        90,
        200,
        [
            5, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 100, 0, 100, 0, 0, 4, 0, 0, 4,
        ],
    ),
    ae2(
        0x02,
        95,
        160,
        [
            7, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 100, 0, 100, 0, 0, 4, 0, 0, 4,
        ],
    ),
    ae2(
        0x01,
        110,
        120,
        [
            8, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 100, 0, 100, 0, 0, 4, 0, 0, 4,
        ],
    ),
    ae2(
        0x03,
        70,
        160,
        [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x02,
        80,
        140,
        [3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x02,
        90,
        120,
        [5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x01,
        100,
        105,
        [7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x01,
        108,
        95,
        [9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x04,
        80,
        125,
        [2, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x04,
        90,
        110,
        [6, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x03,
        100,
        100,
        [8, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x03,
        118,
        95,
        [9, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x02,
        130,
        90,
        [9, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x03,
        80,
        130,
        [2, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x03,
        90,
        110,
        [5, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x02,
        100,
        100,
        [7, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x02,
        115,
        88,
        [8, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x01,
        130,
        74,
        [9, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0xff,
        0,
        0,
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0xff,
        0,
        0,
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0xff,
        0,
        0,
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0xff,
        0,
        0,
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0xff,
        0,
        0,
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x03,
        80,
        170,
        [2, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x02,
        100,
        130,
        [4, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x02,
        120,
        110,
        [6, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x01,
        130,
        100,
        [8, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0xff,
        140,
        90,
        [9, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x03,
        60,
        500,
        [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x02,
        70,
        400,
        [3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x02,
        80,
        300,
        [4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x01,
        90,
        200,
        [6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0xff,
        100,
        175,
        [7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x03,
        70,
        240,
        [2, 0, 27, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x02,
        75,
        180,
        [4, 0, 27, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x02,
        80,
        130,
        [6, 0, 27, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x01,
        100,
        100,
        [7, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x01,
        120,
        90,
        [9, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x02,
        80,
        180,
        [2, 0, 27, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x02,
        90,
        150,
        [4, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x02,
        100,
        130,
        [5, 0, 18, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x02,
        118,
        110,
        [6, 0, 18, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x02,
        130,
        100,
        [7, 0, 18, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x02,
        60,
        210,
        [2, 1, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x02,
        70,
        180,
        [4, 1, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x02,
        90,
        140,
        [6, 1, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x02,
        105,
        120,
        [8, 1, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x02,
        117,
        100,
        [9, 1, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x04,
        60,
        420,
        [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x04,
        78,
        320,
        [4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x04,
        88,
        200,
        [5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x03,
        98,
        170,
        [7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x02,
        109,
        150,
        [8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0xff,
        0,
        0,
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0xff,
        0,
        0,
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0xff,
        0,
        0,
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0xff,
        0,
        0,
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0xff,
        0,
        0,
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x02,
        80,
        150,
        [3, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x02,
        100,
        110,
        [6, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x02,
        110,
        100,
        [8, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x02,
        120,
        90,
        [9, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x02,
        130,
        70,
        [9, 2, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x04,
        70,
        190,
        [
            1, 0, 29, 1, 2, 29, 1, 1, 27, 0, 0, 100, 0, 100, 0, 0, 0, 0, 0, 0,
        ],
    ),
    ae2(
        0x04,
        80,
        150,
        [
            3, 0, 29, 3, 2, 29, 2, 1, 29, 0, 0, 100, 0, 100, 0, 0, 0, 0, 0, 0,
        ],
    ),
    ae2(
        0x04,
        90,
        130,
        [
            5, 0, 29, 4, 2, 29, 4, 1, 29, 0, 0, 100, 0, 100, 0, 0, 0, 0, 0, 0,
        ],
    ),
    ae2(
        0x04,
        100,
        110,
        [
            7, 0, 29, 6, 2, 29, 5, 1, 29, 0, 0, 100, 0, 100, 0, 0, 0, 0, 0, 0,
        ],
    ),
    ae2(
        0x04,
        130,
        100,
        [
            8, 0, 29, 8, 2, 29, 8, 1, 29, 0, 0, 100, 0, 100, 0, 0, 0, 0, 0, 0,
        ],
    ),
    ae2(
        0x02,
        80,
        190,
        [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x02,
        85,
        140,
        [3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x02,
        90,
        130,
        [4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x02,
        95,
        120,
        [6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x02,
        100,
        100,
        [9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x02,
        50,
        420,
        [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x02,
        60,
        370,
        [3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x02,
        70,
        280,
        [5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x02,
        80,
        200,
        [6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x02,
        90,
        150,
        [8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x02,
        70,
        190,
        [1, 0, 27, 1, 0, 27, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x02,
        90,
        150,
        [3, 0, 27, 2, 0, 27, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x02,
        100,
        130,
        [5, 0, 27, 3, 0, 27, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x02,
        110,
        110,
        [6, 0, 27, 5, 0, 27, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x02,
        120,
        90,
        [8, 0, 27, 7, 0, 27, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x03,
        70,
        130,
        [2, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x02,
        93,
        115,
        [4, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x02,
        100,
        105,
        [6, 3, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x01,
        110,
        88,
        [8, 3, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0xff,
        120,
        75,
        [9, 3, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x02,
        1,
        1,
        [0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x02,
        1,
        1,
        [0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x01,
        135,
        58,
        [7, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0x01,
        148,
        50,
        [8, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    ae2(
        0xff,
        160,
        38,
        [9, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
];

/// Adventure's run's `u8` getters (`gm_17E4.c`), by their addresses, for a stage slot,
/// difficulty and index; `None` for another getter.
#[doc(alias = "gm_8017E500")]
#[doc(alias = "gm_8017E5C8")]
#[doc(alias = "gm_8017E5FC")]
#[doc(alias = "gm_8017E630")]
#[doc(alias = "gm_8017E704")]
#[doc(alias = "gm_8017E738")]
#[doc(alias = "gm_8017E76C")]
pub fn getter(getter: CodeRef, stage: u8, difficulty: u8, i: u8) -> Option<u8> {
    let e = &LEVELS[usize::from(difficulty) + usize::from(stage) * 5];
    let i = usize::from(i) * 3;
    Some(match getter.0 {
        0x8017_E500 => e.items,
        0x8017_E5C8 => e.t[i],
        0x8017_E5FC => e.t[i + 1],
        0x8017_E630 => e.t[i + 2],
        0x8017_E704 => e.t[0xE + i],
        0x8017_E738 => e.t[0xF + i],
        0x8017_E76C => e.t[0x10 + i],
        _ => return None,
    })
}

/// Adventure's run's ratio getters' percents: the opponents' attack (`gm_8017E528`) and
/// defense (`gm_8017E578`), the allies' the same two (`gm_8017E664`, `gm_8017E6B4`); `None`
/// for another getter.
#[doc(alias = "gm_8017E528")]
#[doc(alias = "gm_8017E578")]
#[doc(alias = "gm_8017E664")]
#[doc(alias = "gm_8017E6B4")]
pub fn ratio_percent(getter: CodeRef, stage: u8, difficulty: u8) -> Option<u16> {
    let e = &LEVELS[usize::from(difficulty) + usize::from(stage) * 5];
    Some(match getter.0 {
        0x8017_E528 | 0x8017_E664 => e.attack,
        0x8017_E578 | 0x8017_E6B4 => e.defense,
        _ => return None,
    })
}

/// `gm_8017C838`, Adventure's matches' start hook (the stages with the flag 8).
pub const ON_MATCH_START: u32 = 0x8017_C838;
/// `ADVENTURE_GAME_OVER`, `ADVENTURE_LUIGI_CUTSCENE`, `ADVENTURE_MARIO_PEACH_FIGHT`.
const GAME_OVER: u8 = 0x69;
const LUIGI_CUTSCENE: u8 = 0x02;
/// `ADVENTURE_KIRBY_INTRO`, `OUTCOME_TIMEOUT`, and the escape's end hook (`gm_8017E7A0`).
const KIRBY_INTRO: u8 = 0x20;
const OUTCOME_TIMEOUT: u8 = 1;
pub const ESCAPE_END: u32 = 0x8017_E7A0;
/// Bowser's match's end hook, `gm_8017E7FC`.
pub const BOWSER_END: u32 = 0x8017_E7FC;
/// `GM_ADVENTURE`, `ADVENTURE_BOWSERTOY_CUTSCENE`.
const GM_ADVENTURE: u8 = 4;
const BOWSERTOY_CUTSCENE: u8 = 0x5A;
const PEACH_FIGHT: u8 = 0x03;
/// The states whose scene is a match (`GS_VS`), in `gm_Mode_Adventure_States`' order.
const VS_STATES: [u8; 22] = [
    0x01, 0x03, 0x09, 0x0A, 0x11, 0x12, 0x19, 0x1B, 0x21, 0x23, 0x25, 0x29, 0x2B, 0x31, 0x3A, 0x3B,
    0x41, 0x49, 0x51, 0x53, 0x59, 0x5C,
];
/// `FTKIND_*` the start hook loads: Yoshi, Link, Kirby, Pikachu, Pichu, Jigglypuff, the Ice
/// Climbers and the Wire Frames.
const FT_YOSHI: i8 = 0xE;
const FT_LINK: i8 = 6;
const FT_KIRBY: i8 = 4;
const FT_PIKACHU: i8 = 0xC;
const FT_PICHU: i8 = 0x17;
const FT_PURIN: i8 = 0xF;
const FT_POPO: i8 = 0xA;
const FT_NANA: i8 = 0xB;
const FT_BOY: i8 = 0x1D;
const FT_GIRL: i8 = 0x1E;
const FT_NONE: i8 = 0x21;
/// `CKind_Pichu`, `CKind_Purin`.
const CKIND_PICHU: u8 = 0x18;
const CKIND_PURIN: u8 = 0xF;

/// The match's states before `id` (`gm_8017E48C`; the table's ids rise).
#[doc(alias = "gm_8017E48C")]
fn vs_count(id: u8) -> u8 {
    VS_STATES.iter().filter(|&&s| s < id).count() as u8
}

/// The fighter kinds the start hook loads on a stage kind (`gm_8017C838`'s), none past
/// the stage's.
pub fn match_start_kinds(game: &super::super::state::Game, stkind: u16) -> [i8; 3] {
    let mut k = [FT_NONE; 3];
    match stkind {
        0x3B => k[0] = FT_YOSHI,
        0x3F => k[0] = FT_LINK,
        0x44 => k[0] = FT_KIRBY,
        0x48 => {
            k[0] = FT_PIKACHU;
            if game.gm.unlocks.ckind_unlocked(CKIND_PICHU) {
                k[1] = FT_PICHU;
            }
            if game.gm.unlocks.ckind_unlocked(CKIND_PURIN) {
                k[2] = FT_PURIN;
            }
        }
        0x4C => k[..2].copy_from_slice(&[FT_POPO, FT_NANA]),
        0x4E => k[..2].copy_from_slice(&[FT_BOY, FT_GIRL]),
        _ => {}
    }
    k
}

impl Running<'_> {
    /// Adventure's stage's match, `gm_801B4064`: the stage's flags and `x8`, `x9` into the run,
    /// its start data (`gm_8017CE34`) with its time, stage kind, opponents and the color kind
    /// its last try left (`gm_80490910`), the rumble setting, and on a stage with the flag 8
    /// the start hook that loads its joining opponents' fighters (`gm_8017C838`).
    #[doc(alias = "gm_801B4064")]
    pub fn adventure_enter_vs(&mut self) {
        let id = self.game.machine.routing.curr_state_id;
        let (index, entry) = ADVENTURE_TABLE
            .iter()
            .enumerate()
            .find(|(_, e)| e.id == id)
            .expect("an Adventure state's stage");
        let (x4_flag, x5_flag) = if entry.x4 != 0 { (1, 0) } else { (0, 1) };
        let mut run = self.game.one_player.adventure.base;
        run.x8[0] = entry.x1;
        run.x8[1] = 0;
        run.x8[3] = entry.x8;
        run.x8[2] = entry.x9 as u8;
        let color_kind = self.game.one_player.adventure_x910[index];
        let chars = entry.chars.map(|c| c as i8);
        let mut start = self.game.one_player.match_start.clone();
        self.one_player_stage_start(
            &mut start,
            &mut run,
            &chars,
            entry.x4 as u8,
            x4_flag,
            x5_flag,
            i32::from(entry.x2),
            i32::from(entry.stkind),
            i32::from(vs_count(id)),
            i32::from(color_kind),
        );
        self.game.one_player.adventure.base = run;
        let (_, prefs) = super::attract::saved_rules(&self.game.game_data);
        let tags = self.nametag_rumbles();
        super::vs_mode::load_rumble_enabled_with(&mut start, &prefs, &tags);
        if entry.x1 & 8 != 0 {
            start.rules.on_match_start = Some(CodeRef(ON_MATCH_START));
        }
        self.game.one_player.match_start = start;
    }

    /// Adventure's first stage's match's exit, `gm_801B44A0`: the stage marked won (2) or not
    /// (1) in `gm_80490910`, the run's exit (`gm_8017D7AC`, the game over next at no stock),
    /// then Luigi's cutscene next when the time's seconds end in 2, else Peach's Castle.
    #[doc(alias = "gm_801B44A0")]
    pub fn adventure_exit_vs(&mut self) {
        let id = self.game.machine.routing.curr_state_id;
        let index = ADVENTURE_TABLE
            .iter()
            .position(|e| e.id == id)
            .expect("an Adventure state's stage");
        let e = &self.game.one_player.match_exit;
        let (won, time) = (e.x8 != 0, e.x4);
        self.game.one_player.adventure_x910[index] = if won { 2 } else { 1 };
        let mut run = self.game.one_player.adventure.base;
        let on = self.one_player_match_exit(&mut run, GAME_OVER);
        self.game.one_player.adventure.base = run;
        if on {
            let next = if (time % 60) % 10 == 2 {
                LUIGI_CUTSCENE
            } else {
                PEACH_FIGHT
            };
            self.set_next_game_mode_state_id(next);
        }
    }

    /// An Adventure stage's match's exit, `gm_801B4170`: the stage marked won (2) or not (1)
    /// in `gm_80490910`, the run's exit (`gm_8017D7AC`, the game over next at no stock), and
    /// after the table's last stage Adventure's clear (`gm_8017CBAC`).
    #[doc(alias = "gm_801B4170")]
    pub fn adventure_exit_stage(&mut self) {
        let id = self.game.machine.routing.curr_state_id;
        let index = self.adventure_mark_stage(id);
        let mut run = self.game.one_player.adventure.base;
        let on = self.one_player_match_exit(&mut run, GAME_OVER);
        self.game.one_player.adventure.base = run;
        if on && ADVENTURE_TABLE[index + 1].id == 0xFF {
            self.adventure_clear();
        }
    }

    /// `setValUnk`: a stage's mark in `gm_80490910`, 2 won or 1 not, by its state's id; its
    /// table index.
    fn adventure_mark_stage(&mut self, id: u8) -> usize {
        let index = ADVENTURE_TABLE
            .iter()
            .position(|e| e.id == id)
            .expect("an Adventure state's stage");
        let won = self.game.one_player.match_exit.x8 != 0;
        self.game.one_player.adventure_x910[index] = if won { 2 } else { 1 };
        index
    }

    /// Luigi's cutscene's entry, `gm_801B45A4`: Mario's and Luigi's costumes kept apart from
    /// the player's (`gmRegSetupEnemyColorTable`) into the cutscenes' enter data.
    #[doc(alias = "gm_801B45A4")]
    pub fn adventure_enter_luigi(&mut self) {
        let p = self.game.one_player.adventure.base.player;
        let mut colors = [0u8; 3];
        super::one_player::enemy_color_table(
            p.ckind,
            p.color,
            &[CKIND_MARIO as i8, CKIND_LUIGI as i8, NONE as i8],
            &mut colors,
        );
        let d = &mut self.game.one_player.adventure_cutscene;
        d[1] = colors[0];
        d[3] = colors[1];
    }

    /// A cutscene's entry with the player in it, `gm_801B4430`: the player's character (Sheik
    /// for a Zelda the run marks, `xC.x12`) and Adventure's costume into the cutscenes' enter
    /// data.
    #[doc(alias = "gm_801B4430")]
    pub fn adventure_enter_cutscene(&mut self) {
        const CKIND_ZELDA: u8 = 0x12;
        const CKIND_SEAK: u8 = 0x13;
        let run = &self.game.one_player.adventure.base;
        let ckind = run.player.ckind as u8;
        let kind = if ckind == CKIND_ZELDA && run.xc[6] != 0 {
            CKIND_SEAK
        } else {
            ckind
        };
        let color = self.game.game_data.vs.unk_522.color;
        let d = &mut self.game.one_player.adventure_cutscene;
        d[0] = kind;
        d[1] = color;
    }

    /// Brinstar's escape's match, `gm_801B47FC`: the stage's match (`gm_801B4064`) with its
    /// end's hook (`gm_8017E7A0`), and a continue after a game over here resuming at Green
    /// Greens' intro.
    #[doc(alias = "gm_801B47FC")]
    pub fn adventure_enter_escape(&mut self) {
        self.adventure_enter_vs();
        self.game.one_player.match_start.rules.on_match_end = Some(CodeRef(ESCAPE_END));
        self.game.one_player.adventure.base.player.mode = KIRBY_INTRO;
    }

    /// The escape's end, `gm_8017E7A0`: at the time's end the player loses a stock, its
    /// display blown apart and paused.
    #[doc(alias = "gm_8017E7A0")]
    pub fn adventure_escape_end(&mut self, outcome: u8) {
        if outcome == OUTCOME_TIMEOUT {
            self.game.player_lose_stock(0);
            self.game.explode_damage_display(0);
            self.pause_damage_display(0);
        }
    }

    /// Brinstar's escape's exit, `gm_801B4860`: escaped, the run marked (`x76`), else the
    /// stock the time took given back to the match's standings; then the stage's exit, as
    /// `gm_801B4170`'s.
    #[doc(alias = "gm_801B4860")]
    pub fn adventure_exit_escape(&mut self) {
        let e = &mut self.game.one_player.match_exit;
        if e.match_end.outcome != OUTCOME_TIMEOUT {
            self.game.one_player.adventure.x74[2] = 1;
        } else {
            let s = &mut e.match_end.player_standings[0].stocks;
            *s = s.wrapping_add(1);
        }
        self.adventure_exit_stage();
    }

    /// The team Kirby's joiners, `gm_8016A22C` as Adventure calls it (the inline
    /// `gm_8016A22C_inline`): the stage's characters, its count joining at once as many as it
    /// names, one costume for all (the first, 0), Kirby's copies drawn, from the stage point
    /// 1, joining, at the run's level and ratios; the run marked (`xC.x11`).
    fn adventure_joiners(&mut self) {
        let id = self.game.machine.routing.curr_state_id;
        let entry = *stage_entry(id).expect("an Adventure state's stage");
        let count = vs_count(id);
        let run = self.game.one_player.adventure.base;
        let level = run.player.cpu_level;
        let cpu_level = Self::one_player_getter(run.getters[1], count, level, 0);
        let attack = Self::one_player_ratio(run.getters[7], count, level);
        let defense = Self::one_player_ratio(run.getters[8], count, level);
        let kinds = entry.chars.map(|c| c as i8);
        // `gm_8017BE8C`: how many of the three it names.
        let at_once = kinds.iter().filter(|&&k| k != NONE as i8).count() as i32;
        self.setup_opponents(
            &kinds,
            [0; 3],
            1,
            0,
            1,
            run.player.ckind as u8,
            run.player.color,
            i32::from(cpu_level),
            i32::from(entry.x4),
            at_once,
            1,
            0,
            1,
            attack,
            defense,
        );
        self.game.one_player.adventure.base.xc[5] = 1;
    }

    /// The three Kirbys' cutscene's entry, `gm_801B4974`: the player's character and costume
    /// as `gm_801B4430` gives them, the team Kirby's joiners set up, and the cutscene's
    /// Kirbys: the first joiner's costume and three of their copies (`gm_80169540`'s list).
    #[doc(alias = "gm_801B4974")]
    #[doc(alias = "gm_8017BE8C")]
    #[doc(alias = "gm_80169540")]
    pub fn adventure_enter_kirbys(&mut self) {
        self.adventure_enter_cutscene();
        let id = self.game.machine.routing.curr_state_id;
        let n = usize::from(stage_entry(id).expect("an Adventure state's stage").x4);
        self.adventure_joiners();
        let o = &self.game.gm.opponents;
        let (first, copies) = (o.costumes[0], o.copies);
        let d = &mut self.game.one_player.adventure_cutscene;
        d[3] = first as u8;
        d[4] = copies[n - 2] as u8;
        d[5] = copies[n - 1] as u8;
        d[6] = copies[n - 3] as u8;
    }

    /// The team Kirby's match, `gm_801B4B28`: its joiners set up unless the cutscene did
    /// (`xC.x11`), then the stage's match (`gm_801B4064`).
    #[doc(alias = "gm_801B4B28")]
    pub fn adventure_enter_team_kirby(&mut self) {
        if self.game.one_player.adventure.base.xc[5] != 1 {
            self.adventure_joiners();
        }
        self.adventure_enter_vs();
    }

    /// The team Kirby's exit, `gm_801B4C5C`: the stage marked, the run's exit, and past 30
    /// seconds the giant Kirby skipped for Corneria's intro.
    #[doc(alias = "gm_801B4C5C")]
    pub fn adventure_exit_team_kirby(&mut self) {
        let id = self.game.machine.routing.curr_state_id;
        self.adventure_mark_stage(id);
        let frames = self.game.one_player.match_exit.match_end.frame_count;
        let mut run = self.game.one_player.adventure.base;
        let on = self.one_player_match_exit(&mut run, GAME_OVER);
        self.game.one_player.adventure.base = run;
        if on && frames / 60 > 30 {
            // `NEXT_SCENEBLOCK_AFTER(ADVENTURE_GIANTKIRBY_FIGHT)`.
            self.set_next_game_mode_state_id(0x28);
        }
    }

    /// Star Fox's cutscene's entry, `gm_801B4D34`: the next match's opponent (`x7C`), Falco
    /// about a third of the time once he's unlocked (`HSD_Randi(100) <= 33`), else the
    /// stage's.
    #[doc(alias = "gm_801B4D34")]
    pub fn adventure_enter_star_fox(&mut self) {
        let id = self.game.machine.routing.curr_state_id;
        let first = stage_entry(id).expect("an Adventure state's stage").chars[0];
        let falco =
            self.game.gm.unlocks.ckind_unlocked(CKIND_FALCO) && self.game.objs.rng.below(100) <= 33;
        self.game.one_player.adventure.x7c = if falco { CKIND_FALCO } else { first };
    }

    /// Fox's and Falco's match, `gm_801B4DAC`: the stage's match (`gm_801B4064`), with Falco
    /// for its opponent when the cutscene drew him, in a costume kept apart from the player's.
    #[doc(alias = "gm_801B4DAC")]
    pub fn adventure_enter_fox_falco(&mut self) {
        self.adventure_enter_vs();
        if self.game.one_player.adventure.x7c == CKIND_FALCO {
            let p = self.game.one_player.adventure.base.player;
            let mut colors = [self.game.one_player.match_start.players[1].color, 0, 0];
            super::one_player::enemy_color_table(
                p.ckind,
                p.color,
                &[CKIND_FALCO as i8, NONE as i8, NONE as i8],
                &mut colors,
            );
            let s = &mut self.game.one_player.match_start.players[1];
            s.ckind = CKIND_FALCO as _;
            s.color = colors[0];
        }
    }

    /// Ness's match on Onett, `gm_801B4E58`: the stage's match (`gm_801B4064`) with one item
    /// kind on (the mask's bit 7) at frequency 4.
    #[doc(alias = "gm_801B4E58")]
    pub fn adventure_enter_ness(&mut self) {
        self.adventure_enter_vs();
        let rules = &mut self.game.one_player.match_start.rules;
        rules.x20 = 1 << 7;
        rules.item_freq = 4;
    }

    /// Metal Mario's (and Luigi's) match, `gm_801B4EB8`: the stage's match (`gm_801B4064`),
    /// with Luigi's slots left empty while Luigi is locked.
    #[doc(alias = "gm_801B4EB8")]
    pub fn adventure_enter_metal(&mut self) {
        self.adventure_enter_vs();
        if !self.game.gm.unlocks.ckind_unlocked(CKIND_LUIGI) {
            for p in &mut self.game.one_player.match_start.players[1..4] {
                if p.ckind == CKIND_LUIGI as _ {
                    p.slot_type = crate::game::pl::player::PKIND_NONE as u8;
                }
            }
        }
    }

    /// Bowser's match on Final Destination, `gm_801B4F44`: the stage's match (`gm_801B4064`),
    /// the computers at 1.4 times their size, and its end deciding Giga Bowser
    /// ([`BOWSER_END`]).
    #[doc(alias = "gm_801B4F44")]
    pub fn adventure_enter_bowser(&mut self) {
        self.adventure_enter_vs();
        let start = &mut self.game.one_player.match_start;
        for p in &mut start.players[1..4] {
            if i32::from(p.slot_type) == crate::game::pl::player::PKIND_CPU {
                p.model_scale = 1.4;
            }
        }
        start.rules.on_match_end = Some(CodeRef(BOWSER_END));
    }

    /// Bowser's match's exit, `gm_801B4FCC`: the stage marked and the run's exit, without
    /// `gm_801B4170`'s clear.
    #[doc(alias = "gm_801B4FCC")]
    pub fn adventure_exit_bowser(&mut self) {
        let id = self.game.machine.routing.curr_state_id;
        self.adventure_mark_stage(id);
        let mut run = self.game.one_player.adventure.base;
        self.one_player_match_exit(&mut run, GAME_OVER);
        self.game.one_player.adventure.base = run;
    }

    /// Bowser's match's end, `gm_8017E7FC`: Giga Bowser comes (`x77`) in Adventure on Normal
    /// or harder with the run's time and the match's under 0xFD20 frames (18 minutes); else
    /// the run's match ends as played through (`x4_5`). Either way the toy's cutscene is next.
    #[doc(alias = "gm_8017E7FC")]
    pub fn adventure_bowser_end(&mut self, _outcome: u8) {
        let run = &self.game.one_player.adventure;
        let giga = self.game.machine.routing.curr_mode == GM_ADVENTURE
            && run.base.player.cpu_level >= 2
            && run.base.x20.wrapping_add(self.game.vs_frame_count()) < 0xFD20;
        if !giga {
            self.game.vs.start.x4_5 = true;
        }
        self.game.one_player.adventure.x74[3] = u8::from(giga);
        self.set_next_game_mode_state_id(BOWSERTOY_CUTSCENE);
    }

    /// Adventure's clear, `gm_8017CBAC` with Adventure's run and stocks (`gmMainLib_8015CDD4`)
    /// into the ending (mode 0x16).
    fn adventure_clear(&mut self) {
        let run = self.game.one_player.adventure.base;
        let stocks = self.game.game_data.vs.unk_522.stocks;
        self.one_player_clear(&run, stocks, super::endings::GM_ADVENTURE_GOVER);
    }

    /// Bowser's toy's cutscene's exit, `gm_801B5078`: without Giga Bowser (`x77`), Adventure's
    /// clear.
    #[doc(alias = "gm_801B5078")]
    pub fn adventure_exit_toy(&mut self) {
        if self.game.one_player.adventure.x74[3] == 0 {
            self.adventure_clear();
        }
    }

    /// Giga Bowser's match's exit, `gm_801B50C4`: no stock lost in it marked (`x78`), then
    /// the stage marked and the run's exit.
    #[doc(alias = "gm_801B50C4")]
    pub fn adventure_exit_giga(&mut self) {
        let run = &self.game.one_player.adventure.base;
        let e = &self.game.one_player.match_exit;
        if i32::from(run.player.stocks) == i32::from(e.match_end.player_standings[0].stocks) {
            self.game.one_player.adventure.x74[4] = 1;
        }
        let id = self.game.machine.routing.curr_state_id;
        self.adventure_mark_stage(id);
        let mut run = self.game.one_player.adventure.base;
        self.one_player_match_exit(&mut run, GAME_OVER);
        self.game.one_player.adventure.base = run;
    }

    /// Giga Bowser's defeat's cutscene's exit, `gm_801B518C`: Adventure's clear.
    #[doc(alias = "gm_801B518C")]
    pub fn adventure_exit_giga_defeated(&mut self) {
        self.adventure_clear();
    }

    /// Peach's Castle's match, `gm_801B461C`: the stage's match (`gm_801B4064`), and after
    /// Luigi's cutscene Luigi in Mario's place in his costume, the run marked (`x74`).
    #[doc(alias = "gm_801B461C")]
    pub fn adventure_enter_peach(&mut self) {
        self.adventure_enter_vs();
        if self.previous_scene_index() == LUIGI_CUTSCENE {
            let color = self.game.one_player.adventure_cutscene[3];
            let p = &mut self.game.one_player.match_start.players[1];
            p.ckind = CKIND_LUIGI as _;
            p.color = color;
            self.game.one_player.adventure.x74[0] = 1;
        }
    }

    /// The state the mode's machine left last, `gm_GetPreviousSceneIndex`.
    #[doc(alias = "gm_GetPreviousSceneIndex")]
    fn previous_scene_index(&self) -> u8 {
        self.game.machine.routing.prev_state_id
    }

    /// Peach's Castle's match's exit, `gm_801B4684`: the stage marked, the run's exit, and
    /// Luigi's unlock's condition (`x75`): the match won within a minute.
    #[doc(alias = "gm_801B4684")]
    pub fn adventure_exit_peach(&mut self) {
        let id = self.game.machine.routing.curr_state_id;
        self.adventure_mark_stage(id);
        let frames = self.game.one_player.match_exit.match_end.frame_count;
        let mut run = self.game.one_player.adventure.base;
        let on = self.one_player_match_exit(&mut run, GAME_OVER);
        self.game.one_player.adventure.base = run;
        if on {
            self.game.one_player.adventure.x74[1] = u8::from(frames / 60 <= 60);
        }
    }

    /// Kongo Jungle's first match, `gm_801B4768`: the stage's match (`gm_801B4064`) with the
    /// computers at half their size.
    #[doc(alias = "gm_801B4768")]
    pub fn adventure_enter_tiny(&mut self) {
        self.adventure_enter_vs();
        for p in &mut self.game.one_player.match_start.players[1..4] {
            if p.slot_type == 1 {
                p.model_scale =
                    crate::fpu::mem(gekko_fp::fmuls(crate::fpu::reg(p.model_scale), 0.5));
            }
        }
    }

    /// `gm_8017C838`, the start hook: every costume of the stage's joining opponents'
    /// fighters loaded (`ftLib_LoadFighterAllCostumes`), Kirby's hats on Green Greens, and the
    /// camera's quakes at a fifth.
    #[doc(alias = "gm_8017C838")]
    pub fn adventure_match_start(&mut self) {
        let stkind = self.game.vs.start.stkind;
        if stkind == 0x44 {
            let p = self.game.one_player.adventure.base.player;
            let costume = u8::from(p.ckind as u8 == CKIND_KIRBY && p.color == 0);
            self.load_kirby_hats(costume);
        }
        for k in match_start_kinds(self.game, stkind) {
            if k != FT_NONE {
                self.load_fighter_all_costumes(i32::from(k));
            }
        }
        super::super::camera::set_quake_scale(&mut self.game.camera.cam, 0.2);
    }

    /// `ftLib_LoadFighterAllCostumes`: a kind's data, its effects' file, each of its
    /// costumes and its animations loaded unless they are (and Kirby's copies' setup,
    /// `ftData_800857E0`).
    #[doc(alias = "ftLib_LoadFighterAllCostumes")]
    pub fn load_fighter_all_costumes(&mut self, kind: i32) {
        use super::super::ft::data::{MAIN_MEMORY, kind_tables};
        if let Some(file) = self.game.load_fighter_data(self.data, kind) {
            self.check_preloaded(file);
            self.disc_load(file, MAIN_MEMORY);
        }
        let t = kind_tables(kind);
        super::super::ef::load_sync(self, i32::from(t.effect_file));
        for c in 0..i32::from(t.costume_count) {
            if let Some(file) = self.game.load_costume(self.data, kind, c) {
                self.check_preloaded(&file);
                self.disc_load(&file, MAIN_MEMORY);
            }
        }
        self.put_anim_file(kind);
        if kind == super::super::ft::kirby::KIND_KIRBY {
            self.kirby_data_setup();
        }
    }

    /// Kirby's hats for Green Greens, `ftLib_LoadKirbyHats`: every unlocked character's copy
    /// (Zelda's and Sheik's both fighters') in Kirby's costume `costume`.
    #[doc(alias = "ftLib_LoadKirbyHats")]
    pub fn load_kirby_hats(&mut self, costume: u8) {
        for ckind in kirby_hat_characters(self.game) {
            for sub in 0..hat_fighters(ckind) {
                let kind = super::super::pl::player::ckind_ftkind(i32::from(ckind), sub);
                self.kb_load_copy(kind, i32::from(costume));
            }
        }
    }

    /// Kirby's kind's data setup, `ftData_800857E0` (`ftKb_Init_UnkMotionStates5`,
    /// `Player_80031DC8`): each playing slot's fighters' copies in Kirby's first costume.
    #[doc(alias = "ftData_800857E0")]
    #[doc(alias = "ftKb_Init_UnkMotionStates5")]
    // Ports Player_80031DC8; aliased once a check reaches it.
    fn kirby_data_setup(&mut self) {
        for (kind, extra) in playing_kinds(self.game) {
            self.kb_load_copy(kind, 0);
            if extra != -1 {
                self.kb_load_copy(extra, 0);
            }
        }
    }
}
