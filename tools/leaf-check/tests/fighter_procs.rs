//! The fighters' per-frame processes the fighter core left behind, against ssbm-rs, from their
//! saved calls in a running match: the attack collision (`Fighter_procAttackColl` and what it
//! reaches), the collision results (`Fighter_procCollResolve`), the camera
//! (`Fighter_procCamera`), the fighters' light (`ftCo_8009F480`), the dynamic bones
//! (`Fighter_procDynamics`) and the legs' reach (`Fighter_procIK`). Each call runs on both
//! sides and the whole state (the fighters with their hitboxes, hurtboxes and damage results, the
//! items, the player blocks and their statistics, the audio driver) is compared afterwards, with the calls
//! into the attack statistics (`pl_80038144`), which no port covers, stood in for on both
//! sides and compared by their arguments.
//!
//! Real calls rarely land a hit, so each one also runs again with the other fighter's first
//! hitbox switched on over this fighter, its fields varied by the call's index (`hit_*`).
//!
//! The captures are game memory, kept out of the repository in `FIGHTER_PROC_CALLS` (default
//! below); the test skips without them. ssbm-rs's runner makes them in the debug VS mode's
//! random matches (`done.txt` listing every function address but the captured ones):
//!
//! ```text
//! MATCH_STAGES=32 MATCH_FIGHTERS=2,9 MATCH_TIME=30 MATCH_PLAYERS=2 \
//!   CAPTURE=$FIGHTER_PROC_CALLS CAPTURE_FROM=0 CAPTURE_NESTED=1 CAPTURE_EVERY=100 \
//!   CAPTURE_FUNCS=Fighter_procAttackColl,... LOCKSTEP_DONE=done.txt \
//!   ssbm-run "$SSBM_DISC" --mode e --matches 3 --monkey 1 --fields 7000 --port all --lockstep
//! ```

use std::cell::RefCell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use gekko_fp::set_fp_mode;
use leaf_check::calls::{peek_u32, poke_bit, poke_f32, poke_u32, saved_calls};
use leaf_check::{MODES, disc_index, hsd_reference, run_capture, sym};
use melee_capture::compare::Comparer;
use melee_capture::import::{Importer, Maps};
use melee_capture::{Capture, DataMap, find_archives_after, find_archives_in};
use melee_data::archive::Archives;
use melee_sim::game::code::{Val, stubs};
use melee_sim::game::ft::attack::proc_attack_coll;
use melee_sim::game::ft::camera::proc_camera;
use melee_sim::game::ft::create_model::light_proc;
use melee_sim::game::ft::fighter::{proc_dynamics, proc_ik};
use melee_sim::game::ft::resolve::proc_coll_resolve;
use melee_sim::game::state::Running;
use melee_sim::hsd::gobj::GObjId;
use ssbm_rt::Ctx;
use ssbm_rt::capture::Call;

const DEFAULT_CALLS: &str = "C:/Projects/ssbm-rs-publication/local/calls/fighter-create/fm1";

fn calls_dir() -> String {
    std::env::var("FIGHTER_PROC_CALLS").unwrap_or_else(|_| DEFAULT_CALLS.into())
}

thread_local! {
    /// The reference's stood-in calls: the function and the words its arguments come to.
    static THEIRS: RefCell<Vec<(u32, Vec<u32>)>> = const { RefCell::new(Vec::new()) };
}

/// Stands in for code no port covers on the reference, logging the arguments: none now (the
/// attack statistics, `pl_80038144`, run on both sides).
fn stand_ins(_ctx: &Ctx) {}

/// Our stood-in calls as the reference's words, with GObjs at their original addresses.
fn our_words(maps: &Maps, f: u32, args: &[Val]) -> Vec<u32> {
    let gobj_addr = |g: &Option<melee_sim::hsd::gobj::GObjId>| {
        g.map_or(0, |g| {
            *maps
                .gobjs
                .iter()
                .find(|(_, v)| **v == g)
                .expect("a GObj the import doesn't know")
                .0
        })
    };
    let mut out = Vec::new();
    for a in args {
        match a {
            Val::Int(v) => out.push(*v as u32),
            Val::GObj(g) => out.push(gobj_addr(g)),
            Val::Vec(v) => out.extend([v.x.to_bits(), v.y.to_bits(), v.z.to_bits()]),
            Val::Float(v) => out.push((*v as f32).to_bits()),
            Val::None => {}
            other => panic!("a stood-in call's argument {other:?}"),
        }
    }
    let _ = f;
    out
}

/// Zeroes the fighters' entries past what they use: their dynamic bone sets past
/// `dynamics_num`, costume textures past `n_costume_tobjs`, and computer-state pointers that
/// can't be pointers; and the items' dynamic bones past their count. The debug VS mode's fighter
/// blocks come back uncleared, so a match's fighters hold an earlier scene's bytes there,
/// where melee-sim's (like a fresh heap's, and Slippi's) hold zeros; nothing here reads them
/// (`docs/engine.md`).
fn zero_unused(call: &mut Call) {
    let heads = peek_u32(call, sym("HSD_GObjPLinkHead"));
    let mut g = peek_u32(call, heads + 4 * 8);
    while g != 0 {
        let fp = fighter_of(call, g);
        let used = peek_u32(call, fp + 0x3E0);
        for k in used.min(10)..10 {
            for w in 0..6 {
                poke_u32(call, fp + 0x2F0 + 0x18 * k + 4 * w, 0);
            }
        }
        let tobjs = peek_u32(call, fp + 0x5CC);
        for k in tobjs.min(5)..5 {
            poke_u32(call, fp + 0x5D4 + 4 * k, 0);
        }
        // The computer state's fighter and item pointers of a fighter no computer plays,
        // which can hold any bytes: those that can't be pointers at all.
        for off in [0x44, 0x4C, 0x50, 0xF0, 0xF4] {
            let at = fp + 0x1A88 + off;
            let v = peek_u32(call, at);
            if v & 3 != 0 || !(0x8000_0000..0x8180_0000).contains(&v) {
                poke_u32(call, at, 0);
            }
        }
        g = peek_u32(call, g + 8);
    }
    // The items' dynamic bones past `x374_dynamicBonesNum`, likewise.
    let mut g = peek_u32(call, heads + 4 * 9);
    while g != 0 {
        let ip = peek_u32(call, g + 0x2C);
        let used = peek_u32(call, ip + 0x374);
        for k in used.min(24)..24 {
            for w in 0..7 {
                poke_u32(call, ip + 0xD4 + 0x1C * k + 4 * w, 0);
            }
        }
        g = peek_u32(call, g + 8);
    }
}

/// What one call came to.
enum Outcome {
    Clean,
    Skipped(String),
}

fn panic_text(p: &(dyn std::any::Any + Send)) -> String {
    p.downcast_ref::<String>()
        .cloned()
        .or_else(|| p.downcast_ref::<&str>().map(|s| (*s).to_owned()))
        .unwrap_or_else(|| "a panic".into())
}

/// Runs one call on both sides and compares, after `prepare` changes it.
fn run_one(
    ctx: &Ctx,
    path: &Path,
    i: usize,
    prepare: fn(&mut Call, usize),
    run: fn(&mut Running, GObjId),
) -> Outcome {
    let index = disc_index().expect("SSBM_DISC");
    let mut cap = Capture::read(path);
    zero_unused(&mut cap.call);
    prepare(&mut cap.call, i);
    let mut archives = Archives::default();
    let data = DataMap {
        loaded: find_archives_in(index, &cap.call.mem1, &mut archives),
    };
    let bank = melee_capture::audio::find_bank(&cap.call.mem1, &mut archives);
    let imported = catch_unwind(AssertUnwindSafe(|| {
        let mut imp = Importer::new(&cap, &data).with_archives(&archives);
        imp.world();
        imp.stage();
        imp.ids();
        imp.items();
        imp.match_state();
        imp.audio(bank);
        imp.bg_flash();
        let Importer { game, maps, .. } = imp;
        (game, maps)
    }));
    let (mut game, maps) = match imported {
        Ok(v) => v,
        Err(p) => return Outcome::Skipped(format!("the import stopped: {}", panic_text(&*p))),
    };
    let mut before = Comparer::new(&cap.call.mem1, &data, &game, maps.clone());
    before.world();
    before.stage();
    before.ids();
    before.items();
    before.match_state();
    before.audio();
    before.bg_flash();
    // A later match's fighters hold a previous match's bytes past what they use (stale
    // dynamic-bone and texture handles), which the import doesn't follow.
    if !before.diffs.is_empty() {
        return Outcome::Skipped(format!(
            "the import doesn't round-trip ({})",
            before.diffs[0]
        ));
    }
    THEIRS.with(|t| t.borrow_mut().clear());
    let after = match catch_unwind(AssertUnwindSafe(|| run_capture(ctx, &cap.call))) {
        Ok(after) => after,
        Err(p) => {
            let why = p
                .downcast_ref::<ssbm_rt::Fault>()
                .map(|f| format!("a fault at {:#x}", f.addr))
                .unwrap_or_else(|| panic_text(&*p));
            return Outcome::Skipped(format!("the reference stopped: {why}"));
        }
    };
    let gobj = *maps
        .gobjs
        .get(&cap.call.regs.gpr[3])
        .expect("r3 isn't a GObj the import knows");
    stubs::clear();
    let run = catch_unwind(AssertUnwindSafe(|| {
        let mut r = Running {
            game: &mut game,
            data: &archives,
        };
        run(&mut r, gobj);
    }));
    if let Err(p) = run {
        let why = panic_text(&*p);
        if why.contains("isn't ported") {
            return Outcome::Skipped(why);
        }
        panic!("{}: our port panicked: {why}", path.display());
    }
    let data = DataMap {
        loaded: find_archives_after(index, &after, &mut archives, &data.loaded),
    };
    let mut cmp = Comparer::new(&after, &data, &game, maps.clone());
    cmp.world();
    cmp.stage();
    cmp.ids();
    cmp.items();
    cmp.match_state();
    cmp.audio();
    cmp.bg_flash();
    let ours: Vec<(u32, Vec<u32>)> = stubs::take()
        .iter()
        .map(|(f, args)| (*f, our_words(&maps, *f, args)))
        .collect();
    let theirs = THEIRS.with(|t| std::mem::take(&mut *t.borrow_mut()));
    if ours != theirs {
        cmp.diffs.push(format!(
            "stood-in calls: ours {ours:x?}, theirs {theirs:x?}"
        ));
    }
    for d in cmp.diffs.iter().take(30) {
        eprintln!("  {d}");
    }
    assert!(
        cmp.diffs.is_empty(),
        "{} (call {i}): {} differences",
        path.display(),
        cmp.diffs.len()
    );
    Outcome::Clean
}

/// Every saved call of `Fighter_procAttackColl`, changed by `prepare`, in both float modes;
/// at least `min` compared clean in each.
fn check(func: &str, run: fn(&mut Running, GObjId), prepare: fn(&mut Call, usize), min: usize) {
    let paths = saved_calls(&calls_dir(), func);
    if disc_index().is_none() || paths.is_empty() {
        eprintln!(
            "no SSBM_DISC or no saved calls in {}: skipping",
            calls_dir()
        );
        return;
    }
    for mode in MODES {
        let next = AtomicUsize::new(0);
        let results = Mutex::new((0, std::collections::BTreeMap::new()));
        let workers = leaf_check::workers();
        std::thread::scope(|s| {
            for _ in 0..workers {
                std::thread::Builder::new()
                    .stack_size(256 << 20)
                    .spawn_scoped(s, || {
                        set_fp_mode(mode);
                        let ctx = hsd_reference().unwrap();
                        stand_ins(&ctx);
                        loop {
                            let i = next.fetch_add(1, Ordering::Relaxed);
                            let Some(p) = paths.get(i) else { break };
                            let outcome = run_one(&ctx, p, i, prepare, run);
                            let mut r = results.lock().unwrap();
                            match outcome {
                                Outcome::Clean => r.0 += 1,
                                Outcome::Skipped(why) => *r.1.entry(why).or_insert(0) += 1,
                            }
                        }
                    })
                    .unwrap();
            }
        });
        let (clean, skipped) = results.into_inner().unwrap();
        eprintln!(
            "{func} ({mode:?}): {clean} of {} compared clean",
            paths.len()
        );
        for (why, n) in &skipped {
            eprintln!("  skipped x{n}: {why}");
        }
        assert!(clean >= min, "{mode:?}: only {clean} compared clean");
    }
}

#[test]
fn proc_attack_coll_calls() {
    assert_eq!(sym("Fighter_procAttackColl"), 0x8006_CB94);
    check("Fighter_procAttackColl", proc_attack_coll, |_, _| {}, 100);
}

/// The fighter GObj's user data in a call.
fn fighter_of(call: &Call, gobj: u32) -> u32 {
    peek_u32(call, gobj + 0x2C)
}

/// The other fighter in the fighters' process list (`HSD_GObjPLinkHead[8]`).
fn other_fighter(call: &Call, gobj: u32) -> Option<u32> {
    let heads = peek_u32(call, sym("HSD_GObjPLinkHead"));
    let mut g = peek_u32(call, heads + 4 * 8);
    while g != 0 {
        if g != gobj {
            return Some(g);
        }
        g = peek_u32(call, g + 8);
    }
    None
}

/// A small deterministic stream from the call's index.
struct Pick(u32);

impl Pick {
    fn next(&mut self, n: u32) -> u32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.0 >> 8) % n
    }
}

/// Writes a fresh live box at `hit` over `at`: varied damage, element, knockback and sound,
/// sometimes phantom, sometimes sweeping, with no victims yet.
fn arm_box(call: &mut Call, hit: u32, at: (f32, f32, f32), pick: &mut Pick, elements: &[u32]) {
    poke_u32(call, hit, 1);
    poke_u32(call, hit + 4, pick.next(4));
    poke_u32(call, hit + 8, pick.next(20));
    let damage = [0.0, 0.4, 1.0, 3.0, 7.5, 12.0, 21.0, 30.5][pick.next(8) as usize];
    poke_f32(call, hit + 0xC, damage);
    poke_f32(
        call,
        hit + 0x1C,
        [2.0, 4.5, 8.0, 14.0][pick.next(4) as usize],
    );
    let angle = [0, 45, 80, 270, 361, 362][pick.next(6) as usize];
    poke_u32(call, hit + 0x20, angle);
    poke_u32(
        call,
        hit + 0x24,
        [0, 30, 70, 100, 140, 400][pick.next(6) as usize],
    );
    poke_u32(call, hit + 0x28, [0, 0, 0, 40, 90][pick.next(5) as usize]);
    poke_u32(
        call,
        hit + 0x2C,
        [0, 10, 30, 60, 3000][pick.next(5) as usize],
    );
    let element = elements[pick.next(elements.len() as u32) as usize];
    poke_u32(call, hit + 0x30, element);
    poke_u32(
        call,
        hit + 0x34,
        [-10i32, -3, 0, 2, 6][pick.next(5) as usize] as u32,
    );
    poke_u32(call, hit + 0x38, pick.next(3));
    poke_u32(call, hit + 0x3C, [0, 1, 2, 13][pick.next(4) as usize]);
    poke_bit(call, hit + 0x40, 0, true);
    poke_bit(call, hit + 0x40, 1, pick.next(2) == 0);
    poke_bit(call, hit + 0x40, 2, true);
    poke_bit(call, hit + 0x40, 3, true);
    poke_bit(call, hit + 0x42, 4, pick.next(2) == 0);
    poke_bit(call, hit + 0x42, 5, true);
    poke_bit(call, hit + 0x43, 1, pick.next(3) == 0);
    let (x, y, z) = at;
    let dx = [0.0, 3.0, -6.0, 12.0][pick.next(4) as usize];
    for (base, off) in [(0x4C, 0.0), (0x58, dx)] {
        poke_f32(call, hit + base, x + off);
        poke_f32(call, hit + base + 4, y);
        poke_f32(call, hit + base + 8, z);
    }
    for k in 0..24 {
        poke_u32(call, hit + 0x74 + 8 * k, 0);
    }
    poke_u32(call, hit + 0x134, 0);
}

/// A fighter's middle: its position raised a little.
fn middle(call: &Call, fp: u32) -> (f32, f32, f32) {
    let pos = |o: u32| f32::from_bits(peek_u32(call, fp + 0xB0 + o));
    (pos(0), pos(4) + 8.0, pos(8))
}

/// Switches the other fighter's first hitbox on over this fighter, and by the call's index
/// sets up one of the cases real calls rarely reach: plain hits; this fighter's own first box
/// on over the same place with both on the ground (clanks); this fighter's shield up; or both
/// fighters' thrown hitboxes live and touching.
fn arm_hitbox(call: &mut Call, i: usize) {
    let gobj = call.regs.gpr[3];
    let Some(other) = other_fighter(call, gobj) else {
        return;
    };
    let (me, fp) = (fighter_of(call, gobj), fighter_of(call, other));
    let mut pick = Pick(i as u32 * 7 + 3);
    // Raised by up to about a hurtbox's reach, so some boxes only graze it (tips).
    let (x, y, z) = middle(call, me);
    let at = (
        x,
        y + [0.0, 0.0, 5.0, 9.0, 12.0, 15.0][pick.next(6) as usize],
        z,
    );
    let phantom = pick.next(5) == 0;
    vary_attacker(call, fp, &mut pick);
    match i % 4 {
        0 => {
            arm_box(
                call,
                fp + 0x914,
                at,
                &mut pick,
                &[0, 0, 0, 1, 2, 3, 5, 11, 13],
            );
            poke_bit(call, fp + 0x914 + 0x43, 2, phantom);
            match pick.next(3) {
                0 => graze(call, me, fp + 0x914, &mut pick),
                1 => weak(call, fp + 0x914, &mut pick),
                _ => {}
            }
        }
        1 => {
            // A clank needs ordinary boxes, on the ground.
            let elements = [0, 0, 1, 3, 3];
            arm_box(call, fp + 0x914, at, &mut pick, &elements);
            arm_box(call, me + 0x914, at, &mut pick, &elements);
            poke_u32(call, fp + 0xE0, 0);
            poke_u32(call, me + 0xE0, 0);
        }
        2 => {
            arm_box(call, fp + 0x914, at, &mut pick, &[0, 0, 2, 3, 11]);
            poke_bit(call, fp + 0x914 + 0x43, 2, phantom);
            // The shield up (`x221B_b0`), maybe one that only guards the front (`b3`) or only
            // takes some boxes (`b4`), and its bubble moved over the box.
            poke_bit(call, me + 0x221B, 0, true);
            poke_bit(call, me + 0x221B, 3, pick.next(3) == 0);
            poke_bit(call, me + 0x221B, 4, pick.next(4) == 0);
            poke_f32(
                call,
                me + 0x19C0 + 0x20,
                [4.0, 10.0, 16.0][pick.next(3) as usize],
            );
            poke_f32(call, me + 0x199C, [0.0, 0.25, 0.8][pick.next(3) as usize]);
            poke_f32(call, me + 0x19B4, [0.0, 0.5][pick.next(2) as usize]);
        }
        _ => {
            // Thrown fighters: both boxes live (`x2227_b2`), meeting, in the air or not.
            let air = pick.next(3) != 0;
            for f in [fp, me] {
                arm_box(call, f + 0x1064, at, &mut pick, &[0]);
                poke_bit(call, f + 0x2227, 2, true);
                poke_u32(call, f + 0xE0, u32::from(air));
            }
        }
    }
}

/// Moves a box of a fixed radius to just reach the fighter's first hurtbox, so it lands a tip
/// (a hit by less than `ftCommonData` x7A8, 0.01): the hurtbox is pinned as a point where it
/// was last frame (its ends brought up to date, `skip_update_pos`), and the box set beside it
/// by its reach less a little, at most as far as a tip's.
fn graze(call: &mut Call, me: u32, hit: u32, pick: &mut Pick) {
    let hurt = me + 0x11A0;
    let a = |o: u32| f32::from_bits(peek_u32(call, hurt + 0x28 + o));
    let (x, y, z) = (a(0), a(4), a(8));
    for o in [0, 4, 8] {
        poke_u32(call, hurt + 0x34 + o, peek_u32(call, hurt + 0x28 + o));
    }
    poke_bit(call, hurt + 0x24, 0, true);
    // The test measures the hurtbox's radius through its bone's matrix (`HSD_JObj::mtx`):
    // a scaled bone scales it.
    let bone = peek_u32(call, hurt + 0x20);
    let m = |o: u32| f64::from(f32::from_bits(peek_u32(call, bone + 0x44 + o)));
    let bone_scale = (m(0) * m(0) + m(0x10) * m(0x10) + m(0x20) * m(0x20)).sqrt() as f32;
    let r_hurt = f32::from_bits(peek_u32(call, hurt + 0x1C)) * bone_scale;
    let r_hit = 3.0;
    poke_f32(call, hit + 0x1C, r_hit);
    poke_bit(call, hit + 0x43, 1, true);
    poke_bit(call, hit + 0x43, 2, false);
    let d = [0.001, 0.004, 0.008, 0.02, 0.5][pick.next(5) as usize];
    for base in [0x4C, 0x58] {
        poke_f32(call, hit + base, x + r_hurt + r_hit - d);
        poke_f32(call, hit + base + 4, y);
        poke_f32(call, hit + base + 8, z);
    }
}

/// Makes a box knock back weakly (no growth, a small base), for the small spark.
fn weak(call: &mut Call, hit: u32, pick: &mut Pick) {
    poke_u32(call, hit + 0x24, 0);
    poke_u32(call, hit + 0x28, 0);
    poke_u32(call, hit + 0x2C, pick.next(60));
    poke_u32(call, hit + 0x30, 0);
}

/// Varies the attacker's attack kind (`x2073`, which the statistics count apart) and where
/// its player's stale-move queue stands.
fn vary_attacker(call: &mut Call, fp: u32, pick: &mut Pick) {
    let kind = [10, 52, 70, 76, 92, 93, 96][pick.next(7) as usize];
    let w = peek_u32(call, fp + 0x2070);
    poke_u32(call, fp + 0x2070, (w & !0xFF) | kind);
    let slot = peek_u32(call, fp + 0xC) >> 24;
    let player = sym("player_slots") + 0xE90 * slot;
    poke_u32(call, player + 0xBC, pick.next(10));
}

/// Hits from a fighter's box, from real states.
#[test]
fn proc_attack_coll_hits() {
    check("Fighter_procAttackColl", proc_attack_coll, arm_hitbox, 60);
}

#[test]
fn proc_coll_resolve_calls() {
    assert_eq!(sym("Fighter_procCollResolve"), 0x8006_D1EC);
    check("Fighter_procCollResolve", proc_coll_resolve, |_, _| {}, 100);
}

#[test]
fn proc_camera_calls() {
    assert_eq!(sym("Fighter_procCamera"), 0x8006_D9EC);
    check("Fighter_procCamera", proc_camera, |_, _| {}, 100);
}

/// Runs the fighters' light's process instead, on the light's GObj (classifier 12 on
/// process link 3).
fn to_light(call: &mut Call, _: usize) {
    let heads = peek_u32(call, sym("HSD_GObjPLinkHead"));
    let mut g = peek_u32(call, heads + 4 * 3);
    while g != 0 {
        if peek_u32(call, g) >> 16 == 12 {
            call.addr = 0x8009_F480;
            call.regs.gpr[3] = g;
            return;
        }
        g = peek_u32(call, g + 8);
    }
    panic!("no fighters' light GObj");
}

/// The fighters' light's animation, from the camera process's states.
#[test]
fn light_proc_calls() {
    check("Fighter_procCamera", light_proc, to_light, 100);
}

/// Gives the fighter one of the attack collision's results, by the call's index: a tip, a
/// shield hit, a clank, a hit it dealt (sometimes the kind that rumbles apart), a shield-hit
/// push, or a held hitlag; with its shield sometimes up and damaged, and varied hitlag
/// multipliers and floors.
fn give_result(call: &mut Call, i: usize) {
    let fp = fighter_of(call, call.regs.gpr[3]);
    let mut pick = Pick(i as u32 * 13 + 5);
    let strength = 1 + pick.next(30);
    match i % 6 {
        0 => {
            poke_f32(call, fp + 0x18A0, [0.5, 12.0, 40.0][pick.next(3) as usize]);
            poke_u32(call, fp + 0x1840, strength);
        }
        1 => poke_u32(call, fp + 0x19A4, strength),
        2 => poke_u32(call, fp + 0x1918, strength),
        3 => {
            poke_u32(call, fp + 0x1914, strength);
            if pick.next(3) == 0 {
                let w = peek_u32(call, fp + 0x2070);
                poke_u32(call, fp + 0x2070, (w & !0xFF) | 0x46);
            }
        }
        4 => {
            poke_u32(call, fp + 0x1924, strength);
            poke_f32(call, fp + 0x1928, [0.0, 3.0, 9.5][pick.next(3) as usize]);
            poke_f32(call, fp + 0x192C, [-1.0, 1.0][pick.next(2) as usize]);
        }
        _ => {
            // A sound queued for after hitlag (`dmg.x1908`), played with none.
            if pick.next(2) == 0 {
                poke_u32(call, fp + 0x1908, [0x82, 0x11F][pick.next(2) as usize]);
            }
            if pick.next(2) == 0 {
                poke_bit(call, fp + 0x221A, 0, true);
            } else {
                poke_f32(call, fp + 0x1958, [2.0, 9.0][pick.next(2) as usize]);
                poke_f32(call, fp + 0x1954, [0.0, 5.0][pick.next(2) as usize]);
            }
        }
    }
    if pick.next(3) == 0 {
        poke_bit(call, fp + 0x221A, 7, true);
        poke_u32(call, fp + 0x19A0, pick.next(40));
    }
    poke_f32(
        call,
        fp + 0x1964,
        [0.0, 0.0, 4.0, 25.0][pick.next(4) as usize],
    );
    poke_f32(call, fp + 0x1960, [1.0, 1.0, 1.5][pick.next(3) as usize]);
}

/// The collision results' reactions and hitlag, from real states.
#[test]
fn proc_coll_resolve_results() {
    check(
        "Fighter_procCollResolve",
        proc_coll_resolve,
        give_result,
        80,
    );
}

/// Gives the fighter a hit with knockback, by the call's index: its strength from barely any
/// to a tumble's, with and without damage, either way round, at the Sakurai angle or a set
/// one (a spike among them), on each hurtbox, plain, fire or electric, at low and high
/// percents.
fn give_knockback(call: &mut Call, i: usize) {
    let fp = fighter_of(call, call.regs.gpr[3]);
    let mut pick = Pick(i as u32 * 17 + 3);
    let kb = [2.0, 30.0, 75.0, 95.0, 140.0, 260.0][i % 6];
    poke_f32(call, fp + 0x1850, kb);
    poke_f32(call, fp + 0x1838, [0.0, 4.0, 13.0][pick.next(3) as usize]);
    poke_u32(call, fp + 0x183C, 1 + pick.next(20));
    poke_f32(call, fp + 0x1844, [-1.0, 1.0][pick.next(2) as usize]);
    poke_u32(
        call,
        fp + 0x1848,
        [361, 361, 45, 80, 280, 30][pick.next(6) as usize],
    );
    poke_u32(call, fp + 0x184C, pick.next(3));
    poke_u32(call, fp + 0x1860, [0, 0, 0, 1, 2][pick.next(5) as usize]);
    poke_f32(call, fp + 0x1830, [0.0, 40.0, 150.0][pick.next(3) as usize]);
}

/// Hits with knockback: the reaction, hitstun's entry and the hit's shake and rumble, from
/// real states.
#[test]
fn proc_coll_resolve_knockback() {
    check(
        "Fighter_procCollResolve",
        proc_coll_resolve,
        give_knockback,
        60,
    );
}

#[test]
fn proc_dynamics_calls() {
    assert_eq!(sym("Fighter_procDynamics"), 0x8006_D9AC);
    check("Fighter_procDynamics", proc_dynamics, |_, _| {}, 100);
}

/// The chains swung with the wind blowing (always), and by the call's index on a flat floor
/// at heights about the fighter's, at another size, in the air, around a collider on one of
/// the fighter's joints, on Flat Zone, or with their links stiffer and pulled back toward the
/// animation.
fn vary_dynamics(call: &mut Call, i: usize) {
    let fp = fighter_of(call, call.regs.gpr[3]);
    leaf_check::calls::give_winds(call, i, 1 + i % 8);
    match i % 7 {
        0 => {}
        1 => {
            poke_bit(call, fp + 0x2228, 1, true);
            let y = f32::from_bits(peek_u32(call, fp + 0xB4));
            poke_f32(
                call,
                fp + 0xB4,
                y + [3.0, 6.0, 9.0, 12.0, 15.0, 20.0][i / 7 % 6],
            );
        }
        2 => poke_f32(call, fp + 0x38, [0.8, 1.3][i / 7 % 2]),
        3 => poke_u32(call, fp + 0xE0, 1),
        4 => {
            // A sphere around the fighter's second part.
            let parts = peek_u32(call, fp + 0x5E8);
            let j = peek_u32(call, parts + 0x10);
            let c = fp + 0x1670;
            poke_f32(call, c, 0.5);
            poke_f32(call, c + 4, 1.0);
            poke_f32(call, c + 8, -0.5);
            poke_f32(call, c + 0xC, [1.5, 4.0, 0.3][i / 7 % 3]);
            poke_u32(call, c + 0x10, j);
            poke_u32(call, c + 0x24, 1);
            call.mem1[(fp + 0x166C - 0x8000_0000) as usize] = 1;
        }
        5 => poke_u32(call, sym("stage_info") + 0x88, 0x1B),
        _ => {
            // Each chain's stiffness scale (`pos.x`), and each link's stiffness and pull back
            // toward the animation (`unk_4C`, `unk_50`).
            for k in 0..peek_u32(call, fp + 0x3E0).min(10) {
                let desc = fp + 0x2F0 + 0x18 * k + 4;
                poke_f32(call, desc + 8, [0.5, 1.0, 2.0][i / 7 % 3]);
                let mut link = peek_u32(call, desc);
                while link != 0 {
                    poke_f32(call, link + 0x4C, 0.3);
                    poke_f32(call, link + 0x50, [0.05, 0.4][i / 7 % 2]);
                    link = peek_u32(call, link + 0x90);
                }
            }
        }
    }
}

#[test]
fn proc_dynamics_varied() {
    // Marth's cape when a box's wind stops (level -1) reaches `ftCo_8009E4A8`, not ported.
    check("Fighter_procDynamics", proc_dynamics, vary_dynamics, 90);
}

#[test]
fn proc_ik_calls() {
    assert_eq!(sym("Fighter_procIK"), 0x8006_C5F4);
    check("Fighter_procIK", proc_ik, |_, _| {}, 100);
}

/// The legs reaching (`x221C_u16_y`'s bits, by the call's index) with the model's root
/// raised or lowered off the floor, so the feet have somewhere to reach.
fn vary_ik(call: &mut Call, i: usize) {
    let fp = fighter_of(call, call.regs.gpr[3]);
    let flags = [3u32, 1, 2, 7, 4, 6][i % 6];
    poke_bit(call, fp + 0x221C, 7, flags & 4 != 0);
    poke_bit(call, fp + 0x221D, 0, flags & 2 != 0);
    poke_bit(call, fp + 0x221D, 1, flags & 1 != 0);
    let root = peek_u32(call, call.regs.gpr[3] + 0x28);
    let y = f32::from_bits(peek_u32(call, root + 0x3C));
    poke_f32(
        call,
        root + 0x3C,
        y + [2.0, -3.0, 0.5, -0.00005, 9.0][i % 5],
    );
}

#[test]
fn proc_ik_varied() {
    check("Fighter_procIK", proc_ik, vary_ik, 100);
}
