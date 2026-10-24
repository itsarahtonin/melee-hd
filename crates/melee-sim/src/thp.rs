//! The SDK's THP video decoder (`dolphin/thp/THPDec.c`), which the movie player
//! (`lb::mthp`) runs on each frame it shows: a frame is a baseline JPEG of 4:2:0 YUV whose
//! headers [`video_decode`] reads into the decode work area ([`FileInfo`], `THPFileInfo`), then
//! [`decompress`] decodes its scan MCU row by MCU row (`__THPDecompressiMCURow640x480`) into
//! three planes of GX's I8 texture layout (8 by 4 texel tiles), which the movie's sprite draws.
//!
//! The decode is presentation and heap contents: what it writes reaches the screen and, since
//! later scenes take the buffers back uncleared, memory the game may read as uninitialized.
//! It depends only on the frame's bytes, so the movie player keeps which frame it decoded and
//! the frontend decodes it again to draw it. The inverse DCT is the original's paired-single
//! code, operation for operation (`__THPInverseDCTNoYPos`, `__THPInverseDCTY8`); the Huffman
//! decode reads the scan as the original does, 32-bit words at a time with no byte stuffing,
//! and gives what its fast paths give for every code a frame's tables define.

use gekko_fp::{self as fp, Ps};

/// `__THPJpegNaturalOrder`: the zigzag order, padded with 63 past the block's end for a run
/// that skips beyond it.
const NATURAL_ORDER: [u8; 80] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63, 63, 63, 63, 63, 63, 63, 63, 63, 63, 63,
    63, 63, 63, 63, 63, 63,
];

/// `__THPAANScaleFactor`: doubles initialized from single-precision literals (the original's
/// digits, which round to these singles).
#[allow(clippy::excessive_precision)]
const AAN_SCALE: [f64; 8] = [
    1.0f32 as f64,
    1.387039845f32 as f64,
    1.306562965f32 as f64,
    1.175875602f32 as f64,
    1.0f32 as f64,
    0.785694958f32 as f64,
    0.541196100f32 as f64,
    0.275899379f32 as f64,
];

/// The work area's header size, which `THPVideoDecode` clears; its scratch space follows.
pub const INFO_SIZE: u32 = 0x920;

/// A Huffman table (`THPHuffmanTab`). Pointers into the frame are offsets from its data's
/// start; pointers into the work area, offsets from its start.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HuffTab {
    pub quick: [u8; 32],
    pub increment: [u8; 32],
    pub vij: u32,
    pub max_code: [i32; 18],
    pub val_ptr: [i32; 18],
    pub bits: u32,
    pub size_tab: u32,
    pub code_tab: u32,
    pub num_codes: u32,
    pub num_vij: u16,
    /// The size and code tables' contents, in the work area at `size_tab` and `code_tab`.
    pub sizes: Vec<i8>,
    pub codes: Vec<u32>,
}

/// A component (`THPComponent`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Component {
    pub id: u8,
    pub sampling_h: u8,
    pub sampling_v: u8,
    pub quant_selector: u8,
    pub dc_selector: u8,
    pub ac_selector: u8,
    pub pred_dc: i16,
    pub x08: u32,
    pub x0c: u32,
    pub x10: u32,
    pub x14: i32,
    pub x18: i32,
    pub x1c: i32,
    pub x20: i32,
    pub x24: i32,
    pub x28: i32,
}

/// The decode work area's header (`THPFileInfo`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FileInfo {
    /// The bit stream: the word being read (an offset into the frame's data, a multiple of
    /// four from the data's aligned start), that word, and the next bit's place in it (1 the
    /// top bit, 33 none left).
    pub file: u32,
    pub curr_byte: u32,
    pub cnt: u32,
    pub mcu_buffer: [u32; 16],
    pub x_size: u16,
    pub y_size: u16,
    pub scan_start: u32,
    pub x_pixel_size: u16,
    pub y_pixel_size: u16,
    pub x74: u16,
    pub decompressed_y: u16,
    pub valid_huffman_tabs: u8,
    pub valid_quant_tabs: u8,
    pub sampling_h_max: u8,
    pub sampling_v_max: u8,
    pub n_components: u8,
    pub x7d: u8,
    pub huffman_tabs: [HuffTab; 4],
    pub quant_tabs: QuantTabs,
    pub components: [Component; 3],
    pub x8bc: [u8; 16],
    pub mcus_per_row: u16,
    pub x8ce: u16,
    pub x8d0: u16,
    pub x8d2: u16,
    pub x8d4: u16,
    pub x8e8: u16,
    pub x8ea: u16,
    pub x8ec: u16,
    pub x8ee: u16,
    /// Where the next MCU row goes in each plane.
    pub tile: [u32; 3],
    pub n_mcu: u16,
    pub curr_mcu: u16,
    pub rst: u8,
    pub scratch: u32,
}

/// The three quantization tables (`quantTabs`), dequantizing in natural order with the AAN
/// scale folded in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QuantTabs(pub [[f32; 64]; 3]);

impl Default for QuantTabs {
    fn default() -> Self {
        QuantTabs([[0.0; 64]; 3])
    }
}

/// `THPDec_8032FD40_Data`: the image the work area is sized for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct ImageDesc {
    pub val0: i32,
    pub width: u16,
    pub height: u16,
    pub format: u8,
}

/// `THPDec_8032FD40`: the work area's size for `height` rows of `desc`'s image, 0 (with the
/// SDK's report) unless it's 4:2:0.
#[doc(alias = "THPDec_8032FD40")]
pub fn work_size(desc: &ImageDesc, height: u16) -> i32 {
    if desc.format != 4 {
        return 0;
    }
    let w = i32::from(desc.width);
    let n = i32::from(height);
    desc.val0 + 0x4028 + (w / 2) * (n / 2) * 2 + w * n
}

/// `THPROUNDUP`.
fn round_up(a: i32, b: i32) -> i32 {
    (a + (b - 1)) / b
}

/// The frame's bytes, as the decoder reads them past its end (the rest of the read's buffer):
/// zeros, which no code a frame's tables define reaches.
fn byte(data: &[u8], at: u32) -> u8 {
    data.get(at as usize).copied().unwrap_or(0)
}

/// The word at `at`, a multiple of four.
fn word(data: &[u8], at: u32) -> u32 {
    let b = |i: u32| u32::from(byte(data, at.wrapping_add(i)));
    (b(0) << 24) | (b(1) << 16) | (b(2) << 8) | b(3)
}

impl FileInfo {
    fn take(&mut self, data: &[u8]) -> u8 {
        let b = byte(data, self.file);
        self.file += 1;
        b
    }

    fn u16_at(data: &[u8], at: u32) -> u16 {
        (u16::from(byte(data, at)) << 8) | u16::from(byte(data, at + 1))
    }
}

/// `THPVideoDecode`: reads the frame's headers into a cleared work area for an image of
/// `x_size` by `y_size`. `data` is the frame's record as the movie's ring holds it, at a
/// 32-byte boundary: its next frame's size, then the frame's data from offset 4 (the offsets
/// the work area keeps into the frame count from the record's start, so their alignment is
/// the memory's). Returns the work area, or the status the original leaves when it returns
/// null.
#[doc(alias = "THPVideoDecode")]
pub fn video_decode(x_size: u16, y_size: u16, data: &[u8]) -> Result<FileInfo, u8> {
    let mut info = FileInfo {
        scratch: INFO_SIZE,
        x_size,
        y_size,
        cnt: 33,
        ..FileInfo::default()
    };
    // `file` starts at `scanStart` (0) and the data at offset 4 into `data`.
    info.file = 0;
    mcu_buffers(&mut info);
    info.file = 4;
    let mut done = false;
    loop {
        if info.take(data) != 0xFF {
            return Err(3);
        }
        while byte(data, info.file) == 0xFF {
            info.file += 1;
        }
        let status = info.take(data);
        if status <= 0xD7 {
            if status == 0xC4 {
                read_huffman_tables(&mut info, data)?;
            } else if status == 0xC0 {
                read_frame_header(&mut info, data)?;
            } else {
                return Err(11);
            }
        } else if (0xD8..=0xDF).contains(&status) {
            if status == 0xDD {
                restart_definition(&mut info, data);
            } else if status == 0xDB {
                read_quantization_table(&mut info, data);
            } else if status == 0xDA {
                read_scan_header(&mut info, data)?;
                done = true;
                info.scan_start = info.file;
            } else if status != 0xD8 {
                return Err(11);
            }
        } else if status == 0xE0 {
            read_app0(&mut info, data)?;
        } else if (0xE1..=0xEF).contains(&status) {
            let n = FileInfo::u16_at(data, info.file);
            info.file += u32::from(n);
        } else if status == 0xFE {
            let n = FileInfo::u16_at(data, info.file) as i32;
            info.file += 2;
            info.file = info.file.wrapping_add((n - 2).max(0) as u32);
        } else {
            return Err(11);
        }
        if done {
            return Ok(info);
        }
    }
}

/// `THPDec_803300E0`: sixteen 128-byte coefficient blocks from the scratch space's next
/// 32-byte boundary.
#[doc(alias = "THPDec_803300E0")]
fn mcu_buffers(info: &mut FileInfo) {
    let p = info.scratch.next_multiple_of(32);
    info.scratch = p + 16 * 128;
    for (i, b) in info.mcu_buffer.iter_mut().enumerate() {
        *b = p + 128 * i as u32;
    }
}

/// `THPDec_80330158`: the JFIF segment, which must carry no thumbnail.
#[doc(alias = "THPDec_80330158")]
fn read_app0(info: &mut FileInfo, data: &[u8]) -> Result<(), u8> {
    let length = FileInfo::u16_at(data, info.file);
    info.file += 2;
    for &c in b"JFIF\0" {
        if info.take(data) != c {
            return Err(3);
        }
    }
    info.file += 2 + 1 + 4;
    let x_thumb = info.take(data);
    let y_thumb = info.take(data);
    if x_thumb != 0 || y_thumb != 0 {
        return Err(7);
    }
    // The segment's length against how far it read from the data's start.
    if u32::from(length) + 4 != info.file - 4 {
        return Err(8);
    }
    Ok(())
}

/// `__THPReadFrameHeader`.
#[doc(alias = "__THPReadFrameHeader")]
fn read_frame_header(info: &mut FileInfo, data: &[u8]) -> Result<(), u8> {
    info.file += 2;
    if info.take(data) != 8 {
        return Err(10);
    }
    info.y_pixel_size = FileInfo::u16_at(data, info.file);
    info.file += 2;
    info.x_pixel_size = FileInfo::u16_at(data, info.file);
    info.file += 2;
    info.n_components = info.take(data);
    if info.n_components != 3 && info.n_components != 1 {
        return Err(12);
    }
    for i in 0..usize::from(info.n_components) {
        let id = info.take(data);
        let f = info.take(data);
        let q = info.take(data);
        let c = &mut info.components[i];
        c.id = id;
        c.sampling_h = f >> 4;
        c.sampling_v = f & 0xF;
        c.quant_selector = q;
    }
    info.sampling_h_max = 1;
    info.sampling_v_max = 1;
    for c in &info.components[..usize::from(info.n_components)] {
        info.sampling_h_max = info.sampling_h_max.max(c.sampling_h);
        info.sampling_v_max = info.sampling_v_max.max(c.sampling_v);
    }
    let y = i32::from(info.y_pixel_size);
    info.x8d4 = round_up(y, round_up(y, i32::from(info.sampling_v_max) * 8)) as u16;
    for k in 0..usize::from(info.n_components) {
        let (h, v) = (
            i32::from(info.components[k].sampling_h),
            i32::from(info.components[k].sampling_v),
        );
        info.components[k].x08 = round_up(
            i32::from(info.x_pixel_size) * h,
            i32::from(info.sampling_h_max),
        ) as u32;
        info.components[k].x0c = round_up(
            i32::from(info.y_pixel_size) * v,
            i32::from(info.sampling_v_max),
        ) as u32;
    }
    Ok(())
}

/// `__THPReadQuantizationTable`: each table in natural order, scaled by the AAN factors of its
/// row and column.
#[doc(alias = "__THPReadQuantizationTable")]
fn read_quantization_table(info: &mut FileInfo, data: &[u8]) {
    let mut length = FileInfo::u16_at(data, info.file).wrapping_sub(2);
    info.file += 2;
    loop {
        let id = usize::from(info.take(data));
        let mut q = [0.0f64; 64];
        for &n in NATURAL_ORDER.iter().take(64) {
            q[usize::from(n)] = fp::frsp(f64::from(info.take(data)));
        }
        info.valid_quant_tabs |= (1u32 << id) as u8;
        for (row, &r) in AAN_SCALE.iter().enumerate() {
            for (col, &c) in AAN_SCALE.iter().enumerate() {
                let i = row * 8 + col;
                let v = fp::frsp(fp::fmul(c, fp::fmul(q[i], r)));
                info.quant_tabs.0[id][i] = f32::from_bits(fp::stfs(v));
            }
        }
        length = length.wrapping_sub(65);
        if length == 0 {
            break;
        }
    }
}

/// `__THPReadHuffmanTableSpecification`.
#[doc(alias = "__THPReadHuffmanTableSpecification")]
fn read_huffman_tables(info: &mut FileInfo, data: &[u8]) -> Result<(), u8> {
    let mut length = FileInfo::u16_at(data, info.file).wrapping_sub(2);
    info.file += 2;
    loop {
        let i = info.take(data);
        let id = i & 15;
        let class = i >> 4;
        let bits = info.file;
        let t = usize::from((id << 1).wrapping_add(class));
        let mut num_vij: u16 = 0;
        for _ in 0..16 {
            num_vij = num_vij.wrapping_add(u16::from(info.take(data)));
        }
        let h = &mut info.huffman_tabs[t];
        h.bits = bits;
        h.vij = info.file;
        h.num_vij = num_vij;
        info.file += u32::from(num_vij);
        size_table(info, data, t);
        code_table(info, t);
        decoder_tables(info, data, t);
        info.valid_huffman_tabs |= (1u32 << t) as u8;
        length = length.wrapping_sub(17 + num_vij);
        if length == 0 {
            break;
        }
    }
    Ok(())
}

/// `__THPHuffGenerateSizeTable`.
#[doc(alias = "__THPHuffGenerateSizeTable")]
fn size_table(info: &mut FileInfo, data: &[u8], t: usize) {
    let bits = info.huffman_tabs[t].bits;
    let p: u32 = (0..16).map(|l| u32::from(byte(data, bits + l))).sum();
    let h = &mut info.huffman_tabs[t];
    h.size_tab = info.scratch;
    info.scratch += p + 1;
    h.sizes.clear();
    for l in 1..=16u32 {
        for _ in 0..byte(data, bits + l - 1) {
            h.sizes.push(l as i8);
        }
    }
    h.num_codes = h.sizes.len() as u32;
    h.sizes.push(0);
}

/// `__THPHuffGenerateCodeTable`.
#[doc(alias = "__THPHuffGenerateCodeTable")]
fn code_table(info: &mut FileInfo, t: usize) {
    let h = &mut info.huffman_tabs[t];
    let n = h
        .sizes
        .iter()
        .position(|&s| s == 0)
        .unwrap_or(h.sizes.len());
    h.code_tab = info.scratch;
    info.scratch += 4 * n as u32;
    h.codes.clear();
    let mut code: u32 = 0;
    let mut p = 0;
    let mut si = i32::from(h.sizes[0]);
    while h.sizes[p] != 0 {
        while i32::from(h.sizes[p]) == si {
            h.codes.push(code);
            p += 1;
            code = code.wrapping_add(1);
        }
        code <<= 1;
        si += 1;
    }
}

/// `__THPHuffGenerateDecoderTables`.
#[doc(alias = "__THPHuffGenerateDecoderTables")]
fn decoder_tables(info: &mut FileInfo, data: &[u8], t: usize) {
    let h = &mut info.huffman_tabs[t];
    let mut p: i32 = 0;
    for l in 1..=16usize {
        let n = byte(data, h.bits + l as u32 - 1);
        if n != 0 {
            h.val_ptr[l] = p - h.codes[p as usize] as i32;
            p += i32::from(n);
            h.max_code[l] = h.codes[(p - 1) as usize] as i32;
        } else {
            h.max_code[l] = -1;
            h.val_ptr[l] = -1;
        }
    }
    h.max_code[17] = 0xFFFFF;
}

/// `__THPReadScaneHeader`: each component's tables, and the scratch space for its rows.
#[doc(alias = "__THPReadScaneHeader")]
fn read_scan_header(info: &mut FileInfo, data: &[u8]) -> Result<(), u8> {
    info.file += 2;
    let n = info.take(data);
    if n != info.n_components {
        return Err(12);
    }
    for i in 0..usize::from(n) {
        info.file += 1;
        let s = info.take(data);
        info.components[i].dc_selector = s >> 4;
        info.components[i].ac_selector = s & 0xF;
        if u32::from(info.valid_huffman_tabs) & (1 << (s >> 4)) == 0 {
            return Err(15);
        }
        if u32::from(info.valid_huffman_tabs) & (1 << ((s & 0xF) + 1)) == 0 {
            return Err(15);
        }
        info.x74 = info.x_size;
        info.decompressed_y = info.y_size;
        let per_row = info.components[i].x08 as u16;
        let d = i32::from(info.decompressed_y);
        let m = i32::from(info.x8d4);
        let mut rows = round_up(d, m) as u16;
        rows = rows.wrapping_add(if d % m == 0 { 0 } else { 1 });
        rows = rows.wrapping_mul(info.x8d4);
        let shift = i32::from(info.sampling_v_max) - i32::from(info.components[i].sampling_v);
        let rows = (i32::from(rows) >> shift) as u32;
        info.components[i].x10 = info.scratch;
        info.scratch = info
            .scratch
            .wrapping_add(u32::from(per_row).wrapping_mul(rows));
    }
    info.file += 3;
    mcu_layout(info)
}

/// `THPDec_803310CC`: the MCUs' layout and each component's blocks in one.
#[doc(alias = "THPDec_803310CC")]
fn mcu_layout(info: &mut FileInfo) -> Result<(), u8> {
    let hm = i32::from(info.sampling_h_max) * 8;
    let vm = i32::from(info.sampling_v_max) * 8;
    info.mcus_per_row = round_up(i32::from(info.x_pixel_size), hm) as u16;
    info.x8d0 = round_up(i32::from(info.y_pixel_size), vm) as u16;
    info.x8ce = 0;
    for i in 0..usize::from(info.n_components) {
        let c = &mut info.components[i];
        c.x28 = round_up(i32::from(info.x_pixel_size) * i32::from(c.sampling_h), hm);
        c.x24 = round_up(i32::from(info.y_pixel_size) * i32::from(c.sampling_v), vm);
        c.x14 = i32::from(c.sampling_h);
        c.x18 = i32::from(c.sampling_v);
        c.x1c = c.x14 * c.x18;
        c.x20 = c.x14 * 8;
        let mut j = c.x1c;
        if i32::from(info.x8ce) + c.x1c > 0x10 {
            return Err(0x11);
        }
        while j > 0 {
            j -= 1;
            info.x8bc[usize::from(info.x8ce)] = i as u8;
            info.x8ce += 1;
        }
        if info.x8ce > 6 {
            // "THP does not support anything other than 4:2:0!"
            return Ok(());
        }
        info.components[i].pred_dc = 0;
    }
    Ok(())
}

/// `__THPRestartDefinition`.
#[doc(alias = "__THPRestartDefinition")]
fn restart_definition(info: &mut FileInfo, data: &[u8]) {
    info.rst = 1;
    info.file += 2;
    info.n_mcu = FileInfo::u16_at(data, info.file);
    info.file += 2;
    info.curr_mcu = info.n_mcu;
}

/// The three planes a frame decodes into, in GX's I8 texture layout.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Planes {
    pub y: Vec<u8>,
    pub u: Vec<u8>,
    pub v: Vec<u8>,
}

/// `slw`/`srw`: shifts of 32 or more give zero.
fn slw(x: u32, n: u32) -> u32 {
    let n = n & 63;
    if n >= 32 { 0 } else { x << n }
}

/// The decoder's state while it decodes a scan: the work area's bit stream, and the frame.
struct Scan<'a> {
    info: &'a mut FileInfo,
    data: &'a [u8],
}

impl Scan<'_> {
    /// The bits read so far from the record's start: `file`'s word and `cnt`.
    fn bit_pos(&self) -> u32 {
        self.info.file * 8 + self.info.cnt - 1
    }

    /// Puts the stream at bit `pos`, as the original leaves `file`, `currByte` and `cnt`: a
    /// word wholly read stays current with `cnt` 33.
    fn set_bit_pos(&mut self, pos: u32) {
        let (w, b) = if pos > 0 && pos.is_multiple_of(32) {
            (pos / 32 - 1, 32)
        } else {
            (pos / 32, pos % 32)
        };
        self.info.file = 4 * w;
        self.info.cnt = b + 1;
        self.info.curr_byte = word(self.data, self.info.file);
    }

    fn bit(&self, pos: u32) -> u32 {
        let w = word(self.data, 4 * (pos / 32));
        (w >> (31 - pos % 32)) & 1
    }

    /// `n` bits as a number, MSB first.
    fn bits(&mut self, n: u32) -> u32 {
        let mut pos = self.bit_pos();
        let mut v = 0;
        for _ in 0..n {
            v = (v << 1) | self.bit(pos);
            pos += 1;
        }
        self.set_bit_pos(pos);
        v
    }

    /// `__THPHuffDecodeTab`: a code of table `t`, MSB first, checked against each length's
    /// largest code.
    fn decode(&mut self, t: usize) -> i32 {
        let h = &self.info.huffman_tabs[t];
        let mut pos = self.bit_pos();
        let mut code: i32 = 0;
        for l in 1..=17usize {
            code = (code << 1) | self.bit(pos) as i32;
            pos += 1;
            if code <= h.max_code[l] {
                let at = (code + h.val_ptr[l]) as u32;
                let v = byte(self.data, h.vij.wrapping_add(at));
                self.set_bit_pos(pos);
                return i32::from(v);
            }
        }
        unreachable!("maxCode[17] ends every code")
    }

    /// The `t`-bit difference after a code, extended to its sign.
    fn receive(&mut self, t: u32) -> i32 {
        let v = self.bits(t);
        let mut v = v as i32;
        if v.leading_zeros() > 32 - t {
            v = v.wrapping_add(slw(0xFFFF_FFFF, t).wrapping_add(1) as i32);
        }
        v
    }

    /// `__THPHuffDecodeDCTCompY`, `__THPHuffDecodeDCTCompU` and `__THPHuffDecodeDCTCompV`:
    /// component `c`'s next block into `block`.
    fn block(&mut self, c: usize, block: &mut [i16; 64]) {
        *block = [0; 64];
        let comp = self.info.components[c];
        let dc_tab = usize::from(comp.dc_selector) << 1;
        let ac_tab = (usize::from(comp.ac_selector) << 1) + 1;
        let t = self.decode(dc_tab);
        let diff = if t != 0 { self.receive(t as u32) } else { 0 };
        let dc = comp.pred_dc.wrapping_add(diff as i16);
        self.info.components[c].pred_dc = dc;
        block[0] = dc;
        let mut k: usize = 1;
        while k < 64 {
            let s = self.decode(ac_tab);
            let rrrr = (s & 15) as u32;
            let run = s >> 4;
            if rrrr != 0 {
                k += run as usize;
                let v = self.receive(rrrr);
                block[usize::from(NATURAL_ORDER[k])] = v as i16;
            } else {
                if run != 15 {
                    break;
                }
                k += 15;
            }
            k += 1;
        }
    }
}

/// The IDCT's constants as `lfs` loads them, both lanes (the original's digits; `cc4` is
/// the SDK's literal for the square root of two).
#[allow(clippy::excessive_precision, clippy::approx_constant)]
fn cc4() -> Ps {
    Ps::splat(f64::from(1.414213562f32))
}
#[allow(clippy::excessive_precision)]
fn cc2() -> Ps {
    Ps::splat(f64::from(1.847759065f32))
}
#[allow(clippy::excessive_precision)]
fn cc2c6s() -> Ps {
    Ps::splat(f64::from(1.082392200f32))
}
#[allow(clippy::excessive_precision)]
fn cc2c6a() -> Ps {
    Ps::splat(f64::from(-2.613125930f32))
}
fn bias() -> Ps {
    Ps::splat(1024.0)
}

/// `psq_l` of two `s16` coefficients (GQR5: type s16, scale 0).
fn load_coefs(block: &[i16; 64], i: usize) -> Ps {
    Ps::new(f64::from(block[i] as f32), f64::from(block[i + 1] as f32))
}

/// `psq_l` of two singles.
fn load_pair(a: f32, b: f32) -> Ps {
    Ps::new(fp::lfs(a.to_bits()), fp::lfs(b.to_bits()))
}

/// `psq_st` of a pair as singles (GQR0), as the workspace keeps them.
fn store_pair(ws: &mut [f32; 64], i: usize, v: Ps) {
    ws[i] = f32::from_bits(fp::stfs_ftz(v.ps0));
    ws[i + 1] = f32::from_bits(fp::stfs_ftz(v.ps1));
}

/// `psq_st` of a pair as clamped bytes, an eighth of each (GQR6: type u8, scale -3).
fn store_samples(out: &mut [u8], at: usize, v: Ps) {
    let q = |x: f64| ((x as f32) * 0.125).clamp(0.0, 255.0) as u8;
    out[at] = q(v.ps0);
    out[at + 1] = q(v.ps1);
}

/// The rows' pass of `__THPInverseDCTNoYPos` and `__THPInverseDCTY8` (the same in both): each
/// row of the block dequantized and transformed into the workspace, by the shortcut its zero
/// coefficients allow.
fn idct_rows(block: &[i16; 64], q: &[f32; 64], ws: &mut [f32; 64]) {
    let (cc4, cc2, cc2c6s, cc2c6a) = (cc4(), cc2(), cc2c6s(), cc2c6a());
    for r in 0..8 {
        let i = r * 8;
        let o = r * 8;
        let qq = |k: usize| load_pair(q[i + k], q[i + k + 1]);
        let tmp10 = fp::ps_mul(load_coefs(block, i), qq(0));
        let hi = block[i + 4..i + 8].iter().any(|&c| c != 0);
        let mid = block[i + 2] != 0 || block[i + 3] != 0;
        let one = block[i + 1] != 0;
        if hi {
            // `_regularIDCT`.
            let tmp9 = fp::ps_mul(load_coefs(block, i + 2), qq(2));
            let tmp2 = load_coefs(block, i + 4);
            let tmp6 = qq(4);
            let tmp0 = fp::ps_merge01(tmp10, tmp9);
            let tmp3 = load_coefs(block, i + 6);
            let tmp1 = fp::ps_merge01(tmp9, tmp10);
            let tmp7 = qq(6);
            let tmp4 = fp::ps_madd(tmp2, tmp6, tmp0);
            let tmp5 = fp::ps_nmsub(tmp2, tmp6, tmp0);
            let tmp6 = fp::ps_madd(tmp3, tmp7, tmp1);
            let tmp7 = fp::ps_nmsub(tmp3, tmp7, tmp1);
            let tmp0 = fp::ps_add(tmp4, tmp6);
            let tmp3 = fp::ps_sub(tmp4, tmp6);
            let tmp2 = fp::ps_msub(tmp7, cc4, tmp6);
            let tmp8 = fp::ps_sub(tmp7, tmp5);
            let tmp1 = fp::ps_add(tmp5, tmp2);
            let tmp2 = fp::ps_sub(tmp5, tmp2);
            let tmp8 = fp::ps_mul(tmp8, cc2);
            let tmp1 = fp::ps_merge00(tmp0, tmp1);
            let tmp6 = fp::ps_nmsub(tmp5, cc2c6a, tmp8);
            let tmp4 = fp::ps_msub(tmp7, cc2c6s, tmp8);
            let tmp6 = fp::ps_sub(tmp6, tmp0);
            let tmp2 = fp::ps_merge00(tmp2, tmp3);
            let tmp5 = fp::ps_madd(tmp3, cc4, tmp6);
            let tmp7 = fp::ps_merge11(tmp0, tmp6);
            let tmp4 = fp::ps_sub(tmp4, tmp5);
            let tmp3 = fp::ps_add(tmp1, tmp7);
            let tmp4 = fp::ps_merge11(tmp5, tmp4);
            let tmp0 = fp::ps_sub(tmp1, tmp7);
            let tmp5 = fp::ps_add(tmp2, tmp4);
            let tmp6 = fp::ps_sub(tmp2, tmp4);
            let tmp5 = fp::ps_merge10(tmp5, tmp5);
            let tmp0 = fp::ps_merge10(tmp0, tmp0);
            store_pair(ws, o, tmp3);
            store_pair(ws, o + 2, tmp6);
            store_pair(ws, o + 4, tmp5);
            store_pair(ws, o + 6, tmp0);
        } else if mid {
            // `_halfIDCT`.
            let tmp1 = fp::ps_mul(load_coefs(block, i + 2), qq(2));
            let tmp3 = fp::ps_sub(tmp10, tmp1);
            let tmp2 = fp::ps_add(tmp10, tmp1);
            let tmp4 = fp::ps_madd(tmp1, cc4, tmp3);
            let tmp5 = fp::ps_nmsub(tmp1, cc4, tmp2);
            let tmp8 = fp::ps_mul(tmp3, cc2);
            let tmp4 = fp::ps_merge00(tmp2, tmp4);
            let tmp6 = fp::ps_nmsub(tmp1, cc2c6a, tmp8);
            let tmp5 = fp::ps_merge00(tmp5, tmp3);
            let tmp6 = fp::ps_sub(tmp6, tmp2);
            let tmp7 = fp::ps_nmsub(tmp10, cc2c6s, tmp8);
            let tmp2 = fp::ps_merge11(tmp2, tmp6);
            let tmp8 = fp::ps_msub(tmp3, cc4, tmp6);
            let tmp9 = fp::ps_add(tmp4, tmp2);
            let tmp7 = fp::ps_sub(tmp7, tmp8);
            let tmp3 = fp::ps_merge11(tmp8, tmp7);
            let tmp4 = fp::ps_sub(tmp4, tmp2);
            let tmp0 = fp::ps_add(tmp5, tmp3);
            let tmp1 = fp::ps_sub(tmp5, tmp3);
            let tmp1 = fp::ps_merge10(tmp1, tmp1);
            let tmp4 = fp::ps_merge10(tmp4, tmp4);
            store_pair(ws, o, tmp9);
            store_pair(ws, o + 2, tmp0);
            store_pair(ws, o + 4, tmp1);
            store_pair(ws, o + 6, tmp4);
        } else if one {
            // `_quarterIDCT`.
            let tmp2 = fp::ps_msub(tmp10, cc2, tmp10);
            let tmp9 = fp::ps_merge00(tmp10, tmp10);
            let tmp1 = fp::ps_sub(cc2, cc2c6s);
            let tmp3 = fp::ps_msub(tmp10, cc4, tmp2);
            let tmp5 = fp::ps_merge11(tmp10, tmp2);
            let tmp4 = fp::ps_nmsub(tmp10, tmp1, tmp3);
            let tmp7 = fp::ps_add(tmp9, tmp5);
            let tmp6 = fp::ps_merge11(tmp3, tmp4);
            let tmp5 = fp::ps_sub(tmp9, tmp5);
            let tmp8 = fp::ps_add(tmp9, tmp6);
            let tmp6 = fp::ps_sub(tmp9, tmp6);
            let tmp6 = fp::ps_merge10(tmp6, tmp6);
            let tmp5 = fp::ps_merge10(tmp5, tmp5);
            store_pair(ws, o, tmp7);
            store_pair(ws, o + 2, tmp8);
            store_pair(ws, o + 4, tmp6);
            store_pair(ws, o + 6, tmp5);
        } else {
            // The DC alone.
            let tmp0 = fp::ps_merge00(tmp10, tmp10);
            for k in 0..4 {
                store_pair(ws, o + 2 * k, tmp0);
            }
        }
    }
}

/// The columns' pass: two columns at a time into the plane at `base` (the 8 by 8 block's
/// top-left tile byte), its rows 0 to 3 in one tile row and 4 to 7 in the next (`wid` texels
/// wide).
fn idct_columns(ws: &[f32; 64], out: &mut [u8], base: usize, wid: usize) {
    let (cc4, cc2, cc2c6s, cc2c6a, bias) = (cc4(), cc2(), cc2c6s(), cc2c6a(), bias());
    let w = |row: usize, pair: usize| load_pair(ws[row * 8 + 2 * pair], ws[row * 8 + 2 * pair + 1]);
    let tmp10 = w(0, 0);
    let tmp11 = w(4, 0);
    let tmp12 = w(2, 0);
    let mut tmp6 = fp::ps_add(tmp10, tmp11);
    let tmp13 = w(6, 0);
    let mut tmp8 = fp::ps_sub(tmp10, tmp11);
    tmp6 = fp::ps_add(tmp6, bias);
    let mut tmp7 = fp::ps_add(tmp12, tmp13);
    let mut tmp9 = fp::ps_sub(tmp12, tmp13);
    let mut tmp0 = fp::ps_add(tmp6, tmp7);
    tmp8 = fp::ps_add(tmp8, bias);
    for pair in 0..4 {
        let out0 = base + 2 * pair;
        let out1 = out0 + wid * 4;
        let tmp4 = w(1, pair);
        tmp9 = fp::ps_msub(tmp9, cc4, tmp7);
        let tmp5 = w(3, pair);
        let tmp3 = fp::ps_sub(tmp6, tmp7);
        let tmp1 = fp::ps_add(tmp8, tmp9);
        let t6 = w(5, pair);
        let tmp2 = fp::ps_sub(tmp8, tmp9);
        let t7 = w(7, pair);
        let t8 = fp::ps_add(t6, tmp5);
        let t6 = fp::ps_sub(t6, tmp5);
        let t9 = fp::ps_add(tmp4, t7);
        let t4 = fp::ps_sub(tmp4, t7);
        let t7 = fp::ps_add(t9, t8);
        let t5 = fp::ps_sub(t9, t8);
        let t8 = fp::ps_add(t6, t4);
        let t9 = fp::ps_add(tmp0, t7);
        let t8 = fp::ps_mul(t8, cc2);
        let tmp23 = fp::ps_sub(tmp0, t7);
        let t6 = fp::ps_madd(t6, cc2c6a, t8);
        let t6 = fp::ps_sub(t6, t7);
        store_samples(out, out0, t9);
        let t4 = fp::ps_msub(t4, cc2c6s, t8);
        let t9 = fp::ps_add(tmp1, t6);
        let t5 = fp::ps_msub(t5, cc4, t6);
        let tmp22 = fp::ps_sub(tmp1, t6);
        store_samples(out, out0 + 8, t9);
        let t8 = fp::ps_add(tmp2, t5);
        let t4 = fp::ps_add(t4, t5);
        store_samples(out, out0 + 16, t8);
        let t9 = fp::ps_sub(tmp3, t4);
        let tmp20 = fp::ps_add(tmp3, t4);
        store_samples(out, out0 + 24, t9);
        let tmp21 = fp::ps_sub(tmp2, t5);
        store_samples(out, out1, tmp20);
        store_samples(out, out1 + 8, tmp21);
        store_samples(out, out1 + 16, tmp22);
        store_samples(out, out1 + 24, tmp23);
        if pair < 3 {
            // The next pair's start, as the loop computes it.
            let tmp10 = w(0, pair + 1);
            let tmp11 = w(4, pair + 1);
            let tmp12 = w(2, pair + 1);
            let tmp13 = w(6, pair + 1);
            tmp6 = fp::ps_add(tmp10, tmp11);
            tmp8 = fp::ps_sub(tmp10, tmp11);
            tmp6 = fp::ps_add(tmp6, bias);
            tmp7 = fp::ps_add(tmp12, tmp13);
            tmp9 = fp::ps_sub(tmp12, tmp13);
            tmp0 = fp::ps_add(tmp6, tmp7);
            tmp8 = fp::ps_add(tmp8, bias);
        }
    }
}

/// `__THPInverseDCTNoYPos` (`y8` false) and `__THPInverseDCTY8`: a block into the MCU row's
/// buffer at texel column `x`, in its first eight rows or its second. (Both are inlined:
/// the executable has no symbol for them.)
fn inverse_dct(block: &[i16; 64], q: &[f32; 64], x: u32, y8: bool, out: &mut [u8], wid: u32) {
    let mut ws = [0f32; 64];
    idct_rows(block, q, &mut ws);
    let base = (x * 4) as usize + if y8 { (wid * 8) as usize } else { 0 };
    idct_columns(&ws, out, base, wid as usize);
}

/// `THPDec_80331340` (`width` 640) and `THPDec_803313D0`: the scan decoded MCU row by MCU row
/// into `planes`, each row written whole through the locked cache's buffers
/// (`__THPDecompressiMCURow640x480`, `__THPDecompressiMCURowNxN`). `data` is as
/// [`video_decode`] took it.
#[doc(alias = "THPDec_80331340")]
#[doc(alias = "THPDec_803313D0")]
#[doc(alias = "__THPDecompressiMCURow640x480")]
#[doc(alias = "__THPDecompressiMCURowNxN")]
#[doc(alias = "__THPHuffDecodeDCTCompY")]
#[doc(alias = "__THPHuffDecodeDCTCompU")]
#[doc(alias = "__THPHuffDecodeDCTCompV")]
pub fn decompress(info: &mut FileInfo, data: &[u8], width: u32, planes: &mut Planes) {
    prep_bit_stream(info, data);
    let (yw, cw) = if width == 640 {
        (640, 320)
    } else {
        (width, width / 2)
    };
    // The locked cache's row buffers (`__THPLCWork672`), and what each row stores of them.
    let (y_len, c_len) = if width == 640 {
        (0x2800, 0xA00)
    } else {
        (256 * (width / 16), 64 * (width / 16))
    };
    let mut row_y = vec![0u8; 0x2800.max(y_len as usize)];
    let mut row_u = vec![0u8; 0xA00.max(c_len as usize)];
    let mut row_v = vec![0u8; 0xA00.max(c_len as usize)];
    let mut blocks = [[0i16; 64]; 6];
    while u32::from(info.x8ee) < u32::from(info.x8ea) + u32::from(info.decompressed_y) {
        for cl in 0..u32::from(info.mcus_per_row) {
            {
                let mut s = Scan { info, data };
                for (b, comp) in blocks.iter_mut().zip([0, 0, 0, 0, 1, 2]) {
                    s.block(comp, b);
                }
            }
            let q = |c: usize| info.quant_tabs.0[usize::from(info.components[c].quant_selector)];
            let x = cl * 16;
            let qy = q(0);
            // Wider than the row buffer: what the original writes past the locked cache's
            // buffer is lost.
            ensure(&mut row_y, (x * 4 + yw * 12 + 64) as usize);
            inverse_dct(&blocks[0], &qy, x, false, &mut row_y, yw);
            inverse_dct(&blocks[1], &qy, x + 8, false, &mut row_y, yw);
            inverse_dct(&blocks[2], &qy, x, true, &mut row_y, yw);
            inverse_dct(&blocks[3], &qy, x + 8, true, &mut row_y, yw);
            let x = x / 2;
            ensure(&mut row_u, (x * 4 + cw * 4 + 64) as usize);
            ensure(&mut row_v, (x * 4 + cw * 4 + 64) as usize);
            inverse_dct(&blocks[4], &q(1), x, false, &mut row_u, cw);
            inverse_dct(&blocks[5], &q(2), x, false, &mut row_v, cw);
            if info.rst != 0 {
                info.curr_mcu = info.curr_mcu.wrapping_sub(1);
                if info.curr_mcu == 0 {
                    info.curr_mcu = info.n_mcu;
                    let mut cnt = 1 + ((info.cnt + 6) & 0xFFFF_FFF8);
                    if cnt > 32 {
                        cnt = 33;
                    }
                    info.cnt = cnt;
                    for c in &mut info.components {
                        c.pred_dc = 0;
                    }
                }
            }
        }
        for (plane, row, len, at) in [
            (&mut planes.y, &row_y, y_len, 0),
            (&mut planes.u, &row_u, c_len, 1),
            (&mut planes.v, &row_v, c_len, 2),
        ] {
            let t = info.tile[at] as usize;
            let len = len as usize;
            if plane.len() < t + len {
                plane.resize(t + len, 0);
            }
            plane[t..t + len].copy_from_slice(&row[..len]);
            info.tile[at] += len as u32;
        }
        info.x8ee = info.x8ee.wrapping_add(info.x8d4);
    }
}

fn ensure(v: &mut Vec<u8>, n: usize) {
    if v.len() < n {
        v.resize(n, 0);
    }
}

/// `__THPPrepBitStream`: the stream at the scan's first aligned word, and each table's
/// five-bit lookup (`quick`, `increment`); the components' tables chosen.
#[doc(alias = "__THPPrepBitStream")]
fn prep_bit_stream(info: &mut FileInfo, data: &[u8]) {
    let offset = info.file & 3;
    if info.cnt != 33 {
        info.cnt = info.cnt.wrapping_sub((3 - offset) * 8);
    } else {
        info.cnt = offset * 8 + 1;
    }
    info.file &= !3;
    info.curr_byte = word(data, info.file);
    for i in 0..4 {
        if u32::from(info.valid_huffman_tabs) & (1 << i) == 0 {
            continue;
        }
        let h = &mut info.huffman_tabs[i];
        for j in 0..32 {
            h.quick[j] = 0xFF;
            for k in 0..5 {
                let code = (j >> (5 - k - 1)) as i32;
                if code <= h.max_code[k + 1] {
                    let at = (code + h.val_ptr[k + 1]) as u32;
                    h.quick[j] = byte(data, h.vij.wrapping_add(at));
                    h.increment[j] = (k + 1) as u8;
                    break;
                }
            }
        }
    }
}

/// A frame's planes for an image of `width` by `height` (`fn_8001EF5C`'s decode): `frame` is
/// the frame's record as the file holds it (its next frame's size, then its data).
pub fn decode_frame(frame: &[u8], width: u16, height: u16) -> Result<Planes, u8> {
    let mut info = video_decode(width, height, frame)?;
    let w = u32::from(width);
    let h = u32::from(height);
    let mut planes = Planes {
        y: vec![0; (w * h) as usize],
        u: vec![0; (w * h / 4) as usize],
        v: vec![0; (w * h / 4) as usize],
    };
    decompress(&mut info, frame, w, &mut planes);
    Ok(planes)
}
