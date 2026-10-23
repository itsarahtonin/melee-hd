//! melee-sim's port of the SDK's THP decoder (`melee_sim::thp`) against ssbm-rs's, frame by
//! frame over the disc's movies: each frame's headers read into the work area
//! (`THPVideoDecode`), then its scan decoded into the three planes (`THPDec_80331340`), as
//! the movie player does. Every field of the work area (the bit stream where each step left
//! it, the tables, the components, the tiles' places), the size and code tables in its scratch
//! space and every byte of the planes are compared.
//!
//! The opening movie's 3,036 frames run whole, and every eighth frame of the other movies
//! (How to Play, the Special Movie and the endings), which the same decoder plays.

use leaf_check::{SCRATCH, each_set, handle, reference};
use melee_sim::thp::{self, FileInfo};
use ssbm_rt::{Ctx, Handle};
use ssbm_types::fns;

/// Where the frame's record, the work area, the planes and the arguments stand: each at a
/// 32-byte boundary, as the movie player's buffer has them.
const REC: u32 = SCRATCH;
const WORK: u32 = SCRATCH + 0x2_0000;
const PY: u32 = SCRATCH + 0x4_0000;
const PU: u32 = PY + 640 * 480;
const PV: u32 = PU + 320 * 240;
const HDR: u32 = SCRATCH + 0x1_F000;
const DESC: u32 = SCRATCH + 0x1_F020;
const STATUS: u32 = SCRATCH + 0x1_F040;

/// A movie's frames: each frame's record (its next frame's size, then its data).
fn frames(file: &[u8]) -> Vec<(usize, usize)> {
    let be = |at: usize| u32::from_be_bytes(file[at..at + 4].try_into().unwrap()) as usize;
    let (n, first, first_size) = (be(0x1C), be(0x20), be(0x28));
    let mut out = Vec::with_capacity(n);
    let (mut at, mut size) = (first, first_size);
    for _ in 0..n {
        out.push((at, size));
        let next = be(at);
        at += size;
        size = next;
    }
    out
}

fn read_bytes(ctx: &Ctx, at: u32, n: usize) -> Vec<u8> {
    (0..n as u32).map(|i| ctx.read_u8(at + i)).collect()
}

/// The reference's work area as melee-sim's [`FileInfo`] keeps it: pointers into the frame
/// as offsets from the record, into the work area from its start, into the planes from each
/// plane's.
fn theirs(ctx: &Ctx) -> FileInfo {
    let w = |o: u32| ctx.read_u32(WORK + o);
    let h = |o: u32| ctx.read_u16(WORK + o);
    let b = |o: u32| ctx.read_u8(WORK + o);
    let rec = |a: u32| a.wrapping_sub(REC);
    let work = |a: u32| a.wrapping_sub(WORK);
    let mut info = FileInfo {
        file: rec(w(0)),
        curr_byte: w(4),
        cnt: w(8),
        x_size: h(0x50),
        y_size: h(0x52),
        scan_start: rec(w(0x6C)),
        x_pixel_size: h(0x70),
        y_pixel_size: h(0x72),
        x74: h(0x74),
        decompressed_y: h(0x76),
        valid_huffman_tabs: b(0x78),
        valid_quant_tabs: b(0x79),
        sampling_h_max: b(0x7A),
        sampling_v_max: b(0x7B),
        n_components: b(0x7C),
        x7d: b(0x7D),
        mcus_per_row: h(0x8CC),
        x8ce: h(0x8CE),
        x8d0: h(0x8D0),
        x8d2: h(0x8D2),
        x8d4: h(0x8D4),
        x8e8: h(0x8E8),
        x8ea: h(0x8EA),
        x8ec: h(0x8EC),
        x8ee: h(0x8EE),
        tile: [
            w(0x8F0).wrapping_sub(PY),
            w(0x8F4).wrapping_sub(PU),
            w(0x8F8).wrapping_sub(PV),
        ],
        n_mcu: h(0x8FC),
        curr_mcu: h(0x8FE),
        rst: b(0x900),
        scratch: work(w(0x904)),
        ..FileInfo::default()
    };
    for i in 0..16 {
        info.mcu_buffer[i] = work(w(0x10 + 4 * i as u32));
    }
    for (t, tab) in info.huffman_tabs.iter_mut().enumerate() {
        let o = 0x88 + 0xEC * t as u32;
        for j in 0..32 {
            tab.quick[j] = b(o + j as u32);
            tab.increment[j] = b(o + 0x20 + j as u32);
        }
        tab.vij = rec(w(o + 0x40));
        for l in 0..18 {
            tab.max_code[l] = w(o + 0x44 + 4 * l as u32) as i32;
            tab.val_ptr[l] = w(o + 0x8C + 4 * l as u32) as i32;
        }
        tab.bits = rec(w(o + 0xD8));
        tab.size_tab = work(w(o + 0xDC));
        tab.code_tab = work(w(o + 0xE0));
        tab.num_codes = w(o + 0xE4);
        tab.num_vij = h(o + 0xE8);
    }
    for (q, tab) in info.quant_tabs.0.iter_mut().enumerate() {
        for (i, v) in tab.iter_mut().enumerate() {
            *v = f32::from_bits(w(0x438 + 4 * (64 * q + i) as u32));
        }
    }
    for (i, c) in info.components.iter_mut().enumerate() {
        let o = 0x838 + 0x2C * i as u32;
        c.id = b(o);
        c.sampling_h = b(o + 1);
        c.sampling_v = b(o + 2);
        c.quant_selector = b(o + 3);
        c.dc_selector = b(o + 4);
        c.ac_selector = b(o + 5);
        c.pred_dc = h(o + 6) as i16;
        c.x08 = w(o + 8);
        c.x0c = w(o + 0xC);
        c.x10 = work(w(o + 0x10));
        c.x14 = w(o + 0x14) as i32;
        c.x18 = w(o + 0x18) as i32;
        c.x1c = w(o + 0x1C) as i32;
        c.x20 = w(o + 0x20) as i32;
        c.x24 = w(o + 0x24) as i32;
        c.x28 = w(o + 0x28) as i32;
    }
    for i in 0..16 {
        info.x8bc[i] = b(0x8BC + i as u32);
    }
    info
}

/// melee-sim's tables as the work area's scratch space holds them, from the reference's
/// work area.
fn check_tables(ctx: &Ctx, ours: &FileInfo, what: &str) {
    for (t, tab) in ours.huffman_tabs.iter().enumerate() {
        if ours.valid_huffman_tabs & (1 << t) == 0 {
            continue;
        }
        let sizes: Vec<i8> = (0..tab.sizes.len() as u32)
            .map(|i| ctx.read_u8(WORK + tab.size_tab + i) as i8)
            .collect();
        assert_eq!(sizes, tab.sizes, "{what}: table {t}'s sizes");
        let codes: Vec<u32> = (0..tab.codes.len() as u32)
            .map(|i| ctx.read_u32(WORK + tab.code_tab + 4 * i))
            .collect();
        assert_eq!(codes, tab.codes, "{what}: table {t}'s codes");
    }
}

/// Compares the work areas, leaving out the tables' contents (checked from memory).
fn check_info(ctx: &Ctx, ours: &FileInfo, what: &str) {
    let mut t = theirs(ctx);
    for (a, b) in t.huffman_tabs.iter_mut().zip(&ours.huffman_tabs) {
        a.sizes.clone_from(&b.sizes);
        a.codes.clone_from(&b.codes);
    }
    let bits = |q: &thp::QuantTabs| q.0.map(|t| t.map(f32::to_bits));
    assert_eq!(
        bits(&t.quant_tabs),
        bits(&ours.quant_tabs),
        "{what}: quantization tables"
    );
    t.quant_tabs = ours.quant_tabs;
    assert_eq!(t, *ours, "{what}: the work area");
}

/// Decodes `frames` of `file` on both sides and compares, one reference per call.
fn run(name: &str, file: &[u8], frames: &[(usize, usize)]) {
    let ctx = reference().expect("SSBM_DISC");
    fns::THPInit(&ctx);
    let be = |at: usize| u32::from_be_bytes(file[at..at + 4].try_into().unwrap());
    let (w, h) = (be(0x10) as u16, be(0x14) as u16);
    ctx.write_u16(HDR, w);
    ctx.write_u16(HDR + 2, h);
    ctx.write_u32(DESC, 0);
    ctx.write_u16(DESC + 4, w);
    ctx.write_u16(DESC + 6, h);
    ctx.write_u8(DESC + 8, 4);
    for &(at, size) in frames {
        let what = format!("{name} frame at {at:#x}");
        // The read's buffer: the record rounded up to 32 bytes, as the player reads it.
        let len = size.next_multiple_of(32).min(file.len() - at);
        let rec = &file[at..at + len];
        ctx.write_bytes(REC, rec);
        let got = fns::THPVideoDecode(
            &ctx,
            handle(&ctx, HDR),
            handle(&ctx, STATUS),
            handle(&ctx, WORK),
            handle(&ctx, REC + 4),
            handle(&ctx, DESC),
        );
        let ours = thp::video_decode(w, h, rec);
        let mut info = match ours {
            Ok(i) => {
                assert_eq!(got.addr(), WORK, "{what}: THPVideoDecode's work area");
                i
            }
            Err(s) => {
                assert_eq!(got.addr(), 0, "{what}: THPVideoDecode failed only on ours");
                assert_eq!(ctx.read_u8(STATUS), s, "{what}: the status");
                continue;
            }
        };
        assert_eq!(ctx.read_u8(STATUS), 0, "{what}: the status");
        check_info(&ctx, &info, &what);
        check_tables(&ctx, &info, &what);
        let mut planes = thp::Planes {
            y: vec![0; usize::from(w) * usize::from(h)],
            u: vec![0; usize::from(w) * usize::from(h) / 4],
            v: vec![0; usize::from(w) * usize::from(h) / 4],
        };
        fns::THPDec_80331340(
            &ctx,
            handle(&ctx, WORK),
            handle(&ctx, PY),
            handle(&ctx, PU),
            handle(&ctx, PV),
        );
        thp::decompress(&mut info, rec, u32::from(w), &mut planes);
        check_info(&ctx, &info, &format!("{what}, decoded"));
        for (plane, at, ours) in [
            ("Y", PY, &planes.y),
            ("U", PU, &planes.u),
            ("V", PV, &planes.v),
        ] {
            let theirs = read_bytes(&ctx, at, ours.len());
            if let Some(i) = (0..ours.len()).find(|&i| ours[i] != theirs[i]) {
                panic!(
                    "{what}: {plane} plane differs from byte {i:#x}: ours {:#04x}, theirs {:#04x}",
                    ours[i], theirs[i]
                );
            }
        }
    }
}

fn movie(name: &str) -> Option<Vec<u8>> {
    let path = std::env::var("SSBM_DISC").ok()?;
    let disc = melee_data::disc::Disc::open(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    Some(
        disc.read_file(name)
            .unwrap_or_else(|e| panic!("{name}: {e:?}")),
    )
}

/// `frames`, split among the workers.
fn check(name: &str, file: &[u8], frames: &[(usize, usize)]) {
    let chunks: Vec<&[(usize, usize)]> = frames.chunks(frames.len().div_ceil(16).max(1)).collect();
    each_set(&chunks, |c| c.len(), |c| run(name, file, c));
}

#[test]
fn opening_movie_decodes() {
    let Some(file) = movie("MvOpen.mth") else {
        return;
    };
    let all = frames(&file);
    assert_eq!(all.len(), 3036);
    check("MvOpen.mth", &file, &all);
}

#[test]
fn other_movies_decode() {
    let Some(path) = std::env::var("SSBM_DISC").ok() else {
        return;
    };
    let disc = melee_data::disc::Disc::open(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let names: Vec<String> = disc
        .files()
        .expect("the disc's files")
        .into_iter()
        .filter(|n| n.ends_with(".mth") && !n.ends_with("MvOpen.mth"))
        .collect();
    assert!(names.len() > 3, "the disc's movies: {names:?}");
    for name in names {
        let file = disc.read_file(&name).expect("a movie");
        let some: Vec<(usize, usize)> = frames(&file).into_iter().step_by(8).collect();
        check(&name, &file, &some);
    }
}
