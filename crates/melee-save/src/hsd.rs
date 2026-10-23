//! The HSD card layer's requests (`sysdolphin/baselib/card.c`): each queues commands on the
//! layer's ring, which [`crate::exec`] runs. The planners here are the original's, command for
//! command; only their asynchronous paths are ported, the ones the game takes (every request
//! runs with `async` 1: the synchronous paths, and `fn_803ACC0C`, `fn_803ACD58` and
//! `fn_803ACFC0` that only they call, are dead).

use crate::state::{BLOCK_HEADER, CardState, FREE, seq_cmp};

/// A buffer a command reads or writes, as the original passes pointers: a logical file's data,
/// the header's parts, the file's name, or the layer's sector buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Buf {
    File(u8),
    Comment,
    Banner,
    Icons,
    Filename,
    SectorBuf,
}

/// A pointer: a buffer and an offset into it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Ptr {
    pub buf: Buf,
    pub off: u32,
}

impl Ptr {
    pub fn at(buf: Buf, off: u32) -> Self {
        Self { buf, off }
    }

    fn add(self, n: u32) -> Self {
        Self {
            buf: self.buf,
            off: self.off + n,
        }
    }
}

/// A ring command (`CardCmd`), with the fields its handler reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cmd {
    /// `CARD_CMD_WRITE_BLOCK`: a block's header and `size` bytes of data, the rest cleared,
    /// enciphered and written.
    WriteBlock {
        file_idx: i32,
        phys: i32,
        block_id: i32,
        seq: i32,
        data: Option<Ptr>,
        offset: i32,
        size: i32,
    },
    /// `CARD_CMD_READ_BLOCK`: a block read and deciphered, its data copied out; a negative
    /// `phys` only loads the sector buffer.
    ReadBlock {
        phys: i32,
        data: Option<Ptr>,
        offset: i32,
        size: i32,
    },
    /// `CARD_CMD_CLEAR_BUF`.
    ClearBuf { data: Ptr, size: i32 },
    /// `CARD_CMD_VERIFY_BLOCK`: the block compared with what a write would put there.
    VerifyBlock {
        block_id: i32,
        seq: i32,
        data: Option<Ptr>,
        offset: i32,
        size: i32,
    },
    /// `CARD_CMD_CHECK_VERIFIED`.
    CheckVerified,
    /// `CARD_CMD_CREATE_FILE`.
    CreateFile { filename: Ptr, size: i32 },
    /// `CARD_CMD_SET_STATUS`.
    SetStatus,
    /// `CARD_CMD_WRITE_HEADER`.
    WriteHeader {
        index: i32,
        banner: Option<Ptr>,
        icons: Option<Ptr>,
    },
    /// `CARD_CMD_VERIFY_HEADER`.
    VerifyHeader {
        index: i32,
        banner: Option<Ptr>,
        icons: Option<Ptr>,
    },
    /// `CARD_CMD_READ_HEADER`.
    ReadHeader {
        index: i32,
        comment: Option<Ptr>,
        banner: Option<Ptr>,
        icons: Option<Ptr>,
    },
    /// `CARD_CMD_GET_STATUS`.
    GetStatus { file_no: i32 },
    /// `CARD_CMD_SCAN_BLOCK`: a block's id, sequence number and file table taken in.
    ScanBlock { phys: i32, offset: i32 },
    /// `CARD_CMD_REPAIR` ([`repair`]).
    Repair,
    /// `CARD_CMD_READ_SECTOR`.
    ReadSector { phys: i32, offset: i32 },
    /// `CARD_CMD_WRITE_SECTOR`.
    WriteSector {
        phys: i32,
        block_id: i32,
        seq: i32,
        offset: i32,
    },
    /// `CARD_CMD_SCAN_FILE`: the header and every block scanned, then repaired.
    ScanFile,
}

/// What the ring completes for a request, whose callback runs once it drains.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActiveKind {
    ReadFile,
    WriteFile,
    /// Files of flags 1 and 2: on completion the older of two copies of each block is
    /// marked stale.
    WriteFile12,
    WriteFile3,
    OpenOrReadHeader,
    CreateFile,
    SetStatus,
}

/// The request the ring is completing: its kind and the callback's argument.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Active {
    pub kind: ActiveKind,
    pub arg: i32,
}

/// The ring and the request it's completing, as the planners leave them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Ring {
    pub cmds: std::collections::VecDeque<Cmd>,
    pub active: Option<Active>,
}

impl Ring {
    fn push(&mut self, c: Cmd) {
        self.cmds.push_back(c);
    }

    /// The tail, for a rollback.
    fn snap(&self) -> usize {
        self.cmds.len()
    }

    fn rewind(&mut self, snap: usize) {
        self.cmds.truncate(snap);
    }
}

/// The load of block 0's sector before its write (`phys` 0, nothing copied).
fn read_init(ofs: i32) -> Cmd {
    Cmd::ReadBlock {
        phys: 0,
        data: None,
        offset: ofs,
        size: 0,
    }
}

/// Queues a scan of block `phys` (`CARD_CMD_SCAN_BLOCK`).
#[doc(alias = "fn_803AC258")]
pub fn queue_scan(ring: &mut Ring, state: &CardState, phys: i32) -> i32 {
    ring.push(Cmd::ScanBlock {
        phys,
        offset: state.block_offset(phys),
    });
    0
}

/// Queues the repair (`CARD_CMD_REPAIR`).
#[doc(alias = "fn_803AC2A4")]
pub fn queue_repair(ring: &mut Ring) -> i32 {
    ring.push(Cmd::Repair);
    0
}

/// Queues a read of each header sector into the given buffers.
#[doc(alias = "fn_803ACF30")]
pub fn queue_read_header(
    ring: &mut Ring,
    state: &CardState,
    comment: Option<Ptr>,
    banner: Option<Ptr>,
    icons: Option<Ptr>,
) -> i32 {
    for index in 0..state.header_sectors() {
        ring.push(Cmd::ReadHeader {
            index: index as i32,
            comment,
            banner,
            icons,
        });
    }
    0
}

/// Opens file `file_no`: its status, then a scan of the file (`fn_803ADE4C`).
#[doc(alias = "fn_803ADE4C")]
pub fn open(ring: &mut Ring, file_no: i32) -> i32 {
    ring.push(Cmd::GetStatus { file_no });
    ring.push(Cmd::ScanFile);
    ring.active = Some(Active {
        kind: ActiveKind::OpenOrReadHeader,
        arg: 0,
    });
    0
}

/// Reads the header into the given buffers.
#[doc(alias = "fn_803B26CC")]
pub fn read_header(
    ring: &mut Ring,
    state: &mut CardState,
    comment: Option<Ptr>,
    banner: Option<Ptr>,
    icons: Option<Ptr>,
) -> i32 {
    state.set_header_size(state.icon_info().header_size());
    queue_read_header(ring, state, comment, banner, icons);
    ring.active = Some(Active {
        kind: ActiveKind::OpenOrReadHeader,
        arg: 0,
    });
    0
}

/// Each logical block of a file mapped to the physical block holding it, and the newest
/// sequence number among them: the read's and writes' scan of `block_ids`.
fn data_chunk(state: &CardState, file_idx: usize, i: u32) -> i32 {
    if file_idx == 0 && i == 0 {
        (state.sector_size() - BLOCK_HEADER) as i32 - state.first_block_offset() as i32
    } else {
        (state.sector_size() - BLOCK_HEADER) as i32
    }
}

/// Reads logical file `file_idx` into `buf` (`fn_803ADF90`): -260 when its blocks' sequence
/// numbers differ, -259 when one is missing, before anything is queued.
#[doc(alias = "fn_803ADF90")]
pub fn read_file(ring: &mut Ring, state: &CardState, file_idx: i32, buf: Ptr) -> i32 {
    let idx = file_idx as usize;
    let before = state.blocks_before(idx);
    let file_size = state.file_size(idx);
    let blocks = state.file_blocks(idx) as i32;
    let total = state.total_blocks();
    let mut map = vec![-1i32; blocks.max(0) as usize];
    let mut status = 0;
    let mut seq = -1;
    for phys in 1..=total {
        let logical = state.block_id(phys as usize) - before;
        if (0..blocks).contains(&logical) {
            let s = state.block_seq(phys as usize);
            if seq < 0 {
                seq = s;
            } else {
                if seq != s {
                    status = -260;
                }
                if seq_cmp(seq, s) > 0 {
                    seq = s;
                }
            }
            map[logical as usize] = phys;
        }
    }
    if file_idx == 0 && state.block_id(0) == 0 {
        if let Some(m) = map.first_mut() {
            *m = 0;
        }
        if seq >= 0 && seq != state.block_seq(0) {
            status = -260;
        }
    }
    if map.iter().any(|&p| p < 0) {
        status = -259;
    }
    if status != 0 {
        return status;
    }
    let mut dst = buf;
    let mut remaining = file_size;
    let mut i = 0;
    while i < blocks && remaining > 0 {
        let chunk = data_chunk(state, idx, i as u32);
        let phys = map[i as usize];
        if remaining > chunk {
            if phys >= 0 {
                ring.push(Cmd::ReadBlock {
                    phys,
                    data: Some(dst),
                    offset: state.block_offset(phys),
                    size: chunk,
                });
            } else if chunk != 0 {
                ring.push(Cmd::ClearBuf {
                    data: dst,
                    size: chunk,
                });
            }
            remaining -= chunk;
            dst = dst.add(chunk as u32);
        } else {
            if phys >= 0 {
                ring.push(Cmd::ReadBlock {
                    phys,
                    data: Some(dst),
                    offset: state.block_offset(phys),
                    size: remaining,
                });
            }
            remaining = 0;
        }
        i += 1;
    }
    ring.active = Some(Active {
        kind: ActiveKind::ReadFile,
        arg: file_idx,
    });
    status
}

/// Queues a block's verify, its handler comparing the block with what a write would put there.
fn verify(ring: &mut Ring, state: &CardState, phys: i32, id: i32, seq: i32, data: Ptr, size: i32) {
    if size == 0 {
        return;
    }
    ring.push(Cmd::VerifyBlock {
        block_id: id,
        seq,
        data: Some(data),
        offset: state.block_offset(phys),
        size,
    });
}

/// Queues a block's write; block 0, which shares the header's last sector, after a load of
/// that sector. -0x101 for any other block at physical 0.
#[allow(clippy::too_many_arguments)]
fn write(
    ring: &mut Ring,
    state: &CardState,
    phys: i32,
    id: i32,
    seq: i32,
    data: Option<Ptr>,
    size: i32,
    file_idx: i32,
) -> i32 {
    let ofs = state.block_offset(phys);
    if phys == 0 {
        if id != 0 {
            return -0x101;
        }
        ring.push(read_init(ofs));
    }
    ring.push(Cmd::WriteBlock {
        file_idx,
        phys,
        block_id: id,
        seq,
        data,
        offset: ofs,
        size,
    });
    0
}

/// Writes a file kept twice (flags 0, `fn_803AE7F8`): when both copies are whole at the newest
/// sequence number, verifies them first (the writes then run only if something differs); then
/// writes both with the next number, filling missing copies from free blocks. -257 when a
/// first copy has no block, -258 when a second hasn't (the first is still written).
#[doc(alias = "fn_803AE7F8")]
pub fn write_twice(ring: &mut Ring, state: &CardState, file_idx: i32, buf: Ptr) -> i32 {
    let idx = file_idx as usize;
    let before = state.blocks_before(idx);
    let file_size = state.file_size(idx);
    let blocks = state.file_blocks(idx) as i32;
    let total = state.total_blocks();
    let n = blocks.max(0) as usize;
    let mut map = [vec![-1i32; n], vec![-1i32; n]];
    let mut free = Vec::new();
    let mut seq = -1;
    let mut repair_result = 0;
    for phys in 1..=total {
        let id = state.block_id(phys as usize);
        if id < 0 {
            free.push(phys);
            continue;
        }
        let logical = id - before;
        if (0..blocks).contains(&logical) {
            let s = state.block_seq(phys as usize);
            if seq == -1 || seq_cmp(seq, s) < 0 {
                seq = s;
            }
            let l = logical as usize;
            if map[0][l] < 0 {
                map[0][l] = phys;
            } else {
                map[1][l] = phys;
            }
        }
    }
    let bad = |p: i32| p < 0 || state.block_seq(p as usize) != seq;
    let verify_failed = (0..n).any(|i| bad(map[0][i]) || bad(map[1][i]));
    let [first, second] = &mut map;
    for (a, b) in first.iter_mut().zip(second.iter_mut()) {
        if *a < 0 {
            match free.pop() {
                Some(p) => *a = p,
                None => return -257,
            }
        }
        if *b < 0 {
            match free.pop() {
                Some(p) => *b = p,
                None => repair_result = -258,
            }
        }
    }
    let snap = ring.snap();
    let chunk = (state.sector_size() - BLOCK_HEADER) as i32;
    if !verify_failed {
        for copy in &map {
            let mut remaining = file_size;
            let mut data = buf;
            let mut i = 0;
            while i < blocks && remaining > 0 {
                let phys = copy[i as usize];
                if remaining as u32 > chunk as u32 {
                    if phys >= 0 {
                        verify(ring, state, phys, before + i, seq, data, chunk);
                    }
                    remaining -= chunk;
                    data = data.add(chunk as u32);
                } else {
                    if phys >= 0 {
                        verify(ring, state, phys, before + i, seq, data, remaining);
                    }
                    remaining = 0;
                }
                i += 1;
            }
        }
        ring.push(Cmd::CheckVerified);
    }
    let seq = (seq + 1) & 0xFF;
    for copy in &map {
        let mut remaining = file_size;
        let mut data = buf;
        let mut i = 0;
        while i < blocks && remaining > 0 {
            let phys = copy[i as usize];
            let size = if remaining as u32 > chunk as u32 {
                chunk
            } else {
                remaining
            };
            if phys >= 0
                && write(
                    ring,
                    state,
                    phys,
                    before + i,
                    seq,
                    Some(data),
                    size,
                    file_idx,
                ) < 0
            {
                ring.rewind(snap);
                return -0x101;
            }
            if size == chunk && remaining as u32 > chunk as u32 {
                remaining -= chunk;
                data = data.add(chunk as u32);
            } else {
                remaining = 0;
            }
            i += 1;
        }
    }
    ring.active = Some(Active {
        kind: ActiveKind::WriteFile,
        arg: file_idx,
    });
    repair_result
}

/// Writes a file kept once with spares (flags 1 and 2, `fn_803AF3F0`): verifies the current
/// copy when it's whole, then writes the next sequence number into the file's stale copies,
/// or free blocks, or as a last resort over the current copy; extra stale copies are cleared.
/// -257 when there are too few blocks.
#[doc(alias = "fn_803AF3F0")]
pub fn write_alternating(ring: &mut Ring, state: &CardState, file_idx: i32, buf: Ptr) -> i32 {
    let idx = file_idx as usize;
    let before = state.blocks_before(idx);
    let file_size = state.file_size(idx);
    let blocks = state.file_blocks(idx) as i32;
    let total = state.total_blocks();
    let snap = ring.snap();
    let n = blocks.max(0) as usize;
    let mut primary = vec![-1i32; n];
    let mut secondary: Vec<i32> = vec![-1; n];
    let mut sec_count = 0usize;
    let mut free = Vec::new();
    let mut seq = -1;
    for phys in 1..=total {
        let id = state.block_id(phys as usize);
        if id >= 0 {
            let logical = id - before;
            if (0..blocks).contains(&logical) {
                if seq_cmp(seq, state.block_seq(phys as usize)) < 0 {
                    seq = state.block_seq(phys as usize);
                }
                primary[logical as usize] = phys;
            }
        } else {
            let logical = -(id + before);
            if (0..blocks).contains(&logical) {
                set_at(&mut secondary, sec_count, phys);
                sec_count += 1;
            } else {
                free.push(phys);
            }
        }
    }
    let needs_rewrite = primary
        .iter()
        .any(|&p| p < 0 || state.block_seq(p as usize) != seq);
    let chunk = (state.sector_size() - BLOCK_HEADER) as i32;
    if !needs_rewrite {
        let mut remaining = file_size;
        let mut data = buf;
        let mut i = 0;
        while i < blocks && remaining > 0 {
            let phys = primary[i as usize];
            if remaining as u32 > chunk as u32 {
                verify(ring, state, phys, before + i, seq, data, chunk);
                remaining -= chunk;
                data = data.add(chunk as u32);
            } else {
                verify(ring, state, phys, before + i, seq, data, remaining);
                remaining = 0;
            }
            i += 1;
        }
        ring.push(Cmd::CheckVerified);
    }
    if sec_count > n {
        for &p in &secondary[n..sec_count] {
            if write(ring, state, p, 0xFFFF, 0, None, 0, file_idx) < 0 {
                ring.rewind(snap);
                return -0x101;
            }
        }
    } else if sec_count < n {
        while sec_count < n {
            let Some(p) = free.pop() else { break };
            set_at(&mut secondary, sec_count, p);
            sec_count += 1;
        }
        if sec_count < n {
            for p in primary.iter_mut() {
                if sec_count >= n {
                    break;
                }
                if *p >= 0 {
                    set_at(&mut secondary, sec_count, *p);
                    sec_count += 1;
                    *p = -1;
                }
            }
            if sec_count < n {
                // The original returns with its verifies still queued and no request active.
                return -257;
            }
        }
    }
    let seq = (seq + 1) & 0xFF;
    let mut remaining = file_size;
    let mut data = buf;
    let mut i = 0;
    while i < blocks && remaining > 0 {
        let size = if remaining as u32 > chunk as u32 {
            chunk
        } else {
            remaining
        };
        if write(
            ring,
            state,
            secondary[i as usize],
            before + i,
            seq,
            Some(data),
            size,
            file_idx,
        ) < 0
        {
            ring.rewind(snap);
            return -0x101;
        }
        if remaining as u32 > chunk as u32 {
            remaining -= chunk;
            data = data.add(chunk as u32);
        } else {
            remaining = 0;
        }
        i += 1;
    }
    ring.active = Some(Active {
        kind: ActiveKind::WriteFile12,
        arg: file_idx,
    });
    0
}

/// Writes a file kept once (flags 3, `fn_803B0120`): in place, over its current blocks, after
/// verifying them when they're whole at one sequence number; missing blocks come from its
/// stale copies or free blocks, other stale copies are cleared. File 0's first block is
/// block 0. -257 when there are too few blocks.
#[doc(alias = "fn_803B0120")]
pub fn write_once(ring: &mut Ring, state: &CardState, file_idx: i32, buf: Ptr) -> i32 {
    let idx = file_idx as usize;
    let before = state.blocks_before(idx);
    let file_size = state.file_size(idx);
    let blocks = state.file_blocks(idx) as i32;
    let total = state.total_blocks();
    let snap = ring.snap();
    let n = blocks.max(0) as usize;
    let mut primary = vec![-1i32; n.max(1)];
    let mut secondary: Vec<i32> = Vec::new();
    let mut free = Vec::new();
    let mut seq = -1;
    let mut needs_rewrite = false;
    if file_idx == 0 {
        if state.block_id(0) == 0 {
            seq = state.block_seq(0);
        } else {
            needs_rewrite = true;
        }
        primary[0] = 0;
    }
    for phys in 1..=total {
        let id = state.block_id(phys as usize);
        if id >= 0 {
            let logical = id - before;
            if file_idx == 0 && logical == 0 {
                secondary.push(phys);
            } else if (0..blocks).contains(&logical) {
                let s = state.block_seq(phys as usize);
                if seq_cmp(seq, s) < 0 {
                    seq = s;
                }
                primary[logical as usize] = phys;
            }
        } else {
            let logical = -(id + before);
            if (0..blocks).contains(&logical) {
                secondary.push(phys);
            } else {
                free.push(phys);
            }
        }
    }
    for p in primary.iter_mut().take(n) {
        if *p < 0 {
            needs_rewrite = true;
            if let Some(s) = secondary.pop() {
                *p = s;
            } else if let Some(f) = free.pop() {
                *p = f;
            } else {
                return -257;
            }
        }
    }
    if !needs_rewrite {
        needs_rewrite = primary[..n]
            .iter()
            .any(|&p| p < 0 || state.block_seq(p as usize) != seq);
    }
    let chunk_of = |id: i32| {
        if id == 0 {
            (state.sector_size() - BLOCK_HEADER) as i32 - state.first_block_offset() as i32
        } else {
            (state.sector_size() - BLOCK_HEADER) as i32
        }
    };
    if !needs_rewrite {
        let mut remaining = file_size;
        let mut data = buf;
        let mut i = 0;
        while i < blocks && remaining > 0 {
            let chunk = chunk_of(before + i);
            if remaining > chunk {
                verify(
                    ring,
                    state,
                    primary[i as usize],
                    before + i,
                    seq,
                    data,
                    chunk,
                );
                remaining -= chunk;
                data = data.add(chunk as u32);
            } else {
                verify(
                    ring,
                    state,
                    primary[i as usize],
                    before + i,
                    seq,
                    data,
                    remaining,
                );
                remaining = 0;
            }
            i += 1;
        }
        ring.push(Cmd::CheckVerified);
    }
    while let Some(p) = secondary.pop() {
        if write(ring, state, p, 0xFFFF, 0, None, 0, file_idx) < 0 {
            ring.rewind(snap);
            return -0x101;
        }
    }
    let seq = (seq + 1) & 0xFF;
    let mut remaining = file_size;
    let mut data = buf;
    let mut i = 0;
    while i < blocks && remaining > 0 {
        let id = before + i;
        let chunk = chunk_of(id);
        let size = if remaining > chunk { chunk } else { remaining };
        if write(
            ring,
            state,
            primary[i as usize],
            id,
            seq,
            Some(data),
            size,
            file_idx,
        ) < 0
        {
            ring.rewind(snap);
            return -0x101;
        }
        if remaining > chunk {
            remaining -= chunk;
            data = data.add(chunk as u32);
        } else {
            remaining = 0;
        }
        i += 1;
    }
    ring.active = Some(Active {
        kind: ActiveKind::WriteFile3,
        arg: file_idx,
    });
    0
}

fn set_at(v: &mut Vec<i32>, i: usize, x: i32) {
    if i >= v.len() {
        v.resize(i + 1, -1);
    }
    v[i] = x;
}

/// Writes the header (`fn_803B0E9C`): for an existing file, verifies each header sector first;
/// the last sector, which block 0 shares, is cleared for a new file and loaded for an old one.
#[doc(alias = "fn_803B0E9C")]
pub fn write_header(
    ring: &mut Ring,
    state: &mut CardState,
    banner: Option<Ptr>,
    icons: Option<Ptr>,
    is_new: bool,
) -> i32 {
    state.set_header_size(state.icon_info().header_size());
    let sectors = state.header_sectors() as i32;
    if !is_new {
        for index in 0..sectors {
            ring.push(Cmd::VerifyHeader {
                index,
                banner,
                icons,
            });
        }
        ring.push(Cmd::CheckVerified);
    }
    for index in 0..sectors {
        if index + 1 == sectors {
            if is_new {
                ring.push(Cmd::ClearBuf {
                    data: Ptr::at(Buf::SectorBuf, 0),
                    size: state.sector_size() as i32,
                });
            } else {
                ring.push(Cmd::ReadBlock {
                    phys: -1,
                    data: None,
                    offset: index * state.sector_size() as i32,
                    size: i32::from(state.file_size(0) > 0),
                });
            }
        }
        ring.push(Cmd::WriteHeader {
            index,
            banner,
            icons,
        });
    }
    0
}

/// A new file's blocks (`fn_803B1338`): file 0 from block 0, then each file's first copy in
/// order, the spares (whose file table names file 9), then the second copies of files kept
/// twice; every sequence number 0. A file with no data gets blocks with none.
#[doc(alias = "fn_803B1338")]
pub fn write_new_blocks(ring: &mut Ring, state: &CardState) -> i32 {
    let data_of = |i: usize| (state.file_data(i) != 0).then_some(Ptr::at(Buf::File(i as u8), 0));
    let chunk = (state.sector_size() - BLOCK_HEADER) as i32;
    let mut spare = 0;
    let mut block_id = 1;
    let mut phys = 1;
    let first = state.file_blocks(0) as i32;
    if first != 0 {
        let hdr = state.first_block_offset() as i32;
        let ofs0 = state.block_offset(0);
        ring.push(read_init(ofs0));
        let mut offset = 0;
        match data_of(0) {
            None => ring.push(Cmd::WriteBlock {
                file_idx: 0,
                phys: 0,
                block_id: 0,
                seq: 0,
                data: None,
                offset: ofs0,
                size: 0,
            }),
            Some(d) => {
                let size = state.sector_size() as i32 - hdr - BLOCK_HEADER as i32;
                ring.push(Cmd::WriteBlock {
                    file_idx: 0,
                    phys: 0,
                    block_id: 0,
                    seq: 0,
                    data: Some(d),
                    offset: ofs0,
                    size,
                });
                offset = size;
            }
        }
        for _ in 1..first {
            let r = match data_of(0) {
                None => write(ring, state, phys, block_id, 0, None, 0, 0),
                Some(d) => {
                    let r = write(
                        ring,
                        state,
                        phys,
                        block_id,
                        0,
                        Some(d.add(offset as u32)),
                        chunk,
                        0,
                    );
                    offset += chunk;
                    r
                }
            };
            if r < 0 {
                return r;
            }
            phys += 1;
            block_id += 1;
        }
    }
    let mut index = 1usize;
    while index < 9 {
        let blocks = if state.file_size(index) <= 0 {
            0
        } else {
            state.file_blocks(index) as i32
        };
        let mut offset = 0;
        if blocks > 0 {
            for _ in 0..blocks {
                let r = match data_of(index) {
                    None => write(ring, state, phys, block_id, 0, None, 0, index as i32),
                    Some(d) => {
                        let r = write(
                            ring,
                            state,
                            phys,
                            block_id,
                            0,
                            Some(d.add(offset as u32)),
                            chunk,
                            index as i32,
                        );
                        offset += chunk;
                        r
                    }
                };
                if r < 0 {
                    return r;
                }
                phys += 1;
                block_id += 1;
            }
            match state.file_flags(index) {
                1 => spare = spare.max(blocks),
                2 => spare = spare.max(1),
                _ => {}
            }
        }
        index += 1;
    }
    for _ in 0..spare {
        let r = write(ring, state, phys, 0xFFFF, 0, None, 0, index as i32);
        if r < 0 {
            return r;
        }
        block_id += 1;
        phys += 1;
    }
    let _ = block_id;
    let mut id = 1;
    if first != 0 {
        id += first - 1;
    }
    for file in 1..9 {
        let blocks = state.file_blocks(file) as i32;
        let mut offset = 0;
        if blocks <= 0 {
            continue;
        }
        match state.file_flags(file) {
            0 => {
                for _ in 0..blocks {
                    let r = match data_of(file) {
                        None => write(ring, state, phys, id, 0, None, 0, file as i32),
                        Some(d) => {
                            let r = write(
                                ring,
                                state,
                                phys,
                                id,
                                0,
                                Some(d.add(offset as u32)),
                                chunk,
                                file as i32,
                            );
                            offset += chunk;
                            r
                        }
                    };
                    if r < 0 {
                        return r;
                    }
                    phys += 1;
                    id += 1;
                }
            }
            1..=3 => id += blocks,
            _ => {}
        }
    }
    0
}

/// Creates the file (`fn_803B1F78`): the card file of the size its blocks need, the header,
/// the blocks, then its status.
#[doc(alias = "fn_803B1F78")]
pub fn create(
    ring: &mut Ring,
    state: &mut CardState,
    banner: Option<Ptr>,
    icons: Option<Ptr>,
) -> i32 {
    let snap = ring.snap();
    let size = state.sector_size() as i32 * state.sectors_needed();
    ring.push(Cmd::CreateFile {
        filename: Ptr::at(Buf::Filename, 0),
        size,
    });
    write_header(ring, state, banner, icons, true);
    let r = write_new_blocks(ring, state);
    if r < 0 {
        ring.rewind(snap);
        return r;
    }
    ring.push(Cmd::SetStatus);
    ring.active = Some(Active {
        kind: ActiveKind::CreateFile,
        arg: 0,
    });
    0
}

/// Sets the file's status (`fn_803B21E8`): the header rewritten if it differs, then the
/// status.
#[doc(alias = "fn_803B21E8")]
pub fn set_status(
    ring: &mut Ring,
    state: &mut CardState,
    banner: Option<Ptr>,
    icons: Option<Ptr>,
) -> i32 {
    write_header(ring, state, banner, icons, false);
    ring.push(Cmd::SetStatus);
    ring.active = Some(Active {
        kind: ActiveKind::SetStatus,
        arg: 0,
    });
    0
}

/// The repair after a scan (`fn_803AD16C`): picks each file's newest whole set of blocks,
/// marks the other copies of files kept once stale (their ids negated), and for files kept
/// twice clears surplus copies and queues a copy of each block whose mirror is missing or
/// behind. Results: -257 when the files' block count isn't the file's, -0x103 when a file has
/// a block missing, -0x104 when no set of its blocks is whole at one sequence number (the
/// newest of each is taken), -0x102 or -0x10B when a copy can't be queued.
#[doc(alias = "fn_803AD16C")]
pub fn repair(ring: &mut Ring, state: &mut CardState) -> i32 {
    let num = state.num_blocks();
    let mut result = 0;
    let len = (num.max(0) + 1) as usize;
    let mut newmap = vec![-1i32; 64.max(len)];
    let mut ids: Vec<i32> = (0..len).map(|p| state.block_id(p)).collect();
    let seq: Vec<i32> = (0..len).map(|p| state.block_seq(p)).collect();
    if num != state.total_blocks() {
        return -257;
    }
    for file in 0..9 {
        let blocks = state.file_blocks(file) as i32;
        if blocks <= 0 {
            continue;
        }
        let before = state.blocks_before(file);
        let n = blocks as usize;
        let mut filemap = vec![-1i32; n];
        let mut chosen = vec![-1i32; n];
        for _pass in 0..=num {
            let mut cur = -1;
            for phys in 0..len {
                if ids[phys] >= 0 {
                    let logical = ids[phys] - before;
                    if (0..blocks).contains(&logical) && (cur == -1 || seq_cmp(cur, seq[phys]) > 0)
                    {
                        cur = seq[phys];
                    }
                }
            }
            if cur < 0 {
                break;
            }
            for phys in 0..len {
                let logical = ids[phys] - before;
                if (0..blocks).contains(&logical) && cur == seq[phys] {
                    filemap[logical as usize] = phys as i32;
                }
            }
            if filemap.iter().all(|&p| p >= 0 && cur == seq[p as usize]) {
                chosen.copy_from_slice(&filemap);
            }
            for &p in &filemap {
                if p >= 0 {
                    ids[p as usize] = FREE;
                }
            }
        }
        if chosen[0] >= 0 {
            for i in 0..n {
                newmap[before as usize + i] = chosen[i];
            }
        } else {
            for i in 0..n {
                if filemap[i] < 0 {
                    result = -0x103;
                } else {
                    newmap[before as usize + i] = filemap[i];
                    if result != -0x103 {
                        result = -0x104;
                    }
                }
            }
        }
    }
    for file in 0..9 {
        if state.file_size(file) <= 0 || !matches!(state.file_flags(file), 1 | 2) {
            continue;
        }
        let blocks = state.file_blocks(file) as i32;
        let before = state.blocks_before(file);
        for i in 0..blocks {
            let id = before + i;
            let keep = newmap[id as usize];
            if keep >= 0 {
                for phys in 1..=num {
                    if phys != keep && state.block_id(phys as usize) == id {
                        state.set_block_id(phys as usize, -id);
                    }
                }
            }
        }
    }
    for file in 0..9 {
        if state.file_size(file) <= 0 || state.file_flags(file) != 0 {
            continue;
        }
        let blocks = state.file_blocks(file) as i32;
        let before = state.blocks_before(file);
        for i in 0..blocks {
            let id = before + i;
            let src = newmap[id as usize];
            if src < 0 {
                continue;
            }
            let target_seq = state.block_seq(src as usize);
            let mut dup = -1;
            for phys in 1..=num {
                if phys != src && state.block_id(phys as usize) == id {
                    if dup < 0 {
                        dup = phys;
                    } else {
                        let offset = state.block_offset(phys);
                        ring.push(Cmd::WriteBlock {
                            file_idx: file as i32,
                            phys,
                            block_id: 0xFFFF,
                            seq: 0,
                            data: None,
                            offset,
                            size: 0,
                        });
                        state.set_block_id(phys as usize, FREE);
                        state.set_block_seq(phys as usize, 0);
                    }
                }
            }
            let copy_needed = if dup == -1 {
                dup = (1..=num)
                    .rev()
                    .find(|&p| state.block_id(p as usize) < 0)
                    .unwrap_or(-1);
                dup >= 0
            } else {
                state.block_seq(dup as usize) != target_seq
            };
            if copy_needed {
                if src == 0 || dup == 0 {
                    if result == 0 || result == -0x10B {
                        result = -0x102;
                    }
                } else {
                    ring.push(Cmd::ReadSector {
                        phys: src,
                        offset: state.block_offset(src),
                    });
                    ring.push(Cmd::WriteSector {
                        phys: dup,
                        block_id: id,
                        seq: target_seq,
                        offset: state.block_offset(dup),
                    });
                }
            }
        }
    }
    result
}
