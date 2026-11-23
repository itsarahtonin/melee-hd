//! Lets the reference run display-pass code (GX calls) whose state effects a test compares.

use leaf_check::sym;
use ssbm_rt::{Ctx, Mmio};

/// Where the reference's `__GXData` goes, clear of the scratch memory and the stack.
const GX_DATA: u32 = 0x8170_0000;

/// GX's command FIFO, which nothing in these tests reads: writes vanish, reads give 0.
struct Fifo;

impl Mmio for Fifo {
    fn read(&self, _: &Ctx, _: u32, _: u32) -> u32 {
        0
    }

    fn write(&self, _: &Ctx, _: u32, _: u32, _: u32) {}
}

/// Gives the reference somewhere for GX commands to go and a zeroed `__GXData` with the SDK's
/// default texture and palette region callbacks, as `GXInit` leaves them.
pub fn enable_gx(ctx: &Ctx) {
    ctx.set_mmio(Box::new(Fifo));
    ctx.write_bytes(GX_DATA, &[0; 0x500]);
    ctx.write_u32(GX_DATA + 0x410, sym("__GXDefaultTexRegionCallback"));
    ctx.write_u32(GX_DATA + 0x414, sym("__GXDefaultTlutRegionCallback"));
    ssbm_types::fns::gx(ctx).set(leaf_check::handle(ctx, GX_DATA));
}

/// Makes the reference's `HSD_Panic` and `__assert` panic with their file, line and message
/// rather than halting the game.
pub fn readable_panics(ctx: &Ctx) {
    for f in ["HSD_Panic", "__assert"] {
        ctx.register(sym(f), |ctx| {
            let text = |mut at: u32| {
                let mut s = String::new();
                loop {
                    match ctx.read_u8(at) {
                        0 => break s,
                        b => s.push(char::from(b)),
                    }
                    at += 1;
                }
            };
            let (file, line, msg) = (ctx.regs.r(3), ctx.regs.r(4), ctx.regs.r(5));
            panic!("{} line {line}: {}", text(file), text(msg));
        });
    }
}
