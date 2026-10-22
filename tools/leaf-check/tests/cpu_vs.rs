//! A VS match's computer players' frame (`ftCo_800B3900` with computer kind 4,
//! `game::ft::cpu_vs`, `cpu_attack`, `cpu_move`) against the original's machine code, from
//! real states: every saved call is imported and run on both sides, and the whole state
//! compared afterwards. The reference runs the original's machine code throughout
//! ([`leaf_check::run_as_machine_code`]), as `nana`'s does.
//!
//! The captures are game memory, kept out of the repository in `CPU_VS_CALLS` (default below),
//! one directory per run. ssbm-rs's runner made them on the level 1 CPU games split out of three
//! ranked replays (`local/replays/hf-cpu/`, docs/oracle.md): five of Link against a CPU Ness,
//! one on each tournament stage but Dream Land (every frame), a CPU Mewtwo against Falco on
//! Battlefield (every fifth) and a CPU Ness against Marth on Battlefield (every ninth), each
//! with the frames that ran code no check had verified before:
//!
//! ```text
//! C=local/calls/cpu-vs
//! SLIPPI_PLAYER_NAMES=0 CAPTURE=$C/hf-ness-marth-bf-69664b46-g1 CAPTURE_FROM=0 CAPTURE_EVERY=9 \
//!   CAPTURE_FUNCS=ftCo_800B3900 LOCKSTEP_DONE=local/calls/ics/done-noncpu.txt \
//!   ssbm-run-rf "$SSBM_DISC" --replay local/replays/hf-cpu/hf-ness-marth-bf-69664b46-g1.slp \
//!   --fields 3900 --port all --lockstep
//! ```
//!
//! Each is packed (`pack-calls`).
//!
//! Five more sets come from the level 9 oracle matches (`local/bench/oracle-m4`, docs/oracle.md's
//! oracle sets, `--fp hardware`): `oracle-cpu-puff-fod` (every seventh frame of the first 3000
//! fields: shields, rolls and dodges), `oracle-cpu-luigi-bf` (the same: Luigi's rules and
//! recovery), `oracle-cpu-sweep-dk-bf` (every twentieth of three Donkey Kongs at levels 1, 4 and
//! 7: his carry and the standings between CPUs), `oracle-cpu-mewtwo-dl` (every frame from field
//! 2300 to 2700: Shadow Ball charged and let go) and `oracle-cpu-zelda-bf` (every frame from 1250
//! to 1700: Din's Fire steered).
//!
//! Five more, for a CPU holding its own item and the island end beside with none in reach:
//! `oracle-cpu-peach-fod`, `oracle-cpu-link-dl` and `oracle-cpu-ylink-ys` (every frame of the
//! stretches where the CPU holds a turnip or a bomb), and `oracle-cpu-ics-fd` and
//! `oracle-cpu-sweep-samus-fd` (every frame of the stretch where a CPU's target falls under
//! Final Destination, out of reach of both its ends).
//!
//! Two more, for the stages' own CPU code: `oracle-cpu-stage-poke-floats` (every call from field
//! 1410, CPUs off camera on the ground heading for another island or their target,
//! `ftCo_800A6FC4`) and `oracle-cpu-stage-kongo-jungle-64` (every call from field 7090, Nana in
//! the barrel until it shoots her, `ftCo_800AC434`), the oracle's `cpu-stages` matches captured
//! as above with `--fp hardware` and `CAPTURE_EVERY=1`.
//!
//! `cpu_vs_changed` changes the saved frames to reach what those don't: level 9, other
//! characters' recoveries, ranged attacks, charge releases and follow-ups, an enemy entering
//! the stage, the CPU off camera, its target off the stage or under it with no island end in
//! reach (the destination then takes the stack words `ftCo_800A4BEC`'s frame left), the enemy
//! moves it dodges, its DI with no knockback, which reads the registers its frame leaves, and
//! the item it holds used from range (`ftCo_800B9A04`).

use std::collections::BTreeSet;
use std::sync::Mutex;

use leaf_check::calls::{
    Prepare, Report, Ret, Variant, check_variants, gobj_arg, peek_u32, poke_f32, poke_u32,
    saved_calls,
};

fn peek_f32(call: &Call, addr: u32) -> f32 {
    f32::from_bits(peek_u32(call, addr))
}
use ssbm_rt::capture::Call;

const CALLS: &str = "calls/cpu-vs";

fn base() -> String {
    std::env::var("CPU_VS_CALLS").unwrap_or_else(|_| melee_local_data::ssbm_rs_local(CALLS))
}

/// The capture sets, one per run.
fn dirs() -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(base()) else {
        return Vec::new();
    };
    let mut v: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.path().to_string_lossy().replace('\\', "/"))
        .collect();
    v.sort();
    v
}

/// `Fighter` offsets.
const KIND: u32 = 0x4;
const PLAYER: u32 = 0xC;
const MOTION: u32 = 0x10;
const POS: u32 = 0xB0;
const GROUND_OR_AIR: u32 = 0xE0;
/// The byte holding `x221D_b6` (entering, as a respawn), and that bit in its word at `x221C`.
const X221C: u32 = 0x221C;
const ENTERING: u32 = 0x0002_0000;
/// Knockback's speed; the byte holding `x221A_b3` (in hitstun) and that bit in its word.
const KB_VEL: u32 = 0x8C;
const X221A: u32 = 0x221A;
const HITSTUN: u32 = 0x1000_0000;
/// The held item's GObj; `x2164`, which makes a fighter untargetable when set.
const ITEM_GOBJ: u32 = 0x1974;
const X2164: u32 = 0x2164;
/// `Item::kind`.
const ITEM_KIND: u32 = 0x10;
/// `It_Kind_Capsule`, `_Box`, `_M_Ball`: a thrown item, a container, the Poké Ball.
const IT_CAPSULE: u32 = 0;
const IT_BOX: u32 = 1;
const IT_M_BALL: u32 = 34;
/// The swung items: `It_Kind_Harisen`, `_LipStick`, `_StarRod`, `_Sword`, `_Bat`, `_Parasol`.
const IT_HARISEN: u32 = 24;
const IT_LIPSTICK: u32 = 23;
const IT_STARROD: u32 = 22;
const IT_SWORD: u32 = 12;
const IT_BAT: u32 = 11;
const IT_PARASOL: u32 = 13;
/// The enemy items the CPU goes for (`ftCo_800A648C`): `It_Kind_Kuriboh` (a Goomba) and
/// `_Whitebea`.
const IT_KURIBOH: u32 = 43;
const IT_WHITEBEA: u32 = 0xD9;
/// The items the shield behavior rolls away from or through: `It_Kind_Flipper`,
/// `_TaruCann` (the barrel cannon), `_DKinoko` (the Poison Mushroom).
const IT_FLIPPER: u32 = 31;
const IT_TARU_CANN: u32 = 5;
const IT_DKINOKO: u32 = 22;
/// `Item::pos`.
const ITEM_POS: u32 = 0x4C;
/// The word holding `x2226_b4` (a power-up's color animation) and `x2226_b5` (its
/// refraction), and those bits; the color animation's word (`x508.x28_colanim`); the word
/// whose top byte is the cloak's vibrate frame (`x2134`).
const X2224: u32 = 0x2224;
const POWER_UP_COLORS: u32 = 0x0800 | 0x0400;
const COLANIM: u32 = 0x530;
const VIBRATE: u32 = 0x2134;
/// `CpuFighter` (`Fighter::x1A88`) offsets.
const CPU: u32 = 0x1A88;
const CPU_KIND: u32 = CPU + 0xC;
const LEVEL: u32 = CPU + 0x10;
const BEHAVIOR: u32 = CPU + 0x18;
const RANGED_DELAY: u32 = CPU + 0x34;
const FRAMES: u32 = CPU + 0x7C;
const SCRIPT: u32 = CPU + 0x44C;
const SCRIPT_WAIT: u32 = CPU + 0x450;
/// `CpuFighter::x44`, the target.
const TARGET: u32 = CPU + 0x44;
/// `CpuFighter::x54`, the destination.
const DESTINATION: u32 = CPU + 0x54;
/// `GA_Ground`.
const GROUND: u32 = 0;
/// `GA_Air`; the VS computer kind; the recovery, attack, ranged and DI behaviors.
const AIR: u32 = 1;
const VS: u32 = 4;
const RECOVER: u32 = 4;
const ATTACK: u32 = 2;
const RANGED: u32 = 3;
const DI: u32 = 18;
/// The barrel behavior (17); `ftCo_MS_BarrelWait` and `ftCo_MS_Barrel`; the barrel's angle in
/// `StageInfo` (`x73C`).
const BARREL: u32 = 17;
const BARREL_WAIT: u32 = 0x125;
const BARREL_CANNON: u32 = 0x154;
const BARREL_ANGLE: u32 = 0x73C;
/// The moves `ftCo_800BB768` dodges: Bowser's Bowser Bomb (`ftKp_MS_SpecialLw`) and up
/// smash (`ftCo_MS_AttackHi4`), Yoshi's Yoshi Bomb (`ftYs_MS_SpecialAirLw`), Samus's Screw
/// Attack (`ftSs_MS_SpecialHi`).
const KP_BOMB: u32 = 361;
const ATTACK_HI4: u32 = 63;
const YS_BOMB: u32 = 368;
const SS_SCREW: u32 = 353;
/// `Ft_Kind_*`.
const MARIO: u32 = 0;
const FOX: u32 = 1;
const DONKEY: u32 = 3;
const KOOPA: u32 = 5;
const NESS: u32 = 8;
const PIKACHU: u32 = 12;
const SAMUS: u32 = 13;
const YOSHI: u32 = 14;
const MEWTWO: u32 = 16;
const LUIGI: u32 = 17;
const ZELDA: u32 = 19;
const FALCO: u32 = 22;
const PICHU: u32 = 23;
/// The charging states: `ftDk_MS_SpecialNLoop`, `ftSs_MS_SpecialNHold`,
/// `ftMt_MS_SpecialNLoopFull`; Ness's PK Flash (`ftNs_MS_SpecialNStart`) and PK Thunder
/// (`ftNs_MS_SpecialHiStart`); Yoshi's egg roll (`ftYs_MS_SpecialAirSStart_0`).
const DK_CHARGE: u32 = 370;
const SS_CHARGE: u32 = 344;
const MT_CHARGE: u32 = 343;
const NS_PK_FLASH: u32 = 348;
const NS_PK_THUNDER: u32 = 358;
const YS_EGG_ROLL: u32 = 0x164;

/// The VS computer player's `Fighter` in a saved frame, if it is one.
fn vs_cpu(call: &Call) -> Option<u32> {
    let fp = peek_u32(call, call.regs.gpr[3] + 0x2C);
    (peek_u32(call, fp + CPU_KIND) == VS).then_some(fp)
}

/// Every other fighter, in the fighters' list order.
fn others(call: &Call, me: u32) -> Vec<u32> {
    let heads = peek_u32(call, leaf_check::sym("HSD_GObjPLinkHead"));
    let mut g = peek_u32(call, heads + 8 * 4);
    let mut v = Vec::new();
    while g != 0 {
        let fp = peek_u32(call, g + 0x2C);
        if fp != me {
            v.push(fp);
        }
        g = peek_u32(call, g + 0x8);
    }
    v
}

/// The fighters of other players.
fn enemies(call: &Call, me: u32) -> Vec<u32> {
    let heads = peek_u32(call, leaf_check::sym("HSD_GObjPLinkHead"));
    let mut g = peek_u32(call, heads + 8 * 4);
    let slot = peek_u32(call, me + PLAYER) >> 24;
    let mut v = Vec::new();
    while g != 0 {
        let fp = peek_u32(call, g + 0x2C);
        if peek_u32(call, fp + PLAYER) >> 24 != slot {
            v.push(fp);
        }
        g = peek_u32(call, g + 0x8);
    }
    v
}

/// Ends the CPU's script, so the frame writes a new one for its behavior.
fn end_script(call: &mut Call, fp: u32) {
    poke_u32(call, fp + SCRIPT, 0);
    poke_u32(call, fp + SCRIPT_WAIT, 0);
}

/// How the frame is changed.
#[derive(Clone, Copy, Debug)]
enum Change {
    /// At level 9 with its frame count a multiple of 60, its script ended: the levels' own
    /// branches (the jump at the target's checks, the shield's, Ness's PK Thunder recovery).
    Level9,
    /// Recovering in the air (behavior 4) as another character at level 9, its script
    /// ended: that character's recovery (`ftCo_800A96B8`).
    RecoverAs(u32),
    /// As another character with its ranged attack's delay over and its script ended: the
    /// ranged attack's check and the character's own (`ftCo_800B9CBC`).
    RangedAs(u32),
    /// As another character in a charging state (`ftCo_800ADC28`), in the attack behavior
    /// (its follow-up, `ftCo_800B658C`) or the ranged one (`ftCo_800B9020`), script ended.
    StateAs(u32, u32, u32),
    /// Its enemies entering the stage: the target closed in on (`ftCo_800A20A0`,
    /// `ftCo_800A80E4`).
    EnemyEntering,
    /// Moved off camera, in the air: heading back (`ftCo_800AE7AC`).
    OffCamera,
    /// Moved off camera to the stage's right on the ground: heading for another island toward
    /// the blast zones' middle (`ftCo_800A6FC4`) or, with none in that direction, toward its
    /// target (`ftCo_800A75DC`). `target_under` also puts its enemies in the air under the
    /// middle of the stage: on Final Destination, whose one island is the fighter's own, it
    /// heads for them, no island end is in reach beside them, and the destination takes what
    /// `ftCo_800A6FC4`'s frame left in `ftCo_800A75DC`'s vector, its save of f20.
    OffCameraGround { target_under: bool },
    /// In a barrel (behavior 17, `ftCo_800AC434`) with its script ended.
    Barrel(Barrel),
    /// Its enemies in the air off the stage: the island end beside them (`ftCo_800A4768`).
    TargetOffStage,
    /// Its enemies in the air under the middle of the stage: on Final Destination no island
    /// end is in reach beside them, and the destination takes what `ftCo_800A75DC`'s frame
    /// holds, the back chain and the link register slot `ftCo_800A4BEC`'s frame left (its
    /// last call `ftCo_800A0F00`'s return address). `last_hidden` also makes the last other
    /// fighter in the list untargetable when it isn't the only one, so the target search's
    /// last saving call is `ftCo_IsAlly`.
    TargetUnderStage { last_hidden: bool },
    /// Holding an item, its ranged attack's delay over and its script ended: the held item
    /// from range (`ftCo_800B9A04`), as the item's kind or as `kind`.
    Held(Option<u32>),
    /// Holding an item made the swung `kind`, on the ground with a target, its script ended:
    /// the swung item's attacks with its reach (`ftCo_800B52AC`).
    Swung(u32),
    /// The last item in the items' list made the enemy item `kind`, `dx` in front of the
    /// fighter at its height, its script ended: the enemy item it goes for
    /// (`ftCo_800A648C`) and its attacks on it (`ftCo_800B5AB0`).
    EnemyItem(u32, f32),
    /// Its enemies under a power-up's colors with the cloak's vibrate frame `frame` (2 hides
    /// them, 1 every other 120 frames, by their own frame count, set to 200), its script
    /// ended: the cloak's check (`ftCo_800A0F00`, `ft_80087A18`).
    Cloaked(u32),
    /// On the ground left of the middle, moved as far right: an item it goes for on an
    /// island to its left ends at that island's right end (`ftCo_800A866C`).
    MirroredX,
    /// Every item made `kind`, its script ended: where an item's hitbox comes at it on the
    /// ground, the shield behavior's rolls by the item (`ftCo_800BA9A0`, `ftCo_800A2998`).
    ItemsAs(u32),
    /// At level 9 with its script ended, its enemies (as `kind`) in `motion` at `(dx, dy)`
    /// from it: the moves it dodges (`ftCo_800BB768`).
    EnemyMove(u32, u32, f32, f32),
    /// In hitstun with no knockback, at level 9, its script ended: the DI pushes the stick
    /// by the registers the frame leaves (the stale-register rule). `Seed`: already in the
    /// DI with the frame count a multiple of 400, so the behavior's coin flip leaves the seed
    /// pointer; `Switch`: switching to the DI, whose coin flip leaves it too; `Entering`: in
    /// the DI with its enemies entering, so the behavior's standing query compiles the
    /// results and leaves its `MatchEnd + 0x30`. The others leave what the destination's
    /// last floor check left (`mpCheckFloor`'s last tested line): `Floor` as saved (its own
    /// frame's slot after a level line); `FloorAway` and `FloorAwayGround` with the
    /// destination moved far off the stage, in the air and on the ground (where the decision
    /// heads nowhere this frame it stays, and its check tests no line: the pick's flags
    /// slot, unless a check of the floor below the airborne fighter follows);
    /// `FloorOffCamera` high above the stage with the destination far (the pick a frame
    /// deeper, under `ftCo_800AE7AC`, which keeps the destination: the first check tests no
    /// line, the second, for the floor below, does) and `FloorOffCameraAway` far to the side
    /// (neither check tests a line).
    DiZeroKnockback(Di),
}

/// [`Change::Barrel`]'s cases.
#[derive(Clone, Copy, Debug)]
enum Barrel {
    /// In the stage's barrel (`ftCo_MS_BarrelWait`) pointing at this angle
    /// (`Ground_801C4DA0`'s, `stage_info` 0x73C): A is tapped from 15 degrees right of up to up.
    Wait(f32),
    /// In a barrel cannon (`ftCo_MS_Barrel`) at level 9, the frame count as given: the coin
    /// flip for a tap comes on multiples of 5 (400 and 405; 401 is none).
    Cannon(u32),
    /// The state as saved (out of a barrel): back to the usual behavior.
    Out,
}

/// [`Change::DiZeroKnockback`]'s cases.
#[derive(Clone, Copy, Debug)]
enum Di {
    Seed,
    Switch,
    Entering,
    Floor,
    FloorAway,
    FloorAwayGround,
    FloorOffCamera,
    FloorOffCameraAway,
}

impl Change {
    /// Whether the change is for the items-on sets' frames alone.
    fn items_on_only(self) -> bool {
        matches!(
            self,
            Change::Swung(_)
                | Change::EnemyItem(..)
                | Change::Cloaked(_)
                | Change::MirroredX
                | Change::ItemsAs(_)
        )
    }
}

/// Changes a saved frame as `change` says: whether it did.
fn change_frame(call: &mut Call, change: Change) -> bool {
    let Some(fp) = vs_cpu(call) else {
        return false;
    };
    let air = peek_u32(call, fp + GROUND_OR_AIR) == AIR;
    match change {
        Change::Level9 => {
            poke_u32(call, fp + LEVEL, 9);
            let n = peek_u32(call, fp + FRAMES);
            poke_u32(call, fp + FRAMES, n - n % 60);
            end_script(call, fp);
        }
        Change::RecoverAs(kind) => {
            if !air || peek_u32(call, fp + BEHAVIOR) != RECOVER {
                return false;
            }
            poke_u32(call, fp + KIND, kind);
            poke_u32(call, fp + LEVEL, 9);
            end_script(call, fp);
        }
        Change::RangedAs(kind) => {
            if peek_u32(call, fp + TARGET) == 0 {
                return false;
            }
            poke_u32(call, fp + KIND, kind);
            poke_u32(call, fp + RANGED_DELAY, 0);
            end_script(call, fp);
        }
        Change::StateAs(kind, motion, behavior) => {
            if peek_u32(call, fp + TARGET) == 0 {
                return false;
            }
            poke_u32(call, fp + KIND, kind);
            poke_u32(call, fp + MOTION, motion);
            poke_u32(call, fp + BEHAVIOR, behavior);
            end_script(call, fp);
        }
        Change::EnemyEntering => {
            for e in enemies(call, fp) {
                let w = peek_u32(call, e + X221C);
                poke_u32(call, e + X221C, w | ENTERING);
            }
            end_script(call, fp);
        }
        Change::OffCamera => {
            poke_f32(call, fp + POS, 300.0);
            poke_f32(call, fp + POS + 4, 150.0);
            poke_u32(call, fp + GROUND_OR_AIR, AIR);
            end_script(call, fp);
        }
        Change::OffCameraGround { target_under } => {
            poke_f32(call, fp + POS, 400.0);
            poke_u32(call, fp + GROUND_OR_AIR, GROUND);
            if target_under {
                for e in enemies(call, fp) {
                    poke_f32(call, e + POS, 0.0);
                    poke_f32(call, e + POS + 4, -100.0);
                    poke_u32(call, e + GROUND_OR_AIR, AIR);
                }
            }
            end_script(call, fp);
        }
        Change::Barrel(barrel) => {
            match barrel {
                Barrel::Wait(angle) => {
                    poke_u32(call, fp + MOTION, BARREL_WAIT);
                    poke_f32(call, leaf_check::sym("stage_info") + BARREL_ANGLE, angle);
                }
                Barrel::Cannon(frames) => {
                    poke_u32(call, fp + MOTION, BARREL_CANNON);
                    poke_u32(call, fp + LEVEL, 9);
                    poke_u32(call, fp + FRAMES, frames);
                }
                Barrel::Out => {
                    let ms = peek_u32(call, fp + MOTION);
                    if ms == BARREL_WAIT || ms == BARREL_CANNON {
                        return false;
                    }
                }
            }
            poke_u32(call, fp + BEHAVIOR, BARREL);
            end_script(call, fp);
        }
        Change::EnemyMove(kind, motion, dx, dy) => {
            poke_u32(call, fp + LEVEL, 9);
            end_script(call, fp);
            let (x, y) = (peek_f32(call, fp + POS), peek_f32(call, fp + POS + 4));
            for e in enemies(call, fp) {
                poke_u32(call, e + KIND, kind);
                poke_u32(call, e + MOTION, motion);
                poke_f32(call, e + POS, x + dx);
                poke_f32(call, e + POS + 4, y + dy);
            }
        }
        Change::DiZeroKnockback(di) => {
            if peek_u32(call, fp + X221A) & HITSTUN == 0 {
                return false;
            }
            let in_di = peek_u32(call, fp + BEHAVIOR) == DI;
            if in_di == matches!(di, Di::Switch) {
                return false;
            }
            poke_f32(call, fp + KB_VEL, 0.0);
            poke_f32(call, fp + KB_VEL + 4, 0.0);
            poke_u32(call, fp + LEVEL, 9);
            end_script(call, fp);
            // 400 % 120 and 60 % 120 are past 120 - 12 * 9: the level's chance pushes.
            poke_u32(
                call,
                fp + FRAMES,
                if matches!(di, Di::Seed) { 400 } else { 60 },
            );
            match di {
                Di::Entering => {
                    for e in enemies(call, fp) {
                        let w = peek_u32(call, e + X221C);
                        poke_u32(call, e + X221C, w | ENTERING);
                    }
                }
                Di::FloorAway | Di::FloorAwayGround => {
                    poke_f32(call, fp + DESTINATION, 500.0);
                    poke_f32(call, fp + DESTINATION + 4, -500.0);
                    if matches!(di, Di::FloorAwayGround) {
                        poke_u32(call, fp + GROUND_OR_AIR, GROUND);
                    }
                }
                Di::FloorOffCameraAway => {
                    poke_f32(call, fp + POS, 1000.0);
                    poke_f32(call, fp + POS + 4, 150.0);
                    poke_u32(call, fp + GROUND_OR_AIR, AIR);
                    poke_f32(call, fp + DESTINATION, 1000.0);
                    poke_f32(call, fp + DESTINATION + 4, -500.0);
                }
                Di::FloorOffCamera => {
                    poke_f32(call, fp + POS, 0.0);
                    poke_f32(call, fp + POS + 4, 400.0);
                    poke_u32(call, fp + GROUND_OR_AIR, AIR);
                    poke_f32(call, fp + DESTINATION, 500.0);
                    poke_f32(call, fp + DESTINATION + 4, -500.0);
                }
                Di::Seed | Di::Switch | Di::Floor => {}
            }
        }
        Change::TargetOffStage => {
            for e in enemies(call, fp) {
                poke_f32(call, e + POS, -120.0);
                poke_f32(call, e + POS + 4, -20.0);
                poke_u32(call, e + GROUND_OR_AIR, AIR);
            }
            end_script(call, fp);
        }
        Change::TargetUnderStage { last_hidden } => {
            if last_hidden {
                let others = others(call, fp);
                let [.., before, last] = others[..] else {
                    return false;
                };
                // The target stays targetable.
                if peek_u32(call, before + X2164) != 0 {
                    return false;
                }
                poke_u32(call, last + X2164, 1);
            }
            for e in enemies(call, fp) {
                poke_f32(call, e + POS, 0.0);
                poke_f32(call, e + POS + 4, -100.0);
                poke_u32(call, e + GROUND_OR_AIR, AIR);
            }
            end_script(call, fp);
        }
        Change::Swung(kind) => {
            let item = peek_u32(call, fp + ITEM_GOBJ);
            if item == 0 || air || peek_u32(call, fp + TARGET) == 0 {
                return false;
            }
            let ip = peek_u32(call, item + 0x2C);
            poke_u32(call, ip + ITEM_KIND, kind);
            end_script(call, fp);
        }
        Change::EnemyItem(kind, dx) => {
            let heads = peek_u32(call, leaf_check::sym("HSD_GObjPLinkHead"));
            let mut g = peek_u32(call, heads + 9 * 4);
            let mut last = 0;
            while g != 0 {
                last = g;
                g = peek_u32(call, g + 0x8);
            }
            if last == 0 {
                return false;
            }
            let ip = peek_u32(call, last + 0x2C);
            let facing = peek_f32(call, fp + 0x2C);
            poke_u32(call, ip + ITEM_KIND, kind);
            poke_f32(call, ip + ITEM_POS, peek_f32(call, fp + POS) + facing * dx);
            poke_f32(call, ip + ITEM_POS + 4, peek_f32(call, fp + POS + 4));
            end_script(call, fp);
        }
        Change::Cloaked(frame) => {
            for e in enemies(call, fp) {
                let w = peek_u32(call, e + X2224);
                poke_u32(call, e + X2224, w | POWER_UP_COLORS);
                poke_u32(call, e + COLANIM, 0);
                let v = peek_u32(call, e + VIBRATE);
                poke_u32(call, e + VIBRATE, (v & 0x00FF_FFFF) | frame << 24);
                poke_u32(call, e + FRAMES, 200);
            }
            end_script(call, fp);
        }
        Change::ItemsAs(kind) => {
            let heads = peek_u32(call, leaf_check::sym("HSD_GObjPLinkHead"));
            let mut g = peek_u32(call, heads + 9 * 4);
            if g == 0 {
                return false;
            }
            while g != 0 {
                let ip = peek_u32(call, g + 0x2C);
                poke_u32(call, ip + ITEM_KIND, kind);
                g = peek_u32(call, g + 0x8);
            }
            end_script(call, fp);
        }
        Change::MirroredX => {
            let x = peek_f32(call, fp + POS);
            if air || x > -10.0 {
                return false;
            }
            poke_f32(call, fp + POS, -x);
            end_script(call, fp);
        }
        Change::Held(kind) => {
            let item = peek_u32(call, fp + ITEM_GOBJ);
            if item == 0 || peek_u32(call, fp + TARGET) == 0 {
                return false;
            }
            if let Some(kind) = kind {
                let ip = peek_u32(call, item + 0x2C);
                poke_u32(call, ip + ITEM_KIND, kind);
            }
            poke_u32(call, fp + RANGED_DELAY, 0);
            end_script(call, fp);
        }
    }
    true
}

fn run_frame(
    r: &mut melee_sim::game::state::Running,
    maps: &melee_capture::import::Maps,
    call: &Call,
) -> Ret {
    r.cpu_frame(gobj_arg(maps, call));
    Ret::Nothing
}

/// The clean comparisons among `reports` of the calls at `changed` (indices into `paths`), or
/// of every call with `changed` empty.
fn compared(reports: &[Report], paths: &[std::path::PathBuf], changed: &BTreeSet<usize>) -> usize {
    reports
        .iter()
        .flat_map(|r| &r.clean)
        .filter(|(p, _)| {
            changed.is_empty()
                || paths
                    .iter()
                    .position(|q| q == p)
                    .is_some_and(|i| changed.contains(&i))
        })
        .count()
}

/// Every saved frame compares clean as saved.
#[test]
fn cpu_vs_frame() {
    leaf_check::shard_test!();
    leaf_check::run_as_machine_code();
    let dirs = dirs();
    let none = |_: &mut Call, _: usize| {};
    let variants = [Variant {
        prepare: &none,
        ours: &run_frame,
        stubs: &[],
    }];
    let per_dir = leaf_check::each_set(
        &dirs,
        |dir| saved_calls(dir, "ftCo_800B3900").len(),
        |dir| {
            let paths = saved_calls(dir, "ftCo_800B3900");
            let reports = check_variants(dir, "ftCo_800B3900", &variants);
            compared(&reports[0], &paths, &BTreeSet::new())
        },
    );
    let total: usize = per_dir.iter().sum();
    eprintln!("compared clean as saved: {total}");
    if dirs.is_empty() || leaf_check::reference().is_none() {
        return;
    }
    assert!(total > 0, "no saved frame compared");
}

/// The changes [`cpu_vs_changed`] makes, each of which must compare clean somewhere.
fn changes() -> Vec<Change> {
    let mut v = vec![Change::Level9];
    for kind in [
        MARIO, FOX, FALCO, PIKACHU, PICHU, YOSHI, NESS, LUIGI, ZELDA, SAMUS,
    ] {
        v.push(Change::RecoverAs(kind));
    }
    for kind in [NESS, YOSHI, SAMUS, MEWTWO] {
        v.push(Change::RangedAs(kind));
    }
    v.extend([
        Change::StateAs(DONKEY, DK_CHARGE, ATTACK),
        Change::StateAs(SAMUS, SS_CHARGE, ATTACK),
        Change::StateAs(MEWTWO, MT_CHARGE, ATTACK),
        Change::StateAs(NESS, NS_PK_FLASH, ATTACK),
        Change::StateAs(NESS, NS_PK_THUNDER, RANGED),
        Change::StateAs(YOSHI, YS_EGG_ROLL, RANGED),
        Change::EnemyEntering,
        Change::OffCamera,
        Change::OffCameraGround {
            target_under: false,
        },
        Change::OffCameraGround { target_under: true },
        Change::Barrel(Barrel::Wait(0.0)),
        Change::Barrel(Barrel::Wait(-0.2)),
        Change::Barrel(Barrel::Wait(-0.3)),
        Change::Barrel(Barrel::Wait(0.1)),
        Change::Barrel(Barrel::Cannon(400)),
        Change::Barrel(Barrel::Cannon(401)),
        Change::Barrel(Barrel::Cannon(405)),
        Change::Barrel(Barrel::Out),
        Change::TargetOffStage,
        Change::TargetUnderStage { last_hidden: false },
        Change::TargetUnderStage { last_hidden: true },
        Change::Held(None),
        Change::Held(Some(IT_CAPSULE)),
        Change::Held(Some(IT_BOX)),
        Change::Held(Some(IT_M_BALL)),
        Change::Swung(IT_HARISEN),
        Change::Swung(IT_LIPSTICK),
        Change::Swung(IT_STARROD),
        Change::Swung(IT_SWORD),
        Change::Swung(IT_BAT),
        Change::Swung(IT_PARASOL),
        Change::EnemyItem(IT_KURIBOH, 10.0),
        Change::EnemyItem(IT_WHITEBEA, 20.0),
        Change::Cloaked(2),
        Change::Cloaked(1),
        Change::MirroredX,
        Change::ItemsAs(IT_FLIPPER),
        Change::ItemsAs(IT_TARU_CANN),
        Change::ItemsAs(IT_DKINOKO),
        Change::EnemyMove(KOOPA, KP_BOMB, 0.0, 20.0),
        Change::EnemyMove(KOOPA, ATTACK_HI4, 0.0, -10.0),
        Change::EnemyMove(YOSHI, YS_BOMB, 0.0, 20.0),
        Change::EnemyMove(SAMUS, SS_SCREW, 0.0, -10.0),
        Change::DiZeroKnockback(Di::Seed),
        Change::DiZeroKnockback(Di::Switch),
        Change::DiZeroKnockback(Di::Entering),
        Change::DiZeroKnockback(Di::Floor),
        Change::DiZeroKnockback(Di::FloorAway),
        Change::DiZeroKnockback(Di::FloorAwayGround),
        Change::DiZeroKnockback(Di::FloorOffCamera),
        Change::DiZeroKnockback(Di::FloorOffCameraAway),
    ]);
    v
}

/// The saved frames changed to reach what the replays don't: each changed frame compares
/// clean, and each change compares clean in some frame. Each change is a unit of the binary's
/// shards ([`leaf_check::shard`]): its assertion needs only its own frames.
#[test]
fn cpu_vs_changed() {
    leaf_check::run_as_machine_code();
    let all = changes();
    let changes: Vec<Change> = leaf_check::shard::units("cpu_vs_changed", all.len())
        .into_iter()
        .map(|i| all[i])
        .collect();
    if changes.is_empty() {
        return;
    }
    let dirs = dirs();
    let per_dir = leaf_check::each_set(
        &dirs,
        |dir| saved_calls(dir, "ftCo_800B3900").len(),
        |dir| {
            // A change runs every frame of a set, changed or not: the items-on changes run
            // only on the items-on sets, whose frames they are for.
            let items_on = dir.contains("oracle-cpu-items-");
            let active: Vec<usize> = (0..changes.len())
                .filter(|&v| items_on || !changes[v].items_on_only())
                .collect();
            if active.is_empty() {
                return vec![0; changes.len()];
            }
            let changed: Vec<Mutex<BTreeSet<usize>>> =
                changes.iter().map(|_| Mutex::default()).collect();
            let prepares: Vec<Box<Prepare>> = active
                .iter()
                .map(|&v| {
                    let changed = &changed;
                    let change = changes[v];
                    Box::new(move |c: &mut Call, i: usize| {
                        if change_frame(c, change) {
                            changed[v].lock().unwrap().insert(i);
                        }
                    }) as Box<Prepare>
                })
                .collect();
            let variants: Vec<Variant> = prepares
                .iter()
                .map(|p| Variant {
                    prepare: p.as_ref(),
                    ours: &run_frame,
                    stubs: &[],
                })
                .collect();
            let paths = saved_calls(dir, "ftCo_800B3900");
            let reports = check_variants(dir, "ftCo_800B3900", &variants);
            let mut out = vec![0; changes.len()];
            for (&v, r) in active.iter().zip(&reports) {
                let c = changed[v].lock().unwrap();
                if !c.is_empty() {
                    out[v] = compared(r, &paths, &c);
                }
            }
            out
        },
    );
    let mut totals = vec![0; changes.len()];
    for t in &per_dir {
        for (total, n) in totals.iter_mut().zip(t) {
            *total += n;
        }
    }
    for (c, t) in changes.iter().zip(&totals) {
        eprintln!("compared clean, {c:?}: {t}");
    }
    if dirs.is_empty() || leaf_check::reference().is_none() {
        return;
    }
    let never: Vec<_> = changes
        .iter()
        .zip(&totals)
        .filter(|(_, t)| **t == 0)
        .map(|(c, _)| c)
        .collect();
    assert!(never.is_empty(), "changes that never compared: {never:?}");
}
