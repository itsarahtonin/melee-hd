//! melee-sim's GObj scheduler against ssbm-rs's ports of HSD's, run side by side on random
//! sequences of creates, frees, process changes, reprioritizations, render links and
//! `run_procs`, including processes that change the scheduler while they run. After every
//! step the two must hold the same lists in the same order and have run the same processes.

use std::cell::RefCell;
use std::collections::HashMap;

use leaf_check::{handle, reference, sym};
use melee_sim::hsd::gobj::{self, GObjId, Host, Kinds, Place, ProcId, World};
use ssbm_rt::{Addr, Ctx, FnPtr, Handle};
use ssbm_types::fns;
use ssbm_types::records::{_HSD_ObjAllocData, HSD_GObj, HSD_GObjProc};

/// Fake code addresses for the test's processes and render callbacks.
const FUNC_BASE: u32 = 0x817E_0000;
const FUNCS: u32 = 4;
const RENDER_BASE: u32 = 0x817E_0100;
const USER_REMOVE: u32 = 0x817E_0200;
const OBJ_REMOVE: u32 = 0x817E_0300;
/// Scratch for the scheduler's tables and the object heap.
const TABLES: u32 = 0x8140_0000;
const OBJ_HEAP: u32 = 0x8150_0000;
const P_LINKS_USED: u8 = 6;
const S_LINKS_USED: u8 = 5;

#[derive(Clone, Debug)]
struct T;
impl Kinds for T {
    type Proc = u32;
    type Render = u32;
    type UserData = u32;
    type Obj = u32;
}

/// What a process does while it runs, decided on ssbm-rs's side and replayed on ours.
#[derive(Clone, Copy, Debug)]
enum Action {
    Nothing,
    FreeSelf,
    RemoveSelfProc,
    ChangeSelf {
        how: u32,
        p_link: u8,
        priority: u8,
    },
    CreateWithProc {
        classifier: u16,
        p_link: u8,
        priority: u8,
        func: u32,
        s_link: u8,
    },
    FreeOther(GObjId),
    ChangeOther {
        g: GObjId,
        how: u32,
        p_link: u8,
        priority: u8,
    },
    RemoveOtherProc(ProcId),
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// Identity between ssbm-rs's addresses and our slots.
#[derive(Default)]
struct Ids {
    gobj_of: HashMap<u32, u32>,
    gobj_addr: HashMap<u32, u32>,
    proc_of: HashMap<u32, u32>,
    proc_addr: HashMap<u32, u32>,
}

impl Ids {
    fn tie(
        of: &mut HashMap<u32, u32>,
        addr_of: &mut HashMap<u32, u32>,
        addr: u32,
        slot: u32,
        what: &str,
    ) {
        if let Some(&s) = of.get(&addr) {
            assert_eq!(
                s, slot,
                "{what} at {addr:#x} reused as slot {slot}, was slot {s}"
            );
        }
        if let Some(&a) = addr_of.get(&slot) {
            assert_eq!(a, addr, "{what} slot {slot} at {addr:#x}, was at {a:#x}");
        }
        of.insert(addr, slot);
        addr_of.insert(slot, addr);
    }
    fn gobj(&mut self, addr: u32, g: GObjId) {
        Self::tie(&mut self.gobj_of, &mut self.gobj_addr, addr, g.0, "GObj");
    }
    fn proc(&mut self, addr: u32, p: ProcId) {
        Self::tie(&mut self.proc_of, &mut self.proc_addr, addr, p.0, "process");
    }
}

/// ssbm-rs's side of a `run_procs`: what each invocation did.
#[derive(Default)]
struct Run {
    rng: Option<Rng>,
    /// Per invocation: the GObj's address, the function, and the action taken.
    log: Vec<(u32, u32, Action)>,
    /// Addresses of what actions created, in order.
    created: Vec<(u32, u32)>,
    live_gobjs: Vec<GObjId>,
    live_procs: Vec<ProcId>,
}

thread_local! {
    /// Releases of user data and objects, in order, on ssbm-rs's side.
    static RELEASES: RefCell<Vec<(u32, u32)>> = const { RefCell::new(Vec::new()) };
    static RUN: RefCell<Run> = RefCell::new(Run::default());
    static IDS: RefCell<Ids> = RefCell::new(Ids::default());
}

fn gobj_at(ctx: &Ctx, addr: u32) -> HSD_GObj<'_> {
    handle(ctx, addr)
}

/// A test process on ssbm-rs's side: logs itself and maybe acts.
fn native_proc(ctx: &Ctx) {
    let me = ctx.regs.r(3);
    let current_proc = ctx.read_u32(sym("HSD_GObj_CurrentInvokedProc"));
    let func = ctx.read_u32(current_proc + 0x14);
    let next_proc = ctx.read_u32(sym("HSD_GObj_NextInvokedProc"));
    let action = RUN.with(|r| {
        let mut r = r.borrow_mut();
        let (live_g, live_p) = (r.live_gobjs.clone(), r.live_procs.clone());
        let rng = r.rng.as_mut().unwrap();
        let roll = rng.below(22);
        let ids = |f: &dyn Fn(&Ids) -> bool| IDS.with(|i| f(&i.borrow()));
        let a = match roll {
            0 => Action::FreeSelf,
            1 => Action::RemoveSelfProc,
            2 | 3 => Action::ChangeSelf {
                how: rng.below(2) as u32,
                p_link: rng.below(P_LINKS_USED as u64) as u8,
                priority: rng.below(4) as u8,
            },
            4 | 5 => Action::CreateWithProc {
                classifier: rng.below(3) as u16,
                p_link: rng.below(P_LINKS_USED as u64) as u8,
                priority: rng.below(4) as u8,
                func: rng.below(FUNCS as u64) as u32,
                s_link: rng.below(S_LINKS_USED as u64) as u8,
            },
            6 if !live_g.is_empty() => {
                let g = live_g[rng.below(live_g.len() as u64) as usize];
                // Not the running GObj, nor one owning the process that runs next: the
                // original would walk freed memory there.
                let addr = ids(&|i| i.gobj_addr.get(&g.0).is_some_and(|&a| a == me));
                let owns_next = next_proc != 0 && ctx.read_u32(next_proc + 0x10) == ids_addr(g);
                if addr || owns_next {
                    Action::Nothing
                } else {
                    Action::FreeOther(g)
                }
            }
            7 if !live_p.is_empty() => {
                let p = live_p[rng.below(live_p.len() as u64) as usize];
                let pa = IDS.with(|i| i.borrow().proc_addr[&p.0]);
                if pa == current_proc || pa == next_proc {
                    Action::Nothing
                } else {
                    Action::RemoveOtherProc(p)
                }
            }
            8..=10 if !live_g.is_empty() => {
                let g = live_g[rng.below(live_g.len() as u64) as usize];
                let is_me = ids(&|i| i.gobj_addr.get(&g.0).is_some_and(|&a| a == me));
                let (how, p_link, priority) = (
                    rng.below(2) as u32,
                    rng.below(P_LINKS_USED as u64) as u8,
                    rng.below(4) as u8,
                );
                if is_me {
                    Action::Nothing
                } else {
                    Action::ChangeOther {
                        g,
                        how,
                        p_link,
                        priority,
                    }
                }
            }
            _ => Action::Nothing,
        };
        r.log.push((me, func, a));
        a
    });
    match action {
        Action::Nothing => {}
        Action::FreeSelf => {
            forget_gobj(ctx, me);
            fns::HSD_GObjFree(ctx, gobj_at(ctx, me));
        }
        Action::RemoveSelfProc => {
            forget_proc(current_proc);
            fns::HSD_GObjProc_RemoveProc(ctx, handle(ctx, current_proc));
        }
        Action::ChangeSelf {
            how,
            p_link,
            priority,
        } => fns::HSD_GObjPLink_ChangeGObjPri_Unk(
            ctx,
            how,
            gobj_at(ctx, me),
            p_link,
            priority,
            handle(ctx, 0),
        ),
        Action::CreateWithProc {
            classifier,
            p_link,
            priority,
            func,
            s_link,
        } => {
            let g = fns::GObj_Create(ctx, classifier, p_link, priority).addr();
            let p = fns::HSD_GObj_SetupProc(
                ctx,
                gobj_at(ctx, g),
                handle(ctx, FUNC_BASE + 4 * func),
                s_link,
            )
            .addr();
            RUN.with(|r| r.borrow_mut().created.push((g, p)));
        }
        Action::FreeOther(g) => {
            let a = ids_addr(g);
            forget_gobj(ctx, a);
            fns::HSD_GObjFree(ctx, gobj_at(ctx, a));
        }
        Action::ChangeOther {
            g,
            how,
            p_link,
            priority,
        } => fns::HSD_GObjPLink_ChangeGObjPri_Unk(
            ctx,
            how,
            gobj_at(ctx, ids_addr(g)),
            p_link,
            priority,
            handle(ctx, 0),
        ),
        Action::RemoveOtherProc(p) => {
            let a = IDS.with(|i| i.borrow().proc_addr[&p.0]);
            forget_proc(a);
            fns::HSD_GObjProc_RemoveProc(ctx, handle(ctx, a));
        }
    }
}

/// Drops a GObj that ssbm-rs's side is about to free, and its processes, from the targets
/// later actions may pick.
fn forget_gobj(ctx: &Ctx, addr: u32) {
    let mut procs = Vec::new();
    let mut pc = ctx.read_u32(addr + 0x18);
    while pc != 0 {
        procs.push(pc);
        pc = ctx.read_u32(pc);
    }
    IDS.with(|i| {
        let i = i.borrow();
        let g = i.gobj_of.get(&addr).copied();
        let ps: Vec<u32> = procs
            .iter()
            .filter_map(|a| i.proc_of.get(a).copied())
            .collect();
        RUN.with(|r| {
            let mut r = r.borrow_mut();
            r.live_gobjs.retain(|x| Some(x.0) != g);
            r.live_procs.retain(|x| !ps.contains(&x.0));
        });
    });
}

fn forget_proc(addr: u32) {
    let p = IDS.with(|i| i.borrow().proc_of.get(&addr).copied());
    RUN.with(|r| r.borrow_mut().live_procs.retain(|x| Some(x.0) != p));
}

fn ids_addr(g: GObjId) -> u32 {
    IDS.with(|i| i.borrow().gobj_addr[&g.0])
}

fn native_render(_: &Ctx) {}

fn native_user_remove(ctx: &Ctx) {
    let data = ctx.regs.r(3);
    RELEASES.with(|r| r.borrow_mut().push((0xFF, data)));
}

fn native_obj_remove_0(ctx: &Ctx) {
    let obj = ctx.regs.r(3);
    RELEASES.with(|r| r.borrow_mut().push((0, obj)));
}

fn native_obj_remove_1(ctx: &Ctx) {
    let obj = ctx.regs.r(3);
    RELEASES.with(|r| r.borrow_mut().push((1, obj)));
}

/// Our side: replays ssbm-rs's actions in order.
struct Ours {
    world: World<T>,
    actions: Vec<Action>,
    at: usize,
    log: Vec<(GObjId, u32)>,
    created: Vec<(GObjId, ProcId)>,
    releases: Vec<(u32, u32)>,
}

impl Host<T> for Ours {
    fn world(&mut self) -> &mut World<T> {
        &mut self.world
    }
    fn invoke(&mut self, g: GObjId, func: u32) {
        let action = self.actions[self.at];
        self.at += 1;
        self.log.push((g, func));
        let current = self.world.current_proc().unwrap();
        match action {
            Action::Nothing => {}
            Action::FreeSelf => gobj::free(self, g),
            Action::RemoveSelfProc => self.world.remove_proc(current),
            Action::ChangeSelf {
                how,
                p_link,
                priority,
            } => {
                let place = if how == 0 {
                    Place::FromTail
                } else {
                    Place::FromHead
                };
                self.world.change_priority(g, place, p_link, priority)
            }
            Action::CreateWithProc {
                classifier,
                p_link,
                priority,
                func,
                s_link,
            } => {
                let n = self.world.create(classifier, p_link, priority);
                let p = self.world.setup_proc(n, func, s_link);
                self.created.push((n, p));
            }
            Action::FreeOther(o) => gobj::free(self, o),
            Action::ChangeOther {
                g,
                how,
                p_link,
                priority,
            } => {
                let place = if how == 0 {
                    Place::FromTail
                } else {
                    Place::FromHead
                };
                self.world.change_priority(g, place, p_link, priority)
            }
            Action::RemoveOtherProc(p) => self.world.remove_proc(p),
        }
    }
    fn remove_user_data(&mut self, _: GObjId, _: u8, data: u32) {
        self.releases.push((0xFF, data));
    }
    fn remove_obj(&mut self, kind: u8, obj: u32) {
        self.releases.push((u32::from(kind), obj));
    }
}

fn setup_reference() -> Option<Ctx> {
    let ctx = reference()?;
    for i in 0..FUNCS {
        ctx.register(FUNC_BASE + 4 * i, native_proc);
        ctx.register(RENDER_BASE + 4 * i, native_render);
    }
    ctx.register(USER_REMOVE, native_user_remove);
    ctx.register(OBJ_REMOVE, native_obj_remove_0);
    ctx.register(OBJ_REMOVE + 4, native_obj_remove_1);
    fns::HSD_ObjSetHeap(&ctx, 0x0010_0000, handle::<Addr>(&ctx, OBJ_HEAP));
    // HSD_GObjInit's tables, as it would allocate them, with Melee's limits.
    let (p_links, gx_links, pris) = (64u32, 64u32, 25u32);
    let lib = sym("HSD_GObjLibInitData");
    ctx.write_u8(lib, (p_links - 1) as u8);
    ctx.write_u8(lib + 1, (gx_links - 1) as u8);
    ctx.write_u8(lib + 2, (pris - 1) as u8);
    ctx.write_u32(lib + 4, 0);
    let mask = TABLES;
    ctx.write_u64(mask, 0);
    ctx.write_u32(lib + 8, mask);
    let mut at = TABLES + 0x10;
    let mut table = |name: &str, n: u32| {
        ctx.write_u32(sym(name), at);
        ctx.fill(at, 0, 4 * n);
        at += 4 * n;
    };
    table("HSD_GObjPLinkHead", p_links);
    table("plinklow_gobjs", p_links);
    table("HSD_GObjGXLinkHead", gx_links + 1);
    table("HSD_GObj_804D7820", gx_links + 1);
    table("HSD_GObj_GObjProcHead", pris);
    table("HSD_GObj_ProcList", pris * p_links);
    // The object kinds' release functions (HSD_GObj_804D7810).
    ctx.write_u32(sym("HSD_GObj_804D7810"), at);
    ctx.write_u32(at, OBJ_REMOVE);
    ctx.write_u32(at + 4, OBJ_REMOVE + 4);
    fns::HSD_ObjAllocInit(
        &ctx,
        handle::<_HSD_ObjAllocData>(&ctx, sym("gobj_alloc_data")),
        0x38,
        4,
    );
    fns::HSD_ObjAllocInit(
        &ctx,
        handle::<_HSD_ObjAllocData>(&ctx, sym("gobjproc_alloc_data")),
        0x18,
        4,
    );
    for name in [
        "HSD_GObj_804D783C",
        "HSD_GObj_CurrentInvokedProcGObj",
        "HSD_GObj_CurrentInvokedProc",
    ] {
        ctx.write_u32(sym(name), 0);
    }
    ctx.fill(sym("HSD_GObj_DelayedProcInfo"), 0, 0x10);
    Some(ctx)
}

/// Asserts both schedulers hold the same lists.
fn compare(ctx: &Ctx, w: &World<T>, step: usize) {
    IDS.with(|ids| {
        let ids = ids.borrow();
        let g = |addr: u32| {
            if addr == 0 {
                None
            } else {
                Some(GObjId(ids.gobj_of[&addr]))
            }
        };
        let p = |addr: u32| {
            if addr == 0 {
                None
            } else {
                Some(ProcId(ids.proc_of[&addr]))
            }
        };
        let heads = ctx.read_u32(sym("HSD_GObjPLinkHead"));
        let tails = ctx.read_u32(sym("plinklow_gobjs"));
        for link in 0..64u32 {
            let mut theirs = Vec::new();
            let mut cur = ctx.read_u32(heads + 4 * link);
            while cur != 0 {
                theirs.push(g(cur).unwrap());
                cur = ctx.read_u32(cur + 8);
            }
            let mut ours = Vec::new();
            let mut c = w.first_in(link as u8);
            while let Some(x) = c {
                ours.push(x);
                c = w.gobj(x).next;
            }
            assert_eq!(ours, theirs, "step {step}: p_link {link} order");
            assert_eq!(
                w.last_in(link as u8),
                g(ctx.read_u32(tails + 4 * link)),
                "step {step}: p_link {link} tail"
            );
            for &x in &ours {
                let a = ids.gobj_addr[&x.0];
                let o = w.gobj(x);
                assert_eq!(
                    o.p_priority,
                    ctx.read_u8(a + 4),
                    "step {step}: priority of {x:?}"
                );
                assert_eq!(
                    o.gx_link,
                    ctx.read_u8(a + 3),
                    "step {step}: gx_link of {x:?}"
                );
                assert_eq!(
                    o.render_priority,
                    ctx.read_u8(a + 5),
                    "step {step}: render priority of {x:?}"
                );
                assert_eq!(
                    o.prev,
                    g(ctx.read_u32(a + 0xC)),
                    "step {step}: prev of {x:?}"
                );
                let mut chain = Vec::new();
                let mut pc = ctx.read_u32(a + 0x18);
                while pc != 0 {
                    chain.push(p(pc).unwrap());
                    pc = ctx.read_u32(pc);
                }
                let mut our_chain = Vec::new();
                let mut pc = o.proc;
                while let Some(y) = pc {
                    our_chain.push(y);
                    pc = w.proc(y).child;
                }
                assert_eq!(our_chain, chain, "step {step}: processes of {x:?}");
                for &y in &our_chain {
                    let pa = ids.proc_addr[&y.0];
                    let flags = ctx.read_u8(pa + 0xD);
                    let pr = w.proc(y);
                    assert_eq!(
                        pr.flags_1,
                        flags & 0x80 != 0,
                        "step {step}: flags_1 of {y:?}"
                    );
                    assert_eq!(
                        pr.flags_3,
                        (flags >> 4) & 3,
                        "step {step}: flags_3 of {y:?}"
                    );
                    assert_eq!(
                        pr.s_link,
                        ctx.read_u8(pa + 0xC),
                        "step {step}: s_link of {y:?}"
                    );
                    assert_eq!(
                        pr.prev,
                        p(ctx.read_u32(pa + 8)),
                        "step {step}: prev of {y:?}"
                    );
                }
            }
        }
        let phead = ctx.read_u32(sym("HSD_GObj_GObjProcHead"));
        let plist = ctx.read_u32(sym("HSD_GObj_ProcList"));
        for s in 0..25u32 {
            let mut theirs = Vec::new();
            let mut cur = ctx.read_u32(phead + 4 * s);
            while cur != 0 {
                theirs.push(p(cur).unwrap());
                cur = ctx.read_u32(cur + 4);
            }
            let mut ours = Vec::new();
            let mut c = w.first_proc(s as u8);
            while let Some(x) = c {
                ours.push(x);
                c = w.proc(x).next;
            }
            assert_eq!(ours, theirs, "step {step}: run order of priority {s}");
            for link in 0..64u32 {
                let theirs = p(ctx.read_u32(plist + 4 * (link + s * 64)));
                let ours = w.proc_last_for(link as u8, s as u8);
                assert_eq!(
                    ours, theirs,
                    "step {step}: last process of p_link {link} priority {s}"
                );
            }
        }
        let gx = ctx.read_u32(sym("HSD_GObjGXLinkHead"));
        for link in 0..65u32 {
            let mut theirs = Vec::new();
            let mut cur = ctx.read_u32(gx + 4 * link);
            while cur != 0 {
                theirs.push(g(cur).unwrap());
                cur = ctx.read_u32(cur + 0x10);
            }
            let mut ours = Vec::new();
            let mut c = w.first_rendered(link as u8);
            while let Some(x) = c {
                ours.push(x);
                c = w.gobj(x).next_gx;
            }
            assert_eq!(ours, theirs, "step {step}: render link {link} order");
        }
    });
}

fn live(w: &World<T>) -> (Vec<GObjId>, Vec<ProcId>) {
    let (mut gs, mut ps) = (Vec::new(), Vec::new());
    for link in 0..64u8 {
        let mut c = w.first_in(link);
        while let Some(x) = c {
            gs.push(x);
            let mut pc = w.gobj(x).proc;
            while let Some(y) = pc {
                ps.push(y);
                pc = w.proc(y).child;
            }
            c = w.gobj(x).next;
        }
    }
    (gs, ps)
}

#[test]
fn scheduler_matches() {
    for seed in 1..=40u64 {
        run_seed(seed);
    }
}

fn run_seed(seed: u64) {
    let Some(ctx) = setup_reference() else { return };
    let ctx = &ctx;
    IDS.with(|i| *i.borrow_mut() = Ids::default());
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15 ^ seed.wrapping_mul(0x1234_5678_9ABC));
    let mut ours = Ours {
        world: World::new(),
        actions: Vec::new(),
        at: 0,
        log: Vec::new(),
        created: Vec::new(),
        releases: Vec::new(),
    };
    for step in 0..600 {
        let (gs, ps) = live(&ours.world);
        let pick_g = |rng: &mut Rng| gs[rng.below(gs.len() as u64) as usize];
        let roll = rng.below(19);
        if std::env::var_os("GOBJ_TRACE").is_some() {
            eprintln!("seed {seed} step {step}: roll {roll}");
        }
        match roll {
            0..=3 => {
                let how = rng.below(4) as u32;
                let (classifier, p_link, priority) = (
                    rng.below(3) as u16,
                    rng.below(P_LINKS_USED as u64) as u8,
                    rng.below(4) as u8,
                );
                // A position on the same link, as the game always passes.
                let same: Vec<GObjId> = gs
                    .iter()
                    .copied()
                    .filter(|&o| ours.world.gobj(o).p_link == p_link)
                    .collect();
                let position = if how >= 2 && !same.is_empty() {
                    Some(same[rng.below(same.len() as u64) as usize])
                } else {
                    None
                };
                let how = if position.is_none() { how % 2 } else { how };
                let pos_addr = position.map_or(0, ids_addr);
                let a = fns::CreateGObj(
                    ctx,
                    how as i32,
                    classifier,
                    p_link,
                    priority,
                    gobj_at(ctx, pos_addr),
                )
                .addr();
                let place = match how {
                    0 => Place::FromTail,
                    1 => Place::FromHead,
                    2 => Place::After(position.unwrap()),
                    _ => Place::Before(position.unwrap()),
                };
                let g = ours.world.create_at(place, classifier, p_link, priority);
                IDS.with(|i| i.borrow_mut().gobj(a, g));
            }
            4..=6 if !gs.is_empty() => {
                let g = pick_g(&mut rng);
                let (func, s_link) = (
                    rng.below(FUNCS as u64) as u32,
                    rng.below(S_LINKS_USED as u64) as u8,
                );
                let a = fns::HSD_GObj_SetupProc(
                    ctx,
                    gobj_at(ctx, ids_addr(g)),
                    handle::<FnPtr>(ctx, FUNC_BASE + 4 * func),
                    s_link,
                )
                .addr();
                let p = ours.world.setup_proc(g, func, s_link);
                IDS.with(|i| i.borrow_mut().proc(a, p));
            }
            7 if !ps.is_empty() => {
                let p = ps[rng.below(ps.len() as u64) as usize];
                let a = IDS.with(|i| i.borrow().proc_addr[&p.0]);
                fns::HSD_GObjProc_RemoveProc(ctx, handle::<HSD_GObjProc>(ctx, a));
                ours.world.remove_proc(p);
            }
            8 if !gs.is_empty() => {
                let g = pick_g(&mut rng);
                fns::HSD_GObjFree(ctx, gobj_at(ctx, ids_addr(g)));
                gobj::free(&mut ours, g);
            }
            9 if !gs.is_empty() => {
                let g = pick_g(&mut rng);
                let how = rng.below(4) as u32;
                let (p_link, priority) = (rng.below(P_LINKS_USED as u64) as u8, rng.below(4) as u8);
                let others: Vec<GObjId> = gs
                    .iter()
                    .copied()
                    .filter(|&o| o != g && ours.world.gobj(o).p_link == p_link)
                    .collect();
                let position = if how >= 2 && !others.is_empty() {
                    Some(others[rng.below(others.len() as u64) as usize])
                } else {
                    None
                };
                let how = if position.is_none() { how % 2 } else { how };
                fns::HSD_GObjPLink_ChangeGObjPri_Unk(
                    ctx,
                    how,
                    gobj_at(ctx, ids_addr(g)),
                    p_link,
                    priority,
                    gobj_at(ctx, position.map_or(0, ids_addr)),
                );
                let place = match how {
                    0 => Place::FromTail,
                    1 => Place::FromHead,
                    2 => Place::After(position.unwrap()),
                    _ => Place::Before(position.unwrap()),
                };
                ours.world.change_priority(g, place, p_link, priority);
            }
            10 if !gs.is_empty() => {
                let g = pick_g(&mut rng);
                let a = ids_addr(g);
                let has = ours.world.gobj(g).gx_link != gobj::GX_LINK_NONE;
                match (has, rng.below(3)) {
                    (false, 0) => {
                        let (link, prio) = (rng.below(4) as u8, rng.below(4) as u8);
                        fns::GObj_SetupGXLink(
                            ctx,
                            gobj_at(ctx, a),
                            handle(ctx, RENDER_BASE),
                            link,
                            prio as u32,
                        );
                        ours.world.setup_gx_link(g, 0, link, prio);
                    }
                    (false, s) => {
                        let prio = rng.below(4) as u8;
                        if s == 1 {
                            fns::GObj_SetupGXLinkMax(
                                ctx,
                                gobj_at(ctx, a),
                                handle(ctx, RENDER_BASE),
                                prio as u32,
                            );
                        } else {
                            fns::GObj_SetupGXLinkMaxSorted(
                                ctx,
                                gobj_at(ctx, a),
                                handle(ctx, RENDER_BASE),
                                prio as u32,
                            );
                        }
                        ours.world.setup_gx_link_max(g, 0, prio, s == 2);
                    }
                    (true, 0) => {
                        fns::HSD_GObjGXLink_8039084C(ctx, gobj_at(ctx, a));
                        ours.world.gx_unlink(g);
                    }
                    (true, 1) => {
                        let (link, prio) = (rng.below(4) as u8, rng.below(4) as u8);
                        fns::HSD_GObjGXLink_80390908(ctx, gobj_at(ctx, a), link, prio);
                        ours.world.gx_relink(g, link, prio);
                    }
                    (true, _) => {
                        let linked: Vec<GObjId> = gs
                            .iter()
                            .copied()
                            .filter(|&o| o != g && ours.world.gobj(o).gx_link != gobj::GX_LINK_NONE)
                            .collect();
                        if let Some(&o) = linked.get(rng.below(linked.len().max(1) as u64) as usize)
                        {
                            fns::HSD_GObjGXLink_803909D8(
                                ctx,
                                gobj_at(ctx, a),
                                gobj_at(ctx, ids_addr(o)),
                            );
                            ours.world.gx_relink_before(g, o);
                        }
                    }
                }
            }
            11 if !gs.is_empty() => {
                let g = pick_g(&mut rng);
                let paused = rng.below(2) == 0;
                if paused {
                    fns::HSD_GObj_80390C5C(ctx, gobj_at(ctx, ids_addr(g)));
                } else {
                    fns::HSD_GObj_80390C84(ctx, gobj_at(ctx, ids_addr(g)));
                }
                ours.world.set_paused(g, paused);
            }
            16 if !gs.is_empty() => {
                let g = pick_g(&mut rng);
                if ours.world.gobj(g).user_data_kind == 0xFF {
                    let (kind, data) = (rng.below(3) as u8, 0x8000_1000 + step as u32);
                    fns::GObj_InitUserData(
                        ctx,
                        gobj_at(ctx, ids_addr(g)),
                        kind,
                        handle(ctx, USER_REMOVE),
                        handle(ctx, data),
                    );
                    ours.world.init_user_data(g, kind, data);
                }
            }
            17 if !gs.is_empty() => {
                let g = pick_g(&mut rng);
                let a = ids_addr(g);
                if ours.world.gobj(g).obj_kind == 0xFF {
                    let (kind, obj) = (rng.below(2) as u8, 0x8000_2000 + step as u32);
                    fns::HSD_GObjObject_80390A70(ctx, gobj_at(ctx, a), kind, handle(ctx, obj));
                    ours.world.set_obj(g, kind, obj);
                } else {
                    let theirs = fns::HSD_GObjObject_80390ADC(ctx, gobj_at(ctx, a)).addr();
                    let ours_obj = ours.world.take_obj(g).map(|o| o.1);
                    assert_eq!(
                        ours_obj,
                        Some(theirs),
                        "seed {seed} step {step}: detached object"
                    );
                }
            }
            18 if !gs.is_empty() => {
                let g = pick_g(&mut rng);
                if rng.below(2) == 0 {
                    fns::HSD_GObj_80390CD4(ctx, gobj_at(ctx, ids_addr(g)));
                    ours.world.mark_ran(g);
                } else {
                    fns::HSD_GObj_80390CAC(ctx, gobj_at(ctx, ids_addr(g)));
                    ours.world.clear_flags_2(g);
                }
            }
            12 => {
                let (classifier, link) =
                    (rng.below(3) as u16, rng.below(P_LINKS_USED as u64) as u8);
                let theirs = fns::HSD_GObjObject_80390A3C(ctx, classifier, link).addr();
                let theirs = IDS.with(|i| {
                    if theirs == 0 {
                        None
                    } else {
                        Some(GObjId(i.borrow().gobj_of[&theirs]))
                    }
                });
                assert_eq!(
                    ours.world.find(classifier, link),
                    theirs,
                    "seed {seed} step {step}: find"
                );
            }
            _ => {
                // A tick: ssbm-rs's side decides each process's action; ours replays them.
                RUN.with(|r| {
                    let mut r = r.borrow_mut();
                    *r = Run::default();
                    r.rng = Some(Rng(rng.next() | 1));
                    r.live_gobjs = gs.clone();
                    r.live_procs = ps.clone();
                });
                fns::HSD_GObj_RunProcs(ctx);
                let (log, created) = RUN.with(|r| {
                    let mut r = r.borrow_mut();
                    (std::mem::take(&mut r.log), std::mem::take(&mut r.created))
                });
                ours.actions = log.iter().map(|l| l.2).collect();
                ours.at = 0;
                ours.log.clear();
                ours.created.clear();
                gobj::run_procs(&mut ours);
                assert_eq!(
                    ours.at,
                    ours.actions.len(),
                    "seed {seed} step {step}: invocations"
                );
                IDS.with(|i| {
                    let mut i = i.borrow_mut();
                    for (&(ga, pa), &(g, p)) in created.iter().zip(&ours.created) {
                        i.gobj(ga, g);
                        i.proc(pa, p);
                    }
                });
                let theirs: Vec<(GObjId, u32)> = log
                    .iter()
                    .map(|&(a, f, _)| {
                        (
                            GObjId(IDS.with(|i| i.borrow().gobj_of[&a])),
                            (f - FUNC_BASE) / 4,
                        )
                    })
                    .collect();
                assert_eq!(ours.log, theirs, "seed {seed} step {step}: processes run");
            }
        }
        let theirs = RELEASES.with(|r| std::mem::take(&mut *r.borrow_mut()));
        assert_eq!(
            std::mem::take(&mut ours.releases),
            theirs,
            "seed {seed} step {step}: releases"
        );
        compare(ctx, &ours.world, step);
    }
}
