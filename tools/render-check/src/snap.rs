//! A GX stream's draws and copies as the GPU sees them, for comparing two streams draw by
//! draw: each draw's transformed vertices, the raster and TEV registers it uses and the
//! textures it samples, by content rather than address.

use std::cell::RefCell;

use melee_gx::texture::{self, Image};
use melee_gx::xform::{self, Viewport};
use melee_gx::{Draw, Memory, State, tev, vertex};

/// BP registers a draw's output depends on, other than textures' addresses: the generation
/// mode, indirect matrices and stages, scissor, line and point sizes, TEV orders, texture
/// coordinate scales, depth and blending, the pixel engine's control, texture modes and
/// sizes, TEV stages, fog, alpha compare, depth textures and konst selections.
const BP_REGS: &[usize] = &[
    0x00, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16,
    0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F, 0x20, 0x21, 0x22, 0x25, 0x26, 0x27, 0x28,
    0x29, 0x2A, 0x2B, 0x2C, 0x2D, 0x2E, 0x2F, 0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38,
    0x39, 0x3A, 0x3B, 0x3C, 0x3D, 0x3E, 0x3F, 0x40, 0x41, 0x42, 0x43, 0x59, 0xC0, 0xC1, 0xC2, 0xC3,
    0xC4, 0xC5, 0xC6, 0xC7, 0xC8, 0xC9, 0xCA, 0xCB, 0xCC, 0xCD, 0xCE, 0xCF, 0xD0, 0xD1, 0xD2, 0xD3,
    0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9, 0xDA, 0xDB, 0xDC, 0xDD, 0xDE, 0xDF, 0xE8, 0xE9, 0xEA, 0xEB,
    0xEC, 0xED, 0xEE, 0xEF, 0xF0, 0xF1, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7, 0xF8, 0xF9, 0xFA, 0xFB,
    0xFC, 0xFD,
];

pub fn bp_name(reg: usize) -> &'static str {
    match reg {
        0x00 => "GENMODE",
        0x06..=0x0E => "IND_MTX",
        0x10..=0x1F => "IND_CMD",
        0x20 => "SCISSOR_TL",
        0x21 => "SCISSOR_BR",
        0x22 => "LINE_PT",
        0x25 | 0x26 => "RAS1_SS",
        0x27 => "IREF",
        0x28..=0x2F => "TREF",
        0x30..=0x3F => "SU_TS",
        0x40 => "ZMODE",
        0x41 => "CMODE0",
        0x42 => "CMODE1",
        0x43 => "PE_CTRL",
        0x59 => "SCISSOR_OFS",
        0x80..=0x83 | 0xA0..=0xA3 => "TX_MODE0",
        0x84..=0x87 | 0xA4..=0xA7 => "TX_MODE1",
        0x88..=0x8B | 0xA8..=0xAB => "TX_IMAGE0",
        0x98..=0x9B | 0xB8..=0xBB => "TX_TLUT",
        0xC0..=0xDF if reg.is_multiple_of(2) => "TEV_COLOR_ENV",
        0xC0..=0xDF => "TEV_ALPHA_ENV",
        0xE8 => "FOG_RANGE",
        0xE9..=0xED => "FOG_RANGE_K",
        0xEE..=0xF1 => "FOG",
        0xF2 => "FOG_COLOR",
        0xF3 => "ALPHA_COMPARE",
        0xF4 => "ZTEX0",
        0xF5 => "ZTEX1",
        0xF6..=0xFD => "TEV_KSEL",
        _ => "?",
    }
}

/// The end of the transform unit's matrix and light memory.
const XF_MEM_END: usize = 0x680;

/// A texture a draw samples.
#[derive(Clone, Debug, PartialEq)]
pub struct Tex {
    pub map: usize,
    pub width: u32,
    pub height: u32,
    pub format: u32,
    pub mode0: u32,
    pub mode1: u32,
    pub hash: u64,
    /// The palette's register (its TMEM place and format), and the hash of the entries the
    /// image indexes.
    pub tlut: Option<(u32, u64)>,
    pub address: u32,
    /// The palette's first entries, for debugging (not compared).
    pub tlut_head: Vec<u8>,
    /// The palette's entries the image indexes, as TMEM holds them, to name the entries that
    /// differ (compared by `tlut`).
    pub palette: Vec<u8>,
    /// The highest palette entry the image's data indexes (a color-indexed format's).
    pub highest_index: Option<u32>,
}

/// A transformed vertex: screen position, colors and texture coordinates.
#[derive(Clone, Copy, Debug, Default)]
pub struct Vert {
    pub screen: [f32; 3],
    pub color: [[u8; 4]; 2],
    pub tex: [[f32; 3]; 8],
}

#[derive(Clone, Debug)]
pub struct DrawSnap {
    pub primitive: u8,
    pub count: u16,
    pub bp: Vec<(usize, u32)>,
    /// TEV color and konst registers as RA, BG halves, masked to what the stages read.
    pub tev: [u32; 16],
    pub textures: Vec<Tex>,
    pub verts: Vec<Vert>,
    pub texgens: u32,
    /// XF registers the draw's colors and coordinates depend on: the channels' count, colors
    /// and controls, and the texture coordinate generators.
    pub xf: Vec<(usize, u32)>,
    /// The transform unit's memory: the position, texture, normal and post-transform
    /// matrices and the lights (`0x000` to `0x67F`), whether the draw reads them or not, so
    /// a load the original makes and the port doesn't shows even before a draw reads it.
    pub xf_mem: Vec<u32>,
    /// The vertex layout and the first fetched bytes, for debugging (read through `Debug`).
    #[allow(dead_code)]
    pub layout: String,
    #[allow(dead_code)]
    pub head: Vec<u8>,
    /// The stream's commands since the event before, with `CHECK_COMMANDS` (for debugging,
    /// read through `Debug`).
    #[allow(dead_code)]
    pub commands: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct CopySnap {
    pub value: u32,
    pub regs: Vec<(usize, u32)>,
}

#[derive(Clone, Debug)]
pub enum Event {
    Draw(Box<DrawSnap>),
    Copy(CopySnap),
}

/// Records what a stream draws.
#[derive(Default)]
pub struct Recorder {
    pub events: RefCell<Vec<Event>>,
    normals: RefCell<vertex::NormalCache>,
    /// The stream's commands since the last event, with `CHECK_COMMANDS`.
    commands: RefCell<Vec<String>>,
}

fn hash(bytes: &[u8]) -> u64 {
    melee_gx::fnv(0xCBF2_9CE4_8422_2325, bytes)
}

impl Recorder {
    /// Notes a command of the stream itself with `CHECK_COMMANDS`, for the next event.
    pub fn command(&self, bytes: &[u8]) {
        if std::env::var_os("CHECK_COMMANDS").is_none() {
            return;
        }
        let be = |b: &[u8]| u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
        let text = match bytes[0] {
            0x08 => format!("CP {:02X} {:08X}", bytes[1], be(&bytes[2..])),
            0x10 => {
                let head = be(&bytes[1..]);
                let words: Vec<String> = bytes[5..]
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|w| format!("{:08X}", u32::from_be_bytes(*w)))
                    .collect();
                format!("XF {:04X} {}", head & 0xFFFF, words.join(" "))
            }
            0x20 | 0x28 | 0x30 | 0x38 => {
                format!("LOAD_INDX {:02X} {:08X}", bytes[0], be(&bytes[1..]))
            }
            0x40 => format!("CALL_DL {:08X} {:X}", be(&bytes[1..]), be(&bytes[5..])),
            0x61 => format!("BP {:02X} {:06X}", bytes[1], be(&bytes[1..]) & 0xFF_FFFF),
            0x80..=0xBF => format!(
                "PRIM {:02X} n {}",
                bytes[0],
                u16::from_be_bytes([bytes[1], bytes[2]])
            ),
            op => format!("{op:02X}"),
        };
        self.commands.borrow_mut().push(text);
    }

    pub fn draw(&self, state: &State, draw: &Draw<'_>, mem: &dyn Memory) {
        // `CHECK_ALL_BP` compares every register but those holding addresses or triggers.
        let bp = if std::env::var_os("CHECK_ALL_BP").is_some() {
            (0..256)
                .filter(|r| {
                    !matches!(r, 0x45..=0x4B | 0x4D | 0x52 | 0x55..=0x58 | 0x64..=0x67 | 0x94..=0x97 | 0xB4..=0xB7 | 0xE0..=0xE7 | 0xFE | 0xFF)
                })
                .map(|r| (r, state.bp[r]))
                .collect()
        } else {
            BP_REGS.iter().map(|&r| (r, state.bp[r])).collect()
        };
        let usage = tev::usage(&state.bp);
        let mut t = [0u32; 16];
        for i in 0..4 {
            let (ra, bg) = tev::masked(state.color_ra[i], state.color_bg[i], usage.color[i]);
            let (kra, kbg) = tev::masked(state.konst_ra[i], state.konst_bg[i], usage.konst[i]);
            t[4 * i..4 * i + 4].copy_from_slice(&[ra, bg, kra, kbg]);
        }
        let mut textures = Vec::new();
        for map in state.textures_used() {
            let image = Image::of(&state.bp, map);
            let mut data = vec![0; image.size() as usize];
            mem.read(image.address, &mut data);
            // The highest palette entry the image's texels (every level's) index.
            let highest_index = match image.format {
                8 => data
                    .iter()
                    .map(|b| u32::from(b >> 4).max(u32::from(b & 15)))
                    .max(),
                9 => data.iter().map(|&b| u32::from(b)).max(),
                10 => data
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|c| u32::from(u16::from_be_bytes(*c) & 0x3FFF))
                    .max(),
                _ => None,
            };
            let mut tlut_head = Vec::new();
            let mut palette = Vec::new();
            // The palette's entries the image indexes, by content. A palette region loads
            // whole (256 entries for `GX_TLUT0`..), so entries past a shorter palette hold
            // the bytes after it in memory, which no texel reads (docs/player.md, Palette
            // entries).
            let tlut = texture::palette_entries(image.format).map(|entries| {
                let reg = state.bp[texture::reg(texture::SETTLUT, map)];
                let at = ((reg & 0x3FF) << 9) as usize;
                let used = highest_index.map_or(entries, |i| (i + 1).min(entries));
                let end = (at + 2 * used as usize).min(state.tmem.len());
                tlut_head = state.tmem[at..end.min(at + 32)].to_vec();
                palette = state.tmem[at..end].to_vec();
                (reg & 0xFFF, hash(&state.tmem[at..end]))
            });
            textures.push(Tex {
                map,
                width: image.width,
                height: image.height,
                format: image.format,
                mode0: state.bp[texture::reg(texture::SETMODE0, map)],
                mode1: state.bp[texture::reg(texture::SETMODE1, map)],
                hash: hash(&data),
                tlut,
                address: image.address,
                tlut_head,
                palette,
                highest_index,
            });
        }
        let xf_index = [state.xf[xform::MATINDEX_A], state.xf[xform::MATINDEX_A + 1]];
        let mut inputs = Vec::new();
        vertex::decode(
            &state.cp,
            xf_index,
            usize::from(draw.vat),
            draw.data,
            usize::from(draw.count),
            &mut self.normals.borrow_mut(),
            &mut inputs,
        );
        let vp = Viewport::of(&state.xf);
        let verts = inputs
            .iter()
            .map(|v| {
                let o = xform::transform(&state.xf, &state.bp, v);
                Vert {
                    screen: vp.screen(o.clip),
                    color: o.color,
                    tex: o.tex,
                }
            })
            .collect();
        self.events
            .borrow_mut()
            .push(Event::Draw(Box::new(DrawSnap {
                primitive: draw.primitive,
                count: draw.count,
                bp,
                tev: t,
                textures,
                verts,
                texgens: state.xf[xform::NUM_TEXGENS],
                xf_mem: state.xf[..XF_MEM_END].to_vec(),
                xf: (0x1009..0x1012)
                    .chain(0x101A..0x1027)
                    .chain(0x1040..0x1040 + (state.xf[xform::NUM_TEXGENS] as usize & 0xF))
                    .map(|r| (r, state.xf[r]))
                    .collect(),
                layout: format!("{:?}", draw.layout),
                head: draw.data[..draw.data.len().min(48)].to_vec(),
                commands: std::mem::take(&mut *self.commands.borrow_mut()),
            })));
    }

    pub fn copy(&self, state: &State, value: u32) {
        let regs = [
            0x01, 0x02, 0x03, 0x04, 0x49, 0x4A, 0x4B, 0x4D, 0x4E, 0x4F, 0x50, 0x51, 0x53, 0x54,
        ]
        .iter()
        .map(|&r| (r, state.bp[r]))
        .collect();
        self.commands.borrow_mut().clear();
        self.events
            .borrow_mut()
            .push(Event::Copy(CopySnap { value, regs }));
    }
}

/// A one-line summary of a draw.
pub fn summary(d: &DrawSnap) -> String {
    let get = |r: usize| d.bp.iter().find(|x| x.0 == r).map_or(0, |x| x.1);
    let stages = (get(0x00) >> 10 & 0xF) + 1;
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for v in &d.verts {
        x0 = x0.min(v.screen[0]);
        y0 = y0.min(v.screen[1]);
        x1 = x1.max(v.screen[0]);
        y1 = y1.max(v.screen[1]);
    }
    let tex: Vec<String> = d
        .textures
        .iter()
        .map(|t| format!("{}:{}x{}f{}", t.map, t.width, t.height, t.format))
        .collect();
    format!(
        "prim {:02X} n {} stages {} blend {:06X} z {:06X} tex [{}] box ({:.0},{:.0})-({:.0},{:.0})",
        d.primitive,
        d.count,
        stages,
        get(0x41) & 0xFF_FFFF,
        get(0x40) & 0xFF_FFFF,
        tex.join(" "),
        x0,
        y0,
        x1,
        y1
    )
}

/// Whether two of a draw's floats are the same (equal, or both NaN): the
/// comparison allows no tolerance, so a rounding difference in a matrix shows (one left a few
/// of Dream Land's background pixels different through the depth test).
fn same(a: f32, b: f32) -> bool {
    a == b || (a.is_nan() && b.is_nan())
}

/// The bits of each BP register a draw's output depends on: TEV orders, stages, konst
/// selections and indirect commands of the stages that run; texture coordinate scales of the
/// coordinates generated, their line and point offsets only for lines and points; fog and its
/// range adjustment only when on.
fn used_bits(d: &DrawSnap) -> impl Fn(usize) -> u32 + '_ {
    let get = |r: usize| d.bp.iter().find(|x| x.0 == r).map_or(0, |x| x.1);
    let stages = ((get(0x00) >> 10 & 0xF) + 1) as usize;
    let coords = (d.texgens & 0xF) as usize;
    let lines = (0xA8..=0xBF).contains(&d.primitive);
    let fog = get(0xF1) >> 21 & 7 != 0;
    let range = get(0xE8) >> 10 & 1 != 0;
    move |r: usize| -> u32 {
        let all = 0xFF_FFFF;
        match r {
            0x28..=0x2F => {
                let s = 2 * (r - 0x28);
                (if s < stages { 0xFFF } else { 0 }) | (if s + 1 < stages { 0xFFF000 } else { 0 })
            }
            0x10..=0x1F => {
                if r - 0x10 < stages {
                    all
                } else {
                    0
                }
            }
            0xC0..=0xDF => {
                if (r - 0xC0) / 2 < stages {
                    all
                } else {
                    0
                }
            }
            0xF6..=0xFD => {
                let s = 2 * (r - 0xF6);
                0xF | (if s < stages { 0x3FF0 } else { 0 })
                    | (if s + 1 < stages { 0xFFC000 } else { 0 })
            }
            0x30..=0x3F => {
                if (r - 0x30) / 2 < coords {
                    if lines { all } else { all & !(3 << 18) }
                } else {
                    0
                }
            }
            0xEE..=0xF2 => {
                if fog {
                    all
                } else {
                    0
                }
            }
            0xE9..=0xED => {
                if range {
                    all
                } else {
                    0
                }
            }
            _ => all,
        }
    }
}

/// Which entries of two otherwise equal textures' palettes differ, against the highest entry
/// the image indexes.
fn palette_difference(a: &Tex, b: &Tex) -> String {
    let entries: Vec<usize> = a
        .palette
        .as_chunks::<2>()
        .0
        .iter()
        .zip(b.palette.as_chunks::<2>().0)
        .enumerate()
        .filter(|(_, (x, y))| x != y)
        .map(|(i, _)| i)
        .collect();
    format!(
        "texture {} ({}x{} format {}): {} palette entries differ, {} to {} (the image indexes up to {:?}; first: {:02X?} vs {:02X?})",
        a.map,
        a.width,
        a.height,
        a.format,
        entries.len(),
        entries.first().copied().unwrap_or(0),
        entries.last().copied().unwrap_or(0),
        a.highest_index,
        entries.first().map(|&i| &a.palette[2 * i..2 * i + 2]),
        entries.first().map(|&i| &b.palette[2 * i..2 * i + 2]),
    )
}

/// How two draws differ, or nothing.
pub fn differences(a: &DrawSnap, b: &DrawSnap) -> Vec<String> {
    let mut out = Vec::new();
    if (a.primitive, a.count) != (b.primitive, b.count) {
        out.push(format!(
            "primitive {:02X}x{} vs {:02X}x{}",
            a.primitive, a.count, b.primitive, b.count
        ));
        return out;
    }
    // What either draw uses (fog on in one and off in the other shows).
    let (ma, mb) = (used_bits(a), used_bits(b));
    for (x, y) in a.bp.iter().zip(&b.bp) {
        let mask = ma(x.0) | mb(x.0);
        if x.1 & mask != y.1 & mask {
            out.push(format!(
                "BP {:02X} {} {:06X} vs {:06X}",
                x.0,
                bp_name(x.0),
                x.1 & 0xFF_FFFF,
                y.1 & 0xFF_FFFF
            ));
        }
    }
    if a.tev != b.tev {
        out.push(format!("TEV registers {:08X?} vs {:08X?}", a.tev, b.tev));
    }
    for (x, y) in a.xf.iter().zip(&b.xf) {
        // A color channel's registers matter only with that many channels.
        let chans = a.xf.first().map_or(0, |c| c.1);
        let used = match x.0 {
            0x100A | 0x100C | 0x100E | 0x1010 => chans >= 1,
            0x100B | 0x100D | 0x100F | 0x1011 => chans >= 2,
            _ => true,
        };
        if used && x.1 != y.1 {
            out.push(format!("XF {:04X} {:08X} vs {:08X}", x.0, x.1, y.1));
        }
    }
    if let Some(at) = (0..a.xf_mem.len().min(b.xf_mem.len())).find(|&i| a.xf_mem[i] != b.xf_mem[i])
    {
        let n = (0..a.xf_mem.len().min(b.xf_mem.len()))
            .filter(|&i| a.xf_mem[i] != b.xf_mem[i])
            .count();
        out.push(format!(
            "XF memory: {n} words differ, the first at {at:04X} ({:08X} vs {:08X})",
            a.xf_mem[at], b.xf_mem[at]
        ));
    }
    if a.texgens != b.texgens {
        out.push(format!("texgens {} vs {}", a.texgens, b.texgens));
    }
    if a.textures.len() != b.textures.len() {
        out.push(format!(
            "{} textures vs {}",
            a.textures.len(),
            b.textures.len()
        ));
    } else {
        for (x, y) in a.textures.iter().zip(&b.textures) {
            let strip = |t: &Tex| Tex {
                address: 0,
                palette: Vec::new(),
                ..t.clone()
            };
            if strip(x) != strip(y) {
                let only_palette = Tex {
                    tlut: None,
                    ..strip(x)
                } == Tex {
                    tlut: None,
                    ..strip(y)
                };
                if only_palette {
                    out.push(palette_difference(x, y));
                } else {
                    out.push(format!("texture {x:X?} vs {y:X?}"));
                }
            }
        }
    }
    let (mut pos, mut col, mut tex) = (0usize, 0usize, 0usize);
    let mut worst = 0f32;
    let mut worst_col = [0u8; 2];
    // The color channels the draw has (`XF 1009`); the transform unit's other outputs reach
    // nothing.
    let chans = a.xf.first().map_or(2, |c| (c.1 & 3) as usize).min(2);
    for (x, y) in a.verts.iter().zip(&b.verts) {
        for k in 0..3 {
            if !same(x.screen[k], y.screen[k]) {
                pos += 1;
                worst = worst.max((x.screen[k] - y.screen[k]).abs());
            }
        }
        for ((p, q), worst) in x.color.iter().zip(&y.color).zip(&mut worst_col).take(chans) {
            for (a, b) in p.iter().zip(q) {
                let d = a.abs_diff(*b);
                if d > 0 {
                    col += 1;
                    *worst = (*worst).max(d);
                }
            }
        }
        for t in 0..8 {
            for k in 0..3 {
                if !same(x.tex[t][k], y.tex[t][k]) {
                    tex += 1;
                }
            }
        }
    }
    if pos > 0 {
        out.push(format!("{pos} position components differ (worst {worst})"));
    }
    if col > 0 {
        let (x, y) = (&a.verts[0], &b.verts[0]);
        out.push(format!(
            "{col} color components differ (worst {} levels in color 0, {} in color 1; first vertex {:?} vs {:?})",
            worst_col[0], worst_col[1], x.color, y.color
        ));
    }
    if tex > 0 {
        let (x, y) = (&a.verts[0], &b.verts[0]);
        out.push(format!(
            "{tex} texture coordinates differ (first vertex {:?} vs {:?})",
            &x.tex[..2],
            &y.tex[..2]
        ));
    }
    out
}

/// What lines a draw up with another: its primitive, vertex count and textures' shapes.
fn signature(e: &Event) -> (u8, u16, Vec<(u32, u32, u32)>) {
    match e {
        Event::Draw(d) => (
            d.primitive,
            d.count,
            d.textures
                .iter()
                .map(|t| (t.width, t.height, t.format))
                .collect(),
        ),
        Event::Copy(c) => (0, 0, vec![(c.value, 0, 0)]),
    }
}

/// A one-line summary of an event.
pub fn event_summary(e: &Event) -> String {
    match e {
        Event::Draw(d) => summary(d),
        Event::Copy(c) => format!("COPY {:08X}", c.value),
    }
}

/// Lines two streams' events up by their longest common subsequence of signatures: pairs of
/// indexes, with `None` on the side that lacks an event.
pub fn align(a: &[Event], b: &[Event]) -> Vec<(Option<usize>, Option<usize>)> {
    let sa: Vec<_> = a.iter().map(signature).collect();
    let sb: Vec<_> = b.iter().map(signature).collect();
    let (n, m) = (sa.len(), sb.len());
    // lcs[i][j]: the longest common subsequence of a[i..] and b[j..].
    let mut lcs = vec![0u16; (n + 1) * (m + 1)];
    let at = |i: usize, j: usize| i * (m + 1) + j;
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[at(i, j)] = if sa[i] == sb[j] {
                lcs[at(i + 1, j + 1)] + 1
            } else {
                lcs[at(i + 1, j)].max(lcs[at(i, j + 1)])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut out = Vec::new();
    while i < n && j < m {
        if sa[i] == sb[j] {
            out.push((Some(i), Some(j)));
            i += 1;
            j += 1;
        } else if lcs[at(i + 1, j)] >= lcs[at(i, j + 1)] {
            out.push((Some(i), None));
            i += 1;
        } else {
            out.push((None, Some(j)));
            j += 1;
        }
    }
    out.extend((i..n).map(|i| (Some(i), None)));
    out.extend((j..m).map(|j| (None, Some(j))));
    out
}

/// Whether two copies are the same but for where they copy to (BP 0x4B, an address).
pub fn same_copy(a: &CopySnap, b: &CopySnap) -> bool {
    let strip = |c: &CopySnap| -> Vec<(usize, u32)> {
        c.regs.iter().copied().filter(|r| r.0 != 0x4B).collect()
    };
    a.value == b.value && strip(a) == strip(b)
}
