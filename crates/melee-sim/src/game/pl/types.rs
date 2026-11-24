//! The player blocks' structs (`pl/player.h`, `pl/types.h`), hand-written ([design call 3's
//! second half](../../../../../docs/refactor.md#design-call-3s-second-half-the-owned-structs)):
//! a slot's block (`StaticPlayer`, the decomp's `_StaticPlayer`), its flag bytes, and the match
//! statistics it keeps in its stale-move table (`StaleMoveTable`: the attacks and actions
//! counted, the combos, the KO and Bob-omb rings, the point bonuses' counts). Each field's
//! `#[at]` is its place in the original's struct, which the comparer's mirror reads
//! (`#[derive(Mirror)]`) and the layout check holds to the decomp's (`game::mirror`). The
//! fields keep the original's order, so the sync test's hash reads them as the generated
//! structs' did. The stale-move table's last-KO hit (`ft_800898B4_t`), which a fighter's damage
//! holds too, stays generated in `game::structs`.

use crate::game::handles::CodeRef;
use crate::game::structs::ft_800898B4_t;
use crate::hsd::gobj::GObjId;

/// The decomp's `_StaticPlayer_flags`.
#[doc(alias("_StaticPlayer_flags"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "_StaticPlayer_flags", size = 0x1)]
pub struct PlayerFlags {
    #[at(0x0, bits(0, 1))]
    #[doc(alias("b0"))]
    pub rumbles: bool,
    /// A joiner of a setup that has several (drawn as a wireframe), `Player_SetFlagsBit1`.
    #[at(0x0, bits(1, 1))]
    #[doc(alias("b1"))]
    pub joiner: bool,
    /// The player has a second fighter (the Ice Climbers' Nana).
    #[at(0x0, bits(2, 1))]
    #[doc(alias("b2"))]
    pub two_fighters: bool,
    /// Its fighter comes in through the entry state.
    #[at(0x0, bits(3, 1))]
    #[doc(alias("b3"))]
    pub enters: bool,
    /// The entry waits `ftCommonData` x6C8 frames more.
    #[at(0x0, bits(4, 1))]
    #[doc(alias("b4"))]
    pub entry_waits: bool,
    #[at(0x0, bits(5, 1))]
    pub is_metal: bool,
    /// The fighter's `takes_damage_cloaked` and `no_color_anims`.
    #[at(0x0, bits(6, 1))]
    #[doc(alias("b6"))]
    pub damage_cloaked: bool,
    #[at(0x0, bits(7, 1))]
    #[doc(alias("b7"))]
    pub no_color_anims: bool,
}

/// The decomp's `_StaticPlayer_more_flags`.
#[doc(alias("_StaticPlayer_more_flags"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "_StaticPlayer_more_flags", size = 0x1)]
pub struct PlayerRuleFlags {
    /// The fighter's `x2034` setup runs its rule set (`ftCo_800C8FC4`).
    #[at(0x0, bits(0, 1))]
    #[doc(alias("b0"))]
    pub init_rule_set: bool,
    /// The fighter's `rules_voice_muted`, `is_stamina` and `sfx_size`.
    #[at(0x0, bits(1, 1))]
    #[doc(alias("b1"))]
    pub voice_muted: bool,
    #[at(0x0, bits(2, 1))]
    #[doc(alias("b2"))]
    pub stamina: bool,
    /// Offscreen damage applies to it (`Fighter_procAnim`).
    #[at(0x0, bits(3, 1))]
    #[doc(alias("b3"))]
    pub offscreen_damage: bool,
    /// No magnifier bubble shows it.
    #[at(0x0, bits(4, 1))]
    #[doc(alias("b4"))]
    pub no_magnifier: bool,
    /// It's KO'd upward (`enter_dead_up`).
    #[at(0x0, bits(5, 1))]
    #[doc(alias("b5"))]
    pub dies_up: bool,
    #[at(0x0, bits(6, 2))]
    #[doc(alias("b6"))]
    pub sfx_size: u8,
}

/// The decomp's `_StaticPlayer_flagsAE`.
#[doc(alias("_StaticPlayer_flagsAE"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "_StaticPlayer_flagsAE", size = 0x1)]
pub struct PlayerInitFlags {
    /// The fighter's `attrs_scaled`.
    #[at(0x0, bits(0, 1))]
    #[doc(alias("b0"))]
    pub attrs_scaled: bool,
    /// Read by Kirby's setup (`ftKb_Init`), set by nothing a match reaches.
    #[at(0x0, bits(1, 1))]
    #[doc(alias("b1"))]
    pub kirby_init_flag: bool,
    #[at(0x0, bits(2, 1))]
    #[doc(alias("b2"))]
    pub unused_b2: bool,
    #[at(0x0, bits(3, 1))]
    #[doc(alias("b3"))]
    pub unused_b3: bool,
    #[at(0x0, bits(4, 1))]
    #[doc(alias("b4"))]
    pub unused_b4: bool,
    #[at(0x0, bits(5, 1))]
    #[doc(alias("b5"))]
    pub unused_b5: bool,
    #[at(0x0, bits(6, 1))]
    #[doc(alias("b6"))]
    pub unused_b6: bool,
    #[at(0x0, bits(7, 1))]
    #[doc(alias("b7"))]
    pub unused_b7: bool,
}

/// The decomp's `StaleMoveTable_StaleMoves`.
#[doc(alias("StaleMoveTable_StaleMoves"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "StaleMoveTable_StaleMoves", size = 0x4)]
pub struct StaleMove {
    #[at(0x0)]
    pub move_id: u16,
    #[at(0x2)]
    pub attack_instance: u16,
}

/// The decomp's `plAttackStats`.
#[doc(alias("plAttackStats"))]
#[derive(
    melee_derive::CloneFrom, Debug, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "plAttackStats", size = 0x1AC)]
pub struct AttackStats {
    #[at(0x0)]
    pub total: u32,
    #[at(0x4)]
    pub by_attack_counts: [u32; 100],
    #[at(0x194)]
    pub thrown_item_count: u32,
    #[at(0x198)]
    pub aerials_count: u32,
    #[at(0x19C)]
    pub specials_count: u32,
    /// The attacks but grabs, pummels and throws (the attack flags' `count_x1A0`).
    #[at(0x1A0)]
    #[doc(alias("x1A0_count"))]
    pub strikes_count: u32,
    /// The reflectors' (the attack flags' `count_x1A4`: the cape, the shine, the bat, ...).
    #[at(0x1A4)]
    #[doc(alias("x1A4_count"))]
    pub reflectors_count: u32,
    /// Hitboxes angled in the common data's spike range (`pl_80037ECC`).
    #[at(0x1A8)]
    #[doc(alias("x1A8"))]
    pub spikes_count: u32,
}

impl Default for AttackStats {
    fn default() -> Self {
        AttackStats {
            total: 0,
            by_attack_counts: [0; 100],
            thrown_item_count: 0,
            aerials_count: 0,
            specials_count: 0,
            strikes_count: 0,
            reflectors_count: 0,
            spikes_count: 0,
        }
    }
}

/// The decomp's `plActionStats`.
#[doc(alias("plActionStats"))]
#[derive(
    melee_derive::CloneFrom, Debug, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(
    c = "plActionStats",
    size = 0x5C0,
    pad(0x5BC, bits(4, 4)),
    pad(0x5BD, 3)
)]
pub struct ActionStats {
    #[at(0x0)]
    pub attacks: AttackStats,
    #[at(0x1AC)]
    pub hits: AttackStats,
    /// A third `plAttackStats` the mirror keeps as bytes: the attacks that landed (an attack
    /// record's first hit), by id.
    #[at(0x358, union)]
    #[doc(alias("x358"))]
    pub landed: [u8; 428],
    /// By attack id, the players it hit (a bit each).
    #[at(0x504)]
    #[doc(alias("x504"))]
    pub victims: [u8; 100],
    #[at(0x568)]
    #[doc(alias("x568"))]
    pub front_hits: u32,
    #[at(0x56C)]
    #[doc(alias("x56C"))]
    pub back_hits: u32,
    #[at(0x570)]
    #[doc(alias("x570"))]
    pub other_hits: u32,
    /// Down tilts' and down smashes' hits.
    #[at(0x574)]
    #[doc(alias("x574"))]
    pub down_hits: u32,
    /// The actions counted past the attacks' ids (`pl_800386D8`'s ids from 100): turns, squats,
    /// walks, dashes, runs, jumps, air jumps, wall bounces.
    #[at(0x578)]
    #[doc(alias("x578"))]
    pub turns: u32,
    #[at(0x57C)]
    #[doc(alias("x57C"))]
    pub squats: u32,
    #[at(0x580)]
    #[doc(alias("x580"))]
    pub walks: u32,
    #[at(0x584)]
    #[doc(alias("x584"))]
    pub dashes: u32,
    #[at(0x588)]
    #[doc(alias("x588"))]
    pub runs: u32,
    #[at(0x58C)]
    #[doc(alias("x58C"))]
    pub jumps: u32,
    #[at(0x590)]
    #[doc(alias("x590"))]
    pub air_jumps: u32,
    #[at(0x594)]
    #[doc(alias("x594"))]
    pub wall_bounces: u32,
    /// Ids 108 to 115: the downs (two), techs, platform drops, ledge catches, taunts, Fox's
    /// taunt, powershields.
    #[at(0x598)]
    #[doc(alias("x598"))]
    pub more_actions: [u32; 8],
    #[at(0x5B8)]
    #[doc(alias("x5B8"))]
    pub unused_5b8: [u8; 4],
    /// The player turned, walked, jumped, jumped in the air (the attack flags' bits).
    #[at(0x5BC, bits(0, 1))]
    #[doc(alias("x5BC_b0"))]
    pub has_turned: bool,
    #[at(0x5BC, bits(1, 1))]
    #[doc(alias("x5BC_b1"))]
    pub has_walked: bool,
    #[at(0x5BC, bits(2, 1))]
    #[doc(alias("x5BC_b2"))]
    pub has_jumped: bool,
    #[at(0x5BC, bits(3, 1))]
    #[doc(alias("x5BC_b3"))]
    pub has_air_jumped: bool,
}

impl Default for ActionStats {
    fn default() -> Self {
        ActionStats {
            attacks: Default::default(),
            hits: Default::default(),
            landed: [0; 428],
            victims: [0; 100],
            front_hits: 0,
            back_hits: 0,
            other_hits: 0,
            down_hits: 0,
            turns: 0,
            squats: 0,
            walks: 0,
            dashes: 0,
            runs: 0,
            jumps: 0,
            air_jumps: 0,
            wall_bounces: 0,
            more_actions: [0; 8],
            unused_5b8: [0; 4],
            has_turned: false,
            has_walked: false,
            has_jumped: false,
            has_air_jumped: false,
        }
    }
}

/// The decomp's `pl_x5EC_t_x10`.
#[doc(alias("pl_x5EC_t_x10"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(
    c = "pl_x5EC_t_x10",
    size = 0x14,
    pad(0xA, 2),
    pad(0x12, bits(1, 7)),
    pad(0x13, 1)
)]
pub struct AttackerCombo {
    #[at(0x0)]
    #[doc(alias("x0"))]
    pub damage: f32,
    #[at(0x4)]
    #[doc(alias("x4"))]
    pub hits: i16,
    #[at(0x6)]
    #[doc(alias("x6"))]
    pub unused_6: u16,
    /// The same attack's hits in a row, and that attack.
    #[at(0x8)]
    #[doc(alias("x8"))]
    pub repeats: i16,
    #[at(0xC)]
    #[doc(alias("xC"))]
    pub attack: i32,
    /// Hits while this player's in the air (cleared when it lands).
    #[at(0x10)]
    #[doc(alias("x10"))]
    pub air_hits: i16,
    /// A jab hit it while it shielded.
    #[at(0x12, bits(0, 1))]
    #[doc(alias("x12_b0"))]
    pub shield_jabbed: bool,
}

/// The decomp's `pl_x5EC_t`.
#[doc(alias("pl_x5EC_t"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "pl_x5EC_t", size = 0x88, pad(0x1, 3))]
pub struct ComboStats {
    #[at(0x0)]
    #[doc(alias("x0"))]
    pub last_victim: u8,
    #[at(0x4)]
    #[doc(alias("x4"))]
    pub last_damage: f32,
    /// The hits in combos, and the combos (`pl_800414C0`'s average).
    #[at(0x8)]
    #[doc(alias("x8"))]
    pub combo_hits: u32,
    #[at(0xC)]
    #[doc(alias("xC"))]
    pub combo_count: u32,
    /// By attacker, its combo on this player.
    #[at(0x10)]
    #[doc(alias("x10"))]
    pub by_attacker: [AttackerCombo; 6],
}

/// The decomp's `pl_x8C0_t_x4`.
#[doc(alias("pl_x8C0_t_x4"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "pl_x8C0_t_x4", size = 0x4, pad(0x3, bits(1, 7)))]
pub struct KoRingEntry {
    #[at(0x0)]
    #[doc(alias("x0"))]
    pub instance: u16,
    #[at(0x2)]
    #[doc(alias("x2"))]
    pub kos: u8,
    #[at(0x3, bits(0, 1))]
    #[doc(alias("x3_b0"))]
    pub self_ko: bool,
}

/// The decomp's `pl_x8C0_t`.
#[doc(alias("pl_x8C0_t"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "pl_x8C0_t", size = 0x18)]
pub struct KoRing {
    #[at(0x0)]
    #[doc(alias("x0"))]
    pub next: i32,
    #[at(0x4)]
    #[doc(alias("x4"))]
    pub entries: [KoRingEntry; 5],
}

/// The decomp's `pl_x8D8_t_x4`.
#[doc(alias("pl_x8D8_t_x4"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "pl_x8D8_t_x4", size = 0x8, pad(0x4, bits(2, 6)), pad(0x5, 3))]
pub struct BobOmbRingEntry {
    #[at(0x0)]
    #[doc(alias("x0"))]
    pub spawn: u32,
    #[at(0x4, bits(0, 1))]
    #[doc(alias("x4_b0"))]
    pub hit: bool,
    #[at(0x4, bits(1, 1))]
    #[doc(alias("x4_b1"))]
    pub used: bool,
}

/// The decomp's `pl_x8D8_t`.
#[doc(alias("pl_x8D8_t"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "pl_x8D8_t", size = 0x2C)]
pub struct BobOmbRing {
    #[at(0x0)]
    #[doc(alias("x0"))]
    pub next: i32,
    #[at(0x4)]
    #[doc(alias("x4"))]
    pub entries: [BobOmbRingEntry; 5],
}

/// The decomp's `StaleMoveTable`.
#[derive(
    melee_derive::CloneFrom, Debug, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "StaleMoveTable", size = 0xCF4, pad(0xCD6, 2))]
pub struct StaleMoveTable {
    #[at(0x0)]
    pub current_index: i32,
    #[at(0x4)]
    #[doc(alias("StaleMoves"))]
    pub queue: [StaleMove; 10],
    #[at(0x2C)]
    #[doc(alias("total_attack_count_struct"))]
    pub actions: ActionStats,
    #[at(0x5EC)]
    #[doc(alias("x5EC"))]
    pub combos: ComboStats,
    /// By the item log's kind: items taken in hand that had been thrown, and of those, ones no
    /// player had touched (`pl_8003E17C`).
    #[at(0x674)]
    #[doc(alias("x674"))]
    pub items_picked_up: [u32; 39],
    #[at(0x710)]
    #[doc(alias("x710"))]
    pub fresh_items_picked_up: [i32; 39],
    /// Each kind's run of containers giving it (`pl_8003E4A4`).
    #[at(0x7AC)]
    #[doc(alias("x7AC"))]
    pub container_runs: [i32; 39],
    /// Each Pokémon's count out of the player's Poké Balls.
    #[at(0x848)]
    #[doc(alias("x848"))]
    pub pokemon_released: [i32; 30],
    #[at(0x8C0)]
    #[doc(alias("x8C0"))]
    pub ko_ring: KoRing,
    #[at(0x8D8)]
    #[doc(alias("x8D8"))]
    pub bombhei_ring: BobOmbRing,
    /// The bonuses, by kind: a flag or a count.
    #[at(0x904)]
    #[doc(alias("x904"))]
    pub bonuses: [u32; 215],
    #[at(0xC60)]
    #[doc(alias("xC60"))]
    pub damage_taken: f32,
    #[at(0xC64)]
    #[doc(alias("xC64"))]
    pub peak_damage: f32,
    #[at(0xC68)]
    #[doc(alias("xC68"))]
    pub damage_recovered: i32,
    #[at(0xC6C)]
    #[doc(alias("xC6C"))]
    pub damage_dealt: f32,
    /// The damage dealt by pummels, and by the Super Scope's shots.
    #[at(0xC70)]
    #[doc(alias("xC70"))]
    pub pummel_damage: f32,
    #[at(0xC74)]
    #[doc(alias("xC74"))]
    pub scope_damage: f32,
    /// The damage dealt to each player.
    #[at(0xC78)]
    #[doc(alias("xC78"))]
    pub damage_to: [f32; 6],
    /// The combo order bonus's step (`pl_8003FC88`).
    #[at(0xC90)]
    #[doc(alias("xC90"))]
    pub combo_step: u32,
    /// Falls as a star KO.
    #[at(0xC94)]
    #[doc(alias("xC94"))]
    pub star_falls: i32,
    /// The damage taken from the stage's devices.
    #[at(0xC98)]
    #[doc(alias("xC98"))]
    pub stage_damage: f32,
    /// The match's running average of the percent.
    #[at(0xC9C)]
    #[doc(alias("xC9C"))]
    pub average_percent: f32,
    /// The victims' percents at the hits, and their count (`pl_8003ED0C`).
    #[at(0xCA0)]
    #[doc(alias("xCA0"))]
    pub victim_percent_sum: f32,
    #[at(0xCA4)]
    #[doc(alias("xCA4"))]
    pub victim_percent_count: i32,
    /// The Ray Gun's hits in a row.
    #[at(0xCA8)]
    #[doc(alias("xCA8"))]
    pub ray_gun_hits: u32,
    /// The player whose Lip's Stick flowered this one (6 none).
    #[at(0xCAC)]
    #[doc(alias("xCAC"))]
    pub flowered_by: u32,
    /// Hits on teammates under friendly fire (`pl_80040D8C`).
    #[at(0xCB0)]
    #[doc(alias("xCB0"))]
    pub teammate_hits: i32,
    /// KOs off the top.
    #[at(0xCB4)]
    #[doc(alias("xCB4"))]
    pub up_kos: i32,
    /// The last KO's: who scored it, the attack's flags, its hit's record and attack instance.
    #[at(0xCB8)]
    #[doc(alias("xCB8"))]
    pub last_ko_by: i32,
    #[at(0xCBC, union)]
    #[doc(alias("xCBC"))]
    pub last_ko_attack: [u8; 4],
    #[at(0xCC0)]
    #[doc(alias("xCC0"))]
    pub last_ko_hit: ft_800898B4_t,
    #[at(0xCD4)]
    #[doc(alias("xCD4"))]
    pub last_ko_instance: u16,
    #[at(0xCD8)]
    #[doc(alias("xCD8"))]
    pub powershields: i32,
    /// The SDI's distance in x and y (`pl_800401F0`).
    #[at(0xCDC)]
    #[doc(alias("xCDC"))]
    pub sdi_distance_x: f32,
    #[at(0xCE0)]
    #[doc(alias("xCE0"))]
    pub sdi_distance_y: f32,
    /// Hits taken with knockback past `pl_804D6470` x98.
    #[at(0xCE4)]
    #[doc(alias("xCE4"))]
    pub strong_hits_taken: u32,
    #[at(0xCE8)]
    #[doc(alias("xCE8"))]
    pub ledge_catches: u32,
    /// The frames spent mashing out of a grab or a flower, and the mashes.
    #[at(0xCEC)]
    #[doc(alias("xCEC"))]
    pub mash_frames: i32,
    #[at(0xCF0)]
    #[doc(alias("xCF0"))]
    pub mashes: i32,
}

impl Default for StaleMoveTable {
    fn default() -> Self {
        StaleMoveTable {
            current_index: 0,
            queue: core::array::from_fn(|_| Default::default()),
            actions: Default::default(),
            combos: Default::default(),
            items_picked_up: [0; 39],
            fresh_items_picked_up: [0; 39],
            container_runs: [0; 39],
            pokemon_released: [0; 30],
            ko_ring: Default::default(),
            bombhei_ring: Default::default(),
            bonuses: [0; 215],
            damage_taken: 0.0,
            peak_damage: 0.0,
            damage_recovered: 0,
            damage_dealt: 0.0,
            pummel_damage: 0.0,
            scope_damage: 0.0,
            damage_to: [0.0; 6],
            combo_step: 0,
            star_falls: 0,
            stage_damage: 0.0,
            average_percent: 0.0,
            victim_percent_sum: 0.0,
            victim_percent_count: 0,
            ray_gun_hits: 0,
            flowered_by: 0,
            teammate_hits: 0,
            up_kos: 0,
            last_ko_by: 0,
            last_ko_attack: [0; 4],
            last_ko_hit: Default::default(),
            last_ko_instance: 0,
            powershields: 0,
            sdi_distance_x: 0.0,
            sdi_distance_y: 0.0,
            strong_hits_taken: 0,
            ledge_catches: 0,
            mash_frames: 0,
            mashes: 0,
        }
    }
}

/// The decomp's `_StaticPlayer`.
#[doc(alias("_StaticPlayer"))]
#[derive(
    melee_derive::CloneFrom, Debug, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "_StaticPlayer", size = 0xE90)]
pub struct StaticPlayer {
    #[at(0x0)]
    pub player_state: i32,
    #[at(0x4)]
    pub ckind: i32,
    #[at(0x8)]
    pub pkind: i32,
    #[at(0xC)]
    pub transformed: [u8; 2],
    #[at(0xE)]
    #[doc(alias("unk0E"))]
    pub unused_0e: i16,
    #[at(0x10, union)]
    pub player_poses: [u8; 48],
    #[at(0x40)]
    pub facing_direction: f32,
    #[at(0x44)]
    pub costume_id: u8,
    #[at(0x45)]
    #[doc(alias("unk45"))]
    pub effect_color: u8,
    #[at(0x46)]
    pub sub_color: i8,
    #[at(0x47)]
    pub team: u8,
    #[at(0x48)]
    pub pad_port: u8,
    #[at(0x49)]
    pub cpu_level: u8,
    #[at(0x4A)]
    pub cpu_type: u8,
    #[at(0x4B)]
    pub handicap: u8,
    #[at(0x4C)]
    #[doc(alias("unk4C"))]
    pub entry_delay: i8,
    #[at(0x4D)]
    #[doc(alias("unk4D"))]
    pub kirby_copy_kind: i8,
    #[at(0x4E)]
    #[doc(alias("unk4E"))]
    pub spawn_point: i8,
    #[at(0x4F)]
    #[doc(alias("unk4F"))]
    pub unused_4f: i8,
    /// Master Hand's defense ratio (`Player_GetUnk50`, which no setter sets).
    #[at(0x50)]
    #[doc(alias("unk50"))]
    pub unset_ratio: f32,
    #[at(0x54)]
    pub attack_ratio: f32,
    #[at(0x58)]
    pub defense_ratio: f32,
    #[at(0x5C)]
    pub model_scale: f32,
    #[at(0x60, union)]
    pub staminas: [u8; 8],
    #[at(0x68)]
    pub falls: [i32; 2],
    #[at(0x70)]
    pub kos_by_player: [u32; 6],
    #[at(0x88)]
    pub match_frame_count: u32,
    #[at(0x8C)]
    pub self_destructs: u16,
    #[at(0x8E)]
    pub stocks: i8,
    #[at(0x8F)]
    #[doc(alias("unk8F"))]
    pub unused_8f: i8,
    #[at(0x90)]
    pub current_coins: i32,
    #[at(0x94)]
    pub total_coins: i32,
    #[at(0x98)]
    #[doc(alias("unk98"))]
    pub coins_knocked: i32,
    #[at(0x9C)]
    #[doc(alias("unk9C"))]
    pub coins_lost: i32,
    #[at(0xA0)]
    pub joystick_direction_input_count: [i32; 2],
    #[at(0xA8)]
    pub nametag_slot_id: i32,
    #[at(0xAC)]
    pub flags: PlayerFlags,
    #[at(0xAD)]
    pub more_flags: PlayerRuleFlags,
    #[at(0xAE)]
    #[doc(alias("flagsAE"))]
    pub init_flags: PlayerInitFlags,
    #[at(0xAF)]
    #[doc(alias("unkAF"))]
    pub unused_af: i8,
    #[at(0xB0)]
    pub player_entity: [Option<GObjId>; 2],
    #[at(0xB8)]
    pub struct_func: Option<CodeRef>,
    #[at(0xBC)]
    pub stale_moves: StaleMoveTable,
    /// The player's stats past the stale moves (`pl::ext_u32` and the rest).
    #[at(0xDB0)]
    #[doc(alias("xDB0"))]
    pub stats_ext: [u8; 224],
}

impl Default for StaticPlayer {
    fn default() -> Self {
        StaticPlayer {
            player_state: 0,
            ckind: 0,
            pkind: 0,
            transformed: [0; 2],
            unused_0e: 0,
            player_poses: [0; 48],
            facing_direction: 0.0,
            costume_id: 0,
            effect_color: 0,
            sub_color: 0,
            team: 0,
            pad_port: 0,
            cpu_level: 0,
            cpu_type: 0,
            handicap: 0,
            entry_delay: 0,
            kirby_copy_kind: 0,
            spawn_point: 0,
            unused_4f: 0,
            unset_ratio: 0.0,
            attack_ratio: 0.0,
            defense_ratio: 0.0,
            model_scale: 0.0,
            staminas: [0; 8],
            falls: [0; 2],
            kos_by_player: [0; 6],
            match_frame_count: 0,
            self_destructs: 0,
            stocks: 0,
            unused_8f: 0,
            current_coins: 0,
            total_coins: 0,
            coins_knocked: 0,
            coins_lost: 0,
            joystick_direction_input_count: [0; 2],
            nametag_slot_id: 0,
            flags: Default::default(),
            more_flags: Default::default(),
            init_flags: Default::default(),
            unused_af: 0,
            player_entity: [None; 2],
            struct_func: None,
            stale_moves: Default::default(),
            stats_ext: [0; 224],
        }
    }
}
