//! Lists the archives loaded in a saved call: `archives CALL`.
use melee_capture::{Capture, find_archives};
use melee_data::archive::Archives;
use melee_data::disc::Disc;

fn main() {
    let path = std::env::args().nth(1).expect("archives CALL");
    let cap = Capture::read(path.as_ref());
    let disc = Disc::open(std::env::var("SSBM_DISC").unwrap()).unwrap();
    let mut archives = Archives::default();
    let t = std::time::Instant::now();
    let found = find_archives(&cap.call.mem1, &disc, &mut archives);
    for l in &found {
        println!("{:#010x} {:#x} bytes {}", l.file, l.data_len, l.name);
    }
    println!(
        "{} archives in {:?}; call {:#x}",
        found.len(),
        t.elapsed(),
        cap.call.addr
    );
}
