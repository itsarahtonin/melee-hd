//! Fighter creation (`Fighter_Create` and what it reaches, through `Player_80031AD0`) against
//! ssbm-rs, from the state just before a VS match creates its fighters: each capture's
//! players' fighters are created in slot order on both sides, and the whole state (the
//! scheduler, every joint tree, the fighters with their parts, attributes, DObj lists,
//! dynamics, shadows and lights, the stage, the player blocks and the fighter globals) and the
//! calls into unported subsystems are compared after each slot.
//!
//! The debug VS mode loads fighters' files from the disc inside `Fighter_Create`; ssbm-rs's
//! side reads them from the disc here, and melee-sim's side is handed the same files, as its
//! frontend would hand them over before the match, with the ARAM address ssbm-rs's side gave
//! the animation file. `HSD_ObjAlloc` hands the original uncleared blocks: the reference's
//! fighter blocks are zeroed as a fresh heap's are (the console's first match in a session,
//! and Slippi's playback, which zeroes them; see `docs/engine.md`).
//!
//! The captures are game memory, kept out of the repository in `FIGHTER_CREATE_CALLS`
//! (default below), one directory per run; the test skips without them. ssbm-rs's runner
//! (`cargo build --release -p ssbm-run`) makes them in the debug VS mode's random matches,
//! with `done.txt` listing every function address except `fn_8016DEEC`:
//!
//! ```text
//! MATCH_STAGES=32 MATCH_FIGHTERS=2,9 MATCH_TIME=30 MATCH_PLAYERS=2 \
//!   CAPTURE=$FIGHTER_CREATE_CALLS/start2 CAPTURE_FROM=0 CAPTURE_NESTED=1 CAPTURE_EVERY=1 \
//!   CAPTURE_FUNCS=fn_8016DEEC LOCKSTEP_DONE=done.txt \
//!   ssbm-run "$SSBM_DISC" --mode e --matches 3 --monkey 1 --fields 7000 --port all --lockstep
//! ```
//!
//! and again with `MATCH_STAGES=31,32 MATCH_PLAYERS=4 MATCH_TEAMS=1 MATCH_TIME=20
//! --fields 6000` into `start4`.

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;

use gekko_fp::{FpMode, set_fp_mode};
use leaf_check::{MODES, disc_index, hsd_reference, sym};
use melee_capture::compare::Comparer;
use melee_capture::import::{Importer, Maps};
use melee_capture::{
    Capture, DataMap, FT_ANIM_FILES, MEM1, anim_subarchives, disc_file, find_archives_in,
    find_archives_named,
};
use melee_data::archive::{Archive, ArchiveId, Archives};
use melee_sim::game::code::{Val, stubs};
use melee_sim::game::ft::AnimFile;
use melee_sim::game::ft::create::addr;
use melee_sim::game::ft::data::kind_tables;
use melee_sim::game::pl::player::mapping;
use melee_sim::game::state::{Game, Running};
use ssbm_rt::Ctx;

const DEFAULT_CALLS: &str = "C:/Projects/ssbm-rs-publication/local/calls/fighter-create";

/// The saved states before fighter creation (`fn_8016DEEC`'s calls) in the captures'
/// directories.
fn captures() -> Vec<PathBuf> {
    let dir: PathBuf = std::env::var("FIGHTER_CREATE_CALLS")
        .unwrap_or_else(|_| DEFAULT_CALLS.into())
        .into();
    let mut found: Vec<PathBuf> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .flat_map(|d| std::fs::read_dir(d).into_iter().flatten().flatten())
        .map(|e| e.path())
        .filter(|p| {
            let n = p.file_name().unwrap().to_string_lossy();
            n.starts_with("fn_8016DEEC-call") && n.ends_with(".call")
        })
        .collect();
    found.sort();
    if found.is_empty() {
        eprintln!(
            "no captures before fighter creation in {}: make them with ssbm-rs's runner (see this file's docs)",
            dir.display()
        );
    }
    found
}

thread_local! {
    static OBJ_ALLOC: Cell<Option<ssbm_rt::Native>> = const { Cell::new(None) };
    /// ssbm-rs's side's calls into stood-in functions: address, r3, r4.
    static THEIR_CALLS: RefCell<Vec<(u32, u32, u32)>> = const { RefCell::new(Vec::new()) };
}

fn c_str(ctx: &Ctx, mut a: u32) -> String {
    let mut v = Vec::new();
    while ctx.read_u8(a) != 0 {
        v.push(ctx.read_u8(a));
        a += 1;
    }
    String::from_utf8_lossy(&v).into_owned()
}

fn disc_bytes(name: &str) -> Vec<u8> {
    let index = disc_index().expect("SSBM_DISC");
    disc_file(index, name.trim_start_matches('/'))
        .unwrap_or_else(|| panic!("{name} isn't on the disc"))
        .to_vec()
}

/// The reference's file reads, from the disc (the reference has no disc drive): a whole file
/// into memory (`lbFile_8001668C`), and a file into a heap the game picks, which for ARAM
/// keeps only the address (`lbFile_800168A0`; ARAM reads come from the disc, see
/// `leaf_check::reference`). Fighter blocks come back zeroed, and the effects' loads and the
/// items' registration are stood in for and logged.
fn install(ctx: &Ctx) {
    ctx.register(sym("lbFile_8001668C"), |ctx| {
        let (name, dst, size) = (ctx.regs.r(3), ctx.regs.r(4), ctx.regs.r(5));
        let bytes = disc_bytes(&c_str(ctx, name));
        ctx.write_bytes(dst, &bytes);
        ctx.write_u32(size, bytes.len() as u32);
    });
    ctx.register(sym("lbFile_800168A0"), |ctx| {
        let (heap, name, dst, size) = (ctx.regs.r(3), ctx.regs.r(4), ctx.regs.r(5), ctx.regs.r(6));
        let sp = ctx.regs.r(1);
        ctx.regs.set_r(1, sp - 0x40);
        ctx.regs.set_r(3, name);
        ctx.invoke(sym("lbDvd_8001819C"));
        let preloaded = ctx.regs.r(3);
        ctx.regs.set_r(3, name);
        ctx.invoke(sym("lbFileGetSize"));
        let len = ctx.regs.r(3);
        let ret = if preloaded != 0 {
            ctx.write_u32(dst, preloaded);
            1
        } else {
            ctx.regs.set_r(3, heap);
            ctx.regs.set_r(4, (len + 31) & !31);
            ctx.invoke(sym("lbHeap_80015BD0"));
            let at = ctx.regs.r(3);
            ctx.write_u32(dst, at);
            if at >= MEM1 {
                ctx.write_bytes(at, &disc_bytes(&c_str(ctx, name)));
            }
            0
        };
        ctx.write_u32(size, len);
        ctx.regs.set_r(1, sp);
        ctx.regs.set_r(3, ret);
    });
    let original = ctx.entry(sym("HSD_ObjAlloc")).expect("HSD_ObjAlloc").native;
    OBJ_ALLOC.with(|o| o.set(Some(original)));
    ctx.register(sym("HSD_ObjAlloc"), |ctx| {
        let data = ctx.regs.r(3);
        OBJ_ALLOC.with(|o| o.get()).unwrap()(ctx);
        let fighter_blocks = [
            "fighter_alloc_data",
            "fighter_dat_attrs_alloc_data",
            "fighter_parts_alloc_data",
            "fighter_dobj_list_alloc_data",
            "fighter_x2040_alloc_data",
        ];
        if fighter_blocks.iter().any(|n| sym(n) == data) {
            let at = ctx.regs.r(3);
            ctx.fill(at, 0, ctx.read_u32(data + 0x20));
            ctx.regs.set_r(3, at);
        }
    });
    ctx.register(addr::EFASYNC_LOAD_SYNC, |ctx| {
        let call = (addr::EFASYNC_LOAD_SYNC, ctx.regs.r(3), ctx.regs.r(4));
        THEIR_CALLS.with(|c| c.borrow_mut().push(call));
    });
}

fn import(cap: &Capture, data: &DataMap) -> (Game, Maps) {
    let mut imp = Importer::new(cap, data);
    imp.world();
    imp.stage();
    imp.ids();
    imp.items();
    imp.match_state();
    (imp.game, imp.maps)
}

fn compare(mem: &[u8], data: &DataMap, game: &Game, maps: Maps) -> Vec<String> {
    let mut cmp = Comparer::new(mem, data, game, maps);
    cmp.world();
    cmp.stage();
    cmp.ids();
    cmp.items();
    cmp.match_state();
    cmp.diffs
}

fn panic_text(p: &(dyn std::any::Any + Send)) -> String {
    p.downcast_ref::<String>()
        .cloned()
        .or_else(|| p.downcast_ref::<&str>().map(|s| (*s).to_owned()))
        .unwrap_or_else(|| "a panic".into())
}

/// What one capture's run came to.
enum Outcome {
    Clean(usize),
    Skipped,
    Unported(String),
}

/// Creates every playing slot's fighters from a capture on both sides in `mode`, comparing
/// after each slot.
fn run_capture(ctx: &Ctx, path: &std::path::Path, mode: FpMode, first_init: bool) -> Outcome {
    set_fp_mode(mode);
    let index = disc_index().expect("SSBM_DISC");
    let cap = Capture::read(path);
    let mut archives = Archives::default();
    let data = DataMap {
        loaded: find_archives_in(index, &cap.call.mem1, &mut archives),
    };
    // A later match's state the importer can't take (the last match's freed objects that
    // pointers outlived) is skipped and counted.
    let Ok((mut game, maps)) = catch_unwind(AssertUnwindSafe(|| import(&cap, &data))) else {
        return Outcome::Skipped;
    };
    let before: HashSet<String> = compare(&cap.call.mem1, &data, &game, maps.clone())
        .into_iter()
        .collect();
    // The fighter kinds the playing slots need, and their files on our side.
    let slots: Vec<i32> = (0..6).filter(|&s| game.player_pkind(s) != 3).collect();
    let mut named: Vec<(String, ArchiveId)> = Vec::new();
    let mut anims: Vec<(i32, String, Vec<(u32, ArchiveId)>)> = Vec::new();
    for &s in &slots {
        let m = mapping(game.player(s).ckind);
        for kind in [m.internal_id, m.extra_internal_id] {
            if kind < 0 {
                continue;
            }
            let t = kind_tables(i32::from(kind));
            let mut files: Vec<&str> = vec![t.dat_file.unwrap()];
            let c = game.player_costume(s) as usize;
            let c = if c >= usize::from(t.costume_count) {
                0
            } else {
                c
            };
            files.push(t.costumes[c].file.unwrap());
            for f in files {
                if !named.iter().any(|(n, _)| n == f) {
                    let id = archives.add(Archive::parse(&disc_bytes(f)).unwrap());
                    named.push((f.to_owned(), id));
                }
            }
            let aj = t.anim_file.unwrap();
            if !anims.iter().any(|(_, n, _)| n == aj) {
                let mut subs = Vec::new();
                for (off, bytes) in anim_subarchives(index, aj) {
                    let id = archives.add(Archive::parse(&bytes).unwrap());
                    named.push((format!("{aj}@{off:#x}"), id));
                    subs.push((off, id));
                }
                anims.push((i32::from(kind), aj.to_owned(), subs));
            }
        }
    }
    for (n, id) in &named {
        if !n.contains('@') {
            game.ft.data.files.archives.push((n.clone(), *id));
        }
    }
    // The fighters' first setup loads `PlCo.dat` again (the debug VS mode reads it from the
    // disc): a copy of its own on our side too.
    if first_init {
        let id = archives.add(Archive::parse(&disc_bytes("PlCo.dat")).unwrap());
        named.push(("PlCo.dat".into(), id));
        let files = &mut game.ft.data.files.archives;
        files.retain(|(n, _)| n != "PlCo.dat");
        files.push(("PlCo.dat".into(), id));
    }
    cap.call.load(ctx);
    let mut done = 0;
    let steps: Vec<i32> = if first_init { vec![-1] } else { Vec::new() }
        .into_iter()
        .chain(slots.iter().copied())
        .collect();
    for s in steps {
        // ssbm-rs's side: the fighters' first setup (-1), or a slot's fighters.
        THEIR_CALLS.with(|c| c.borrow_mut().clear());
        if s < 0 {
            ctx.invoke(sym("Player_80036DA4"));
        } else {
            ctx.regs.set_r(3, s as u32);
            ctx.invoke(sym("Player_80031AD0"));
        }
        let mut after = vec![0; cap.call.mem1.len()];
        let _ = ctx.mem.read_bytes(MEM1, &mut after);
        let word = |a: u32| {
            let i = (a - MEM1) as usize;
            u32::from_be_bytes(after[i..i + 4].try_into().unwrap())
        };
        // The animation files where ssbm-rs's side put them in ARAM.
        for (kind, name, subs) in &anims {
            let aram = word(FT_ANIM_FILES + 8 * *kind as u32);
            if aram != 0 && !game.ft.data.files.anim_files.iter().any(|(n, _)| n == name) {
                game.ft.data.files.anim_files.push((
                    name.clone(),
                    AnimFile {
                        aram,
                        anims: subs.clone(),
                    },
                ));
            }
        }
        // Ours.
        stubs::clear();
        stubs::set(addr::EFASYNC_LOAD_SYNC, Val::None);
        let ours = catch_unwind(AssertUnwindSafe(|| {
            let mut r = Running {
                game: &mut game,
                data: &archives,
            };
            if s < 0 {
                r.player_80036da4();
            } else {
                r.player_create_fighters(s);
            }
        }));
        if let Err(p) = ours {
            let why = panic_text(&*p);
            if why.contains("isn't ported") {
                return Outcome::Unported(why);
            }
            panic!(
                "{} slot {s} ({mode:?}): ours panicked: {why}",
                path.display()
            );
        }
        let after_data = DataMap {
            loaded: find_archives_named(index, &after, &mut archives, &data.loaded, &named),
        };
        let mut diffs = Vec::new();
        let ours = stubs::take();
        let theirs = THEIR_CALLS.with(|c| std::mem::take(&mut *c.borrow_mut()));
        if ours.len() != theirs.len() {
            diffs.push(format!(
                "stood-in calls: ours {ours:x?}, theirs {theirs:x?}"
            ));
        } else {
            for ((of, args), (tf, r3, r4)) in ours.iter().zip(&theirs) {
                let regs = [*r3, *r4];
                let same = of == tf
                    && args.iter().zip(regs).all(|(a, r)| match a {
                        Val::Int(v) => *v as u32 == r,
                        _ => true,
                    });
                if !same {
                    diffs.push(format!(
                        "stood-in call: ours {of:#x} {args:x?}, theirs {tf:#x} {r3:#x} {r4:#x}"
                    ));
                }
            }
        }
        diffs.extend(
            compare(&after, &after_data, &game, maps.clone())
                .into_iter()
                .filter(|d| !before.contains(d)),
        );
        for d in diffs.iter().take(30) {
            eprintln!("  {d}");
        }
        assert!(
            diffs.is_empty(),
            "{} slot {s} ({mode:?}): {} differences",
            path.display(),
            diffs.len()
        );
        done += 1;
    }
    Outcome::Clean(done)
}

#[test]
fn fighter_create() {
    check(false);
}

/// The fighters' first setup again from the same states, then their creation.
#[test]
fn fighter_first_init() {
    check(true);
}

fn check(first_init: bool) {
    let paths = captures();
    if paths.is_empty() || disc_index().is_none() {
        return;
    }
    std::thread::Builder::new()
        .stack_size(512 << 20)
        .spawn(move || {
            let ctx = hsd_reference().expect("SSBM_DISC");
            install(&ctx);
            let (mut created, mut skipped, mut blocked) = (0, 0, Vec::new());
            for mode in MODES {
                for p in &paths {
                    match run_capture(&ctx, p, mode, first_init) {
                        Outcome::Clean(n) => created += n,
                        Outcome::Skipped => skipped += 1,
                        Outcome::Unported(why) => blocked.push(format!("{}: {why}", p.display())),
                    }
                }
            }
            set_fp_mode(FpMode::Hardware);
            eprintln!(
                "{created} steps compared clean, {skipped} captures the importer can't take, {} blocked",
                blocked.len()
            );
            for b in &blocked {
                eprintln!("  {b}");
            }
            assert!(blocked.is_empty(), "fighter creation reaches unported code");
            assert!(created >= 8, "only {created} slots compared");
        })
        .unwrap()
        .join()
        .unwrap();
}

/// `ftParts_80074194`'s part flags on joints without DObjs, every flag bit and depth: the
/// fighters' joints set few of them.
#[test]
fn part_flags() {
    use melee_sim::game::ft::parts::{bone_from_joint, set_bone_xd};
    use melee_sim::game::structs::FighterBone;
    let Some(ctx) = leaf_check::reference() else {
        return;
    };
    let scratch = leaf_check::SCRATCH;
    let (fighter, bone, jobj, index) = (
        scratch,
        scratch + 0x3000,
        scratch + 0x3100,
        scratch + 0x3200,
    );
    let mut rand = leaf_check::Rand(0xB0E5);
    for i in 0..4000u32 {
        let flags = if i < 32 { 1 << i } else { rand.bits() as u32 };
        let depth = rand.below(40) as u32;
        let old = [rand.bits() as u8, rand.bits() as u8];
        let old_c = [rand.bits() as u8, rand.bits() as u8];
        ctx.fill(scratch, 0, 0x4000);
        ctx.write_u32(jobj + 0x14, flags);
        ctx.write_u32(jobj + 0x18, 0);
        ctx.write_u8(bone + 8, old[0]);
        ctx.write_u8(bone + 9, old[1]);
        ctx.write_u8(bone + 0xC, old_c[0]);
        ctx.write_u8(bone + 0xD, old_c[1]);
        ctx.write_u32(index, (i % 3) as u32);
        ssbm_types::fns::ftParts_80074194(
            &ctx,
            leaf_check::handle(&ctx, fighter),
            leaf_check::handle(&ctx, bone),
            leaf_check::handle(&ctx, jobj),
            leaf_check::handle(&ctx, index),
            depth,
        );
        let mut ours = FighterBone {
            x8: old,
            xC_u: [old_c[0], old_c[1], 0, 0],
            ..FighterBone::default()
        };
        bone_from_joint(&mut ours, flags, depth);
        // No DObjs: the last DObj index from the count so far, and no DObjs of its own.
        let count = (i % 3) as u8;
        set_bone_xd(&mut ours, if count != 0 { count - 1 } else { 0 });
        melee_sim::game::ft::parts::set_bone_bit(&mut ours, 1, 6, false);
        let theirs = [
            ctx.read_u8(bone + 8),
            ctx.read_u8(bone + 9),
            ctx.read_u8(bone + 0xC),
            ctx.read_u8(bone + 0xD),
        ];
        assert_eq!(
            [ours.x8[0], ours.x8[1], ours.xC_u[0], ours.xC_u[1]],
            theirs,
            "flags {flags:#x}, depth {depth}"
        );
    }
}
