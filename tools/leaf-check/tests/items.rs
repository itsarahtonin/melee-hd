//! The items (`melee/it`) against ssbm-rs from real game states: each test imports every
//! capture of one function, runs ssbm-rs's side and ours from the same state, in both float
//! modes, and compares the whole state afterwards (the scheduler, every joint tree, fighters,
//! items, the stage and the camera) and what the call returned.
//!
//! The captures are game memory, kept out of the repository in `ITEM_CALLS` (default below,
//! beside the function lists the commands use); tests skip when it has no capture of their
//! function. Both sets come from ssbm-rs's runner (its main since the state trace landed there,
//! built with `cargo build --release -p ssbm-run`) on a Fox against
//! Sheik replay on Final Destination, with `items-funcs.txt` listing the item units' functions
//! (item.c through it_279C.c, itfoxlaser.c and itfoxblaster.c: `ADDR NAME` per line, from the
//! decomp's symbols.txt), `procs-funcs.txt` the item processes and creation functions, and
//! `done-nonitems.txt` every other function's address:
//!
//! ```text
//! R="target/release/ssbm-run $SSBM_DISC --replay Game_20260129T133652.slp --port all \
//!   --lockstep --known local/replays/known/Game_20260129T133652.txt"
//! CAPTURE=$ITEM_CALLS/trial CAPTURE_FROM=300 CAPTURE_NESTED=1 CAPTURE_FUNCS=@items-funcs.txt \
//!   LOCKSTEP_DONE=done-nonitems.txt $R --fields 1500
//! CAPTURE=$ITEM_CALLS/fox-fd CAPTURE_FROM=300 CAPTURE_EVERY=300 CAPTURE_NESTED=1 \
//!   CAPTURE_FUNCS=@procs-funcs.txt LOCKSTEP_DONE=done-nonitems.txt $R --fields 5000
//! ```
//!
//! The debug VS set (`debug1`) is a match of two Pikachus on Battlefield with the item switch
//! narrowed to the ported kinds, the monkey playing; its item pool grows into heap that holds
//! the opening movie's frame (docs/oracle.md), which [`debug_round_trips`] checks the import
//! with:
//!
//! ```text
//! MATCH_FIGHTERS=13 MATCH_STAGES=31 MATCH_PLAYERS=2 MATCH_TIME=480 UNLOCK_ALL=1 MONKEY_B=2 \
//!   MATCH_ITEMS=0,1,2,4,6,7,8,9,11,12,14,15,28 STALL_BEATS=200 CAPTURE_INTERACTED=1 \
//!   CAPTURE=$ITEM_CALLS/debug1 CAPTURE_FROM=0 CAPTURE_NESTED=1 CAPTURE_EVERY=50 \
//!   CAPTURE_FUNCS=@items-funcs.txt LOCKSTEP_DONE=done-nonitems.txt \
//!   ssbm-run "$SSBM_DISC" --mode e --matches 21 --monkey 21 --fields 6000 --port all --lockstep
//! ```
//!
//! The replay set (`oracle-items-ported-fd`) comes from milestone 4's items-on pilot, the
//! ported kinds on Final Destination (`melee-bench oracle-sets`, docs/oracle.md), played from
//! its recording as the character sets are, every thousandth call saved (2,020 calls, 11 GB as
//! files before `pack-calls`; every fiftieth made 98 GB by field 6,000):
//!
//! ```text
//! SLIPPI_PLAYER_NAMES=0 CAPTURE_INTERACTED=1 CAPTURE=$ITEM_CALLS/oracle-items-ported-fd \
//!   CAPTURE_FROM=0 CAPTURE_NESTED=1 CAPTURE_EVERY=1000 CAPTURE_FUNCS=@items-funcs.txt \
//!   LOCKSTEP_DONE=done-nonitems.txt ssbm-run "$SSBM_DISC" --replay \
//!   $SSBM_RS_LOCAL/bench/oracle-m4/items-ported-fd.slp --fp hardware --fields 7440 --port all \
//!   --lockstep
//! ```
//!
//! The runner's mutated and mismatching saves aren't used and can be deleted.

use melee_local_data::ssbm_rs_local;
use std::path::PathBuf;
use std::sync::{Condvar, Mutex, OnceLock};

use leaf_check::fp::{FpMode, set_fp_mode};
use leaf_check::{MODES, reference};
use melee_capture::compare::Comparer;
use melee_capture::corpus::Corpus;
use melee_capture::import::{Importer, Maps};
use melee_capture::{Capture, DataMap, DiscIndex, MEM1, find_archives_in};
use melee_data::archive::{Archives, DataRef};
use melee_sim::game::handles::ItemId;
use melee_sim::game::mirror::{Cx, Mem, Mirror};
use melee_sim::game::state::{Game, Running};
use melee_sim::game::structs::{Item, SpawnItem};
use melee_sim::hsd::gobj::GObjId;
use melee_sim::hsd::jobj::JObjId;
use melee_sim::vec::Vec3;
use ssbm_rt::Ctx;

const DEFAULT_CALLS: &str = "calls/items";

fn calls_dir() -> PathBuf {
    std::env::var("ITEM_CALLS")
        .unwrap_or_else(|_| ssbm_rs_local(DEFAULT_CALLS))
        .into()
}

/// Every capture of `func` under the calls directory, in name order.
fn captures(func: &str) -> Vec<Source> {
    let mut out = Vec::new();
    let Ok(dirs) = std::fs::read_dir(calls_dir()) else {
        return out;
    };
    for d in dirs.flatten() {
        for f in melee_capture::pack::entries(d.path()) {
            let name = f.file_name().unwrap().to_string_lossy().into_owned();
            if name.starts_with(&format!("{func}-call")) && name.ends_with(".call") {
                out.push(Source::File(f));
            }
        }
    }
    out.sort_by_key(Source::name);
    let addr = leaf_check::sym(func);
    for (name, c) in corpora() {
        out.extend(
            (0..c.len())
                .filter(|&i| c.addr(i) == addr)
                .map(|i| Source::Corpus(c, i, format!("{name}#{i}"))),
        );
    }
    out
}

/// Every corpus in the calls directory (`NAME.corpus`), read once.
fn corpora() -> &'static [(String, Corpus)] {
    static CORPORA: OnceLock<Vec<(String, Corpus)>> = OnceLock::new();
    CORPORA.get_or_init(|| {
        let Ok(files) = std::fs::read_dir(calls_dir()) else {
            return Vec::new();
        };
        let mut out: Vec<(String, Corpus)> = files
            .flatten()
            .filter(|f| f.path().extension().is_some_and(|e| e == "corpus"))
            .map(|f| {
                let name = f.file_name().to_string_lossy().into_owned();
                (name, Corpus::read(&f.path()))
            })
            .collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    })
}

/// A saved call: a file of its own, or one of a corpus's.
enum Source {
    File(PathBuf),
    Corpus(&'static Corpus, usize, String),
}

impl Source {
    fn read(&self) -> Capture {
        match self {
            Source::File(p) => Capture::read(p),
            Source::Corpus(c, i, _) => c.capture(*i),
        }
    }

    fn name(&self) -> String {
        match self {
            Source::File(p) => p.file_name().unwrap().to_string_lossy().into_owned(),
            Source::Corpus(_, _, n) => n.clone(),
        }
    }
}

struct Env {
    ctx: Ctx,
    /// The disc's archives, indexed once for every check.
    index: &'static DiscIndex,
}

fn env() -> Option<Env> {
    let ctx = reference()?;
    Some(Env {
        ctx,
        index: leaf_check::disc_index()?,
    })
}

/// A turn among the checks running at once (each holds a few captured states).
struct Turn;

static TURNS: (Mutex<usize>, Condvar) = (Mutex::new(0), Condvar::new());

impl Turn {
    /// How many at once: [`leaf_check::sets`] (`LEAF_CHECK_SETS`), as many as the sets of saved
    /// calls other checks run at once.
    fn max() -> usize {
        leaf_check::sets()
    }

    fn take() -> Turn {
        let (count, freed) = &TURNS;
        let mut n = count.lock().unwrap_or_else(|e| e.into_inner());
        while *n >= Self::max() {
            n = freed.wait(n).unwrap_or_else(|e| e.into_inner());
        }
        *n += 1;
        Turn
    }
}

impl Drop for Turn {
    fn drop(&mut self) {
        let (count, freed) = &TURNS;
        *count.lock().unwrap_or_else(|e| e.into_inner()) -= 1;
        freed.notify_one();
    }
}

/// What the call returned or wrote outside the state, to compare with ssbm-rs's.
#[derive(Debug)]
enum Ret {
    None,
    Bool(bool),
    Int(u32),
    /// A GObj, which pairs with the address ssbm-rs returned.
    GObj(Option<GObjId>),
    /// A joint, likewise.
    JObj(Option<JObjId>),
    /// Words the call wrote at an address outside the state (an out parameter or a stack
    /// local), as they should read afterwards.
    Words(u32, Vec<u32>),
    /// A float returned in f1.
    Float(f64),
    /// A bool returned in r3, and words written at an address as with `Words`.
    BoolWords(bool, u32, Vec<u32>),
    /// An integer returned in r3, and words written as with `Words`.
    IntWords(u32, u32, Vec<u32>),
}

/// A captured call, as our side's tests read its arguments.
struct Args<'a> {
    cap: &'a Capture,
    maps: &'a Maps,
    data: &'a DataMap,
}

impl Args<'_> {
    /// The `i`th integer argument (r3 on).
    fn int(&self, i: usize) -> u32 {
        self.cap.call.regs.gpr[3 + i]
    }

    /// The `i`th float argument (f1 on), as the original passes a single.
    fn float(&self, i: usize) -> f32 {
        f32::from_bits(leaf_check::fp::stfs(self.cap.call.regs.fpr[1 + i].ps0))
    }

    /// The GObj in the `i`th argument.
    fn gobj(&self, i: usize) -> GObjId {
        self.maps.gobjs[&self.int(i)]
    }

    /// The GObj in the `i`th argument, or none for null.
    fn opt_gobj(&self, i: usize) -> Option<GObjId> {
        (self.int(i) != 0).then(|| self.gobj(i))
    }

    /// The item an `Item*` argument points to.
    fn item(&self, i: usize) -> ItemId {
        self.maps.items[&self.int(i)]
    }

    /// The GObj of the item an `Item*` argument points to.
    fn item_gobj(&self, i: usize) -> GObjId {
        self.maps.gobjs[&self.cap.u32(self.int(i) + 4)]
    }

    /// The joint a `HSD_JObj*` argument points to.
    fn jobj(&self, i: usize) -> Option<JObjId> {
        (self.int(i) != 0).then(|| self.maps.jobjs[&self.int(i)])
    }

    /// Game data a pointer argument points to.
    fn data(&self, i: usize) -> Option<DataRef> {
        (self.int(i) != 0).then(|| self.data.get(self.int(i)).expect("game data"))
    }

    /// The `Vec3` a pointer argument points to.
    fn vec(&self, i: usize) -> Vec3 {
        let a = self.int(i);
        let f = |o: u32| f32::from_bits(self.cap.u32(a + o));
        Vec3::new(f(0), f(4), f(8))
    }

    /// The struct a pointer argument points to.
    fn get<T: Mirror>(&self, i: usize) -> T {
        let m = Mem {
            bytes: &self.cap.call.mem1,
            base: MEM1,
        };
        T::import(&m, self.int(i), &mut ArgCx(self.maps))
    }

    /// The index of a hitbox a `HitCapsule*` argument points to in the item at `item`.
    fn hitbox(&self, i: usize, item: u32) -> usize {
        ((self.int(i) - item - 0x5D4) / 0x13C) as usize
    }
}

/// What a test runs on our side, given the imported state and the captured call.
type Ours = dyn Fn(&mut Running, &Args) -> Ret;

/// Runs every capture of `func` on both sides and requires the states to agree.
fn check(func: &str, ours: &Ours) {
    check_calls(func, func, &|_| true, ours);
}

/// What a test changes in a captured call before both sides run it (the function called, its
/// arguments, memory); false skips the capture.
type Prep<'a> = dyn Fn(&mut ssbm_rt::capture::Call) -> bool + 'a;

/// Runs `target` on both sides from every capture of `source`, prepared by `prep`: a function
/// no replay calls, or a branch none reaches, run from real states.
fn check_on(source: &str, target: &str, prep: &Prep<'_>, ours: &Ours) {
    let addr = leaf_check::sym(target);
    check_calls(
        source,
        target,
        &move |call| {
            call.addr = addr;
            prep(call)
        },
        ours,
    );
}

/// [`check`] and [`check_on`]: every capture of `source`, prepared, run on both sides.
fn check_calls(source: &str, func: &str, prep: &Prep<'_>, ours: &Ours) {
    let files = captures(source);
    if files.is_empty() {
        eprintln!(
            "no captures of {source} in {}: skipping (see this file's docs to make them)",
            calls_dir().display()
        );
        return;
    }
    let _turn = Turn::take();
    let Some(env) = env() else { return };
    let mut blocked = Vec::new();
    let mut skipped = 0;
    'captures: for source in &files {
        let mut cap = source.read();
        if !prep(&mut cap.call) {
            skipped += 1;
            continue;
        }
        let name = source.name();
        // The state, imported once (in the first mode, as each mode's run did) for both modes'
        // runs, each from its own copy.
        set_fp_mode(MODES[0]);
        let mut archives = Archives::default();
        let data = DataMap {
            loaded: find_archives_in(env.index, &cap.call.mem1, &mut archives),
        };
        let bank = melee_capture::audio::find_bank(&cap.call.mem1, &mut archives);
        let mut imp = Importer::new(&cap, &data);
        imp.world();
        imp.stage();
        imp.ids();
        imp.items();
        imp.audio(bank);
        let Importer {
            game: imported,
            maps: imported_maps,
            ..
        } = imp;
        for mode in MODES {
            set_fp_mode(mode);
            let mut game = imported.clone();
            let maps = imported_maps.clone();
            let args = Args {
                cap: &cap,
                maps: &maps,
                data: &data,
            };
            let ran = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ours(
                    &mut Running {
                        game: &mut game,
                        data: &archives,
                    },
                    &args,
                )
            }));
            let ret = match ran {
                Ok(r) => r,
                Err(p) => match unported(&*p) {
                    Some(why) => {
                        blocked.push(format!("{name}: {why}"));
                        continue 'captures;
                    }
                    None => std::panic::resume_unwind(p),
                },
            };
            cap.call.load(&env.ctx);
            env.ctx.invoke(cap.call.addr);
            let mut after = vec![0; cap.call.mem1.len()];
            let _ = env.ctx.mem.read_bytes(MEM1, &mut after);
            let r3 = env.ctx.regs.r(3);
            let f1 = env.ctx.regs.f(1);
            let mut cmp = Comparer::new(&after, &data, &game, maps);
            cmp.world();
            cmp.stage();
            cmp.ids();
            cmp.items();
            cmp.audio();
            match ret {
                Ret::None => {}
                Ret::Bool(b) => cmp.diffs_check("r3", u32::from(b), r3),
                Ret::Int(v) => cmp.diffs_check("r3", v, r3),
                Ret::GObj(g) => {
                    let paired = (r3 != 0).then(|| cmp.maps.gobjs.get(&r3).copied());
                    if paired != g.map(Some) {
                        cmp.diffs
                            .push(format!("r3: ours {g:?}, theirs {r3:#x} ({paired:?})"));
                    }
                }
                Ret::JObj(j) => {
                    use melee_sim::game::mirror::Cx;
                    cmp.compare_handle("r3", "JObjId", j.map(|j| j.0), r3);
                }
                Ret::Float(v) => {
                    if v.to_bits() != f1.to_bits() {
                        cmp.diffs.push(format!("f1: ours {v}, theirs {f1}"));
                    }
                }
                Ret::BoolWords(..) | Ret::IntWords(..) => {
                    let (v, at, words) = match ret {
                        Ret::BoolWords(b, at, w) => (u32::from(b), at, w),
                        Ret::IntWords(v, at, w) => (v, at, w),
                        _ => unreachable!(),
                    };
                    cmp.diffs_check("r3", v, r3);
                    for (k, w) in words.iter().enumerate() {
                        let at = at + 4 * k as u32;
                        let i = (at - MEM1) as usize;
                        let theirs = u32::from_be_bytes(after[i..i + 4].try_into().unwrap());
                        cmp.diffs_check(&format!("{at:#x}"), *w, theirs);
                    }
                }
                Ret::Words(at, words) => {
                    for (k, w) in words.iter().enumerate() {
                        let at = at + 4 * k as u32;
                        let i = (at - MEM1) as usize;
                        let theirs = u32::from_be_bytes(after[i..i + 4].try_into().unwrap());
                        cmp.diffs_check(&format!("{at:#x}"), *w, theirs);
                    }
                }
            }
            if !cmp.diffs.is_empty() {
                for d in cmp.diffs.iter().take(30) {
                    eprintln!("  {d}");
                }
                panic!("{name} ({mode:?}): {} differences", cmp.diffs.len());
            }
            assert_eq!(game.objs.stale_reads, 0, "{name}: stale reads");
        }
    }
    set_fp_mode(FpMode::Hardware);
    let ran = files.len() - skipped;
    eprintln!(
        "{func}: {} captures agree, {} reach unported code",
        ran - blocked.len(),
        blocked.len()
    );
    for b in &blocked {
        eprintln!("  {b}");
    }
    assert!(
        blocked.len() < ran,
        "{func}: every capture reaches unported code"
    );
}

trait DiffsCheck {
    fn diffs_check(&mut self, what: &str, ours: u32, theirs: u32);
}

impl DiffsCheck for Comparer<'_> {
    fn diffs_check(&mut self, what: &str, ours: u32, theirs: u32) {
        if ours != theirs {
            self.diffs
                .push(format!("{what}: ours {ours:#x}, theirs {theirs:#x}"));
        }
    }
}

/// Why our side stopped, if it reached code no port covers yet: a boundary, not a difference.
fn unported(p: &(dyn std::any::Any + Send)) -> Option<String> {
    let msg = p
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| p.downcast_ref::<&str>().map(|s| (*s).to_owned()))?;
    (msg.starts_with("not implemented")
        || msg.contains("isn't ported")
        || msg.contains("need the effect port"))
    .then_some(msg)
}

/// Imports a struct a call's pointer argument points to, with its GObj pointers mapped.
struct ArgCx<'a>(&'a Maps);

impl Cx for ArgCx<'_> {
    fn import_handle(&mut self, kind: &'static str, addr: u32) -> Option<u32> {
        if addr == 0 {
            return None;
        }
        assert_eq!(kind, "GObjId", "an argument holds a {kind}");
        Some(self.0.gobjs[&addr].0)
    }
    fn import_data(&mut self, addr: u32) -> Option<DataRef> {
        panic!("an argument points into game data at {addr:#x}")
    }
    fn compare_handle(&mut self, _: &str, _: &'static str, _: Option<u32>, _: u32) {
        unreachable!()
    }
    fn compare_data(&mut self, _: &str, _: Option<DataRef>, _: u32) {
        unreachable!()
    }
    fn mismatch(&mut self, _: &str, _: String, _: String) {
        unreachable!()
    }
}

/// A function of a GObj: our port of the captured function, called with its GObj.
fn check_proc(func: &str, run: fn(&mut Running, GObjId)) {
    check(func, &move |r, a| {
        run(r, a.gobj(0));
        Ret::None
    });
}

/// A function of a GObj on the game state alone.
fn check_game(func: &str, run: fn(&mut Game, GObjId)) {
    check(func, &move |r, a| {
        run(r.game, a.gobj(0));
        Ret::None
    });
}

/// A predicate on a GObj, returning whether the item is to be destroyed (or similar).
fn check_pred(func: &str, run: fn(&mut Running, GObjId) -> bool) {
    check(func, &move |r, a| Ret::Bool(run(r, a.gobj(0))));
}

/// A function of an `Item*`.
fn check_item(func: &str, run: fn(&mut Item)) {
    check(func, &move |r, a| {
        run(&mut r.game.items[a.item(0).0]);
        Ret::None
    });
}

/// The bits of a vector, as words.
fn vec_words(v: Vec3) -> Vec<u32> {
    vec![v.x.to_bits(), v.y.to_bits(), v.z.to_bits()]
}

use melee_sim::game::it::kinds::{self, capsule};
use melee_sim::game::it::kinds::{foxblaster, foxlaser};
use melee_sim::game::it::owner as it_owner;
use melee_sim::game::it::spawn;
use melee_sim::game::it::{anim, coll, ground, hitbox, item, lib26b1, lib279c, lib2725, maplib};

// item.c

#[test]
fn hitlag_proc() {
    check_proc("Item_802693E4", item::proc_hitlag);
}

#[test]
fn anim_proc() {
    check_proc("Item_80269528", item::proc_anim);
}

#[test]
fn phys_proc() {
    check_proc("Item_802697D4", item::proc_phys);
}

#[test]
fn coll_proc() {
    check_proc("Item_80269978", item::proc_coll);
}

#[test]
fn accessory_proc() {
    check_proc("Item_80269A9C", item::proc_accessory);
}

#[test]
fn jumped_on_proc() {
    check_proc("Item_80269B60", item::proc_jumped_on);
}

#[test]
fn grab_proc() {
    check_proc("Item_80269BE4", item::proc_grab);
}

#[test]
fn hit_proc() {
    check_proc("Item_80269C5C", item::proc_hit);
}

#[test]
fn react_proc() {
    check_proc("Item_8026A294", item::proc_react);
}

#[test]
fn dynamics_proc() {
    check_proc("Item_8026A788", item::proc_dynamics);
}

#[test]
fn spawn_air() {
    check("Item_80268B18", &|r, a| {
        let mut spawn: SpawnItem = a.get(0);
        Ret::GObj(item::spawn_air(r, &mut spawn))
    });
}

#[test]
fn create() {
    check("Item_8026862C", &|r, a| {
        let spawn: SpawnItem = a.get(0);
        Ret::GObj(item::create(r, &spawn))
    });
}

#[test]
fn hold_kind() {
    check("Item_802674AC", &|_, a| {
        let spawn: SpawnItem = a.get(0);
        Ret::Words(a.int(0) + 0xC, vec![item::hold_kind_of(spawn.kind) as u32])
    });
}

#[test]
fn init_position() {
    check("Item_80267130", &|r, a| {
        let spawn: SpawnItem = a.get(1);
        item::init_position(r, a.gobj(0), &spawn);
        Ret::None
    });
}

#[test]
fn init_vars() {
    check("Item_80267AA8", &|r, a| {
        let spawn: SpawnItem = a.get(1);
        item::init_vars(r, a.gobj(0), &spawn);
        Ret::None
    });
}

#[test]
fn uncount() {
    check_game("Item_802675A8", item::uncount);
}

#[test]
fn count() {
    check_game("Item_802676F4", item::count);
}

#[test]
fn at_limit() {
    check("Item_8026784C", &|r, a| {
        Ret::Bool(item::at_limit(r.game, a.int(0) as i32))
    });
}

#[test]
fn set_kind_data() {
    check("Item_80267978", &|r, a| {
        item::set_kind_data(r.game, r.data, a.gobj(0));
        Ret::None
    });
}

#[test]
fn set_material_class() {
    check_game("Item_8026814C", item::set_material_class);
}

#[test]
fn make_bone_table() {
    check("Item_802682F0", &|r, a| {
        item::make_bone_table(r, a.gobj(0));
        Ret::Bool(true)
    });
}

#[test]
fn apply_scale() {
    check_game("Item_8026849C", item::apply_scale);
}

#[test]
fn init_dynamics() {
    check_proc("Item_80268560", item::init_dynamics);
}

#[test]
fn add_anims() {
    check("Item_80268BE0", &|r, a| {
        item::add_anims(r, a.jobj(0), a.data(1), a.data(2), a.data(3), a.item(4));
        Ret::None
    });
}

#[test]
fn animate() {
    check_proc("Item_802694CC", item::animate);
}

#[test]
fn check_blast_zones() {
    check_pred("Item_802696CC", item::check_blast_zones);
}

#[test]
fn clear_hit_record() {
    check("Item_80269CC4", &|r, a| {
        item::clear_hit_record(melee_sim::game::it::item_mut(r.game, a.gobj(0)));
        Ret::None
    });
}

#[test]
fn on_spawned() {
    check_proc("Item_8026A810", item::on_spawned);
}

#[test]
fn play_sfx() {
    check("Item_8026AE84", &|r, a| {
        let g = a.item_gobj(0);
        item::play_sfx(r, g, a.int(1) as i32, a.int(2) as u8, a.int(3) as u8);
        Ret::None
    });
}

#[test]
fn stop_sfx2() {
    check("Item_8026B074", &|r, a| {
        let g = a.item_gobj(0);
        item::stop_sfx2(r, g);
        Ret::None
    });
}

#[test]
fn change_state() {
    check("Item_80268E5C", &|r, a| {
        item::change_state(r, a.gobj(0), a.int(1) as i32, a.int(2));
        Ret::None
    });
}

// it_26B1.c

#[test]
fn attack_id() {
    check("itGetAttackId", &|r, a| {
        Ret::Int(lib26b1::attack_id(r.game, a.gobj(0)) as u32)
    });
}

#[test]
fn owner() {
    check("itGetOwner", &|r, a| {
        Ret::GObj(lib26b1::owner(r.game, a.gobj(0)))
    });
}

#[test]
fn enable_pickup() {
    check_game("it_8026B390", lib26b1::enable_pickup);
}

#[test]
fn disable_pickup() {
    check_game("it_8026B3A8", lib26b1::disable_pickup);
}

#[test]
fn ecb_center() {
    check("it_8026BB88", &|r, a| {
        Ret::Words(a.int(1), vec_words(lib26b1::ecb_center(r.game, a.gobj(0))))
    });
}

#[test]
fn last_ecb_center() {
    check("it_8026BBCC", &|r, a| {
        Ret::Words(
            a.int(1),
            vec_words(lib26b1::last_ecb_center(r.game, a.gobj(0))),
        )
    });
}

#[test]
fn is_stomp() {
    check("it_8026BC68", &|r, a| {
        Ret::Bool(lib26b1::is_stomp(r.game, a.gobj(0)))
    });
}

#[test]
fn set_xdcd_b3() {
    check_game("it_8026BD6C", lib26b1::set_xdcd_b3);
}

#[test]
fn check_burst() {
    check_proc("it_8026C368", lib26b1::check_burst);
}

/// The debug VS sets (`debug*`, see the file's docs), whose items' blocks and the heap their
/// pool grows into hold an earlier scene's bytes (the opening movie's frame, docs/oracle.md):
/// every saved call imports and compares back with its memory, and some hold words unknown
/// until written. But for the material setups (`itmaterial.c`), which run inside the display
/// pass, whose current objects the import doesn't take mid-draw, and the item code's setup
/// (`Item_80266F70`, with its archive's `it_8027870C`), which runs before the stage exists.
#[test]
fn debug_round_trips() {
    leaf_check::calls::debug_round_trips(&calls_dir(), &|f| {
        matches!(
            f,
            "Item_80266F70" | "it_8027870C" | "fn_80277D8C" | "it_80277F90" | "it_80278108"
        )
    });
}

// itgroundcoll.c

#[test]
fn stage_crossing() {
    // The calls that ask for no normal (the shots'): the port gives the point alone.
    let no_normal = |c: &mut Call| c.regs.gpr[6] == 0;
    check_calls("it_8026E9A4", "it_8026E9A4", &no_normal, &|r, a| {
        match ground::stage_crossing(r.game, a.vec(1), a.vec(2)) {
            Some(p) => {
                let at = a.int(2);
                // The point lands on `to`, which for the shots is the item's position.
                let gobj = a.gobj(0);
                let ip = melee_sim::game::it::item_mut(r.game, gobj);
                if at == a.cap.u32(a.int(0) + 0x2C) + 0x4C {
                    ip.pos = p;
                    Ret::Bool(true)
                } else {
                    Ret::Words(at, vec_words(p))
                }
            }
            None => Ret::Bool(false),
        }
    });
}

// itcoll.c

#[test]
fn clear_damage_log() {
    check("it_8026F9A0", &|r, _| {
        coll::clear_damage_log(r.game);
        Ret::None
    });
}

#[test]
fn share_group_victims() {
    check("it_8026FCF8", &|r, a| {
        let g = a.item_gobj(0);
        coll::share_group_victims(r.game, g, a.hitbox(1, a.int(0)));
        Ret::None
    });
}

#[test]
fn hit_by_items() {
    check_proc("it_802706D0", coll::hit_by_items);
}

#[test]
fn take_hits() {
    check_proc("it_80270E30", coll::take_hits);
}

#[test]
fn update_hitbox() {
    check("it_8027129C", &|r, a| {
        coll::update_hitbox(r.game, r.data, a.gobj(0), a.int(1) as usize);
        Ret::None
    });
}

#[test]
fn update_hitboxes() {
    check("it_8027137C", &|r, a| {
        coll::update_hitboxes(r.game, r.data, a.gobj(0));
        Ret::None
    });
}

#[test]
fn age_hit_victims() {
    check_game("it_8027146C", coll::age_hit_victims);
}

#[test]
fn refresh_hurtboxes() {
    check_game("it_802714C0", coll::refresh_hurtboxes);
}

#[test]
fn set_hurtbox_state() {
    check("it_80271508", &|r, a| {
        coll::set_hurtbox_state(r.game, a.gobj(0), a.int(1) as i32);
        Ret::None
    });
}

#[test]
fn init_hurtboxes() {
    check_proc("it_8027163C", coll::init_hurtboxes);
}

#[test]
fn rotate_ecb() {
    check("it_80271830", &|r, a| {
        let ip = &mut r.game.items[a.item(0).0];
        coll::rotate_ecb(&mut ip.xBEC, a.float(0));
        Ret::None
    });
}

#[test]
fn update_ecb() {
    check("it_80271A58", &|r, a| {
        coll::update_ecb(r.game, r.data, a.gobj(0));
        Ret::None
    });
}

#[test]
fn keep_clear() {
    check("it_802721B8", &|r, a| {
        coll::keep_clear(r.game, r.data, a.gobj(0));
        Ret::None
    });
}

#[test]
fn clear_x1b() {
    check_game("it_80272280", coll::clear_x1b);
}

#[test]
fn set_x1b() {
    check_game("it_80272298", coll::set_x1b);
}

#[test]
fn track_fighters() {
    check_game("it_802722B0", coll::track_fighters);
}

#[test]
fn update_dynamics_points() {
    check("it_80272304", &|r, a| {
        coll::update_dynamics_points(r.game, r.data, a.gobj(0));
        Ret::None
    });
}

#[test]
fn set_hit_damage() {
    check("it_80272460", &|r, a| {
        let g = a.gobj(2);
        let item = a.cap.u32(a.int(2) + 0x2C);
        coll::set_hit_damage(r.game, r.data, g, a.hitbox(0, item), a.int(1));
        Ret::None
    });
    // A fighter owner of another height scales the damage first (`ftCo_CalcYScaledKnockback`).
    for scale in [0.75f32, 1.25] {
        let prep = move |c: &mut Call| {
            let item = w32(c, c.regs.gpr[5] + 0x2C);
            let owner = w32(c, item + 0x518);
            if owner == 0 {
                return false;
            }
            set32(c, w32(c, owner + 0x2C) + 0x38, scale.to_bits());
            true
        };
        check_calls("it_80272460", "it_80272460", &prep, &|r, a| {
            let g = a.gobj(2);
            let item = a.cap.u32(a.int(2) + 0x2C);
            coll::set_hit_damage(r.game, r.data, g, a.hitbox(0, item), a.int(1));
            Ret::None
        });
    }
}

// it_2725.c

#[test]
fn clear_hitboxes() {
    check("it_802725D4", &|r, a| {
        lib2725::clear_hitboxes(r.game, r.data, a.gobj(0));
        Ret::None
    });
}

#[test]
fn show() {
    check_game("it_80272940", lib2725::show);
}

#[test]
fn bone() {
    check("it_80272CC0", &|r, a| {
        Ret::JObj(lib2725::bone(r.game, r.data, a.gobj(0), a.int(1) as i32))
    });
}

#[test]
fn set_uniform_scale() {
    check("it_80272F7C", &|r, a| {
        lib2725::set_uniform_scale(r.game, a.jobj(0).expect("a joint"), a.float(0));
        Ret::None
    });
}

#[test]
fn tick_lifetime() {
    check("it_80273130", &|r, a| {
        Ret::Bool(lib2725::tick_lifetime(r.game, a.gobj(0)))
    });
}

#[test]
fn pickup_sfx() {
    check_proc("it_80273168", lib2725::pickup_sfx);
}

#[test]
fn destroy_effect() {
    // Without an effect (they wait for the effect port) or a sound (the audio driver's).
    let prep = |c: &mut Call| {
        c.regs.gpr[4] = u32::MAX;
        c.regs.gpr[5] = SFX_SILENT;
        true
    };
    check_calls("it_8027327C", "it_8027327C", &prep, &|r, a| {
        lib2725::destroy_effect(r, a.gobj(0), a.int(1) as i32, a.int(2) as i32);
        Ret::None
    });
}

#[test]
fn play_sfx_unless_quiet() {
    check("it_802732E4", &|r, a| {
        let g = a.item_gobj(0);
        lib2725::play_sfx(r, g, a.int(1) as i32);
        Ret::None
    });
}

#[test]
fn clear_x64() {
    check_game("it_8027346C", lib2725::clear_x64);
}

#[test]
fn clear_x58() {
    check_game("it_80273484", lib2725::clear_x58);
}

#[test]
fn set_velocity() {
    check("it_80273500", &|r, a| {
        lib2725::set_velocity(r.game, a.gobj(0), a.vec(1));
        Ret::None
    });
}

#[test]
fn clamp_fall() {
    check("it_80274658", &|r, a| {
        Ret::Int(lib2725::clamp_fall(r.game, a.gobj(0), a.float(0)) as u32)
    });
}

#[test]
fn spin_joint() {
    check("it_802746F8", &|r, a| {
        Ret::JObj(lib2725::spin_joint(r.game, r.data, a.gobj(0)))
    });
}

#[test]
fn is_hit_counted() {
    check("it_80274C78", &|r, a| {
        Ret::Bool(lib2725::is_hit_counted(r.game, a.gobj(0)))
    });
}

#[test]
fn clear_stomp() {
    check_game("it_80274C88", lib2725::clear_stomp);
}

#[test]
fn ecb_motion() {
    check("it_80274D04", &|r, a| {
        let item = a.cap.u32(a.int(0) + 0x2C);
        assert_eq!(
            a.int(1),
            item + 0xB54,
            "the ECB motion goes to the item's xB54"
        );
        lib2725::ecb_motion(r.game, a.gobj(0));
        Ret::None
    });
}

#[test]
fn ecb_extent() {
    check_game("it_80274D6C", lib2725::ecb_extent);
}

#[test]
fn init_spin() {
    check_game("it_80274DAC", lib2725::init_spin);
}

#[test]
fn clear_attack_id() {
    check_game("it_80274EF8", lib2725::clear_attack_id);
}

#[test]
fn set_lifetime() {
    check("it_80275158", &|r, a| {
        lib2725::set_lifetime(r.game, r.data, a.gobj(0), a.float(0));
        Ret::None
    });
}

#[test]
fn set_damage_taken() {
    check("it_802753BC", &|r, a| {
        lib2725::set_damage_taken(r.game, a.gobj(0), a.int(1) as i16);
        Ret::None
    });
}

// ithitbox.c

#[test]
fn clear_dealt() {
    check_game("it_802753DC", hitbox::clear_dealt);
}

#[test]
fn clear_hit_ids() {
    check_game("it_80275474", hitbox::clear_hit_ids);
}

#[test]
fn clear_hit_ids2() {
    check_game("it_80275504", hitbox::clear_hit_ids2);
}

#[test]
fn scale_hitbox() {
    check("it_80275594", &|r, a| {
        let mult = leaf_check::fp::lfs(a.float(0).to_bits());
        hitbox::scale_hitbox(r.game, a.gobj(0), a.int(1) as usize, mult);
        Ret::None
    });
}

#[test]
fn start_blink() {
    check_game("it_802756D0", hitbox::start_blink);
}

#[test]
fn update_boxes() {
    check_game("it_8027574C", hitbox::update_boxes);
}

// itmaplib.c

#[test]
fn init_coll() {
    check("it_80275E98", &|r, a| {
        let spawn: SpawnItem = a.get(1);
        maplib::init_coll(r, a.gobj(0), &spawn);
        Ret::None
    });
}

#[test]
fn place_air() {
    check("it_80276100", &|r, a| {
        maplib::place_air(r, a.gobj(0), a.vec(1));
        Ret::None
    });
}

#[test]
fn enter_air() {
    check_item("it_802762BC", maplib::enter_air);
}

// itanimlist.c

#[test]
fn create_hitbox() {
    check("it_802790C0", &|r, a| {
        let item = a.cap.u32(a.int(0) + 0x2C);
        assert_eq!(a.int(1), item + 0x524, "the item's own script");
        anim::create_hitbox(r, a.gobj(0));
        Ret::None
    });
}

#[test]
fn run_script() {
    check_proc("it_802799E4", anim::run_script);
}

#[test]
fn clear_overlay() {
    check_item("it_80279B64", anim::clear_overlay);
}

#[test]
fn run_overlay() {
    check_proc("it_80279BE0", anim::run_overlay);
}

// it_279C.c

#[test]
fn track_spawn() {
    check("it_8027B0C4", &|r, a| {
        let spawn: SpawnItem = a.get(1);
        lib279c::track_spawn(r, a.gobj(0), &spawn);
        Ret::None
    });
}

// itfoxlaser.c

#[test]
fn laser_spawn() {
    check("it_8029C504", &|r, a| {
        foxlaser::spawn(
            r,
            a.gobj(0),
            a.vec(1),
            a.int(2) as i32,
            a.int(3) as i32,
            a.float(0),
            a.float(1),
        );
        Ret::None
    });
}

#[test]
fn laser_spawn_0() {
    check("it_8029C6A4", &|r, a| {
        foxlaser::spawn_0(
            r,
            a.float(0),
            a.float(1),
            a.gobj(0),
            a.vec(1),
            a.int(2) as i32,
        );
        Ret::None
    });
}

#[test]
fn laser_spawn_1() {
    check("it_8029C6CC", &|r, a| {
        foxlaser::spawn_1(
            r,
            a.float(0),
            a.float(1),
            a.gobj(0),
            a.vec(1),
            a.int(2) as i32,
        );
        Ret::None
    });
}

#[test]
fn laser_anim() {
    check_pred("itFoxlaser_UnkMotion1_Anim", foxlaser::animate);
}

#[test]
fn laser_phys() {
    check_proc("itFoxlaser_UnkMotion1_Phys", foxlaser::physics);
}

#[test]
fn laser_coll() {
    check_pred("itFoxlaser_UnkMotion1_Coll", foxlaser::collide);
}

#[test]
fn laser_reflected() {
    check_pred("itFoxLaser_Logic94_Reflected", foxlaser::reflected);
}

#[test]
fn laser_shield_bounced() {
    check_pred("itFoxLaser_Logic94_ShieldBounced", foxlaser::shield_bounced);
}

// itfoxblaster.c

#[test]
fn blaster_follow() {
    check_pred("itFoxblaster_UnkMotion8_Anim", foxblaster::follow_fighter);
}

#[test]
fn blaster_phys() {
    check_proc("itFoxblaster_UnkMotion8_Phys", |r, g| {
        melee_sim::game::code::run_proc(
            r,
            g,
            melee_sim::game::handles::CodeRef(foxblaster::MOTION8_PHYS),
        )
    });
}

#[test]
fn blaster_recoil() {
    check_game("it_802AE200", foxblaster::recoil);
}

#[test]
fn blaster_pulse() {
    check_game("it_802AE63C", foxblaster::pulse);
}

#[test]
fn blaster_reset() {
    check_game("it_802AE7B8", foxblaster::reset);
}

#[test]
fn blaster_set_visible() {
    check("it_802ADDD0", &|r, a| {
        if a.int(0) != 0 {
            foxblaster::set_visible(r, a.gobj(0), a.int(1) as i32);
        }
        Ret::None
    });
}

#[test]
fn blaster_shot() {
    check_game("it_802AE1D0", foxblaster::shot);
}

#[test]
fn blaster_on_shot() {
    check_proc("it_802ADEF0", foxblaster::on_shot);
}

#[test]
fn blaster_on_shot_inner() {
    check_proc("it_802ADF10", foxblaster::on_shot);
}

#[test]
fn blaster_start_pulse() {
    check_proc("it_802AE538", foxblaster::start_pulse);
}

#[test]
fn blaster_end_pulse() {
    check_game("it_802AE608", foxblaster::end_pulse);
}

#[test]
fn blaster_picked_up() {
    check_proc("itFoxBlaster_Logic96_PickedUp", foxblaster::picked_up);
}

// Functions and branches no replay reached, run from real states: every item's hitlag
// process capture (each item once a tick) with the function called and its arguments set.

/// Every item tick's capture.
const TICKS: &str = "Item_802693E4";

type Call = ssbm_rt::capture::Call;

/// A word of a captured call's memory.
fn w32(c: &Call, a: u32) -> u32 {
    let i = (a - MEM1) as usize;
    u32::from_be_bytes(c.mem1[i..i + 4].try_into().unwrap())
}

/// Changes a word of a captured call's memory.
fn set32(c: &mut Call, a: u32, v: u32) {
    let i = (a - MEM1) as usize;
    c.mem1[i..i + 4].copy_from_slice(&v.to_be_bytes());
}

/// Sets bits of a byte of a captured call's memory.
fn or8(c: &mut Call, a: u32, bits: u8) {
    c.mem1[(a - MEM1) as usize] |= bits;
}

/// The item of the GObj in r3.
fn item_ptr(c: &Call) -> u32 {
    w32(c, c.regs.gpr[3] + 0x2C)
}

/// The kind of the item of the GObj in r3.
fn item_kind(c: &Call) -> i32 {
    w32(c, item_ptr(c) + 0x10) as i32
}

/// Passes the item instead of its GObj.
fn pass_item(c: &mut Call) -> bool {
    c.regs.gpr[3] = item_ptr(c);
    true
}

/// Sets float argument `i` as the original passes a single.
fn set_f(c: &mut Call, i: usize, v: f32) {
    c.regs.fpr[i].ps0 = f64::from(v);
    c.regs.fpr[i].ps1 = f64::from(v);
}

/// Sets bits of the item's `xDC8_word`, numbered from the top.
fn set_dc8(c: &mut Call, bits: &[u32]) {
    let at = item_ptr(c) + 0xDC8;
    let w = bits.iter().fold(w32(c, at), |w, b| w | 0x8000_0000 >> b);
    set32(c, at, w);
}

/// The shots' captures only (their shield and reflection callbacks are ported).
fn lasers(c: &mut Call) -> bool {
    matches!(item_kind(c), 54 | 55)
}

/// Passes the item's owner as the second argument (the GObj an item forgets).
fn pass_owner(c: &mut Call) -> bool {
    c.regs.gpr[4] = w32(c, item_ptr(c) + 0x518);
    true
}

#[test]
fn seek_anim() {
    let prep = |c: &mut Call| {
        set_f(c, 1, 3.0);
        true
    };
    check_on(TICKS, "Item_80268DD4", &prep, &|r, a| {
        item::seek_anim(r, a.gobj(0), a.float(0));
        Ret::None
    });
}

#[test]
fn next_attack_id() {
    check_on(TICKS, "Item_8026AE60", &|_| true, &|r, _| {
        Ret::Int(item::next_attack_id(r.game))
    });
}

/// The item in r3 and sound `sfx` in r4, for the item sound functions.
fn sfx_args(sfx: u32) -> impl Fn(&mut Call) -> bool {
    move |c| {
        pass_item(c);
        c.regs.gpr[4] = sfx;
        c.regs.gpr[5] = 0x7F;
        c.regs.gpr[6] = 0x40;
        true
    }
}

/// The silent sound and the slot-stopping one, the item sound functions' paths that play
/// nothing.
const SFX_SILENT: u32 = 0x83D60;
const SFX_STOP: u32 = 0x83D61;

#[test]
fn play_sfx_silent() {
    check_on(TICKS, "Item_8026AE84", &sfx_args(SFX_SILENT), &|r, a| {
        item::play_sfx(r, a.item_gobj(0), a.int(1) as i32, 0x7F, 0x40);
        Ret::None
    });
}

/// A sound of the shared bank, through the audio driver.
#[test]
fn play_sfx_sound() {
    check_on(TICKS, "Item_8026AE84", &sfx_args(0x96), &|r, a| {
        item::play_sfx(
            r,
            a.item_gobj(0),
            a.int(1) as i32,
            a.int(2) as u8,
            a.int(3) as u8,
        );
        Ret::None
    });
}

#[test]
fn play_sfx1() {
    // The silent and stop ids, and a sound of the shared bank.
    for sfx in [SFX_SILENT, SFX_STOP, 0x96] {
        check_on(TICKS, "Item_8026AF0C", &sfx_args(sfx), &|r, a| {
            item::play_sfx1(
                r,
                a.item_gobj(0),
                a.int(1) as i32,
                a.int(2) as u8,
                a.int(3) as u8,
            );
            Ret::None
        });
    }
}

#[test]
fn play_sfx2() {
    for sfx in [SFX_SILENT, SFX_STOP, 0x96] {
        check_on(TICKS, "Item_8026AFA0", &sfx_args(sfx), &|r, a| {
            item::play_sfx2(
                r,
                a.item_gobj(0),
                a.int(1) as i32,
                a.int(2) as u8,
                a.int(3) as u8,
            );
            Ret::None
        });
    }
}

#[test]
fn stop_sfx1() {
    check_on(TICKS, "Item_8026B034", &pass_item, &|r, a| {
        item::stop_sfx1(r, a.item_gobj(0));
        Ret::None
    });
}

#[test]
fn is_grabbable() {
    check_on(TICKS, "Item_IsGrabbable", &|_| true, &|r, a| {
        Ret::Bool(item::is_grabbable(r.game, a.gobj(0)))
    });
}

#[test]
fn add_damage() {
    // Up to the cap and past it.
    for (have, add) in [(5, 7), (500, 600)] {
        let prep = move |c: &mut Call| {
            let ip = item_ptr(c);
            set32(c, ip + 0xC9C, have);
            c.regs.gpr[3] = ip;
            c.regs.gpr[4] = add;
            true
        };
        check_on(TICKS, "Item_80269CA0", &prep, &|r, a| {
            item::add_damage(&mut r.game.items[a.item(0).0], a.int(1) as i32);
            Ret::None
        });
    }
}

#[test]
fn enter_hitlag() {
    check_on(TICKS, "Item_8026A158", &|_| true, &|r, a| {
        item::enter_hitlag(r, a.gobj(0));
        Ret::None
    });
}

#[test]
fn exit_hitlag() {
    check_on(TICKS, "Item_8026A1E8", &|_| true, &|r, a| {
        item::exit_hitlag(r, a.gobj(0));
        Ret::None
    });
}

#[test]
fn end_hitlags() {
    // The hit's or the clank's hitlag alone, then with the other holding it.
    for bits in [&[7][..], &[7, 8], &[8], &[8, 10]] {
        let prep = move |c: &mut Call| {
            set_dc8(c, bits);
            true
        };
        check_on(TICKS, "Item_8026A0A0", &prep, &|r, a| {
            item::end_hit_hitlag(r, a.gobj(0));
            Ret::None
        });
        check_on(TICKS, "Item_8026A0FC", &prep, &|r, a| {
            item::end_clank_hitlag(r, a.gobj(0));
            Ret::None
        });
    }
}

#[test]
fn react_shield() {
    // Bouncing off a shield in the air. Bit 4 bounces at any angle; without it, only below
    // the limit, after turning, and above it the shot hits the shield and is destroyed.
    for (bits, angle) in [(0x0C, 2.0f32), (0x04, 0.5), (0x08, 2.0)] {
        let bounce = move |c: &mut Call| {
            let ip = item_ptr(c);
            set32(c, ip + 0xC0, 1);
            set32(c, ip + 0xC54, angle.to_bits());
            or8(c, ip + 0xDCE, bits);
            lasers(c)
        };
        check_on(TICKS, "Item_80269DC8", &bounce, &|r, a| {
            Ret::Bool(item::react_shield(r, a.gobj(0)))
        });
    }
}

/// An item destroyed (`Item_8026A8EC`), its sounds stopped (`Item_8026B0B4`) among the rest,
/// from every saved call of it: as saved, then with a playing sound's handle (the driver's
/// first active voice) in each of its three sound slots in turn, which none of the saves has.
#[test]
fn destroy() {
    for slot in [None, Some(0xD64), Some(0xD68), Some(0xD6C)] {
        let prep = move |c: &mut Call| {
            let Some(at) = slot else { return true };
            let voice = w32(c, leaf_check::sym("AXDriver_804D7794"));
            if voice == 0 {
                return false;
            }
            let handle = w32(c, voice + 0xC);
            set32(c, item_ptr(c) + at, handle);
            true
        };
        check_calls("Item_8026A8EC", "Item_8026A8EC", &prep, &|r, a| {
            item::destroy(r, a.gobj(0));
            Ret::None
        });
    }
}

#[test]
fn react_reflect() {
    // Reflected by its own fighter.
    let prep = |c: &mut Call| {
        let ip = item_ptr(c);
        let owner = w32(c, ip + 0x518);
        set32(c, ip + 0xC64, owner);
        lasers(c) && owner != 0
    };
    check_on(TICKS, "Item_80269F14", &prep, &|r, a| {
        Ret::Bool(item::react_reflect(r, a.gobj(0)))
    });
}

#[test]
fn spawn_ground() {
    check_on("Item_80268B18", "Item_80268B5C", &|_| true, &|r, a| {
        let mut spawn: SpawnItem = a.get(0);
        Ret::GObj(item::spawn_ground(r, &mut spawn))
    });
    check_on("Item_80268B18", "Item_80268B9C", &|_| true, &|r, a| {
        let mut spawn: SpawnItem = a.get(0);
        item::spawn_ground_marked(r, &mut spawn);
        Ret::None
    });
}

#[test]
fn clear_hitbox() {
    for idx in [0, 1] {
        let prep = move |c: &mut Call| {
            c.regs.gpr[4] = idx;
            true
        };
        check_on(TICKS, "it_80272560", &prep, &|r, a| {
            lib2725::clear_hitbox(r.game, r.data, a.gobj(0), a.int(1) as usize);
            Ret::None
        });
    }
}

#[test]
fn flicker() {
    check_on(TICKS, "it_802728C8", &|_| true, &|r, a| {
        lib2725::flicker(r.game, r.data, a.gobj(0));
        Ret::None
    });
}

#[test]
fn spin() {
    let prep = |c: &mut Call| {
        let ip = item_ptr(c);
        set32(c, ip + 0xD3C, 0.25f32.to_bits());
        true
    };
    check_on(TICKS, "it_80274A64", &prep, &|r, a| {
        lib2725::spin(r.game, r.data, a.gobj(0));
        Ret::None
    });
}

#[test]
fn uncount_hits() {
    check_on(TICKS, "it_80274C60", &|_| true, &|r, a| {
        lib2725::uncount_hits(r.game, a.gobj(0));
        Ret::None
    });
}

#[test]
fn drop_owner() {
    check_on(TICKS, "it_80275390", &|_| true, &|r, a| {
        lib2725::drop_owner(r.game, a.gobj(0));
        Ret::None
    });
}

#[test]
fn end_blink() {
    check_on(TICKS, "it_802756E0", &|_| true, &|r, a| {
        hitbox::end_blink(r.game, a.gobj(0));
        Ret::None
    });
}

#[test]
fn enter_ground() {
    check_on(TICKS, "it_802762B0", &pass_item, &|r, a| {
        maplib::enter_ground(&mut r.game.items[a.item(0).0]);
        Ret::None
    });
}

#[test]
fn clear_overlay_copy() {
    // `it_80279BBC` is a second copy of `it_80279B64`.
    check_on(TICKS, "it_80279BBC", &pass_item, &|r, a| {
        anim::clear_overlay(&mut r.game.items[a.item(0).0]);
        Ret::None
    });
}

#[test]
fn hitlag_frames() {
    for d in [0, 1, 7, 13, 42, 999] {
        let prep = move |c: &mut Call| {
            c.regs.gpr[3] = d;
            true
        };
        check_on(TICKS, "it_8026B424", &prep, &move |r, _| {
            Ret::Float(lib26b1::hitlag_frames(r.game, r.data, d as i32))
        });
    }
}

#[test]
fn forget() {
    check_on(TICKS, "it_8026B894", &pass_owner, &|r, a| {
        Ret::Bool(lib26b1::forget(r.game, a.gobj(0), a.opt_gobj(1)))
    });
}

#[test]
fn on_line() {
    // The line its collision stands on.
    let prep = |c: &mut Call| {
        let ip = item_ptr(c);
        let floor = w32(c, ip + 0x4C4);
        set32(c, ip + 0xC30, floor);
        true
    };
    check_on(TICKS, "it_8026D604", &prep, &|r, a| {
        Ret::Bool(ground::on_line(r.game, a.gobj(0)))
    });
}

#[test]
fn is_heavy() {
    check_on(TICKS, "itIsHeavy", &|_| true, &|r, a| {
        Ret::Bool(lib26b1::is_heavy(r.game, r.data, a.gobj(0)))
    });
}

#[test]
fn pushes() {
    check_on(TICKS, "it_80271B60", &|_| true, &|r, a| {
        coll::push_from_fighters(r.game, r.data, a.gobj(0));
        Ret::None
    });
    check_on(TICKS, "it_80271D2C", &|_| true, &|r, a| {
        coll::push_from_items(r.game, r.data, a.gobj(0), false);
        Ret::None
    });
    check_on(TICKS, "it_80271F78", &|_| true, &|r, a| {
        coll::push_from_items(r.game, r.data, a.gobj(0), true);
        Ret::None
    });
}

#[test]
fn laser_forget() {
    let prep = |c: &mut Call| pass_owner(c) && lasers(c);
    check_on(TICKS, "itFoxLaser_Logic94_EvtUnk", &prep, &|r, a| {
        foxlaser::forget(r, a.gobj(0), a.opt_gobj(1));
        Ret::None
    });
}

/// The blasters' captures only.
fn blasters(c: &mut Call) -> bool {
    matches!(item_kind(c), 74 | 75)
}

#[test]
fn blaster_forget() {
    let prep = |c: &mut Call| pass_owner(c) && blasters(c);
    check_on(TICKS, "itFoxBlaster_Logic96_EvtUnk", &prep, &|r, a| {
        foxblaster::forget(r, a.gobj(0), a.opt_gobj(1));
        Ret::None
    });
}

#[test]
fn hold() {
    check("it_802742F4", &|r, a| {
        item::hold(r, a.gobj(0), a.gobj(1), a.int(2) as i32);
        Ret::None
    });
}

#[test]
fn blaster_spawn() {
    check("it_802AE8A8", &|r, a| {
        let g = foxblaster::spawn(
            r,
            a.float(0),
            a.opt_gobj(0),
            a.vec(1),
            a.int(2) as i32,
            a.int(3) as i32,
        );
        Ret::GObj(g)
    });
}

/// Passes the item's owner, a fighter, first, and the item second.
fn owner_first(c: &mut Call) -> bool {
    let item = c.regs.gpr[3];
    c.regs.gpr[3] = w32(c, item_ptr(c) + 0x518);
    c.regs.gpr[4] = item;
    c.regs.gpr[3] != 0
}

#[test]
fn held_items() {
    check_on(TICKS, "ftLib_GetHeldSpecialItem", &owner_first, &|r, a| {
        Ret::GObj(it_owner::held_special_item(r.game, a.gobj(0)))
    });
    check_on(TICKS, "ftLib_IsHoldingItem", &owner_first, &|r, a| {
        Ret::Bool(it_owner::is_holding(r.game, a.gobj(0), a.gobj(1)))
    });
}

#[test]
fn clear_blaster() {
    let prep = |c: &mut Call| blasters(c) && owner_first(c);
    check_on(TICKS, "ftFx_SpecialN_ClearBlaster", &prep, &|r, a| {
        foxblaster::clear_from_owner(r.game, a.gobj(0));
        Ret::None
    });
}

#[test]
fn remove_camera_box() {
    check_on(TICKS, "Item_80267454", &|_| true, &|r, a| {
        item::remove_camera_box(r.game, a.gobj(0));
        Ret::None
    });
}

#[test]
fn y_scaled_knockback() {
    use ssbm_types::fns;
    let Some(ctx) = reference() else { return };
    let xs = leaf_check::inputs(3.0, 400);
    for mode in MODES {
        set_fp_mode(mode);
        for (k, &x) in xs.iter().enumerate() {
            let scales = [0.5, 0.8, 1.0, 1.3, 2.0, xs[(k * 7) % xs.len()]];
            let ys = [0.0, 0.5, 1.0, 1.5, -0.5, -1.5, xs[(k * 13) % xs.len()]];
            for s in scales.into_iter().filter(|&s| s != 0.0) {
                for y in ys {
                    let (x, s, y) = (leaf_check::arg(x), leaf_check::arg(s), leaf_check::arg(y));
                    let theirs = fns::ftCo_CalcYScaledKnockback(&ctx, x, s, y);
                    let ours = it_owner::y_scaled(x, s, y);
                    assert_eq!(
                        ours.to_bits(),
                        theirs.to_bits(),
                        "ftCo_CalcYScaledKnockback({x}, {s}, {y}) in {mode:?}"
                    );
                }
            }
        }
    }
    set_fp_mode(FpMode::Hardware);
}

// itspawn.c, from the items-on corpus (see the file's docs).

/// The stage's item counts a pointer argument points to (`Ground_801C2AD8`'s 35 words).
fn counts_arg(a: &Args, i: usize) -> [i32; 35] {
    let p = a.int(i);
    std::array::from_fn(|k| a.cap.u32(p + 4 * k as u32) as i32)
}

/// The 64-bit mask passed in the register pair from r5.
fn mask_arg(a: &Args) -> u64 {
    u64::from(a.int(2)) << 32 | u64::from(a.int(3))
}

/// Which of the spawn tables a pointer argument names.
fn table_arg(a: &Args, i: usize) -> fn(&mut Game) -> &mut spawn::PickTable {
    let p = a.int(i);
    if p == leaf_check::sym("it_804A0E30") + 4 {
        |g| &mut g.it.spawn.spawner.table
    } else if p == leaf_check::sym("it_804A0E50") {
        |g| &mut g.it.spawn.no_containers
    } else if p == leaf_check::sym("it_804A0E60") {
        |g| &mut g.it.spawn.monsters
    } else {
        panic!("a pick table at {p:#x}")
    }
}

#[test]
fn spawn_proc() {
    check_proc("fn_8026C88C", spawn::proc_spawn);
}

#[test]
fn spawn_setup() {
    check("it_8026D018", &|r, _| {
        spawn::setup(r);
        Ret::None
    });
}

#[test]
fn spawn_tables() {
    check("it_8026CB9C", &|r, a| {
        let counts = counts_arg(a, 0);
        let mask = mask_arg(a);
        let t = &mut r.game.it.spawn.spawner.table;
        spawn::build(t, &counts, mask, 0, a.float(0));
        Ret::None
    });
    check("it_8026CD50", &|r, a| {
        let counts = counts_arg(a, 0);
        let mask = mask_arg(a);
        let t = &mut r.game.it.spawn.no_containers;
        spawn::build(t, &counts, mask, 6, a.float(0));
        Ret::None
    });
    check("it_8026CF04", &|r, _| {
        spawn::build_monsters(r.game, r.data);
        Ret::None
    });
    check("it_8026CA4C", &|r, a| {
        let counts = counts_arg(a, 1);
        let mask = u64::from(a.int(2)) << 32 | u64::from(a.int(3));
        let first = a.int(4) as i32;
        let total = spawn::total(&counts, mask, first, a.float(0));
        table_arg(a, 0)(r.game).total = total;
        Ret::None
    });
}

#[test]
fn spawn_pick() {
    check("it_8026C75C", &|r, a| {
        Ret::Int(spawn::pick_kind(r.game, table_arg(a, 0)) as u32)
    });
    check("bisectValue", &|r, a| {
        let t = table_arg(a, 1)(r.game).clone();
        let w = t.weights.as_deref().unwrap();
        Ret::Int(spawn::bisect(a.int(0) as i32, w, a.int(2) as i32, a.int(3) as i32) as u32)
    });
    check("it_8026D324", &|r, a| {
        Ret::Bool(spawn::kind_on(r.game, a.int(0) as i32))
    });
}

#[test]
fn spawn_point() {
    check("it_8026CB3C", &|r, a| {
        let at = a.int(0);
        match spawn::spawn_point(r.game, r.data) {
            Some(p) => Ret::BoolWords(true, at, vec_words(p)),
            None => Ret::Bool(false),
        }
    });
    check("it_8026B684", &|r, a| {
        let pos = a.vec(0);
        Ret::Float(f64::from(it_owner::facing_for(r.game, r.data, pos)))
    });
}

#[test]
fn spawn_queries() {
    // From the spawner's process: no item drawn and nothing spawned, only the answers.
    check_on("fn_8026C88C", "it_8026C704", &|_| true, &|r, _| {
        Ret::Bool(spawn::poke_balls_full(r.game))
    });
    check_on("fn_8026C88C", "it_8026D3CC", &|_| true, &|r, _| {
        Ret::Bool(spawn::healing_on(r.game))
    });
    // The words go below the stack pointer, where nothing lives.
    let words = |c: &mut Call| {
        c.regs.gpr[3] = c.regs.gpr[1] - 0x200;
        true
    };
    check_on("fn_8026C88C", "it_8026C47C", &words, &|r, a| {
        let w = spawn::kinds_out(r.game);
        Ret::Words(a.int(0), w.iter().map(|&x| x as u32).collect())
    });
    for kind in [0, 1, 6, 9, 54] {
        let prep = move |c: &mut Call| {
            c.regs.gpr[3] = kind;
            true
        };
        check_on("fn_8026C88C", "it_80272828", &prep, &move |r, _| {
            Ret::GObj(spawn::find_kind(r.game, kind as i32))
        });
    }
}

#[test]
fn spawn_pick_without_poke_balls() {
    // The table ending in a Poké Ball while no more may appear (as many out as allowed).
    for size in [None, Some(1u8)] {
        let prep = move |c: &mut Call| {
            let t = c.regs.gpr[3];
            if let Some(n) = size {
                let w = w32(c, t) & 0x00FF_FFFF | u32::from(n) << 24;
                set32(c, t, w);
            }
            let n = w32(c, t) >> 24;
            let last = w32(c, t + 4) + n - 1;
            let w = w32(c, last & !3);
            let shift = 8 * (3 - (last & 3));
            set32(c, last & !3, w & !(0xFF << shift) | 34 << shift);
            let counts = leaf_check::sym("Item_804A0C64");
            let limit = w32(c, counts + 0x20);
            set32(c, counts + 0x1C, limit);
            true
        };
        check_on("it_8026C75C", "it_8026C75C", &prep, &|r, a| {
            Ret::Int(spawn::pick_kind(r.game, table_arg(a, 0)) as u32)
        });
    }
}

#[test]
fn init_globals() {
    check("Item_80266FCC", &|r, _| {
        item::init_globals(r.game, r.data);
        Ret::None
    });
}

#[test]
fn drop_at() {
    // A shot dropped at a fighter's feet, from the spawner's states, then with the limit
    // reached: the states with Fox's articles loaded.
    for full in [false, true] {
        let prep = move |c: &mut Call| {
            let table = w32(c, leaf_check::sym("it_804D6D38"));
            if table == 0 || w32(c, table + 4 * (54 - 43)) == 0 {
                return false;
            }
            let at = c.regs.gpr[1] - 0x200;
            for (k, v) in [0.0f32, 0.0, 0.0].iter().enumerate() {
                set32(c, at + 4 * k as u32, v.to_bits());
            }
            c.regs.gpr[3] = at;
            c.regs.gpr[4] = 54;
            if full {
                let counts = leaf_check::sym("Item_804A0C64");
                let limit = w32(c, counts + 0x64);
                set32(c, counts + 0x60, limit);
            }
            true
        };
        check_on("fn_8026C88C", "it_8026D258", &prep, &|r, a| {
            Ret::Bool(spawn::drop_at(r, a.vec(0), 54))
        });
    }
}

/// Sets the random seed (`*HSD_RandSeedPtr`) of a captured call.
fn set_seed(c: &mut Call, seed: u32) {
    let at = w32(c, leaf_check::sym("HSD_RandSeedPtr"));
    set32(c, at, seed);
}

/// Seeds spread over the generator's range.
fn seeds() -> impl Iterator<Item = u32> {
    (0..6u32).map(|i| i.wrapping_mul(0x9E37_79B9) ^ 0x5EED)
}

#[test]
fn spawn_seeded() {
    // The setup's first countdown and the spawn points, from many seeds.
    for seed in seeds() {
        let prep = move |c: &mut Call| {
            set_seed(c, seed);
            true
        };
        check_calls("it_8026D018", "it_8026D018", &prep, &|r, _| {
            spawn::setup(r);
            Ret::None
        });
        check_calls("it_8026CB3C", "it_8026CB3C", &prep, &|r, a| {
            let at = a.int(0);
            match spawn::spawn_point(r.game, r.data) {
                Some(p) => Ret::BoolWords(true, at, vec_words(p)),
                None => Ret::Bool(false),
            }
        });
    }
}

#[test]
fn spawn_bisect_edges() {
    // Every weight's edge, and past the last, up to the longest table captured.
    let longest = captures("bisectValue")
        .iter()
        .map(|s| {
            let c = s.read().call;
            w32(&c, c.regs.gpr[4]) >> 24
        })
        .max()
        .unwrap_or(0);
    for k in 0..3 * (longest + 1) {
        let prep = move |c: &mut Call| {
            let t = c.regs.gpr[4];
            let n = w32(c, t) >> 24;
            let i = k / 3;
            if i > n {
                return false;
            }
            let weights = w32(c, t + 0xC);
            let at = weights + 2 * i.min(n - 1);
            let w = (w32(c, at & !3) >> (16 * (1 - ((at & 2) >> 1)))) & 0xFFFF;
            c.regs.gpr[3] = (w + k % 3).wrapping_sub(1) + if i == n { 50 } else { 0 };
            true
        };
        check_calls("bisectValue", "bisectValue", &prep, &|r, a| {
            let t = table_arg(a, 1)(r.game).clone();
            let w = t.weights.as_deref().unwrap();
            Ret::Int(spawn::bisect(a.int(0) as i32, w, a.int(2) as i32, a.int(3) as i32) as u32)
        });
    }
}

// itmaplib.c and itgroundcoll.c, from the items-on corpus.

thread_local! {
    /// The callback a captured call passed, for [`passed_cb`].
    static PASSED: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// Runs the callback the captured call passed (`PASSED`), as the kinds' dispatch has it.
fn passed_cb(r: &mut Running, g: GObjId) {
    let a = PASSED.with(|p| p.get());
    if !melee_sim::game::it::kinds::run_event(r, g, a) {
        melee_sim::game::code::unported_call(&format!("item callback {a:#010x}"));
    }
}

/// [`passed_cb`] for the callback in argument `i`.
fn cb_arg(a: &Args, i: usize) -> ground::Event {
    let addr = a.int(i);
    PASSED.with(|p| p.set(addr));
    passed_cb
}

#[test]
fn maplib_queries() {
    check("it_80276214", &|r, a| {
        maplib::prep_coll(r.game, r.data, a.gobj(0));
        Ret::None
    });
    check("it_80276278", &|r, a| {
        maplib::turn_box(r.game, r.data, a.gobj(0));
        Ret::None
    });
    check("it_802762D8", &|r, a| {
        Ret::Bool(maplib::at_edge(r.game, a.gobj(0)))
    });
    check("it_80276308", &|r, a| {
        Ret::Int(maplib::touched_wall(r.game, a.gobj(0)) as u32)
    });
    check("it_80276348", &|r, a| {
        let (t, n) = maplib::touched_wall_normal(r.game, a.gobj(0));
        match n {
            Some(n) => Ret::IntWords(t as u32, a.int(1), vec_words(n)),
            None => Ret::Int(t as u32),
        }
    });
    check("it_802763E0", &|r, a| {
        Ret::Int(maplib::touched_ceiling(r.game, a.gobj(0)) as u32)
    });
    // Where the normal goes is the caller's: the calls that pass a stack vector (a barrel's
    // own, into its variables, run with its states).
    let on_stack = |c: &mut Call| c.regs.gpr[5].wrapping_sub(c.regs.gpr[1]) < 0x1000;
    check_calls("it_80276408", "it_80276408", &on_stack, &|r, a| {
        let cd = r.game.items[a.maps.items[&(a.int(1) - 0x378)].0]
            .x378_itemColl
            .clone();
        match maplib::surface_normal(&cd) {
            Some(n) => Ret::Words(a.int(2), vec_words(n)),
            None => Ret::None,
        }
    });
    check("it_8027649C", &|r, a| {
        Ret::Float(maplib::slope_angle(r.game, a.gobj(0)))
    });
    check("it_80276CB8", &|r, a| {
        maplib::tilt_on_floor(r.game, r.data, a.gobj(0));
        Ret::None
    });
    check("it_802765BC", &|r, a| {
        maplib::tilt_to_slope(r.game, r.data, a.gobj(0), a.int(1) as i32);
        Ret::None
    });
    check("it_80276CEC", &|r, a| {
        maplib::remember_surface(r.game, a.gobj(0));
        Ret::None
    });
}

#[test]
fn maplib_bounces() {
    check("it_80276D9C", &|r, a| {
        Ret::Bool(maplib::unstick(r.game, a.gobj(0), a.int(1) as i32))
    });
    check("it_80276FC4", &|r, a| {
        maplib::bounce(r, a.gobj(0), a.int(1) as i32);
        Ret::None
    });
    check("it_8027770C", &|r, a| {
        Ret::Bool(maplib::bounce_off_wall(r.game, r.data, a.gobj(0)))
    });
    check("it_8027781C", &|r, a| {
        Ret::Bool(maplib::reflect_velocity(r.game, r.data, a.gobj(0)))
    });
    check("it_80275D5C", &|r, a| {
        let b: melee_sim::game::structs::_itECB = a.get(1);
        maplib::set_box(r.game, a.gobj(0), b);
        Ret::None
    });
    check("it_80275DFC", &|r, a| {
        maplib::reset_box(r.game, a.gobj(0));
        Ret::None
    });
}

#[test]
fn maplib_skids() {
    check("it_80277040", &|r, a| {
        Ret::Bool(maplib::slides(r.game, r.data, a.gobj(0)))
    });
    check("it_80277544", &|r, a| {
        Ret::Bool(maplib::start_skid(r.game, r.data, a.gobj(0)))
    });
    check("it_8027737C", &|r, a| {
        let at = a.int(1);
        let mut pos = a.vec(1);
        maplib::slide(r.game, r.data, a.gobj(0), &mut pos);
        Ret::Words(at, vec_words(pos))
    });
}

#[test]
fn groundcoll() {
    check("it_8026D564", &|r, a| {
        Ret::Bool(ground::ground(r, a.gobj(0)))
    });
    check("it_8026D62C", &|r, a| {
        let cb = cb_arg(a, 1);
        ground::ground_or_fall(r, a.gobj(0), cb);
        Ret::None
    });
    check("it_8026D8A4", &|r, a| {
        let cb = cb_arg(a, 1);
        Ret::Bool(ground::ground_to_ledge(r, a.gobj(0), cb))
    });
    check("it_8026DA08", &|r, a| Ret::Bool(ground::air(r, a.gobj(0))));
    check("it_8026DF34", &|r, a| Ret::Bool(ground::air(r, a.gobj(0))));
    check("it_8026DC24", &|r, a| {
        Ret::Bool(ground::settles(r.game, r.data, a.gobj(0)))
    });
    check("it_8026DD5C", &|r, a| Ret::Bool(ground::land(r, a.gobj(0))));
    check("it_8026DDFC", &|r, a| {
        Ret::Bool(ground::count_landing(r, a.gobj(0)))
    });
    check("it_8026E15C", &|r, a| {
        let cb = cb_arg(a, 1);
        ground::fall(r, a.gobj(0), cb);
        Ret::None
    });
    check("it_8026E32C", &|r, a| {
        let cb = cb_arg(a, 1);
        Ret::Int(ground::fall_unbreakable(r, a.gobj(0), cb) as u32)
    });
    check("it_8026E414", &|r, a| {
        let cb = cb_arg(a, 1);
        ground::fall_to_floor(r, a.gobj(0), cb);
        Ret::None
    });
}

// itcapsule.c, from the items-on corpus.

#[test]
fn capsule_spawned() {
    check_proc("itCapsule_Logic0_Spawned", capsule::spawned);
}

/// Makes the table without containers drop only capsules, from a seed.
fn only_capsules(c: &mut Call, seed: u32) {
    let t = leaf_check::sym("it_804A0E50");
    let n = w32(c, t) >> 24;
    let kinds = w32(c, t + 4);
    for i in 0..n {
        let at = kinds + i;
        let w = w32(c, at & !3);
        let shift = 8 * (3 - (at & 3));
        set32(c, at & !3, w & !(0xFF << shift));
    }
    set_seed(c, seed);
}

#[test]
fn capsule_landed() {
    // From the capsules' standing states.
    let prep = |c: &mut Call| item_kind(c) == 0;
    check_on(
        "itCapsule_UnkMotion0_Anim",
        "it_8027D0B8",
        &prep,
        &|r, a| {
            capsule::landed(r, a.gobj(0));
            Ret::None
        },
    );
    check_on(
        "itCapsule_UnkMotion0_Anim",
        "it_8027D148",
        &prep,
        &|r, a| {
            capsule::fell(r, a.gobj(0));
            Ret::None
        },
    );
}

#[test]
fn capsule_opens() {
    // Opened where it stands, dropping capsules (the only kind ported that can come out); the
    // sixth seed's capsule explodes.
    for seed in seeds() {
        let prep = move |c: &mut Call| {
            only_capsules(c, seed);
            item_kind(c) == 0
        };
        check_on(
            "itCapsule_UnkMotion0_Anim",
            "it_8027CF30",
            &prep,
            &|r, a| {
                capsule::land_check(r, a.gobj(0));
                Ret::None
            },
        );
        check_on(
            "itCapsule_UnkMotion0_Anim",
            "itCapsule_Logic0_DmgReceived",
            &prep,
            &|r, a| Ret::Bool(capsule::hit(r, a.gobj(0))),
        );
    }
}

#[test]
fn capsule_explodes() {
    let prep = |c: &mut Call| item_kind(c) == 0;
    check_on(
        "itCapsule_UnkMotion0_Anim",
        "it_8027CFE8",
        &prep,
        &|r, a| {
            capsule::explode(r, a.gobj(0));
            Ret::None
        },
    );
}

#[test]
fn capsule_states() {
    check_pred("itCapsule_UnkMotion0_Anim", |r, g| {
        kinds::run_pred(r, g, capsule::ANIM0).unwrap()
    });
    check_proc("itCapsule_UnkMotion0_Phys", |r, g| {
        assert!(kinds::run_event(r, g, capsule::PHYS0));
    });
    check_pred("itCapsule_UnkMotion0_Coll", capsule::coll0);
    check_pred("itCapsule_UnkMotion4_Anim", |r, g| {
        kinds::run_pred(r, g, capsule::ANIM4).unwrap()
    });
    check_proc("itCapsule_UnkMotion4_Phys", capsule::phys4);
    check_pred("itCapsule_UnkMotion4_Coll", capsule::coll4);
}

/// The capsules' standing captures.
fn standing_capsule(c: &mut Call) -> bool {
    item_kind(c) == 0
}

/// A capsule's callback run on the capsules' standing (`air` false) or falling captures.
fn on_capsules(target: &str, air: bool, ours: &Ours) {
    let source = if air {
        "itCapsule_UnkMotion4_Anim"
    } else {
        "itCapsule_UnkMotion0_Anim"
    };
    check_on(source, target, &standing_capsule, ours);
}

#[test]
fn capsule_callbacks() {
    for (target, f) in [
        (
            "itCapsule_Logic0_PickedUp",
            capsule::picked_up as fn(&mut Running, GObjId),
        ),
        ("itCapsule_Logic0_Dropped", capsule::dropped),
        ("itCapsule_Logic0_Thrown", capsule::thrown),
        ("itCapsule_Logic0_EnteredAir", capsule::entered_air),
        ("itCapsule_UnkMotion3_Phys", capsule::phys3),
        ("itCapsule_UnkMotion2_Phys", capsule::idle_phys),
        ("itCapsule_UnkMotion5_Phys", capsule::idle_phys),
        ("itCapsule_UnkMotion6_Phys", capsule::idle_phys),
    ] {
        on_capsules(target, false, &move |r, a| {
            f(r, a.gobj(0));
            Ret::None
        });
    }
    for (target, f, air) in [
        (
            "itCapsule_UnkMotion2_Anim",
            capsule::idle_anim as fn(&mut Running, GObjId) -> bool,
            false,
        ),
        ("itCapsule_UnkMotion6_Anim", capsule::idle_anim, false),
        ("itCapsule_UnkMotion5_Coll", capsule::idle_anim, false),
        ("itCapsule_UnkMotion5_Anim", capsule::vanishing, false),
        ("itCapsule_UnkMotion6_Coll", capsule::coll6, false),
        ("itCapsule_UnkMotion3_Coll", capsule::coll3, true),
        ("itCapsule_UnkMotion6_Coll", capsule::coll6, true),
    ] {
        on_capsules(target, air, &move |r, a| Ret::Bool(f(r, a.gobj(0))));
    }
}

#[test]
fn capsule_hits() {
    // Hit, clanked or reflected while standing (it opens: drops capsules), or while
    // exploding (state 5, nothing).
    for msid in [None, Some(5)] {
        for target in [
            "itCapsule_Logic0_DmgDealt",
            "itCapsule_Logic0_Clanked",
            "itCapsule_Logic0_Reflected",
        ] {
            let prep = move |c: &mut Call| {
                only_capsules(c, 0x5EED);
                if let Some(m) = msid {
                    set32(c, item_ptr(c) + 0x24, m);
                }
                standing_capsule(c)
            };
            check_on("itCapsule_UnkMotion0_Anim", target, &prep, &|r, a| {
                Ret::Bool(capsule::hit(r, a.gobj(0)))
            });
        }
    }
    // A shield, thrown (state 3) or dropped (4), or not.
    for msid in [0u32, 3, 4] {
        let prep = move |c: &mut Call| {
            only_capsules(c, 0x5EED);
            set32(c, item_ptr(c) + 0x24, msid);
            set32(c, item_ptr(c) + 0xC58, 0.6f32.to_bits());
            set32(c, item_ptr(c) + 0xC5C, 0.8f32.to_bits());
            standing_capsule(c)
        };
        check_on(
            "itCapsule_UnkMotion0_Anim",
            "itCapsule_Logic0_HitShield",
            &prep,
            &|r, a| Ret::Bool(capsule::hit_shield(r, a.gobj(0))),
        );
        check_on(
            "itCapsule_UnkMotion0_Anim",
            "itCapsule_Logic0_ShieldBounced",
            &prep,
            &|r, a| Ret::Bool(capsule::shield_bounced(r, a.gobj(0))),
        );
    }
    // Already exploding: breaking again does nothing.
    let prep = |c: &mut Call| {
        set32(c, item_ptr(c) + 0xDD4 + 4, 1);
        standing_capsule(c)
    };
    check_on(
        "itCapsule_UnkMotion0_Anim",
        "it_8027D2DC",
        &prep,
        &|r, a| {
            capsule::break_open(r, a.gobj(0));
            Ret::None
        },
    );
}

/// Makes the monsters' table drop only capsules.
fn monsters_capsules(c: &mut Call) {
    let t = leaf_check::sym("it_804A0E60");
    let n = w32(c, t) >> 24;
    let kinds = w32(c, t + 4);
    for i in 0..n {
        let at = kinds + i;
        let w = w32(c, at & !3);
        let shift = 8 * (3 - (at & 3));
        set32(c, at & !3, w & !(0xFF << shift));
    }
}

/// Points r`reg` at a vector below the stack pointer, set to `v`.
fn vec_arg(c: &mut Call, reg: usize, off: u32, v: [f32; 3]) {
    let at = c.regs.gpr[1] - off;
    for (k, x) in v.iter().enumerate() {
        set32(c, at + 4 * k as u32, x.to_bits());
    }
    c.regs.gpr[reg] = at;
}

#[test]
fn drops() {
    use melee_sim::game::it::drop;
    let base = |c: &mut Call| {
        only_capsules(c, 0x5EED);
        monsters_capsules(c);
        standing_capsule(c)
    };
    check_on(
        "itCapsule_UnkMotion0_Anim",
        "it_8026F3AC",
        &base,
        &|r, _| Ret::Int(drop::pick(r.game) as u32),
    );
    for (n, same) in [(1u32, 0u32), (3, 0), (2, 1)] {
        let prep = move |c: &mut Call| {
            c.regs.gpr[4] = 0;
            c.regs.gpr[5] = n;
            c.regs.gpr[6] = same;
            base(c)
        };
        check_on(
            "itCapsule_UnkMotion0_Anim",
            "it_8026F3D4",
            &prep,
            &|r, a| {
                Ret::Bool(
                    drop::drop_contents(r, a.gobj(0), a.int(2) as i32, a.int(3) & 1 != 0).stopped,
                )
            },
        );
    }
    for chk in [0u32, 1] {
        let prep = move |c: &mut Call| {
            vec_arg(c, 4, 0x3000, [3.0, 7.0, 0.0]);
            c.regs.gpr[5] = chk;
            base(c)
        };
        check_on(
            "itCapsule_UnkMotion0_Anim",
            "it_8026F53C",
            &prep,
            &|r, a| {
                let vel = (a.int(2) != 0).then(|| a.vec(1));
                drop::pop_up(r.game, r.data, a.gobj(0), vel);
                let v = r.game.items[a.maps.items[&a.cap.u32(a.int(0) + 0x2C)].0].x40_vel;
                Ret::Words(a.int(1), vec_words(v))
            },
        );
    }
    let prep = |c: &mut Call| {
        c.regs.gpr[4] = 0;
        vec_arg(c, 5, 0x3000, [10.0, 20.0, 5.0]);
        base(c)
    };
    check_on(
        "itCapsule_UnkMotion0_Anim",
        "it_8026F5C8",
        &prep,
        &|r, a| Ret::GObj(drop::spawn_at(r, Some(a.gobj(0)), 0, a.vec(2))),
    );
    for chk in [0u32, 1] {
        let prep = move |c: &mut Call| {
            vec_arg(c, 4, 0x3000, [10.0, 20.0, 5.0]);
            vec_arg(c, 5, 0x3020, [1.0, 2.0, 0.0]);
            c.regs.gpr[6] = chk;
            base(c)
        };
        check_on(
            "itCapsule_UnkMotion0_Anim",
            "it_8026F6BC",
            &prep,
            &|r, a| {
                let vel = (a.int(3) != 0).then(|| a.vec(2));
                Ret::GObj(drop::drop_monster(r, Some(a.gobj(0)), a.vec(1), vel))
            },
        );
        let prep = move |c: &mut Call| {
            let ok = base(c);
            vec_arg(c, 3, 0x3000, [10.0, 20.0, 5.0]);
            vec_arg(c, 4, 0x3020, [1.0, 2.0, 0.0]);
            c.regs.gpr[5] = chk;
            ok
        };
        check_on(
            "itCapsule_UnkMotion0_Anim",
            "it_8026F7C8",
            &prep,
            &|r, a| {
                let vel = (a.int(2) != 0).then(|| a.vec(1));
                Ret::GObj(drop::drop_random(r, a.vec(0), vel))
            },
        );
    }
    let prep = |c: &mut Call| {
        vec_arg(c, 4, 0x3000, [10.0, 20.0, 5.0]);
        vec_arg(c, 5, 0x3020, [1.0, 2.0, 0.0]);
        c.regs.gpr[6] = 0;
        base(c)
    };
    check_on(
        "itCapsule_UnkMotion0_Anim",
        "it_8026F8B4",
        &prep,
        &|r, a| Ret::Bool(drop::drop_coins(r, a.gobj(0), a.vec(1), None)),
    );
}

/// A check taking a callback, a predicate, and a check returning what it touched.
type WithEvent = fn(&mut Running, GObjId, ground::Event);
type Pred = fn(&mut Running, GObjId) -> bool;
type IntFn = fn(&mut Running, GObjId) -> i32;

#[test]
fn groundcoll_variants() {
    // Every variant from the capsules' falling and standing states, with the capsule's own
    // callbacks passed (landing, falling).
    let air = "itCapsule_UnkMotion4_Coll";
    let ground = "itCapsule_UnkMotion0_Coll";
    let with_cbs = |c: &mut Call| {
        c.regs.gpr[4] = capsule::LANDED;
        c.regs.gpr[5] = capsule::FELL;
        standing_capsule(c)
    };
    let events: [(&str, WithEvent); 8] = [
        ("it_8026E248", ground::fall_keep_hits),
        ("it_8026E4D0", ground::fall_to_floor_rescaled),
        ("it_8026E5A0", ground::fall_until_touch),
        ("it_8026E664", ground::fall_until_touch_no_ceiling),
        ("it_8026E71C", ground::fall_until_touch_unstick),
        ("it_8026E7E0", ground::fall_unstick),
        ("it_8026D6F4", ground::ground_or),
        ("it_8026D78C", ground::ground_or_then_air),
    ];
    for source in [air, ground] {
        for (target, f) in events {
            check_on(source, target, &with_cbs, &move |r, a| {
                let cb = cb_arg(a, 1);
                f(r, a.gobj(0), cb);
                Ret::None
            });
        }
        check_on(source, "it_8026D938", &with_cbs, &|r, a| {
            let cb = cb_arg(a, 1);
            ground::line_or(r, a.gobj(0), cb);
            Ret::None
        });
        // Only on the ground: from the air, a grounded pass can land without a floor, and
        // the slope check then reads a normal the original never set.
        if source == ground {
            check_on(source, "it_8026E8C4", &with_cbs, &|r, a| {
                let (stopped, fell) = (cb_arg(a, 1), passed_fell(a));
                ground::ground_skid(r, a.gobj(0), stopped, fell);
                Ret::None
            });
        }
        let bools: [(&str, Pred); 7] = [
            ("it_8026D5CC", ground::ground_probe),
            ("it_8026D9A0", ground::air_stay),
            ("it_8026DA70", ground::air_probe),
            ("it_8026DBC8", ground::try_land),
            ("it_8026DE98", ground::count_landing),
            ("it_8026DFB0", ground::air_touched_any),
            ("it_8026E058", ground::air_touched_floor_or_wall),
        ];
        for (target, f) in bools {
            // Landing without having touched a floor isn't something the game does.
            if target == "it_8026DBC8" && source == air {
                continue;
            }
            check_on(source, target, &standing_capsule, &move |r, a| {
                Ret::Bool(f(r, a.gobj(0)))
            });
        }
        let ints: [(&str, IntFn); 2] = [
            ("it_8026DAA8", ground::air_touches),
            ("it_8026DB40", ground::air_touches_walls),
        ];
        for (target, f) in ints {
            check_on(source, target, &standing_capsule, &move |r, a| {
                Ret::Int(f(r, a.gobj(0)) as u32)
            });
        }
        check_on(source, "it_8026D82C", &standing_capsule, &|r, a| {
            ground::ground_or_air(r, a.gobj(0));
            Ret::None
        });
        check_on(source, "it_8026E0F4", &standing_capsule, &|r, a| {
            ground::air_stay(r, a.gobj(0));
            Ret::None
        });
    }
}

thread_local! {
    /// The second callback a captured call passed.
    static PASSED2: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// Runs the second callback the captured call passed.
fn passed_cb2(r: &mut Running, g: GObjId) {
    let a = PASSED2.with(|p| p.get());
    if !kinds::run_event(r, g, a) {
        melee_sim::game::code::unported_call(&format!("item callback {a:#010x}"));
    }
}

/// [`passed_cb2`] for the callback in r5.
fn passed_fell(a: &Args) -> ground::Event {
    let addr = a.int(2);
    PASSED2.with(|p| p.set(addr));
    passed_cb2
}

#[test]
fn item_lib_small() {
    // Small setters and helpers, on the capsules' standing states.
    let on = |target: &str, f: fn(&mut Running, GObjId)| {
        check_on(
            "itCapsule_UnkMotion0_Anim",
            target,
            &standing_capsule,
            &move |r, a| {
                f(r, a.gobj(0));
                Ret::None
            },
        );
    };
    on("it_8026BD24", |r, g| lib26b1::set_xdd0_b3(r.game, g));
    on("it_8026BDB4", |r, g| lib26b1::stop_keeping_clear(r.game, g));
    on("it_8027518C", |r, g| {
        lib2725::start_vanishing(r.game, r.data, g)
    });
    on("it_80275444", |r, g| hitbox::set_hit_ids(r.game, g));
    on("it_80273454", |r, g| lib2725::clear_velocity(r.game, g));
    on("it_80274740", |r, g| lib2725::stop_spin(r.game, r.data, g));
    on("Item_8026ADC0", item::enter_air_state);
    check_on(
        "itCapsule_UnkMotion0_Anim",
        "it_802763B8",
        &standing_capsule,
        &|r, a| Ret::Bool(maplib::touched_floor(r.game, a.gobj(0))),
    );
    check_on(
        "itCapsule_UnkMotion0_Anim",
        "it_80274990",
        &standing_capsule,
        &|r, a| Ret::Float(f64::from(lib2725::spin_angle(r.game, r.data, a.gobj(0)))),
    );
    for bone in [0u32, 1] {
        let prep = move |c: &mut Call| {
            c.regs.gpr[4] = bone;
            standing_capsule(c)
        };
        check_on(
            "itCapsule_UnkMotion0_Anim",
            "it_80276934",
            &prep,
            &|r, a| {
                maplib::tilt_to_line(r.game, r.data, a.gobj(0), a.int(1) as i32);
                Ret::None
            },
        );
    }
    // A spinning item's spin from a skid in either direction.
    for x in [3.0f32, -3.0] {
        let prep = move |c: &mut Call| {
            set32(c, item_ptr(c) + 0x34, 0.5f32.to_bits());
            vec_arg(c, 4, 0x3000, [x, 1.0, 0.0]);
            standing_capsule(c)
        };
        check_on(
            "itCapsule_UnkMotion0_Anim",
            "it_802775F0",
            &prep,
            &|r, a| {
                maplib::spin_from_speed(r.game, a.gobj(0), a.vec(1));
                Ret::None
            },
        );
    }
    // Hitboxes scaled, with the bounce sound off (`xDCD.b2`), and the wall bounce's sound.
    let quiet = |c: &mut Call| {
        or8(c, item_ptr(c) + 0xDCD, 0x20);
        standing_capsule(c)
    };
    check_on(
        "itCapsule_UnkMotion0_Anim",
        "it_8027321C",
        &quiet,
        &|r, a| {
            lib2725::bounce_sfx(r, a.gobj(0));
            Ret::None
        },
    );
    for mult in [0.5f32, 2.0] {
        let prep = move |c: &mut Call| {
            set_f(c, 1, mult);
            standing_capsule(c)
        };
        check_on(
            "itCapsule_UnkMotion0_Anim",
            "it_80275640",
            &prep,
            &|r, a| {
                hitbox::scale_damage(r.game, r.data, a.gobj(0), f64::from(a.float(0)));
                Ret::None
            },
        );
    }
}

/// A kind's callback: an event or a predicate.
#[derive(Clone, Copy)]
enum Cb {
    Event(fn(&mut Running, GObjId)),
    Pred(fn(&mut Running, GObjId) -> bool),
}

impl Cb {
    fn run(self, r: &mut Running, g: GObjId) -> Ret {
        match self {
            Cb::Event(f) => {
                f(r, g);
                Ret::None
            }
            Cb::Pred(f) => Ret::Bool(f(r, g)),
        }
    }
}

/// Each callback on every captured call of it.
fn kind_direct(list: &[(&str, Cb)]) {
    // Several at once ([`leaf_check::each_set`]), each taking its [`Turn`].
    leaf_check::each_set(
        list,
        |&(name, _)| captures(name).len(),
        |&(name, cb)| check(name, &move |r, a| cb.run(r, a.gobj(0))),
    );
}

/// Each callback run from the captures of `source` (items of kind `kind`).
fn kind_perturbed(source: &str, kind: i32, list: &[(&str, Cb)]) {
    // Several at once ([`leaf_check::each_set`]), each taking its [`Turn`].
    leaf_check::each_set(
        list,
        |_| 1,
        |&(name, cb)| {
            let prep = move |c: &mut Call| item_kind(c) == kind;
            check_on(source, name, &prep, &move |r, a| cb.run(r, a.gobj(0)));
        },
    );
}

#[test]
fn heart() {
    use melee_sim::game::it::kinds::heart as h;
    kind_direct(&[
        ("itHeart_Logic8_Spawned", Cb::Event(h::spawned)),
        ("itHeart_Logic8_Destroyed", Cb::Event(h::destroyed)),
        ("itHeart_UnkMotion0_Anim", Cb::Pred(h::spin)),
        ("itHeart_UnkMotion0_Phys", Cb::Event(h::idle)),
        ("itHeart_UnkMotion0_Coll", Cb::Pred(h::coll0)),
        ("itHeart_UnkMotion3_Anim", Cb::Pred(h::spin)),
        ("itHeart_UnkMotion3_Phys", Cb::Event(h::phys3)),
        ("itHeart_UnkMotion3_Coll", Cb::Pred(h::coll3)),
        ("itHeart_UnkMotion2_Anim", Cb::Pred(h::still)),
        ("itHeart_UnkMotion2_Phys", Cb::Event(h::idle)),
    ]);
    kind_perturbed(
        "itHeart_UnkMotion0_Anim",
        8,
        &[
            ("it_80283C7C", Cb::Event(h::landed)),
            ("it_80283DD4", Cb::Event(h::fell)),
            ("it_80283BD4", Cb::Event(h::second_heal)),
            ("itHeart_Logic8_PickedUp", Cb::Event(h::picked_up)),
            ("itHeart_Logic8_Dropped", Cb::Event(h::dropped)),
            ("itHeart_Logic8_EnteredAir", Cb::Event(h::entered_air)),
            ("itHeart_UnkMotion4_Anim", Cb::Pred(h::still)),
            ("itHeart_UnkMotion4_Phys", Cb::Event(h::idle)),
            ("itHeart_UnkMotion4_Coll", Cb::Pred(h::coll4)),
        ],
    );
}

#[test]
fn tomato() {
    use melee_sim::game::it::kinds::tomato as t;
    kind_direct(&[
        ("itTomato_Logic9_Spawned", Cb::Event(t::spawned)),
        ("itTomato_Logic9_Destroyed", Cb::Event(t::destroyed)),
        ("itTomato_UnkMotion0_Anim", Cb::Pred(t::still)),
        ("itTomato_UnkMotion0_Phys", Cb::Event(t::gravity)),
        ("itTomato_UnkMotion0_Coll", Cb::Pred(t::falling)),
        ("itTomato_UnkMotion1_Anim", Cb::Pred(t::still)),
        ("itTomato_UnkMotion1_Phys", Cb::Event(t::idle)),
        ("itTomato_UnkMotion1_Coll", Cb::Pred(t::coll1)),
        ("itTomato_UnkMotion3_Anim", Cb::Pred(t::still)),
        ("itTomato_UnkMotion3_Phys", Cb::Event(t::idle)),
    ]);
    kind_perturbed(
        "itTomato_UnkMotion1_Anim",
        9,
        &[
            ("it_80284358", Cb::Event(t::fell)),
            ("it_802843E4", Cb::Event(t::landed)),
            ("it_80284458", Cb::Event(t::slipped)),
            ("it_8028428C", Cb::Event(t::second_heal)),
            ("itTomato_Logic9_PickedUp", Cb::Event(t::picked_up)),
            ("itTomato_Logic9_Dropped", Cb::Event(t::dropped)),
            ("itTomato_Logic9_EnteredAir", Cb::Event(t::entered_air)),
            ("itTomato_UnkMotion4_Anim", Cb::Pred(t::still)),
            ("itTomato_UnkMotion4_Phys", Cb::Event(t::gravity)),
            ("itTomato_UnkMotion4_Coll", Cb::Pred(t::falling)),
            ("itTomato_UnkMotion5_Anim", Cb::Pred(t::still)),
            ("itTomato_UnkMotion5_Phys", Cb::Event(t::idle)),
            ("itTomato_UnkMotion5_Coll", Cb::Pred(t::coll5)),
        ],
    );
}

#[test]
fn bat() {
    use melee_sim::game::it::kinds::bat as b;
    kind_direct(&[
        ("itBat_Logic11_Spawned", Cb::Event(b::spawned)),
        ("it_80284974", Cb::Event(b::landed)),
        ("itBat_UnkMotion0_Anim", Cb::Pred(b::still)),
        ("itBat_UnkMotion0_Phys", Cb::Event(b::idle)),
        ("itBat_UnkMotion0_Coll", Cb::Pred(b::coll0)),
        ("it_802849F0", Cb::Event(b::fell)),
        ("itBat_UnkMotion3_Anim", Cb::Pred(b::still)),
        ("itBat_UnkMotion3_Phys", Cb::Event(b::phys3)),
        ("itBat_UnkMotion3_Coll", Cb::Pred(b::coll3)),
        ("itBat_Logic11_PickedUp", Cb::Event(b::picked_up)),
        ("itBat_UnkMotion2_Anim", Cb::Pred(b::still)),
        ("itBat_UnkMotion2_Phys", Cb::Event(b::idle)),
        ("itBat_UnkMotion2_Coll", Cb::Pred(b::still)),
        ("itBat_Logic11_Dropped", Cb::Event(b::dropped)),
        ("itBat_Logic11_DmgDealt", Cb::Pred(b::dmg_dealt)),
        ("itBat_Logic11_Clanked", Cb::Pred(b::bounce)),
    ]);
    kind_perturbed(
        "itBat_UnkMotion0_Anim",
        11,
        &[
            ("itBat_Logic11_Thrown", Cb::Event(b::dropped)),
            ("itBat_Logic11_EnteredAir", Cb::Event(b::entered_air)),
            ("itBat_UnkMotion4_Anim", Cb::Pred(b::still)),
            ("itBat_UnkMotion4_Phys", Cb::Event(b::idle)),
            ("itBat_UnkMotion4_Coll", Cb::Pred(b::coll4)),
            ("itBat_Logic11_Reflected", Cb::Pred(b::reflected)),
            ("itBat_Logic11_HitShield", Cb::Pred(b::bounce)),
            ("itBat_Logic11_ShieldBounced", Cb::Pred(b::shield_bounced)),
        ],
    );
    // Thrown (state 3), a hit bounces it back.
    let thrown = |c: &mut Call| {
        set32(c, item_ptr(c) + 0x24, 3);
        item_kind(c) == 11
    };
    check_on(
        "itBat_UnkMotion0_Anim",
        "itBat_Logic11_DmgDealt",
        &thrown,
        &|r, a| Ret::Bool(melee_sim::game::it::kinds::bat::dmg_dealt(r, a.gobj(0))),
    );
}

#[test]
fn sword() {
    use melee_sim::game::it::kinds::sword as s;
    kind_direct(&[
        ("itSword_Logic12_Spawned", Cb::Event(s::spawned)),
        ("it_80285424", Cb::Event(s::landed)),
        ("itSword_UnkMotion0_Anim", Cb::Pred(s::ease)),
        ("itSword_UnkMotion0_Phys", Cb::Event(s::idle)),
        ("itSword_UnkMotion0_Coll", Cb::Pred(s::coll0)),
        ("it_802855F8", Cb::Event(s::fell)),
        ("itSword_UnkMotion3_Anim", Cb::Pred(s::ease)),
        ("itSword_UnkMotion3_Phys", Cb::Event(s::phys3)),
        ("itSword_UnkMotion3_Coll", Cb::Pred(s::coll3)),
        ("itSword_Logic12_PickedUp", Cb::Event(s::picked_up)),
        ("itSword_UnkMotion2_Anim", Cb::Pred(s::anim2)),
        ("itSword_UnkMotion2_Phys", Cb::Event(s::idle)),
        ("itSword_UnkMotion2_Coll", Cb::Pred(s::still)),
        ("itSword_Logic12_Dropped", Cb::Event(s::dropped)),
        ("itSword_Logic12_Thrown", Cb::Event(s::dropped)),
        ("itSword_Logic12_DmgDealt", Cb::Pred(s::dmg_dealt)),
        ("it_80284E30", Cb::Event(s::scale_blade)),
        ("it_80285084", Cb::Event(|r, g| s::ease_length(r.game, g))),
        ("it_80285140", Cb::Event(|r, g| s::ease_width(r.game, g))),
        ("it_802851FC", Cb::Event(|r, g| s::ease_swing(r.game, g))),
    ]);
    check("it_80284FC4", &|r, a| {
        s::grow(r.game, a.gobj(0), a.int(1) as i32, a.float(0));
        Ret::None
    });
    check("it_80285024", &|r, a| {
        s::shrink(r.game, a.gobj(0), a.int(1) as i32);
        Ret::None
    });
    kind_perturbed(
        "itSword_UnkMotion0_Anim",
        12,
        &[
            ("itSword_Logic12_EnteredAir", Cb::Event(s::entered_air)),
            ("itSword_UnkMotion4_Anim", Cb::Pred(s::ease)),
            ("itSword_UnkMotion4_Phys", Cb::Event(s::idle)),
            ("itSword_UnkMotion4_Coll", Cb::Pred(s::coll4)),
            ("itSword_Logic12_Reflected", Cb::Pred(s::reflected)),
            ("itSword_Logic12_Clanked", Cb::Pred(s::bounce)),
            ("itSword_Logic12_HitShield", Cb::Pred(s::bounce)),
            ("itSword_Logic12_ShieldBounced", Cb::Pred(s::shield_bounced)),
        ],
    );
    check_on(
        "itSword_UnkMotion2_Anim",
        "ftLib_IsSwordSwing",
        &|c| {
            let ip = item_ptr(c);
            let owner = w32(c, ip + 0x518);
            c.regs.gpr[3] = owner;
            owner != 0
        },
        &|r, a| Ret::Bool(s::is_sword_swing(r.game, a.gobj(0))),
    );
    // A sword dropped at a point (as the item switch's spawns drop one), on the stage's
    // plane whatever the depth given.
    for (k, at) in [[10.0, 30.0, 0.0], [-42.5, 7.25, 3.0]]
        .into_iter()
        .enumerate()
    {
        let prep = move |c: &mut Call| {
            vec_arg(c, 3, 0x3000 + 0x100 * k as u32, at);
            true
        };
        check_on(
            "itSword_UnkMotion0_Anim",
            "itSword_Spawn",
            &prep,
            &|r, a| Ret::GObj(s::spawn_at(r, a.vec(0))),
        );
    }
    // A swinging fighter's trail: its widths, written to two words below the stack (either
    // left out), its look and its joint, from every state of a sword.
    for (w0, w1) in [(true, true), (false, true), (true, false)] {
        let prep = move |c: &mut Call| {
            let sp = c.regs.gpr[1];
            c.regs.gpr[4] = if w0 { sp - 0x3000 } else { 0 };
            c.regs.gpr[5] = if w1 { sp - 0x2FFC } else { 0 };
            set32(c, sp - 0x3000, 0xDEAD_BEEF);
            set32(c, sp - 0x2FFC, 0xDEAD_BEEF);
            item_kind(c) == 12
        };
        for src in ["itSword_UnkMotion0_Anim", "itSword_UnkMotion2_Anim"] {
            check_on(src, "it_802852B8", &prep, &move |r, a| {
                let (x, y) = s::trail_widths(r.game, a.gobj(0));
                let word = |on: bool, v: f32| if on { v.to_bits() } else { 0xDEAD_BEEF };
                let at = a.cap.call.regs.gpr[1] - 0x3000;
                Ret::Words(at, vec![word(w0, x), word(w1, y)])
            });
        }
    }
    for src in ["itSword_UnkMotion0_Anim", "itSword_UnkMotion2_Anim"] {
        check_on(src, "it_80285300", &|c| item_kind(c) == 12, &|r, a| {
            Ret::Int(a.data.addr(s::trail_params(r.game, r.data, a.gobj(0))))
        });
        check_on(src, "it_80285314", &|c| item_kind(c) == 12, &|r, a| {
            Ret::JObj(s::trail_joint(r.game, r.data, a.gobj(0)))
        });
    }
}

#[test]
fn hammer() {
    use melee_sim::game::it::kinds::hammer as h;
    kind_direct(&[
        ("itHammer_Logic28_Spawned", Cb::Event(h::spawned)),
        ("it_80293F84", Cb::Event(h::landed)),
        ("itHammer_UnkMotion0_Anim", Cb::Pred(h::still)),
        ("itHammer_UnkMotion0_Phys", Cb::Event(h::idle)),
        ("itHammer_UnkMotion0_Coll", Cb::Pred(h::coll0)),
        ("it_8029402C", Cb::Event(h::fell)),
        ("itHammer_UnkMotion4_Anim", Cb::Pred(h::still)),
        ("itHammer_UnkMotion1_Phys", Cb::Event(h::phys_fall)),
        ("itHammer_UnkMotion1_Coll", Cb::Pred(h::coll1)),
        ("itHammer_Logic28_PickedUp", Cb::Event(h::picked_up)),
        ("itHammer_UnkMotion3_Anim", Cb::Pred(h::anim3)),
        ("itHammer_Logic28_Dropped", Cb::Event(h::dropped)),
        ("it_80293D94", Cb::Event(h::hitlag)),
    ]);
    check("it_80293DCC", &|r, a| {
        Ret::Int(h::swings_to_lose_head(r, a.gobj(0)) as u32)
    });
    kind_perturbed(
        "itHammer_UnkMotion0_Anim",
        28,
        &[
            ("itHammer_Logic28_EnteredAir", Cb::Event(h::entered_air)),
            ("itHammer_UnkMotion5_Anim", Cb::Pred(h::still)),
            ("itHammer_UnkMotion5_Phys", Cb::Event(h::idle)),
            ("itHammer_UnkMotion5_Coll", Cb::Pred(h::coll5)),
            ("itHammer_UnkMotion4_Phys", Cb::Event(h::phys_fall)),
            ("itHammer_UnkMotion4_Coll", Cb::Pred(h::coll4)),
            ("it_802941A4", Cb::Event(h::headless)),
        ],
    );
    // The headless hammer: in the air it's gone on touching anything, and held it turns
    // headless.
    for source in ["itHammer_UnkMotion1_Coll", "itHammer_UnkMotion3_Anim"] {
        let prep = |c: &mut Call| {
            set32(c, item_ptr(c) + 0xD4C, 0);
            true
        };
        let ours: &Ours = if source.ends_with("Coll") {
            &|r, a| Ret::Bool(h::coll1(r, a.gobj(0)))
        } else {
            &|r, a| Ret::Bool(h::anim3(r, a.gobj(0)))
        };
        check_calls(source, source, &prep, ours);
    }
    // The head flies off (with heads left) or the joint just hides (with none), from the held
    // hammers the head's chance was drawn for.
    for left in [1u32, 0, 2] {
        let prep = move |c: &mut Call| {
            set32(c, item_ptr(c) + 0xD4C, left);
            true
        };
        check_on("it_80293DCC", "it_80293E34", &prep, &|r, a| {
            h::lose_head(r, a.gobj(0));
            Ret::None
        });
    }
    check_on(
        "itHammer_UnkMotion3_Anim",
        "itHammer_Logic28_EvtUnk",
        &pass_owner,
        &|r, a| {
            h::forget(r, a.gobj(0), a.opt_gobj(1));
            Ret::None
        },
    );
}

#[test]
fn hammer_head() {
    use melee_sim::game::it::kinds::hammerhead as h;
    // No game made a head fly off, so its callbacks run on the hammers' states (the attributes
    // they read are the hammer's).
    kind_perturbed(
        "itHammer_UnkMotion0_Anim",
        28,
        &[
            ("itHammerHead_Logic40_Spawned", Cb::Event(h::spawned)),
            ("it_80299D7C", Cb::Event(h::fly)),
            ("itHammerHead_Logic40_PickedUp", Cb::Event(h::picked_up)),
            ("itHammerhead_UnkMotion1_Anim", Cb::Pred(h::no)),
            ("itHammerhead_UnkMotion1_Phys", Cb::Event(h::idle)),
            ("itHammerHead_Logic40_Dropped", Cb::Event(h::thrown)),
            ("itHammerHead_Logic40_Thrown", Cb::Event(h::thrown)),
            ("itHammerhead_UnkMotion2_Anim", Cb::Pred(h::tick)),
            ("itHammerhead_UnkMotion2_Phys", Cb::Event(h::phys2)),
            ("itHammerhead_UnkMotion2_Coll", Cb::Pred(h::coll2)),
            ("it_80299F94", Cb::Event(h::landed)),
            ("it_80299FB4", Cb::Event(h::landed)),
            ("itHammerhead_UnkMotion3_Anim", Cb::Pred(h::tick)),
            ("itHammerhead_UnkMotion3_Phys", Cb::Event(h::idle)),
            ("itHammerhead_UnkMotion3_Coll", Cb::Pred(h::coll3)),
            ("itHammerHead_Logic40_DmgDealt", Cb::Pred(h::no)),
            ("itHammerHead_Logic40_Clanked", Cb::Pred(h::no)),
            ("itHammerHead_Logic40_HitShield", Cb::Pred(h::no)),
            ("itHammerHead_Logic40_Reflected", Cb::Pred(h::reflected)),
            ("itHammerHead_Logic40_DmgReceived", Cb::Pred(h::no)),
        ],
    );
    // Near the end of its life, it blinks (hidden every `x44` ticks); at the end, it's gone.
    for life in [1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0] {
        let prep = move |c: &mut Call| {
            set32(c, item_ptr(c) + 0xD44, life.to_bits());
            item_kind(c) == 28
        };
        check_on(
            "itHammer_UnkMotion0_Anim",
            "itHammerhead_UnkMotion2_Anim",
            &prep,
            &|r, a| Ret::Bool(h::tick(r, a.gobj(0))),
        );
    }
    check_on(
        "itHammer_UnkMotion3_Anim",
        "itHammerHead_Logic40_EvtUnk",
        &pass_owner,
        &|r, a| {
            h::forget(r, a.gobj(0), a.opt_gobj(1));
            Ret::None
        },
    );
}

#[test]
fn green_shell() {
    use melee_sim::game::it::kinds::gshell as s;
    kind_direct(&[
        ("itGShell_Logic14_Spawned", Cb::Event(s::spawned)),
        ("itGshell_UnkMotion0_Anim", Cb::Pred(s::still)),
        ("itGshell_UnkMotion0_Phys", Cb::Event(s::idle)),
        ("itGshell_UnkMotion0_Coll", Cb::Pred(s::coll0)),
        ("itGshell_UnkMotion1_Anim", Cb::Pred(s::anim1)),
        ("itGshell_UnkMotion1_Phys", Cb::Event(s::phys_fall)),
        ("itGshell_UnkMotion1_Coll", Cb::Pred(s::coll1)),
        ("itGShell_Logic14_PickedUp", Cb::Event(s::picked_up)),
        ("itGshell_UnkMotion2_Anim", Cb::Pred(s::still)),
        ("itGshell_UnkMotion2_Phys", Cb::Event(s::idle)),
        ("itGshell_UnkMotion3_Anim", Cb::Pred(s::anim3)),
        ("itGshell_UnkMotion3_Phys", Cb::Event(s::phys_fall)),
        ("itGshell_UnkMotion3_Coll", Cb::Pred(s::coll3)),
        ("itGshell_UnkMotion6_Anim", Cb::Pred(s::anim6)),
        ("itGshell_UnkMotion6_Phys", Cb::Event(s::phys6)),
        ("itGshell_UnkMotion6_Coll", Cb::Pred(s::coll6)),
        ("itGshell_UnkMotion8_Anim", Cb::Pred(s::anim8)),
        ("itGshell_UnkMotion8_Phys", Cb::Event(s::phys_fall)),
        ("itGshell_UnkMotion8_Coll", Cb::Pred(s::coll3)),
        ("itGShell_Logic14_DmgDealt", Cb::Pred(s::dmg_dealt)),
        ("itGShell_Logic14_DmgReceived", Cb::Pred(s::dmg_received)),
        ("itGShell_Logic14_Reflected", Cb::Pred(s::reflected)),
        ("itGShell_Logic14_Clanked", Cb::Pred(s::clanked)),
        ("itGShell_Logic14_HitShield", Cb::Pred(s::hit_shield)),
        ("it_8028B8D8", Cb::Event(s::trail)),
        ("it_8028B988", Cb::Event(s::count_owner_grace)),
        ("it_8028BA2C", Cb::Event(s::landed)),
        ("it_8028BAD8", Cb::Event(s::knocked)),
        ("it_8028BC2C", Cb::Event(s::stomped)),
        ("it_8028BE54", Cb::Event(s::stopped)),
        ("it_8028C018", Cb::Event(s::fell)),
        ("it_8028C3A8", Cb::Event(s::slide)),
        ("it_8028C898", Cb::Event(s::slide_air)),
        ("it_8028CF68", Cb::Pred(s::jumped_on)),
    ]);
    // Each state's callbacks from the shells' other states.
    for source in [
        "itGshell_UnkMotion0_Anim",
        "itGshell_UnkMotion1_Anim",
        "itGshell_UnkMotion3_Anim",
        "itGshell_UnkMotion6_Anim",
        "itGshell_UnkMotion8_Anim",
    ] {
        kind_perturbed(
            source,
            14,
            &[
                ("itGShell_Logic14_Thrown", Cb::Event(s::thrown)),
                ("itGShell_Logic14_Dropped", Cb::Event(s::dropped)),
                ("itGshell_UnkMotion4_Anim", Cb::Pred(s::anim3)),
                ("itGshell_UnkMotion4_Phys", Cb::Event(s::phys_fall)),
                ("itGshell_UnkMotion4_Coll", Cb::Pred(s::coll3)),
                ("itGShell_Logic14_EnteredAir", Cb::Event(s::entered_air)),
                ("itGshell_UnkMotion9_Anim", Cb::Pred(s::still)),
                ("itGshell_UnkMotion9_Phys", Cb::Event(s::idle)),
                ("itGshell_UnkMotion9_Coll", Cb::Pred(s::coll9)),
                ("itGShell_Logic14_DmgReceived", Cb::Pred(s::dmg_received)),
                ("itGShell_Logic14_Clanked", Cb::Pred(s::clanked)),
                ("itGShell_Logic14_HitShield", Cb::Pred(s::hit_shield)),
                (
                    "itGShell_Logic14_ShieldBounced",
                    Cb::Pred(s::shield_bounced),
                ),
                ("it_8028CF68", Cb::Pred(s::jumped_on)),
                ("it_8028BC2C", Cb::Event(s::stomped)),
                ("it_8028B988", Cb::Event(s::count_owner_grace)),
            ],
        );
    }
    check_on(
        "itGshell_UnkMotion0_Anim",
        "itGShell_Logic14_EvtUnk",
        &pass_owner,
        &|r, a| {
            s::forget(r, a.gobj(0), a.opt_gobj(1));
            Ret::None
        },
    );
}

#[test]
fn red_shell() {
    use melee_sim::game::it::kinds::rshell as s;
    kind_direct(&[
        ("it_3F14_Logic15_Spawned", Cb::Event(s::spawned)),
        ("itRshell_UnkMotion0_Anim", Cb::Pred(s::still)),
        ("itRshell_UnkMotion0_Phys", Cb::Event(s::idle)),
        ("itRshell_UnkMotion0_Coll", Cb::Pred(s::coll0)),
        ("itRshell_UnkMotion1_Anim", Cb::Pred(s::still)),
        ("itRshell_UnkMotion1_Phys", Cb::Event(s::phys_fall)),
        ("itRshell_UnkMotion1_Coll", Cb::Pred(s::coll1)),
        ("itRshell_UnkMotion5_Anim", Cb::Pred(s::anim5)),
        ("itRshell_UnkMotion5_Phys", Cb::Event(s::phys5)),
        ("itRshell_UnkMotion5_Coll", Cb::Pred(s::coll5)),
        ("it_3F14_Logic15_DmgDealt", Cb::Pred(s::dmg_dealt)),
        ("it_3F14_Logic15_DmgReceived", Cb::Pred(s::dmg_received)),
        ("it_3F14_Logic15_HitShield", Cb::Pred(s::hit_shield)),
        ("it_8028CFE0", Cb::Event(s::trail)),
        ("it_8028D090", Cb::Event(s::landed)),
        ("it_8028D100", Cb::Event(s::knocked)),
        ("it_8028D26C", Cb::Event(s::stomped)),
        ("it_8028D390", Cb::Event(s::count_hit)),
        ("it_8028D3B8", Cb::Event(s::bounced)),
        ("fn_8028D4A8", Cb::Event(s::turn_back)),
        ("it_8028D4E4", Cb::Event(s::cap_speed)),
        ("it_8028D62C", Cb::Event(s::stopped)),
        ("it_8028D7F0", Cb::Event(s::fell)),
        ("it_8028DAE4", Cb::Event(s::chase)),
        ("it_8028E170", Cb::Event(s::chase_air)),
        ("it_8028E6C0", Cb::Pred(s::jumped_on)),
    ]);
    check("it_8028D56C", &|r, a| {
        Ret::Float(f64::from(s::launch_speed(
            r,
            a.gobj(0),
            a.float(0),
            a.float(1),
        )))
    });
    for source in [
        "itRshell_UnkMotion0_Anim",
        "itRshell_UnkMotion1_Anim",
        "itRshell_UnkMotion5_Anim",
    ] {
        kind_perturbed(
            source,
            15,
            &[
                ("itRShell_Logic15_PickedUp", Cb::Event(s::picked_up)),
                ("itRshell_UnkMotion2_Anim", Cb::Pred(s::still)),
                ("itRshell_UnkMotion2_Phys", Cb::Event(s::idle)),
                ("it_3F14_Logic15_Thrown", Cb::Event(s::thrown)),
                ("itRshell_UnkMotion3_Anim", Cb::Pred(s::anim3)),
                ("itRshell_UnkMotion3_Phys", Cb::Event(s::phys_fall)),
                ("itRshell_UnkMotion3_Coll", Cb::Pred(s::coll3)),
                ("it_3F14_Logic15_Dropped", Cb::Event(s::dropped)),
                ("itRshell_UnkMotion4_Anim", Cb::Pred(s::anim3)),
                ("itRshell_UnkMotion4_Phys", Cb::Event(s::phys_fall)),
                ("itRshell_UnkMotion4_Coll", Cb::Pred(s::coll3)),
                ("itRshell_UnkMotion6_Anim", Cb::Pred(s::anim6)),
                ("itRshell_UnkMotion6_Phys", Cb::Event(s::phys_fall)),
                ("itRshell_UnkMotion6_Coll", Cb::Pred(s::coll3)),
                ("it_3F14_Logic15_EnteredAir", Cb::Event(s::entered_air)),
                ("itRshell_UnkMotion7_Anim", Cb::Pred(s::still)),
                ("itRshell_UnkMotion7_Phys", Cb::Event(s::idle)),
                ("itRshell_UnkMotion7_Coll", Cb::Pred(s::coll7)),
                ("it_3F14_Logic15_DmgDealt", Cb::Pred(s::dmg_dealt)),
                ("it_3F14_Logic15_DmgReceived", Cb::Pred(s::dmg_received)),
                ("itRShell_Logic15_Reflected", Cb::Pred(s::reflected)),
                ("itRShell_Logic15_Clanked", Cb::Pred(s::clanked)),
                ("it_3F14_Logic15_HitShield", Cb::Pred(s::hit_shield)),
                (
                    "itRShell_Logic15_ShieldBounced",
                    Cb::Pred(s::shield_bounced),
                ),
                ("it_8028E6C0", Cb::Pred(s::jumped_on)),
                ("it_8028D3B8", Cb::Event(s::bounced)),
            ],
        );
    }
    // The chase's turn towards the nearest fighter, every frame instead of every `x40` (from
    // chasing shells only: a resting shell's hitboxes still hold a freed item's joints).
    let prep = |c: &mut Call| {
        set32(c, item_ptr(c) + 0xDD4 + 0x10, 0);
        item_kind(c) == 15
    };
    check_on(
        "itRshell_UnkMotion5_Anim",
        "itRshell_UnkMotion5_Phys",
        &prep,
        &|r, a| {
            s::phys5(r, a.gobj(0));
            Ret::None
        },
    );
    check_on(
        "itRshell_UnkMotion0_Anim",
        "itRShell_Logic15_EvtUnk",
        &pass_owner,
        &|r, a| {
            s::forget(r, a.gobj(0), a.opt_gobj(1));
            Ret::None
        },
    );
}

#[test]
fn bob_omb() {
    use melee_sim::game::it::kinds::bombhei as b;
    kind_direct(&[
        ("itBombhei_Logic6_Spawned", Cb::Event(b::spawned)),
        ("itBombhei_UnkMotion0_Anim", Cb::Pred(b::anim0)),
        ("itBombhei_UnkMotion0_Phys", Cb::Event(b::idle)),
        ("itBombhei_UnkMotion0_Coll", Cb::Pred(b::coll0)),
        ("itBombhei_UnkMotion1_Anim", Cb::Pred(b::still)),
        ("itBombhei_UnkMotion1_Phys", Cb::Event(b::phys1)),
        ("itBombhei_UnkMotion1_Coll", Cb::Pred(b::coll1)),
        ("itBombhei_Logic6_PickedUp", Cb::Event(b::picked_up)),
        ("itBombhei_UnkMotion8_Anim", Cb::Pred(b::anim8)),
        ("itBombhei_UnkMotion8_Phys", Cb::Event(b::idle)),
        ("itBombhei_UnkMotion3_Anim", Cb::Pred(b::anim3)),
        ("itBombhei_UnkMotion3_Phys", Cb::Event(b::idle)),
        ("itBombhei_UnkMotion3_Coll", Cb::Pred(b::coll3)),
        ("itBombhei_UnkMotion2_Anim", Cb::Pred(b::anim2)),
        ("itBombhei_UnkMotion2_Phys", Cb::Event(b::idle)),
        ("itBombhei_UnkMotion2_Coll", Cb::Pred(b::coll2)),
        ("itBombhei_UnkMotion10_Anim", Cb::Pred(b::anim10)),
        ("itBombhei_UnkMotion10_Phys", Cb::Event(b::phys_spin)),
        ("itBombhei_UnkMotion10_Coll", Cb::Pred(b::coll10)),
        ("itBombhei_UnkMotion11_Anim", Cb::Pred(b::anim11)),
        ("itBombhei_UnkMotion11_Phys", Cb::Event(b::idle)),
        ("itBombhei_UnkMotion11_Coll", Cb::Pred(b::coll11)),
    ]);
    // Every state's callbacks from the bob-ombs walking, lying, turning and flying.
    for source in [
        "itBombhei_UnkMotion0_Anim",
        "itBombhei_UnkMotion1_Anim",
        "itBombhei_UnkMotion2_Anim",
        "itBombhei_UnkMotion3_Anim",
        "itBombhei_UnkMotion10_Anim",
    ] {
        kind_perturbed(
            source,
            6,
            &[
                ("it_8027D820", Cb::Event(b::fuse)),
                ("fn_8027DAC8", Cb::Event(b::landed)),
                ("it_8027DE18", Cb::Event(b::fell)),
                ("it_3F14_Logic6_Dropped", Cb::Event(b::dropped)),
                ("it_8027E978", Cb::Event(b::stand_up)),
                ("it_8027EE04", Cb::Event(b::walk)),
                ("it_8027F42C", Cb::Event(b::turn)),
                ("itBombhei_UnkMotion4_Anim", Cb::Pred(b::anim4)),
                ("itBombhei_UnkMotion4_Phys", Cb::Event(b::idle)),
                ("itBombhei_UnkMotion4_Coll", Cb::Pred(b::coll3)),
                ("it_8027F8E0", Cb::Event(b::light)),
                ("itBombhei_UnkMotion5_Anim", Cb::Pred(b::anim5)),
                ("itBombhei_UnkMotion5_Phys", Cb::Event(b::idle)),
                ("itBombhei_UnkMotion5_Coll", Cb::Pred(b::coll5)),
                ("fn_8027FCA8", Cb::Event(b::light_air)),
                ("itBombhei_UnkMotion6_Anim", Cb::Pred(b::anim6)),
                ("itBombhei_UnkMotion6_Phys", Cb::Event(b::phys_spin)),
                ("fn_8028007C", Cb::Event(b::landed_lit)),
                ("itBombhei_UnkMotion6_Coll", Cb::Pred(b::coll6)),
                ("it_3F14_Logic6_Thrown", Cb::Event(b::thrown)),
                ("fn_80280974", Cb::Event(b::landed_thrown)),
                ("it_80280DC0", Cb::Event(b::explode_once)),
                ("it_8027D730", Cb::Event(b::detonate)),
                ("it_3F14_Logic6_DmgDealt", Cb::Pred(b::dmg)),
                ("it_3F14_Logic6_DmgReceived", Cb::Pred(b::dmg)),
                ("it_3F14_Logic6_EnteredAir", Cb::Event(b::entered_air)),
                ("itBombhei_UnkMotion12_Anim", Cb::Pred(b::still)),
                ("itBombhei_UnkMotion12_Phys", Cb::Event(b::idle)),
                ("itBombhei_UnkMotion12_Coll", Cb::Pred(b::coll12)),
                ("itBombhei_Logic6_Clanked", Cb::Pred(b::clanked)),
                ("itBombhei_Logic6_Reflected", Cb::Pred(b::reflected)),
                ("it_3F14_Logic6_HitShield", Cb::Pred(b::hit_shield)),
                ("it_3F14_Logic6_ShieldBounced", Cb::Pred(b::hit_shield)),
            ],
        );
    }
    // A lit bob-omb's fuse at its last frames, and the hard landings that set it off.
    for fuse in [1.0f32, 2.0, 30.0] {
        let prep = move |c: &mut Call| {
            set32(c, item_ptr(c) + 0xDD4 + 0x1C, fuse.to_bits());
            set32(c, item_ptr(c) + 0xDD4 + 0xC, 1);
            item_kind(c) == 6
        };
        check_on(
            "itBombhei_UnkMotion2_Anim",
            "it_8027D820",
            &prep,
            &|r, a| {
                b::fuse(r, a.gobj(0));
                Ret::None
            },
        );
    }
    // Thrown at another size.
    let prep = |c: &mut Call| {
        set32(c, item_ptr(c) + 0xDD4 + 0x14, 1.5f32.to_bits());
        item_kind(c) == 6
    };
    check_on(
        "itBombhei_UnkMotion2_Anim",
        "it_3F14_Logic6_Thrown",
        &prep,
        &|r, a| {
            b::thrown(r, a.gobj(0));
            Ret::None
        },
    );
    // A bob-omb dropped lit at a point (as a stage or event drops one).
    let prep = |c: &mut Call| {
        vec_arg(c, 3, 0x3000, [10.0, 30.0, 0.0]);
        true
    };
    check_on(
        "itBombhei_UnkMotion2_Anim",
        "it_8027D670",
        &prep,
        &|r, a| Ret::GObj(b::spawn_at(r, a.vec(0))),
    );
    check_on(
        "itBombhei_UnkMotion2_Anim",
        "itBombhei_Logic6_EvtUnk",
        &pass_owner,
        &|r, a| {
            b::forget(r, a.gobj(0), a.opt_gobj(1));
            Ret::None
        },
    );
}

#[test]
fn shell_paths() {
    use melee_sim::game::it::kinds::{gshell as g, rshell as s};
    // The trail's countdown between puffs.
    for (source, target, kind, off) in [
        ("itGshell_UnkMotion6_Anim", "it_8028B8D8", 14, 0x8),
        ("itRshell_UnkMotion5_Anim", "it_8028CFE0", 15, 0x8),
    ] {
        let prep = move |c: &mut Call| {
            set32(c, item_ptr(c) + 0xDD4 + off, 5.0f32.to_bits());
            item_kind(c) == kind
        };
        let ours: &Ours = if kind == 14 {
            &|r, a| {
                g::trail(r, a.gobj(0));
                Ret::None
            }
        } else {
            &|r, a| {
                s::trail(r, a.gobj(0));
                Ret::None
            }
        };
        check_on(source, target, &prep, ours);
    }
    // A red shell landing from a throw, and taking to the air chasing.
    let prep = |c: &mut Call| item_kind(c) == 15;
    check_on("itRshell_UnkMotion5_Coll", "it_8028D090", &prep, &|r, a| {
        s::landed(r, a.gobj(0));
        Ret::None
    });
    check_on("itRshell_UnkMotion5_Coll", "it_8028E170", &prep, &|r, a| {
        s::chase_air(r, a.gobj(0));
        Ret::None
    });
}

#[test]
fn effect_router() {
    use melee_sim::game::it::effect;
    check("it_802787B4", &|r, a| {
        effect::spawn_simple(r, a.gobj(0), a.int(1) as i32);
        Ret::None
    });
    let ours: &Ours = &|r, a| {
        let (id, bone) = (a.int(1) as i32, a.int(2) as i32);
        effect::spawn(r, a.gobj(0), id, bone, a.vec(3), a.vec(4), a.int(5) == 1);
        Ret::None
    };
    check("it_80278800", ours);
    // One id of each of the router's ways, now and queued.
    for id in [
        0x402u32, 0x446, 0x417, 0x415, 0x44E, 0x423, 0x3E9, 0x3ED, 0x3F7, 0x435, 0x404, 0x513,
        0x514, 0x515, 0x42B, 0x420,
    ] {
        for sync in [0u32, 1] {
            let prep = move |c: &mut Call| {
                c.regs.gpr[4] = id;
                c.regs.gpr[8] = sync;
                true
            };
            check_calls("it_80278800", "it_80278800", &prep, ours);
        }
    }
}

#[test]
fn crate_box() {
    use melee_sim::game::it::kinds::crate_box as b;
    kind_direct(&[
        ("itBox_Logic1_Spawned", Cb::Event(b::spawned)),
        ("itBox_Logic1_Destroyed", Cb::Event(b::destroyed)),
        ("itBox_UnkMotion0_Anim", Cb::Pred(b::anim0)),
        ("itBox_UnkMotion0_Phys", Cb::Event(b::idle)),
        ("itBox_UnkMotion0_Coll", Cb::Pred(b::coll0)),
        ("itBox_UnkMotion4_Anim", Cb::Pred(b::no)),
        ("itBox_UnkMotion1_Phys", Cb::Event(b::phys1)),
        ("itBox_UnkMotion1_Coll", Cb::Pred(b::coll1)),
        ("itBox_UnkMotion7_Anim", Cb::Pred(b::anim7)),
        ("itBox_UnkMotion7_Phys", Cb::Event(b::idle)),
        ("itBox_UnkMotion7_Coll", Cb::Pred(b::no)),
        ("itBox_Logic1_DmgReceived", Cb::Pred(b::dmg_received)),
    ]);
    for source in ["itBox_UnkMotion0_Anim", "itBox_UnkMotion4_Anim"] {
        kind_perturbed(
            source,
            1,
            &[
                ("fn_80286480", Cb::Event(b::landed)),
                ("it_8028655C", Cb::Event(b::fell)),
                ("itBox_Logic1_PickedUp", Cb::Event(b::picked_up)),
                ("itBox_UnkMotion2_Anim", Cb::Pred(b::no)),
                ("itBox_UnkMotion2_Phys", Cb::Event(b::idle)),
                ("itBox_Logic1_Thrown", Cb::Event(b::thrown)),
                ("itBox_UnkMotion4_Phys", Cb::Event(b::phys4)),
                ("itBox_UnkMotion3_Coll", Cb::Pred(b::coll3)),
                ("itBox_Logic1_Dropped", Cb::Event(b::dropped)),
                ("itBox_UnkMotion4_Coll", Cb::Pred(b::coll4)),
                ("it_80286AA4", Cb::Event(b::open_empty)),
                ("itBox_UnkMotion6_Anim", Cb::Pred(b::anim6)),
                ("itBox_UnkMotion6_Phys", Cb::Event(b::idle)),
                ("itBox_UnkMotion6_Coll", Cb::Pred(b::no)),
                ("it_80286BA0", Cb::Event(b::open)),
                ("itBox_Logic1_DmgDealt", Cb::Pred(b::hit)),
                ("itBox_Logic1_Clanked", Cb::Pred(b::hit)),
                ("itBox_Logic1_HitShield", Cb::Pred(b::hit)),
                ("itBox_Logic1_Reflected", Cb::Pred(b::hit)),
                ("itBox_Logic1_EnteredAir", Cb::Event(b::entered_air)),
                ("itBox_UnkMotion5_Anim", Cb::Pred(b::no)),
                ("itBox_UnkMotion5_Phys", Cb::Event(b::idle)),
                ("itBox_UnkMotion5_Coll", Cb::Pred(b::coll5)),
                ("it_802870A4", Cb::Event(b::float)),
                ("itBox_UnkMotion8_Anim", Cb::Pred(b::no)),
                ("itBox_UnkMotion8_Phys", Cb::Event(b::phys8)),
                ("itBox_UnkMotion8_Coll", Cb::Pred(b::no)),
                ("it_8028733C", Cb::Event(b::follow)),
                ("it_802863BC", Cb::Pred(b::lands_upright)),
            ],
        );
    }
    // Enough damage taken to break it.
    let prep = |c: &mut Call| {
        set32(c, item_ptr(c) + 0xC9C, 500);
        item_kind(c) == 1
    };
    check_on(
        "itBox_UnkMotion0_Anim",
        "itBox_Logic1_DmgReceived",
        &prep,
        &|r, a| Ret::Bool(b::dmg_received(r, a.gobj(0))),
    );
    // The weighted draws, from the boxes' states (capsules the only kind to drop).
    for seed in seeds() {
        let prep = move |c: &mut Call| {
            only_capsules(c, seed);
            c.regs.gpr[4..8].copy_from_slice(&[3, 2, 1, 4]);
            item_kind(c) == 1
        };
        check_on("itBox_UnkMotion0_Anim", "it_80286340", &prep, &|r, a| {
            Ret::Bool(b::has_items(
                r,
                a.int(1) as i32,
                a.int(2) as i32,
                a.int(3) as i32,
                a.int(4) as i32,
            ))
        });
        check_on("itBox_UnkMotion0_Anim", "it_80286248", &prep, &|r, a| {
            b::drop_items(
                r,
                a.gobj(0),
                a.int(1) as i32,
                a.int(2) as i32,
                a.int(3) as i32,
                a.int(4) as i32,
            );
            Ret::None
        });
    }
    // A box floating with a barrel (here, the box itself).
    let prep = |c: &mut Call| item_kind(c) == 1;
    check_on("itBox_UnkMotion0_Anim", "it_80286088", &prep, &|r, a| {
        Ret::GObj(b::spawn_floating(r, a.opt_gobj(0)))
    });
    check_on(
        "itBox_UnkMotion0_Anim",
        "itBox_Logic1_EvtUnk",
        &pass_owner,
        &|r, a| {
            b::forget(r, a.gobj(0), a.opt_gobj(1));
            Ret::None
        },
    );
}

#[test]
fn barrel() {
    use melee_sim::game::it::kinds::taru as t;
    kind_direct(&[
        ("it_3F14_Logic2_Spawned", Cb::Event(t::spawned)),
        ("itTaru_UnkMotion0_Anim", Cb::Pred(t::anim0)),
        ("itTaru_UnkMotion0_Phys", Cb::Event(t::idle)),
        ("itTaru_UnkMotion0_Coll", Cb::Pred(t::coll0)),
        ("itTaru_UnkMotion1_Anim", Cb::Pred(t::no)),
        ("itTaru_UnkMotion1_Phys", Cb::Event(t::phys1)),
        ("itTaru_UnkMotion1_Coll", Cb::Pred(t::coll1)),
    ]);
    for source in ["itTaru_UnkMotion0_Anim", "itTaru_UnkMotion1_Anim"] {
        kind_perturbed(
            source,
            2,
            &[
                ("it_802874F0", Cb::Event(t::roll)),
                ("it_80287690", Cb::Event(t::turn)),
                ("it_80287D0C", Cb::Event(t::fell)),
                ("itTaru_Logic2_PickedUp", Cb::Event(t::picked_up)),
                ("itTaru_UnkMotion2_Anim", Cb::Pred(t::no)),
                ("itTaru_UnkMotion2_Phys", Cb::Event(t::idle)),
                ("itTaru_Logic2_Dropped", Cb::Event(t::let_go)),
                ("itTaru_Logic2_Thrown", Cb::Event(t::thrown)),
                ("it_80287F20", Cb::Event(t::let_go)),
                ("itTaru_UnkMotion3_Anim", Cb::Pred(t::tick)),
                ("itTaru_UnkMotion3_Phys", Cb::Event(t::phys3)),
                ("itTaru_UnkMotion3_Coll", Cb::Pred(t::coll3)),
                ("it_80288194", Cb::Event(t::rolling)),
                ("it_802881B4", Cb::Event(t::land_rolling)),
                ("it_802881FC", Cb::Event(t::rolling)),
                ("itTaru_UnkMotion5_Anim", Cb::Pred(t::anim5)),
                ("itTaru_UnkMotion5_Phys", Cb::Event(t::phys5)),
                ("itTaru_UnkMotion5_Coll", Cb::Pred(t::coll5)),
                ("itTaru_UnkMotion4_Anim", Cb::Pred(t::tick)),
                ("itTaru_UnkMotion4_Phys", Cb::Event(t::phys4)),
                ("itTaru_UnkMotion4_Coll", Cb::Pred(t::coll5)),
                ("it_802885C8", Cb::Event(t::open_empty)),
                ("itTaru_UnkMotion6_Anim", Cb::Pred(t::anim6)),
                ("itTaru_UnkMotion6_Phys", Cb::Event(t::idle)),
                ("itTaru_UnkMotion6_Coll", Cb::Pred(t::no)),
                ("it_802886C4", Cb::Event(t::open)),
                ("itTaru_UnkMotion7_Anim", Cb::Pred(t::anim7)),
                ("itTaru_UnkMotion7_Phys", Cb::Event(t::idle)),
                ("itTaru_UnkMotion7_Coll", Cb::Pred(t::no)),
                ("it_3F14_Logic2_DmgDealt", Cb::Pred(t::hit)),
                ("it_3F14_Logic2_Clanked", Cb::Pred(t::hit)),
                ("it_3F14_Logic2_HitShield", Cb::Pred(t::hit)),
                ("it_3F14_Logic2_Reflected", Cb::Pred(t::hit)),
                ("it_3F14_Logic2_DmgReceived", Cb::Pred(t::dmg_received)),
            ],
        );
    }
    // Rolling and carried barrels, their speeds and tilts set (the `xDB0` flag a carried
    // barrel straightens with).
    for (speed, carried, slope) in [(0.5f32, 0u32, 0.3f32), (-2.0, 1, 0.0), (9.0, 1, -0.5)] {
        let prep = move |c: &mut Call| {
            let ip = item_ptr(c);
            set32(c, ip + 0xDD4 + 0x10, speed.to_bits());
            set32(c, ip + 0xDD4 + 0x14, slope.to_bits());
            set32(c, ip + 0xDB0, carried);
            item_kind(c) == 2
        };
        for (target, f) in [
            ("it_802874F0", t::roll as fn(&mut Running, GObjId)),
            ("it_80287690", t::turn),
            ("itTaru_UnkMotion4_Phys", t::phys4),
        ] {
            check_on("itTaru_UnkMotion0_Anim", target, &prep, &move |r, a| {
                f(r, a.gobj(0));
                Ret::None
            });
        }
        check_on(
            "itTaru_UnkMotion0_Anim",
            "itTaru_UnkMotion5_Coll",
            &prep,
            &|r, a| Ret::Bool(t::coll5(r, a.gobj(0))),
        );
    }
    // Hits short of breaking it: a lying barrel knocked rolling (on a seed whose draw hits),
    // a rolling one pushed.
    for (msid, seed) in [(0u32, 0u32), (0, 1), (0, 2), (4, 0), (5, 0)] {
        let prep = move |c: &mut Call| {
            let ip = item_ptr(c);
            set32(c, ip + 0x24, msid);
            set32(c, ip + 0xC9C, 0);
            set32(c, ip + 0xCA0, 12);
            set_seed(c, seeds().nth(seed as usize).unwrap());
            item_kind(c) == 2
        };
        check_on(
            "itTaru_UnkMotion0_Anim",
            "it_3F14_Logic2_DmgReceived",
            &prep,
            &|r, a| Ret::Bool(t::dmg_received(r, a.gobj(0))),
        );
    }
    check_on(
        "itTaru_UnkMotion0_Anim",
        "itTaru_Logic2_EvtUnk",
        &pass_owner,
        &|r, a| {
            t::forget(r, a.gobj(0), a.opt_gobj(1));
            Ret::None
        },
    );
}

#[test]
fn script_commands() {
    use melee_sim::game::it::anim;
    // Each of the items' own commands run at the script of an exploding item (the capsules'
    // and bob-ombs' blasts have scripts), the command info in r4.
    let prep = |c: &mut Call| {
        let ip = item_ptr(c);
        c.regs.gpr[4] = ip + 0x524;
        w32(c, ip + 0x52C) != 0
    };
    for source in ["itCapsule_UnkMotion5_Anim", "itBombhei_UnkMotion11_Anim"] {
        check_on(source, "it_80278F2C", &prep, &|r, a| {
            anim::script_effect(r, a.gobj(0));
            Ret::None
        });
        for (name, op) in [
            ("it_80279888", 21),
            ("it_802798D4", 22),
            ("it_8027990C", 23),
            ("it_80279958", 24),
        ] {
            check_on(source, name, &prep, &move |r, a| {
                anim::run_command(r, a.gobj(0), op);
                Ret::None
            });
        }
    }
}

#[test]
fn winds() {
    use melee_sim::game::gr::ground;
    // The wind sources age every stage tick; exploding bob-ombs' states have some blowing.
    let tick: &Ours = &|r, _| {
        ground::quake_list_tick(r.game);
        Ret::None
    };
    check_on("itBombhei_UnkMotion11_Anim", "lb_800115F4", &|_| true, tick);
}

#[test]
fn party_ball() {
    use melee_sim::game::it::kinds::kusudama as k;
    kind_direct(&[
        ("itKusudama_Logic4_Spawned", Cb::Event(k::spawned)),
        ("itKusudama_UnkMotion0_Anim", Cb::Pred(k::upright)),
        ("itKusudama_UnkMotion0_Phys", Cb::Event(k::idle)),
        ("itKusudama_UnkMotion0_Coll", Cb::Pred(k::coll0)),
        ("itKusudama_UnkMotion1_Anim", Cb::Pred(k::anim1)),
        ("itKusudama_UnkMotion1_Phys", Cb::Event(k::idle)),
        ("itKusudama_UnkMotion1_Coll", Cb::Pred(k::coll1)),
        ("itKusudama_UnkMotion2_Anim", Cb::Pred(k::upright)),
        ("itKusudama_UnkMotion2_Phys", Cb::Event(k::phys2)),
        ("itKusudama_UnkMotion2_Coll", Cb::Pred(k::coll2)),
        ("itKusudama_UnkMotion3_Anim", Cb::Pred(k::anim3)),
        ("itKusudama_UnkMotion3_Phys", Cb::Event(k::idle)),
        ("itKusudama_UnkMotion3_Coll", Cb::Pred(k::coll3)),
        ("itKusudama_Logic4_PickedUp", Cb::Event(k::picked_up)),
        ("itKusudama_UnkMotion4_Anim", Cb::Pred(k::no)),
        ("itKusudama_UnkMotion4_Phys", Cb::Event(k::idle)),
        ("itKusudama_Logic4_Thrown", Cb::Event(k::thrown)),
        ("itKusudama_UnkMotion8_Anim", Cb::Pred(k::anim8)),
        ("itKusudama_UnkMotion8_Phys", Cb::Event(k::idle)),
        ("itKusudama_UnkMotion8_Coll", Cb::Pred(k::no)),
        ("itKusudama_Logic4_DmgReceived", Cb::Pred(k::dmg_received)),
    ]);
    for source in [
        "itKusudama_UnkMotion0_Anim",
        "itKusudama_UnkMotion2_Anim",
        "itKusudama_UnkMotion3_Anim",
    ] {
        kind_perturbed(
            source,
            4,
            &[
                (
                    "it_80289B50",
                    Cb::Event(|r, g| k::open_or_burst(r, g, true)),
                ),
                ("it_8028A190", Cb::Event(k::stopped)),
                ("it_8028A3CC", Cb::Event(k::fell)),
                ("it_8028A544", Cb::Event(k::rise)),
                ("itKusudama_UnkMotion6_Anim", Cb::Pred(k::no)),
                ("itKusudama_UnkMotion6_Phys", Cb::Event(k::phys6)),
                ("itKusudama_UnkMotion5_Coll", Cb::Pred(k::coll5)),
                ("itKusudama_Logic4_Dropped", Cb::Event(k::dropped)),
                ("itKusudama_UnkMotion6_Coll", Cb::Pred(k::coll6)),
                ("it_8028AC74", Cb::Event(k::burst)),
                ("itKusudama_UnkMotion7_Anim", Cb::Pred(k::anim7)),
                ("itKusudama_UnkMotion7_Phys", Cb::Event(k::idle)),
                ("itKusudama_UnkMotion7_Coll", Cb::Pred(k::no)),
                ("it_8028AD44", Cb::Event(k::open)),
                ("itKusudama_Logic4_DmgDealt", Cb::Pred(k::dmg_dealt)),
                ("itKusudama_Logic4_Clanked", Cb::Pred(k::clanked)),
                ("itKusudama_Logic4_HitShield", Cb::Pred(k::clanked)),
                ("itKusudama_Logic4_Reflected", Cb::Pred(k::clanked)),
                ("it_80289A00", Cb::Event(k::face_front)),
            ],
        );
    }
    for angle in [0.0f32, 1.5707964, 4.0] {
        let prep = move |c: &mut Call| {
            set_f(c, 1, angle);
            item_kind(c) == 4
        };
        check_on(
            "itKusudama_UnkMotion0_Anim",
            "it_802897C8",
            &prep,
            &|r, a| {
                k::tip_towards(r, a.gobj(0), a.float(0));
                Ret::None
            },
        );
        check_on(
            "itKusudama_UnkMotion0_Anim",
            "it_80289910",
            &prep,
            &|r, a| {
                k::tip_to(r, a.gobj(0), a.float(0));
                Ret::None
            },
        );
    }
    // The rain from each weight (capsules the only kind to drop, no monsters), unowned.
    for (seed, w) in seeds().zip([
        [1u32, 0, 0],
        [0, 1, 0],
        [0, 0, 1],
        [1, 1, 1],
        [5, 2, 3],
        [0, 0, 7],
    ]) {
        let prep = move |c: &mut Call| {
            only_capsules(c, seed);
            set32(c, leaf_check::sym("Item_804A0C64") + 0x30, 0);
            set32(c, item_ptr(c) + 0x518, 0);
            c.regs.gpr[4..7].copy_from_slice(&w);
            c.regs.gpr[7] = 2;
            item_kind(c) == 4
        };
        check_on(
            "itKusudama_UnkMotion0_Anim",
            "it_80289BE8",
            &prep,
            &|r, a| {
                k::rain(
                    r,
                    a.gobj(0),
                    a.int(1) as i32,
                    a.int(2) as i32,
                    a.int(3) as i32,
                );
                Ret::None
            },
        );
        check_on(
            "itKusudama_UnkMotion0_Anim",
            "it_8028A114",
            &prep,
            &|r, a| {
                Ret::Bool(k::has_items(
                    r,
                    a.int(1) as i32,
                    a.int(2) as i32,
                    a.int(3) as i32,
                    2,
                ))
            },
        );
    }
    check_on(
        "itKusudama_UnkMotion0_Anim",
        "itKusudama_Logic4_EvtUnk",
        &pass_owner,
        &|r, a| {
            k::forget(r, a.gobj(0), a.opt_gobj(1));
            Ret::None
        },
    );
}

#[test]
fn one_player_level() {
    // Outside the single-player modes, from the items' VS states.
    check_on(
        "itKusudama_UnkMotion0_Anim",
        "gm_8017E068",
        &|_| true,
        &|r, _| Ret::Int(item::one_player_cpu_level(r) as u32),
    );
}
