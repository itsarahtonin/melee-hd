//! melee-save's ports of HSD's save encryption (`sysdolphin/baselib/crypt.c`) and card file
//! format (`sysdolphin/baselib/card.c`) against ssbm-rs's, bit for bit.
//!
//! The format's functions run on a `CardState` laid out in the reference's memory as melee-save
//! keeps it (the original's bytes), on random states: random file sizes, flags, banner and
//! icon formats, and blocks scattered over the physical slots with random sequence numbers,
//! stale copies and free blocks, around the layouts the game makes (the save's, a snapshot's).
//! Each request's planner runs on both from the same state, and the state after, the commands
//! queued on the ring (every field their handlers read), the request made active and the result
//! are compared.

use leaf_check::{Rand, SCRATCH, handle, reference};
use melee_save::crypt;
use melee_save::hsd::{self, ActiveKind, Buf, Cmd, Ptr, Ring};
use melee_save::state::{CardState, IconInfo, seq_cmp};
use ssbm_rt::{Ctx, FnPtr};
use ssbm_types::fns;
use ssbm_types::tu::sysdolphin__baselib__card as card;
use ssbm_types::tu::sysdolphin__baselib__crypt as crypt_tu;

const BUF: u32 = SCRATCH;
const DEST: u32 = SCRATCH + 0x3000;
const STATE: u32 = SCRATCH + 0x4000;
const ICON: u32 = SCRATCH + 0x4800;
const TABLE: u32 = SCRATCH + 0x4900;
const SECTOR_BUF: u32 = SCRATCH + 0x6000;
/// Where the requests' buffers stand; only their addresses reach the ring.
const FILES: u32 = 0x8170_0000;
const COMMENT: u32 = 0x816F_0000;
const BANNER: u32 = 0x816F_1000;
const ICONS: u32 = 0x816F_4000;
const FILENAME: u32 = 0x816F_6000;
const CALLBACK: u32 = 0x8000_5000;
const COMMANDS: u32 = 0x804D_1148;
const ACTIVE: u32 = 0x804D_1138;

fn bytes(ctx: &Ctx, at: u32, n: usize) -> Vec<u8> {
    (0..n as u32).map(|i| ctx.read_u8(at + i)).collect()
}

fn random_bytes(r: &mut Rand, n: usize) -> Vec<u8> {
    (0..n).map(|_| r.bits() as u8).collect()
}

/// Lengths around the ones the layer uses (block 0's 0x190, a sector) and random ones.
fn lengths(r: &mut Rand, n: usize, min: usize) -> Vec<usize> {
    let mut v: Vec<usize> = [0, 1, 15, 16, 17, 31, 32, 0x190, 0x1E40, 0x1E70, 0x2000]
        .into_iter()
        .filter(|&l| l >= min)
        .collect();
    while v.len() < n {
        v.push(min + r.below(0x2001 - min as u64) as usize);
    }
    v
}

#[test]
fn checksum_matches() {
    let Some(ctx) = reference() else { return };
    let mut r = Rand(0x5EED_C4EC_0001);
    for (k, len) in lengths(&mut r, 3000, 0).into_iter().enumerate() {
        let mut data = random_bytes(&mut r, len);
        if k % 3 == 1 {
            // Bytes that make neighbouring sums equal, for the inversion pass.
            let init = crypt::checksum(&[]);
            for i in 1..len.min(16) {
                if r.below(2) == 0 {
                    data[i] = init[i - 1].wrapping_sub(init[i]);
                }
            }
            if len > 16 {
                data[16..].fill(0);
            }
        }
        ctx.write_bytes(BUF, &data);
        fns::HSD_Checksum(&ctx, handle(&ctx, BUF), len as i32, handle(&ctx, DEST));
        assert_eq!(
            bytes(&ctx, DEST, 16),
            crypt::checksum(&data),
            "HSD_Checksum of {len} bytes, case {k}"
        );
    }
}

#[test]
fn encrypt_and_decrypt_match() {
    let Some(ctx) = reference() else { return };
    let mut r = Rand(0x5EED_C4EC_0002);
    for (k, len) in lengths(&mut r, 2000, 16).into_iter().enumerate() {
        let plain = random_bytes(&mut r, len);
        ctx.write_bytes(BUF, &plain);
        assert_eq!(fns::HSD_Encrypt(&ctx, handle(&ctx, BUF), len as i32), 0);
        let mut ours = plain.clone();
        crypt::encrypt(&mut ours);
        assert_eq!(
            bytes(&ctx, BUF, len),
            ours,
            "HSD_Encrypt of {len} bytes, case {k}"
        );
        // Deciphered as written, then with a byte or the checksum broken.
        let mut cipher = ours.clone();
        match k % 3 {
            1 if len > 16 => {
                let at = 16 + r.below(len as u64 - 16) as usize;
                cipher[at] ^= 1 << r.below(8);
            }
            2 => cipher[r.below(16) as usize] ^= 0x80,
            _ => {}
        }
        ctx.write_bytes(BUF, &cipher);
        let theirs = fns::HSD_Decrypt(&ctx, handle(&ctx, BUF), len as i32);
        let mut ours = cipher.clone();
        let ok = crypt::decrypt(&mut ours);
        assert_eq!(
            theirs,
            if ok { 0 } else { -1 },
            "HSD_Decrypt's result, {len} bytes, case {k}"
        );
        assert_eq!(
            bytes(&ctx, BUF, len),
            ours,
            "HSD_Decrypt of {len} bytes, case {k}"
        );
    }
}

#[test]
fn bytes_match() {
    let Some(ctx) = reference() else { return };
    for prev in 0..=255u8 {
        for cur in 0..=255u8 {
            assert_eq!(
                crypt_tu::encryptByte(&ctx, prev, i32::from(cur)),
                i32::from(crypt::encrypt_byte(prev, cur)),
                "encryptByte({prev}, {cur})"
            );
            assert_eq!(
                crypt_tu::decryptByte(&ctx, u32::from(prev), u32::from(cur)),
                i32::from(crypt::decrypt_byte(prev, cur)),
                "decryptByte({prev}, {cur})"
            );
        }
    }
}

fn random_icon(r: &mut Rand) -> IconInfo {
    let mut info = IconInfo {
        banner_format: r.below(4) as u8,
        unused: r.below(3) as u8,
        ..IconInfo::default()
    };
    let icons = r.below(9) as usize;
    for i in 0..8 {
        info.icon_format[i] = r.below(4) as u8;
        info.icon_speed[i] = if i < icons {
            1 + r.below(3) as u8
        } else {
            r.below(2) as u8 * 2
        };
    }
    if r.below(3) == 0 {
        // The game's: a direct-color banner and one indexed icon.
        info = melee_save::melee::icon_info();
    }
    info
}

/// A file layout: the save's, a snapshot's, or random files of random flags.
fn random_files(r: &mut Rand, s: &mut CardState) {
    match r.below(4) {
        0 => {
            for (i, e) in melee_save::melee::manifest().files.iter().enumerate() {
                if e.size != 0 {
                    s.set_file(i, e.size, e.flags, FILES + 0x1_0000 * i as u32);
                }
            }
        }
        1 => s.set_file(0, 0x100 + r.below(0x40000) as i32, 3, FILES),
        _ => {
            for i in 0..9 {
                if r.below(3) == 0 {
                    let size = match r.below(4) {
                        0 => 1 + r.below(0x1FE0) as i32,
                        1 => 0x1FE0 * (1 + r.below(3) as i32),
                        2 => 1 + r.below(0x6000) as i32,
                        _ => r.below(0x8_0000) as i32,
                    };
                    let data = if r.below(4) == 0 {
                        0
                    } else {
                        FILES + 0x1_0000 * i
                    };
                    // Flags are two bits in a block's file table.
                    s.set_file(i as usize, size, r.below(4) as i32, data);
                }
            }
        }
    }
}

/// A state as a scan leaves it: the files, their blocks scattered with sequence numbers.
fn random_state(r: &mut Rand) -> CardState {
    let sector = if r.below(8) == 0 { 0x4000 } else { 0x2000 };
    let mut s = CardState::new(0, sector, SECTOR_BUF);
    s.set_file_no(r.below(127) as i32);
    s.set_icon_info(&random_icon(r));
    // A layout that fits a card: the card file's blocks are what an open's repair found, so
    // the requests after one see at most a card's worth (the original's maps hold 64).
    loop {
        random_files(r, &mut s);
        let blocks: i32 = (0..9).map(|f| s.file_blocks(f) as i32).sum();
        if s.total_blocks() <= 62 && blocks <= 62 {
            break;
        }
        for f in 0..9 {
            s.set_file(f, 0, 0, 0);
        }
    }
    let total = s.total_blocks();
    let num = match r.below(6) {
        0 => total + r.below(3) as i32 - 1,
        _ => total,
    }
    .clamp(0, 62);
    s.set_num_blocks(num);
    let ids: i32 = (0..9).map(|f| s.file_blocks(f) as i32).sum::<i32>() + 1;
    let base_seq = r.below(256) as i32;
    if r.below(2) == 0 {
        // As a create lays the blocks out (first copies, spares, mirrors), then saves and
        // damage: blocks rewritten a sequence number on, rotated, lost or duplicated.
        let mut layout = vec![(melee_save::state::FREE, 0); num as usize + 1];
        let mut phys = 1usize;
        let mut id = 1 + (s.file_blocks(0) as i32 - 1).max(0);
        if s.file_blocks(0) > 0 {
            layout[0] = (0, base_seq);
            for k in 1..s.file_blocks(0) as i32 {
                if phys < layout.len() {
                    layout[phys] = (k, base_seq);
                }
                phys += 1;
            }
        }
        let mut mirrors = Vec::new();
        for f in 1..9 {
            for _ in 0..s.file_blocks(f) {
                if phys < layout.len() {
                    layout[phys] = (id, base_seq);
                }
                if s.file_flags(f) == 0 {
                    mirrors.push(id);
                }
                phys += 1;
                id += 1;
            }
        }
        phys += (s.total_blocks() as usize).saturating_sub(phys - 1 + mirrors.len());
        for m in mirrors {
            if phys < layout.len() {
                layout[phys] = (m, base_seq);
            }
            phys += 1;
        }
        for (p, &(id, seq)) in layout.iter().enumerate() {
            let (id, seq) = match r.below(40) {
                0 => (melee_save::state::FREE, 0),
                1 => (id, (seq + 1) & 0xFF),
                2 => (-id, seq),
                3 => (
                    layout[r.below(layout.len() as u64) as usize].0,
                    (seq + r.below(2) as i32) & 0xFF,
                ),
                _ => (id, seq),
            };
            s.set_block_id(p, id);
            s.set_block_seq(p, seq);
        }
        return s;
    }
    for p in 0..=num as usize {
        let id = match r.below(8) {
            0 => melee_save::state::FREE,
            1 => -(r.below(ids.max(1) as u64) as i32),
            2 => 0xFFFF,
            _ => r.below(ids.max(1) as u64) as i32,
        };
        let seq = match r.below(4) {
            0 => r.below(256) as i32,
            _ => (base_seq + r.below(3) as i32) & 0xFF,
        };
        s.set_block_id(p, id);
        s.set_block_seq(p, seq);
    }
    s
}

fn put_state(ctx: &Ctx, s: &CardState) {
    ctx.write_bytes(STATE, s.bytes());
}

fn get_state(ctx: &Ctx) -> CardState {
    CardState::from_bytes(bytes(ctx, STATE, s_len()).try_into().unwrap())
}

fn s_len() -> usize {
    CardState::default().bytes().len()
}

#[test]
fn format_helpers_match() {
    let Some(ctx) = reference() else { return };
    let ctx = &ctx;
    let mut r = Rand(0x5EED_C4EC_0003);
    let st = || handle::<ssbm_types::records::CardState>(ctx, STATE);
    for case in 0..20_000 {
        let mut s = random_state(&mut r);
        put_state(ctx, &s);
        let info = s.icon_info();
        assert_eq!(
            fns::hsd_803AC340(ctx, handle(ctx, STATE + 0x3B0)) as u32,
            info.header_size(),
            "hsd_803AC340, case {case}"
        );
        for i in 0..9 {
            assert_eq!(
                card::fn_803AC634(ctx, st(), i),
                s.file_blocks(i as usize),
                "fn_803AC634({i}), case {case}"
            );
        }
        for i in 0..=9 {
            assert_eq!(
                card::fn_803AC6B8(ctx, st(), i),
                s.blocks_before(i as usize),
                "fn_803AC6B8({i}), case {case}"
            );
            card::fn_803AC3F8(ctx, st(), handle(ctx, TABLE), i);
            assert_eq!(
                bytes(ctx, TABLE, 12),
                s.pack_table(i),
                "fn_803AC3F8({i}), case {case}"
            );
        }
        assert_eq!(
            card::fn_803AC7DC(ctx, st()),
            s.total_blocks(),
            "fn_803AC7DC, case {case}"
        );
        let phys = r.below(70) as i32 - 2;
        assert_eq!(
            card::fn_803ACBE8(ctx, st(), phys),
            s.block_offset(phys),
            "fn_803ACBE8({phys}), case {case}"
        );
        // A file table from a block, any index a byte can hold.
        let mut table = [0u8; 12];
        for e in table.as_chunks_mut::<4>().0 {
            e[0] = match r.below(4) {
                0 => r.bits() as u8,
                _ => r.below(10) as u8,
            };
            let size = if r.below(4) == 0 {
                0
            } else {
                r.bits() as u32 & 0xFF_FFFF
            };
            e[1..].copy_from_slice(&size.to_be_bytes()[1..]);
        }
        ctx.write_bytes(TABLE, &table);
        card::hsd_803AC558(ctx, st(), handle(ctx, TABLE));
        s.unpack_table(&table);
        assert!(get_state(ctx) == s, "hsd_803AC558, case {case}");
        // Registering a file, the icons, the sectors a new file needs, a reset.
        let (i, size, flags, data) = (
            r.below(9) as i32,
            r.bits() as i32,
            r.below(5) as i32,
            r.bits() as u32,
        );
        fns::hsd_803AC3E0(ctx, st(), i, size, flags, handle(ctx, data));
        s.set_file(i as usize, size, flags, data);
        assert!(get_state(ctx) == s, "hsd_803AC3E0, case {case}");
        let icon = random_icon(&mut r);
        ctx.write_bytes(ICON, &icon.to_bytes());
        assert_eq!(
            fns::hsd_SetCardIconInfo(ctx, st(), handle(ctx, ICON)),
            s.set_icon_info(&icon)
        );
        assert!(get_state(ctx) == s, "hsd_SetCardIconInfo, case {case}");
        let mut s = random_state(&mut r);
        put_state(ctx, &s);
        assert_eq!(
            fns::hsd_803B2674(ctx, st()),
            s.sectors_needed(),
            "hsd_803B2674, case {case}"
        );
        assert!(get_state(ctx) == s, "hsd_803B2674's state, case {case}");
        let (chan, sector, buf) = (
            r.below(2) as i32,
            0x2000 << r.below(2),
            r.bits() as u32 & !0x1F,
        );
        fns::hsd_803B24E4(ctx, st(), chan, sector as i32, handle(ctx, buf));
        assert!(
            get_state(ctx) == CardState::new(chan, sector, buf),
            "hsd_803B24E4, case {case}"
        );
    }
}

#[test]
fn seq_cmp_matches() {
    let Some(ctx) = reference() else { return };
    for a in -2..=257 {
        for b in -2..=257 {
            assert_eq!(
                card::fn_803ACB74(&ctx, a, b),
                seq_cmp(a, b),
                "fn_803ACB74({a}, {b})"
            );
        }
    }
}

fn addr(p: Ptr) -> u32 {
    let base = match p.buf {
        Buf::File(i) => FILES + 0x1_0000 * u32::from(i),
        Buf::Comment => COMMENT,
        Buf::Banner => BANNER,
        Buf::Icons => ICONS,
        Buf::Filename => FILENAME,
        Buf::SectorBuf => SECTOR_BUF,
    };
    base + p.off
}

fn opt(p: Option<Ptr>) -> u32 {
    p.map_or(0, addr)
}

/// A command's type and the words of its arguments its handler reads, by offset.
fn words(c: &Cmd) -> (u32, Vec<(u32, u32)>) {
    let w = |v: &[(u32, u32)]| v.to_vec();
    match *c {
        Cmd::WriteBlock {
            file_idx,
            phys,
            block_id,
            seq,
            data,
            offset,
            size,
        } => (
            1,
            w(&[
                (0x8, file_idx as u32),
                (0xC, phys as u32),
                (0x10, block_id as u32),
                (0x14, seq as u32),
                (0x18, opt(data)),
                (0x1C, offset as u32),
                (0x20, size as u32),
            ]),
        ),
        Cmd::ReadBlock {
            phys,
            data,
            offset,
            size,
        } => (
            2,
            w(&[
                (0x10, phys as u32),
                (0x18, opt(data)),
                (0x1C, offset as u32),
                (0x20, size as u32),
            ]),
        ),
        Cmd::ClearBuf { data, size } => (4, w(&[(0x18, addr(data)), (0x20, size as u32)])),
        Cmd::VerifyBlock {
            block_id,
            seq,
            data,
            offset,
            size,
        } => (
            5,
            w(&[
                (0x10, block_id as u32),
                (0x14, seq as u32),
                (0x18, opt(data)),
                (0x1C, offset as u32),
                (0x20, size as u32),
            ]),
        ),
        Cmd::CheckVerified => (6, vec![]),
        Cmd::CreateFile { filename, size } => (7, w(&[(0x8, addr(filename)), (0xC, size as u32)])),
        Cmd::SetStatus => (8, vec![]),
        Cmd::WriteHeader {
            index,
            banner,
            icons,
        } => (
            9,
            w(&[(0x8, index as u32), (0xC, opt(banner)), (0x10, opt(icons))]),
        ),
        Cmd::VerifyHeader {
            index,
            banner,
            icons,
        } => (
            0xA,
            w(&[(0x8, index as u32), (0xC, opt(banner)), (0x10, opt(icons))]),
        ),
        Cmd::ReadHeader {
            index,
            comment,
            banner,
            icons,
        } => (
            0xB,
            w(&[
                (0x8, index as u32),
                (0xC, opt(comment)),
                (0x10, opt(banner)),
                (0x14, opt(icons)),
            ]),
        ),
        Cmd::GetStatus { file_no } => (0xC, w(&[(0x8, file_no as u32)])),
        Cmd::ScanBlock { phys, offset } => (0xD, w(&[(0x10, phys as u32), (0x1C, offset as u32)])),
        Cmd::Repair => (0xE, vec![]),
        Cmd::ReadSector { phys, offset } => (0xF, w(&[(0xC, phys as u32), (0x1C, offset as u32)])),
        Cmd::WriteSector {
            phys,
            block_id,
            seq,
            offset,
        } => (
            0x10,
            w(&[
                (0xC, phys as u32),
                (0x10, block_id as u32),
                (0x14, seq as u32),
                (0x1C, offset as u32),
            ]),
        ),
        Cmd::ScanFile => (0x11, vec![]),
    }
}

fn active_type(k: ActiveKind) -> u32 {
    match k {
        ActiveKind::ReadFile => 1,
        ActiveKind::WriteFile => 2,
        ActiveKind::WriteFile12 => 3,
        ActiveKind::WriteFile3 => 4,
        ActiveKind::OpenOrReadHeader => 5,
        ActiveKind::CreateFile => 6,
        ActiveKind::SetStatus => 7,
    }
}

/// Empties the reference's ring and request, idle as `fn_803AA790` finds them.
fn reset_ring(ctx: &Ctx) {
    ctx.fill(COMMANDS, 0, 128 * 0x24);
    ctx.fill(ACTIVE, 0, 0x10);
    card::curr_head(ctx).set(0);
    card::curr_tail(ctx).set(0);
    card::hsd_804D7998(ctx).set(-1);
    card::hsd_804D799C(ctx).set(2);
}

/// The reference's ring, request and state after a call against ours.
fn compare(ctx: &Ctx, what: &str, theirs: i32, ours: i32, ring: &Ring, s: &CardState) {
    assert_eq!(theirs, ours, "{what}: result");
    let tail = card::curr_tail(ctx).get() as u32;
    assert_eq!(tail as usize, ring.cmds.len(), "{what}: commands queued");
    for (i, c) in ring.cmds.iter().enumerate() {
        let at = COMMANDS + 0x24 * i as u32;
        let (ty, args) = words(c);
        assert_eq!(ctx.read_u32(at), ty, "{what}: command {i}'s type ({c:?})");
        assert_eq!(ctx.read_u32(at + 4), STATE, "{what}: command {i}'s state");
        for (off, v) in args {
            assert_eq!(
                ctx.read_u32(at + off),
                v,
                "{what}: command {i} ({c:?}) at +{off:#x}"
            );
        }
    }
    match ring.active {
        Some(a) => {
            assert_eq!(
                ctx.read_u32(ACTIVE),
                active_type(a.kind),
                "{what}: active request"
            );
            assert_eq!(ctx.read_u32(ACTIVE + 4), STATE, "{what}: active state");
            assert_eq!(
                ctx.read_u32(ACTIVE + 8),
                CALLBACK,
                "{what}: active callback"
            );
            assert_eq!(
                ctx.read_u32(ACTIVE + 12) as i32,
                a.arg,
                "{what}: active argument"
            );
        }
        None => assert_eq!(ctx.read_u32(ACTIVE), 0, "{what}: no request active"),
    }
    assert!(get_state(ctx) == *s, "{what}: state");
}

#[test]
fn repair_matches() {
    let Some(ctx) = reference() else { return };
    let ctx = &ctx;
    let mut r = Rand(0x5EED_C4EC_0004);
    let mut results = std::collections::BTreeMap::new();
    let mut queued = std::collections::BTreeMap::<String, u32>::new();
    for case in 0..30_000 {
        let mut s = random_state(&mut r);
        put_state(ctx, &s);
        reset_ring(ctx);
        let theirs = card::fn_803AD16C(ctx, handle(ctx, STATE));
        let mut ring = Ring::default();
        let ours = hsd::repair(&mut ring, &mut s);
        compare(
            ctx,
            &format!("fn_803AD16C, case {case}"),
            theirs,
            ours,
            &ring,
            &s,
        );
        *results.entry(ours).or_insert(0) += 1;
        for c in &ring.cmds {
            *queued
                .entry(
                    format!("{:?}", c)
                        .split([' ', '{'])
                        .next()
                        .unwrap()
                        .to_owned(),
                )
                .or_insert(0) += 1;
        }
    }
    eprintln!("fn_803AD16C's results over the cases: {results:?}; commands queued: {queued:?}");
}

#[test]
fn requests_match() {
    let Some(ctx) = reference() else { return };
    let ctx = &ctx;
    let mut r = Rand(0x5EED_C4EC_0005);
    let st = || handle::<ssbm_types::records::CardState>(ctx, STATE);
    let cb = || handle::<FnPtr>(ctx, CALLBACK);
    let p = |b: Buf| Some(Ptr::at(b, 0));
    let mut counts = std::collections::BTreeMap::<&str, (u32, u32)>::new();
    for case in 0..40_000 {
        let mut s = random_state(&mut r);
        put_state(ctx, &s);
        reset_ring(ctx);
        let mut ring = Ring::default();
        let file = r.below(9) as i32;
        let file_ptr = Ptr::at(Buf::File(file as u8), 0);
        let (what, theirs, ours) = match r.below(13) {
            0 => (
                "fn_803ADF90",
                card::fn_803ADF90(ctx, st(), file, handle(ctx, addr(file_ptr)), 1, cb()),
                hsd::read_file(&mut ring, &s, file, file_ptr),
            ),
            1 => (
                "fn_803AE7F8",
                card::fn_803AE7F8(ctx, st(), file, handle(ctx, addr(file_ptr)), 1, cb()),
                hsd::write_twice(&mut ring, &s, file, file_ptr),
            ),
            2 => (
                "fn_803AF3F0",
                card::fn_803AF3F0(ctx, st(), file, handle(ctx, addr(file_ptr)), 1, cb()),
                hsd::write_alternating(&mut ring, &s, file, file_ptr),
            ),
            3 => (
                "fn_803B1338",
                card::fn_803B1338(ctx, st(), 1),
                hsd::write_new_blocks(&mut ring, &s),
            ),
            12 => (
                "fn_803B0120",
                card::fn_803B0120(ctx, st(), file, handle(ctx, addr(file_ptr)), 1, cb()),
                hsd::write_once(&mut ring, &s, file, file_ptr),
            ),
            4 => {
                let is_new = r.below(2) as i32;
                (
                    "fn_803B0E9C",
                    card::fn_803B0E9C(
                        ctx,
                        st(),
                        handle(ctx, BANNER),
                        handle(ctx, ICONS),
                        is_new,
                        1,
                    ),
                    hsd::write_header(
                        &mut ring,
                        &mut s,
                        p(Buf::Banner),
                        p(Buf::Icons),
                        is_new != 0,
                    ),
                )
            }
            5 => (
                "fn_803B1F78",
                card::fn_803B1F78(
                    ctx,
                    st(),
                    handle(ctx, FILENAME),
                    handle(ctx, BANNER),
                    handle(ctx, ICONS),
                    cb(),
                ),
                hsd::create(&mut ring, &mut s, p(Buf::Banner), p(Buf::Icons)),
            ),
            6 => (
                "fn_803B21E8",
                card::fn_803B21E8(ctx, st(), handle(ctx, BANNER), handle(ctx, ICONS), cb()),
                hsd::set_status(&mut ring, &mut s, p(Buf::Banner), p(Buf::Icons)),
            ),
            7 => {
                let no = r.below(127) as i32;
                (
                    "fn_803ADE4C",
                    card::fn_803ADE4C(ctx, st(), no, cb()),
                    hsd::open(&mut ring, no),
                )
            }
            8 => (
                "fn_803B26CC",
                card::fn_803B26CC(
                    ctx,
                    st(),
                    handle(ctx, COMMENT),
                    handle(ctx, BANNER),
                    handle(ctx, ICONS),
                    cb(),
                ),
                hsd::read_header(
                    &mut ring,
                    &mut s,
                    p(Buf::Comment),
                    p(Buf::Banner),
                    p(Buf::Icons),
                ),
            ),
            9 => (
                "fn_803ACF30",
                card::fn_803ACF30(ctx, st(), handle(ctx, 0), handle(ctx, 0), handle(ctx, 0)),
                hsd::queue_read_header(&mut ring, &s, None, None, None),
            ),
            10 => {
                let blk = r.below(64) as i32;
                (
                    "fn_803AC258",
                    card::fn_803AC258(ctx, st(), blk),
                    hsd::queue_scan(&mut ring, &s, blk),
                )
            }
            _ => (
                "fn_803AC2A4",
                card::fn_803AC2A4(ctx, st()),
                hsd::queue_repair(&mut ring),
            ),
        };
        compare(
            ctx,
            &format!("{what}, case {case}"),
            theirs,
            ours,
            &ring,
            &s,
        );
        let c = counts.entry(what).or_default();
        c.0 += 1;
        c.1 += u32::from(ours < 0);
    }
    eprintln!("requests compared (calls, failing results): {counts:?}");
}
