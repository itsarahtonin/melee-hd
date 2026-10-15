//! The player on Windows carries an application manifest: Common Controls 6, which its dialogs'
//! own button labels need (rfd's `common-controls-v6`), and per-monitor DPI awareness.

fn main() {
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        embed_manifest::embed_manifest(embed_manifest::new_manifest("melee-hd"))
            .expect("the player's manifest");
    }
    println!("cargo:rerun-if-changed=build.rs");
}
