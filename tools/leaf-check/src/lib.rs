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

/// The reference with HAL's heap and object allocators set up as `HSD_InitComponent` does
/// (an OS heap from 0x80F00000 to 0x81500000, then `HSD_ObjInit`'s allocators), so ports
/// that allocate HSD objects (joints, display objects, animations, ID-table entries) run.
pub fn hsd_reference() -> Option<Ctx> {
    use ssbm_rt::{Addr, Handle};
    let ctx = reference()?;
    let (lo, hi) = (HEAP_LO, HEAP_HI);
    let lo =
        ssbm_types::fns::OSInitAlloc(&ctx, handle::<Addr>(&ctx, lo), handle::<Addr>(&ctx, hi), 1)
            .addr();
    let lo = (lo + 31) & !31;
    let heap =
        ssbm_types::fns::OSCreateHeap(&ctx, handle::<Addr>(&ctx, lo), handle::<Addr>(&ctx, hi));
    ssbm_types::fns::OSSetCurrentHeap(&ctx, heap);
    ctx.write_u32(sym("current_heap"), heap as u32);
    ssbm_types::fns::HSD_ObjSetHeap(&ctx, hi - lo, handle::<Addr>(&ctx, 0));
    use ssbm_types::fns::*;
    for init in [
        HSD_ListInitAllocData,
        HSD_AObjInitAllocData,
        HSD_FObjInitAllocData,
        HSD_IDInitAllocData,
        HSD_VecInitAllocData,
        HSD_MtxInitAllocData,
        HSD_RObjInitAllocData,
        HSD_RenderInitAllocData,
        HSD_ShadowInitAllocData,
        HSD_ZListInitAllocData,
    ] {
        init(&ctx);
    }
    Some(ctx)
}

/// The OS heap `hsd_reference` sets up.
pub const HEAP_LO: u32 = 0x80F0_0000;
pub const HEAP_HI: u32 = 0x8150_0000;

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
