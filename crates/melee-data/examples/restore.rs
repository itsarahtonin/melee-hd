//! Restores an NKit image to a checked RVZ, as the player does:
//! `cargo run -p melee-data --example restore -- NKIT OUT.rvz`. For checking locally; never
//! commit what it writes.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [nkit, out] = &args[..] else {
        eprintln!("usage: restore NKIT OUT.rvz");
        std::process::exit(2);
    };
    let start = std::time::Instant::now();
    let mut shown = 0;
    let result = melee_data::nkit::restore(nkit.as_ref(), out.as_ref(), |at, of| {
        let percent = at * 100 / of.max(1);
        if percent >= shown + 10 {
            shown = percent;
            eprintln!("{percent}%");
        }
    });
    match result {
        Ok(()) => eprintln!(
            "restored to {out} in {:.1} s",
            start.elapsed().as_secs_f64()
        ),
        Err(e) => {
            eprintln!("{nkit}: {e}");
            std::process::exit(1);
        }
    }
}
