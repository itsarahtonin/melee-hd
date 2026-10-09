//! melee-sim's expression bytecode against ssbm-rs's `HSD_ByteCodeEval`, in both float modes:
//! the same result and the same random seed after every program.
//!
//! The disc holds little bytecode: the only RObj expressions in bytecode form are in
//! `ItCo.dat`, and they all run one program (return the first argument). Most of the check
//! runs programs built from the opcode table instead: random stack-safe sequences over
//! every instruction, with random immediates, arguments and seeds.

use leaf_check::fp::{FpMode, set_fp_mode};
use leaf_check::{Rand, dat, handle, load_archive, reference, sym, write_f32s};
use melee_data::archive::{Archive, Archives, DataRef};
use melee_data::disc::Disc;
use melee_sim::hsd::bytecode;
use melee_sim::rng::Rng;
use ssbm_rt::{Addr, Ctx};
use ssbm_types::fns;

const OBJ_HEAP: u32 = 0x8100_0000;
/// Where the reference reads the arguments from.
const ARGS: u32 = leaf_check::SCRATCH;

/// The reference's list pool, which its evaluator keeps its stack in.
fn init_lists(ctx: &Ctx) {
    fns::HSD_ObjSetHeap(ctx, 0x0040_0000, handle::<Addr>(ctx, OBJ_HEAP));
    fns::HSD_ListInitAllocData(ctx);
}

/// Runs one program on both sides and compares the results and the seeds.
fn check(
    ctx: &Ctx,
    archives: &Archives,
    code: DataRef,
    code_addr: u32,
    args: &[f32],
    seed: u32,
    what: &str,
) {
    write_f32s(ctx, ARGS, args);
    let seed_at = sym("seed");
    ctx.write_u32(seed_at, seed);
    let theirs = fns::HSD_ByteCodeEval(
        ctx,
        handle(ctx, code_addr),
        handle(ctx, ARGS),
        args.len() as i32,
    );
    let mut rng = Rng::new(seed);
    let ours = bytecode::eval(archives, &mut rng, Some(code), args);
    assert_eq!(
        ours.to_bits(),
        leaf_check::bits(theirs),
        "{what} args {args:?} seed {seed:#x}"
    );
    assert_eq!(rng.seed, ctx.read_u32(seed_at), "{what}: seed after");
}

/// A program's length, or `None` for bytecode the evaluator rejects: it underflows, reads an
/// argument past `nb_args`, has an unknown opcode or doesn't return. Conditional jumps are
/// followed into the code they may skip.
fn validate(b: &[u8], nb_args: u32) -> Option<usize> {
    let (mut i, mut depth) = (0usize, 0usize);
    while i < b.len() && i < 4096 {
        let op = b[i];
        i += 1;
        let operand = |n: usize| {
            b.get(i..i + n)
                .map(|s| s.iter().fold(0usize, |a, &x| a << 8 | usize::from(x)))
        };
        match op {
            0 => {}
            1 => return (depth >= 1).then_some(i),
            2 => {
                if operand(2)? >= nb_args as usize {
                    return None;
                }
                depth += 1;
                i += 2;
            }
            3 => {
                depth = depth.checked_sub(1)?;
                i += 2;
            }
            4 => i += 2 + operand(2)?,
            5 => {
                depth = depth.saturating_sub(operand(1)?);
                i += 1;
            }
            6 => {
                depth += 1;
                i += 4;
            }
            0x3C => {
                if operand(1)? >= depth {
                    return None;
                }
                depth += 1;
                i += 1;
            }
            7..=0x16 | 0x28 | 0x31 => {
                depth.checked_sub(1)?;
            }
            0x17..=0x27 | 0x29..=0x30 | 0x32..=0x3B => depth = depth.checked_sub(2)? + 1,
            _ => return None,
        }
    }
    None
}

/// The bytecode programs the disc's RObj expressions run: RObjDescs of type bytecode (in
/// any file, reached through any pointer) whose bytecode validates against their argument
/// count. By file: (name, the file, [(program offset, argument count)]).
type DiscPrograms = Vec<(String, Vec<u8>, Vec<(u32, u32)>)>;

fn disc_programs(disc: &Disc) -> DiscPrograms {
    let mut out = Vec::new();
    for name in disc.files().unwrap() {
        if !(name.ends_with(".dat") || name.ends_with(".usd")) {
            continue;
        }
        let bytes = disc.read_file(&name).unwrap();
        let Ok(a) = Archive::parse(&bytes) else {
            continue;
        };
        let mut found = Vec::new();
        let len = a.data().len() as u32;
        let mut targets: Vec<u32> = (0..len / 4)
            .map(|i| i * 4)
            .filter(|&o| a.is_pointer(o))
            .map(|o| a.u32(o))
            .filter(|&t| t % 4 == 0 && t + 12 <= len)
            .collect();
        targets.sort_unstable();
        targets.dedup();
        for t in targets {
            // HSD_RObjDesc: next, flags (type in 0x70000000; 0x30000000 is bytecode), and
            // HSD_ByteCodeExpDesc { bytecode, rvalue list }.
            if a.u32(t + 4) & 0x7000_0000 != 0x3000_0000 || (a.u32(t) != 0 && !a.is_pointer(t)) {
                continue;
            }
            let Some(bc) = a.ptr(t + 8).filter(|&bc| bc + 8 <= len) else {
                continue;
            };
            let Some(code) = a.ptr(bc) else { continue };
            let mut nb_args = 0;
            if let Some(mut rv) = a.ptr(bc + 4) {
                while rv + 8 <= len && a.ptr(rv + 4).is_some() {
                    nb_args += a.u32(rv).count_ones();
                    rv += 8;
                }
            }
            if validate(&a.data()[code as usize..], nb_args).is_some() {
                found.push((code, nb_args));
            }
        }
        if !found.is_empty() {
            out.push((name, bytes, found));
        }
    }
    out
}

#[test]
fn disc_bytecode_matches() {
    let Some(ctx) = reference() else { return };
    let ctx = &ctx;
    init_lists(ctx);
    let disc = Disc::open(std::env::var("SSBM_DISC").unwrap()).unwrap();
    let files = disc_programs(&disc);
    let mut r = Rand(0x9E37_79B9_7F4A_7C15);
    let (mut programs, mut runs) = (0, 0);
    for (name, bytes, found) in &files {
        let data_addr = load_archive(ctx, bytes);
        let mut archives = Archives::default();
        let id = archives.add(Archive::parse(bytes).unwrap());
        for &(code, nb_args) in found {
            let code_ref = DataRef {
                archive: id,
                offset: code,
            };
            for mode in [FpMode::Hardware, FpMode::Slippi] {
                set_fp_mode(mode);
                for _ in 0..200 {
                    let args: Vec<f32> = (0..nb_args).map(|_| value(&mut r)).collect();
                    let seed = r.bits() as u32;
                    let what = format!("{name} {code:#x} in {mode:?}");
                    check(
                        ctx,
                        &archives,
                        code_ref,
                        data_addr + code,
                        &args,
                        seed,
                        &what,
                    );
                    runs += 1;
                }
            }
            programs += 1;
        }
    }
    set_fp_mode(FpMode::Hardware);
    eprintln!("{programs} disc programs, {runs} runs");
    assert!(programs >= 16, "only {programs} programs on the disc");
}

/// A float worth computing with: special values, small integers, angles, values in range,
/// and random bit patterns.
fn value(r: &mut Rand) -> f32 {
    const SPECIAL: [f32; 14] = [
        0.0,
        -0.0,
        1.0,
        -1.0,
        0.5,
        90.0,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
        f32::MIN_POSITIVE,
        1.0e-40,
        f32::MAX,
        -f32::MAX,
        3.0,
    ];
    match r.below(6) {
        0 => SPECIAL[r.below(SPECIAL.len() as u64) as usize],
        1 => f32::from_bits(r.bits() as u32),
        2 => r.range(-360.0, 360.0),
        3 => r.range(-1.5, 1.5),
        4 => (r.below(21) as i32 - 10) as f32,
        _ => r.range(-1.0e4, 1.0e4),
    }
}

/// An immediate word: a float or an integer.
fn word(r: &mut Rand) -> u32 {
    match r.below(4) {
        0 => (r.below(41) as i32 - 20) as u32,
        1 => r.bits() as u32,
        _ => value(r).to_bits(),
    }
}

/// A program builder that keeps the stack deep enough for every instruction, so the
/// evaluator never asserts. It counts the instructions it emits.
struct Gen<'a> {
    r: &'a mut Rand,
    code: Vec<u8>,
    depth: usize,
    nb_args: u32,
    used: &'a mut [u64; 256],
}

/// Instructions that replace the top value, and those that combine the top two (expf and
/// powf, 0x14 and 0x21, come with their own operands).
const UNARY: [u8; 17] = [
    7, 8, 9, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x15, 0x16, 0x28, 0x31,
];
const BINARY: [u8; 34] = [
    0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F, 0x20, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27,
    0x29, 0x2A, 0x2B, 0x2C, 0x2D, 0x2E, 0x2F, 0x30, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39,
    0x3A, 0x3B,
];

impl Gen<'_> {
    fn op(&mut self, op: u8) {
        self.used[op as usize] += 1;
        self.code.push(op);
    }

    fn push_word(&mut self, w: u32) {
        self.op(6);
        self.code.extend_from_slice(&w.to_be_bytes());
        self.depth += 1;
    }

    fn push(&mut self) {
        if self.nb_args > 0 && self.r.below(2) == 0 {
            let a = self.r.below(u64::from(self.nb_args)) as u16;
            self.op(2);
            self.code.extend_from_slice(&a.to_be_bytes());
            self.depth += 1;
        } else {
            let w = word(self.r);
            self.push_word(w);
        }
    }

    /// One instruction (or a short group) that leaves the stack at least as deep as `floor`.
    fn step(&mut self, floor: usize) {
        match self.r.below(16) {
            0..=2 => self.push(),
            3 => self.op(0),
            4 | 5 if self.depth > floor => {
                let op = UNARY[self.r.below(UNARY.len() as u64) as usize];
                self.op(op);
            }
            6..=8 if self.depth > floor + 1 => {
                let op = BINARY[self.r.below(BINARY.len() as u64) as usize];
                self.op(op);
                self.depth -= 1;
            }
            // expf and powf on values where they return (see tests/math.rs: their series
            // never settle for some inputs, which the game doesn't pass).
            9 => {
                let x = self.r.range(-12.0, 12.0);
                self.push_word(x.to_bits());
                self.op(0x14);
            }
            10 => {
                let x = self.r.range(0.05, 20.0);
                let y = self.r.range(-3.0, 3.0);
                self.push_word(x.to_bits());
                self.push_word(y.to_bits());
                self.op(0x21);
                self.depth -= 1;
            }
            11 if self.depth > 0 => {
                let n = self.r.below(self.depth as u64) as u8;
                self.op(0x3C);
                self.code.push(n);
                self.depth += 1;
            }
            12 if self.depth > floor => {
                let n = self.r.below((self.depth - floor) as u64 + 1) as u8;
                self.op(5);
                self.code.push(n);
                self.depth -= n as usize;
            }
            // Popping past the bottom does nothing.
            12 if self.depth == 0 => {
                self.op(5);
                self.code.push(self.r.below(3) as u8);
            }
            13 if self.depth > floor => {
                // Skip a block that leaves the stack as it was, when the popped value is
                // nonzero.
                self.op(3);
                self.depth -= 1;
                let block = self.block();
                self.code
                    .extend_from_slice(&(block.len() as u16).to_be_bytes());
                self.code.extend_from_slice(&block);
            }
            14 => {
                // Jump over bytes that never run.
                self.op(4);
                let n = self.r.below(6) as u16;
                self.code.extend_from_slice(&n.to_be_bytes());
                for _ in 0..n {
                    self.code.push(self.r.bits() as u8);
                }
            }
            _ => self.push(),
        }
    }

    /// A stack-neutral block for a conditional jump to skip or run.
    fn block(&mut self) -> Vec<u8> {
        let (start, depth) = (std::mem::take(&mut self.code), self.depth);
        for _ in 0..self.r.below(6) {
            self.step(depth);
        }
        while self.depth > depth {
            self.op(5);
            self.code.push(1);
            self.depth -= 1;
        }
        std::mem::replace(&mut self.code, start)
    }
}

/// A random valid program over `nb_args` arguments.
fn program(r: &mut Rand, nb_args: u32, used: &mut [u64; 256]) -> Vec<u8> {
    let mut g = Gen {
        r,
        code: Vec::new(),
        depth: 0,
        nb_args,
        used,
    };
    for _ in 0..1 + g.r.below(30) {
        g.step(0);
    }
    if g.depth == 0 {
        g.push();
    }
    g.op(1);
    g.code
}

#[test]
fn built_bytecode_matches() {
    let Some(ctx) = reference() else { return };
    let ctx = &ctx;
    init_lists(ctx);
    let mut r = Rand(0x2545_F491_4F6C_DD1D);
    let mut used = [0u64; 256];
    let mut runs = 0;
    for batch in 0..40 {
        // A batch of programs in one archive: each starts at a word boundary.
        let mut data = Vec::new();
        let mut programs = Vec::new();
        for _ in 0..250 {
            let nb_args = r.below(5) as u32;
            let p = program(&mut r, nb_args, &mut used);
            assert!(
                validate(&p, nb_args).is_some(),
                "built an invalid program {p:02x?}"
            );
            programs.push((data.len() as u32, nb_args, p.clone()));
            data.extend_from_slice(&p);
            data.resize(data.len().next_multiple_of(4), 0);
        }
        let file = dat(&data, &[]);
        let data_addr = load_archive(ctx, &file);
        let mut archives = Archives::default();
        let id = archives.add(Archive::parse(&file).unwrap());
        let mode = if batch % 2 == 0 {
            FpMode::Hardware
        } else {
            FpMode::Slippi
        };
        set_fp_mode(mode);
        for (at, nb_args, p) in &programs {
            for _ in 0..3 {
                let args: Vec<f32> = (0..*nb_args).map(|_| value(&mut r)).collect();
                let seed = r.bits() as u32;
                check(
                    ctx,
                    &archives,
                    DataRef {
                        archive: id,
                        offset: *at,
                    },
                    data_addr + at,
                    &args,
                    seed,
                    &format!("{p:02x?} in {mode:?}"),
                );
                runs += 1;
            }
        }
    }
    set_fp_mode(FpMode::Hardware);
    // A null program returns 0.
    assert_eq!(
        bytecode::eval(&Archives::default(), &mut Rng::default(), None, &[]).to_bits(),
        leaf_check::bits(fns::HSD_ByteCodeEval(
            ctx,
            handle(ctx, 0),
            handle(ctx, ARGS),
            0
        ))
    );
    for op in 0..=0x3Cu8 {
        assert!(
            used[op as usize] >= 100,
            "opcode {op:#x} ran only {} times",
            used[op as usize]
        );
    }
    eprintln!("{runs} runs");
}
