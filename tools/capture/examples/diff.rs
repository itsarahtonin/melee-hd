//! Where a loaded archive differs from its file: `diff CALL NAME ADDR`.
use melee_capture::{Capture, MEM1};
use melee_data::archive::Archive;
use melee_data::disc::Disc;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let cap = Capture::read(a[1].as_ref());
    let disc = Disc::open(std::env::var("SSBM_DISC").unwrap()).unwrap();
    let bytes = disc.read_file(&a[2]).unwrap();
    let arch = Archive::parse(&bytes).unwrap();
    let at = u32::from_str_radix(a[3].trim_start_matches("0x"), 16).unwrap();
    let base = (at - MEM1) as usize + 0x20;
    let data = arch.data();
    let mut runs: Vec<(usize, usize)> = Vec::new();
    for i in (0..data.len()).step_by(4) {
        if arch.is_pointer(i as u32) {
            continue;
        }
        let m = &cap.call.mem1[base + i..base + i + 4];
        if m != &data[i..i + 4] {
            match runs.last_mut() {
                Some(r) if r.1 + 4 >= i => r.1 = i,
                _ => runs.push((i, i)),
            }
        }
    }
    println!("{} runs", runs.len());
    for (s, e) in runs.iter().take(30) {
        println!(
            "  {s:#x}..{e:#x} ({} words) first mem {:08x} file {:08x}",
            (e - s) / 4 + 1,
            u32::from_be_bytes(cap.call.mem1[base + s..base + s + 4].try_into().unwrap()),
            u32::from_be_bytes(data[*s..s + 4].try_into().unwrap())
        );
    }
    for (n, o) in arch.publics() {
        println!("public {n} {o:#x}");
    }
}
