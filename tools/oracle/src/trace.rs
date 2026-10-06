//! Reads ssbm-rs's `STATE_TRACE` files: the game's raw state at the end of every engine tick
//! (ssbm-rs `tools/run/src/trace.rs` documents the format).

use std::io::Read;

const MAGIC: &[u8; 8] = b"SSBMTRC\0";
const VERSION: u32 = 1;

/// What a region holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Seed,
    Players,
    Stage,
    Camera,
    Fighter,
    Item,
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
            _ => return None,
        })
    }
}

/// A run of the game's memory, big-endian as the game keeps it.
pub struct Region {
    pub kind: Kind,
    /// Where it was, and the GObj owning a fighter or item (0 for globals): what ties a
    /// fighter or item to itself from tick to tick.
    #[allow(dead_code)]
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

/// Every tick in the file. A run cut short ends at its last complete tick.
pub fn read(path: &str) -> Vec<Tick> {
    let file = std::fs::File::open(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let mut r = std::io::BufReader::new(file);
    let mut magic = [0; 8];
    r.read_exact(&mut magic)
        .unwrap_or_else(|e| panic!("{path}: {e}"));
    assert!(&magic == MAGIC, "{path}: not a state trace");
    let version = u32(&mut r).unwrap();
    assert!(
        version == VERSION,
        "{path}: trace version {version}, expected {VERSION}"
    );
    let mut z = zstd::Decoder::with_buffer(r).unwrap();
    let mut ticks = Vec::new();
    while let Some(tick) = read_tick(&mut z) {
        assert!(
            tick.number as usize == ticks.len(),
            "{path}: tick {} where {} was due",
            tick.number,
            ticks.len()
        );
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
