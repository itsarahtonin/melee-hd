//! ssbm-rs's verified ports, ready to call one function at a time: melee-sim's leaf functions
//! are checked against them input by input (`docs/oracle.md`, leaf functions).
//!
//! The ports run in ssbm-rs's runtime with the disc's `main.dol` loaded, so they find the
//! constants and tables the original code reads. Tests need `SSBM_DISC` and skip without it.

use ssbm_rt::Ctx;

/// Where the reference's stack starts: MEM1 above everything the DOL loads.
const STACK_TOP: u32 = 0x817F_0000;

/// ssbm-rs's runtime with the game's executable loaded and every port registered, or `None`
/// when `SSBM_DISC` is unset.
pub fn reference() -> Option<Ctx> {
    let Ok(path) = std::env::var("SSBM_DISC") else {
        eprintln!("SSBM_DISC is unset: skipping the comparison with ssbm-rs");
        return None;
    };
    let disc = ssbm_disc::Disc::open(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let dol = disc.main_dol().unwrap_or_else(|e| panic!("{path}: {e}"));
    let ctx = Ctx::new();
    dol.load_into(&ctx.mem).expect("main.dol fits in memory");
    ssbm_game::register(&ctx, |_| true);
    ctx.regs.set_r(1, STACK_TOP);
    // r2 and r13 as `__init_registers` sets them: its lis/ori pairs give the small-data bases.
    let init = sym("__init_registers");
    let pair = |at: u32| (ctx.read_u32(at) & 0xFFFF) << 16 | (ctx.read_u32(at + 4) & 0xFFFF);
    ctx.regs.set_r(2, pair(init + 8));
    ctx.regs.set_r(13, pair(init + 16));
    ssbm_types::fns::__sinit_trigf_c(&ctx);
    Some(ctx)
}

/// A symbol's address.
pub fn sym(name: &str) -> u32 {
    ssbm_types::symbols::SYMBOLS
        .iter()
        .find(|s| s.2 == name)
        .unwrap_or_else(|| panic!("no symbol named {name}"))
        .0
}

/// A float as the original loads it into a register.
pub fn arg(x: f32) -> f64 {
    gekko_fp::lfs(x.to_bits())
}

/// A returned register's bits, as the original stores it.
pub fn bits(r: f64) -> u32 {
    gekko_fp::stfs(r)
}

/// Inputs that exercise a float function: special values, then pseudo-random bit patterns
/// and values spread over `range`, from a fixed seed so failures reproduce.
pub fn inputs(range: f32, count: usize) -> Vec<f32> {
    let mut v: Vec<f32> = [
        0.0,
        -0.0,
        1.0,
        -1.0,
        0.5,
        -0.5,
        f32::MIN_POSITIVE,
        -f32::MIN_POSITIVE,
        f32::from_bits(1),
        f32::MAX,
        f32::MIN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
        std::f32::consts::PI,
        std::f32::consts::FRAC_PI_2,
        -std::f32::consts::FRAC_PI_2,
        std::f32::consts::FRAC_PI_4,
    ]
    .to_vec();
    let mut s: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        s
    };
    for _ in 0..count {
        v.push(f32::from_bits(next() as u32));
        let unit = (next() >> 40) as f32 / (1u64 << 24) as f32;
        v.push((unit * 2.0 - 1.0) * range);
    }
    v
}

/// Scratch memory for arguments passed by pointer, clear of the DOL and the stack.
pub const SCRATCH: u32 = 0x8160_0000;

/// A handle to `addr` for passing to a port.
pub fn handle<'a, H: ssbm_rt::Handle<'a>>(ctx: &'a Ctx, addr: u32) -> H {
    H::from_at(ssbm_rt::At::new(ctx, addr))
}

/// Writes floats to memory, as the game stores them.
pub fn write_f32s(ctx: &Ctx, addr: u32, values: &[f32]) {
    for (i, v) in values.iter().enumerate() {
        ctx.write_u32(addr + 4 * i as u32, v.to_bits());
    }
}

pub fn read_f32s(ctx: &Ctx, addr: u32, n: usize) -> Vec<u32> {
    (0..n).map(|i| ctx.read_u32(addr + 4 * i as u32)).collect()
}

/// Where [`load_archive`] keeps the reference's `HSD_Archive` and the file it parses.
const ARCHIVE: u32 = 0x8070_0000;
const FILE: u32 = 0x8080_0000;
/// The largest file [`load_archive`] takes, clear of the memory above it.
pub const FILE_MAX: usize = 0x0070_0000;

/// Loads a DAT file into the reference and parses it as the game does (`HSD_ArchiveParse`
/// relocates its pointers), returning the address of its data block. Each call replaces the
/// previous archive.
pub fn load_archive(ctx: &Ctx, file: &[u8]) -> u32 {
    assert!(file.len() <= FILE_MAX, "a {:#x}-byte archive", file.len());
    ctx.write_bytes(FILE, file);
    let parsed = ssbm_types::fns::HSD_ArchiveParse(
        ctx,
        handle(ctx, ARCHIVE),
        handle(ctx, FILE),
        file.len() as u32,
    );
    assert_eq!(parsed, 0, "HSD_ArchiveParse");
    ctx.read_u32(ARCHIVE + 0x20)
}

/// A DAT file holding `data`, with the words at `relocs` marked as pointers (offsets into
/// `data`) and no symbols: test data both melee-data and the reference can load.
pub fn dat(data: &[u8], relocs: &[u32]) -> Vec<u8> {
    let mut f = Vec::new();
    let size = 0x20 + data.len() + 4 * relocs.len();
    for w in [size as u32, data.len() as u32, relocs.len() as u32, 0, 0] {
        f.extend_from_slice(&w.to_be_bytes());
    }
    f.resize(0x20, 0);
    f.extend_from_slice(data);
    for r in relocs {
        f.extend_from_slice(&r.to_be_bytes());
    }
    f
}

/// A xorshift generator for test inputs, from a fixed seed so failures reproduce.
pub struct Rand(pub u64);

impl Rand {
    pub fn bits(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    /// Uniform in [0, n).
    pub fn below(&mut self, n: u64) -> u64 {
        self.bits() % n
    }

    /// Uniform in [lo, hi].
    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        let unit = (self.bits() >> 40) as f32 / (1u64 << 24) as f32;
        lo + (hi - lo) * unit
    }
}
