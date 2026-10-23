//! Reading the game's structs out of ssbm-rs's memory and comparing them with melee-sim's,
//! for the oracle and the tests. `structgen` generates each struct's [`Mirror`].

use melee_data::archive::DataRef;

use super::handles::CodeRef;
use crate::quat::Quat;
use crate::vec::Vec3;

/// A run of big-endian memory, as the game keeps it, at address `base`.
pub struct Mem<'a> {
    pub bytes: &'a [u8],
    pub base: u32,
}

impl Mem<'_> {
    fn at(&self, a: u32, n: usize) -> &[u8] {
        let i = a.wrapping_sub(self.base) as usize;
        &self.bytes[i..i + n]
    }

    pub fn u8(&self, a: u32) -> u8 {
        self.at(a, 1)[0]
    }

    pub fn u16(&self, a: u32) -> u16 {
        u16::from_be_bytes(self.at(a, 2).try_into().unwrap())
    }

    pub fn u32(&self, a: u32) -> u32 {
        u32::from_be_bytes(self.at(a, 4).try_into().unwrap())
    }

    pub fn u64(&self, a: u32) -> u64 {
        u64::from_be_bytes(self.at(a, 8).try_into().unwrap())
    }

    pub fn bytes<const N: usize>(&self, a: u32) -> [u8; N] {
        self.at(a, N).try_into().unwrap()
    }

    /// A bitfield of `bits` bits, `bit` bits from the start of the struct at `at`, counted from
    /// each byte's most significant bit as the compiler allocates them.
    pub fn bits(&self, at: u32, bit: u32, bits: u32) -> u32 {
        let mut v = 0u32;
        for b in bit..bit + bits {
            let byte = self.u8(at + b / 8);
            v = v << 1 | u32::from(byte >> (7 - b % 8) & 1);
        }
        v
    }
}

/// How ssbm-rs's pointers relate to melee-sim's handles and game data, which only the caller
/// knows (which joint is which, where archives were loaded), and where differences go.
pub trait Cx {
    /// The handle for one of ssbm-rs's objects (`kind` names the handle type), or none for null.
    fn import_handle(&mut self, kind: &'static str, addr: u32) -> Option<u32>;
    fn import_data(&mut self, addr: u32) -> Option<DataRef>;
    fn compare_handle(&mut self, path: &str, kind: &'static str, ours: Option<u32>, theirs: u32);
    fn compare_data(&mut self, path: &str, ours: Option<DataRef>, theirs: u32);
    fn mismatch(&mut self, path: &str, ours: String, theirs: String);
    /// Code read the field at `path` while it was stale (a new item's variable it hadn't
    /// written, `it::Var`).
    fn stale_read(&mut self, _path: &str) {}
}

/// A struct melee-sim mirrors from the game.
pub trait Mirror: Sized {
    const SIZE: u32;
    fn import(m: &Mem, at: u32, cx: &mut dyn Cx) -> Self;
    fn compare(&self, m: &Mem, at: u32, path: &str, cx: &mut dyn Cx);
}

/// A run of big-endian memory to write a struct into, at address `base`.
pub struct MemMut<'a> {
    pub bytes: &'a mut [u8],
    pub base: u32,
}

impl MemMut<'_> {
    fn at(&mut self, a: u32, n: usize) -> &mut [u8] {
        let i = a.wrapping_sub(self.base) as usize;
        &mut self.bytes[i..i + n]
    }

    pub fn put_u8(&mut self, a: u32, v: u8) {
        self.at(a, 1)[0] = v;
    }

    pub fn put_u16(&mut self, a: u32, v: u16) {
        self.at(a, 2).copy_from_slice(&v.to_be_bytes());
    }

    pub fn put_u32(&mut self, a: u32, v: u32) {
        self.at(a, 4).copy_from_slice(&v.to_be_bytes());
    }

    pub fn put_u64(&mut self, a: u32, v: u64) {
        self.at(a, 8).copy_from_slice(&v.to_be_bytes());
    }

    pub fn put_bytes(&mut self, a: u32, v: &[u8]) {
        self.at(a, v.len()).copy_from_slice(v);
    }

    /// A bitfield of `bits` bits, `bit` bits from the start of the struct at `at`, as
    /// [`Mem::bits`] reads it.
    pub fn put_bits(&mut self, at: u32, bit: u32, bits: u32, v: u32) {
        for (k, b) in (bit..bit + bits).enumerate() {
            let set = v >> (bits - 1 - k as u32) & 1 != 0;
            let byte = &mut self.at(at + b / 8, 1)[0];
            let mask = 0x80u8 >> (b % 8);
            *byte = if set { *byte | mask } else { *byte & !mask };
        }
    }
}

/// A struct melee-sim writes back as the game keeps it in memory (the card's data, which a
/// save writes to the card).
pub trait Export {
    fn export(&self, m: &mut MemMut, at: u32);
}

/// A function pointer as the mirror holds it.
pub fn code(addr: u32) -> Option<CodeRef> {
    (addr != 0).then_some(CodeRef(addr))
}

fn floats<const N: usize>(m: &Mem, at: u32) -> [f32; N] {
    core::array::from_fn(|i| f32::from_bits(m.u32(at + 4 * i as u32)))
}

fn compare_floats(ours: &[f32], m: &Mem, at: u32, path: &str, cx: &mut dyn Cx) {
    let theirs: Vec<u32> = (0..ours.len() as u32).map(|i| m.u32(at + 4 * i)).collect();
    let ours_bits: Vec<u32> = ours.iter().map(|v| v.to_bits()).collect();
    if ours_bits != theirs {
        cx.mismatch(
            path,
            format!("{ours:?}"),
            format!(
                "{:?}",
                theirs
                    .iter()
                    .map(|&b| f32::from_bits(b))
                    .collect::<Vec<_>>()
            ),
        );
    }
}

impl Mirror for Vec3 {
    const SIZE: u32 = 0xC;

    fn import(m: &Mem, at: u32, _: &mut dyn Cx) -> Self {
        let [x, y, z] = floats(m, at);
        Vec3::new(x, y, z)
    }

    fn compare(&self, m: &Mem, at: u32, path: &str, cx: &mut dyn Cx) {
        compare_floats(&[self.x, self.y, self.z], m, at, path, cx);
    }
}

impl Mirror for Quat {
    const SIZE: u32 = 0x10;

    fn import(m: &Mem, at: u32, _: &mut dyn Cx) -> Self {
        let [x, y, z, w] = floats(m, at);
        Quat { x, y, z, w }
    }

    fn compare(&self, m: &Mem, at: u32, path: &str, cx: &mut dyn Cx) {
        compare_floats(&[self.x, self.y, self.z, self.w], m, at, path, cx);
    }
}

/// A vector in loaded game data.
pub fn read_vec3(data: &melee_data::archive::Archives, at: DataRef) -> Vec3 {
    Vec3::new(data.f32(at), data.f32(at.at(4)), data.f32(at.at(8)))
}
