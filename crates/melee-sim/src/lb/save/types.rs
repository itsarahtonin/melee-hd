//! The card's data (`GmCardData`), hand-written ([design call 3's second
//! half](../../../../docs/refactor.md#design-call-3s-second-half-the-owned-structs)): the save
//! data (`GmSaveData`: the unlocks, the records the unlocks and trophies count, the totals, the
//! preferences, the trophies, each fighter's records) and the name tags' banks
//! (`NameTagData`), which a save writes to the card (`#[mirror(export)]`) and a load reads back.
//! Each field's `#[at]` is its place in the original's struct, which the comparer's mirror
//! reads and writes back (`#[derive(Mirror)]`) and the layout check holds to the decomp's
//! (`game::mirror`); the fields keep the original's order, so the sync test's hash reads them
//! as the generated structs' did.

/// The decomp's `gmm_retval_ED98`.
#[doc(alias("gmm_retval_ED98"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "gmm_retval_ED98", size = 0x20, export)]
pub struct ChallengerRecords {
    #[at(0x0)]
    #[doc(alias("x0"))]
    pub vs_matches: u32,
    #[at(0x4)]
    #[doc(alias("x4"))]
    pub combined_vs_play_time: u32,
    #[at(0x8)]
    pub padding: [u8; 4],
    /// Bits by character: played; Classic, Adventure, All-Star and Target Test cleared.
    #[at(0xC)]
    #[doc(alias("xC"))]
    pub played: i32,
    #[at(0x10)]
    #[doc(alias("x10"))]
    pub classic_cleared: i32,
    #[at(0x14)]
    #[doc(alias("x14"))]
    pub adventure_cleared: i32,
    #[at(0x18)]
    #[doc(alias("x18"))]
    pub all_star_cleared: i32,
    #[at(0x1C)]
    #[doc(alias("x1C"))]
    pub target_cleared: i32,
}

/// The decomp's `gmm_retval_EDB0`.
#[doc(alias("gmm_retval_EDB0"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "gmm_retval_EDB0", size = 0x8, export)]
pub struct StageUnlockRecords {
    #[at(0x0)]
    #[doc(alias("x0"))]
    pub vs_matches: i32,
    #[at(0x4)]
    #[doc(alias("x4"))]
    pub target_cleared: i32,
}

/// The decomp's `gmm_retval_EDBC`.
#[doc(alias("gmm_retval_EDBC"))]
#[derive(
    melee_derive::CloneFrom, Debug, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "gmm_retval_EDBC", size = 0x178, export, pad(0x4A, 2))]
pub struct TrophyRecords {
    #[at(0x0)]
    #[doc(alias("x0"))]
    pub vs_matches: i32,
    #[at(0x4)]
    #[doc(alias("x4"))]
    pub coin_matches: u32,
    #[at(0x8)]
    #[doc(alias("x8"))]
    pub target_cleared: i32,
    #[at(0xC)]
    #[doc(alias("xC"))]
    pub kos: u32,
    /// The VS distance walked (`fn_80161C90`).
    #[at(0x10)]
    #[doc(alias("x10"))]
    pub walk_distance: i32,
    #[at(0x14)]
    #[doc(alias("x14"))]
    pub most_coins: u32,
    #[at(0x18)]
    #[doc(alias("x18"))]
    pub plays: [u16; 25],
    /// Each character's best Target Test time (the C splits the array).
    #[at(0x4C)]
    #[doc(alias("x4C"))]
    pub target_best: [i32; 4],
    #[at(0x5C)]
    #[doc(alias("padding_x4C"))]
    pub target_best_rest: [u8; 84],
    #[at(0xB0)]
    #[doc(alias("xB0"))]
    pub home_run_best: [i32; 25],
    #[at(0x114)]
    #[doc(alias("x114"))]
    pub training_combo_best: [i32; 25],
}

impl Default for TrophyRecords {
    fn default() -> Self {
        TrophyRecords {
            vs_matches: 0,
            coin_matches: 0,
            target_cleared: 0,
            kos: 0,
            walk_distance: 0,
            most_coins: 0,
            plays: [0; 25],
            target_best: [0; 4],
            target_best_rest: [0; 84],
            home_run_best: [0; 25],
            training_combo_best: [0; 25],
        }
    }
}

/// The decomp's `gmm_x1868_1A8_t`.
#[doc(alias("gmm_x1868_1A8_t"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "gmm_x1868_1A8_t", size = 0x8, export, pad(0x7, 1))]
pub struct MessageRecords {
    #[at(0x0)]
    #[doc(alias("x0"))]
    pub kos: u32,
    #[at(0x4)]
    #[doc(alias("x4"))]
    pub all_characters_shown: u8,
    #[at(0x5)]
    #[doc(alias("x5"))]
    pub stages_shown: u8,
    #[at(0x6)]
    #[doc(alias("x6"))]
    pub everything_shown: u8,
}

/// The decomp's `GamePrefs`.
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(
    c = "GamePrefs",
    size = 0x20,
    export,
    pad(0x1, 7),
    pad(0x17, 1),
    pad(0x1C, 4)
)]
pub struct GamePrefs {
    #[at(0x0)]
    pub item_freq: u8,
    #[at(0x8)]
    pub item_mask: u64,
    #[at(0x10)]
    pub rumble_enabled: [u8; 4],
    #[at(0x14)]
    pub sound_balance: i8,
    #[at(0x15)]
    pub deflicker: u8,
    #[at(0x16)]
    pub saved_language: u8,
    #[at(0x18)]
    pub stage_mask: u32,
}

/// The decomp's `GmStats`.
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "GmStats", size = 0x44, export)]
pub struct GmStats {
    #[at(0x0)]
    pub sd_count: u16,
    #[at(0x2)]
    pub pad_2: [u8; 2],
    #[at(0x4)]
    pub attacks_hit: u32,
    #[at(0x8)]
    pub attacks_total: u32,
    #[at(0xC)]
    pub damage_dealt: i32,
    #[at(0x10)]
    pub damage_taken: i32,
    #[at(0x14)]
    pub damage_recovered: i32,
    #[at(0x18)]
    pub peak_damage: u16,
    #[at(0x1A)]
    pub match_count: u16,
    #[at(0x1C)]
    pub victories: u16,
    #[at(0x1E)]
    pub losses: u16,
    #[at(0x20)]
    pub play_time: u32,
    #[at(0x24)]
    pub total_player_count: u32,
    #[at(0x28)]
    pub walk_distance: i32,
    #[at(0x2C)]
    /// The matches' distances risen in the air (`MatchPlayerData::rise_distance`'s sum).
    #[doc(alias("run_distance"))]
    pub rise_distance: i32,
    #[at(0x30)]
    pub fall_distance: i32,
    #[at(0x34)]
    /// The distances knockback carried (`MatchPlayerData::knockback_distance`'s sum).
    #[doc(alias("peak_height"))]
    pub knockback_distance: i32,
    #[at(0x38)]
    pub coins_collected: i32,
    #[at(0x3C)]
    pub coins_swiped: i32,
    #[at(0x40)]
    pub coins_lost: i32,
}

/// The decomp's `FighterData_x7C`.
#[doc(alias("FighterData_x7C"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "FighterData_x7C", size = 0x30, export)]
pub struct FighterRecords {
    /// Target Test: cleared, then the best time; before it, the most targets broken.
    #[at(0x0, bits(0, 1))]
    #[doc(alias("b0"))]
    pub target_cleared: bool,
    #[at(0x0, bits(1, 1))]
    #[doc(alias("b1"))]
    pub ten_man_cleared: bool,
    #[at(0x0, bits(2, 1))]
    #[doc(alias("b2"))]
    pub hundred_man_cleared: bool,
    #[at(0x0, bits(3, 1))]
    #[doc(alias("b3"))]
    pub fifteen_min_cleared: bool,
    #[at(0x0, bits(4, 1))]
    #[doc(alias("b4"))]
    pub classic_cleared: bool,
    #[at(0x0, bits(5, 1))]
    #[doc(alias("b5"))]
    pub adventure_cleared: bool,
    #[at(0x0, bits(6, 1))]
    #[doc(alias("b6"))]
    pub all_star_cleared: bool,
    #[at(0x0, bits(7, 3))]
    pub b789: u8,
    #[at(0x1, bits(2, 3))]
    pub b10_to_12: u8,
    #[at(0x1, bits(5, 3))]
    pub b13_to_15: u8,
    /// Training's best combo.
    #[at(0x2)]
    #[doc(alias("x7E"))]
    pub training_combo: u16,
    /// The highest level each 1P mode was cleared on.
    #[at(0x4)]
    #[doc(alias("x80"))]
    pub classic_level: u8,
    #[at(0x5)]
    #[doc(alias("x81"))]
    pub adventure_level: u8,
    #[at(0x6)]
    #[doc(alias("x82"))]
    pub all_star_level: u8,
    #[at(0x7)]
    #[doc(alias("x83"))]
    pub unused_83: i8,
    #[at(0x8)]
    #[doc(alias("x84"))]
    pub home_run_distance: i32,
    #[at(0xC)]
    #[doc(alias("x88"))]
    pub classic_score: i32,
    #[at(0x10)]
    #[doc(alias("x8C"))]
    pub adventure_score: i32,
    #[at(0x14)]
    #[doc(alias("x90"))]
    pub all_star_score: i32,
    #[at(0x18)]
    #[doc(alias("x94"))]
    pub target_record: u32,
    #[at(0x1C)]
    #[doc(alias("x98"))]
    pub ten_man_time: i32,
    #[at(0x20)]
    #[doc(alias("x9C"))]
    pub hundred_man_time: i32,
    #[at(0x24)]
    #[doc(alias("xA0"))]
    pub three_min_kos: u16,
    #[at(0x26)]
    #[doc(alias("xA2"))]
    pub fifteen_min_kos: u16,
    #[at(0x28)]
    #[doc(alias("xA4"))]
    pub endless_kos: i32,
    #[at(0x2C)]
    #[doc(alias("xA8"))]
    pub cruel_kos: i32,
}

/// The decomp's `FighterData`.
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "FighterData", size = 0xAC, export)]
pub struct FighterData {
    #[at(0x0)]
    pub fighter_kos: [u16; 25],
    #[at(0x32)]
    pub padding_0x32: [u8; 2],
    #[at(0x34)]
    pub stats: GmStats,
    /// The times played, a big-endian half in two bytes.
    #[at(0x78)]
    #[doc(alias("x78"))]
    pub play_count_hi: i8,
    #[at(0x79)]
    #[doc(alias("x79"))]
    pub play_count_lo: i8,
    /// `x0.b0`: the character was played.
    #[at(0x7A, union)]
    #[doc(alias("x7A"))]
    pub played_flags: [u8; 1],
    #[at(0x7B)]
    #[doc(alias("x7B"))]
    pub unused_7b: i8,
    #[at(0x7C)]
    #[doc(alias("x7C"))]
    pub records: FighterRecords,
}

/// The decomp's `GmSaveData`.
#[derive(
    melee_derive::CloneFrom, Debug, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "GmSaveData", size = 0x1790, export)]
pub struct GmSaveData {
    #[at(0x0)]
    pub unlocked_characters: u16,
    #[at(0x2)]
    #[doc(alias("x186A"))]
    pub unlocked_stages: u16,
    #[at(0x4)]
    #[doc(alias("x186C"))]
    pub unlocked_features: u8,
    #[at(0x5)]
    pub pad_5: [i8; 3],
    #[at(0x8)]
    #[doc(alias("unk_8"))]
    pub challenger_records: ChallengerRecords,
    #[at(0x28)]
    #[doc(alias("unk_28"))]
    pub stage_unlock_records: StageUnlockRecords,
    #[at(0x30)]
    #[doc(alias("unk_30"))]
    pub trophy_records: TrophyRecords,
    #[at(0x1A8)]
    #[doc(alias("unk_1A8"))]
    pub message_records: MessageRecords,
    #[at(0x1B0)]
    pub time_matches: u32,
    #[at(0x1B4)]
    pub stock_matches: u32,
    #[at(0x1B8)]
    pub coin_matches: u32,
    #[at(0x1BC)]
    pub bonus_matches: u32,
    #[at(0x1C0)]
    pub stamina_matches: u32,
    #[at(0x1C4)]
    pub match_resets: u32,
    #[at(0x1C8)]
    #[doc(alias("x1A30"))]
    pub vs_contestants: i32,
    #[at(0x1CC)]
    #[doc(alias("x1A34"))]
    pub vs_play_time: i32,
    #[at(0x1D0)]
    #[doc(alias("x1A38"))]
    pub combined_vs_play_time: i32,
    #[at(0x1D4)]
    #[doc(alias("x1A3C"))]
    pub one_player_time: i32,
    #[at(0x1D8)]
    #[doc(alias("x1A40"))]
    pub unused_1a40: i32,
    /// The tournament's sudden deaths' seconds.
    #[at(0x1DC)]
    #[doc(alias("x1A44"))]
    pub tournament_time: i32,
    #[at(0x1E0)]
    #[doc(alias("x1A48"))]
    pub coins: u32,
    #[at(0x1E4)]
    #[doc(alias("x1A4C"))]
    pub most_coins: i32,
    #[at(0x1E8)]
    #[doc(alias("x1A50"))]
    pub power_count: i32,
    #[at(0x1EC)]
    #[doc(alias("x1A54"))]
    pub power_time: i32,
    #[at(0x1F0)]
    #[doc(alias("x1A58"))]
    pub total_damage: i32,
    #[at(0x1F4)]
    #[doc(alias("x1A5C"))]
    pub total_kos: i32,
    #[at(0x1F8)]
    #[doc(alias("x1A60"))]
    pub total_self_destructs: i32,
    #[at(0x1FC)]
    #[doc(alias("x1A64"))]
    pub unused_1a64: i32,
    /// A bit per event cleared, and each event's best (the C splits the array).
    #[at(0x200)]
    #[doc(alias("x1A68"))]
    pub events_cleared: i64,
    #[at(0x208)]
    #[doc(alias("x1A70"))]
    pub event_scores: [i32; 4],
    #[at(0x218)]
    #[doc(alias("padding_x1A70"))]
    pub event_scores_rest: [u8; 188],
    /// Event 51 ended with all three stocks left.
    #[at(0x2D4)]
    #[doc(alias("x1B3C"))]
    pub final_event_all_stocks: u8,
    #[at(0x2D5)]
    #[doc(alias("pad_2D5"))]
    pub pad_2d5: [i8; 3],
    /// The messages pending and noted (a bit each), the trophies (a bit each), and each
    /// message's time.
    #[at(0x2D8)]
    #[doc(alias("x1B40"))]
    pub messages_pending: [u32; 3],
    #[at(0x2E4)]
    #[doc(alias("x1B4C"))]
    pub messages_noted: [u32; 3],
    #[at(0x2F0)]
    #[doc(alias("x1B58"))]
    pub trophies: [u32; 10],
    #[at(0x318)]
    #[doc(alias("x1B80"))]
    pub message_times: [u32; 66],
    /// The bonuses seen (a bit each; the C splits the array).
    #[at(0x420)]
    #[doc(alias("x1C88"))]
    pub bonuses_seen: [u32; 3],
    #[at(0x42C)]
    #[doc(alias("padding_x1C88"))]
    pub bonuses_seen_rest: [u8; 28],
    #[at(0x448)]
    #[doc(alias("x1CB0"))]
    pub prefs: GamePrefs,
    #[at(0x468)]
    pub trophy_count: i16,
    #[at(0x46A)]
    pub trophy_category_flags: u16,
    #[at(0x46C)]
    pub trophy_flags: [u16; 293],
    #[at(0x6B6)]
    pub padding_trophy_flags: [u8; 14],
    #[at(0x6C4)]
    #[doc(alias("x1F2C"))]
    pub fighters: [FighterData; 25],
}

impl Default for GmSaveData {
    fn default() -> Self {
        GmSaveData {
            unlocked_characters: 0,
            unlocked_stages: 0,
            unlocked_features: 0,
            pad_5: [0; 3],
            challenger_records: Default::default(),
            stage_unlock_records: Default::default(),
            trophy_records: Default::default(),
            message_records: Default::default(),
            time_matches: 0,
            stock_matches: 0,
            coin_matches: 0,
            bonus_matches: 0,
            stamina_matches: 0,
            match_resets: 0,
            vs_contestants: 0,
            vs_play_time: 0,
            combined_vs_play_time: 0,
            one_player_time: 0,
            unused_1a40: 0,
            tournament_time: 0,
            coins: 0,
            most_coins: 0,
            power_count: 0,
            power_time: 0,
            total_damage: 0,
            total_kos: 0,
            total_self_destructs: 0,
            unused_1a64: 0,
            events_cleared: 0,
            event_scores: [0; 4],
            event_scores_rest: [0; 188],
            final_event_all_stocks: 0,
            pad_2d5: [0; 3],
            messages_pending: [0; 3],
            messages_noted: [0; 3],
            trophies: [0; 10],
            message_times: [0; 66],
            bonuses_seen: [0; 3],
            bonuses_seen_rest: [0; 28],
            prefs: Default::default(),
            trophy_count: 0,
            trophy_category_flags: 0,
            trophy_flags: [0; 293],
            padding_trophy_flags: [0; 14],
            fighters: core::array::from_fn(|_| Default::default()),
        }
    }
}

/// The decomp's `NameTagData`.
#[derive(
    melee_derive::CloneFrom, Debug, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "NameTagData", size = 0x1A4, export)]
pub struct NameTagData {
    #[at(0x0)]
    pub vs_kos: [u16; 120],
    #[at(0xF0)]
    pub stats: GmStats,
    #[at(0x134)]
    pub play_time_by_fighter: [u32; 25],
    #[at(0x198)]
    pub namedata: [i8; 8],
    /// The byte a four-letter name ends at; the handicap; a name read past its end reaches the
    /// padding.
    #[at(0x1A0)]
    #[doc(alias("x1A0"))]
    pub name_end: i8,
    #[at(0x1A1)]
    pub rumble_enabled: u8,
    #[at(0x1A2)]
    #[doc(alias("x1A2"))]
    pub handicap: i8,
    #[at(0x1A3)]
    #[doc(alias("padding_x1A2"))]
    pub pad_1a3: u8,
}

impl Default for NameTagData {
    fn default() -> Self {
        NameTagData {
            vs_kos: [0; 120],
            stats: Default::default(),
            play_time_by_fighter: [0; 25],
            namedata: [0; 8],
            name_end: 0,
            rumble_enabled: 0,
            handicap: 0,
            pad_1a3: 0,
        }
    }
}

/// The decomp's `NameTagDataBank`.
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "NameTagDataBank", size = 0x1F2C, export)]
pub struct NameTagDataBank {
    #[at(0x0)]
    pub inner: [NameTagData; 19],
}

/// The decomp's `GmCardData`.
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "GmCardData", size = 0xF1C8, export, pad(0xF1C4, 4))]
pub struct GmCardData {
    #[at(0x0)]
    pub save_data: GmSaveData,
    #[at(0x1790)]
    pub nametag_banks: [NameTagDataBank; 7],
}
