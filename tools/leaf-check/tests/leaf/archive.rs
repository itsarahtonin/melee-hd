//! melee-data's disc reader and archive parser against ssbm-rs's: every DAT and USD file on
//! the disc reads the same bytes, and parses to the same pointers, publics and externs as
//! ssbm-rs's port of `HSD_ArchiveParse` leaves in memory.

use leaf_check::{handle, reference};
use melee_data::{archive::Archive, disc::Disc};
use ssbm_rt::Handle;
use ssbm_types::fns;
use ssbm_types::records::HSD_Archive;

const ARCHIVE: u32 = 0x8070_0000;
const NAME: u32 = 0x8070_0100;
const FILE: u32 = 0x8080_0000;
const SENTINEL: u32 = 0x8123_4560;
const FILE_MAX: usize = 0x00F0_0000;

#[test]
fn archives_parse_the_same() {
    let Some(ctx) = reference() else { return };
    let path = std::env::var("SSBM_DISC").unwrap();
    let ours = Disc::open(&path).unwrap();
    let theirs = ssbm_disc::Disc::open(&path).unwrap();
    let files = ours.files().unwrap();
    let (mut checked, mut refused, mut unaligned, mut externs) = (0, 0, 0, 0);
    for f in files
        .iter()
        .filter(|f| f.ends_with(".dat") || f.ends_with(".usd"))
    {
        if std::env::var_os("ARCHIVE_TRACE").is_some() {
            eprintln!("{f}");
        }
        let bytes = ours.read_file(f).unwrap();
        assert_eq!(bytes, theirs.read_file(f).unwrap(), "{f}: bytes");
        let archive = match Archive::parse(&bytes) {
            Ok(a) => a,
            // Concatenated archives (animation files): the outer header gives only the first
            // one's size, and HSD_ArchiveParse refuses the whole file for the same reason.
            Err(melee_data::archive::ArchiveError::SizeMismatch { header, file }) => {
                assert!(
                    (header as usize) < file,
                    "{f}: header {header:#x} past the file"
                );
                refused += 1;
                continue;
            }
            Err(e) => panic!("{f}: {e}"),
        };
        if bytes.len() > FILE_MAX {
            continue;
        }
        ctx.write_bytes(FILE, &bytes);
        let r = fns::HSD_ArchiveParse(
            &ctx,
            handle::<HSD_Archive>(&ctx, ARCHIVE),
            handle(&ctx, FILE),
            bytes.len() as u32,
        );
        assert_eq!(r, 0, "{f}: ssbm-rs refused it");
        let data = ctx.read_u32(ARCHIVE + 0x20);
        let size = archive.data().len() as u32;
        // Bytes an unaligned relocation touches hold an address-dependent value there.
        let quirks: Vec<u32> = archive.unaligned_relocs().collect();
        if !quirks.is_empty() {
            eprintln!("{f}: unaligned relocations at {quirks:#x?}");
            unaligned += quirks.len();
        }
        // Every word: relocated words hold their target's address, the rest are untouched.
        for off in (0..size.saturating_sub(3)).step_by(4) {
            if quirks.iter().any(|&q| q < off + 4 && off < q + 4) {
                continue;
            }
            let word = ctx.read_u32(data + off);
            if archive.is_pointer(off) {
                assert_eq!(
                    archive.ptr(off),
                    Some(word.wrapping_sub(data)),
                    "{f}: pointer at {off:#x}"
                );
            } else {
                assert_eq!(archive.u32(off), word, "{f}: word at {off:#x}");
            }
        }
        for (name, offset) in archive.publics() {
            let mut c = name.as_bytes().to_vec();
            c.push(0);
            ctx.write_bytes(NAME, &c);
            let at =
                fns::HSD_ArchiveGetPublicAddress(&ctx, handle(&ctx, ARCHIVE), handle(&ctx, NAME))
                    .addr();
            assert_eq!(at.wrapping_sub(data), offset, "{f}: public {name}");
            assert_eq!(
                archive.public(name),
                Some(offset),
                "{f}: public {name} by name"
            );
        }
        let nb_extern = ctx.read_u32(ARCHIVE + 0x10);
        for i in 0..nb_extern {
            let at = fns::HSD_ArchiveGetExtern(&ctx, handle(&ctx, ARCHIVE), i as i32).addr();
            let mut name = Vec::new();
            let mut a = at;
            while ctx.read_u8(a) != 0 {
                name.push(ctx.read_u8(a));
                a += 1;
            }
            assert_eq!(
                archive.extern_name(i as usize),
                Some(String::from_utf8_lossy(&name).as_ref()),
                "{f}: extern {i}"
            );
            // The slots the extern's chain runs through get the symbol's address.
            let name = String::from_utf8_lossy(&name).into_owned();
            let before: Vec<u32> = (0..size / 4).map(|w| ctx.read_u32(data + 4 * w)).collect();
            let mut c = name.as_bytes().to_vec();
            c.push(0);
            ctx.write_bytes(NAME, &c);
            fns::HSD_ArchiveLocateExtern(
                &ctx,
                handle(&ctx, ARCHIVE),
                handle(&ctx, NAME),
                handle(&ctx, SENTINEL),
            );
            let written: Vec<u32> = (0..size / 4)
                .filter(|&w| before[w as usize] != ctx.read_u32(data + 4 * w))
                .map(|w| 4 * w)
                .collect();
            let mut ours = archive.extern_slots(&name);
            ours.sort_unstable();
            externs += 1;
            assert_eq!(ours, written, "{f}: slots of extern {name}");
        }
        checked += 1;
    }
    eprintln!(
        "{checked} archives compared, {refused} concatenated files left to their loaders, {unaligned} unaligned relocations, {externs} externs resolved"
    );
    assert!(checked > 500, "only {checked} archives compared");
}
