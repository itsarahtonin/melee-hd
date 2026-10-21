//! Reads ssbm-rs's `STATE_TRACE` files: the game's raw state at the end of every engine tick
//! (ssbm-rs `tools/run/src/trace.rs` documents the format). Version 3 adds main memory
//! (`STATE_TRACE_MEMORY=1`): each tick's changed pages, XORed with what they held the tick
//! before, which [`Image`] keeps whole. Version 4 adds the time base to those ticks. Version 5
//! records the pages the main thread's stack and the GX FIFO share with the rest of memory,
//! their own bytes held at zero, where earlier versions left those pages out (the OS heap
//! descriptors after the FIFO among them).

use std::io::Read;

const MAGIC: &[u8; 8] = b"SSBMTRC\0";
const VERSIONS: std::ops::RangeInclusive<u32> = 2..=5;

/// Main memory's address and size.
pub const MEM1: u32 = 0x8000_0000;
pub const MEM1_SIZE: usize = 24 << 20;

/// What a region holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Seed,
    Players,
    Stage,
    Camera,
    Fighter,
    Item,
    /// A fighter's model joint, in its tree's pre-order.
    JObj,
    /// A page of main memory XORed with what it held at the previous tick.
    Page,
    /// The time base at the tick's end, with memory (version 4).
    Time,
}

impl Kind {
    fn from_byte(b: u8) -> Option<Self> {
        Some(match b {
            0 => Kind::Seed,
            1 => Kind::Players,
            2 => Kind::Stage,
            3 => Kind::Camera,
            4 => Kind::Fighter,
            5 => Kind::Item,
            6 => Kind::JObj,
            7 => Kind::Page,
            8 => Kind::Time,
            _ => return None,
        })
    }
}

/// A run of the game's memory, big-endian as the game keeps it.
pub struct Region {
    pub kind: Kind,
    /// Where it was, and the GObj owning a fighter or item (0 for globals): what ties a
    /// fighter or item to itself from tick to tick.
    pub address: u32,
    #[allow(dead_code)]
    pub gobj: u32,
    pub bytes: Vec<u8>,
}

impl Region {
    pub fn u8(&self, at: usize) -> u8 {
        self.bytes[at]
    }
    pub fn u32(&self, at: usize) -> u32 {
        u32::from_be_bytes(self.bytes[at..at + 4].try_into().unwrap())
    }
}

pub struct Tick {
    pub number: u32,
    pub regions: Vec<Region>,
}

impl Tick {
    pub fn of(&self, kind: Kind) -> impl Iterator<Item = &Region> {
        self.regions.iter().filter(move |r| r.kind == kind)
    }
}

fn u32(r: &mut impl Read) -> std::io::Result<u32> {
    let mut b = [0; 4];
    r.read_exact(&mut b)?;
    Ok(u32::from_le_bytes(b))
}

type Decoder = zstd::Decoder<'static, std::io::BufReader<std::io::BufReader<std::fs::File>>>;

/// A trace read a tick at a time.
pub struct Reader {
    path: String,
    /// The format's version.
    pub version: u32,
    z: Decoder,
    next: u32,
}

impl Reader {
    pub fn open(path: &str) -> Reader {
        let file = std::fs::File::open(path).unwrap_or_else(|e| panic!("{path}: {e}"));
        let mut r = std::io::BufReader::new(file);
        let mut magic = [0; 8];
        r.read_exact(&mut magic)
            .unwrap_or_else(|e| panic!("{path}: {e}"));
        assert!(&magic == MAGIC, "{path}: not a state trace");
        let version = u32(&mut r).unwrap();
        assert!(
            VERSIONS.contains(&version),
            "{path}: trace version {version}, expected {VERSIONS:?}"
        );
        Reader {
            path: path.to_owned(),
            version,
            z: zstd::Decoder::new(r).unwrap(),
            next: 0,
        }
    }

    /// The next tick, or none at the end. A run cut short ends at its last complete tick.
    pub fn tick(&mut self) -> Option<Tick> {
        let tick = read_tick(&mut self.z)?;
        assert!(
            tick.number == self.next,
            "{}: tick {} where {} was due",
            self.path,
            tick.number,
            self.next
        );
        self.next += 1;
        Some(tick)
    }
}

/// Every tick in the file, without main memory's pages.
pub fn read(path: &str) -> Vec<Tick> {
    let mut r = Reader::open(path);
    let mut ticks = Vec::new();
    while let Some(mut tick) = r.tick() {
        tick.regions
            .retain(|r| !matches!(r.kind, Kind::Page | Kind::Time));
        ticks.push(tick);
    }
    ticks
}

fn read_tick(r: &mut impl Read) -> Option<Tick> {
    let number = u32(r).ok()?;
    let count = u32(r).ok()?;
    let mut regions = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let mut kind = [0; 1];
        r.read_exact(&mut kind).ok()?;
        let address = u32(r).ok()?;
        let gobj = u32(r).ok()?;
        let len = u32(r).ok()?;
        let mut bytes = vec![0; len as usize];
        r.read_exact(&mut bytes).ok()?;
        regions.push(Region {
            kind: Kind::from_byte(kind[0])?,
            address,
            gobj,
            bytes,
        });
    }
    Some(Tick { number, regions })
}

/// Main memory as a trace with pages last held it.
pub struct Image {
    pub mem1: Vec<u8>,
    /// Whether any tick so far held pages.
    pub seen: bool,
    /// The time base at the last tick that held it.
    pub tb: Option<u64>,
    /// A bit for each page ([`melee_capture::PAGE`]) a tick changed since the last
    /// [`Image::take_changed`].
    changed: Vec<u64>,
}

impl Default for Image {
    fn default() -> Self {
        Image {
            mem1: vec![0; MEM1_SIZE],
            seen: false,
            tb: None,
            changed: vec![0; (MEM1_SIZE / melee_capture::PAGE).div_ceil(64)],
        }
    }
}

impl Image {
    /// Applies a tick's pages: memory as it stood at the end of that tick.
    pub fn apply(&mut self, tick: &Tick) {
        for p in tick.of(Kind::Page) {
            let at = (p.address - MEM1) as usize;
            for (m, x) in self.mem1[at..at + p.bytes.len()].iter_mut().zip(&p.bytes) {
                *m ^= x;
            }
            let pages =
                at / melee_capture::PAGE..(at + p.bytes.len()).div_ceil(melee_capture::PAGE);
            for page in pages {
                self.changed[page / 64] |= 1 << (page % 64);
            }
            self.seen = true;
        }
        if let Some(t) = tick.of(Kind::Time).next() {
            self.tb = Some(u64::from_be_bytes(t.bytes[..8].try_into().unwrap()));
        }
    }

    pub fn u32(&self, addr: u32) -> u32 {
        let i = (addr - MEM1) as usize;
        u32::from_be_bytes(self.mem1[i..i + 4].try_into().unwrap())
    }

    /// The pages the ticks applied since the last call changed (or may have: a page a tick
    /// holds is one that changed), in increasing order.
    pub fn take_changed(&mut self) -> Vec<usize> {
        let mut out = Vec::new();
        for (w, bits) in self.changed.iter_mut().enumerate() {
            while *bits != 0 {
                out.push(64 * w + bits.trailing_zeros() as usize);
                *bits &= *bits - 1;
            }
        }
        out
    }
}
