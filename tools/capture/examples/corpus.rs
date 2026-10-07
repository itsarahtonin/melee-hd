//! Lists the calls a corpus holds, by function: `corpus FILE`.
use melee_capture::corpus::Corpus;

fn main() {
    let path = std::env::args().nth(1).expect("corpus FILE");
    let c = Corpus::read(path.as_ref());
    let mut counts = std::collections::BTreeMap::<u32, usize>::new();
    for i in 0..c.len() {
        *counts.entry(c.addr(i)).or_default() += 1;
    }
    let name = |a: u32| {
        ssbm_types::symbols::SYMBOLS
            .iter()
            .find(|s| s.0 == a)
            .map_or("?", |s| s.2)
    };
    for (a, n) in &counts {
        println!("{a:08x} {n:4} {}", name(*a));
    }
    println!("{} calls of {} functions", c.len(), counts.len());
}
