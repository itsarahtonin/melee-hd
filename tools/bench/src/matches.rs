//! The benchmark matches and the Gecko codes that boot the game straight into each.
//!
//! Each match is a VS match's settings (`StartMeleeData`, the block Slippi records as a replay's
//! game info) and random seed. `melee-bench codes DIR` writes a Dolphin game settings file per
//! match (`DIR/NAME.ini`), whose codes take the game from boot to the match as Slippi's playback
//! does (into DebugMelee, through its result screen to the match), with the match's settings and
//! seed set where Slippi's playback restores a replay's, and no EXI device. ssbm-rs's runner
//! applies the same file (`BENCH_CODES`) to record the match; see `docs/performance.md`.

use std::fmt::Write as _;

use crate::gecko_words::{INFO, INFO_DATA, SCENE, SCENE_DATA};

/// A fighter: external character ID (`CharacterKind`) and costume.
#[derive(Clone, Copy)]
pub struct Player {
    pub character: u8,
    pub costume: u8,
}

pub struct Match {
    pub name: &'static str,
    /// VS stage ID (`stkind`).
    pub stage: u16,
    pub players: [Player; 2],
    pub seed: u32,
}

const FOX: u8 = 0x02;
const MARTH: u8 = 0x09;
const PEACH: u8 = 0x0C;
const ICE_CLIMBERS: u8 = 0x0E;
const POKEMON_STADIUM: u16 = 0x03;
const BATTLEFIELD: u16 = 0x1F;
const FINAL_DESTINATION: u16 = 0x20;

const fn p(character: u8, costume: u8) -> Player {
    Player { character, costume }
}

/// The benchmark set: the performance goal's matches, and Fox–Fox for while only Fox is ported.
pub const MATCHES: &[Match] = &[
    Match {
        name: "fox-fox-fd",
        stage: FINAL_DESTINATION,
        players: [p(FOX, 0), p(FOX, 1)],
        seed: 0x1F0F_0001,
    },
    Match {
        name: "fox-marth-bf",
        stage: BATTLEFIELD,
        players: [p(FOX, 0), p(MARTH, 0)],
        seed: 0x1F0F_0002,
    },
    Match {
        name: "fox-marth-fd",
        stage: FINAL_DESTINATION,
        players: [p(FOX, 0), p(MARTH, 0)],
        seed: 0x1F0F_0003,
    },
    Match {
        name: "fox-marth-ps",
        stage: POKEMON_STADIUM,
        players: [p(FOX, 0), p(MARTH, 0)],
        seed: 0x1F0F_0004,
    },
    // Nana runs CPU code, and the CPU's DI reads a byte of her heap address when her knockback
    // is zero: in Dolphin, melee-bench pins it to the recording's (its DI patch; see
    // docs/performance.md).
    Match {
        name: "ics-peach-ps",
        stage: POKEMON_STADIUM,
        players: [p(ICE_CLIMBERS, 0), p(PEACH, 0)],
        seed: 0x1F0F_0005,
    },
];

const NESS: u8 = 0x0B;

/// Matches the CPU AI plays for the oracle (`docs/oracle.md`), not timed: characters no replay
/// the ruleset knows has, against ported ones on ported stages.
pub const ORACLE_MATCHES: &[Match] = &[
    Match {
        name: "ness-fox-bf",
        stage: BATTLEFIELD,
        players: [p(NESS, 0), p(FOX, 0)],
        seed: 0x1F0F_0101,
    },
    Match {
        name: "ness-marth-fd",
        stage: FINAL_DESTINATION,
        players: [p(NESS, 0), p(MARTH, 0)],
        seed: 0x1F0F_0102,
    },
    Match {
        name: "ness-fox-ps",
        stage: POKEMON_STADIUM,
        players: [p(NESS, 0), p(FOX, 1)],
        seed: 0x1F0F_0103,
    },
];

/// Settings as a Slippi Dolphin VS match records them (stock mode, 4 stocks, 8 minutes, items
/// off, pausing off, two players in slots 1 and 2, the rest empty), from a recorded replay.
const RULES: [u8; 0x60] = {
    let mut r = [0u8; 0x60];
    let head: [u8; 0x3C] = [
        0x32, 0x01, 0x8E, 0x4C, 0xC3, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xFF, 0x6E, 0x00,
        0x20, 0x00, 0x00, 0x01, 0xE0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x3F,
        0x80, 0x00, 0x00, 0x3F, 0x80, 0x00, 0x00, 0x3F, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    let mut i = 0;
    while i < head.len() {
        r[i] = head[i];
        i += 1;
    }
    r
};

/// One player's block (`PlayerInitData`): a human with 4 stocks, VS CPU kind 4 at level 9 (what
/// the CPU AI that plays the controller takes), handicap 9, no name tag; or an empty slot.
fn player_block(player: Option<Player>, slot: u8) -> [u8; 0x24] {
    let one = [0x3F, 0x80, 0x00, 0x00];
    let mut b = [0u8; 0x24];
    match player {
        Some(pl) => {
            b[..0x10].copy_from_slice(&[
                pl.character,
                0x00,
                0x04,
                pl.costume,
                0x00,
                slot,
                0x00,
                0x00,
                0x09,
                u8::from(slot == 0),
                0x78,
                0x00,
                0xC0,
                0x00,
                0x04,
                0x09,
            ]);
        }
        None => {
            let ckind = if slot < 4 { 0x15 } else { 0x21 };
            let flags = if slot < 4 { 0xC0 } else { 0x40 };
            b[..0x10].copy_from_slice(&[
                ckind, 0x03, 0x04, 0x00, 0x00, 0xFF, 0x00, 0x00, 0x09, 0x00, 0x78, 0x00, flags,
                0x00, 0x04, 0x00,
            ]);
        }
    }
    b[0x18..0x1C].copy_from_slice(&one);
    b[0x1C..0x20].copy_from_slice(&one);
    b[0x20..0x24].copy_from_slice(&one);
    b
}

impl Match {
    /// The match's `StartMeleeData` (0x138 bytes).
    pub fn start_data(&self) -> Vec<u8> {
        let mut d = RULES.to_vec();
        d[0xE..0x10].copy_from_slice(&self.stage.to_be_bytes());
        for slot in 0..6u8 {
            let player = self.players.get(usize::from(slot)).copied();
            d.extend_from_slice(&player_block(player, slot));
        }
        d
    }

    /// The Gecko codes, as a Dolphin game settings file.
    pub fn ini(&self) -> String {
        let data = self.start_data();
        let mut scene = SCENE.to_vec();
        splice(&mut scene, SCENE_DATA, &data);
        let mut info = INFO.to_vec();
        let mut with_seed = data.clone();
        with_seed.extend_from_slice(&self.seed.to_be_bytes());
        splice(&mut info, INFO_DATA, &with_seed);

        let mut s = String::new();
        let _ = writeln!(
            s,
            "# melee-bench: boots into the benchmark match {}.",
            self.name
        );
        let _ = writeln!(
            s,
            "# Written by `melee-bench codes`; see melee-hd's docs/performance.md."
        );
        s += "[Gecko]\n";
        s += "$melee-bench boot\n";
        s += "*Into DebugMelee's result screen at boot, as Slippi's playback does (Boot to Playback Scene).\n";
        s += BOOT_SCENE;
        s += "*The result screen loads the match's files and moves on to the match (tools/bench/gecko/scene.s).\n";
        push_c2(&mut s, 0x801A_6348, &scene);
        // Slippi's set also has "ScenePrep_DebugResult = ScenePrep_DebugMenu" (041B16A8
        // 801B09C0), but Dolphin's INI reader drops a code line with '=' in its comment, so
        // playback never applies it; applied, it breaks the result screen's entry.
        s += "*The result screen runs as a menu scene (Slippi's Swap MinorType).\n";
        s += "043DDA9C 07000000\n";
        s += "*StartMelee takes the match's settings and seed (tools/bench/gecko/info.s).\n";
        push_c2(&mut s, 0x8016_E748, &info);
        s += "$melee-bench gameplay\n";
        s += "*Slippi's recording setup that changes the game: Init Player Data and Init Stage Data\n";
        s += "*(tagged affects-gameplay) and the larger player block (ExtendPlayerBlock).\n";
        s += SLIPPI_GAMEPLAY;
        s += "[Gecko_Enabled]\n";
        s += "$melee-bench boot\n";
        s += "$melee-bench gameplay\n";
        s
    }
}

/// Slippi's Boot to Playback Scene (Playback/Core/Scene, slippi-ssbm-asm, GPL-3.0).
const BOOT_SCENE: &str = "\
C21A45A0 00000007
48000019 7C6802A6
3C80803D 6084ADA8
90640004 4800001C
4E800021 38600003
3C808047 60849D30
98640003 4E800020
3800000E 00000000
";

/// Slippi's Init Player Data, Init Stage Data (Common/) and ExtendPlayerBlock (Recording/),
/// from slippi-ssbm-asm (GPL-3.0).
const SLIPPI_GAMEPLAY: &str = "\
C2068EEC 00000006
3BC30000 3C808045
60848FD0 80840020
3D808000 618CC160
7D8903A6 4E800421
7FC3F378 3C808046
60000000 00000000
C21C154C 00000004
38800204 3D808000
618CC160 7D8903A6
4E800421 281A0000
60000000 00000000
040679BC 38802600
";

fn splice(words: &mut [u32], at: usize, data: &[u8]) {
    for (i, chunk) in data.chunks(4).enumerate() {
        let mut w = [0u8; 4];
        w[..chunk.len()].copy_from_slice(chunk);
        words[at + i] = u32::from_be_bytes(w);
    }
}

/// A `C2` code: `words` at `addr`, then the branch back the code handler fills in.
fn push_c2(s: &mut String, addr: u32, words: &[u32]) {
    let mut body = words.to_vec();
    if body.len().is_multiple_of(2) {
        body.push(0x6000_0000);
    }
    body.push(0);
    let _ = writeln!(s, "C2{:06X} {:08X}", addr & 0x01FF_FFFF, body.len() / 2);
    for pair in body.chunks(2) {
        let _ = writeln!(s, "{:08X} {:08X}", pair[0], pair[1]);
    }
}
