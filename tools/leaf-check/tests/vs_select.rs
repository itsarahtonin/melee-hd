//! VS mode's setup helpers against ssbm-rs's ports, on random data: Sudden Death's start
//! data (`gm_SetupSuddenDeath`), the player block setups and the one-player screens' data
//! (`gm_1B03.c`), the team costumes (`gm_80169264` and its two siblings), and the announcer's
//! name calls (`gm_80168C5C`, `lbAudioAx_800243F4`).

use std::cell::RefCell;

use leaf_check::{Rand, SCRATCH, handle, hsd_reference};
use melee_capture::import::sym;
use melee_data::archive::DataRef;
use melee_sim::game::gm::{setup_players, vs_melee};
use melee_sim::game::mirror::{Cx, Mem, Mirror};
use melee_sim::game::structs::{MatchEnd, PlayerInitData, StartMeleeData, VsModeData};
use ssbm_rt::Ctx;
use ssbm_types::fns;

#[derive(Default)]
struct Diffs(Vec<String>);

impl Cx for Diffs {
    fn import_handle(&mut self, _: &'static str, addr: u32) -> Option<u32> {
        (addr != 0).then_some(addr)
    }
    fn import_data(&mut self, _: u32) -> Option<DataRef> {
        None
    }
    fn compare_handle(&mut self, path: &str, _: &'static str, ours: Option<u32>, theirs: u32) {
        if ours.unwrap_or(0) != theirs {
            self.0.push(format!("{path}: {ours:?} vs {theirs:#x}"));
        }
    }
    fn compare_data(&mut self, _: &str, _: Option<DataRef>, _: u32) {}
    fn mismatch(&mut self, path: &str, ours: String, theirs: String) {
        self.0.push(format!("{path}: {ours} vs {theirs}"));
    }
}

fn read(ctx: &Ctx, at: u32, len: u32) -> Vec<u8> {
    let mut b = vec![0; len as usize];
    let _ = ctx.mem.read_bytes(at, &mut b);
    b
}

fn random_bytes(rand: &mut Rand, len: u32) -> Vec<u8> {
    (0..len).map(|_| rand.bits() as u8).collect()
}

fn import<T: Mirror>(bytes: &[u8], at: u32) -> T {
    T::import(&Mem { bytes, base: at }, at, &mut Diffs::default())
}

fn compare<T: Mirror>(ours: &T, after: &[u8], at: u32, what: &str) {
    let mut d = Diffs::default();
    ours.compare(
        &Mem {
            bytes: after,
            base: at,
        },
        at,
        what,
        &mut d,
    );
    assert!(d.0.is_empty(), "{what}: {:?}", d.0);
}

/// The start data's pointers zeroed (the hooks), as no setup writes or reads them.
fn start_bytes(rand: &mut Rand) -> Vec<u8> {
    let mut b = random_bytes(rand, StartMeleeData::SIZE);
    b[0x38..0x60].fill(0);
    b
}

/// Sudden Death's setup on random start data and a random match end (small stock counts and
/// teams so every branch runs).
#[test]
fn sudden_death() {
    let Some(ctx) = hsd_reference() else { return };
    let ctx = &ctx;
    let start_at = SCRATCH;
    let end_at = SCRATCH + 0x200;
    let mut rand = Rand(0x5D5D_0001_u64);
    for n in 0..2000 {
        let sb = start_bytes(&mut rand);
        let mut eb = random_bytes(&mut rand, MatchEnd::SIZE);
        // Match kind, outcome, teams and each player's stocks, team and loss small enough to
        // take each branch.
        eb[5] = rand.below(3) as u8;
        eb[4] = rand.below(3) as u8;
        eb[6] = rand.below(2) as u8;
        for i in 0..6 {
            let p = 0x58 + 0xA8 * i;
            eb[p + 1] = [0x12, 0x13, 2, 9][rand.below(4) as usize];
            eb[p + 2] = [7, 0x13, 1][rand.below(3) as usize];
            eb[p + 5] = rand.below(2) as u8;
            eb[p + 7] = rand.below(5) as u8;
            eb[p + 8] = rand.below(3) as u8;
        }
        for t in 0..5 {
            eb[0x1C + 0xC * t + 8] = rand.below(2) as u8;
        }
        let mut ours: StartMeleeData = import(&sb, start_at);
        let end: MatchEnd = import(&eb, end_at);
        setup_players::setup_sudden_death(&mut ours, &end);
        ctx.write_bytes(start_at, &sb);
        ctx.write_bytes(end_at, &eb);
        fns::gm_SetupSuddenDeath(ctx, handle(ctx, start_at), handle(ctx, end_at));
        let after = read(ctx, start_at, StartMeleeData::SIZE);
        compare(&ours, &after, start_at, &format!("gm_SetupSuddenDeath {n}"));
    }
}

/// The player block setups on random blocks and arguments.
#[test]
fn player_setups() {
    let Some(ctx) = hsd_reference() else { return };
    let ctx = &ctx;
    let at = SCRATCH;
    let mut rand = Rand(0x5D5D_0002_u64);
    for n in 0..3000 {
        let b = random_bytes(&mut rand, PlayerInitData::SIZE);
        let (ck, col, st, slot) = (
            rand.bits() as u8,
            rand.bits() as u8,
            rand.bits() as u8,
            rand.below(6) as u8,
        );
        let mut ours: PlayerInitData = import(&b, at);
        ctx.write_bytes(at, &b);
        let p = handle(ctx, at);
        match n % 3 {
            0 => {
                setup_players::setup_human(&mut ours, ck, col, st, slot);
                fns::gm_SetupHumanPlayer(ctx, p, ck, col, st, slot);
            }
            1 => {
                setup_players::setup_cpu(&mut ours, ck, col, st, slot);
                fns::gm_SetupCpuPlayer(ctx, p, ck, col, st, slot);
            }
            _ => {
                let s = i32::from(rand.below(8) as u8) - 2;
                setup_players::set_slot(&mut ours, s);
                fns::gm_801B05F4(ctx, p, s);
            }
        }
        let after = read(ctx, at, PlayerInitData::SIZE);
        compare(&ours, &after, at, &format!("player setup {n}"));
    }
}

/// The one-player screens' data, set and read back, on random `CSSData`.
#[test]
fn one_player_screens() {
    let Some(ctx) = hsd_reference() else { return };
    let ctx = &ctx;
    let at = SCRATCH;
    let out = SCRATCH + 0x200;
    let size = 8 + VsModeData::SIZE;
    let mut rand = Rand(0x5D5D_0003_u64);
    for n in 0..2000 {
        let mut b = random_bytes(&mut rand, size);
        b[4..8].fill(0);
        b[0x10 + 0x38..0x10 + 0x60].fill(0);
        // A port the blocks have.
        b[0] = 0;
        b[1] = rand.below(4) as u8 + 1;
        let mut ours = vs_melee::CssData {
            unk_0x0: u16::from_be_bytes([b[0], b[1]]),
            match_type: b[2],
            pending_scene_change: b[3],
            vs: import(&b[8..], at + 8),
        };
        ctx.write_bytes(at, &b);
        let css = handle(ctx, at);
        let (ck, st, col, tag, lvl) = (
            rand.bits() as i8,
            rand.bits() as i8,
            rand.bits() as i8,
            rand.bits() as u8,
            rand.bits() as u8,
        );
        match n % 4 {
            0 => {
                let (ty, slot) = (rand.bits() as u8, rand.below(4) as u8);
                setup_players::css_one_player(&mut ours, ty, ck, st, col, tag, lvl, slot);
                fns::gm_801B06B0(ctx, css, ty, ck, st, col, tag, lvl, slot);
            }
            1 => {
                let first = rand.below(3) as u8;
                setup_players::css_set_partner(&mut ours, ck, st, col, tag, lvl, first);
                fns::gm_801B07B4(ctx, css, ck, st, col, tag, lvl, first);
            }
            2 => {
                ctx.write_bytes(out, &[0; 8]);
                fns::gm_801B0730(
                    ctx,
                    css,
                    handle(ctx, out),
                    handle(ctx, out + 1),
                    handle(ctx, out + 2),
                    handle(ctx, out + 3),
                    handle(ctx, out + 4),
                );
                let (c, s, k, t, l) = setup_players::css_one_player_choice(&ours);
                assert_eq!(
                    read(ctx, out, 5),
                    vec![c as u8, s, k, t, l],
                    "gm_801B0730 {n}"
                );
            }
            _ => {
                ctx.write_bytes(out, &[0; 8]);
                fns::gm_801B07E8(
                    ctx,
                    css,
                    handle(ctx, out),
                    handle(ctx, out + 1),
                    handle(ctx, out + 2),
                    handle(ctx, out + 3),
                    handle(ctx, out + 4),
                );
                let (c, s, k, t, l) = setup_players::css_partner_choice(&ours);
                assert_eq!(
                    read(ctx, out, 5),
                    vec![c as u8, s as u8, k as u8, t as u8, l],
                    "gm_801B07E8 {n}"
                );
            }
        }
        let after = read(ctx, at, size);
        assert_eq!(
            (ours.unk_0x0, ours.match_type, ours.pending_scene_change),
            (u16::from_be_bytes([after[0], after[1]]), after[2], after[3]),
            "CSSData {n}'s header"
        );
        compare(&ours.vs, &after[8..], at + 8, &format!("CSSData {n}"));
    }
}

/// Each character kind's team costumes, past the table too.
#[test]
fn team_costumes() {
    let Some(ctx) = hsd_reference() else { return };
    let ctx = &ctx;
    for k in 0..=255u8 {
        let theirs = [
            fns::gm_80169264(ctx, k),
            fns::gm_801692BC(ctx, k),
            fns::gm_80169290(ctx, k),
        ];
        let ours = [0, 1, 2].map(|t| vs_melee::team_costume(k, t));
        assert_eq!(ours, theirs, "ckind {k}");
    }
}

thread_local! {
    static CALL: RefCell<Vec<(u32, u32)>> = const { RefCell::new(Vec::new()) };
}

/// The name calls: each kind's id (`lbAudioAx_800243F4` stood in for), and each id's track
/// (`HSD_AudioSFXStartParam`, which the original's inlined `fn_80023750` calls, stood in for).
#[test]
fn name_calls() {
    let Some(ctx) = hsd_reference() else { return };
    let ctx = &ctx;
    let record = |ctx: &Ctx| {
        let g = &ctx.regs.gpr;
        CALL.with(|c| c.borrow_mut().push((g[3].get(), g[6].get())));
        g[3].set(0);
    };
    ctx.register(sym("lbAudioAx_800243F4"), record);
    for k in 0..=40u32 {
        CALL.with(|c| c.borrow_mut().clear());
        fns::gm_80168C5C(ctx, k);
        let theirs: Vec<u32> = CALL.with(|c| c.borrow().iter().map(|c| c.0).collect());
        let ours: Vec<u32> = vs_melee::name_call(k)
            .map(|id| id as u32)
            .into_iter()
            .collect();
        assert_eq!(ours, theirs, "ckind {k}");
    }
    let Some(ctx) = hsd_reference() else { return };
    let ctx = &ctx;
    ctx.register(sym("HSD_AudioSFXStartParam"), record);
    for id in 0x7C820..0x7C860 {
        CALL.with(|c| c.borrow_mut().clear());
        fns::lbAudioAx_800243F4(ctx, id);
        let theirs = CALL.with(|c| c.borrow().clone());
        let track = melee_sim::lb::audio::name_call_track(id) as u32;
        assert_eq!(theirs, vec![(id as u32, track)], "id {id:#x}");
    }
}
