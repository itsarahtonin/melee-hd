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
    /// Whether the block is a new object's leftovers (`game::leftovers`): what its words
    /// point into is the freed objects', not the new one's.
    fn leftover(&self) -> bool {
        false
    }
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

/// What laying a struct out for the heap's image needs from the caller: the address each
/// handle and data place names now, where it knows it (none leaves the word unknown).
pub trait LayCx {
    /// The address of the object a handle names (`kind` names the handle type); `Some(0)` for
    /// none.
    fn handle(&self, kind: &'static str, h: Option<u32>) -> Option<u32>;
    /// The address of a place in loaded data; `Some(0)` for none.
    fn data(&self, d: Option<DataRef>) -> Option<u32>;
}

/// A struct laid out as the game keeps it, for the heap's image ([`crate::hsd::image`]): what
/// its block holds once the object goes back, which a later object there takes as leftovers.
/// A field the comparer leaves out (a stale variable, an unknown word) stays unknown.
pub trait LayOut {
    fn lay(&self, l: &mut crate::hsd::export::Lay, at: u32, cx: &dyn LayCx);
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

impl LayOut for Vec3 {
    fn lay(&self, l: &mut crate::hsd::export::Lay, at: u32, _: &dyn LayCx) {
        for (i, v) in [self.x, self.y, self.z].iter().enumerate() {
            l.put_u32(at + 4 * i as u32, v.to_bits());
        }
    }
}

impl LayOut for Quat {
    fn lay(&self, l: &mut crate::hsd::export::Lay, at: u32, _: &dyn LayCx) {
        for (i, v) in [self.x, self.y, self.z, self.w].iter().enumerate() {
            l.put_u32(at + 4 * i as u32, v.to_bits());
        }
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

#[cfg(test)]
mod label_tests {
    use super::*;
    use crate::game::structs::{Fighter, Item, itGShell_ItemVars};

    /// Collects the comparer's labels.
    #[derive(Default)]
    struct Labels(Vec<String>);

    impl Cx for Labels {
        fn import_handle(&mut self, _: &'static str, a: u32) -> Option<u32> {
            (a != 0).then_some(a)
        }
        fn import_data(&mut self, _: u32) -> Option<DataRef> {
            None
        }
        fn compare_handle(&mut self, p: &str, _: &'static str, o: Option<u32>, t: u32) {
            if o.unwrap_or(0) != t {
                self.0.push(p.to_owned());
            }
        }
        fn compare_data(&mut self, _: &str, _: Option<DataRef>, _: u32) {}
        fn mismatch(&mut self, p: &str, _: String, _: String) {
            self.0.push(p.to_owned());
        }
    }

    /// A renamed field's label is its name with the original's beside it (design call 11):
    /// ssbm-rs's traces, the engine notes and Melee's bugs cite offsets.
    #[test]
    fn a_renamed_fields_label_gives_the_originals_name() {
        let block = vec![0u8; Fighter::SIZE as usize];
        let m = Mem {
            bytes: &block,
            base: 0x8000_0000,
        };
        let mut fp = Fighter::import(&m, 0x8000_0000, &mut Labels::default());
        let mut l = Labels::default();
        fp.compare(&m, 0x8000_0000, "fighter", &mut l);
        assert!(l.0.is_empty(), "{:?}", l.0);
        fp.min_hitlag = 1.0;
        fp.in_hitstun = true;
        fp.cpu.spawn_arg = 2;
        fp.cur_pos.x = 3.0;
        let mut l = Labels::default();
        fp.compare(&m, 0x8000_0000, "fighter", &mut l);
        assert_eq!(
            l.0,
            [
                "fighter.cur_pos",
                "fighter.min_hitlag (x1964)",
                "fighter.cpu.spawn_arg (x14)",
                "fighter.in_hitstun (x221C_b6)",
            ]
        );
    }

    /// The same for an item (task 22): its own fields, a bit of its flag word, and a kind's
    /// variables (the green shell's).
    #[test]
    fn a_renamed_item_fields_label_gives_the_originals_name() {
        let block = vec![0u8; Item::SIZE as usize];
        let m = Mem {
            bytes: &block,
            base: 0x8000_0000,
        };
        let mut ip = Item::import(&m, 0x8000_0000, &mut Labels::default());
        let mut l = Labels::default();
        ip.compare(&m, 0x8000_0000, "item", &mut l);
        assert!(l.0.is_empty(), "{:?}", l.0);
        ip.pos.x = 3.0;
        ip.hit_damage = 1;
        ip.flags.flags.held = true;
        let mut l = Labels::default();
        ip.compare(&m, 0x8000_0000, "item", &mut l);
        assert_eq!(
            l.0,
            [
                "item.pos",
                "item.hit_damage (xCA0)",
                "item.flags (xDC8_word).flags.held (x13)",
            ]
        );

        let mut v = itGShell_ItemVars::import(&m, 0x8000_0000, &mut Labels::default());
        v.life.set(1.0);
        v.popped.set(true);
        let mut l = Labels::default();
        v.compare(&m, 0x8000_0000, "gshell", &mut l);
        assert_eq!(l.0, ["gshell.life (xDD4)", "gshell.popped (xDEC_b3)"]);
    }

    /// A struct's labels after `f` changes it from a zeroed block.
    fn labels_after<T: Mirror>(path: &str, f: impl FnOnce(&mut T)) -> Vec<String> {
        let block = vec![0u8; T::SIZE as usize];
        let m = Mem {
            bytes: &block,
            base: 0x8000_0000,
        };
        let mut v = T::import(&m, 0x8000_0000, &mut Labels::default());
        f(&mut v);
        let mut l = Labels::default();
        v.compare(&m, 0x8000_0000, path, &mut l);
        l.0
    }

    /// The same for step 3's structs (task 23): one field of each, as the comparer prints it.
    #[test]
    fn step_3s_renamed_fields_label_gives_the_originals_name() {
        use crate::game::structs::{
            _StaticPlayer, Camera, Ground, GroundParam, MatchPlayerData, StageInfo, StartMeleeRules,
        };
        assert_eq!(
            labels_after::<Fighter>("fighter", |fp| {
                fp.coll_data.map_generation = 1;
                fp.hitboxes[0].kb_growth = 1;
                fp.dmg.percent = 1.0;
            }),
            [
                "fighter.coll_data.map_generation (x38)",
                "fighter.hitboxes (x914)[0].kb_growth (x24)",
                "fighter.dmg.percent (x1830_percent)",
            ]
        );
        assert_eq!(
            labels_after::<Fighter>("fighter", |fp| {
                fp.state_flags.holds_grab = true;
                fp.reflect.damage_mul = 1.0;
            }),
            [
                "fighter.reflect (ReflectAttr).damage_mul (x1A34_damageMul)",
                "fighter.state_flags (x221B).holds_grab (x221B_b5)",
            ]
        );
        assert_eq!(
            labels_after::<Item>("item", |ip| ip.hit_flags.bits.silenced = true),
            ["item.hit_flags (xDCD_flag).bits (x0).silenced (b2)"]
        );
        assert_eq!(
            labels_after::<Camera>("camera", |c| {
                c.zoom_step = 1;
                c.orbit_limits.min_distance = 1.0;
            }),
            [
                "camera.zoom_step (x2BA)",
                "camera.orbit_limits (x2D0).min_distance (unk28)"
            ]
        );
        assert_eq!(
            labels_after::<StageInfo>("stage_info", |s| {
                s.bits.fog_enabled = true;
                s.acid_level = 1.0;
            }),
            [
                "stage_info.bits (unk8C).fog_enabled (b2)",
                "stage_info.acid_level (x724)"
            ]
        );
        assert_eq!(
            labels_after::<GroundParam>("param", |p| p.cam_pan_rate = 1.0),
            ["param.cam_pan_rate (x1C)"]
        );
        assert_eq!(
            labels_after::<Ground>("ground", |g| g.flags.shadowed = true),
            ["ground.flags (x10_flags).shadowed (b5)"]
        );
        assert_eq!(
            labels_after::<StartMeleeRules>("rules", |r| r.item_switch = 1),
            ["rules.item_switch (x20)"]
        );
        assert_eq!(
            labels_after::<MatchPlayerData>("standing", |p| p.kos = 1),
            ["standing.kos (x20)"]
        );
        assert_eq!(
            labels_after::<_StaticPlayer>("player", |p| {
                p.coins_lost = 1;
                p.flags.rumbles = true;
            }),
            ["player.coins_lost (unk9C)", "player.flags.rumbles (b0)"]
        );
    }
}
