//! The event mode's saved choice and round state (`EventData`, the game data's `x530`,
//! hand-written as [`super::super::types`] are): the event, its fighter and color, the round's
//! score and counters, and the target test's choice beside it (`TargetTestChoice`).

/// The decomp's `gmm_x0_584_t`.
#[doc(alias("gmm_x0_584_t"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "gmm_x0_584_t", size = 0x4)]
pub struct TargetTestChoice {
    #[at(0x0)]
    #[doc(alias("unk_584"))]
    pub ckind: i8,
    #[at(0x1)]
    #[doc(alias("unk_585"))]
    pub color: u8,
    #[at(0x2)]
    #[doc(alias("unk_586"))]
    pub nametag: u8,
    #[at(0x3)]
    #[doc(alias("unk_587"))]
    pub unused_587: i8,
}

/// The decomp's `EventData`.
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "EventData", size = 0x58, pad(0x39, 3), pad(0x46, 2))]
pub struct EventData {
    /// The first player's character and costume, as played.
    #[at(0x0)]
    #[doc(alias("x0"))]
    pub char_kind: u8,
    #[at(0x1)]
    #[doc(alias("x1"))]
    pub char_color: u8,
    /// The select's choice.
    #[at(0x2)]
    #[doc(alias("x2"))]
    pub chosen_char: i8,
    #[at(0x3)]
    #[doc(alias("x3"))]
    pub chosen_color: u8,
    #[at(0x4)]
    pub nametag: u8,
    #[at(0x5)]
    #[doc(alias("unk_535"))]
    pub event: u8,
    #[at(0x6)]
    #[doc(alias("x6"))]
    pub port: u8,
    #[at(0x7)]
    #[doc(alias("x7"))]
    pub team: u8,
    /// The random opponents kept for the event's retries (players 2 to 4).
    #[at(0x8)]
    #[doc(alias("x8"))]
    pub random_char_1: i8,
    #[at(0x9)]
    #[doc(alias("x9"))]
    pub random_char_2: i8,
    #[at(0xA)]
    #[doc(alias("xA"))]
    pub random_char_3: i8,
    #[at(0xB, bits(0, 1))]
    #[doc(alias("xB_0"))]
    pub timer_counts_up: bool,
    #[at(0xB, bits(1, 1))]
    #[doc(alias("xB_1"))]
    pub cleared: bool,
    #[at(0xB, bits(2, 1))]
    #[doc(alias("xB_2"))]
    pub round_won: bool,
    #[at(0xB, bits(3, 1))]
    #[doc(alias("xB_3"))]
    pub crowd: bool,
    #[at(0xB, bits(4, 1))]
    #[doc(alias("xB_4"))]
    pub series: bool,
    #[at(0xB, bits(5, 1))]
    #[doc(alias("xB_5"))]
    pub voices_loaded: bool,
    #[at(0xB, bits(6, 1))]
    #[doc(alias("xB_6"))]
    pub timed: bool,
    #[at(0xB, bits(7, 1))]
    #[doc(alias("xB_7"))]
    pub unused_b7: bool,
    #[at(0xC)]
    #[doc(alias("xC"))]
    pub score: i32,
    /// The event's process's counter (the egg dropped, a round's wait, the joiner joined) and
    /// state (the egg broke, Event 36's round).
    #[at(0x10)]
    #[doc(alias("x10"))]
    pub proc_counter: i32,
    #[at(0x14)]
    #[doc(alias("x14"))]
    pub unread_14: i32,
    #[at(0x18)]
    #[doc(alias("x18"))]
    pub proc_state: i32,
    #[at(0x1C)]
    #[doc(alias("x1C"))]
    pub quake_scale: f32,
    /// A series: the round, the first player's stocks and damage carried over, the time left,
    /// the series' frames, the character kept (Sheik, 0x21 none).
    #[at(0x20)]
    #[doc(alias("x20"))]
    pub round: i32,
    #[at(0x24)]
    #[doc(alias("x24"))]
    pub round_stocks: i32,
    #[at(0x28)]
    #[doc(alias("x28"))]
    pub round_damage: i32,
    #[at(0x2C)]
    #[doc(alias("x2C"))]
    pub round_seconds: i32,
    #[at(0x30)]
    #[doc(alias("x30"))]
    pub round_frames: i32,
    #[at(0x34)]
    #[doc(alias("x34"))]
    pub series_frames: i32,
    #[at(0x38)]
    #[doc(alias("x38"))]
    pub kept_char: u8,
    #[at(0x3C)]
    #[doc(alias("x3C"))]
    pub coins: i32,
    #[at(0x40)]
    #[doc(alias("x40"))]
    pub play_frames: i32,
    /// How the files load (`gm_801BBB64`: 0 the first player's character set, 1 to choose, 2 to
    /// 4 a series' rounds), whether a Kirby crowd copies, the stage, the players' characters and
    /// costumes.
    #[at(0x44)]
    #[doc(alias("x44"))]
    pub load_kind: i8,
    #[at(0x45)]
    #[doc(alias("x45"))]
    pub crowd_kirby_copies: u8,
    #[at(0x48)]
    #[doc(alias("x48"))]
    pub stage: i32,
    #[at(0x4C)]
    #[doc(alias("x4C"))]
    pub chars: [i8; 4],
    #[at(0x50)]
    #[doc(alias("x50"))]
    pub colors: [u8; 4],
    #[at(0x54)]
    #[doc(alias("unk_584"))]
    pub target_test: TargetTestChoice,
}
