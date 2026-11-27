//! A match's start data (`StartMeleeData`): the rules (`StartMeleeRules`) and each of the six
//! players' setup (`PlayerInitData`), which a mode fills before the match scene starts and the
//! match reads as it sets up.

use crate::game::ft::CpuKind;
use crate::game::gr::StKind;
use crate::game::handles::{CodeRef, RawPtr};
use crate::game::mirror::Narrow;
use crate::game::pl::player::PlayerKind;

/// The decomp's `StartMeleeRules`.
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "StartMeleeRules", size = 0x60, pad(0x15, 3))]
pub struct StartMeleeRules {
    #[at(0x0, bits(0, 3))]
    pub match_kind: u8,
    /// How the damage displays show (`ifStatus_802F665C`).
    #[at(0x0, bits(3, 3))]
    #[doc(alias("x0_3"))]
    pub damage_display: u8,
    #[at(0x0, bits(6, 1))]
    pub timer_enabled: bool,
    #[at(0x0, bits(7, 1))]
    pub timer_counts_up: bool,
    /// The frame counter starts at `start_frame` as it is (else one less).
    #[at(0x1, bits(0, 1))]
    #[doc(alias("x1_0"))]
    pub exact_start_frame: bool,
    /// The coins count toward the clear's bonus.
    #[at(0x1, bits(1, 1))]
    #[doc(alias("x1_1"))]
    pub counts_coins: bool,
    /// The countdown's announcement is the long one (8 rather than 3).
    #[at(0x1, bits(2, 1))]
    #[doc(alias("x1_2"))]
    pub long_countdown: bool,
    /// The "Go!" is announced as kind 8 rather than 4.
    #[at(0x1, bits(3, 1))]
    #[doc(alias("x1_3"))]
    pub alt_go: bool,
    /// The stage's layout is the rules' (`stage_layout`): no alternate from held buttons.
    #[at(0x1, bits(4, 1))]
    #[doc(alias("x1_4"))]
    pub fixed_layout: bool,
    /// The match's end leaves the controllers' rumble as it is.
    #[at(0x1, bits(5, 1))]
    #[doc(alias("x1_5"))]
    pub keeps_rumble: bool,
    #[at(0x1, bits(6, 1))]
    pub timer_shows_hours: bool,
    #[at(0x1, bits(7, 1))]
    pub friendly_fire: bool,
    #[at(0x2, bits(0, 1))]
    pub is_stock: bool,
    /// Kept by the HUD's offscreen arrows (`arrows.x30`).
    #[at(0x2, bits(1, 1))]
    #[doc(alias("x2_1"))]
    pub arrows_flag: bool,
    /// Written by the modes that set up rules, read by none.
    #[at(0x2, bits(2, 1))]
    #[doc(alias("x2_2"))]
    pub unread_2_2: bool,
    #[at(0x2, bits(3, 1))]
    pub single_button: bool,
    #[at(0x2, bits(4, 1))]
    pub disable_pausing: bool,
    /// The damage displays show KOs and teammates' KOs.
    #[at(0x2, bits(5, 1))]
    #[doc(alias("x2_5"))]
    pub damage_display_kos: bool,
    #[at(0x2, bits(6, 1))]
    #[doc(alias("x2_6"))]
    pub damage_displays_on: bool,
    #[at(0x2, bits(7, 1))]
    #[doc(alias("x2_7"))]
    pub damage_display_fades: bool,
    #[at(0x3, bits(0, 1))]
    #[doc(alias("x3_0"))]
    pub shows_score: bool,
    /// The pause screen's controls it lists (the stick, Z, L+R+A+Start), whether it lists none,
    /// and whether the timers show.
    #[at(0x3, bits(1, 1))]
    #[doc(alias("x3_1"))]
    pub pause_shows_stick: bool,
    #[at(0x3, bits(2, 1))]
    #[doc(alias("x3_2"))]
    pub pause_z_retries: bool,
    #[at(0x3, bits(3, 1))]
    #[doc(alias("x3_3"))]
    pub pause_shows_z: bool,
    #[at(0x3, bits(4, 1))]
    #[doc(alias("x3_4"))]
    pub pause_lras_quits: bool,
    #[at(0x3, bits(5, 1))]
    #[doc(alias("x3_5"))]
    pub pause_shows_lras: bool,
    #[at(0x3, bits(6, 1))]
    #[doc(alias("x3_6"))]
    pub pause_hides_controls: bool,
    #[at(0x3, bits(7, 1))]
    #[doc(alias("x3_7"))]
    pub pause_shows_timers: bool,
    /// The pause camera (and the rules' `on_pause_override`).
    #[at(0x4, bits(0, 1))]
    #[doc(alias("x4_0"))]
    pub pause_camera: bool,
    #[at(0x4, bits(1, 1))]
    pub is_vs: bool,
    /// A single-player run's or Target Test's rules (`gm_8016B0E8`).
    #[at(0x4, bits(2, 1))]
    #[doc(alias("x4_2"))]
    pub single_player: bool,
    /// A game over goes to its own scene.
    #[at(0x4, bits(3, 1))]
    #[doc(alias("x4_3"))]
    pub game_over_scene: bool,
    /// The score is the clear's (`fn_8017F294`), else `score`.
    #[at(0x4, bits(4, 1))]
    #[doc(alias("x4_4"))]
    pub clear_scores: bool,
    #[at(0x4, bits(5, 1))]
    #[doc(alias("x4_5"))]
    pub allows_continue: bool,
    #[at(0x4, bits(6, 1))]
    #[doc(alias("x4_6"))]
    pub time_bonus: bool,
    #[at(0x4, bits(7, 1))]
    #[doc(alias("x4_7"))]
    pub damage_bonus: bool,
    /// The match has no outcome of its own (it ends from outside).
    #[at(0x5, bits(0, 1))]
    #[doc(alias("x5_0"))]
    pub endless: bool,
    /// The match goes on past the opponents' KOs (a horde's).
    #[at(0x5, bits(1, 1))]
    #[doc(alias("x5_1"))]
    pub goes_on: bool,
    #[at(0x5, bits(2, 1))]
    #[doc(alias("x5_2"))]
    pub no_fighter_hits: bool,
    #[at(0x5, bits(3, 1))]
    #[doc(alias("x5_3"))]
    pub stadium_zoom: bool,
    #[at(0x5, bits(4, 1))]
    #[doc(alias("x5_4"))]
    pub unused_5_4: bool,
    #[at(0x5, bits(5, 1))]
    #[doc(alias("x5_5"))]
    pub unused_5_5: bool,
    #[at(0x5, bits(6, 1))]
    #[doc(alias("x5_6"))]
    pub unused_5_6: bool,
    #[at(0x5, bits(7, 1))]
    #[doc(alias("x5_7"))]
    pub unused_5_7: bool,
    /// A Sudden Death: no items but the Bob-ombs' rain (`gm_8016B238`).
    #[at(0x6)]
    #[doc(alias("x6"))]
    pub sudden_death: u8,
    /// The match's ending kind, if the rules set one.
    #[at(0x7)]
    #[doc(alias("x7"))]
    pub end_kind: u8,
    #[at(0x8)]
    pub is_teams: u8,
    /// The run's kind: 1 a Target Test stage.
    #[at(0x9)]
    #[doc(alias("x9"))]
    pub run_kind: u8,
    /// The stage's layout (0: chosen by held buttons, `fn_8016E5C0`).
    #[at(0xA)]
    #[doc(alias("xA"))]
    pub stage_layout: u8,
    #[at(0xB)]
    pub item_freq: i8,
    #[at(0xC)]
    pub sd_penalty: i8,
    /// Frames the match waits after its end.
    #[at(0xD)]
    #[doc(alias("xD"))]
    pub end_wait: u8,
    #[at(0xE)]
    pub stkind: Narrow<StKind, u16>,
    #[at(0x10)]
    pub time_limit: u32,
    #[at(0x14)]
    #[doc(alias("x14"))]
    pub start_frame: u8,
    #[at(0x18)]
    #[doc(alias("x18"))]
    pub score: u32,
    #[at(0x1C)]
    #[doc(alias("x1C_pad"))]
    pub pad_1c: [u32; 1],
    /// The items the match may spawn, a bit each by kind.
    #[at(0x20)]
    #[doc(alias("x20"))]
    pub item_switch: u64,
    /// The Pokémon a Poké Ball lets out, 0 for a random one.
    #[at(0x28)]
    #[doc(alias("x28"))]
    pub forced_pokemon: i32,
    #[at(0x2C)]
    #[doc(alias("x2C"))]
    pub quake_scale: f32,
    #[at(0x30)]
    #[doc(alias("x30"))]
    pub damage_ratio: f32,
    #[at(0x34)]
    pub game_speed: f32,
    #[at(0x38)]
    pub on_unpause_override: Option<CodeRef>,
    #[at(0x3C)]
    pub on_pause_override: Option<CodeRef>,
    #[at(0x40)]
    pub check_for_pauser_override: Option<CodeRef>,
    #[at(0x44)]
    pub on_match_start: Option<CodeRef>,
    #[at(0x48)]
    pub on_frame_start: Option<CodeRef>,
    #[at(0x4C)]
    pub on_frame_end: Option<CodeRef>,
    #[at(0x50)]
    pub on_match_end: Option<CodeRef>,
    /// A single-player run's opponents' list, and a challenger's record.
    #[at(0x54)]
    #[doc(alias("x54"))]
    pub opponent_list: RawPtr,
    #[at(0x58)]
    #[doc(alias("x58"))]
    pub challenger_record: RawPtr,
    #[at(0x5C)]
    #[doc(alias("pad_x5C"))]
    pub pad_5c: [u8; 4],
}

/// The decomp's `PlayerInitData`.
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "PlayerInitData", size = 0x24, pad(0x16, 2))]
pub struct PlayerInitData {
    #[at(0x0)]
    pub ckind: i8,
    #[at(0x1)]
    pub slot_type: Narrow<PlayerKind, u8>,
    #[at(0x2)]
    pub stocks: i8,
    #[at(0x3)]
    pub color: u8,
    #[at(0x4)]
    pub slot: u8,
    #[at(0x5)]
    pub spawn_pos: i8,
    #[at(0x6)]
    pub spawn_dir: i8,
    #[at(0x7)]
    pub sub_color: u8,
    #[at(0x8)]
    pub handicap: i8,
    #[at(0x9)]
    pub team: u8,
    #[at(0xA)]
    pub nametag: u8,
    /// The player's `more_flags.sfx_size`.
    #[at(0xB)]
    #[doc(alias("xB"))]
    pub sfx_size: u8,
    #[at(0xC, bits(0, 1))]
    pub rumble_enabled: bool,
    /// The fighter comes in through the entry state, staggered (`entry_delay`).
    #[at(0xC, bits(1, 1))]
    #[doc(alias("xC_b1"))]
    pub enters: bool,
    #[at(0xC, bits(2, 1))]
    pub vs_metal: bool,
    /// Copied to the VS fighter's flag `b1`, which nothing reads.
    #[at(0xC, bits(3, 1))]
    #[doc(alias("xC_b3"))]
    pub unread_c_b3: bool,
    #[at(0xC, bits(4, 1))]
    pub vs_invisible: bool,
    /// The player's `attrs_scaled`, `voice_muted` and `stamina`.
    #[at(0xC, bits(5, 1))]
    #[doc(alias("xC_b5"))]
    pub attrs_scaled: bool,
    #[at(0xC, bits(6, 1))]
    #[doc(alias("xC_b6"))]
    pub voice_muted: bool,
    #[at(0xC, bits(7, 1))]
    #[doc(alias("xC_b7"))]
    pub stamina: bool,
    /// The stamina display explodes at 0 HP (the hands).
    #[at(0xD, bits(0, 1))]
    #[doc(alias("xD_b0"))]
    pub hp_explodes: bool,
    /// The fighter shows its team's hearts.
    #[at(0xD, bits(1, 1))]
    #[doc(alias("xD_b1"))]
    pub shows_hearts: bool,
    /// The player takes no offscreen damage, has no magnifier bubble, is KO'd upward.
    #[at(0xD, bits(2, 1))]
    #[doc(alias("xD_b2"))]
    pub no_offscreen_damage: bool,
    #[at(0xD, bits(3, 1))]
    #[doc(alias("xD_b3"))]
    pub no_magnifier: bool,
    #[at(0xD, bits(4, 1))]
    #[doc(alias("xD_b4"))]
    pub dies_up: bool,
    #[at(0xD, bits(5, 1))]
    #[doc(alias("xD_b5"))]
    pub unused_d_b5: bool,
    #[at(0xD, bits(6, 1))]
    #[doc(alias("xD_b6"))]
    pub unused_d_b6: bool,
    #[at(0xD, bits(7, 1))]
    #[doc(alias("xD_b7"))]
    pub unused_d_b7: bool,
    #[at(0xE)]
    pub cpu_kind: Narrow<CpuKind, u8>,
    #[at(0xF)]
    pub cpu_level: u8,
    #[at(0x10)]
    pub damage: u16,
    #[at(0x12)]
    pub damage1: u16,
    #[at(0x14)]
    pub hp: u16,
    #[at(0x18)]
    pub attack_ratio: f32,
    #[at(0x1C)]
    pub defense_ratio: f32,
    #[at(0x20)]
    pub model_scale: f32,
}

/// The decomp's `StartMeleeData`.
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "StartMeleeData", size = 0x138)]
pub struct StartMeleeData {
    #[at(0x0)]
    pub rules: StartMeleeRules,
    #[at(0x60)]
    pub players: [PlayerInitData; 6],
}
