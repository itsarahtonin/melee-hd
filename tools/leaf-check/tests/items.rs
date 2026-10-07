//! The items (`melee/it`) against ssbm-rs from real game states: each test imports every
//! capture of one function, runs ssbm-rs's side and ours from the same state, in both float
//! modes, and compares the whole state afterwards (the scheduler, every joint tree, fighters,
//! items, the stage and the camera) and what the call returned.
//!
//! The captures are game memory, kept out of the repository. Regenerate them with ssbm-rs's
//! runner (branch `feat/state-trace`), from `C:/Projects/ssbm-rs-state-trace`, with
//! `items-funcs.txt` listing the item units' functions by address (item.c through it_279C.c,
//! itfoxlaser.c and itfoxblaster.c: `ADDR NAME` per line, from the decomp's symbols.txt) and
//! `done-nonitems.txt` every other function's address:
//!
//! ```text
//! cargo build --release -p ssbm-run
//! CAPTURE=$ITEM_CALLS/fox-fd CAPTURE_NESTED=1 CAPTURE_FUNCS=@items-funcs.txt \
//!   LOCKSTEP_DONE=done-nonitems.txt target/release/ssbm-run "$SSBM_DISC" \
//!   --replay Game_20260129T133652.slp --fields 1500 --port all --lockstep \
//!   --known local/replays/known/Game_20260129T133652.txt
//! ```
//!
//! (Fox against Sheik on Final Destination; the mutated and mismatching saves aren't used and
//! can be deleted.) `ITEM_CALLS` names the directory (default below); tests skip when it has no
//! capture of their function.

use std::path::PathBuf;
use std::sync::{Condvar, Mutex};

use gekko_fp::{FpMode, set_fp_mode};
use leaf_check::{MODES, reference};
use melee_capture::compare::Comparer;
use melee_capture::import::{Importer, Maps};
use melee_capture::{Capture, DataMap, DiscIndex, MEM1, disc_index, find_archives_in};
use melee_data::archive::{Archives, DataRef};
use melee_data::disc::Disc;
use melee_sim::game::handles::ItemId;
use melee_sim::game::mirror::{Cx, Mem, Mirror};
use melee_sim::game::state::{Game, Running};
use melee_sim::game::structs::{Item, SpawnItem};
use melee_sim::hsd::gobj::GObjId;
use melee_sim::hsd::jobj::JObjId;
use melee_sim::vec::Vec3;
use ssbm_rt::Ctx;

const DEFAULT_CALLS: &str = "C:/Projects/scratch/items/calls";

fn calls_dir() -> PathBuf {
    std::env::var("ITEM_CALLS")
        .unwrap_or_else(|_| DEFAULT_CALLS.into())
        .into()
}

/// Every capture of `func` under the calls directory, in name order.
fn captures(func: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(dirs) = std::fs::read_dir(calls_dir()) else {
        return out;
    };
    for d in dirs.flatten() {
        let Ok(files) = std::fs::read_dir(d.path()) else {
            continue;
        };
        for f in files.flatten() {
            let name = f.file_name().to_string_lossy().into_owned();
            if name.starts_with(&format!("{func}-call")) && name.ends_with(".call") {
                out.push(f.path());
            }
        }
    }
    out.sort();
    out
}

struct Env {
    ctx: Ctx,
    index: DiscIndex,
}

fn env() -> Option<Env> {
    let ctx = reference()?;
    let disc = Disc::open(std::env::var("SSBM_DISC").unwrap()).unwrap();
    Some(Env {
        ctx,
        index: disc_index(&disc),
    })
}

/// A turn among the checks running at once (each holds a few captured states).
struct Turn;

static TURNS: (Mutex<usize>, Condvar) = (Mutex::new(0), Condvar::new());

impl Turn {
    const MAX: usize = 3;

    fn take() -> Turn {
        let (count, freed) = &TURNS;
        let mut n = count.lock().unwrap_or_else(|e| e.into_inner());
        while *n >= Self::MAX {
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
        f32::from_bits(gekko_fp::stfs(self.cap.call.regs.fpr[1 + i].ps0))
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
    'captures: for path in &files {
        let mut cap = Capture::read(path);
        if !prep(&mut cap.call) {
            skipped += 1;
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        for mode in MODES {
            set_fp_mode(mode);
            let mut archives = Archives::default();
            let data = DataMap {
                loaded: find_archives_in(&env.index, &cap.call.mem1, &mut archives),
            };
            let mut imp = Importer::new(&cap, &data);
            imp.world();
            imp.stage();
            imp.ids();
            imp.items();
            let Importer { mut game, maps, .. } = imp;
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

use melee_sim::game::it::kinds::{foxblaster, foxlaser};
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
        item::play_sfx(r.game, g, a.int(1) as i32, a.int(2) as u8, a.int(3) as u8);
        Ret::None
    });
}

#[test]
fn stop_sfx2() {
    check("Item_8026B074", &|r, a| {
        let g = a.item_gobj(0);
        item::stop_sfx2(r.game, g);
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

// itgroundcoll.c

#[test]
fn stage_crossing() {
    check("it_8026E9A4", &|r, a| {
        assert_eq!(a.int(3), 0, "a normal out parameter");
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
    check_game("it_80273168", lib2725::pickup_sfx);
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
        lib2725::play_sfx(r.game, g, a.int(1) as i32);
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
        let mult = gekko_fp::lfs(a.float(0).to_bits());
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
            foxblaster::set_visible(r.game, a.gobj(0), a.int(1) as i32);
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
    check_game("it_802AE538", foxblaster::start_pulse);
}

#[test]
fn blaster_end_pulse() {
    check_game("it_802AE608", foxblaster::end_pulse);
}

#[test]
fn blaster_picked_up() {
    check_proc("itFoxBlaster_Logic96_PickedUp", foxblaster::picked_up);
}

#[test]
fn blaster_remove() {
    check("it_802AEAB4", &|r, a| {
        foxblaster::remove(r, a.opt_gobj(0));
        Ret::None
    });
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

/// The silent sound and the slot-stopping one: the item sound functions' paths that play
/// nothing (playing waits for the audio driver's port).
const SFX_SILENT: u32 = 0x83D60;
const SFX_STOP: u32 = 0x83D61;

#[test]
fn play_sfx_silent() {
    check_on(TICKS, "Item_8026AE84", &sfx_args(SFX_SILENT), &|r, a| {
        item::play_sfx(r.game, a.item_gobj(0), a.int(1) as i32, 0x7F, 0x40);
        Ret::None
    });
}

#[test]
fn play_sfx1() {
    for sfx in [SFX_SILENT, SFX_STOP] {
        check_on(TICKS, "Item_8026AF0C", &sfx_args(sfx), &|r, a| {
            item::play_sfx1(r.game, a.item_gobj(0), a.int(1) as i32, 0x7F, 0x40);
            Ret::None
        });
    }
}

#[test]
fn play_sfx2() {
    for sfx in [SFX_SILENT, SFX_STOP] {
        check_on(TICKS, "Item_8026AFA0", &sfx_args(sfx), &|r, a| {
            item::play_sfx2(r.game, a.item_gobj(0), a.int(1) as i32, 0x7F, 0x40);
            Ret::None
        });
    }
}

#[test]
fn stop_sfx1() {
    check_on(TICKS, "Item_8026B034", &pass_item, &|r, a| {
        item::stop_sfx1(r.game, a.item_gobj(0));
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
    // Bouncing off a shield in the air (hitting one destroys a shot, which waits for the
    // audio driver's port).
    let bounce = |c: &mut Call| {
        let ip = item_ptr(c);
        set32(c, ip + 0xC0, 1);
        or8(c, ip + 0xDCE, 0x0C);
        lasers(c)
    };
    check_on(TICKS, "Item_80269DC8", &bounce, &|r, a| {
        Ret::Bool(item::react_shield(r, a.gobj(0)))
    });
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
fn clear_overlay2() {
    for target in ["it_80279BBC"] {
        check_on(TICKS, target, &pass_item, &|r, a| {
            anim::clear_overlay(&mut r.game.items[a.item(0).0]);
            Ret::None
        });
    }
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
