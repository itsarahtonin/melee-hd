//! Runs the HSD card layer's ring (`hsd_803AAA48`, with each command's completion from
//! `hsd_803A949C`) on a mounted card, one command after another: what the original does over
//! several frames as the card's interrupts come in, done at once, as on ssbm-rs's card where
//! every erase and program finishes immediately.
//!
//! The commands' effects are the original's: a write builds the block's header and file table
//! and enciphers it, a verify compares and sets the ring's result, a scan takes in ids and file
//! tables, and the result decides which later commands run (2 a verify's mismatch, 1 all
//! verified so the writes are skipped, negative an error that empties the ring).

use crate::card::{CardError, Mounted, Stat};
use crate::crypt;
use crate::hsd::{self, ActiveKind, Buf, Cmd, Ptr, Ring};
use crate::state::{BLOCK_HEADER, CardState, DIGEST, FREE, banner_size, seq_cmp};

/// The buffers a request's commands point into.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Bufs {
    /// Each logical file's data: a read's destination, a write's source.
    pub files: [Vec<u8>; 9],
    /// A header read's comment.
    pub comment: Vec<u8>,
    pub banner: Vec<u8>,
    pub icons: Vec<u8>,
    /// The file's name, for a create.
    pub filename: Vec<u8>,
}

/// One of the layer's requests (`CardRequest`), as `fn_803AA790` starts them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Request {
    ReadFile(i32),
    WriteFile(i32),
    Create,
    SetStatus,
    Open(i32),
    ReadHeader,
}

/// The HSD layer for one card file: its state, the sector buffer and the ring.
#[derive(Clone, Debug)]
pub struct Layer {
    pub state: CardState,
    pub sector: Vec<u8>,
    pub ring: Ring,
    result: i32,
    /// What the layer asks of the card, in order, for the time it takes: `q` a request taken
    /// up while the layer is idle, `r` a sector read, `w` a sector written, `s` a file's
    /// status set, `c` a file created.
    pub log: Vec<u8>,
    /// Each copy into the request's buffers (all but the sector buffer), in order, with how
    /// many log entries came before it: a read's bytes reach its buffers at the read that
    /// brings them.
    pub puts: Vec<(usize, Ptr, Vec<u8>)>,
}

/// A callback's arguments: the file index (or 0) and the result.
pub type Callback = (i32, i32);

impl Layer {
    /// A layer for a card file of `chan` (`hsd_803B24E4`).
    pub fn new(chan: i32) -> Self {
        Self {
            state: CardState::new(chan, crate::state::SECTOR, 0),
            sector: vec![0; crate::state::SECTOR as usize],
            ring: Ring::default(),
            result: 0,
            log: Vec::new(),
            puts: Vec::new(),
        }
    }

    fn bytes<'b>(&'b self, bufs: &'b Bufs, p: Ptr) -> &'b [u8] {
        let off = p.off as usize;
        let b: &[u8] = match p.buf {
            Buf::File(i) => &bufs.files[usize::from(i)],
            Buf::Comment => &bufs.comment,
            Buf::Banner => &bufs.banner,
            Buf::Icons => &bufs.icons,
            Buf::Filename => &bufs.filename,
            Buf::SectorBuf => &self.sector,
        };
        b.get(off..).unwrap_or(&[])
    }

    /// Copies `src` to `p`, growing a buffer that's too short.
    fn put(&mut self, bufs: &mut Bufs, p: Ptr, src: &[u8]) {
        let b: &mut Vec<u8> = match p.buf {
            Buf::File(i) => &mut bufs.files[usize::from(i)],
            Buf::Comment => &mut bufs.comment,
            Buf::Banner => &mut bufs.banner,
            Buf::Icons => &mut bufs.icons,
            Buf::Filename => &mut bufs.filename,
            Buf::SectorBuf => &mut self.sector,
        };
        let off = p.off as usize;
        if b.len() < off + src.len() {
            b.resize(off + src.len(), 0);
        }
        b[off..off + src.len()].copy_from_slice(src);
        if p.buf != Buf::SectorBuf {
            self.puts.push((self.log.len(), p, src.to_vec()));
        }
    }

    /// `n` bytes at `p`, zeros past the buffer's end.
    fn take(&self, bufs: &Bufs, p: Ptr, n: usize) -> Vec<u8> {
        let mut v = self.bytes(bufs, p).to_vec();
        v.resize(n, 0);
        v
    }

    /// Starts a request as `fn_803AA790` does and runs the ring until it drains: the
    /// callbacks it makes, in order.
    pub fn request(&mut self, card: &mut Mounted, bufs: &mut Bufs, req: Request) -> Vec<Callback> {
        let mut calls = Vec::new();
        self.log.push(b'q');
        let (r, arg) = match req {
            Request::ReadFile(i) => (
                hsd::read_file(
                    &mut self.ring,
                    &self.state,
                    i,
                    Ptr::at(Buf::File(i as u8), 0),
                ),
                i,
            ),
            Request::WriteFile(i) => {
                let buf = Ptr::at(Buf::File(i as u8), 0);
                let r = match self.state.file_flags(i as usize) {
                    0 => hsd::write_twice(&mut self.ring, &self.state, i, buf),
                    1 | 2 => hsd::write_alternating(&mut self.ring, &self.state, i, buf),
                    3 => hsd::write_once(&mut self.ring, &self.state, i, buf),
                    _ => -0x101,
                };
                (r, i)
            }
            Request::Create => (
                hsd::create(
                    &mut self.ring,
                    &mut self.state,
                    Some(Ptr::at(Buf::Banner, 0)),
                    Some(Ptr::at(Buf::Icons, 0)),
                ),
                0,
            ),
            Request::SetStatus => (
                hsd::set_status(
                    &mut self.ring,
                    &mut self.state,
                    Some(Ptr::at(Buf::Banner, 0)),
                    Some(Ptr::at(Buf::Icons, 0)),
                ),
                0,
            ),
            Request::Open(no) => (hsd::open(&mut self.ring, no), 0),
            Request::ReadHeader => (
                hsd::read_header(
                    &mut self.ring,
                    &mut self.state,
                    Some(Ptr::at(Buf::Comment, 0)),
                    Some(Ptr::at(Buf::Banner, 0)),
                    Some(Ptr::at(Buf::Icons, 0)),
                ),
                0,
            ),
        };
        if r < 0 {
            calls.push((arg, r));
        }
        self.run(card, bufs, &mut calls);
        calls
    }

    /// Runs commands until the ring is empty, then completes the active request.
    fn run(&mut self, card: &mut Mounted, bufs: &mut Bufs, calls: &mut Vec<Callback>) {
        loop {
            if self.result < 0 {
                self.ring.cmds.clear();
            }
            let Some(cmd) = self.ring.cmds.pop_front() else {
                if let Some(active) = self.ring.active.take() {
                    if active.kind == ActiveKind::WriteFile12 {
                        self.retire_older_copies(active.arg);
                    }
                    calls.push((active.arg, self.result));
                }
                self.result = 0;
                return;
            };
            self.step(card, bufs, cmd);
        }
    }

    /// The completion of a write of a file kept once: of two copies of a block, the older's id
    /// is negated.
    fn retire_older_copies(&mut self, file_idx: i32) {
        let s = &mut self.state;
        let before = s.blocks_before(file_idx as usize);
        let blocks = s.file_blocks(file_idx as usize) as i32;
        let total = s.total_blocks();
        let mut map = vec![-1i32; blocks.max(0) as usize];
        for j in 1..=total {
            let id = s.block_id(j as usize);
            if id < 0 {
                continue;
            }
            let logical = id - before;
            if !(0..blocks).contains(&logical) {
                continue;
            }
            let l = logical as usize;
            if map[l] < 0 {
                map[l] = j;
            } else if seq_cmp(s.block_seq(map[l] as usize), s.block_seq(j as usize)) < 0 {
                let old = map[l] as usize;
                s.set_block_id(old, s.block_id(old).wrapping_neg());
                map[l] = j;
            } else {
                s.set_block_id(j as usize, id.wrapping_neg());
            }
        }
    }

    fn fail(&mut self, e: CardError) {
        self.result = e.code();
    }

    /// Reads the sector at the file's `offset` into the sector buffer.
    fn load(&mut self, card: &Mounted, offset: i32) -> Result<(), CardError> {
        let no = self.state.file_no();
        card.fast_open(no)?;
        self.log.push(b'r');
        card.read(no, offset as u32, &mut self.sector)
    }

    fn store(&mut self, card: &mut Mounted, offset: i32) -> Result<(), CardError> {
        let no = self.state.file_no();
        card.fast_open(no)?;
        self.log.push(b'w');
        let sector = self.sector.clone();
        card.write(no, offset as u32, &sector)
    }

    fn step(&mut self, card: &mut Mounted, bufs: &mut Bufs, cmd: Cmd) {
        let skip = self.result == 1;
        match cmd {
            Cmd::VerifyBlock {
                block_id,
                seq,
                data,
                offset,
                size,
            } => {
                if self.result == 2 {
                    return;
                }
                if let Err(e) = self.load(card, offset) {
                    return self.fail(e);
                }
                let hdr = if block_id == 0 {
                    if size <= 0 {
                        return;
                    }
                    self.state.first_block_offset() as usize
                } else {
                    0
                };
                if !crypt::decrypt(&mut self.sector[hdr..]) {
                    self.result = 2;
                    return;
                }
                let b = &self.sector[hdr..];
                let id = i32::from(b[0x10]) << 8 | i32::from(b[0x11]);
                let want = data.map(|p| self.take(bufs, p, size.max(0) as usize));
                let b = &self.sector[hdr..];
                if id != block_id
                    || i32::from(b[0x12]) != seq
                    || (size > 0 && want.is_some_and(|w| b[0x20..0x20 + size as usize] != w[..]))
                {
                    self.result = 2;
                }
            }
            Cmd::ReadHeader {
                index,
                comment,
                banner,
                icons,
            } => {
                if let Err(e) = self.state_fast_open(card) {
                    return self.fail(e);
                }
                if index == 0 {
                    self.state.digest_mut().fill(0);
                }
                let offset = index * self.state.sector_size() as i32;
                if let Err(e) = self.load(card, offset) {
                    return self.fail(e);
                }
                self.header_sector(bufs, index, comment, banner, icons, false);
            }
            Cmd::VerifyHeader {
                index,
                banner,
                icons,
            } => {
                if self.result == 2 {
                    return;
                }
                if let Err(e) = self.state_fast_open(card) {
                    return self.fail(e);
                }
                if index == 0 {
                    self.state.digest_mut().fill(0);
                }
                let offset = index * self.state.sector_size() as i32;
                if let Err(e) = self.load(card, offset) {
                    return self.fail(e);
                }
                self.header_sector(bufs, index, None, banner, icons, true);
            }
            Cmd::CheckVerified => self.result = if self.result == 2 { 0 } else { 1 },
            Cmd::ReadBlock {
                phys,
                data,
                offset,
                size,
            } => {
                if skip {
                    return;
                }
                if let Err(e) = self.load(card, offset) {
                    return self.fail(e);
                }
                if phys < 0 {
                    return;
                }
                let hdr = if phys == 0 {
                    if size <= 0 {
                        return;
                    }
                    self.state.first_block_offset() as usize
                } else {
                    0
                };
                if !crypt::decrypt(&mut self.sector[hdr..]) {
                    self.result = -0x105;
                    return;
                }
                if let Some(p) = data
                    && size > 0
                {
                    let src = self.sector[hdr + 0x20..hdr + 0x20 + size as usize].to_vec();
                    self.put(bufs, p, &src);
                }
            }
            Cmd::ReadSector { offset, .. } => {
                if skip {
                    return;
                }
                if let Err(e) = self.load(card, offset) {
                    return self.fail(e);
                }
                if !crypt::decrypt(&mut self.sector) {
                    self.result = -0x105;
                }
            }
            Cmd::ClearBuf { data, size } => {
                if !skip {
                    let zeros = vec![0; size.max(0) as usize];
                    self.put(bufs, data, &zeros);
                }
            }
            Cmd::WriteBlock {
                file_idx,
                phys,
                block_id,
                seq,
                data,
                offset,
                size,
            } => {
                if skip {
                    return;
                }
                if let Err(e) = self.state_fast_open(card) {
                    return self.fail(e);
                }
                let hdr = if block_id > 0 {
                    0
                } else {
                    self.state.first_block_offset() as usize
                };
                let sector = self.state.sector_size() as usize;
                let size = size.max(0) as usize;
                if size > 0
                    && let Some(p) = data
                {
                    let src = self.take(bufs, p, size);
                    self.sector[hdr + 0x20..hdr + 0x20 + size].copy_from_slice(&src);
                }
                self.sector[hdr + size + 0x20..sector].fill(0);
                self.sector[hdr..hdr + 0x20].fill(0);
                self.sector[hdr + 0x10] = (block_id >> 8) as u8;
                self.sector[hdr + 0x11] = block_id as u8;
                self.sector[hdr + 0x12] = seq as u8;
                let table = self.state.pack_table(file_idx);
                self.sector[hdr + 0x13..hdr + 0x1F].copy_from_slice(&table);
                crypt::encrypt(&mut self.sector[hdr..]);
                let p = phys as usize;
                if let Err(e) = self.store(card, offset) {
                    self.state.set_block_id(p, FREE);
                    self.state.set_block_seq(p, 0);
                    return self.fail(e);
                }
                if block_id != 0xFFFF {
                    self.state.set_block_id(p, block_id);
                    self.state.set_block_seq(p, seq);
                } else {
                    self.state.set_block_id(p, FREE);
                    self.state.set_block_seq(p, 0);
                }
            }
            Cmd::WriteSector {
                phys,
                block_id,
                seq,
                offset,
            } => {
                if skip {
                    return;
                }
                if let Err(e) = self.state_fast_open(card) {
                    return self.fail(e);
                }
                crypt::encrypt(&mut self.sector);
                let p = phys as usize;
                if let Err(e) = self.store(card, offset) {
                    self.state.set_block_id(p, FREE);
                    self.state.set_block_seq(p, 0);
                    return self.fail(e);
                }
                if block_id != 0xFFFF {
                    self.state.set_block_id(p, block_id);
                    self.state.set_block_seq(p, seq);
                } else {
                    self.state.set_block_id(p, FREE);
                    self.state.set_block_seq(p, 0);
                }
            }
            Cmd::CreateFile { filename, size } => {
                let name = self.bytes(bufs, filename).to_vec();
                let name = &name[..name.iter().position(|&c| c == 0).unwrap_or(name.len())];
                self.log.push(b'c');
                match card.create(name, size as u32) {
                    Ok(no) => self.state.set_file_no(no),
                    Err(e) => self.fail(e),
                }
            }
            Cmd::SetStatus => {
                if skip {
                    return;
                }
                let no = self.state.file_no();
                let mut stat = match card.get_status(no) {
                    Ok(s) => s,
                    Err(e) => return self.fail(e),
                };
                let info = self.state.icon_info();
                stat.set_comment_addr(0);
                stat.set_icon_addr(0x40);
                stat.set_banner_format((stat.banner_format() & !3) | info.banner_format);
                let (mut format, mut speed) =
                    (u32::from(stat.icon_format()), u32::from(stat.icon_speed()));
                let mut k = 0;
                while k < 8 && info.icon_speed[k] != 0 {
                    let bit = 2 * k as u32;
                    format = (format & !(3 << bit)) | (u32::from(info.icon_format[k]) << bit);
                    speed = (speed & !(3 << bit)) | (u32::from(info.icon_speed[k]) << bit);
                    k += 1;
                }
                for k in k..8 {
                    format &= !(3 << (2 * k));
                    speed &= !(3 << (2 * k));
                }
                stat.set_icon_format(format as u16);
                stat.set_icon_speed(speed as u16);
                self.log.push(b's');
                let r = card.set_status(no, &mut stat);
                self.state.stat_mut().copy_from_slice(&stat.0);
                if let Err(e) = r {
                    self.fail(e);
                }
            }
            Cmd::WriteHeader {
                index,
                banner,
                icons,
            } => {
                if skip {
                    return;
                }
                if let Err(e) = self.state_fast_open(card) {
                    return self.fail(e);
                }
                self.build_header(bufs, index, banner, icons);
                let offset = index * self.state.sector_size() as i32;
                if let Err(e) = self.store(card, offset) {
                    self.fail(e);
                }
            }
            Cmd::GetStatus { file_no } => {
                self.state.set_file_no(file_no);
                match card.get_status(file_no) {
                    Err(e) => self.fail(e),
                    Ok(stat) => {
                        self.unpack_stat(&stat);
                        if stat.icon_addr() != 0x40 {
                            self.result = -0x106;
                        }
                    }
                }
            }
            Cmd::ScanBlock { phys, offset } => {
                if let Err(e) = self.load(card, offset) {
                    return self.fail(e);
                }
                self.scan(phys);
            }
            Cmd::Repair => {
                let r = hsd::repair(&mut self.ring, &mut self.state);
                if r < 0 {
                    self.result = r;
                }
            }
            Cmd::ScanFile => {
                hsd::queue_read_header(&mut self.ring, &self.state, None, None, None);
                for phys in 0..=self.state.num_blocks() {
                    hsd::queue_scan(&mut self.ring, &self.state, phys);
                }
                hsd::queue_repair(&mut self.ring);
            }
        }
    }

    fn state_fast_open(&self, card: &Mounted) -> Result<(), CardError> {
        card.fast_open(self.state.file_no())
    }

    /// `unpackCardStat`: the banner and icons' formats, the header's size and the number of
    /// blocks from the file's status.
    fn unpack_stat(&mut self, stat: &Stat) {
        let mut info = self.state.icon_info();
        info.banner_format = stat.banner_format() & 3;
        for k in 0..8 {
            info.icon_format[k] = ((stat.icon_format() >> (2 * k)) & 3) as u8;
            info.icon_speed[k] = ((stat.icon_speed() >> (2 * k)) & 3) as u8;
        }
        self.state.set_icon_info_bytes(&info);
        self.state.set_header_size(info.header_size());
        let sector = self.state.sector_size();
        let used = self.state.header_size() + sector + 0x2F;
        self.state
            .set_num_blocks((stat.length() / sector).wrapping_sub(used / sector) as i32);
    }

    /// A scanned block's completion: its id and sequence number taken, its file table merged.
    fn scan(&mut self, phys: i32) {
        let p = phys as usize;
        let hdr = if phys == 0 {
            self.state.first_block_offset() as usize
        } else {
            0
        };
        if !crypt::decrypt(&mut self.sector[hdr..]) {
            self.state.set_block_id(p, FREE);
            self.state.set_block_seq(p, 0);
            return;
        }
        let b = &self.sector[hdr..];
        let id = i32::from(b[0x11]) + (i32::from(b[0x10]) << 8);
        let seq = i32::from(b[0x12]);
        let table: [u8; 12] = b[0x13..0x1F].try_into().unwrap();
        self.state.set_block_id(p, id);
        let bad = if phys == 0 {
            id != 0
        } else {
            id < 0 || id > self.state.num_blocks()
        };
        if bad {
            self.state.set_block_id(p, FREE);
            self.state.set_block_seq(p, 0);
        } else {
            self.state.set_block_seq(p, seq);
            self.state.unpack_table(&table);
        }
    }

    /// A header sector read (`CARD_CMD_READ_HEADER`, `CARD_CMD_VERIFY_HEADER`): its parts
    /// copied out or compared, its digest computed and, in the last sector, checked.
    fn header_sector(
        &mut self,
        bufs: &mut Bufs,
        index: i32,
        comment: Option<Ptr>,
        banner: Option<Ptr>,
        icons: Option<Ptr>,
        verify: bool,
    ) {
        let hs = self.state.header_size() as usize;
        let sector = self.state.sector_size() as usize;
        let banner_len = banner_size(self.state.icon_info().banner_format) as usize;
        let mismatch = if verify { 2 } else { -0x107 };
        let at = 0x10 * index as usize;
        let mut last_len = None;
        if index == 0 {
            if verify {
                if self.sector[..0x40] != self.state.comment() {
                    self.result = 2;
                    return;
                }
                if banner_len > 0
                    && let Some(p) = banner
                    && self.sector[0x40..0x40 + banner_len] != self.take(bufs, p, banner_len)[..]
                {
                    self.result = 2;
                    return;
                }
            } else {
                if let Some(p) = comment {
                    let src = self.sector[..0x40].to_vec();
                    self.put(bufs, p, &src);
                }
                if banner_len > 0
                    && let Some(p) = banner
                {
                    let src = self.sector[0x40..0x40 + banner_len].to_vec();
                    self.put(bufs, p, &src);
                }
            }
            let start = banner_len + 0x40;
            let end = if hs > sector { sector } else { hs };
            if let Some(p) = icons {
                if verify {
                    if self.sector[start..end] != self.take(bufs, p, end - start)[..] {
                        self.result = 2;
                        return;
                    }
                } else {
                    let src = self.sector[start..end].to_vec();
                    self.put(bufs, p, &src);
                }
            }
            if hs <= sector {
                last_len = Some(hs);
            } else {
                let sum = crypt::checksum(&self.sector[..sector]);
                self.state.digest_mut()[at..at + 0x10].copy_from_slice(&sum);
            }
        } else {
            let icons_off = sector * index as usize - 0x40 - banner_len;
            let remaining = hs.wrapping_sub(sector * index as usize);
            let len = if remaining > sector {
                sector
            } else {
                remaining
            };
            if let Some(p) = icons {
                let p = Ptr::at(p.buf, p.off + icons_off as u32);
                if verify {
                    if self.sector[..len] != self.take(bufs, p, len)[..] {
                        self.result = 2;
                        return;
                    }
                } else {
                    let src = self.sector[..len].to_vec();
                    self.put(bufs, p, &src);
                }
            }
            if remaining > sector {
                let sum = crypt::checksum(&self.sector[..sector]);
                self.state.digest_mut()[at..at + 0x10].copy_from_slice(&sum);
            } else {
                last_len = Some(remaining);
            }
        }
        if let Some(len) = last_len {
            let sum = crypt::checksum(&self.sector[..len]);
            self.state.digest_mut()[at..at + 0x10].copy_from_slice(&sum);
            if self.sector[len..len + DIGEST as usize] != self.state.digest()[..] {
                self.result = mismatch;
            }
        }
    }

    /// `CARD_CMD_WRITE_HEADER`'s sector: the comment, banner and icons, the digest after the
    /// last of them; the rest of the buffer as it was.
    fn build_header(&mut self, bufs: &Bufs, index: i32, banner: Option<Ptr>, icons: Option<Ptr>) {
        let hs = self.state.header_size() as usize;
        let sector = self.state.sector_size() as usize;
        let banner_len = banner_size(self.state.icon_info().banner_format) as usize;
        let at = 0x10 * index as usize;
        let icons_at = |s: &Self, off: usize, n: usize| {
            icons.map_or_else(
                || vec![0; n],
                |p| s.take(bufs, Ptr::at(p.buf, p.off + off as u32), n),
            )
        };
        if index == 0 {
            let comment = self.state.comment();
            self.sector[..0x40].copy_from_slice(&comment);
            let mut pos = 0x40;
            if banner_len > 0 {
                let b =
                    banner.map_or_else(|| vec![0; banner_len], |p| self.take(bufs, p, banner_len));
                self.sector[0x40..0x40 + banner_len].copy_from_slice(&b);
                pos += banner_len;
            }
            self.state.digest_mut().fill(0);
            if hs > sector {
                let ic = icons_at(self, 0, sector - pos);
                self.sector[pos..sector].copy_from_slice(&ic);
                let sum = crypt::checksum(&self.sector[..sector]);
                self.state.digest_mut()[at..at + 0x10].copy_from_slice(&sum);
            } else {
                let ic = icons_at(self, 0, hs - pos);
                self.sector[pos..hs].copy_from_slice(&ic);
                let sum = crypt::checksum(&self.sector[..hs]);
                self.state.digest_mut()[at..at + 0x10].copy_from_slice(&sum);
                let digest = self.state.digest();
                self.sector[hs..hs + 0x30].copy_from_slice(&digest);
            }
        } else {
            let remaining = hs.wrapping_sub(sector * index as usize);
            let icons_off = sector * index as usize - 0x40 - banner_len;
            if remaining > sector {
                let ic = icons_at(self, icons_off, sector);
                self.sector[..sector].copy_from_slice(&ic);
                let sum = crypt::checksum(&self.sector[..sector]);
                self.state.digest_mut()[at..at + 0x10].copy_from_slice(&sum);
            } else {
                let ic = icons_at(self, icons_off, remaining);
                self.sector[..remaining].copy_from_slice(&ic);
                let sum = crypt::checksum(&self.sector[..remaining]);
                self.state.digest_mut()[at..at + 0x10].copy_from_slice(&sum);
                let digest = self.state.digest();
                self.sector[remaining..remaining + 0x30].copy_from_slice(&digest);
            }
        }
    }
}

/// A block as a write leaves it on the card: the header (`id`, `seq`, the file table), `data`,
/// zeros to the end, enciphered. `out` is the block's bytes, from its header on.
pub fn encode_block(out: &mut [u8], id: i32, seq: i32, table: &[u8; 12], data: &[u8]) {
    let size = data.len();
    out[0x20..0x20 + size].copy_from_slice(data);
    out[0x20 + size..].fill(0);
    out[..0x20].fill(0);
    out[0x10] = (id >> 8) as u8;
    out[0x11] = id as u8;
    out[0x12] = seq as u8;
    out[0x13..0x1F].copy_from_slice(table);
    crypt::encrypt(out);
    let _ = BLOCK_HEADER;
}
