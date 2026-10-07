//! Which game functions captured calls reach in the reference, to plan what a port needs:
//! `reach CALL...` prints, for each function reached, how many of the calls reach it and the
//! deepest caller chain it was first met under. `REACH_TREE=1` prints each call's tree instead.
//! Needs `SSBM_DISC`.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use leaf_check::{reference, run_capture};
use melee_capture::Capture;

fn main() {
    let ctx = reference().expect("SSBM_DISC");
    let tree = std::env::var_os("REACH_TREE").is_some();
    let log: Rc<RefCell<Vec<(u32, usize)>>> = Rc::default();
    for addr in ctx.registered() {
        let log = log.clone();
        ctx.set_hook(
            addr,
            Rc::new(move |ctx| log.borrow_mut().push((addr, ctx.native_stack().len()))),
        );
    }
    let mut reached: BTreeMap<u32, (usize, String)> = BTreeMap::new();
    let paths: Vec<String> = std::env::args().skip(1).collect();
    for path in &paths {
        // `PATH@ADDR` runs the call as a different function, as a harness prepare can.
        let (file, addr) = match path.rsplit_once('@') {
            Some((f, a)) => (f, Some(u32::from_str_radix(a, 16).expect("@ADDR"))),
            None => (path.as_str(), None),
        };
        let mut cap = Capture::read(file.as_ref());
        if let Some(a) = addr {
            cap.call.addr = a;
        }
        log.borrow_mut().clear();
        let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run_capture(&ctx, &cap.call);
        }));
        if let Err(p) = run {
            let why = p
                .downcast_ref::<ssbm_rt::Fault>()
                .map(|f| format!("fault at {:#x}", f.addr))
                .or_else(|| p.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "panic".to_owned());
            let stack: Vec<String> = ctx.native_stack().iter().map(|&f| name(f)).collect();
            println!("{path}: stopped: {why} in {}", stack.join(" > "));
        }
        let calls = log.borrow().clone();
        if tree {
            println!("{path}:");
            let mut last = None;
            for &(a, depth) in &calls {
                if last == Some((a, depth)) {
                    continue;
                }
                last = Some((a, depth));
                println!("{:width$}{}", "", name(a), width = 2 * depth);
            }
            continue;
        }
        let mut stack: Vec<u32> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for &(a, depth) in &calls {
            stack.truncate(depth);
            stack.push(a);
            if seen.insert(a) {
                let e = reached.entry(a).or_insert_with(|| {
                    let chain: Vec<String> = stack.iter().map(|&f| name(f)).collect();
                    (0, chain.join(" > "))
                });
                e.0 += 1;
            }
        }
    }
    if !tree {
        let mut rows: Vec<_> = reached.into_iter().collect();
        rows.sort_by_key(|(_, (n, chain))| (std::cmp::Reverse(*n), chain.clone()));
        for (a, (n, chain)) in rows {
            println!("{n:4}/{} {a:08x} {chain}", paths.len());
        }
    }
}

fn name(addr: u32) -> String {
    ssbm_types::symbols::SYMBOLS
        .iter()
        .find(|s| s.0 == addr)
        .map_or_else(|| format!("{addr:#x}"), |s| s.2.to_string())
}
