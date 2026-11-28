//! The dual-GPU hint: the Windows executable exports `NvOptimusEnablement` and
//! `AmdPowerXpressRequestHighPerformance`, each a DWORD holding 1, which NVIDIA's and AMD's
//! laptop drivers read from the export table to run the program on the discrete GPU. Read
//! back from the built executable's export table with a small PE reader.

#![cfg(all(windows, target_env = "msvc"))]

/// The executable's bytes and its sections (virtual address, virtual size, file offset, size
/// in the file).
struct Pe {
    bytes: Vec<u8>,
    sections: Vec<(u32, u32, u32, u32)>,
    exports: (u32, u32),
}

fn u16_at(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes(b[at..at + 2].try_into().unwrap())
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}

impl Pe {
    fn read(path: &str) -> Pe {
        let bytes = std::fs::read(path).expect("the built executable");
        assert_eq!(&bytes[..2], b"MZ", "{path} is a PE image");
        let pe = u32_at(&bytes, 0x3C) as usize;
        assert_eq!(&bytes[pe..pe + 4], b"PE\0\0");
        let coff = pe + 4;
        let count = usize::from(u16_at(&bytes, coff + 2));
        let optional = coff + 20;
        let optional_size = usize::from(u16_at(&bytes, coff + 16));
        assert_eq!(u16_at(&bytes, optional), 0x20B, "a PE32+ image");
        // PE32+'s data directories start 112 bytes in; the export table is the first.
        let exports = (
            u32_at(&bytes, optional + 112),
            u32_at(&bytes, optional + 116),
        );
        let first = optional + optional_size;
        let sections = (0..count)
            .map(|i| {
                let s = first + 40 * i;
                (
                    u32_at(&bytes, s + 12),
                    u32_at(&bytes, s + 8),
                    u32_at(&bytes, s + 20),
                    u32_at(&bytes, s + 16),
                )
            })
            .collect();
        Pe {
            bytes,
            sections,
            exports,
        }
    }

    /// The file offset of a virtual address that has bytes in the file.
    fn offset(&self, rva: u32) -> usize {
        let &(va, _, raw, _) = self
            .sections
            .iter()
            .find(|&&(va, vsize, _, rsize)| rva >= va && rva < va + vsize.max(rsize))
            .unwrap_or_else(|| panic!("{rva:#x} in a section"));
        (rva - va + raw) as usize
    }

    fn name(&self, rva: u32) -> String {
        let at = self.offset(rva);
        let end = self.bytes[at..].iter().position(|&b| b == 0).unwrap();
        String::from_utf8_lossy(&self.bytes[at..at + end]).into_owned()
    }

    /// Each export by name, with the address it names.
    fn exports(&self) -> Vec<(String, u32)> {
        let (dir, size) = self.exports;
        if size == 0 {
            return Vec::new();
        }
        let d = self.offset(dir);
        let b = &self.bytes;
        let names = u32_at(b, d + 24) as usize;
        let (functions, name_table, ordinals) = (
            self.offset(u32_at(b, d + 28)),
            self.offset(u32_at(b, d + 32)),
            self.offset(u32_at(b, d + 36)),
        );
        (0..names)
            .map(|i| {
                let name = self.name(u32_at(b, name_table + 4 * i));
                let ordinal = usize::from(u16_at(b, ordinals + 2 * i));
                (name, u32_at(b, functions + 4 * ordinal))
            })
            .collect()
    }
}

#[test]
fn the_executable_asks_for_the_discrete_gpu() {
    let pe = Pe::read(env!("CARGO_BIN_EXE_melee-hd"));
    let exports = pe.exports();
    for name in [
        "NvOptimusEnablement",
        "AmdPowerXpressRequestHighPerformance",
    ] {
        let &(_, rva) = exports
            .iter()
            .find(|(n, _)| n == name)
            .unwrap_or_else(|| panic!("{name} isn't exported: {exports:?}"));
        let at = pe.offset(rva);
        assert_eq!(u32_at(&pe.bytes, at), 1, "{name}'s value");
    }
}
