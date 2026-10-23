//! The Special Melee modes' VS entries (`gmtiny.c` and the others: the callbacks that change
//! the start data, and Stamina Mode's own entry) against ssbm-rs's ports, on random start
//! data.

use leaf_check::{Rand, SCRATCH, handle, hsd_reference};
use melee_capture::import::sym;
use melee_data::archive::DataRef;
use melee_sim::game::gm::vs_mode;
use melee_sim::game::mirror::{Cx, Mem, Mirror};
use melee_sim::game::structs::{PlayerInitData, StartMeleeData};
use ssbm_rt::Ctx;
use ssbm_types::fns;
use ssbm_types::tu::melee__gm__gminvisible as invisible;
use ssbm_types::tu::melee__gm__gmlightning as lightning;

/// The differences a comparison finds.
#[derive(Default)]
struct Diffs(Vec<String>);

impl Cx for Diffs {
    fn import_handle(&mut self, _: &'static str, addr: u32) -> Option<u32> {
        (addr != 0).then_some(addr)
    }
    fn import_data(&mut self, addr: u32) -> Option<DataRef> {
        assert_eq!(addr, 0, "a data pointer in the start data");
        None
    }
    fn compare_handle(&mut self, path: &str, _: &'static str, ours: Option<u32>, theirs: u32) {
        if ours.unwrap_or(0) != theirs {
            self.0.push(format!("{path}: {ours:?} vs {theirs:#x}"));
        }
    }
    fn compare_data(&mut self, path: &str, ours: Option<DataRef>, theirs: u32) {
        if ours.is_some() || theirs != 0 {
            self.0.push(format!("{path}: {ours:?} vs {theirs:#x}"));
        }
    }
    fn mismatch(&mut self, path: &str, ours: String, theirs: String) {
        self.0.push(format!("{path}: {ours} vs {theirs}"));
    }
}

/// The start data of `GmVsMode_Stamina`'s entry (9) in the save's VS mode table
/// (`gmm_x0::modes.table` at 0x590, 0x140 bytes an entry, its start data at 8).
pub fn stamina_vs_data(ctx: &Ctx) -> u32 {
    ctx.read_u32(sym("gmMainLib_804D3EE0")) + 0x590 + 9 * 0x140 + 8
}

fn read(ctx: &Ctx, at: u32, len: u32) -> Vec<u8> {
    let mut b = vec![0; len as usize];
    let _ = ctx.mem.read_bytes(at, &mut b);
    b
}

fn random_bytes(rand: &mut Rand, len: u32) -> Vec<u8> {
    (0..len).map(|_| rand.bits() as u8).collect()
}

/// The player block callbacks (Super Sudden Death's, Invisible Melee's, Tiny and Giant
/// Melee's) on random blocks: ours on the block imported, theirs on it in memory, the results
/// field for field.
#[test]
fn player_callbacks() {
    let Some(ctx) = hsd_reference() else { return };
    let ctx = &ctx;
    type Theirs = for<'a> fn(
        &'a Ctx,
        ssbm_types::records::PlayerInitData<'a>,
        ssbm_types::records::PlayerInitData<'a>,
    );
    let modes: [(&str, vs_mode::PlayerCallback, Theirs); 4] = [
        (
            "fn_801B8C5C",
            vs_mode::super_sudden_death_player,
            fns::fn_801B8C5C,
        ),
        (
            "initVsPlayer",
            vs_mode::invisible_player,
            invisible::initVsPlayer,
        ),
        ("fn_801B8E50", vs_mode::tiny_player, fns::fn_801B8E50),
        ("fn_801B9060", vs_mode::giant_player, fns::fn_801B9060),
    ];
    let size = PlayerInitData::SIZE;
    let mut rand = Rand(0x5EC1_A1F0_000D_u64);
    for (name, ours_cb, theirs_cb) in modes {
        for n in 0..500 {
            let bytes = random_bytes(&mut rand, size);
            let mut ours = PlayerInitData::import(
                &Mem {
                    bytes: &bytes,
                    base: SCRATCH,
                },
                SCRATCH,
                &mut Diffs::default(),
            );
            ours_cb(&mut ours);
            ctx.write_bytes(SCRATCH, &bytes);
            theirs_cb(ctx, handle(ctx, SCRATCH), handle(ctx, SCRATCH + 0x100));
            let after = read(ctx, SCRATCH, size);
            let mut d = Diffs::default();
            ours.compare(
                &Mem {
                    bytes: &after,
                    base: SCRATCH,
                },
                SCRATCH,
                "player",
                &mut d,
            );
            assert!(d.0.is_empty(), "{name} {n}: {:?}", d.0);
        }
    }
}

/// The rules callbacks (Slo-Mo and Lightning Melee's speeds, Single-Button Melee's flag and
/// Fixed-Camera Mode's unpause hook) on random start data, as [`player_callbacks`].
#[test]
fn rules_callbacks() {
    let Some(ctx) = hsd_reference() else { return };
    let ctx = &ctx;
    type Theirs = for<'a> fn(
        &'a Ctx,
        ssbm_types::records::StartMeleeData<'a>,
        ssbm_types::records::StartMeleeData<'a>,
    );
    let modes: [(&str, vs_mode::StartCallback, Theirs); 4] = [
        ("fn_801BA5B4", vs_mode::slow_mo_rules, fns::fn_801BA5B4),
        (
            "fn_801BA7AC",
            vs_mode::lightning_rules,
            lightning::fn_801BA7AC,
        ),
        (
            "fn_801BA1B4",
            vs_mode::single_button_rules,
            fns::fn_801BA1B4,
        ),
        ("fn_801B9FB8", vs_mode::fixed_camera_rules, fns::fn_801B9FB8),
    ];
    let size = StartMeleeData::SIZE;
    let mut rand = Rand(0x5EC1_A1F0_000E_u64);
    for (name, ours_cb, theirs_cb) in modes {
        for n in 0..300 {
            let mut bytes = random_bytes(&mut rand, size);
            // The rules' callbacks and pointers (0x38 to 0x5B), none or another mode's hook.
            bytes[0x38..0x5C].fill(0);
            if rand.below(2) == 0 {
                bytes[0x38..0x3C].copy_from_slice(&0x8016_5268u32.to_be_bytes());
            }
            let mut ours = StartMeleeData::import(
                &Mem {
                    bytes: &bytes,
                    base: SCRATCH,
                },
                SCRATCH,
                &mut Diffs::default(),
            );
            ours_cb(&mut ours);
            ctx.write_bytes(SCRATCH, &bytes);
            theirs_cb(ctx, handle(ctx, SCRATCH), handle(ctx, SCRATCH + 0x200));
            let after = read(ctx, SCRATCH, size);
            let mut d = Diffs::default();
            ours.compare(
                &Mem {
                    bytes: &after,
                    base: SCRATCH,
                },
                SCRATCH,
                "start",
                &mut d,
            );
            assert!(d.0.is_empty(), "{name} {n}: {:?}", d.0);
        }
    }
}

/// Stamina Mode's VS entry (`gm_801B931C`) from random VS data: ours on the data imported,
/// theirs from the mode's entry in the save's VS mode table into the scene's enter data. The
/// menus' rules (`gm_80167BC8`), the rumble (`gm_LoadRumbleEnabled`) and the announcer's load
/// are checked on their own; here they are stand-ins on their side and left out on ours.
#[test]
fn stamina_entry() {
    let Some(ctx) = hsd_reference() else { return };
    let ctx = &ctx;
    for name in ["gm_80167BC8", "gm_LoadRumbleEnabled", "gm_LoadAnnouncer"] {
        ctx.register(sym(name), |_| {});
    }
    let vs = stamina_vs_data(ctx);
    let size = StartMeleeData::SIZE;
    let state = SCRATCH;
    let enter = SCRATCH + 0x100;
    let mut rand = Rand(0x5EC1_A1F0_000F_u64);
    for n in 0..300 {
        let mut bytes = random_bytes(&mut rand, size);
        bytes[0x38..0x5C].fill(0);
        // A team match or not, as the sub-colours read it.
        bytes[8] = rand.below(3) as u8;
        let vs_data = StartMeleeData::import(
            &Mem {
                bytes: &bytes,
                base: vs,
            },
            vs,
            &mut Diffs::default(),
        );
        let ours = vs_mode::stamina_entry(&vs_data);
        ctx.write_bytes(vs, &bytes);
        // The scene's state: its enter data, garbage before the entry.
        ctx.write_bytes(enter, &random_bytes(&mut rand, size));
        let st: ssbm_types::records::GameModeState = handle(ctx, state);
        st.info().set_enter_data(handle(ctx, enter));
        fns::gm_801B931C(ctx, st);
        let after = read(ctx, enter, size);
        let mut d = Diffs::default();
        ours.compare(
            &Mem {
                bytes: &after,
                base: enter,
            },
            enter,
            "start",
            &mut d,
        );
        assert!(d.0.is_empty(), "gm_801B931C {n}: {:?}", d.0);
    }
}
