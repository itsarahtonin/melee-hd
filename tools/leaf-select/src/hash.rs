//! SHA-1, in hex: stable across builds and platforms, for keys that outlive one run.

use sha1::{Digest, Sha1};

pub fn hex(bytes: &[u8]) -> String {
    let mut h = Sha1::new();
    h.update(bytes);
    to_hex(&h.finalize())
}

fn to_hex(d: &[u8]) -> String {
    d.iter().map(|b| format!("{b:02x}")).collect()
}

/// A hash of labelled parts, each kept apart from the next.
#[derive(Default)]
pub struct Hasher(Sha1);

impl Hasher {
    pub fn part(&mut self, label: &str, bytes: &[u8]) -> &mut Self {
        for p in [label.as_bytes(), bytes] {
            self.0.update((p.len() as u64).to_le_bytes());
            self.0.update(p);
        }
        self
    }

    pub fn finish(self) -> String {
        to_hex(&self.0.finalize())
    }
}
