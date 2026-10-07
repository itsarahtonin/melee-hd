//! Copies a file from the disc: `cargo run -p melee-data --example extract -- DISC NAME OUT`.
//! For inspecting game data locally; never commit what it writes.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [disc, name, out] = &args[..] else {
        eprintln!("usage: extract DISC NAME OUT");
        std::process::exit(2);
    };
    let disc = melee_data::disc::Disc::open(disc).unwrap();
    std::fs::write(out, disc.read_file(name).unwrap()).unwrap();
}
