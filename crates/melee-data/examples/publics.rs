//! Lists an archive's public symbols: `cargo run -p melee-data --example publics -- DISC NAME`.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [disc, name] = &args[..] else {
        eprintln!("usage: publics DISC NAME");
        std::process::exit(2);
    };
    let disc = melee_data::disc::Disc::open(disc).unwrap();
    let archive = melee_data::archive::Archive::parse(&disc.read_file(name).unwrap()).unwrap();
    for (n, o) in archive.publics() {
        println!("{o:#08x} {n}");
    }
}
