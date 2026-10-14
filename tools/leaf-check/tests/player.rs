//! The player blocks' accessors (`melee/pl/player.c`) against ssbm-rs's ports: from real VS
//! states (the captures `vs.rs` reads), random sequences of reads and writes run on both sides,
//! comparing every value read and the six blocks after each step, and the whole state (fighters
//! included) at the end. Tests skip without the captures; `vs.rs` says how to make them.

use melee_local_data::ssbm_rs_local;
use std::path::PathBuf;

use leaf_check::fp::{FpMode, set_fp_mode};
use leaf_check::{MODES, Rand, arg, handle, hsd_reference};
use melee_capture::compare::Comparer;
use melee_capture::import::{Importer, Maps, sym};
use melee_capture::{Capture, DataMap, MEM1, disc_index, find_archives_in};
use melee_data::archive::{Archives, DataRef};
use melee_data::disc::Disc;
use melee_sim::game::mirror::{Cx, Mem, Mirror};
use melee_sim::game::state::{Game, Running};
use melee_sim::game::structs::_StaticPlayer;
use melee_sim::hsd::gobj::GObjId;
use melee_sim::vec::Vec3;
use ssbm_rt::{Ctx, Handle};
use ssbm_types::fns;

const DEFAULT_CALLS: &str = "calls/match";

/// The states the sequences start from: the match's frame, as it plays.
const BASE: &str = "fn_8016CFE0";

fn bases() -> Vec<PathBuf> {
    let dir: PathBuf = std::env::var("MATCH_CALLS")
        .unwrap_or_else(|_| ssbm_rs_local(DEFAULT_CALLS))
        .into();
    let mut found: Vec<PathBuf> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .flat_map(melee_capture::pack::entries)
        .filter(|p| {
            let n = p.file_name().unwrap().to_string_lossy();
            n.starts_with(&format!("{BASE}-call")) && n.ends_with(".call")
        })
        .collect();
    found.sort();
    if found.is_empty() {
        eprintln!("no captures of {BASE} in {}: skipping", dir.display());
    }
    found
}

/// Compares one block's handles through the importer's pairs.
struct BlockCx<'a> {
    maps: &'a Maps,
    diffs: Vec<String>,
}

impl Cx for BlockCx<'_> {
    fn import_handle(&mut self, _: &'static str, _: u32) -> Option<u32> {
        unreachable!()
    }
    fn import_data(&mut self, _: u32) -> Option<DataRef> {
        unreachable!()
    }
    fn compare_handle(&mut self, path: &str, kind: &'static str, ours: Option<u32>, theirs: u32) {
        // A block holds GObjs only (its fighters).
        assert_eq!(kind, "GObjId", "{path}");
        let t = (theirs != 0).then(|| self.maps.gobjs.get(&theirs).map(|g| g.0));
        if t.flatten() != ours {
            self.diffs
                .push(format!("{path}: {kind} ours {ours:?}, theirs {theirs:#x}"));
        }
    }
    fn compare_data(&mut self, _: &str, _: Option<DataRef>, _: u32) {}
    fn mismatch(&mut self, path: &str, ours: String, theirs: String) {
        self.diffs
            .push(format!("{path}: ours {ours}, theirs {theirs}"));
    }
}

/// Where the test puts vectors passed by pointer, in ssbm-rs's memory: free scratch.
const VEC_AT: u32 = leaf_check::SCRATCH;

fn put_vec(ctx: &Ctx, v: Vec3) {
    leaf_check::put_vec(ctx, VEC_AT, v);
}

fn get_vec(ctx: &Ctx) -> Vec<u32> {
    leaf_check::read_f32s(ctx, VEC_AT, 3)
}

/// A value read on both sides.
#[derive(Debug, PartialEq)]
enum Got {
    None,
    I(i64),
    /// A float's bits as the original holds it in a register.
    F(u64),
    V(Vec<u32>),
    G(Option<GObjId>),
}

fn f(v: f32) -> Got {
    Got::F(arg(v).to_bits())
}

/// One random step: a read or write of a random slot, run on both sides.
fn step(ctx: &Ctx, r: &mut Running, maps: &Maps, rand: &mut Rand) -> (String, Got, Got) {
    let slot = rand.below(6) as i32;
    let sub = rand.below(2) as i32;
    let small = |rand: &mut Rand| rand.below(8) as i32 - 1;
    let flt = |rand: &mut Rand| match rand.below(8) {
        0 => [0.0, -0.0, 1.0, -1.0, f32::NAN][rand.below(5) as usize],
        1 => f32::from_bits(rand.bits() as u32),
        _ => rand.range(-200.0, 200.0),
    };
    let vec = |rand: &mut Rand| Vec3::new(flt(rand), flt(rand), flt(rand));
    let g = |a: u32| (a != 0).then(|| maps.gobjs[&a]);
    let gobj = |h: ssbm_types::records::HSD_GObj| g(h.addr());
    let game = &mut *r.game;
    let i = |v: i32| Got::I(i64::from(v));
    let u = |v: u32| Got::I(i64::from(v));
    let op = rand.below(101);
    let name = format!("op {op} slot {slot} sub {sub}");
    let (ours, theirs) = match op {
        0 => (
            i(game.player_state(slot)),
            i(fns::Player_GetPlayerState(ctx, slot)),
        ),
        1 => (
            i(game.player_ckind(slot)),
            i(fns::Player_GetPlayerCharacter(ctx, slot)),
        ),
        2 => (
            i(game.player_pkind(slot)),
            i(fns::Player_GetPlayerSlotType(ctx, slot)),
        ),
        3 => {
            let s = small(rand);
            (
                i(game.player_kind(slot, s)),
                i(fns::Player_8003248C(ctx, slot, s)),
            )
        }
        4 => {
            let s = small(rand);
            (
                i(game.player_ftkind(slot, s)),
                i(fns::Player_80032610(ctx, slot, s).into()),
            )
        }
        5 => {
            let k = rand.below(33) as i32;
            let s = small(rand);
            (
                i(melee_sim::game::pl::player::ckind_ftkind(k, s)),
                i(fns::Player_800325C8(ctx, k, s).into()),
            )
        }
        6 => {
            fns::Player_LoadPlayerCoords(ctx, slot, handle(ctx, VEC_AT));
            (
                Got::V(leaf_check::vec_bits(game.player_pos(slot))),
                Got::V(get_vec(ctx)),
            )
        }
        7 => {
            fns::Player_GetSpawnPlatformPos(ctx, slot, handle(ctx, VEC_AT));
            (
                Got::V(leaf_check::vec_bits(game.player_spawn_platform_pos(slot))),
                Got::V(get_vec(ctx)),
            )
        }
        8 => {
            fns::Player_GetSomePos(ctx, slot, handle(ctx, VEC_AT));
            (
                Got::V(leaf_check::vec_bits(game.player_some_pos(slot))),
                Got::V(get_vec(ctx)),
            )
        }
        9 => (
            i(game.player_unk4e(slot)),
            i(fns::Player_80032F30(ctx, slot)),
        ),
        10 => (
            f(game.player_facing(slot)),
            Got::F(fns::Player_GetFacingDirection(ctx, slot).to_bits()),
        ),
        11 => (
            u(game.player_costume(slot)),
            u(fns::Player_GetCostumeId(ctx, slot)),
        ),
        12 => (
            u(game.player_sub_color(slot).into()),
            u(fns::Player_GetSubColor(ctx, slot).into()),
        ),
        13 => (i(game.player_team(slot)), i(fns::Player_GetTeam(ctx, slot))),
        14 => (
            i(game.player_port(slot)),
            i(fns::Player_GetPadPort(ctx, slot)),
        ),
        15 => (
            i(game.player_cpu_level(slot)),
            i(fns::Player_GetCpuLevel(ctx, slot)),
        ),
        16 => (
            i(game.player_cpu_type(slot)),
            i(fns::Player_GetCpuType(ctx, slot)),
        ),
        17 => (
            i(game.player_handicap(slot)),
            i(fns::Player_GetHandicap(ctx, slot)),
        ),
        18 => (
            f(game.player_unk50(slot)),
            Got::F(fns::Player_GetUnk50(ctx, slot).to_bits()),
        ),
        19 => (
            f(game.player_attack_ratio(slot)),
            Got::F(fns::Player_GetAttackRatio(ctx, slot).to_bits()),
        ),
        20 => (
            f(game.player_defense_ratio(slot)),
            Got::F(fns::Player_GetDefenseRatio(ctx, slot).to_bits()),
        ),
        21 => (
            f(game.player_model_scale(slot)),
            Got::F(fns::Player_GetModelScale(ctx, slot).to_bits()),
        ),
        22 => (
            i(game.player_stocks(slot)),
            i(fns::Player_GetStocks(ctx, slot)),
        ),
        23 => (i(game.player_p1_stocks()), i(fns::Player_GetP1Stock(ctx))),
        24 => (
            i(game.player_coins(slot)),
            i(fns::Player_GetCoins(ctx, slot)),
        ),
        25 => (
            i(game.player_total_coins(slot)),
            i(fns::Player_GetTotalCoins(ctx, slot)),
        ),
        26 => (
            i(game.player_unk98(slot)),
            i(fns::Player_GetUnk98(ctx, slot)),
        ),
        27 => (
            i(game.player_unk9c(slot)),
            i(fns::Player_GetUnk9C(ctx, slot)),
        ),
        28 => (
            Got::G(game.player_entity(slot)),
            Got::G(gobj(fns::Player_GetEntity(ctx, slot))),
        ),
        29 => (
            Got::G(game.player_entity_at(slot, sub)),
            Got::G(gobj(fns::Player_GetEntityAtIndex(ctx, slot, sub))),
        ),
        30 => (
            i(game.player_damage(slot)),
            i(fns::Player_GetDamage(ctx, slot)),
        ),
        31 => (
            i(game.player_stamina(slot)),
            i(fns::Player_GetOtherStamina(ctx, slot)),
        ),
        32 => (
            i(game.player_remaining_hp(slot)),
            i(fns::Player_GetRemainingHP(ctx, slot)),
        ),
        33 => (
            i(game.player_remaining_hp_at(slot, sub)),
            i(fns::Player_GetRemainingHPByIndex(ctx, slot, sub)),
        ),
        34 => (
            i(game.player_more_flags_bit0(slot).into()),
            i(fns::Player_GetMoreFlagsBit0(ctx, slot)),
        ),
        35 => (
            i(game.player_more_flags_bit1(slot).into()),
            i(fns::Player_GetMoreFlagsBit1(ctx, slot)),
        ),
        36 => (
            i(game.player_more_flags_bit2(slot).into()),
            i(fns::Player_GetMoreFlagsBit2(ctx, slot)),
        ),
        37 => (
            i(game.player_more_flags_bit3(slot).into()),
            i(fns::Player_GetMoreFlagsBit3(ctx, slot)),
        ),
        38 => (
            i(game.player_more_flags_bit4(slot).into()),
            i(fns::Player_GetMoreFlagsBit4(ctx, slot)),
        ),
        39 => (
            i(game.player_more_flags_bit5(slot).into()),
            i(fns::Player_GetMoreFlagsBit5(ctx, slot)),
        ),
        40 => (
            i(game.player_more_flags_bit6(slot).into()),
            i(fns::Player_GetMoreFlagsBit6(ctx, slot)),
        ),
        41 => (
            u(game.player_flags_ae_bit0(slot).into()),
            u(fns::Player_GetFlagsAEBit0(ctx, slot).into()),
        ),
        42 => (
            u(game.player_flags_ae_bit1(slot).into()),
            u(fns::Player_GetFlagsAEBit1(ctx, slot).into()),
        ),
        43 => (
            i(game.player_falls(slot)),
            i(fns::Player_GetFalls(ctx, slot)),
        ),
        44 => (
            i(game.player_falls_at(slot, sub)),
            i(fns::Player_GetFallsByIndex(ctx, slot, sub)),
        ),
        45 => {
            let k = rand.below(6) as i32;
            (
                u(game.player_kos(slot, k)),
                u(fns::Player_GetKOsByPlayerIndex(ctx, slot, k) as u32),
            )
        }
        46 => (
            u(game.player_match_frame_count(slot)),
            u(fns::Player_GetMatchFrameCount(ctx, slot)),
        ),
        47 => (
            u(game.player_self_destructs(slot)),
            u(fns::Player_GetSelfDestructs(ctx, slot)),
        ),
        48 => (
            i(game.player_rumbles(slot).into()),
            i(fns::Player_800353BC(ctx, slot)),
        ),
        49 => {
            let s = small(rand);
            (
                i(game.rumble_enabled(slot, s).into()),
                i(fns::Player_8003544C(ctx, slot, s)),
            )
        }
        50 => (
            u(game.player_nametag(slot).into()),
            u(fns::Player_GetNametagSlotID(ctx, slot).into()),
        ),
        51 => (
            i(game.player_flags_bit1(slot).into()),
            i(fns::Player_GetFlagsBit1(ctx, slot)),
        ),
        52 => (
            i(game.player_flags_bit3(slot).into()),
            i(fns::Player_GetFlagsBit3(ctx, slot)),
        ),
        53 => (
            i(game.player_flags_bit4(slot).into()),
            i(fns::Player_GetFlagsBit4(ctx, slot)),
        ),
        54 => (
            u(game.player_is_metal(slot).into()),
            u(fns::Player_GetFlagsBit5(ctx, slot).into()),
        ),
        55 => (
            u(game.player_flags_bit6(slot).into()),
            u(fns::Player_GetFlagsBit6(ctx, slot).into()),
        ),
        56 => (
            u(game.player_flags_bit7(slot).into()),
            u(fns::Player_GetFlagsBit7(ctx, slot).into()),
        ),
        57 => (
            i(game.player_unk4d(slot)),
            i(fns::Player_GetUnk4D(ctx, slot)),
        ),
        58 => (
            i(game.player_unk4c(slot)),
            i(fns::Player_GetUnk4C(ctx, slot)),
        ),
        59 => (
            u(game.player_unk45(slot)),
            u(fns::Player_GetUnk45(ctx, slot)),
        ),
        60 => (
            i(game.player_stick_inputs(slot, sub)),
            i(fns::Player_GetJoystickCountByIndex(ctx, slot, sub)),
        ),
        61 => (
            i(game.player_fighter_kind(slot)),
            i(fns::Player_80036394(ctx, slot)),
        ),
        62 => (
            i(game.player_last_attacker(slot)),
            i(fns::Player_80036428(ctx, slot)),
        ),
        63 => {
            let s = rand.below(2) == 0;
            (
                i(game.player_800319c4(slot, s).into()),
                i(fns::Player_800319C4(ctx, slot, s.into())),
            )
        }
        // Writes.
        64 => {
            let v = rand.below(33) as i32;
            game.set_player_ckind(slot, v);
            fns::Player_SetPlayerCharacter(ctx, slot, v);
            (Got::None, Got::None)
        }
        65 => {
            let v = rand.below(4) as i32;
            game.set_player_pkind(slot, v);
            fns::Player_SetSlottype(ctx, slot, v);
            (Got::None, Got::None)
        }
        66 => {
            let v = vec(rand);
            game.set_player_pos(slot, v);
            put_vec(ctx, v);
            fns::Player_80032768(ctx, slot, handle(ctx, VEC_AT));
            (Got::None, Got::None)
        }
        67 => {
            let v = vec(rand);
            game.set_player_entity_pos(slot, sub, v);
            put_vec(ctx, v);
            fns::Player_80032828(ctx, slot, sub, handle(ctx, VEC_AT));
            (Got::None, Got::None)
        }
        68 => {
            let v = vec(rand);
            game.set_player_spawn_platform_pos(slot, v);
            put_vec(ctx, v);
            fns::Player_SetSpawnPlatformPos(ctx, slot, handle(ctx, VEC_AT));
            (Got::None, Got::None)
        }
        69 => {
            let v = vec(rand);
            game.set_player_some_pos(slot, v);
            put_vec(ctx, v);
            fns::Player_SetSomePos(ctx, slot, handle(ctx, VEC_AT));
            (Got::None, Got::None)
        }
        70 => {
            let v = rand.bits() as i32;
            game.set_player_unk4e(slot, v);
            fns::Player_80032FA4(ctx, slot, v);
            (Got::None, Got::None)
        }
        71 => {
            let v = flt(rand);
            game.set_player_facing(slot, v);
            fns::Player_SetFacingDirection(ctx, slot, arg(v));
            (Got::None, Got::None)
        }
        72 => {
            let (b, v) = (rand.below(2) == 0, flt(rand));
            game.set_player_facing_if(slot, b, v);
            fns::Player_SetFacingDirectionConditional(ctx, slot, b.into(), arg(v));
            (Got::None, Got::None)
        }
        73 => {
            let v = rand.bits() as i32;
            game.set_player_costume(slot, v);
            fns::Player_SetCostumeId(ctx, slot, v);
            (Got::None, Got::None)
        }
        74 => {
            let v = rand.bits() as i8;
            game.set_player_sub_color(slot, v);
            fns::Player_SetSubColor(ctx, slot, v);
            game.set_player_team(slot, v);
            fns::Player_SetTeam(ctx, slot, v);
            game.set_player_handicap(slot, v);
            fns::Player_SetHandicap(ctx, slot, v);
            (Got::None, Got::None)
        }
        75 => {
            let v = rand.below(4) as i32;
            game.set_player_port(slot, v);
            fns::Player_SetPadPort(ctx, slot, v);
            (Got::None, Got::None)
        }
        76 => {
            let (a, d, m) = (flt(rand), flt(rand), flt(rand));
            game.set_player_attack_ratio(slot, a);
            fns::Player_SetAttackRatio(ctx, slot, arg(a));
            game.set_player_defense_ratio(slot, d);
            fns::Player_SetDefenseRatio(ctx, slot, arg(d));
            game.set_player_model_scale(slot, m);
            fns::Player_SetModelScale(ctx, slot, arg(m));
            (Got::None, Got::None)
        }
        77 => {
            let v = small(rand);
            game.set_player_stocks(slot, v);
            fns::Player_SetStocks(ctx, slot, v);
            (Got::None, Got::None)
        }
        78 => {
            game.player_lose_stock(slot);
            fns::Player_LoseStock(ctx, slot);
            (Got::None, Got::None)
        }
        79 => {
            let v: [i32; 4] = core::array::from_fn(|_| rand.bits() as i32);
            game.set_player_coins(slot, v[0]);
            fns::Player_SetCoins(ctx, slot, v[0]);
            game.set_player_total_coins(slot, v[1]);
            fns::Player_SetTotalCoins(ctx, slot, v[1]);
            game.set_player_unk98(slot, v[2]);
            fns::Player_SetUnk98(ctx, slot, v[2]);
            game.set_player_unk9c(slot, v[3]);
            fns::Player_SetUnk9C(ctx, slot, v[3]);
            (Got::None, Got::None)
        }
        80 => {
            let (a, b) = (rand.below(2) as i32, rand.below(2) as i32);
            game.player_swap_transformed(slot, a, b);
            fns::Player_SwapTransformedStates(ctx, slot, a, b);
            (Got::None, Got::None)
        }
        81 => {
            let v = rand.bits() as i32;
            game.set_player_damage_at(slot, sub, v);
            fns::Player_SetHPByIndex(ctx, slot, sub, v);
            (Got::None, Got::None)
        }
        82 => {
            let v = rand.bits() as i32;
            game.set_player_stamina(slot, v);
            fns::Player_SetOtherStamina(ctx, slot, v);
            (Got::None, Got::None)
        }
        83 => {
            let v = rand.bits() as u8;
            match rand.below(6) {
                0 => {
                    game.set_player_more_flags_bit1(slot, v);
                    fns::Player_SetMoreFlagsBit1(ctx, slot, v);
                }
                1 => {
                    game.set_player_more_flags_bit2(slot, v);
                    fns::Player_SetMoreFlagsBit2(ctx, slot, v);
                }
                2 => {
                    game.set_player_more_flags_bit3(slot, v);
                    fns::Player_SetMoreFlagsBit3(ctx, slot, v);
                }
                3 => {
                    game.set_player_more_flags_bit4(slot, v);
                    fns::Player_SetMoreFlagsBit4(ctx, slot, v);
                }
                4 => {
                    game.set_player_more_flags_bit5(slot, v);
                    fns::Player_SetMoreFlagsBit5(ctx, slot, v);
                }
                _ => {
                    game.set_player_more_flags_bit6(slot, v);
                    fns::Player_SetMoreFlagsBit6(ctx, slot, v);
                }
            }
            (Got::None, Got::None)
        }
        84 => {
            let v = rand.bits() as u8;
            if rand.below(2) == 0 {
                game.set_player_flags_ae_bit0(slot, v);
                fns::Player_SetFlagsAEBit0(ctx, slot, v);
            } else {
                // Its port returns whatever its register held, as the original does; the
                // value isn't compared.
                game.set_player_flags_ae_bit1(slot, v);
                let _ = fns::Player_SetFlagsAEBit1(ctx, slot, v);
            }
            (Got::None, Got::None)
        }
        85 => {
            let v = rand.bits() as i32;
            if rand.below(2) == 0 {
                game.set_player_falls(slot, v);
                fns::Player_SetFalls(ctx, slot, v);
            } else {
                game.set_player_falls_at(slot, sub, v);
                fns::Player_SetFallsByIndex(ctx, slot, sub, v);
            }
            (Got::None, Got::None)
        }
        86 => {
            let s = small(rand);
            game.update_player_match_frame_count(slot, s);
            fns::Player_UpdateMatchFrameCount(ctx, slot, s);
            (Got::None, Got::None)
        }
        87 => {
            let v = match rand.below(3) {
                0 => rand.bits() as u32,
                1 => 0xFFFF,
                _ => rand.below(0x10000) as u32,
            };
            game.set_player_self_destructs(slot, v);
            fns::Player_SetSelfDestructs(ctx, slot, v);
            (Got::None, Got::None)
        }
        88 => {
            let b = rand.below(2) == 0;
            game.set_player_rumble(slot, b);
            fns::Player_SetFlagsBit0(ctx, slot, b.into());
            (Got::None, Got::None)
        }
        89 => {
            if rand.below(2) == 0 {
                game.set_player_flags_bit1(slot);
                fns::Player_SetFlagsBit1(ctx, slot);
            } else {
                game.unset_player_flags_bit1(slot);
                fns::Player_UnsetFlagsBit1(ctx, slot);
            }
            (Got::None, Got::None)
        }
        90 => {
            let v = rand.bits() as u8;
            match rand.below(4) {
                0 => {
                    game.set_player_flags_bit3(slot, v);
                    fns::Player_SetFlagsBit3(ctx, slot, v);
                }
                1 => {
                    game.set_player_is_metal(slot, v);
                    fns::Player_SetFlagsBit5(ctx, slot, v);
                }
                2 => {
                    game.set_player_flags_bit6(slot, v);
                    fns::Player_SetFlagsBit6(ctx, slot, v);
                }
                _ => {
                    game.set_player_flags_bit7(slot, v);
                    fns::Player_SetFlagsBit7(ctx, slot, v);
                }
            }
            (Got::None, Got::None)
        }
        91 => {
            let v = rand.bits() as i32;
            game.set_player_nametag(slot, v);
            fns::Player_SetNametagSlotID(ctx, slot, v);
            game.set_player_unk45(slot, v);
            fns::Player_SetUnk45(ctx, slot, v);
            (Got::None, Got::None)
        }
        92 => {
            let v = rand.bits() as u8;
            game.set_player_unk4d(slot, v as i8);
            fns::Player_SetUnk4D(ctx, slot, v as i8);
            game.set_player_unk4c(slot, v);
            fns::Player_SetUnk4C(ctx, slot, v);
            (Got::None, Got::None)
        }
        93 => {
            game.count_player_stick_input(slot, sub);
            fns::Player_UpdateJoystickCountByIndex(ctx, slot, sub);
            (Got::None, Got::None)
        }
        94 => {
            game.reset_player(slot);
            fns::Player_InitOrResetPlayer(ctx, slot);
            (Got::None, Got::None)
        }
        95 => {
            game.player_80031fb0(slot, sub);
            fns::Player_80031FB0(ctx, slot, sub);
            (Got::None, Got::None)
        }
        96 => {
            let v = small(rand);
            match rand.below(3) {
                0 => {
                    r.player_800366dc(slot, v);
                    fns::Player_800366DC(ctx, slot, v);
                }
                1 => {
                    let x = flt(rand);
                    r.player_80036790(slot, x);
                    fns::Player_80036790(ctx, slot, arg(x));
                }
                _ => {
                    r.player_80036844(slot, v);
                    fns::Player_80036844(ctx, slot, v);
                }
            }
            (Got::None, Got::None)
        }
        97 => {
            r.player_enable_input(slot);
            fns::Player_80031848(ctx, slot);
            (Got::None, Got::None)
        }
        98 => {
            let f = (rand.below(2) == 0).then(|| rand.bits() as u32 & !3 | 0x8000_0000);
            game.set_player_struct_func(slot, f.map(melee_sim::game::handles::CodeRef));
            fns::Player_SetStructFunc(ctx, slot, handle(ctx, f.unwrap_or(0)));
            (Got::None, Got::None)
        }
        // These read the slot's current fighter, which must be there.
        99 if game.player_entity(slot).is_some() => (
            i(game.player_asleep(slot).into()),
            i(fns::Player_8003219C(ctx, slot)),
        ),
        100 if game.player_entity(slot).is_some() => (
            i(game.player_80036058(slot).into()),
            i(fns::Player_80036058(ctx, slot)),
        ),
        _ => (Got::None, Got::None),
    };
    (name, ours, theirs)
}

#[test]
fn player_accessors() {
    let paths = bases();
    let Some(ctx) = hsd_reference() else { return };
    if paths.is_empty() {
        return;
    }
    let disc = disc_index(&Disc::open(std::env::var("SSBM_DISC").unwrap()).unwrap());
    let mut rand = Rand(0x0091_A7E5_u64);
    let mut steps = 0;
    for path in paths.iter().take(3) {
        let cap = Capture::read(path);
        for mode in MODES {
            set_fp_mode(mode);
            let mut archives = Archives::default();
            let data = DataMap {
                loaded: find_archives_in(&disc, &cap.call.mem1, &mut archives),
            };
            let mut imp = Importer::new(&cap, &data);
            imp.world();
            imp.stage();
            imp.match_state();
            let Importer { mut game, maps, .. } = imp;
            let before = {
                let mut c = Comparer::new(&cap.call.mem1, &data, &game, maps.clone());
                c.world();
                c.diffs
            };
            cap.call.load(&ctx);
            let players = sym("player_slots");
            let mut block = vec![0u8; 6 * _StaticPlayer::SIZE as usize];
            for n in 0..2000 {
                let mut r = Running {
                    game: &mut game,
                    data: &archives,
                };
                let (name, ours, theirs) = step(&ctx, &mut r, &maps, &mut rand);
                assert_eq!(ours, theirs, "{} step {n}: {name}", path.display());
                let _ = ctx.mem.read_bytes(players, &mut block);
                let mem = Mem {
                    bytes: &block,
                    base: players,
                };
                let mut cx = BlockCx {
                    maps: &maps,
                    diffs: Vec::new(),
                };
                for (k, p) in game.players.iter().enumerate() {
                    p.compare(
                        &mem,
                        players + k as u32 * _StaticPlayer::SIZE,
                        &format!("player {k}"),
                        &mut cx,
                    );
                }
                assert!(
                    cx.diffs.is_empty(),
                    "{} step {n}: {name}: {:?}",
                    path.display(),
                    &cx.diffs[..cx.diffs.len().min(10)]
                );
                steps += 1;
            }
            // The fighters the accessors wrote to, and the rest of the state.
            let mut after = vec![0; cap.call.mem1.len()];
            let _ = ctx.mem.read_bytes(MEM1, &mut after);
            let mut cmp = Comparer::new(&after, &data, &game, maps.clone());
            cmp.world();
            let new: Vec<&String> = cmp.diffs.iter().filter(|d| !before.contains(d)).collect();
            assert!(
                new.is_empty(),
                "{}: after the steps: {:?}",
                path.display(),
                &new[..new.len().min(10)]
            );
        }
    }
    set_fp_mode(FpMode::Hardware);
    eprintln!("player accessors: {steps} steps agree");
}

/// One random step over the match's and the scene loop's small accessors, run on both sides.
fn gm_step(ctx: &Ctx, game: &mut Game, rand: &mut Rand) -> (String, Got, Got) {
    let slot = rand.below(6) as i32;
    let i = |v: i32| Got::I(i64::from(v));
    let u = |v: u32| Got::I(i64::from(v));
    let b = |v: bool| Got::I(i64::from(v));
    let bit = rand.below(8) as u32;
    let port = rand.below(5) as u8;
    let op = rand.below(52);
    let name = format!("op {op} slot {slot} bit {bit} port {port}");
    let (ours, theirs) = match op {
        0 => (i(game.vs_item_freq()), i(fns::gm_8016AE80(ctx))),
        1 => (f(game.vs_unk34()), Got::F(fns::gm_8016AE94(ctx).to_bits())),
        2 => (u(game.vs_timer_seconds()), u(fns::gm_8016AEEC(ctx))),
        3 => (u(game.vs_frame_count()), u(fns::gm_GetFrameCount(ctx))),
        4 => (u(game.vs_stkind().into()), u(fns::gm_GetStKind(ctx).into())),
        5 => (b(game.vs_is_stock()), i(fns::gm_8016B094(ctx))),
        6 => (b(game.vs_is_coins()), i(fns::gm_8016B0B4(ctx))),
        7 => (b(game.vs_friendly_fire()), i(fns::gm_8016B0D4(ctx))),
        8 => (b(game.vs_x4_2()), i(fns::gm_8016B0E8(ctx))),
        9 => (b(game.vs_single_button()), i(fns::gm_8016B0FC(ctx))),
        10 => (b(game.vs_timer_counts_up()), i(fns::gm_8016B110(ctx))),
        11 => (b(game.vs_x2_1()), i(fns::gm_8016B124(ctx))),
        12 => (b(game.vs_is_ffa()), i(fns::gm_8016B14C(ctx))),
        13 => (b(game.vs_is_teams()), i(fns::gm_8016B168(ctx))),
        14 => (b(game.vs_is_singleplayer()), i(fns::gm_8016B184(ctx))),
        15 => (b(game.vs_x9_is_1()), i(fns::gm_8016B1A8(ctx))),
        16 => (b(game.vs_x5_2()), i(fns::gm_8016B1C4(ctx))),
        17 => (b(game.vs_x5_3()), i(fns::gm_8016B1D8(ctx))),
        18 => (b(game.vs_in_play()), i(fns::gm_8016B1EC(ctx))),
        19 => (b(game.vs_untimed()), i(fns::gm_8016B204(ctx))),
        20 => (u(game.vs_x6().into()), i(fns::gm_8016B238(ctx))),
        21 => (
            f(game.vs_damage_ratio()),
            Got::F(fns::gm_8016B248(ctx).to_bits()),
        ),
        22 => (
            b(game.vs_fighter_x4_b3(slot)),
            i(fns::gm_8016B258(ctx, slot)),
        ),
        23 => {
            game.vs_terminate();
            fns::gm_8016B328(ctx);
            (Got::None, Got::None)
        }
        24 => {
            let v = rand.bits() as i32;
            game.vs_set_respawn_damage(slot, v);
            fns::fn_8016B388(ctx, slot, v);
            (Got::None, Got::None)
        }
        25 => (b(game.is_1p_run()), i(fns::gm_8016B3D8(ctx))),
        26 => (i(game.player_count()), i(fns::gm_8016B558(ctx))),
        27 => (u(game.vs_unk_c().into()), u(fns::fn_8016B728(ctx).into())),
        28 => {
            let ours = std::ptr::eq(game.vs_results(), &game.vs.state.x24C);
            (
                b(ours),
                b(fns::gm_8016B774(ctx).addr() == sym("controller") + 0x24C),
            )
        }
        29 => (b(game.led_from_first(slot)), i(fns::fn_80171B00(ctx, slot))),
        30 => (
            b(game.trailed_from_first(slot)),
            i(fns::fn_80171B2C(ctx, slot)),
        ),
        31 => (b(game.vs_has_x58()), i(fns::gm_8016A944(ctx))),
        32 => {
            let p = port.min(4);
            (
                Got::I(game.buttons_pressed(p) as i64),
                Got::I(fns::gm_GetButtonsPressed(ctx, p) as i64),
            )
        }
        33 => (
            Got::I(game.buttons_triggered(port) as i64),
            Got::I(fns::gm_GetButtonsTriggered(ctx, port) as i64),
        ),
        34 => (
            Got::I(game.buttons_repeated(port) as i64),
            Got::I(fns::gm_801A36C0(ctx, port) as i64),
        ),
        35 => {
            let v = rand.below(40) as i32;
            game.delay_repeat(port.into(), v);
            fns::gm_801A36E0(ctx, port.into(), v);
            (Got::None, Got::None)
        }
        36 => (
            b(game.db_pause_flag(bit)),
            b(fns::gm_GetDbPauseFlag(ctx, bit as i32) != 0),
        ),
        37 => (u(game.db_pause_flags().into()), i(fns::gm_801A4624(ctx))),
        38 => {
            game.set_db_pause_flag(bit);
            fns::gm_SetDbPauseFlag(ctx, bit as i32);
            (Got::None, Got::None)
        }
        39 => {
            game.clear_db_pause_flag(bit);
            fns::gm_ClearDbPauseFlag(ctx, bit as i32);
            (Got::None, Got::None)
        }
        40 => (
            b(game.db_frame_step(bit)),
            b(fns::gm_801A46B8(ctx, bit as i32) != 0),
        ),
        41 => {
            game.default_db_pause_handlers();
            fns::gm_801A4B1C(ctx);
            (Got::None, Got::None)
        }
        42 => {
            let v = rand.below(2) as i32;
            game.set_clock_mask(v);
            fns::gm_801A4B50(ctx, v);
            (Got::None, Got::None)
        }
        43 => {
            if rand.below(2) == 0 {
                game.end_scene();
                fns::gm_801A4B60(ctx);
            } else {
                game.end_scene_now();
                fns::gm_801A4B74(ctx);
            }
            (Got::None, Got::None)
        }
        44 => (u(game.scene_ticks()), u(fns::gm_801A4BA8(ctx))),
        45 => (u(game.scene_game_ticks()), u(fns::gm_801A4BB8(ctx))),
        46 => {
            let (a, c) = (rand.bits() as u32 & !3, rand.bits() as u32 & !3);
            game.set_db_pause_handlers(a, c);
            fns::gm_SetDbPauseInputHandlers(ctx, handle(ctx, a), handle(ctx, c));
            let p = rand.bits() as u32 & !3;
            game.set_pre_gobj_proc(p);
            fns::gm_SetPreGObjProcCallback(ctx, handle(ctx, p));
            (Got::None, Got::None)
        }
        47 => {
            if rand.below(8) == 0 {
                game.init_attack_instance();
                fns::plStale_InitAttackInstance(ctx);
                (Got::None, Got::None)
            } else {
                (
                    u(game.next_attack_instance().into()),
                    u(fns::plStale_IncrementAttackInstance(ctx).into()),
                )
            }
        }
        48 => {
            if rand.below(8) == 0 {
                game.init_attack_records();
                fns::plAttack_80037590(ctx);
                (Got::None, Got::None)
            } else {
                (
                    u(game.next_attack_record().into()),
                    u(fns::plAttack_80037B08(ctx).into()),
                )
            }
        }
        49 => {
            // A game speed for the frame clocks, as `fn_8016E730` and `gm_SetGameSpeed` set it.
            let speed = [1.0, 0.5, 2.0, 1.5, 0.25][rand.below(5) as usize];
            let t = melee_sim::lb::pacing::ticks_at_speed(speed);
            game.pacing.request(t);
            fns::lb_80019880(ctx, t);
            game.pacing.update();
            fns::lb_80019628(ctx);
            (Got::None, Got::None)
        }
        50 => {
            game.pacing.advance();
            if game.pacing.due(0) {
                game.pads.renew_game();
                game.pads.renew_copy();
            }
            fns::lb_80019900(ctx);
            (Got::None, Got::None)
        }
        _ => {
            let k = rand.below(2) as i32;
            (b(game.pacing.due(k as usize)), i(fns::lb_80019A30(ctx, k)))
        }
    };
    (name, ours, theirs)
}

#[test]
fn gm_accessors() {
    let paths = bases();
    let Some(ctx) = hsd_reference() else { return };
    if paths.is_empty() {
        return;
    }
    // The frame clocks' polling alarm and sampling rate are the platform's: stand-ins on
    // ssbm-rs's side, and nothing on ours.
    for name in [
        "PADSetSamplingRate",
        "OSCancelAlarm",
        "OSCreateAlarm",
        "OSSetPeriodicAlarm",
    ] {
        ctx.register(sym(name), |_| {});
    }
    let disc = disc_index(&Disc::open(std::env::var("SSBM_DISC").unwrap()).unwrap());
    let mut rand = Rand(0x006A_CCE5_u64);
    let mut steps = 0;
    for path in paths.iter().take(3) {
        let cap = Capture::read(path);
        set_fp_mode(FpMode::Hardware);
        let mut archives = Archives::default();
        let data = DataMap {
            loaded: find_archives_in(&disc, &cap.call.mem1, &mut archives),
        };
        let mut imp = Importer::new(&cap, &data);
        imp.world();
        imp.stage();
        imp.match_state();
        let Importer { mut game, maps, .. } = imp;
        cap.call.load(&ctx);
        for n in 0..3000 {
            let (name, ours, theirs) = gm_step(&ctx, &mut game, &mut rand);
            assert_eq!(ours, theirs, "{} step {n}: {name}", path.display());
            if n % 50 == 49 {
                let mut after = vec![0; cap.call.mem1.len()];
                let _ = ctx.mem.read_bytes(MEM1, &mut after);
                let mut cmp = Comparer::new(&after, &data, &game, maps.clone());
                cmp.match_state();
                let diffs: Vec<&String> = cmp
                    .diffs
                    .iter()
                    .filter(|d| !d.starts_with("player ") && !d.contains("alarm"))
                    .collect();
                assert!(
                    diffs.is_empty(),
                    "{} step {n}: {name}: {:?}",
                    path.display(),
                    &diffs[..diffs.len().min(10)]
                );
            }
            steps += 1;
        }
    }
    eprintln!("gm accessors: {steps} steps agree");
}
