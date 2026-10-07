//! melee-sim's controller input against ssbm-rs's ports of the SDK's PAD library and HAL's pad
//! library, bit for bit, in both float modes.
//!
//! The pure functions run on every stick position and on random and edge-case values. The pad
//! library runs side by side with ssbm-rs's on random sequences of polls, renewals, flushes,
//! resets and rumble requests, and every field of the queue, the twelve statuses and rumble
//! must match after every step, with the same motor commands from every poll. Where ssbm-rs's
//! code reaches the hardware (the serial interface, the reset button, the rumble motors),
//! native stubs stand in and report what the test decides, which ours receives as arguments.

#[path = "support/rumble.rs"]
mod rumble;

use std::cell::RefCell;

use gekko_fp::{FpMode, set_fp_mode};
use leaf_check::{SCRATCH, handle, reference, sym};
use melee_data::archive::Archives;
use melee_sim::input::controller::{self, Controllers, Flush, PadData, PadLib, Status};
use melee_sim::input::pad::{self, PadStatus, Sample};
use melee_sim::input::rumble::{Script, Table};
use ssbm_rt::Ctx;

/// ssbm-rs's functions, file-local ones included, under one name.
mod fns {
    pub use ssbm_types::fns::*;
    pub use ssbm_types::tu::dolphin__pad__PadClamp::*;
    pub use ssbm_types::tu::dolphin__pad__pad::*;
    pub use ssbm_types::tu::melee__gm__gmmain::*;
    pub use ssbm_types::tu::sysdolphin__baselib__controller::*;
}

const MODES: [FpMode; 2] = [FpMode::Hardware, FpMode::Slippi];
/// Scratch for statuses, response words and the queue.
const PADS: u32 = SCRATCH;
const DATA: u32 = SCRATCH + 0x100;
const QUEUE: u32 = SCRATCH + 0x200;
/// Rumble nodes for HSD_PadInit other than Melee's.
const NODES: u32 = SCRATCH + 0x400;
const PAD_SIZE: u32 = 12;
const STATUS_SIZE: u32 = 0x44;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }
    fn pick<T: Copy>(&mut self, of: &[T]) -> T {
        of[self.below(of.len() as u64) as usize]
    }
    fn u8(&mut self) -> u8 {
        self.next() as u8
    }
    /// One of `of`, or now and then any byte.
    fn u8_of(&mut self, of: &[u8]) -> u8 {
        if self.chance(80) {
            self.pick(of)
        } else {
            self.u8()
        }
    }
    fn i8_of(&mut self, of: &[i8]) -> i8 {
        if self.chance(80) {
            self.pick(of)
        } else {
            self.i8()
        }
    }
    fn i8(&mut self) -> i8 {
        self.next() as i8
    }
    /// A stick axis: the edges of the dead zones, clamps and notches, or anything.
    fn axis(&mut self) -> i8 {
        const EDGES: [i8; 26] = [
            -128, -127, -126, -81, -80, -79, -72, -59, -57, -56, -40, -31, -23, -22, -15, -1, 0, 1,
            15, 22, 23, 56, 57, 80, 81, 127,
        ];
        if self.chance(50) {
            self.pick(&EDGES)
        } else {
            self.i8()
        }
    }
    /// A trigger: around the clamps' ends, or anything.
    fn trigger(&mut self) -> u8 {
        const EDGES: [u8; 12] = [0, 1, 29, 30, 31, 139, 140, 141, 179, 180, 181, 255];
        if self.chance(50) {
            self.pick(&EDGES)
        } else {
            self.u8()
        }
    }
    /// A float for `adc_angle`: the default 0, quarter turns, specials, or anything.
    fn angle(&mut self) -> f32 {
        const EDGES: [f32; 9] = [
            0.0,
            -0.0,
            std::f32::consts::FRAC_PI_4,
            std::f32::consts::FRAC_PI_2,
            std::f32::consts::PI,
            -std::f32::consts::FRAC_PI_4,
            f32::INFINITY,
            f32::NAN,
            1.0e-7,
        ];
        match self.below(3) {
            0 => self.pick(&EDGES),
            1 => (self.below(1 << 24) as f32 / (1 << 24) as f32) * std::f32::consts::PI,
            _ => f32::from_bits(self.next() as u32),
        }
    }
}

fn put_pad(ctx: &Ctx, at: u32, s: &PadStatus) {
    ctx.write_u16(at, s.button);
    ctx.write_u8(at + 2, s.stick_x as u8);
    ctx.write_u8(at + 3, s.stick_y as u8);
    ctx.write_u8(at + 4, s.substick_x as u8);
    ctx.write_u8(at + 5, s.substick_y as u8);
    ctx.write_u8(at + 6, s.trigger_left);
    ctx.write_u8(at + 7, s.trigger_right);
    ctx.write_u8(at + 8, s.analog_a);
    ctx.write_u8(at + 9, s.analog_b);
    ctx.write_u8(at + 10, s.err as u8);
}

fn get_pad(ctx: &Ctx, at: u32) -> PadStatus {
    PadStatus {
        button: ctx.read_u16(at),
        stick_x: ctx.read_u8(at + 2) as i8,
        stick_y: ctx.read_u8(at + 3) as i8,
        substick_x: ctx.read_u8(at + 4) as i8,
        substick_y: ctx.read_u8(at + 5) as i8,
        trigger_left: ctx.read_u8(at + 6),
        trigger_right: ctx.read_u8(at + 7),
        analog_a: ctx.read_u8(at + 8),
        analog_b: ctx.read_u8(at + 9),
        err: ctx.read_u8(at + 10) as i8,
    }
}

fn put_status(ctx: &Ctx, at: u32, s: &Status) {
    for (i, w) in [s.button, s.last_button, s.trigger, s.repeat, s.release]
        .iter()
        .enumerate()
    {
        ctx.write_u32(at + 4 * i as u32, *w);
    }
    ctx.write_u32(at + 0x14, s.repeat_count as u32);
    let bytes = [
        s.stick_x as u8,
        s.stick_y as u8,
        s.sub_stick_x as u8,
        s.sub_stick_y as u8,
        s.analog_l,
        s.analog_r,
        s.analog_a,
        s.analog_b,
    ];
    for (i, b) in bytes.iter().enumerate() {
        ctx.write_u8(at + 0x18 + i as u32, *b);
    }
    for (i, f) in nml(s).iter().enumerate() {
        ctx.write_u32(at + 0x20 + 4 * i as u32, f.to_bits());
    }
    ctx.write_u8(at + 0x40, s.cross_dir);
    ctx.write_u8(at + 0x41, s.err as u8);
}

fn nml(s: &Status) -> [f32; 8] {
    [
        s.nml_stick_x,
        s.nml_stick_y,
        s.nml_sub_stick_x,
        s.nml_sub_stick_y,
        s.nml_analog_l,
        s.nml_analog_r,
        s.nml_analog_a,
        s.nml_analog_b,
    ]
}

fn get_status(ctx: &Ctx, at: u32) -> Status {
    let b = |o: u32| ctx.read_u8(at + o);
    let f = |i: u32| f32::from_bits(ctx.read_u32(at + 0x20 + 4 * i));
    Status {
        button: ctx.read_u32(at),
        last_button: ctx.read_u32(at + 4),
        trigger: ctx.read_u32(at + 8),
        repeat: ctx.read_u32(at + 0xC),
        release: ctx.read_u32(at + 0x10),
        repeat_count: ctx.read_u32(at + 0x14) as i32,
        stick_x: b(0x18) as i8,
        stick_y: b(0x19) as i8,
        sub_stick_x: b(0x1A) as i8,
        sub_stick_y: b(0x1B) as i8,
        analog_l: b(0x1C),
        analog_r: b(0x1D),
        analog_a: b(0x1E),
        analog_b: b(0x1F),
        nml_stick_x: f(0),
        nml_stick_y: f(1),
        nml_sub_stick_x: f(2),
        nml_sub_stick_y: f(3),
        nml_analog_l: f(4),
        nml_analog_r: f(5),
        nml_analog_a: f(6),
        nml_analog_b: f(7),
        cross_dir: b(0x40),
        err: b(0x41) as i8,
    }
}

/// A status with its floats as bit patterns, for exact comparison.
fn status_bits(s: &Status) -> String {
    let mut plain = *s;
    for f in [
        &mut plain.nml_stick_x,
        &mut plain.nml_stick_y,
        &mut plain.nml_sub_stick_x,
        &mut plain.nml_sub_stick_y,
        &mut plain.nml_analog_l,
        &mut plain.nml_analog_r,
        &mut plain.nml_analog_a,
        &mut plain.nml_analog_b,
    ] {
        *f = 0.0;
    }
    let bits: Vec<String> = nml(s)
        .iter()
        .map(|f| format!("{:08x}", f.to_bits()))
        .collect();
    format!("{plain:?} nml {}", bits.join(" "))
}

fn lib_addr(ctx: &Ctx) -> u32 {
    let _ = ctx;
    sym("HSD_PadLibData")
}

/// ssbm-rs's `HSD_PadLibData` and its queue.
fn get_lib(ctx: &Ctx) -> PadLib {
    let at = lib_addr(ctx);
    let b = |o: u32| ctx.read_u8(at + o);
    let qnum = b(0);
    let queue_at = ctx.read_u32(at + 8);
    let queue = (0..u32::from(qnum))
        .map(|q| std::array::from_fn(|i| get_pad(ctx, queue_at + (4 * q + i as u32) * PAD_SIZE)))
        .collect();
    PadLib {
        qnum,
        qread: b(1),
        qwrite: b(2),
        qcount: b(3),
        qtype: b(4),
        queue,
        repeat_start: ctx.read_u32(at + 0xC) as i32,
        repeat_interval: ctx.read_u32(at + 0x10) as i32,
        adc_type: b(0x14) as i8,
        adc_th: b(0x15) as i8,
        adc_angle: f32::from_bits(ctx.read_u32(at + 0x18)),
        clamp_stick_type: b(0x1C),
        clamp_stick_shift: b(0x1D),
        clamp_stick_max: b(0x1E) as i8,
        clamp_stick_min: b(0x1F) as i8,
        clamp_analog_lr_shift: b(0x20),
        clamp_analog_lr_max: b(0x21),
        clamp_analog_lr_min: b(0x22),
        clamp_analog_ab_shift: b(0x23),
        clamp_analog_ab_max: b(0x24),
        clamp_analog_ab_min: b(0x25),
        scale_stick: b(0x26) as i8,
        scale_analog_lr: b(0x27),
        scale_analog_ab: b(0x28),
        cross_dir: b(0x29),
        reset_switch_status: b(0x2A),
        reset_switch: b(0x2B),
    }
}

/// Writes the library's settings (not the queue's position) into ssbm-rs's.
fn put_settings(ctx: &Ctx, p: &PadLib) {
    let at = lib_addr(ctx);
    ctx.write_u8(at + 4, p.qtype);
    ctx.write_u32(at + 0xC, p.repeat_start as u32);
    ctx.write_u32(at + 0x10, p.repeat_interval as u32);
    ctx.write_u8(at + 0x14, p.adc_type as u8);
    ctx.write_u8(at + 0x15, p.adc_th as u8);
    ctx.write_u32(at + 0x18, p.adc_angle.to_bits());
    let bytes = [
        p.clamp_stick_type,
        p.clamp_stick_shift,
        p.clamp_stick_max as u8,
        p.clamp_stick_min as u8,
        p.clamp_analog_lr_shift,
        p.clamp_analog_lr_max,
        p.clamp_analog_lr_min,
        p.clamp_analog_ab_shift,
        p.clamp_analog_ab_max,
        p.clamp_analog_ab_min,
        p.scale_stick as u8,
        p.scale_analog_lr,
        p.scale_analog_ab,
        p.cross_dir,
    ];
    for (i, v) in bytes.iter().enumerate() {
        ctx.write_u8(at + 0x1C + i as u32, *v);
    }
}

fn compare(ctx: &Ctx, ours: &Controllers, nodes: &rumble::Places, what: &dyn Fn() -> String) {
    assert_eq!(
        rumble::ours(&ours.rumble, nodes),
        rumble::theirs(ctx, nodes, ours.rumble.nodes.len()),
        "rumble after {}",
        what()
    );
    let mut theirs = get_lib(ctx);
    let mut mine = ours.lib.clone();
    assert_eq!(
        mine.adc_angle.to_bits(),
        theirs.adc_angle.to_bits(),
        "adc_angle after {}",
        what()
    );
    (mine.adc_angle, theirs.adc_angle) = (0.0, 0.0);
    assert_eq!(mine, theirs, "library after {}", what());
    for (name, sym_name, statuses) in [
        ("master", "HSD_PadMasterStatus", &ours.master),
        ("copy", "HSD_PadCopyStatus", &ours.copy),
        ("game", "HSD_PadGameStatus", &ours.game),
    ] {
        let at = sym(sym_name);
        for (i, s) in statuses.iter().enumerate() {
            let t = get_status(ctx, at + i as u32 * STATUS_SIZE);
            assert_eq!(
                status_bits(s),
                status_bits(&t),
                "{name} status {i} after {}",
                what()
            );
        }
    }
}

/// What the hardware stubs report and what they were asked.
#[derive(Default)]
struct Hw {
    poll: PadData,
    reset_pressed: bool,
    resets: Vec<u32>,
    recalibrations: Vec<u32>,
    busy: [bool; 4],
    status_ready: [bool; 4],
    response_ok: [bool; 4],
    data: [[u32; 2]; 4],
    si_type: [u32; 4],
    origin_reads: Vec<u32>,
}

thread_local! {
    static HW: RefCell<Hw> = RefCell::new(Hw::default());
}

fn hw<R>(f: impl FnOnce(&mut Hw) -> R) -> R {
    HW.with(|h| f(&mut h.borrow_mut()))
}

fn chan(ctx: &Ctx) -> usize {
    ctx.regs.r(3) as usize & 3
}

/// `PADRead` as HSD_PadRenewRawStatus sees it: this step's poll.
fn stub_pad_read(ctx: &Ctx) {
    let at = ctx.regs.r(3);
    let poll = hw(|h| h.poll);
    for (i, s) in poll.iter().enumerate() {
        put_pad(ctx, at + i as u32 * PAD_SIZE, s);
    }
    ctx.regs.set_r(3, 0);
}
fn stub_nothing(_: &Ctx) {}
fn stub_pad_reset(ctx: &Ctx) {
    let mask = ctx.regs.r(3);
    hw(|h| h.resets.push(mask));
    ctx.regs.set_r(3, 1);
}
fn stub_pad_recalibrate(ctx: &Ctx) {
    let mask = ctx.regs.r(3);
    hw(|h| h.recalibrations.push(mask));
    ctx.regs.set_r(3, 1);
}
fn stub_reset_switch(ctx: &Ctx) {
    let on = hw(|h| h.reset_pressed);
    ctx.regs.set_r(3, on as u32);
}
fn stub_si_busy(ctx: &Ctx) {
    let c = chan(ctx);
    ctx.regs.set_r(3, hw(|h| h.busy[c]) as u32);
}
fn stub_si_status(ctx: &Ctx) {
    let c = chan(ctx);
    ctx.regs
        .set_r(3, if hw(|h| h.status_ready[c]) { 8 } else { 0 });
}
fn stub_si_response(ctx: &Ctx) {
    let (c, at) = (chan(ctx), ctx.regs.r(4));
    let (data, ok) = hw(|h| (h.data[c], h.response_ok[c]));
    ctx.write_u32(at, data[0]);
    ctx.write_u32(at + 4, data[1]);
    ctx.regs.set_r(3, ok as u32);
}
fn stub_si_type(ctx: &Ctx) {
    let c = chan(ctx);
    ctx.regs.set_r(3, hw(|h| h.si_type[c]));
}
fn stub_si_transfer(ctx: &Ctx) {
    let c = ctx.regs.r(3);
    hw(|h| h.origin_reads.push(c));
    ctx.regs.set_r(3, 1);
}
fn stub_si_type_async(ctx: &Ctx) {
    ctx.regs.set_r(3, 0);
}

/// ssbm-rs with the hardware the pad code reaches replaced by stubs: below `PADRead` (the
/// serial interface) when checking it, or `PADRead` itself, reporting the step's poll.
fn stubbed(pad_read: bool) -> Option<Ctx> {
    let ctx = reference()?;
    if pad_read {
        ctx.register(sym("PADRead"), stub_pad_read);
    }
    let stubs: [(&str, ssbm_rt::Native); 11] = [
        ("PADReset", stub_pad_reset),
        ("PADRecalibrate", stub_pad_recalibrate),
        ("PADInit", stub_nothing),
        ("OSGetResetSwitchState", stub_reset_switch),
        ("SIIsChanBusy", stub_si_busy),
        ("SIGetStatus", stub_si_status),
        ("SIGetResponse", stub_si_response),
        ("SIGetType", stub_si_type),
        ("SITransfer", stub_si_transfer),
        ("SIGetTypeAsync", stub_si_type_async),
        ("PADControlMotor", rumble::stub_control_motor),
    ];
    for (name, f) in stubs {
        ctx.register(sym(name), f);
    }
    // The SDK state Melee never changes from what the DOL loads, which ours assumes.
    assert_eq!(ctx.read_u32(sym("XPatchBits")), 0xF000_0000, "XPatchBits");
    assert_eq!(ctx.read_u32(sym("AnalogMode")), 0x300, "AnalogMode");
    assert_eq!(
        ctx.read_u32(sym("MakeStatus")),
        sym("SPEC2_MakeStatus"),
        "MakeStatus"
    );
    Some(ctx)
}

fn random_pad(rng: &mut Rng) -> PadStatus {
    PadStatus {
        button: rng.next() as u16,
        stick_x: rng.axis(),
        stick_y: rng.axis(),
        substick_x: rng.axis(),
        substick_y: rng.axis(),
        trigger_left: rng.trigger(),
        trigger_right: rng.trigger(),
        analog_a: rng.trigger(),
        analog_b: rng.trigger(),
        err: rng.pick(&[0, 0, 0, -1, -2, -3, 1, 0x7F]),
    }
}

#[test]
fn sdk_clamp_matches() {
    let Some(ctx) = stubbed(true) else { return };
    let ctx = &ctx;
    let mut rng = Rng(0x1234_5678_9ABC_DEF1);
    let mut regions = vec![pad::STICK, pad::SUBSTICK];
    for _ in 0..300 {
        // ClampStick is only ever passed ClampRegion's constants; other regions keep the
        // diagonal and the reach positive, as those do, so its divisor is never 0.
        regions.push(pad::StickRegion {
            min: rng.i8(),
            max: 1 + rng.below(127) as i8,
            xy: 1 + rng.below(127) as i8,
        });
    }
    let mut points = 0;
    for (n, region) in regions.iter().enumerate() {
        let all = n < 2;
        for i in 0..65536u32 {
            if !all && i % 61 != 0 {
                continue;
            }
            let (x, y) = ((i >> 8) as u8 as i8, i as u8 as i8);
            ctx.write_u8(PADS, x as u8);
            ctx.write_u8(PADS + 1, y as u8);
            fns::ClampStick(
                ctx,
                handle(ctx, PADS),
                handle(ctx, PADS + 1),
                region.max,
                region.xy,
                region.min,
            );
            let theirs = (ctx.read_u8(PADS) as i8, ctx.read_u8(PADS + 1) as i8);
            assert_eq!(
                pad::clamp_stick(x, y, *region),
                theirs,
                "ClampStick {x} {y} {region:?}"
            );
            points += 1;
        }
    }
    for _ in 0..50_000 {
        let mut pads: [PadStatus; 4] = std::array::from_fn(|_| random_pad(&mut rng));
        for (i, p) in pads.iter().enumerate() {
            put_pad(ctx, PADS + i as u32 * PAD_SIZE, p);
        }
        let before = pads;
        fns::PADClamp(ctx, handle(ctx, PADS));
        pad::clamp(&mut pads);
        for (i, p) in pads.iter().enumerate() {
            assert_eq!(
                *p,
                get_pad(ctx, PADS + i as u32 * PAD_SIZE),
                "PADClamp {before:?}"
            );
        }
    }
    eprintln!("ClampStick: {points} points; PADClamp: 50000 polls");
}

#[test]
fn sdk_decoding_matches() {
    let Some(ctx) = stubbed(false) else { return };
    let ctx = &ctx;
    let origin_at = sym("Origin");
    let mut rng = Rng(0x0BAD_F00D_DEAD_BEEF);
    let si_types = [
        0x0900_0000,
        0x0900_FFFF,
        0x0920_0000,
        0x0800_0000,
        0x8B10_0000,
        0,
    ];
    for _ in 0..100_000 {
        // UpdateOrigin on a raw origin report.
        let c = rng.below(4) as usize;
        let raw = random_pad(&mut rng);
        let si_type = if rng.chance(70) {
            rng.pick(&si_types)
        } else {
            rng.next() as u32
        };
        hw(|h| h.si_type[c] = si_type);
        put_pad(ctx, origin_at + c as u32 * PAD_SIZE, &raw);
        fns::UpdateOrigin(ctx, c as i32);
        let origin = get_pad(ctx, origin_at + c as u32 * PAD_SIZE);
        assert_eq!(
            pad::update_origin(&raw, si_type),
            origin,
            "UpdateOrigin {raw:?} {si_type:#x}"
        );

        // SPEC2_MakeStatus against that origin, or any origin bytes.
        let origin = if rng.chance(50) {
            origin
        } else {
            random_pad(&mut rng)
        };
        put_pad(ctx, origin_at + c as u32 * PAD_SIZE, &origin);
        let data = [rng.next() as u32, rng.next() as u32];
        let mut ours = random_pad(&mut rng);
        put_pad(ctx, PADS, &ours);
        ctx.write_u32(DATA, data[0]);
        ctx.write_u32(DATA + 4, data[1]);
        fns::SPEC2_MakeStatus(ctx, c as i32, handle(ctx, PADS), handle(ctx, DATA));
        ours.decode(data, &origin);
        assert_eq!(
            ours,
            get_pad(ctx, PADS),
            "SPEC2_MakeStatus {data:x?} {origin:?}"
        );
    }

    // PADRead's decisions, port by port, against `pad::read`. The channel states it keeps
    // (EnabledBits and the rest) stand for what the input layer knows; PendingBits and a
    // response from a port not being probed lead into PADReset and PADDisable, which drive the
    // serial interface and stay out of this comparison.
    let pending = sym("PendingBits");
    let (enabled, resetting, waiting, checking) = (
        sym("EnabledBits"),
        sym("ResettingBits"),
        sym("WaitingBits"),
        sym("CheckingBits"),
    );
    let resetting_chan = sym("ResettingChan");
    for trial in 0..100_000 {
        let mut samples = [Sample::Waiting; 4];
        let mut origins = [PadStatus::default(); 4];
        let (mut en, mut rs, mut wt) = (0u32, 0u32, 0u32);
        let mut r_chan = 32;
        hw(|h| h.origin_reads.clear());
        for c in 0..4 {
            let bit = 0x8000_0000u32 >> c;
            origins[c] = pad::update_origin(&random_pad(&mut rng), rng.next() as u32);
            put_pad(ctx, origin_at + c as u32 * PAD_SIZE, &origins[c]);
            let mut data = [rng.next() as u32, rng.next() as u32];
            if rng.chance(70) {
                data[0] &= 0x7FFF_FFFF;
            }
            if rng.chance(70) {
                data[0] &= !0x2000_0000;
            }
            let (mut busy, mut ready, mut ok) = (false, false, true);
            samples[c] = match rng.below(8) {
                0 => Sample::Disconnected,
                1 => {
                    if r_chan != 32 || rng.chance(50) {
                        rs |= bit;
                    } else {
                        r_chan = c as i32;
                    }
                    Sample::NotReady
                }
                2 => {
                    busy = true;
                    Sample::Transfer
                }
                3 => {
                    ok = false;
                    Sample::Transfer
                }
                4 => {
                    ready = true;
                    wt |= bit;
                    Sample::Waiting
                }
                _ => Sample::Response(data),
            };
            if samples[c] != Sample::Disconnected {
                en |= bit;
            }
            hw(|h| {
                h.busy[c] = busy;
                h.status_ready[c] = ready;
                h.response_ok[c] = ok;
                h.data[c] = data;
                h.si_type[c] = rng.next() as u32;
            });
        }
        ctx.write_u32(pending, 0);
        ctx.write_u32(enabled, en);
        ctx.write_u32(resetting, rs);
        ctx.write_u32(waiting, wt);
        ctx.write_u32(checking, if rng.chance(50) { 0 } else { wt });
        ctx.write_u32(resetting_chan, r_chan as u32);
        for c in 0..4 {
            put_pad(ctx, PADS + c * PAD_SIZE, &random_pad(&mut rng));
        }
        let _ = fns::PADRead(ctx, handle(ctx, PADS));
        let origin_reads = hw(|h| h.origin_reads.clone());
        for c in 0..4 {
            let ours = pad::read(samples[c], &origins[c]);
            let theirs = get_pad(ctx, PADS + c as u32 * PAD_SIZE);
            assert_eq!(
                ours.status, theirs,
                "PADRead port {c} {:?} trial {trial}",
                samples[c]
            );
            assert_eq!(
                ours.read_origin,
                origin_reads.contains(&(c as u32)),
                "PADRead origin request port {c} {:?}",
                samples[c]
            );
        }
    }
    eprintln!("UpdateOrigin, SPEC2_MakeStatus: 100000 each; PADRead: 100000 polls");
}

#[test]
fn pad_library_helpers_match() {
    let Some(ctx) = stubbed(true) else { return };
    let ctx = &ctx;
    let mut rng = Rng(0x5EED_0FC0_FFEE);
    let lib_at = lib_addr(ctx);

    // HSD_PadClampCheck1: every value against random limits.
    for _ in 0..2_000 {
        let (shift, min, max) = (rng.u8_of(&[0, 1, 1]), rng.trigger(), rng.trigger());
        for v in 0..=255u8 {
            ctx.write_u8(PADS, v);
            fns::HSD_PadClampCheck1(ctx, handle(ctx, PADS), shift, min, max);
            assert_eq!(
                controller::clamp_analog(v, shift, min, max),
                ctx.read_u8(PADS),
                "ClampCheck1 {v} {shift} {min} {max}"
            );
        }
    }

    // HSD_PadClampCheck3 and HSD_PadADConvertCheck1: every stick position against Melee's
    // settings, the library's defaults, every reach, every dead zone within Melee's reach, and
    // random settings. A rounding change in the clamp shows at only a few reaches.
    let mut params: Vec<(u8, i8, i8)> = vec![(1, 0, 80), (0, 0, 127), (1, 15, 80), (1, 30, 127)];
    for _ in 0..8 {
        params.push((rng.u8_of(&[0, 1]), rng.i8(), rng.i8()));
    }
    for m in 1..=127 {
        params.push((0, 0, m));
    }
    for m in 1..80 {
        params.push((1, m, 80));
    }
    let mut adc: Vec<(i8, f32)> = vec![(30, 0.0), (0, 0.0), (30, std::f32::consts::FRAC_PI_4)];
    for _ in 0..8 {
        adc.push((rng.axis(), rng.angle()));
    }
    let dirs = [0x10, 0x20, 0x40, 0x80];
    for mode in MODES {
        set_fp_mode(mode);
        for &(shift, min, max) in &params {
            for i in 0..65536u32 {
                let (x, y) = ((i >> 8) as u8 as i8, i as u8 as i8);
                ctx.write_u8(PADS, x as u8);
                ctx.write_u8(PADS + 1, y as u8);
                fns::HSD_PadClampCheck3(
                    ctx,
                    handle(ctx, PADS),
                    handle(ctx, PADS + 1),
                    shift,
                    min,
                    max,
                );
                let theirs = (ctx.read_u8(PADS) as i8, ctx.read_u8(PADS + 1) as i8);
                assert_eq!(
                    controller::clamp_stick(x, y, shift, min, max),
                    theirs,
                    "ClampCheck3 {x} {y} {shift} {min} {max} {mode:?}"
                );
            }
        }
        for &(th, angle) in &adc {
            let mut lib = Controllers::new(1, 0).lib;
            lib.adc_th = th;
            lib.adc_angle = angle;
            ctx.write_u8(lib_at + 0x15, th as u8);
            ctx.write_u32(lib_at + 0x18, angle.to_bits());
            for i in 0..65536u32 {
                let (x, y) = ((i >> 8) as u8 as i8, i as u8 as i8);
                let button = (rng.next() as u32) & 0xFFFF_FF0F;
                let mut s = Status::DEFAULT;
                s.button = button;
                put_status(ctx, PADS, &s);
                fns::HSD_PadADConvertCheck1(
                    ctx,
                    handle(ctx, PADS),
                    x,
                    y,
                    dirs[0],
                    dirs[1],
                    dirs[2],
                    dirs[3],
                );
                assert_eq!(
                    lib.directions(button, x, y, dirs),
                    ctx.read_u32(PADS),
                    "ADConvertCheck1 {x} {y} th {th} angle {angle:e} {mode:?}"
                );
            }
        }
    }
    set_fp_mode(FpMode::Hardware);

    // HSD_PadCrossDir.
    for _ in 0..100_000 {
        let mut lib = Controllers::new(1, 0).lib;
        lib.cross_dir = rng.u8_of(&[0, 1, 2, 3, 3]);
        ctx.write_u8(lib_at + 0x29, lib.cross_dir);
        let mut s = Status::DEFAULT;
        s.button = rng.next() as u32;
        if rng.chance(50) {
            s.button &= !0xF | (rng.next() as u32 & 0xF);
        }
        s.cross_dir = rng.u8_of(&[0, 1, 2]);
        put_status(ctx, PADS, &s);
        fns::HSD_PadCrossDir(ctx, handle(ctx, PADS));
        let before = s;
        lib.cross_dir(&mut s);
        assert_eq!(
            status_bits(&s),
            status_bits(&get_status(ctx, PADS)),
            "CrossDir {before:?} {}",
            lib.cross_dir
        );
    }
}

/// One poll as PADRead reports it: a port with an error has its inputs cleared, a port with
/// a status has a button bit PADRead leaves (it clears 0x80 and turns 0x2000 into an error).
/// A sequence holds buttons from the last poll `hold` percent of the time, so key repeat starts,
/// and may report errors.
fn random_poll(rng: &mut Rng, last: &PadData, hold: u64, errors: bool) -> PadData {
    std::array::from_fn(|i| {
        let err = if errors {
            rng.pick(&[0, 0, 0, 0, 0, 0, -1, -2, -3])
        } else {
            0
        };
        if err != 0 {
            return PadStatus::error(err);
        }
        let mut s = random_pad(rng);
        s.err = 0;
        s.button &= 0x1F7F;
        if rng.chance(hold) {
            s.button = last[i].button;
        }
        if rng.chance(70) {
            // Analog A and B read 0 in analog mode 3; any value now and then.
            (s.analog_a, s.analog_b) = (0, 0);
        }
        s
    })
}

fn random_settings(rng: &mut Rng, lib: &mut PadLib) {
    lib.qtype = rng.u8_of(&[0, 0, 1, 2, 3]);
    lib.repeat_start = rng.pick(&[45, 1, 2, 5, 0, -1]);
    lib.repeat_interval = rng.pick(&[8, 1, 3, 0]);
    lib.adc_type = rng.i8_of(&[0, 0, 1]);
    lib.adc_th = rng.axis();
    lib.adc_angle = rng.angle();
    lib.clamp_stick_type = rng.pick(&[0, 0, 1]);
    lib.clamp_stick_shift = rng.u8_of(&[0, 1]);
    lib.clamp_stick_max = rng.axis();
    lib.clamp_stick_min = rng.axis();
    lib.clamp_analog_lr_shift = rng.u8_of(&[0, 1]);
    lib.clamp_analog_lr_max = rng.trigger();
    lib.clamp_analog_lr_min = rng.trigger();
    lib.clamp_analog_ab_shift = rng.u8_of(&[0, 1]);
    lib.clamp_analog_ab_max = rng.trigger();
    lib.clamp_analog_ab_min = rng.trigger();
    lib.scale_stick = rng.axis();
    lib.scale_analog_lr = rng.trigger();
    lib.scale_analog_ab = rng.trigger();
    lib.cross_dir = rng.u8_of(&[0, 1, 2, 3]);
}

/// Slippi's playback codes' patches to the pad library (`HSD_PadRenewRawStatus+0x8C`, the
/// word, and `HSD_PadRenewMasterStatus+0x38`, an injection), from playback's code set.
fn playback_codes() -> Vec<ssbm_slippi::gecko::Code> {
    let set = ssbm_slippi::gecko::parse_ini(ssbm_slippi::PLAYBACK_INI);
    let lines: Vec<(u32, u32)> = set.iter().flat_map(|c| c.lines.clone()).collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let (a, d) = lines[i];
        let n = if a >> 24 & 0xFE == 0xC2 {
            d as usize
        } else {
            0
        };
        let site = 0x8000_0000 | (a & 0x01FF_FFFF);
        if site == 0x8037_6A88 || site == 0x8037_7544 {
            out.push(ssbm_slippi::gecko::Code {
                name: format!("{site:#x}"),
                lines: lines[i..=i + n].to_vec(),
            });
        }
        i += 1 + n;
    }
    assert_eq!(out.len(), 2, "playback's pad library patches");
    out
}

/// Puts playback's pad library patches in the reference's code, or the game's own words back,
/// and has the reference run the ports that go with them.
fn set_playback(ctx: &Ctx, on: bool) {
    if on {
        ssbm_slippi::gecko::apply(ctx, &playback_codes(), 0x8000_2000);
    } else {
        // `lbz r0, 0x2(r31)` and `beq .L_80377998`.
        ctx.write_u32(0x8037_6A88, 0x881F_0002);
        ctx.write_u32(0x8037_7544, 0x4182_0454);
    }
    leaf_check::register_patched(ctx);
}

#[test]
fn pad_library_sequences_match() {
    let Some(ctx) = stubbed(true) else { return };
    let ctx = &ctx;
    let mut rng = Rng(0xA5A5_5A5A_0123_4567);
    let game_queue = sym("gmMain_8046B108");
    let game_nodes = sym("gmMain_8046B1F8");
    // The scripts the rumble menu and a few priorities give; tests/rumble.rs runs the game
    // data's.
    let scripts = [
        (Script::table(Table::Vibration), 0),
        (Script::table(Table::Vibration), 3),
    ];
    let data = Archives::default();
    let (mut steps, mut sequences) = (0u64, 0u64);
    // How often the paths that need a run of polls were taken.
    let (mut held_repeats, mut full_queue, mut directions, mut kept) = (0u64, 0u64, 0u64, 0u64);
    let mut motors = 0u64;
    for seq in 0..400u32 {
        let mode = MODES[seq as usize % 2];
        set_fp_mode(mode);
        // The queue's and rumble nodes' memory is zero from boot, and HSD_PadInit runs once;
        // clear them so what's left from the last sequence doesn't show in the comparison.
        let game = seq % 3 != 2;
        let mut nodes = rumble::Places {
            nodes: if game { game_nodes } else { NODES },
            archives: Vec::new(),
        };
        rumble::clear_nodes(ctx, nodes.nodes, 12);
        // Every fourth sequence on the game's queue runs with playback's patches.
        let playback = game && seq % 4 == 1;
        set_playback(ctx, playback);
        let mut ours = if game {
            for a in (0..0xF0).step_by(4) {
                ctx.write_u32(game_queue + a, 0);
            }
            fns::gmMain_8015FD24(ctx);
            let mut c = Controllers::melee();
            c.playback = playback;
            c
        } else {
            let qnum = 1 + rng.below(5) as u8;
            for a in (0..0x100).step_by(4) {
                ctx.write_u32(QUEUE + a, 0);
            }
            // Without nodes the list pointer is null, as ours has none to point at.
            let nb_list = rng.below(13) as u16;
            if nb_list == 0 {
                nodes.nodes = 0;
            }
            fns::HSD_PadInit(
                ctx,
                qnum,
                handle(ctx, QUEUE),
                nb_list,
                handle(ctx, nodes.nodes),
            );
            Controllers::new(qnum, nb_list)
        };
        let nodes = &nodes;
        let compare = |ctx: &Ctx, ours: &Controllers, what: &dyn Fn() -> String| {
            compare(ctx, ours, nodes, what)
        };
        compare(ctx, &ours, &|| format!("init, sequence {seq}"));
        if seq % 4 == 3 {
            random_settings(&mut rng, &mut ours.lib);
            put_settings(ctx, &ours.lib);
        }
        let mut last = [PadStatus::default(); 4];
        let hold = rng.pick(&[85, 97, 100]);
        let errors = rng.chance(60);
        let length = 100 + rng.below(400);
        for step in 0..length {
            let op = rng.below(115);
            let what = |name: &str| format!("{name}, sequence {seq} step {step} {mode:?}");
            match op {
                0..=34 => {
                    let poll = random_poll(&mut rng, &last, hold, errors);
                    last = poll;
                    let err_check = rng.chance(20);
                    let reset = rng.chance(10);
                    hw(|h| {
                        h.poll = poll;
                        h.reset_pressed = reset;
                        h.resets.clear();
                    });
                    full_queue += u64::from(ours.lib.qcount == ours.lib.qnum);
                    fns::HSD_PadRenewRawStatus(ctx, err_check as i32);
                    let req = ours.renew_raw(&data, poll, err_check, reset);
                    let resets = hw(|h| h.resets.clone());
                    let expected: Vec<u32> = if req.reset != 0 {
                        vec![req.reset]
                    } else {
                        vec![]
                    };
                    assert_eq!(expected, resets, "{}", what("RenewRawStatus mask"));
                    assert_eq!(
                        rumble::as_commands(req.motors),
                        rumble::take_motor_commands(),
                        "{}",
                        what("RenewRawStatus motors")
                    );
                    motors += req.motors.iter().flatten().count() as u64;
                    compare(ctx, &ours, &|| {
                        format!("{} {poll:?}", what("RenewRawStatus"))
                    });
                }
                35..=59 => {
                    if ours.lib.qcount != 0 {
                        let poll = ours.lib.queue[usize::from(ours.lib.qread)];
                        kept += poll.iter().filter(|s| s.err == -3).count() as u64;
                    }
                    fns::HSD_PadRenewMasterStatus(ctx);
                    ours.renew_master();
                    directions += ours
                        .master
                        .iter()
                        .filter(|s| s.button & 0xFF_0000 != 0)
                        .count() as u64;
                    compare(ctx, &ours, &|| what("RenewMasterStatus"));
                }
                60..=67 => {
                    fns::HSD_PadRenewCopyStatus(ctx);
                    ours.renew_copy();
                    compare(ctx, &ours, &|| what("RenewCopyStatus"));
                }
                68..=75 => {
                    fns::HSD_PadRenewGameStatus(ctx);
                    ours.renew_game();
                    compare(ctx, &ours, &|| what("RenewGameStatus"));
                }
                76..=87 => {
                    let poll = random_poll(&mut rng, &last, hold, errors);
                    last = poll;
                    let reset = rng.chance(10);
                    hw(|h| {
                        h.poll = poll;
                        h.reset_pressed = reset;
                        h.resets.clear();
                    });
                    fns::HSD_PadRenewStatus(ctx);
                    let req = ours.renew(&data, poll, reset);
                    let resets = hw(|h| h.resets.clone());
                    let expected: Vec<u32> = if req.reset != 0 {
                        vec![req.reset]
                    } else {
                        vec![]
                    };
                    assert_eq!(expected, resets, "{}", what("RenewStatus mask"));
                    assert_eq!(
                        rumble::as_commands(req.motors),
                        rumble::take_motor_commands(),
                        "{}",
                        what("RenewStatus motors")
                    );
                    motors += req.motors.iter().flatten().count() as u64;
                    compare(ctx, &ours, &|| what("RenewStatus"));
                }
                88..=93 => {
                    let (how, ftype) = rng.pick(&[
                        (Flush::Merge, 0),
                        (Flush::Throwaway, 1),
                        (Flush::Leave1, 2),
                        (Flush::Terminate, 3),
                    ]);
                    fns::HSD_PadFlushQueue(ctx, ftype);
                    ours.flush_queue(how);
                    compare(ctx, &ours, &|| what(&format!("FlushQueue {how:?}")));
                }
                94..=96 => {
                    assert_eq!(
                        fns::HSD_PadGetRawQueueCount(ctx),
                        ours.raw_queue_count(),
                        "{}",
                        what("GetRawQueueCount")
                    );
                    assert_eq!(
                        fns::HSD_PadGetResetSwitch(ctx) != 0,
                        ours.reset_switch(),
                        "{}",
                        what("GetResetSwitch")
                    );
                }
                97..=99 => {
                    hw(|h| h.recalibrations.clear());
                    fns::HSD_PadReset(ctx);
                    ours.reset();
                    assert_eq!(
                        hw(|h| h.recalibrations.clone()),
                        vec![0xF000_0000],
                        "{}",
                        what("Reset recalibration")
                    );
                    compare(ctx, &ours, &|| what("Reset"));
                }
                _ => {
                    let mut rand = || rng.next();
                    let op = rumble::random_op(&mut rand, &scripts);
                    let name = format!("{op:?}");
                    rumble::apply(ctx, &mut ours.rumble, nodes, op, &|| what(&name));
                    compare(ctx, &ours, &|| what(&name));
                }
            }
            for s in ours.master.iter().chain(&ours.copy).chain(&ours.game) {
                held_repeats += u64::from(s.repeat != 0 && s.trigger == 0);
            }
            steps += 1;
        }
        sequences += 1;
    }
    set_fp_mode(FpMode::Hardware);
    eprintln!(
        "pad library: {sequences} sequences, {steps} steps; held repeats {held_repeats}, polls \
         into a full queue {full_queue}, statuses with stick directions {directions}, failed \
         transfers kept {kept}, motor commands {motors}"
    );
    assert!(motors > 0, "no poll drove a motor");
}
