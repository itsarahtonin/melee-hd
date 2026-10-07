//! Corpora of saved calls (`CORPUS=FILE` in ssbm-rs's runner): each distinct page of memory
//! once, compressed, then each call's registers and the pages its memory is made of. Far
//! smaller than a file per call when the calls share most of their memory.

use std::path::Path;

use ssbm_rt::capture::Call;

use crate::Capture;

const MAGIC: &[u8; 8] = b"SSBMCORP";
const PAGE: usize = 0x1000;

/// A corpus read back: its pages still compressed, its calls without their memory.
pub struct Corpus {
    pages: Vec<Vec<u8>>,
    calls: Vec<(Call, usize, Vec<u32>)>,
}

impl Corpus {
    pub fn read(path: &Path) -> Self {
        let b = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert!(b.starts_with(MAGIC), "{}: not a corpus", path.display());
        let (mut pages, mut calls) = (Vec::new(), Vec::new());
        let mut at = MAGIC.len();
        // A record cut short ends the corpus, as one a run stopped while writing.
        while at + 5 <= b.len() {
            let kind = b[at];
            let len = u32::from_le_bytes(b[at + 1..at + 5].try_into().unwrap()) as usize;
            let Some(packed) = b.get(at + 5..at + 5 + len) else {
                break;
            };
            at += 5 + len;
            if kind == 1 {
                pages.push(packed.to_vec());
                continue;
            }
            let payload = zstd::decode_all(packed).expect("zstd");
            let n = u32::from_le_bytes(payload[..4].try_into().unwrap()) as usize;
            let head = Call::from_bytes(&payload[4..4 + n]).expect("a saved call");
            let mem1 = u32::from_le_bytes(payload[4 + n..8 + n].try_into().unwrap()) as usize;
            let ids = payload[8 + n..]
                .chunks(4)
                .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
                .collect();
            calls.push((head, mem1, ids));
        }
        Self { pages, calls }
    }

    pub fn len(&self) -> usize {
        self.calls.len()
    }

    pub fn is_empty(&self) -> bool {
        self.calls.is_empty()
    }

    /// The function call `i` entered.
    pub fn addr(&self, i: usize) -> u32 {
        self.calls[i].0.addr
    }

    /// Call `i` with its memory.
    pub fn capture(&self, i: usize) -> Capture {
        let (head, mem1_len, ids) = &self.calls[i];
        let mut bytes = Vec::with_capacity(ids.len() * PAGE);
        for &id in ids {
            let page = zstd::decode_all(&self.pages[id as usize][..]).expect("zstd");
            bytes.extend_from_slice(&page);
        }
        let locked = bytes.split_off(*mem1_len);
        Capture {
            call: Call {
                mem1: bytes,
                locked,
                regs: head.regs.clone(),
                ..*head
            },
        }
    }
}
