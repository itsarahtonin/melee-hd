//! The player on Windows carries its icon (icon/melee-hd.ico, which icon/make.py draws) and an
//! application manifest: Common Controls 6, which its dialogs' own button labels need (rfd's
//! `common-controls-v6`), and per-monitor DPI awareness. Both go in one resource section.
//!
//! It also exports the two values laptop drivers read to run a program on the discrete GPU
//! (`NvOptimusEnablement` and `AmdPowerXpressRequestHighPerformance`, statics in main.rs):
//! a binary exports nothing unless the linker is told to (`tests/gpu_hint.rs` reads them back).

/// The statics the dual-GPU drivers look for among the executable's exports.
const GPU_EXPORTS: [&str; 2] = [
    "NvOptimusEnablement",
    "AmdPowerXpressRequestHighPerformance",
];

fn main() {
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        let manifest = embed_manifest::new_manifest("melee-hd").to_string();
        let mut resources = winresource::WindowsResource::new();
        resources
            .set_icon("icon/melee-hd.ico")
            .set_manifest(&manifest)
            .set("ProductName", "melee-hd")
            .set("FileDescription", "melee-hd");
        resources.compile().expect("the player's resources");
        if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
            for name in GPU_EXPORTS {
                println!("cargo:rustc-link-arg-bins=/EXPORT:{name},DATA");
            }
        }
    }
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=icon/melee-hd.ico");
}
