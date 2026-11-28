//! The first thread's stack on macOS: the player runs there (winit's event loop must), so
//! build.rs has the linker give it the game's 512 MiB (`-stack_size`), which the kernel reads
//! from the executable's `LC_MAIN` command. Read back from the built executable with a small
//! Mach-O reader, and the executable started, which a stack the kernel won't give would stop.

/// main.rs's `GAME_STACK`.
const GAME_STACK: u64 = 512 << 20;

/// `LC_MAIN`: the entry point and the first thread's stack size.
const LC_MAIN: u32 = 0x8000_0028;

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}

fn u64_at(b: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(b[at..at + 8].try_into().unwrap())
}

/// The stack size a thin 64-bit Mach-O executable's `LC_MAIN` asks for (0 for the system's
/// default); none without the command.
fn main_stack(b: &[u8]) -> Option<u64> {
    assert_eq!(u32_at(b, 0), 0xfeed_facf, "a 64-bit Mach-O image");
    let commands = u32_at(b, 16) as usize;
    // The header is 32 bytes; the load commands follow it.
    let mut at = 32;
    for _ in 0..commands {
        let (cmd, size) = (u32_at(b, at), u32_at(b, at + 4) as usize);
        if cmd == LC_MAIN {
            return Some(u64_at(b, at + 16));
        }
        at += size;
    }
    None
}

/// The reader on a made-up image: a header, a command it skips and `LC_MAIN`.
#[test]
fn reads_lc_main() {
    let mut b = Vec::new();
    for w in [0xfeed_facf_u32, 0x0100_000c, 0, 2, 2, 16 + 24, 0, 0] {
        b.extend(w.to_le_bytes());
    }
    // LC_SEGMENT_64's number, cut to 16 bytes.
    for w in [0x19_u32, 16, 0, 0] {
        b.extend(w.to_le_bytes());
    }
    for w in [LC_MAIN, 24] {
        b.extend(w.to_le_bytes());
    }
    b.extend(0x4000_u64.to_le_bytes());
    b.extend(GAME_STACK.to_le_bytes());
    assert_eq!(main_stack(&b), Some(GAME_STACK));
    b[48..52].copy_from_slice(&0x19_u32.to_le_bytes());
    assert_eq!(main_stack(&b), None);
}

#[cfg(target_os = "macos")]
#[test]
fn the_first_thread_has_the_games_stack() {
    let exe = env!("CARGO_BIN_EXE_melee-hd");
    let bytes = std::fs::read(exe).expect("the built executable");
    assert_eq!(main_stack(&bytes), Some(GAME_STACK), "{exe}'s LC_MAIN");
    // It starts with that stack: `--help` prints the usage and exits with 2.
    let out = std::process::Command::new(exe)
        .arg("--help")
        .env(
            "MELEE_HD_SETTINGS",
            std::env::temp_dir().join("melee-hd-mac-stack"),
        )
        .output()
        .expect("melee-hd runs");
    let text = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "{text}");
    assert!(text.contains("usage"), "{text}");
}
