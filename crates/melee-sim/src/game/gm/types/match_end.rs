//! What a match's end leaves for the results (`MatchEnd`): the outcome, the winners, each
//! team's and player's standing and statistics, and the decisions (the results' bonuses) each
//! player earned.

/// The decomp's `MatchTeamData`.
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "MatchTeamData", size = 0xC, pad(0xB, 1))]
pub struct MatchTeamData {
    #[at(0x0)]
    pub score: i32,
    #[at(0x4)]
    pub subscore: i32,
    #[at(0x8)]
    pub is_big_loser: u8,
    #[at(0x9)]
    pub is_small_loser: u8,
    #[at(0xA)]
    pub active: u8,
}

/// The decomp's `MatchPlayerData`.
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "MatchPlayerData", size = 0xA8, pad(0x1A, 2))]
pub struct MatchPlayerData {
    #[at(0x0)]
    pub pkind: u8,
    #[at(0x1)]
    pub ckind: i8,
    #[at(0x2)]
    pub ftkind: i8,
    /// The costume (the top six bits), the rumble and the stamina flags (bits 1 and 0).
    #[at(0x3, union)]
    #[doc(alias("x3_u"))]
    pub flags: [u8; 1],
    /// The name tag (0x78: none).
    #[at(0x4)]
    #[doc(alias("x4"))]
    pub nametag: u8,
    #[at(0x5)]
    pub is_big_loser: u8,
    #[at(0x6)]
    pub is_small_loser: u8,
    #[at(0x7)]
    pub team: u8,
    #[at(0x8)]
    pub stocks: i8,
    #[at(0x9)]
    #[doc(alias("x9"))]
    pub remaining_hp: u8,
    #[at(0xA)]
    pub self_destructs: u16,
    #[at(0xC)]
    pub percent: u16,
    #[at(0xE)]
    #[doc(alias("xE"))]
    pub coins: u16,
    #[at(0x10)]
    pub kills: [u16; 4],
    /// KOs of the players past the four `kills` count.
    #[at(0x18)]
    #[doc(alias("x18"))]
    pub kills_rest: u16,
    #[at(0x1C)]
    #[doc(alias("x1C"))]
    pub coins_held: i32,
    #[at(0x20)]
    #[doc(alias("x20"))]
    pub kos: u32,
    #[at(0x24)]
    #[doc(alias("x24"))]
    pub falls: i32,
    #[at(0x28)]
    #[doc(alias("x28"))]
    pub match_frames: u32,
    #[at(0x2C)]
    pub score: i32,
    #[at(0x30)]
    #[doc(alias("x30"))]
    pub sub_rank: u32,
    /// Hits over attacks, in percent.
    #[at(0x34)]
    #[doc(alias("x34"))]
    pub hit_percent: u8,
    #[at(0x35)]
    #[doc(alias("x35_pad"))]
    pub pad_35: [u8; 3],
    #[at(0x38)]
    #[doc(alias("x38"))]
    pub attacks_hit: u32,
    #[at(0x3C)]
    #[doc(alias("x3C"))]
    pub attacks_total: u32,
    #[at(0x40)]
    #[doc(alias("x40"))]
    pub damage_dealt: u32,
    #[at(0x44)]
    #[doc(alias("x44"))]
    pub damage_taken: u32,
    #[at(0x48)]
    #[doc(alias("x48"))]
    pub damage_recovered: u32,
    #[at(0x4C)]
    #[doc(alias("x4C"))]
    pub peak_damage: u32,
    #[at(0x50)]
    #[doc(alias("x50"))]
    pub walk_distance: u32,
    /// Ten times the distance risen in the air (the player's stats' `rose`; wave 5 named it a
    /// run's).
    #[at(0x54)]
    #[doc(alias("x54"))]
    #[doc(alias("run_distance"))]
    pub rise_distance: u32,
    #[at(0x58)]
    #[doc(alias("x58"))]
    pub fall_distance: u32,
    /// Ten times the distance knockback carried the player (the player's stats' `knocked`).
    #[at(0x5C)]
    #[doc(alias("x5C"))]
    #[doc(alias("peak_height"))]
    pub knockback_distance: u32,
    #[at(0x60)]
    #[doc(alias("x60"))]
    pub coins_collected: u32,
    #[at(0x64)]
    #[doc(alias("x64"))]
    pub coins_swiped: u32,
    #[at(0x68)]
    #[doc(alias("x68"))]
    pub coins_lost: u32,
    /// Seconds on the ground, from the player's stats' timers (`pl_80040CB4`, `pl_80040CD8`).
    #[at(0x6C)]
    #[doc(alias("x6C"))]
    #[doc(alias("air_time"))]
    pub ground_time: u32,
    /// Seconds in the air.
    #[at(0x70)]
    #[doc(alias("x70"))]
    #[doc(alias("ground_time"))]
    pub air_time: u32,
    #[at(0x74)]
    #[doc(alias("x74"))]
    pub ground_attacks: u32,
    #[at(0x78)]
    #[doc(alias("x78"))]
    pub air_attacks: u32,
    #[at(0x7C)]
    #[doc(alias("x7C"))]
    pub items_thrown: u32,
    #[at(0x80)]
    #[doc(alias("x80"))]
    pub specials: u32,
    #[at(0x84)]
    #[doc(alias("x84"))]
    pub grabs: u32,
    #[at(0x88)]
    #[doc(alias("x88"))]
    pub down_attacks: u32,
    /// The action stats' ledge catches (id 112, `pl_800386D8(stats, 112)`: `more_actions[4]`).
    #[at(0x8C)]
    #[doc(alias("x8C"))]
    pub ledge_catches: u32,
    /// Items picked up, every kind's.
    #[at(0x90)]
    #[doc(alias("x90"))]
    pub items_picked_up: u32,
    /// Two more of the stats' timers, in tenths of frames (`pl_80040D44`, `pl_80040D68`), and two
    /// in seconds (`pl_80040CFC`, `pl_80040D20`).
    #[at(0x94)]
    #[doc(alias("x94"))]
    pub timed_stat_1: u32,
    #[at(0x98)]
    #[doc(alias("x98"))]
    pub timed_stat_2: u32,
    #[at(0x9C)]
    #[doc(alias("x9C"))]
    pub timed_stat_3: i32,
    #[at(0xA0)]
    #[doc(alias("xA0"))]
    pub timed_stat_4: i32,
    /// The stale-move table's `teammate_hits` (`pl_80040DB8`).
    #[at(0xA4)]
    #[doc(alias("xA4"))]
    pub teammate_hits: u32,
}

/// The decomp's `UnkResultPlayerData`.
#[doc(alias("UnkResultPlayerData"))]
#[derive(
    melee_derive::CloneFrom, Debug, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "UnkResultPlayerData", size = 0x508, pad(0x101, 3))]
pub struct PlayerDecisions {
    #[at(0x0)]
    #[doc(alias("x0"))]
    pub earned: [u8; 257],
    #[at(0x104)]
    #[doc(alias("x104"))]
    pub points: [i32; 257],
}

impl Default for PlayerDecisions {
    fn default() -> Self {
        PlayerDecisions {
            earned: [0; 257],
            points: [0; 257],
        }
    }
}

/// The decomp's `MatchEnd`.
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "MatchEnd", size = 0x227C, pad(0x1B, 1))]
pub struct MatchEnd {
    /// The scene tick the standings were last worked out (`gm_8016C75C`).
    #[at(0x0)]
    #[doc(alias("x0"))]
    pub standings_tick: u32,
    #[at(0x4)]
    pub outcome: u8,
    #[at(0x5)]
    pub match_kind: u8,
    #[at(0x6)]
    pub is_teams: u8,
    #[at(0x7)]
    #[doc(alias("x7"))]
    pub joiners: u8,
    #[at(0x8)]
    pub frame_count: u32,
    #[at(0xC)]
    pub sd_penalty: i8,
    #[at(0xD)]
    pub n_winners: u8,
    #[at(0xE)]
    pub n_team_winners: u8,
    #[at(0xF)]
    pub loser: u8,
    #[at(0x10)]
    pub winners: [u8; 6],
    #[at(0x16)]
    pub team_winners: [u8; 5],
    #[at(0x1C)]
    pub team_standings: [MatchTeamData; 5],
    #[at(0x58)]
    pub player_standings: [MatchPlayerData; 6],
    /// A single-player mode's bonuses' bits (Classic's, Adventure's, All-Star's results).
    #[at(0x448)]
    #[doc(alias("_x448"))]
    pub mode_bonuses: [u8; 4],
    /// Each player's decisions: earned and their points (`fn_8016C4F4`).
    #[at(0x44C)]
    #[doc(alias("x44C"))]
    pub decisions: [PlayerDecisions; 6],
}
