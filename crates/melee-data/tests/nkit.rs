//! An NKit image of the player's disc, restored ([`melee_data::nkit`]): the restored disc equals
//! the disc byte for byte, so every file and every read's tail past a file's end (`read_after`)
//! is the disc's; the RVZ the player converts it to is checked and plays as the disc; the image
//! inside GCZ restores the same; the image itself is refused as a disc; and a modified or
//! truncated image is refused.
//!
//! `SSBM_NKIT` names the NKit image, made from `SSBM_DISC` with NKit's own tool (`nkit convert
//! DISC --format nkitiso`, NKit 3.2.2 for the image these were written against). Neither is in the
//! repository: the tests note it and pass without `SSBM_NKIT` (an image few machines have, so
//! `MELEE_CHECKS_REQUIRE_DATA` doesn't ask for it), and with it they need `SSBM_DISC`.

use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use melee_data::disc::{Disc, DiscError};
use melee_data::nkit::{self, NKitError, Restored};
use nod::common::{Compression, Format, PartitionKind};
use nod::read::{DiscOptions, DiscReader, DiscStream, PartitionOptions};
use nod::write::{DiscWriter, FormatOptions, ProcessOptions};

/// The NKit image and the disc it was made from.
fn paths() -> Option<(PathBuf, PathBuf)> {
    let Ok(nkit) = std::env::var("SSBM_NKIT") else {
        eprintln!("no SSBM_NKIT (an NKit image of SSBM_DISC): skipping");
        return None;
    };
    let Ok(disc) = std::env::var("SSBM_DISC") else {
        melee_local_data::skip!("SSBM_NKIT without SSBM_DISC: skipping");
        return None;
    };
    Some((nkit.into(), disc.into()))
}

/// Compares the disc's bytes with `read`'s, a MiB at a time.
fn same_bytes(disc: &PathBuf, size: u64, mut read: impl FnMut(&mut [u8], u64)) {
    let mut iso = std::fs::File::open(disc).unwrap();
    assert_eq!(
        iso.metadata().unwrap().len(),
        size,
        "the restored disc's size"
    );
    let mut a = vec![0u8; 1 << 20];
    let mut b = vec![0u8; 1 << 20];
    let mut at = 0;
    while at < size {
        let n = ((size - at) as usize).min(a.len());
        iso.read_exact(&mut a[..n]).unwrap();
        read(&mut b[..n], at);
        if a[..n] != b[..n] {
            let i = a[..n]
                .iter()
                .zip(&b[..n])
                .position(|(x, y)| x != y)
                .unwrap();
            panic!(
                "the restored disc differs from the disc at {:#x}",
                at + i as u64
            );
        }
        at += n as u64;
    }
}

#[test]
fn the_restored_image_is_the_disc_byte_for_byte() {
    let Some((nkit, disc)) = paths() else {
        return;
    };
    let mut r = Restored::open(&nkit).unwrap();
    let size = r.size();
    same_bytes(&disc, size, |buf, at| r.read_at(buf, at).unwrap());
    r.verify().unwrap();
}

#[test]
fn every_file_and_its_tail_are_the_discs() {
    let Some((nkit, disc)) = paths() else {
        return;
    };
    let iso = Disc::open(&disc).unwrap();
    let restored = Disc::restored(Restored::open(&nkit).unwrap()).unwrap();
    assert!(iso.dol().unwrap() == restored.dol().unwrap(), "main.dol");
    let files = iso.files().unwrap();
    assert_eq!(files, restored.files().unwrap());
    assert_eq!(iso.entries().unwrap(), restored.entries().unwrap());
    for f in &files {
        assert_eq!(
            iso.file_info(f).unwrap(),
            restored.file_info(f).unwrap(),
            "{f}"
        );
        assert_eq!(
            iso.file_offset(f).unwrap(),
            restored.file_offset(f).unwrap(),
            "{f}"
        );
        assert!(
            iso.read_file(f).unwrap() == restored.read_file(f).unwrap(),
            "{f}"
        );
        assert_eq!(
            iso.read_after(f).unwrap(),
            restored.read_after(f).unwrap(),
            "{f}"
        );
    }
    eprintln!("{} files the same", files.len());
}

#[test]
fn the_image_itself_isnt_played_as_a_disc() {
    let Some((nkit, _)) = paths() else {
        return;
    };
    assert!(matches!(Disc::open(&nkit), Err(DiscError::NKit)));
}

#[test]
fn the_rvz_is_checked_and_plays_as_the_disc() {
    let Some((nkit, disc)) = paths() else {
        return;
    };
    let dir = std::env::temp_dir().join(format!("melee-data-nkit-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("restored.rvz");
    let start = std::time::Instant::now();
    nkit::restore(&nkit, &out, |_, _| {}).unwrap();
    eprintln!("restored in {:.1} s", start.elapsed().as_secs_f64());
    let result = std::panic::catch_unwind(|| {
        assert!(!dir.join("restored.rvz.part").exists());
        Disc::open(&out).unwrap();
        let rvz = DiscReader::new(&out, &DiscOptions::default()).unwrap();
        let mut r = rvz.clone();
        same_bytes(&disc, rvz.disc_size(), |buf, at| {
            r.seek(SeekFrom::Start(at)).unwrap();
            r.read_exact(buf).unwrap();
        });
        drop((rvz, r));
        // The RVZ's own check, which a byte changed in what was written fails.
        let recorded = Restored::open(&nkit).unwrap().header().crc32;
        nkit::verify_file(&out, recorded).unwrap();
        let mut f = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&out)
            .unwrap();
        let at = f.metadata().unwrap().len() / 2;
        let mut b = [0u8; 1];
        f.seek(SeekFrom::Start(at)).unwrap();
        f.read_exact(&mut b).unwrap();
        f.seek(SeekFrom::Start(at)).unwrap();
        f.write_all(&[b[0] ^ 0x01]).unwrap();
        drop(f);
        assert!(nkit::verify_file(&out, recorded).is_err());
    });
    let _ = std::fs::remove_dir_all(&dir);
    if let Err(p) = result {
        std::panic::resume_unwind(p);
    }
}

/// An NKit image inside GCZ, which NKit 1 wrote as `.nkit.gcz` (NKit 3 writes none): the image
/// compressed with nod's GCZ writer restores to the disc as the image does.
#[test]
fn an_nkit_gcz_restores_too() {
    let Some((nkit, _)) = paths() else {
        return;
    };
    let dir = std::env::temp_dir().join(format!("melee-data-nkit-gcz-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let gcz = dir.join("image.nkit.gcz");
    let result = std::panic::catch_unwind(|| {
        let image = DiscReader::new(&nkit, &DiscOptions::default()).unwrap();
        let options = FormatOptions {
            format: Format::Gcz,
            compression: Compression::Deflate(1),
            block_size: Format::Gcz.default_block_size(),
        };
        let writer = DiscWriter::new(image, &options).unwrap();
        let mut file = std::fs::File::create(&gcz).unwrap();
        let done = writer
            .process(
                |data, _, _| file.write_all(&data),
                &ProcessOptions {
                    processor_threads: 4,
                    ..Default::default()
                },
            )
            .unwrap();
        if !done.header.is_empty() {
            file.seek(SeekFrom::Start(0)).unwrap();
            file.write_all(&done.header).unwrap();
        }
        drop(file);
        let mut r = Restored::open(&gcz).unwrap();
        assert_eq!(r.header(), Restored::open(&nkit).unwrap().header());
        r.verify().unwrap();
        assert!(matches!(Disc::open(&gcz), Err(DiscError::NKit)));
    });
    let _ = std::fs::remove_dir_all(&dir);
    if let Err(p) = result {
        std::panic::resume_unwind(p);
    }
}

/// The NKit image's file with one byte changed, or cut short.
#[derive(Clone)]
struct Altered {
    file: Arc<Mutex<std::fs::File>>,
    len: u64,
    flip: Option<u64>,
}

impl DiscStream for Altered {
    fn read_exact_at(&mut self, buf: &mut [u8], offset: u64) -> std::io::Result<()> {
        if offset + buf.len() as u64 > self.len {
            return Err(std::io::ErrorKind::UnexpectedEof.into());
        }
        let mut f = self.file.lock().unwrap();
        f.seek(SeekFrom::Start(offset))?;
        f.read_exact(buf)?;
        if let Some(at) = self.flip
            && (offset..offset + buf.len() as u64).contains(&at)
        {
            buf[(at - offset) as usize] ^= 0x01;
        }
        Ok(())
    }

    fn stream_len(&mut self) -> std::io::Result<u64> {
        Ok(self.len)
    }
}

fn altered(nkit: &PathBuf, len: Option<u64>, flip: Option<u64>) -> Result<Restored, NKitError> {
    let file = std::fs::File::open(nkit).unwrap();
    let full = file.metadata().unwrap().len();
    let stream = Altered {
        file: Arc::new(Mutex::new(file)),
        len: len.unwrap_or(full),
        flip,
    };
    Restored::new(DiscReader::new_stream(
        Box::new(stream),
        &DiscOptions::default(),
    )?)
}

/// Where a file's data starts in the NKit image (its own file system table's offset).
fn in_image(nkit: &PathBuf, name: &str) -> u64 {
    let image = DiscReader::new(nkit, &DiscOptions::default()).unwrap();
    let mut p = image
        .open_partition_kind(PartitionKind::Data, &PartitionOptions::default())
        .unwrap();
    let meta = p.meta().unwrap();
    let fst = meta.fst().unwrap();
    let (_, node) = fst.find(name).unwrap();
    node.offset(false)
}

#[test]
fn a_modified_image_is_refused() {
    let Some((nkit, _)) = paths() else {
        return;
    };
    let at = in_image(&nkit, "PlFxNr.dat") + 0x100;
    let mut r = altered(&nkit, None, Some(at)).unwrap();
    match r.verify() {
        Err(NKitError::Mismatch { found, recorded }) => {
            eprintln!("refused: CRC32 {found:08X}, recorded {recorded:08X}")
        }
        other => panic!("a modified image: {other:?}"),
    }
    // Its header changed: the CRC32 NKit recorded.
    let mut r = altered(&nkit, None, Some(0x20B)).unwrap();
    assert!(matches!(r.verify(), Err(NKitError::Mismatch { .. })));
}

/// A modified image's restored disc fails the check as it's converted: no RVZ is left, nor its
/// part.
#[test]
fn a_modified_image_isnt_converted() {
    let Some((nkit, _)) = paths() else {
        return;
    };
    let at = in_image(&nkit, "PlFxNr.dat") + 0x100;
    let dir = std::env::temp_dir().join(format!("melee-data-nkit-bad-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("restored.rvz");
    let result = altered(&nkit, None, Some(at))
        .unwrap()
        .write_rvz(&out, |_, _| {});
    let left: Vec<_> = std::fs::read_dir(&dir).unwrap().collect();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        matches!(result, Err(NKitError::Mismatch { .. })),
        "{result:?}"
    );
    assert!(left.is_empty(), "left behind: {left:?}");
}

#[test]
fn a_truncated_image_is_refused() {
    let Some((nkit, _)) = paths() else {
        return;
    };
    let full = std::fs::metadata(&nkit).unwrap().len();
    for len in [full / 2, full - 0x10_0000] {
        match altered(&nkit, Some(len), None) {
            Err(NKitError::Truncated) => {}
            Err(e) => panic!("cut to {len:#x}: {e}"),
            Ok(mut r) => assert!(
                matches!(r.verify(), Err(NKitError::Mismatch { .. })),
                "cut to {len:#x}"
            ),
        }
    }
}
